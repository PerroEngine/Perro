//! Present-chain GPU wins: the merged TAA resolve (MRT) and the two-pass
//! auto-exposure reduction.
//!
//! Both run the real `PresentProcessor` against a headless wgpu device and
//! A/B the new path against the one it replaces; skipped with a note when no
//! adapter is available.
use super::*;
use half::f16;

const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

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
            label: Some("perro_present_test_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        })
        .await
        .ok()
}

/// Deterministic HDR luminance pattern: a horizontal ramp crossed with a
/// vertical banding term, so the reduction sees a wide spread of log2 values
/// instead of a constant (which any summation order would match trivially).
fn scene_luminance(x: u32, y: u32, dimensions: [u32; 2]) -> [f32; 3] {
    let u = x as f32 / dimensions[0].max(1) as f32;
    let v = y as f32 / dimensions[1].max(1) as f32;
    let band = if (x / 7 + y / 5).is_multiple_of(3) {
        4.0
    } else {
        0.25
    };
    [
        (0.02 + u * 2.5) * band,
        (0.05 + v * 1.5) * band,
        (0.01 + u * v * 3.0) * band,
    ]
}

fn scene_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    dimensions: [u32; 2],
) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_present_test_scene"),
        size: wgpu::Extent3d {
            width: dimensions[0],
            height: dimensions[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: SCENE_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut texels = Vec::with_capacity((dimensions[0] * dimensions[1] * 4) as usize);
    for y in 0..dimensions[1] {
        for x in 0..dimensions[0] {
            let rgb = scene_luminance(x, y, dimensions);
            texels.extend_from_slice(&f16::from_f32(rgb[0]).to_le_bytes());
            texels.extend_from_slice(&f16::from_f32(rgb[1]).to_le_bytes());
            texels.extend_from_slice(&f16::from_f32(rgb[2]).to_le_bytes());
            texels.extend_from_slice(&f16::from_f32(1.0).to_le_bytes());
        }
    }
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &texels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(dimensions[0] * 8),
            rows_per_image: Some(dimensions[1]),
        },
        wgpu::Extent3d {
            width: dimensions[0],
            height: dimensions[1],
            depth_or_array_layers: 1,
        },
    );
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn read_buffer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    src: &wgpu::Buffer,
    len: u64,
) -> Vec<u8> {
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("perro_present_test_readback"),
        size: len,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("perro_present_test_copy"),
    });
    encoder.copy_buffer_to_buffer(src, 0, &staging, 0, len);
    queue.submit([encoder.finish()]);
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    rx.recv().expect("map callback").expect("readback map");
    let bytes = slice
        .get_mapped_range()
        .expect("mapped readback range")
        .to_vec();
    staging.unmap();
    bytes
}

