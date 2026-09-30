//! Small readback guards for SSAO blur algebra.
//!
//! This module stays registered below three_d::gpu so it can share the
//! renderer wgpu test setup. The reference shader comes from shipped
//! source with center-tap reuse reversed in-place; this keeps drift visible.

use super::*;
use bytemuck::{Pod, Zeroable};
use std::sync::mpsc;

const SSAO_BLUR: &str = include_str!("../../src/three_d/shaders/ssao_bilateral_blur.wgsl");

const DEPTH_FIXTURE_SHADER: &str = r#"
@group(0) @binding(0) var depth_values: texture_2d<f32>;

struct VsOut { @builtin(position) pos: vec4<f32> };

@vertex fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: VsOut;
    out.pos = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    return out;
}

@fragment fn fs_main(@builtin(position) p: vec4<f32>) -> @builtin(frag_depth) f32 {
    let size = vec2<i32>(textureDimensions(depth_values));
    let pixel = clamp(vec2<i32>(p.xy), vec2<i32>(0), size - vec2<i32>(1));
    return textureLoad(depth_values, pixel, 0).r;
}
"#;

const OLD_SSAO_MAIN: &str = r#"fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) f32 {
    let divisor = i32(max(params.target_divisor, 1u));
    let target_size = vec2<i32>(textureDimensions(ao_tex));
    let pixel = clamp(vec2<i32>(frag.xy), vec2<i32>(0), target_size - vec2<i32>(1));
    let center_full = full_pixel(pixel, divisor);
    let center_depth = textureLoad(depth_tex, center_full, 0);
    if center_depth >= 0.999999 {
        return 1.0;
    }
    let center_distance = view_distance(center_full, center_depth);
    var sum = 0.0;
    var weight_sum = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let q = clamp(pixel + vec2<i32>(x, y), vec2<i32>(0), target_size - vec2<i32>(1));
            let q_full = full_pixel(q, divisor);
            let q_depth = textureLoad(depth_tex, q_full, 0);
            if q_depth < 0.999999 {
                let q_distance = view_distance(q_full, q_depth);
                let relative_depth = abs(q_distance - center_distance) / max(center_distance, 1.0e-3);
                let spatial = exp(-0.75 * f32(x * x + y * y));
                let edge = exp(-relative_depth * params.depth_sigma * 0.33);
                let weight = spatial * edge;
                sum += textureLoad(ao_tex, q, 0).r * weight;
                weight_sum += weight;
            }
        }
    }
    return sum / max(weight_sum, 1.0e-5);
}"#;

fn replace_function(source: &str, name: &str, replacement: &str) -> String {
    let marker = format!("fn {name}");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("missing WGSL function {name}"));
    let open = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .expect("WGSL function body");
    let mut depth = 0usize;
    let mut end = None;
    for (offset, byte) in source.as_bytes()[open..].iter().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(open + offset + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end.expect("WGSL function close");
    format!("{}{}{}", &source[..start], replacement, &source[end..])
}

async fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        })
        .await
        .ok()?;
    adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("perro_shader_equivalence_test_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        })
        .await
        .ok()
}

async fn timestamp_test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        })
        .await
        .ok()?;
    let info = adapter.get_info();
    eprintln!("shader timestamp adapter: {info:?}");
    let features =
        wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
    if !adapter.features().contains(features) {
        return None;
    }
    adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("perro_shader_timestamp_test_device"),
            required_features: features,
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        })
        .await
        .ok()
}

struct TimestampProbe {
    query_set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    period_ns: f32,
}

