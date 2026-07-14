use super::*;

pub(super) fn sky_clear_color(lighting: &Lighting3DState) -> Option<wgpu::Color> {
    let sky = lighting.sky.as_ref()?;
    let day = sample_gradient_color(sky.day_colors.as_ref(), 0.32);
    let evening = sample_gradient_color(sky.evening_colors.as_ref(), 0.32);
    let night = sample_gradient_color(sky.night_colors.as_ref(), 0.32);
    let day_t = day_weight(sky.time.time_of_day);
    let evening_t = evening_weight(sky.time.time_of_day);
    let base = [
        night[0] + (day[0] - night[0]) * day_t,
        night[1] + (day[1] - night[1]) * day_t,
        night[2] + (day[2] - night[2]) * day_t,
    ];
    let c = [
        base[0] + (evening[0] - base[0]) * evening_t,
        base[1] + (evening[1] - base[1]) * evening_t,
        base[2] + (evening[2] - base[2]) * evening_t,
    ];
    Some(wgpu::Color {
        r: c[0].clamp(0.0, 1.0) as f64,
        g: c[1].clamp(0.0, 1.0) as f64,
        b: c[2].clamp(0.0, 1.0) as f64,
        a: 1.0,
    })
}

pub(super) fn sample_gradient_color(colors: &[[f32; 3]], t: f32) -> [f32; 3] {
    if colors.is_empty() {
        return [CLEAR_R as f32, CLEAR_G as f32, CLEAR_B as f32];
    }
    if colors.len() == 1 {
        return colors[0];
    }
    let n = colors.len() - 1;
    let f = t.clamp(0.0, 1.0) * n as f32;
    let i = f.floor() as usize;
    let j = (i + 1).min(n);
    let u = f - i as f32;
    [
        colors[i][0] + (colors[j][0] - colors[i][0]) * u,
        colors[i][1] + (colors[j][1] - colors[i][1]) * u,
        colors[i][2] + (colors[j][2] - colors[i][2]) * u,
    ]
}

pub(super) fn day_weight(time_of_day: f32) -> f32 {
    let t = time_of_day.rem_euclid(1.0);
    let a = (t * std::f32::consts::TAU) - std::f32::consts::FRAC_PI_2;
    ((a.sin() + 1.0) * 0.5).clamp(0.0, 1.0)
}

pub(super) fn evening_weight(time_of_day: f32) -> f32 {
    let t = time_of_day.rem_euclid(1.0);
    let dist = ((t - 0.75 + 0.5).rem_euclid(1.0) - 0.5).abs();
    (1.0 - (dist / 0.23)).clamp(0.0, 1.0)
}

pub(super) fn normalize_sample_count(samples: u32) -> u32 {
    match samples {
        0 | 1 => 1,
        2 => 2,
        4 => SMOOTH_SAMPLE_COUNT,
        _ => 8,
    }
}

pub(super) fn max_supported_msaa_sample_count(
    adapter: &wgpu::Adapter,
    format: wgpu::TextureFormat,
) -> u32 {
    let features = adapter.get_texture_format_features(format);
    let flags = features.flags;
    for count in [16u32, 8, 4, 2, 1] {
        if sample_count_supported(flags, count) {
            return count;
        }
    }
    1
}

pub(super) fn clamp_supported_sample_count(requested: u32, max_supported: u32) -> u32 {
    for count in [16u32, 8, 4, 2, 1] {
        if count > requested || count > max_supported {
            continue;
        }
        return count;
    }
    1
}

#[inline]
pub(super) fn sample_count_supported(
    flags: wgpu::TextureFormatFeatureFlags,
    sample_count: u32,
) -> bool {
    match sample_count {
        1 => true,
        2 => flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X2),
        4 => flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4),
        8 => flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X8),
        16 => flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X16),
        _ => false,
    }
}

const MAX_FRAME_RENDER_PIXELS: u64 = 16_777_216;

pub(crate) fn capped_render_size(width: u32, height: u32, max_dimension: u32) -> (u32, u32) {
    let width = width.max(1);
    let height = height.max(1);
    let max_dimension = max_dimension.max(1);
    let dim_scale = (max_dimension as f64 / width.max(height) as f64).min(1.0);
    let pixels = width as u64 * height as u64;
    let pixel_scale = if pixels > MAX_FRAME_RENDER_PIXELS {
        (MAX_FRAME_RENDER_PIXELS as f64 / pixels as f64).sqrt()
    } else {
        1.0
    };
    let scale = dim_scale.min(pixel_scale).clamp(0.0, 1.0);
    (
        ((width as f64 * scale).round() as u32).max(1),
        ((height as f64 * scale).round() as u32).max(1),
    )
}