fn read_exposure_value(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    present: &PresentProcessor,
) -> f32 {
    let bytes = read_buffer(device, queue, &present.exposure_state_buffer, 16);
    f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn reset_exposure_state(queue: &wgpu::Queue, present: &PresentProcessor) {
    queue.write_buffer(&present.exposure_state_buffer, 0, &[0u8; 16]);
}

fn auto_exposure_settings() -> PresentExposureSettings {
    PresentExposureSettings {
        exposure: 0.25,
        auto_exposure: true,
        min_exposure: -8.0,
        max_exposure: 8.0,
        speed_up: 3.0,
        speed_down: 1.0,
        target_luminance: 0.18,
    }
}

/// The pre-parallelization auto-exposure shader: ONE 64-lane workgroup loops
/// the whole frame. Kept verbatim as the A/B reference for the two-pass
/// reduction that replaced it (same bind group layout, so it binds the live
/// config/state buffers).
const LEGACY_EXPOSURE_WGSL: &str = r#"
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

#[test]
fn exposure_dispatch_geometry_parallelizes_the_reduction() {
    // Old geometry was a hard 1x1x1 for every resolution; the reduction now
    // scales workgroups with the sampled pixel count and folds the partials
    // with one extra small dispatch.
    // 1080p sits just above the 2 MP threshold, so it samples at stride 4:
    // 480 * 270 = 129_600 loads, previously serialized through 64 lanes.
    let hd = [1920u32, 1080];
    let hd_stride = exposure_sample_stride(hd);
    assert_eq!(hd_stride, 4);
    let hd_groups = exposure_dispatch_groups(hd, hd_stride);
    // ceil(129_600 / (64 lanes * 16 samples)) = 127 workgroups; 127 * 64 =
    // 8128 lanes doing ~16 loads each instead of 64 lanes doing ~2025.
    assert_eq!(hd_groups, 127);

    // Under the threshold the stride halves, so a smaller frame can still
    // sample more texels (and dispatch more groups) than a bigger one.
    let wide = [1600u32, 900];
    assert_eq!(exposure_sample_stride(wide), 2);
    assert_eq!(exposure_dispatch_groups(wide, 2), 352);

    // 4K keeps stride 4, so the sample count scales with the frame.
    let uhd = [3840u32, 2160];
    assert_eq!(exposure_sample_stride(uhd), 4);
    assert_eq!(exposure_dispatch_groups(uhd, 4), 507);

    // Tiny targets stay on a single workgroup (no launch cost for a handful
    // of loads), and nothing ever exceeds the partial-array bound.
    assert_eq!(exposure_dispatch_groups([64, 64], 2), 1);
    assert_eq!(exposure_dispatch_groups([1, 1], 2), 1);
    for stride in [1u32, 2, 4] {
        let groups = exposure_dispatch_groups([16_384, 16_384], stride);
        assert!(
            (1..=EXPOSURE_MAX_GROUPS).contains(&groups),
            "groups {groups} out of range at stride {stride}"
        );
    }

    // The shader's partial array is fixed length; the Rust bound and the
    // storage buffer must match it or pass 2 would read past the partials.
    assert!(EXPOSURE_WGSL.contains("partials: array<vec2<f32>, 512>"));
    assert_eq!(EXPOSURE_MAX_GROUPS, 512);
    assert_eq!(
        EXPOSURE_STATE_BUFFER_SIZE,
        16 + u64::from(EXPOSURE_MAX_GROUPS) * 8
    );
    assert_eq!(EXPOSURE_WORKGROUP_SIZE, 64);
}

#[test]
fn exposure_two_pass_matches_the_legacy_single_group_shader() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip exposure reduction test: no wgpu adapter");
            return;
        };
        // Odd dimensions on purpose: exercises the ragged tail of the sample
        // grid and the edge clamp in both shaders.
        let dimensions = [197u32, 113];
        let scene_view = scene_texture(&device, &queue, dimensions);
        let mut present = PresentProcessor::new(&device, OUTPUT_FORMAT);
        present.set_output_size(dimensions[0], dimensions[1]);
        let Some(exposure_bgl) = present.exposure_bgl.as_ref() else {
            eprintln!("skip exposure reduction test: device has no auto-exposure support");
            return;
        };
        let bind_groups = present.create_bind_group(&device, &scene_view);
        let exposure_bind_group = bind_groups
            .exposure
            .as_ref()
            .expect("auto-exposure bind group");

        let legacy_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_present_test_legacy_exposure"),
            source: wgpu::ShaderSource::Wgsl(LEGACY_EXPOSURE_WGSL.into()),
        });
        let legacy_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_present_test_legacy_layout"),
            bind_group_layouts: &[Some(exposure_bgl)],
            immediate_size: 0,
        });
        let legacy_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("perro_present_test_legacy_pipeline"),
            layout: Some(&legacy_layout),
            module: &legacy_shader,
            entry_point: Some("cs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let settings = auto_exposure_settings();
        let delta = 1.0f32 / 60.0;
        let frames = 90;
        let sample_stride = exposure_sample_stride(dimensions);
        let groups = exposure_dispatch_groups(dimensions, sample_stride);
        assert!(groups > 1, "test frame must span several workgroups");

        // Reference run: the old 1x1x1 dispatch, same config/state buffers,
        // same per-frame adaptation inputs.
        reset_exposure_state(&queue, &present);
        for _ in 0..frames {
            let config = ExposureGpuConfig {
                dimensions,
                sample_stride,
                group_count: 1,
                delta_seconds: delta,
                compensation: settings.exposure,
                min_exposure: settings.min_exposure,
                max_exposure: settings.max_exposure,
                speed_up: settings.speed_up,
                speed_down: settings.speed_down,
                target_luminance: settings.target_luminance,
                _pad1: 0.0,
            };
            queue.write_buffer(
                &present.exposure_config_buffer,
                0,
                bytemuck::bytes_of(&config),
            );
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("perro_present_test_legacy_encoder"),
            });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("perro_present_test_legacy_pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&legacy_pipeline);
                pass.set_bind_group(0, exposure_bind_group, &[]);
                pass.dispatch_workgroups(1, 1, 1);
            }
            queue.submit([encoder.finish()]);
        }
        let legacy_value = read_exposure_value(&device, &queue, &present);

        // Live run: the two-pass reduction through the real present path.
        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("perro_present_test_output"),
            size: wgpu::Extent3d {
                width: dimensions[0],
                height: dimensions[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OUTPUT_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());
        reset_exposure_state(&queue, &present);
        for _ in 0..frames {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("perro_present_test_encoder"),
            });
            present.apply(
                &queue,
                &mut encoder,
                &bind_groups,
                &output_view,
                dimensions,
                delta,
                settings,
                HdrStatus::default(),
                None,
            );
            queue.submit([encoder.finish()]);
        }
        let parallel_value = read_exposure_value(&device, &queue, &present);

        assert_eq!(
            present.last_exposure_groups, groups,
            "dispatch geometry: 1x1x1 -> {groups}+1 workgroups"
        );
        eprintln!(
            "exposure converge: parallel={parallel_value} legacy={legacy_value} groups={groups}+1"
        );
        // Adaptation semantics are unchanged; only the summation order moves,
        // so the converged exposure agrees to well under a stop.
        assert!(
            (parallel_value - legacy_value).abs() < 1.0e-3,
            "converged exposure drifted: parallel={parallel_value} legacy={legacy_value}"
        );
        assert!(
            parallel_value.is_finite() && parallel_value.abs() > 1.0e-3,
            "auto-exposure must actually move the state: {parallel_value}"
        );
    });
}