impl TimestampProbe {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            query_set: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("perro_shader_timestamp_queries"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_shader_timestamp_resolve"),
                size: 16,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readback: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_shader_timestamp_readback"),
                size: 16,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
            period_ns: queue.get_timestamp_period(),
        }
    }

    fn read(&self, device: &wgpu::Device) -> Option<f64> {
        let slice = self.readback.slice(..);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        rx.recv().ok()?.ok()?;
        let mapped = slice.get_mapped_range().ok()?;
        let values: &[u64] = bytemuck::cast_slice(&mapped);
        let elapsed = values
            .first()
            .zip(values.get(1))
            .filter(|(start, end)| **end >= **start)
            .map(|(start, end)| (*end - *start) as f64 * f64::from(self.period_ns));
        drop(mapped);
        self.readback.unmap();
        elapsed
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlurParams {
    inv_view_proj: [[f32; 4]; 4],
    full_size: [f32; 2],
    radius_px: f32,
    strength: f32,
    depth_sigma: f32,
    sample_count: u32,
    target_divisor: u32,
    _pad: f32,
}

fn old_ssao_source() -> String {
    replace_function(SSAO_BLUR, "fs_main", OLD_SSAO_MAIN)
}

fn texture_bytes_f32(values: &[f32]) -> &[u8] {
    bytemuck::cast_slice(values)
}

fn write_texture_f32(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    values: &[f32],
    width: u32,
    height: u32,
) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        texture_bytes_f32(values),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

fn write_texture_rgba8(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    values: &[u8],
    width: u32,
    height: u32,
) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        values,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

fn readback_bytes_per_row(width: u32) -> u32 {
    (width * 4).div_ceil(256) * 256
}

fn upload_depth_fixture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    depth: &wgpu::Texture,
    depth_values: &[f32],
    width: u32,
    height: u32,
) {
    let source = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shader_equiv_ssao_depth_source"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    write_texture_f32(queue, &source, depth_values, width, height);
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shader_equiv_ssao_depth_upload_module"),
        source: wgpu::ShaderSource::Wgsl(DEPTH_FIXTURE_SHADER.into()),
    });
    let pipeline = crate::pipeline_cache::create_render_pipeline(
        device,
        wgpu::RenderPipelineDescriptor {
            label: Some("shader_equiv_ssao_depth_upload_pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        },
    );
    let source_view = source.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("shader_equiv_ssao_depth_upload_bind_group"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&source_view),
        }],
    });
    let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("shader_equiv_ssao_depth_upload_encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("shader_equiv_ssao_depth_upload_pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
    queue.submit(Some(encoder.finish()));
}

struct SsaoBlurFixture<'a> {
    source: &'a str,
    divisor: u32,
    sample_count: u32,
    width: u32,
    height: u32,
}

impl<'a> SsaoBlurFixture<'a> {
    fn new(source: &'a str, divisor: u32, sample_count: u32, width: u32, height: u32) -> Self {
        Self {
            source,
            divisor,
            sample_count,
            width,
            height,
        }
    }
}