pub(super) fn create_msaa_color_target(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    sample_count: u32,
) -> Option<MsaaColorTarget> {
    if sample_count <= 1 {
        return None;
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_msaa_color"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    Some(MsaaColorTarget {
        _texture: texture,
        view,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PresentExposureSettings {
    pub exposure: f32,
    pub auto_exposure: bool,
    pub min_exposure: f32,
    pub max_exposure: f32,
    pub speed_up: f32,
    pub speed_down: f32,
    pub target_luminance: f32,
}

fn create_auto_exposure(
    device: &wgpu::Device,
) -> (Option<wgpu::BindGroupLayout>, Option<wgpu::ComputePipeline>) {
    let auto_supported = device.limits().max_storage_buffers_per_shader_stage > 0
        && device.limits().max_compute_invocations_per_workgroup >= 64;
    let (exposure_bgl, exposure_pipeline) = if auto_supported {
        let exposure_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_exposure_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<ExposureGpuConfig>() as u64,
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
                    count: None,
                },
            ],
        });
        let exposure_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_exposure_shader"),
            source: wgpu::ShaderSource::Wgsl(EXPOSURE_WGSL.into()),
        });
        let exposure_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_exposure_layout"),
            bind_group_layouts: &[Some(&exposure_bgl)],
            immediate_size: 0,
        });
        let exposure_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("perro_exposure_pipeline"),
            layout: Some(&exposure_layout),
            module: &exposure_shader,
            entry_point: Some("cs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        (Some(exposure_bgl), Some(exposure_pipeline))
    } else {
        (None, None)
    };
    (exposure_bgl, exposure_pipeline)
}

impl Default for PresentExposureSettings {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            auto_exposure: false,
            min_exposure: -8.0,
            max_exposure: 8.0,
            speed_up: 3.0,
            speed_down: 1.0,
            target_luminance: 0.18,
        }
    }
}

impl PresentExposureSettings {
    pub(super) fn apply_effects(&mut self, effects: &[PostProcessEffect]) {
        for effect in effects {
            if let PostProcessEffect::Exposure {
                exposure,
                auto_exposure,
                min_exposure,
                max_exposure,
                speed_up,
                speed_down,
                target_luminance,
            } = effect
            {
                *self = Self {
                    exposure: finite_or(*exposure, 0.0),
                    auto_exposure: *auto_exposure,
                    min_exposure: finite_or(*min_exposure, -8.0),
                    max_exposure: finite_or(*max_exposure, 8.0),
                    speed_up: finite_or(*speed_up, 3.0).max(0.0),
                    speed_down: finite_or(*speed_down, 1.0).max(0.0),
                    target_luminance: finite_or(*target_luminance, 0.18).max(0.0001),
                };
                if self.min_exposure > self.max_exposure {
                    std::mem::swap(&mut self.min_exposure, &mut self.max_exposure);
                }
            }
        }
    }
}

#[inline]
fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ExposureGpuConfig {
    dimensions: [u32; 2],
    sample_stride: u32,
    _pad0: u32,
    delta_seconds: f32,
    compensation: f32,
    min_exposure: f32,
    max_exposure: f32,
    speed_up: f32,
    speed_down: f32,
    target_luminance: f32,
    _pad1: f32,
}

const PRESENT_WGSL: &str = r#"
@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
struct ExposureUniform { value: vec4<f32> };
@group(0) @binding(2) var<uniform> exposure_state: ExposureUniform;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(pos[vid], 0.0, 1.0);
    out.uv = (out.pos.xy * vec2<f32>(0.5, -0.5)) + vec2<f32>(0.5, 0.5);
    return out;
}