const TAA_SIZE: [u32; 2] = [64, 64];

/// Runs `frames` TAA frames and returns (swapchain bytes, passes encoded by
/// the last frame). `gate` is the swapchain size the processor believes it is
/// writing to: equal to the render size selects the merged MRT resolve, any
/// other value keeps the resolve + blit pair. The attachment itself is always
/// render-sized, so both paths are directly comparable.
fn run_taa(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    scene_view: &wgpu::TextureView,
    gate: [u32; 2],
    frames: usize,
) -> (Vec<u8>, u32) {
    let mut present = PresentProcessor::new(device, OUTPUT_FORMAT);
    present.set_taa_active(true);
    present.set_output_size(gate[0], gate[1]);
    let bind_groups = present.create_bind_group(device, scene_view);
    let size = wgpu::Extent3d {
        width: TAA_SIZE[0],
        height: TAA_SIZE[1],
        depth_or_array_layers: 1,
    };
    let output = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_present_test_taa_output"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OUTPUT_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_present_test_taa_depth"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let identity = [
        [1.0f32, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let taa_frame = PresentTaaFrame {
        depth_view: depth.create_view(&wgpu::TextureViewDescriptor::default()),
        inv_view_proj: identity,
        prev_view_proj: identity,
    };
    let settings = PresentExposureSettings::default();
    for _ in 0..frames {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perro_present_test_taa_encoder"),
        });
        present.apply(
            queue,
            &mut encoder,
            &bind_groups,
            &output_view,
            TAA_SIZE,
            1.0 / 60.0,
            settings,
            HdrStatus::default(),
            Some(&taa_frame),
        );
        queue.submit([encoder.finish()]);
    }

    let bytes_per_row = TAA_SIZE[0] * 4;
    let byte_len = u64::from(bytes_per_row * TAA_SIZE[1]);
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("perro_present_test_taa_readback"),
        size: byte_len,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("perro_present_test_taa_copy"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &output,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(TAA_SIZE[1]),
            },
        },
        size,
    );
    queue.submit([encoder.finish()]);
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    rx.recv().expect("map callback").expect("taa readback map");
    let bytes = slice
        .get_mapped_range()
        .expect("mapped taa readback range")
        .to_vec();
    staging.unmap();
    (bytes, present.last_taa_passes)
}

#[test]
fn taa_merges_the_blit_into_the_resolve_at_matching_sizes() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip TAA merged-resolve test: no wgpu adapter");
            return;
        };
        let scene_view = scene_texture(&device, &queue, TAA_SIZE);
        // Two frames: frame 2 actually blends history, so the comparison
        // covers the accumulation path, not just the history-reset frame.
        let (merged, merged_passes) = run_taa(&device, &queue, &scene_view, TAA_SIZE, 2);
        let (split, split_passes) = run_taa(
            &device,
            &queue,
            &scene_view,
            [TAA_SIZE[0] * 2, TAA_SIZE[1] * 2],
            2,
        );

        assert_eq!(merged_passes, 1, "1:1 must encode ONE render pass");
        assert_eq!(split_passes, 2, "upscale must keep resolve + blit");
        assert_eq!(merged.len(), split.len());

        // The merged path writes the swapchain straight from the resolve; the
        // split path round-trips through the f16 history first, so allow one
        // 8-bit step of difference.
        let mut worst = 0i32;
        for (index, (a, b)) in merged.iter().zip(split.iter()).enumerate() {
            let delta = i32::from(*a) - i32::from(*b);
            worst = worst.max(delta.abs());
            assert!(
                delta.abs() <= 1,
                "byte {index} differs: merged={a} split={b}"
            );
        }
        // Guard against both paths writing an all-clear image (which would
        // make the comparison vacuous).
        assert!(
            merged.iter().any(|byte| *byte > 8),
            "TAA output must not be black"
        );
        eprintln!("taa merged-vs-split worst byte delta: {worst}");
    });
}