async fn render_ssao_blur(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    fixture: SsaoBlurFixture<'_>,
    timestamps: Option<&TimestampProbe>,
) -> Vec<f32> {
    let SsaoBlurFixture {
        source,
        divisor,
        sample_count,
        width,
        height,
    } = fixture;
    let full_width = width * divisor;
    let full_height = height * divisor;
    let ao_values: Vec<u8> = (0..width * height)
        .flat_map(|i| {
            let value = ((0.15 + i as f32 * 0.017) * 255.0).round() as u8;
            [value, 0, 0, 255]
        })
        .collect();
    let mut depth_values = vec![0.4f32; (full_width * full_height) as usize];
    // Sky corners exercise early return; finite edge values exercise clamp.
    for y in [0, height - 1] {
        for x in [0, width - 1] {
            let full_x = (x * divisor + divisor / 2).min(full_width - 1);
            let full_y = (y * divisor + divisor / 2).min(full_height - 1);
            depth_values[(full_y * full_width + full_x) as usize] = 1.0;
        }
    }
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shader_equiv_ssao_module"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let ao = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shader_equiv_ssao_ao"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shader_equiv_ssao_depth"),
        size: wgpu::Extent3d {
            width: full_width,
            height: full_height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    write_texture_rgba8(queue, &ao, &ao_values, width, height);
    upload_depth_fixture(
        device,
        queue,
        &depth,
        &depth_values,
        full_width,
        full_height,
    );
    let output = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shader_equiv_ssao_output"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let params = BlurParams {
        inv_view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
        full_size: [full_width as f32, full_height as f32],
        radius_px: 2.0,
        strength: 1.0,
        depth_sigma: 200.0,
        sample_count,
        target_divisor: divisor,
        _pad: 0.0,
    };
    let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("shader_equiv_ssao_params"),
        contents: bytemuck::bytes_of(&params),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let ao_view = ao.create_view(&wgpu::TextureViewDescriptor::default());
    let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
    let pipeline = crate::pipeline_cache::create_render_pipeline(
        device,
        wgpu::RenderPipelineDescriptor {
            label: Some("shader_equiv_ssao_pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::R32Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        },
    );
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("shader_equiv_ssao_bind_group"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&ao_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&depth_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params.as_entire_binding(),
            },
        ],
    });
    let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("shader_equiv_ssao_readback"),
        size: (readback_bytes_per_row(width) * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("shader_equiv_ssao_encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("shader_equiv_ssao_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: timestamps.map(|probe| wgpu::RenderPassTimestampWrites {
                query_set: &probe.query_set,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: Some(1),
            }),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &output,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(readback_bytes_per_row(width)),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    if let Some(probe) = timestamps {
        encoder.resolve_query_set(&probe.query_set, 0..2, &probe.resolve, 0);
        encoder.copy_buffer_to_buffer(&probe.resolve, 0, &probe.readback, 0, 16);
    }
    queue.submit(Some(encoder.finish()));
    let slice = readback.slice(..);
    let (tx, rx) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    rx.recv()
        .expect("SSAO readback callback")
        .expect("SSAO readback map");
    let mapped = slice.get_mapped_range().expect("SSAO readback range");
    let mut values = Vec::with_capacity((width * height) as usize);
    let bytes_per_row = readback_bytes_per_row(width) as usize;
    for row in 0..height as usize {
        let start = row * bytes_per_row;
        let row_bytes = &mapped[start..start + (width * 4) as usize];
        values.extend_from_slice(bytemuck::cast_slice(row_bytes));
    }
    drop(mapped);
    readback.unmap();
    values
}

#[test]
fn ssao_blur_center_tap_readback_matches_reference_at_edges() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip SSAO shader equivalence: no wgpu adapter");
            return;
        };
        let reference_source = old_ssao_source();
        for (label, divisor, samples) in [("low", 2, 4), ("medium", 2, 8), ("high", 2, 12)] {
            let reference = render_ssao_blur(
                &device,
                &queue,
                SsaoBlurFixture::new(&reference_source, divisor, samples, 5, 4),
                None,
            )
            .await;
            let candidate = render_ssao_blur(
                &device,
                &queue,
                SsaoBlurFixture::new(SSAO_BLUR, divisor, samples, 5, 4),
                None,
            )
            .await;
            assert_eq!(reference.len(), candidate.len(), "{label} output size");
            for (index, (a, b)) in reference.iter().zip(candidate.iter()).enumerate() {
                if a.is_nan() || b.is_nan() {
                    assert!(a.is_nan() && b.is_nan(), "{label} NaN drift at {index}");
                } else {
                    assert!(
                        (a - b).abs() <= 2.0e-6,
                        "{label} SSAO drift at {index}: {a} != {b}"
                    );
                }
            }
        }
    });
}

#[test]
#[ignore = "explicit GPU timestamp probe"]
fn ssao_blur_timestamp_probe_abba() {
    pollster::block_on(async {
        let Some((device, queue)) = timestamp_test_device().await else {
            eprintln!("skip SSAO timestamp probe: no timestamp adapter");
            return;
        };
        let probe = TimestampProbe::new(&device, &queue);
        let reference_source = old_ssao_source();

        // Warm each shader path b4 measured legs. Keep query range around pass only.
        for source in [reference_source.as_str(), SSAO_BLUR] {
            let _ = render_ssao_blur(
                &device,
                &queue,
                SsaoBlurFixture::new(source, 2, 8, 1280, 720),
                Some(&probe),
            )
            .await;
            let _ = probe.read(&device).expect("SSAO warmup timestamp");
        }

        const SAMPLE_COUNT: usize = 30;
        let mut samples = Vec::new();
        for reference in [true, false, false, true] {
            let source = if reference {
                reference_source.as_str()
            } else {
                SSAO_BLUR
            };
            let mut leg = Vec::with_capacity(SAMPLE_COUNT);
            for _ in 0..SAMPLE_COUNT {
                let _ = render_ssao_blur(
                    &device,
                    &queue,
                    SsaoBlurFixture::new(source, 2, 8, 1280, 720),
                    Some(&probe),
                )
                .await;
                leg.push(probe.read(&device).expect("SSAO timestamp"));
            }
            samples.push((reference, timestamp_summary(&mut leg)));
        }
        eprintln!("SSAO timestamp ABBA med/p95 ns @1280x720: {samples:?}");
    });
}

fn timestamp_summary(samples: &mut [f64]) -> (f64, f64) {
    assert!(!samples.is_empty());
    samples.sort_by(|a, b| a.total_cmp(b));
    let median = samples[(samples.len() - 1) / 2];
    let p95_index = (samples.len() * 95).div_ceil(100) - 1;
    (median, samples[p95_index])
}