fn aces_filmic(x: vec3<f32>) -> vec3<f32> {
    return clamp(
        (x * (2.51 * x + vec3<f32>(0.03))) /
            (x * (2.43 * x + vec3<f32>(0.59)) + vec3<f32>(0.14)),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let scene = max(textureSample(input_tex, input_sampler, in.uv).rgb, vec3<f32>(0.0));
    return vec4<f32>(aces_filmic(scene * exp2(exposure_state.value.x)), 1.0);
}
"#;

const EXPOSURE_WGSL: &str = r#"
struct ExposureConfig {
    dimensions: vec2<u32>,
    sample_stride: u32,
    _pad0: u32,
    delta_seconds: f32,
    compensation: f32,
    min_exposure: f32,
    max_exposure: f32,
    speed_up: f32,
    speed_down: f32,
    target_luminance: f32,
    _pad1: f32,
};

struct ExposureState {
    value: vec4<f32>,
};

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var<uniform> cfg: ExposureConfig;
@group(0) @binding(2) var<storage, read_write> state: ExposureState;

var<workgroup> log_luma_sum: array<f32, 64>;
var<workgroup> sample_count: array<u32, 64>;

@compute @workgroup_size(64)
fn cs_main(@builtin(local_invocation_index) lane: u32) {
    let stride = max(cfg.sample_stride, 1u);
    let sample_width = (cfg.dimensions.x + stride - 1u) / stride;
    let sample_height = (cfg.dimensions.y + stride - 1u) / stride;
    let total = sample_width * sample_height;
    var sum = 0.0;
    var count = 0u;
    var index = lane;
    while index < total {
        let sample_xy = vec2<u32>(index % sample_width, index / sample_width) * stride;
        let xy = min(sample_xy, cfg.dimensions - vec2<u32>(1u));
        let rgb = max(textureLoad(scene_tex, vec2<i32>(xy), 0).rgb, vec3<f32>(0.0));
        let luma = max(dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.000001);
        sum += log2(luma);
        count += 1u;
        index += 64u;
    }
    log_luma_sum[lane] = sum;
    sample_count[lane] = count;
    workgroupBarrier();

    var width = 32u;
    while width > 0u {
        if lane < width {
            log_luma_sum[lane] += log_luma_sum[lane + width];
            sample_count[lane] += sample_count[lane + width];
        }
        workgroupBarrier();
        width /= 2u;
    }

    if lane == 0u {
        let n = max(sample_count[0], 1u);
        let avg_log_luma = log_luma_sum[0] / f32(n);
        let target_exposure = clamp(
            log2(max(cfg.target_luminance, 0.0001)) - avg_log_luma + cfg.compensation,
            cfg.min_exposure,
            cfg.max_exposure,
        );
        let speed = select(cfg.speed_down, cfg.speed_up, target_exposure > state.value.x);
        let blend = 1.0 - exp(-max(speed, 0.0) * clamp(cfg.delta_seconds, 0.0, 1.0));
        state.value.x = mix(state.value.x, target_exposure, blend);
    }
}
"#;

impl PresentProcessor {
    pub(super) fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_present_shader"),
            source: wgpu::ShaderSource::Wgsl(PRESENT_WGSL.into()),
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("perro_present_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let exposure_config_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_exposure_config"),
            size: std::mem::size_of::<ExposureGpuConfig>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let exposure_state_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_exposure_state"),
            size: 16,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let exposure_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_exposure_uniform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_present_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_present_layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("perro_present_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let (exposure_bgl, exposure_pipeline) = create_auto_exposure(device);
        Self {
            sampler,
            bgl,
            pipeline,
            exposure_bgl,
            exposure_pipeline,
            exposure_config_buffer,
            exposure_state_buffer,
            exposure_uniform_buffer,
        }
    }

    pub(super) fn create_bind_group(
        &self,
        device: &wgpu::Device,
        input_view: &wgpu::TextureView,
    ) -> PresentBindGroups {
        let tonemap = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_present_bg"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.exposure_uniform_buffer.as_entire_binding(),
                },
            ],
        });
        let exposure = self.exposure_bgl.as_ref().map(|layout| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("perro_exposure_bg"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(input_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.exposure_config_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.exposure_state_buffer.as_entire_binding(),
                    },
                ],
            })
        });
        PresentBindGroups { tonemap, exposure }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        bind_groups: &PresentBindGroups,
        output_view: &wgpu::TextureView,
        dimensions: [u32; 2],
        delta_seconds: f32,
        settings: PresentExposureSettings,
    ) {
        if settings.auto_exposure {
            if let (Some(pipeline), Some(bind_group)) = (
                self.exposure_pipeline.as_ref(),
                bind_groups.exposure.as_ref(),
            ) {
                let pixels = u64::from(dimensions[0]) * u64::from(dimensions[1]);
                let sample_stride = if pixels > 2_000_000 { 4 } else { 2 };
                let config = ExposureGpuConfig {
                    dimensions: [dimensions[0].max(1), dimensions[1].max(1)],
                    sample_stride,
                    _pad0: 0,
                    delta_seconds: finite_or(delta_seconds, 0.0).max(0.0),
                    compensation: settings.exposure,
                    min_exposure: settings.min_exposure,
                    max_exposure: settings.max_exposure,
                    speed_up: settings.speed_up,
                    speed_down: settings.speed_down,
                    target_luminance: settings.target_luminance,
                    _pad1: 0.0,
                };
                queue.write_buffer(&self.exposure_config_buffer, 0, bytemuck::bytes_of(&config));
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("perro_exposure_pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, bind_group, &[]);
                pass.dispatch_workgroups(1, 1, 1);
                drop(pass);
                encoder.copy_buffer_to_buffer(
                    &self.exposure_state_buffer,
                    0,
                    &self.exposure_uniform_buffer,
                    0,
                    16,
                );
            } else {
                write_manual_exposure(queue, self, settings.exposure);
            }
        } else {
            write_manual_exposure(queue, self, settings.exposure);
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_present_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_groups.tonemap, &[]);
        pass.draw(0..3, 0..1);
    }
}

fn write_manual_exposure(queue: &wgpu::Queue, present: &PresentProcessor, exposure: f32) {
    let state = [finite_or(exposure, 0.0), 0.0, 0.0, 0.0];
    queue.write_buffer(
        &present.exposure_uniform_buffer,
        0,
        bytemuck::cast_slice(&state),
    );
    queue.write_buffer(
        &present.exposure_state_buffer,
        0,
        bytemuck::cast_slice(&state),
    );
}

#[allow(dead_code)]
pub(super) fn linear_render_format(surface_format: wgpu::TextureFormat) -> wgpu::TextureFormat {
    linear_render_format_with_hdr(surface_format, true)
}

pub(super) fn supported_linear_render_format(
    adapter: &wgpu::Adapter,
    surface_format: wgpu::TextureFormat,
) -> wgpu::TextureFormat {
    let required = wgpu::TextureUsages::RENDER_ATTACHMENT
        | wgpu::TextureUsages::TEXTURE_BINDING
        | wgpu::TextureUsages::COPY_SRC;
    let hdr_supported = adapter
        .get_texture_format_features(wgpu::TextureFormat::Rgba16Float)
        .allowed_usages
        .contains(required);
    linear_render_format_with_hdr(surface_format, hdr_supported)
}

fn linear_render_format_with_hdr(
    surface_format: wgpu::TextureFormat,
    hdr_supported: bool,
) -> wgpu::TextureFormat {
    match surface_format {
        // Float target: HDR light accumulation headroom and no linear-in-8bit
        // banding in dark gradients; present encodes to the sRGB swapchain.
        wgpu::TextureFormat::Rgba8Unorm
        | wgpu::TextureFormat::Bgra8Unorm
        | wgpu::TextureFormat::Rgba8UnormSrgb
        | wgpu::TextureFormat::Bgra8UnormSrgb
            if hdr_supported =>
        {
            wgpu::TextureFormat::Rgba16Float
        }
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => {
            wgpu::TextureFormat::Rgba8Unorm
        }
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => {
            wgpu::TextureFormat::Bgra8Unorm
        }
        _ => surface_format,
    }
}

pub(super) const fn srgb_surface_view_format(
    surface_format: wgpu::TextureFormat,
) -> Option<wgpu::TextureFormat> {
    match surface_format {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => {
            Some(wgpu::TextureFormat::Rgba8UnormSrgb)
        }
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => {
            Some(wgpu::TextureFormat::Bgra8UnormSrgb)
        }
        _ => None,
    }
}

pub(super) fn choose_present_mode(
    modes: &[wgpu::PresentMode],
    vsync_enabled: bool,
) -> wgpu::PresentMode {
    if let Some(forced) = parse_present_mode_override() {
        if modes.contains(&forced) {
            return forced;
        }
        eprintln!("[perro][gfx] PERRO_PRESENT_MODE set but unsupported by surface: ({forced:?})");
    }

    let preferred = if vsync_enabled {
        [
            wgpu::PresentMode::AutoVsync,
            wgpu::PresentMode::Fifo,
            wgpu::PresentMode::FifoRelaxed,
        ]
        .as_slice()
    } else {
        [
            wgpu::PresentMode::Immediate,
            wgpu::PresentMode::Mailbox,
            wgpu::PresentMode::AutoNoVsync,
        ]
        .as_slice()
    };

    for mode in preferred {
        if modes.contains(mode) {
            if !vsync_enabled
                && matches!(
                    mode,
                    wgpu::PresentMode::AutoVsync
                        | wgpu::PresentMode::Fifo
                        | wgpu::PresentMode::FifoRelaxed
                )
            {
                eprintln!(
                    "[perro][gfx] vsync=false but surface only chose vsync present mode: ({mode:?})"
                );
            }
            return *mode;
        }
    }
    let fallback = modes.first().copied().unwrap_or(wgpu::PresentMode::Fifo);
    if !vsync_enabled {
        eprintln!(
            "[perro][gfx] vsync=false but no no-vsync present mode found; fallback=({fallback:?})"
        );
    }
    fallback
}

pub(super) fn choose_max_frame_latency(_vsync_enabled: bool) -> u32 {
    #[cfg(target_arch = "wasm32")]
    let default = 1;
    #[cfg(not(target_arch = "wasm32"))]
    let default = if _vsync_enabled { 3 } else { 8 };
    std::env::var("PERRO_FRAME_LATENCY")
        .ok()
        .and_then(|raw| raw.parse::<u32>().ok())
        .map(|val| val.clamp(1, 8))
        .unwrap_or(default)
}

pub(super) fn parse_present_mode_override() -> Option<wgpu::PresentMode> {
    let raw = std::env::var("PERRO_PRESENT_MODE").ok()?;
    let norm = raw.trim().to_ascii_lowercase();
    match norm.as_str() {
        "autovsync" | "auto_vsync" => Some(wgpu::PresentMode::AutoVsync),
        "fiforelaxed" | "fifo_relaxed" => Some(wgpu::PresentMode::FifoRelaxed),
        "fifo" => Some(wgpu::PresentMode::Fifo),
        "mailbox" => Some(wgpu::PresentMode::Mailbox),
        "immediate" => Some(wgpu::PresentMode::Immediate),
        "autonovsync" | "auto_novsync" => Some(wgpu::PresentMode::AutoNoVsync),
        _ => {
            eprintln!("[perro][gfx] unknown PERRO_PRESENT_MODE=({raw}); ignore override");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capped_render_size_keeps_safe_size() {
        assert_eq!(capped_render_size(3840, 2160, 8192), (3840, 2160));
    }

    #[test]
    fn capped_render_size_preserves_aspect_when_too_wide() {
        assert_eq!(capped_render_size(10_000, 1_000, 8192), (8192, 819));
    }

    #[test]
    fn capped_render_size_limits_pixel_count() {
        let (width, height) = capped_render_size(8192, 8192, 8192);
        assert!(width as u64 * height as u64 <= MAX_FRAME_RENDER_PIXELS + 8192);
        assert_eq!(width, height);
    }

    #[test]
    fn linear_8bit_surface_uses_srgb_view_and_float_render_target() {
        assert_eq!(
            srgb_surface_view_format(wgpu::TextureFormat::Bgra8Unorm),
            Some(wgpu::TextureFormat::Bgra8UnormSrgb)
        );
        assert_eq!(
            linear_render_format(wgpu::TextureFormat::Bgra8UnormSrgb),
            wgpu::TextureFormat::Rgba16Float
        );
    }

    #[test]
    fn exposure_effect_uses_last_cfg_and_sanitizes_ranges() {
        let mut settings = PresentExposureSettings::default();
        settings.apply_effects(&[
            PostProcessEffect::Exposure {
                exposure: 1.0,
                auto_exposure: false,
                min_exposure: -2.0,
                max_exposure: 2.0,
                speed_up: 1.0,
                speed_down: 1.0,
                target_luminance: 0.18,
            },
            PostProcessEffect::Exposure {
                exposure: -0.5,
                auto_exposure: true,
                min_exposure: 4.0,
                max_exposure: -3.0,
                speed_up: -1.0,
                speed_down: f32::NAN,
                target_luminance: 0.0,
            },
        ]);

        assert_eq!(settings.exposure, -0.5);
        assert!(settings.auto_exposure);
        assert_eq!((settings.min_exposure, settings.max_exposure), (-3.0, 4.0));
        assert_eq!((settings.speed_up, settings.speed_down), (0.0, 1.0));
        assert_eq!(settings.target_luminance, 0.0001);
    }

    #[test]
    fn linear_8bit_fallback_stays_linear() {
        assert_eq!(
            linear_render_format_with_hdr(wgpu::TextureFormat::Bgra8UnormSrgb, false),
            wgpu::TextureFormat::Bgra8Unorm
        );
    }

    #[test]
    fn hdr_present_shaders_parse() {
        for source in [PRESENT_WGSL, EXPOSURE_WGSL] {
            naga::front::wgsl::parse_str(source).expect("HDR present WGSL parses");
        }
    }
}
