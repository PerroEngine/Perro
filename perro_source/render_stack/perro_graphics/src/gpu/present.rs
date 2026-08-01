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

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SurfaceSelection {
    pub format: wgpu::TextureFormat,
    pub view_format: wgpu::TextureFormat,
    pub color_space: wgpu::SurfaceColorSpace,
    pub status: HdrStatus,
}

pub(super) fn choose_surface_selection(
    caps: &wgpu::SurfaceCapabilities,
    display: &wgpu::DisplayHdrInfo,
    mode: HdrMode,
    scene_hdr: bool,
) -> SurfaceSelection {
    let hdr_format = caps
        .color_spaces(wgpu::TextureFormat::Rgba16Float)
        .contains(wgpu::SurfaceColorSpaces::EXTENDED_SRGB_LINEAR)
        .then_some(wgpu::TextureFormat::Rgba16Float);
    let supported = hdr_format.is_some() && scene_hdr;
    let active = mode != HdrMode::Off && supported;
    if active {
        let format = hdr_format.unwrap_or(wgpu::TextureFormat::Rgba16Float);
        let headroom = display.tone_map_headroom().unwrap_or(1.0).max(1.0);
        let peak_nits = display.luminance.and_then(|v| v.max_nits);
        return SurfaceSelection {
            format,
            view_format: format,
            color_space: wgpu::SurfaceColorSpace::ExtendedSrgbLinear,
            status: HdrStatus {
                requested: mode,
                supported,
                active: true,
                scene_hdr,
                color_space: HdrColorSpace::ExtendedSrgbLinear,
                headroom,
                peak_nits,
                fallback: None,
            },
        };
    }

    let format = caps
        .formats
        .iter()
        .copied()
        .find(wgpu::TextureFormat::is_srgb)
        .or_else(|| caps.formats.first().copied())
        .unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb);
    let view_format = srgb_surface_view_format(format).unwrap_or(format);
    SurfaceSelection {
        format,
        view_format,
        color_space: wgpu::SurfaceColorSpace::Srgb,
        status: HdrStatus {
            requested: mode,
            supported,
            active: false,
            scene_hdr,
            color_space: HdrColorSpace::SdrSrgb,
            headroom: 1.0,
            peak_nits: display.luminance.and_then(|v| v.max_nits),
            fallback: Some(if mode == HdrMode::Off {
                HdrFallback::Disabled
            } else {
                HdrFallback::SurfaceUnsupported
            }),
        },
    }
}

pub(super) const MAX_FRAME_RENDER_PIXELS: u64 = 16_777_216;

pub(crate) fn capped_render_size(width: u32, height: u32, max_dimension: u32) -> (u32, u32) {
    capped_render_size_with_pixel_limit(width, height, max_dimension, MAX_FRAME_RENDER_PIXELS)
}

pub(crate) fn capped_render_size_with_pixel_limit(
    width: u32,
    height: u32,
    max_dimension: u32,
    max_pixels: u64,
) -> (u32, u32) {
    let width = width.max(1);
    let height = height.max(1);
    let max_dimension = max_dimension.max(1);
    let max_pixels = max_pixels.max(1);
    let dim_scale = (max_dimension as f64 / width.max(height) as f64).min(1.0);
    let pixels = width as u64 * height as u64;
    let pixel_scale = if pixels > max_pixels {
        (max_pixels as f64 / pixels as f64).sqrt()
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

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PresentOutputConfig {
    hdr_active: f32,
    headroom: f32,
    _pad: [f32; 2],
}

const PRESENT_WGSL: &str = r#"
@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
struct ExposureUniform { value: vec4<f32> };
@group(0) @binding(2) var<uniform> exposure_state: ExposureUniform;
struct OutputUniform { value: vec4<f32> };
@group(0) @binding(3) var<uniform> output_state: OutputUniform;

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

fn aces_curve(x: vec3<f32>) -> vec3<f32> {
    return (x * (2.51 * x + vec3<f32>(0.03))) /
        (x * (2.43 * x + vec3<f32>(0.59)) + vec3<f32>(0.14));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let scene = max(textureSample(input_tex, input_sampler, in.uv).rgb, vec3<f32>(0.0));
    let exposed = scene * exp2(exposure_state.value.x);
    if output_state.value.x > 0.5 {
        let headroom = max(output_state.value.y, 1.0);
        return vec4<f32>(aces_curve(exposed / headroom) * headroom, 1.0);
    }
    return vec4<f32>(clamp(aces_curve(exposed), vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
"#;

// FXAA 3.11-quality post pass; see fxaa.wgsl for the algorithm + the
// green-as-luma and early-exit threshold documentation.
const FXAA_WGSL: &str = include_str!("fxaa.wgsl");

// SMAA 1x pass chain; see the three wgsl files for the algorithm, threshold
// and min-work-gating documentation (branch early-outs instead of a stencil
// prepass) and smaa_lut.rs for the procedural lookup textures.
const SMAA_EDGE_WGSL: &str = include_str!("smaa_edge.wgsl");
const SMAA_WEIGHTS_WGSL: &str = include_str!("smaa_weights.wgsl");
const SMAA_BLEND_WGSL: &str = include_str!("smaa_blend.wgsl");

// TAA v1 resolve pass; see taa.wgsl for the camera-reprojection algorithm,
// the no-motion-vectors tradeoff and the rejection rules.
const TAA_WGSL: &str = include_str!("taa.wgsl");

// History blend factor: 90% history / 10% current per frame (standard TAA
// starting point — fast enough convergence at 60fps, strong shimmer
// suppression; the neighborhood clamp bounds the added latency/ghosting).
const TAA_HISTORY_BLEND: f32 = 0.9;
// Device-z disocclusion threshold. Loose on purpose: device z is nonlinear
// and the history alpha stores it in f16, so a tight threshold would
// misfire at distance; the neighborhood clamp catches what slips through.
const TAA_DEPTH_REJECT: f32 = 0.01;
// History/resolve target format: filterable + renderable everywhere (core
// WebGPU), enough precision for HDR-headroom LDR values, and f16 alpha
// carries the depth for the disocclusion test.
const TAA_HISTORY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Halton(2,3) 8-sample sub-pixel jitter offsets in [-0.5, 0.5) pixels.
/// Standard TAA sequence: low-discrepancy so 8 frames cover the pixel
/// footprint evenly; index wraps so any u32 frame counter can drive it.
pub(super) fn taa_jitter_offset(frame_index: u32) -> [f32; 2] {
    fn halton(mut index: u32, base: u32) -> f32 {
        let mut f = 1.0f32;
        let mut result = 0.0f32;
        while index > 0 {
            f /= base as f32;
            result += f * (index % base) as f32;
            index /= base;
        }
        result
    }
    let index = (frame_index % 8) + 1;
    [halton(index, 2) - 0.5, halton(index, 3) - 0.5]
}

// Fullscreen blit: TAA resolves into the render-resolution history texture,
// this pass copies its rgb to the swapchain (linear upscale when the render
// size is capped, matching the FXAA/SMAA output stage).
const TAA_BLIT_WGSL: &str = r#"
@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

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

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Alpha carries scene depth for the resolve history; drop it here.
    return vec4<f32>(textureSampleLevel(src_tex, src_sampler, in.uv, 0.0).rgb, 1.0);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TaaUniformGpu {
    inv_view_proj: [[f32; 4]; 4],
    prev_view_proj: [[f32; 4]; 4],
    // [history blend, depth reject threshold, 0, 0].
    params: [f32; 4],
}

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
        let output_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_present_output"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
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
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(16),
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
            device: device.clone(),
            sampler,
            bgl,
            pipeline,
            exposure_bgl,
            exposure_pipeline,
            exposure_config_buffer,
            exposure_state_buffer,
            exposure_uniform_buffer,
            output_uniform_buffer,
            last_output_uniform: None,
            output_format,
            fxaa_active: false,
            fxaa: None,
            smaa_active: false,
            smaa: None,
            taa_active: false,
            taa: None,
        }
    }

    /// Turn the FXAA stage on/off. Off drops the lazily allocated target,
    /// pipeline, and bind group (pay-for-use); on allocates nothing until the
    /// first apply() that actually runs the pass.
    pub(super) fn set_fxaa_active(&mut self, active: bool) {
        self.fxaa_active = active;
        if !active {
            self.fxaa = None;
        }
    }

    /// Turn the SMAA stage on/off; same pay-for-use lifecycle as
    /// `set_fxaa_active` (off drops targets, pipelines and lookup textures;
    /// on allocates nothing until the first apply() that runs the passes).
    pub(super) fn set_smaa_active(&mut self, active: bool) {
        self.smaa_active = active;
        if !active {
            self.smaa = None;
        }
    }

    /// Turn the TAA stage on/off; same pay-for-use lifecycle as the other AA
    /// stages (off drops the history pair, pipelines and uniform buffer; on
    /// allocates nothing until the first apply() that runs the passes).
    pub(super) fn set_taa_active(&mut self, active: bool) {
        self.taa_active = active;
        if !active {
            self.taa = None;
        }
    }

    pub(super) fn taa_active(&self) -> bool {
        self.taa_active
    }

    /// Lazily (re)build FXAA resources for the current render size. The
    /// pipeline + layout survive resizes; only the LDR target and its bind
    /// group rebuild when dimensions change.
    fn ensure_fxaa(&mut self, dimensions: [u32; 2]) {
        let size = [dimensions[0].max(1), dimensions[1].max(1)];
        if self.fxaa.as_ref().is_some_and(|fxaa| fxaa.size == size) {
            return;
        }
        let device = self.device.clone();
        let (pipeline, bgl) = match self.fxaa.take() {
            Some(prev) => (prev.pipeline, prev.bgl),
            None => {
                let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("perro_fxaa_shader"),
                    source: wgpu::ShaderSource::Wgsl(FXAA_WGSL.into()),
                });
                let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("perro_fxaa_bgl"),
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
                    ],
                });
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("perro_fxaa_layout"),
                    bind_group_layouts: &[Some(&bgl)],
                    immediate_size: 0,
                });
                let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("perro_fxaa_pipeline"),
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
                            format: self.output_format,
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
                (pipeline, bgl)
            }
        };
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("perro_fxaa_target"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.output_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_fxaa_bg"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&target_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.fxaa = Some(FxaaResources {
            pipeline,
            bgl,
            _target: target,
            target_view,
            bind_group,
            size,
        });
    }

    /// Lazily (re)build SMAA resources for the current render size. The
    /// pipelines, layouts, samplers and lookup textures survive resizes;
    /// only the three render-size targets and their bind groups rebuild
    /// when dimensions change.
    fn ensure_smaa(&mut self, queue: &wgpu::Queue, dimensions: [u32; 2]) {
        let size = [dimensions[0].max(1), dimensions[1].max(1)];
        if self.smaa.as_ref().is_some_and(|smaa| smaa.size == size) {
            return;
        }
        let device = self.device.clone();
        let core = match self.smaa.take() {
            Some(prev) => prev.core,
            None => create_smaa_core(&device, queue, self.output_format),
        };

        let make_target = |label: &str, format: wgpu::TextureFormat| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };
        // Render-resolution chain: tonemapped LDR input, RG8 edges mask,
        // RGBA8 blending weights (both trackers of the render size).
        let ldr_target = make_target("perro_smaa_ldr_target", self.output_format);
        let ldr_view = ldr_target.create_view(&wgpu::TextureViewDescriptor::default());
        let edges_target = make_target("perro_smaa_edges_target", wgpu::TextureFormat::Rg8Unorm);
        let edges_view = edges_target.create_view(&wgpu::TextureViewDescriptor::default());
        let weights_target =
            make_target("perro_smaa_weights_target", wgpu::TextureFormat::Rgba8Unorm);
        let weights_view = weights_target.create_view(&wgpu::TextureViewDescriptor::default());

        let edge_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_smaa_edge_bg"),
            layout: &core.edge_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&ldr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&core.nearest_sampler),
                },
            ],
        });
        let weights_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_smaa_weights_bg"),
            layout: &core.weights_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&edges_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&core.area_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&core.search_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&core.nearest_sampler),
                },
            ],
        });
        let blend_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_smaa_blend_bg"),
            layout: &core.blend_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&ldr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&weights_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        self.smaa = Some(SmaaResources {
            core,
            _ldr_target: ldr_target,
            ldr_view,
            _edges_target: edges_target,
            edges_view,
            _weights_target: weights_target,
            weights_view,
            edge_bind_group,
            weights_bind_group,
            blend_bind_group,
            size,
        });
    }

    /// Lazily (re)build TAA resources for the current render size. The
    /// pipelines, layouts and uniform buffer survive resizes; the LDR target,
    /// history pair and blit bind groups rebuild when dimensions change
    /// (which also invalidates the history so no stale image is reprojected
    /// across a resize).
    fn ensure_taa(&mut self, dimensions: [u32; 2]) {
        let size = [dimensions[0].max(1), dimensions[1].max(1)];
        if self.taa.as_ref().is_some_and(|taa| taa.size == size) {
            return;
        }
        let device = self.device.clone();
        let core = match self.taa.take() {
            Some(prev) => prev.core,
            None => create_taa_core(&device, self.output_format),
        };

        let make_target = |label: &str, format: wgpu::TextureFormat| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };
        let ldr_target = make_target("perro_taa_ldr_target", self.output_format);
        let ldr_view = ldr_target.create_view(&wgpu::TextureViewDescriptor::default());
        let history_a = make_target("perro_taa_history_a", TAA_HISTORY_FORMAT);
        let history_b = make_target("perro_taa_history_b", TAA_HISTORY_FORMAT);
        let history_views = [
            history_a.create_view(&wgpu::TextureViewDescriptor::default()),
            history_b.create_view(&wgpu::TextureViewDescriptor::default()),
        ];
        let blit_bind_groups = [0usize, 1].map(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("perro_taa_blit_bg"),
                layout: &core.blit_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&history_views[i]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            })
        });

        self.taa = Some(TaaResources {
            core,
            _ldr_target: ldr_target,
            ldr_view,
            _history: [history_a, history_b],
            history_views,
            blit_bind_groups,
            history_index: 0,
            history_valid: false,
            size,
        });
    }

    /// Encode the TAA resolve + blit for this frame: reproject/blend into the
    /// write half of the history pair, then blit it to the swapchain. The
    /// resolve bind group is created per frame on purpose — the depth view is
    /// borrowed from the 3D pass (its texture can be recreated behind our
    /// back) and the history pair ping-pongs every frame, so caching would
    /// need invalidation tracking for little gain (one small bind group).
    fn encode_taa(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: &PresentTaaFrame,
        output_view: &wgpu::TextureView,
    ) {
        let Some(taa) = self.taa.as_mut() else {
            return;
        };
        let uniform = TaaUniformGpu {
            inv_view_proj: frame.inv_view_proj,
            prev_view_proj: frame.prev_view_proj,
            params: [
                if taa.history_valid {
                    TAA_HISTORY_BLEND
                } else {
                    0.0
                },
                TAA_DEPTH_REJECT,
                0.0,
                0.0,
            ],
        };
        queue.write_buffer(&taa.core.uniform_buffer, 0, bytemuck::bytes_of(&uniform));

        let read = taa.history_index;
        let write = read ^ 1;
        let resolve_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_taa_resolve_bg"),
            layout: &taa.core.resolve_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&taa.ldr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&taa.history_views[read]),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&frame.depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: taa.core.uniform_buffer.as_entire_binding(),
                },
            ],
        });
        let run_pass = |encoder: &mut wgpu::CommandEncoder,
                        label: &str,
                        view: &wgpu::TextureView,
                        pipeline: &wgpu::RenderPipeline,
                        bind_group: &wgpu::BindGroup| {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..3, 0..1);
        };
        // Pass 1: reproject + blend into the write half of the history pair
        // (rgb = resolved color, a = device depth for next frame's reject).
        run_pass(
            encoder,
            "perro_taa_resolve_pass",
            &taa.history_views[write],
            &taa.core.resolve_pipeline,
            &resolve_bind_group,
        );
        // Pass 2: blit the fresh history to the swapchain (upscaling when the
        // render size is capped). Next frame samples it as history — the
        // ping-pong swap below makes that read side without any copy.
        run_pass(
            encoder,
            "perro_taa_blit_pass",
            output_view,
            &taa.core.blit_pipeline,
            &taa.blit_bind_groups[write],
        );
        taa.history_index = write;
        taa.history_valid = true;
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
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.output_uniform_buffer.as_entire_binding(),
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
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        bind_groups: &PresentBindGroups,
        output_view: &wgpu::TextureView,
        dimensions: [u32; 2],
        delta_seconds: f32,
        settings: PresentExposureSettings,
        hdr_status: HdrStatus,
        taa_frame: Option<&PresentTaaFrame>,
    ) {
        // TAA needs scene depth; frames without a live 3D pipeline (2D-only
        // scenes) tonemap straight to the swapchain and drop history validity
        // so the accumulation restarts cleanly when 3D returns.
        let taa_run = self.taa_active && taa_frame.is_some();
        if taa_run {
            self.ensure_taa(dimensions);
        } else if self.taa_active
            && let Some(taa) = self.taa.as_mut()
        {
            taa.history_valid = false;
        }
        if self.smaa_active {
            self.ensure_smaa(queue, dimensions);
        } else if self.fxaa_active {
            self.ensure_fxaa(dimensions);
        }
        let output = PresentOutputConfig {
            hdr_active: if hdr_status.active { 1.0 } else { 0.0 },
            headroom: finite_or(hdr_status.headroom, 1.0).max(1.0),
            _pad: [0.0; 2],
        };
        // HDR status is display state, not frame state: push only on change.
        let output_bits = [
            output.hdr_active.to_bits(),
            output.headroom.to_bits(),
            output._pad[0].to_bits(),
            output._pad[1].to_bits(),
        ];
        if self.last_output_uniform != Some(output_bits) {
            self.last_output_uniform = Some(output_bits);
            queue.write_buffer(&self.output_uniform_buffer, 0, bytemuck::bytes_of(&output));
        }
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
        // With FXAA, SMAA or TAA active the tonemap resolves into a
        // render-resolution LDR target; the AA pass chain then reads it and
        // writes the swapchain (doing the upscale when the render size is
        // capped). UI composites onto the swapchain after this whole method,
        // so it stays un-blurred. The modes are mutually exclusive (one
        // anti_alias config value); TAA wins over SMAA wins over FXAA if
        // several flags are ever set.
        let taa = if taa_run { self.taa.as_ref() } else { None };
        let smaa = if taa.is_none() && self.smaa_active {
            self.smaa.as_ref()
        } else {
            None
        };
        let fxaa = if taa.is_none() && smaa.is_none() && self.fxaa_active {
            self.fxaa.as_ref()
        } else {
            None
        };
        let taa_encode = taa.is_some();
        let tonemap_view = taa
            .map(|taa| &taa.ldr_view)
            .or_else(|| smaa.map(|smaa| &smaa.ldr_view))
            .or_else(|| fxaa.map(|fxaa| &fxaa.target_view))
            .unwrap_or(output_view);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_present_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: tonemap_view,
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
        drop(pass);
        if let Some(fxaa) = fxaa {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_fxaa_pass"),
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
            pass.set_pipeline(&fxaa.pipeline);
            pass.set_bind_group(0, &fxaa.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        if let Some(smaa) = smaa {
            let run_pass = |encoder: &mut wgpu::CommandEncoder,
                            label: &str,
                            view: &wgpu::TextureView,
                            pipeline: &wgpu::RenderPipeline,
                            bind_group: &wgpu::BindGroup| {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some(label),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
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
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, bind_group, &[]);
                pass.draw(0..3, 0..1);
            };
            // Pass 1: luma edge detection (LDR -> RG8 edges mask).
            run_pass(
                encoder,
                "perro_smaa_edge_pass",
                &smaa.edges_view,
                &smaa.core.edge_pipeline,
                &smaa.edge_bind_group,
            );
            // Pass 2: blending-weight calculation (edges + LUTs -> RGBA8
            // weights; early-outs per pixel on a zero edges mask).
            run_pass(
                encoder,
                "perro_smaa_weights_pass",
                &smaa.weights_view,
                &smaa.core.weights_pipeline,
                &smaa.weights_bind_group,
            );
            // Pass 3: neighborhood blending (LDR + weights -> swapchain,
            // upscaling when the render size is capped; early-outs per pixel
            // on zero weights).
            run_pass(
                encoder,
                "perro_smaa_blend_pass",
                output_view,
                &smaa.core.blend_pipeline,
                &smaa.blend_bind_group,
            );
        }
        // TAA resolve + blit (mutually exclusive with the blocks above; runs
        // after them only to end their borrows before taking &mut self).
        if taa_encode && let Some(frame) = taa_frame {
            self.encode_taa(queue, encoder, frame, output_view);
        }
    }
}

fn smaa_texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn smaa_sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

/// Fullscreen-triangle post pipeline shared by the three SMAA passes.
fn create_smaa_pipeline(
    device: &wgpu::Device,
    label: &str,
    shader_src: &str,
    bgl: &wgpu::BindGroupLayout,
    target_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(shader_src.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(bgl)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
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
                format: target_format,
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
    })
}

/// Size-independent SMAA setup: generates + uploads the two lookup textures
/// (procedural, cached process-wide in smaa_lut.rs), creates the nearest
/// sampler and builds the three pass pipelines.
fn create_smaa_core(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    output_format: wgpu::TextureFormat,
) -> SmaaCore {
    let nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("perro_smaa_nearest_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    let upload_lut = |label: &str,
                      format: wgpu::TextureFormat,
                      width: u32,
                      height: u32,
                      bytes_per_texel: u32,
                      data: &[u8]| {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * bytes_per_texel),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        texture
    };
    let area_tex = upload_lut(
        "perro_smaa_area_tex",
        wgpu::TextureFormat::Rg8Unorm,
        smaa_lut::AREA_TEX_WIDTH,
        smaa_lut::AREA_TEX_HEIGHT,
        2,
        smaa_lut::area_tex_bytes(),
    );
    let area_view = area_tex.create_view(&wgpu::TextureViewDescriptor::default());
    let search_tex = upload_lut(
        "perro_smaa_search_tex",
        wgpu::TextureFormat::R8Unorm,
        smaa_lut::SEARCH_TEX_WIDTH,
        smaa_lut::SEARCH_TEX_HEIGHT,
        1,
        smaa_lut::search_tex_bytes(),
    );
    let search_view = search_tex.create_view(&wgpu::TextureViewDescriptor::default());

    let edge_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("perro_smaa_edge_bgl"),
        entries: &[smaa_texture_entry(0), smaa_sampler_entry(1)],
    });
    let weights_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("perro_smaa_weights_bgl"),
        entries: &[
            smaa_texture_entry(0),
            smaa_texture_entry(1),
            smaa_texture_entry(2),
            smaa_sampler_entry(3),
            smaa_sampler_entry(4),
        ],
    });
    let blend_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("perro_smaa_blend_bgl"),
        entries: &[
            smaa_texture_entry(0),
            smaa_texture_entry(1),
            smaa_sampler_entry(2),
        ],
    });

    let edge_pipeline = create_smaa_pipeline(
        device,
        "perro_smaa_edge_pipeline",
        SMAA_EDGE_WGSL,
        &edge_bgl,
        wgpu::TextureFormat::Rg8Unorm,
    );
    let weights_pipeline = create_smaa_pipeline(
        device,
        "perro_smaa_weights_pipeline",
        SMAA_WEIGHTS_WGSL,
        &weights_bgl,
        wgpu::TextureFormat::Rgba8Unorm,
    );
    let blend_pipeline = create_smaa_pipeline(
        device,
        "perro_smaa_blend_pipeline",
        SMAA_BLEND_WGSL,
        &blend_bgl,
        output_format,
    );

    SmaaCore {
        edge_pipeline,
        edge_bgl,
        weights_pipeline,
        weights_bgl,
        blend_pipeline,
        blend_bgl,
        nearest_sampler,
        _area_tex: area_tex,
        area_view,
        _search_tex: search_tex,
        search_view,
    }
}

/// Size-independent TAA setup: the resolve + blit pipelines/layouts and the
/// reprojection uniform buffer. The fullscreen-triangle pipeline shape is
/// shared with the SMAA passes via `create_smaa_pipeline`.
fn create_taa_core(device: &wgpu::Device, output_format: wgpu::TextureFormat) -> TaaCore {
    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("perro_taa_uniform"),
        size: std::mem::size_of::<TaaUniformGpu>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let resolve_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("perro_taa_resolve_bgl"),
        entries: &[
            // Current LDR + history are read with textureLoad/explicit-LOD
            // sampling; depth is Depth32Float bound as unfilterable float.
            smaa_texture_entry(0),
            smaa_texture_entry(1),
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            smaa_sampler_entry(3),
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<TaaUniformGpu>() as u64
                    ),
                },
                count: None,
            },
        ],
    });
    let blit_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("perro_taa_blit_bgl"),
        entries: &[smaa_texture_entry(0), smaa_sampler_entry(1)],
    });
    let resolve_pipeline = create_smaa_pipeline(
        device,
        "perro_taa_resolve_pipeline",
        TAA_WGSL,
        &resolve_bgl,
        TAA_HISTORY_FORMAT,
    );
    let blit_pipeline = create_smaa_pipeline(
        device,
        "perro_taa_blit_pipeline",
        TAA_BLIT_WGSL,
        &blit_bgl,
        output_format,
    );
    TaaCore {
        resolve_pipeline,
        resolve_bgl,
        blit_pipeline,
        blit_bgl,
        uniform_buffer,
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
    let hdr_supported = scene_hdr_supported(adapter);
    linear_render_format_with_hdr(surface_format, hdr_supported)
}

pub(super) fn scene_hdr_supported(adapter: &wgpu::Adapter) -> bool {
    let required = wgpu::TextureUsages::RENDER_ATTACHMENT
        | wgpu::TextureUsages::TEXTURE_BINDING
        | wgpu::TextureUsages::COPY_SRC;
    adapter
        .get_texture_format_features(wgpu::TextureFormat::Rgba16Float)
        .allowed_usages
        .contains(required)
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
    fn capped_render_size_honors_low_memory_pixel_limit() {
        assert_eq!(
            capped_render_size_with_pixel_limit(3840, 2160, 8192, 1920 * 1080),
            (1920, 1080)
        );
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

    #[test]
    fn fxaa_shader_parses_and_validates() {
        let module = naga::front::wgsl::parse_str(FXAA_WGSL).expect("FXAA WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("FXAA WGSL validates");
    }

    #[test]
    fn fxaa_uses_green_luma_with_early_exit() {
        // Green-as-luma + documented low-contrast early exit are load-bearing
        // for the pass cost; keep them from silently regressing.
        assert!(FXAA_WGSL.contains("return rgb.g;"));
        assert!(FXAA_WGSL.contains("EDGE_THRESHOLD_MIN"));
        assert!(FXAA_WGSL.contains("luma_max * EDGE_THRESHOLD_MAX"));
    }

    #[test]
    fn smaa_shaders_parse_and_validate() {
        for (name, src) in [
            ("edge", SMAA_EDGE_WGSL),
            ("weights", SMAA_WEIGHTS_WGSL),
            ("blend", SMAA_BLEND_WGSL),
        ] {
            let module = naga::front::wgsl::parse_str(src)
                .unwrap_or_else(|err| panic!("SMAA {name} WGSL parses: {err}"));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::empty(),
            )
            .validate(&module)
            .unwrap_or_else(|err| panic!("SMAA {name} WGSL validates: {err}"));
        }
    }

    #[test]
    fn smaa_uses_green_luma_with_documented_gating() {
        // Green-as-luma (consistent with FXAA), the standard SMAA thresholds
        // and the branch-based min-work gating are load-bearing for pass
        // cost/quality; keep them from silently regressing.
        assert!(SMAA_EDGE_WGSL.contains("point_sampler, uv, 0.0).g;"));
        assert!(SMAA_EDGE_WGSL.contains("const SMAA_THRESHOLD: f32 = 0.1;"));
        assert!(
            SMAA_EDGE_WGSL.contains("const SMAA_LOCAL_CONTRAST_ADAPTATION_FACTOR: f32 = 2.0;")
        );
        // Weights pass early-outs on the zero edges mask (no searches), the
        // blend pass on zero weights (center-color return).
        assert!(SMAA_WEIGHTS_WGSL.contains("if e.g > 0.0 {"));
        assert!(SMAA_WEIGHTS_WGSL.contains("if e.r > 0.0 {"));
        assert!(SMAA_BLEND_WGSL.contains("< 0.00001 {"));
    }

    #[test]
    fn anti_alias_mode_sample_counts() {
        use crate::AntiAliasMode;
        assert_eq!(AntiAliasMode::Off.sample_count(), 1);
        assert_eq!(AntiAliasMode::Fxaa.sample_count(), 1);
        assert_eq!(AntiAliasMode::Smaa.sample_count(), 1);
        assert_eq!(AntiAliasMode::Taa.sample_count(), 1);
        assert_eq!(AntiAliasMode::Msaa2.sample_count(), 2);
        assert_eq!(AntiAliasMode::Msaa4.sample_count(), 4);
    }

    #[test]
    fn taa_shaders_parse_and_validate() {
        for (name, src) in [("resolve", TAA_WGSL), ("blit", TAA_BLIT_WGSL)] {
            let module = naga::front::wgsl::parse_str(src)
                .unwrap_or_else(|err| panic!("TAA {name} WGSL parses: {err}"));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::empty(),
            )
            .validate(&module)
            .unwrap_or_else(|err| panic!("TAA {name} WGSL validates: {err}"));
        }
    }

    #[test]
    fn taa_resolve_keeps_documented_rejection_and_clamp() {
        // Camera-reprojection through world space, the off-screen/behind-
        // camera rejects, the depth-disocclusion reject via history alpha and
        // the 3x3 neighborhood clamp are load-bearing for v1 stability; keep
        // them from silently regressing.
        assert!(TAA_WGSL.contains("cfg.inv_view_proj * vec4<f32>(ndc, 1.0)"));
        assert!(TAA_WGSL.contains("cfg.prev_view_proj * vec4<f32>(world_pos, 1.0)"));
        assert!(TAA_WGSL.contains("if prev_clip.w > 1.0e-6"));
        assert!(TAA_WGSL.contains("prev_uv.x < 0.0 || prev_uv.x > 1.0"));
        assert!(TAA_WGSL.contains("abs(history.a - prev_depth_expected) > cfg.params.y"));
        assert!(TAA_WGSL.contains("clamp(history.rgb, c_min, c_max)"));
        // Output alpha must carry device depth for next frame's reject; the
        // blit pass must drop it.
        assert!(TAA_WGSL.contains("return vec4<f32>(resolved, depth);"));
        assert!(TAA_BLIT_WGSL.contains(".rgb, 1.0);"));
    }

    #[test]
    fn taa_jitter_follows_halton_2_3_eight_sample_cycle() {
        // Halton(2,3) for indices 1..=8, offset to [-0.5, 0.5).
        let expected_x = [
            0.5f32,
            0.25,
            0.75,
            0.125,
            0.625,
            0.375,
            0.875,
            0.0625,
        ];
        let expected_y = [
            1.0f32 / 3.0,
            2.0 / 3.0,
            1.0 / 9.0,
            4.0 / 9.0,
            7.0 / 9.0,
            2.0 / 9.0,
            5.0 / 9.0,
            8.0 / 9.0,
        ];
        for i in 0..8u32 {
            let [jx, jy] = taa_jitter_offset(i);
            assert!(
                (jx - (expected_x[i as usize] - 0.5)).abs() < 1.0e-6,
                "halton2 sample {i}"
            );
            assert!(
                (jy - (expected_y[i as usize] - 0.5)).abs() < 1.0e-6,
                "halton3 sample {i}"
            );
            // Sub-pixel bound: every offset stays inside the pixel footprint.
            assert!(jx.abs() <= 0.5 && jy.abs() <= 0.5, "sample {i} in bounds");
            // The sequence wraps every 8 frames (u32 counter safe).
            assert_eq!(taa_jitter_offset(i), taa_jitter_offset(i + 8));
        }
    }

    #[test]
    fn hdr_present_curve_keeps_values_above_sdr_white() {
        let exposed = 4.0_f32;
        let headroom = 4.0_f32;
        let x = exposed / headroom;
        let mapped = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14) * headroom;
        assert!(mapped > 1.0);
        assert!(PRESENT_WGSL.contains("aces_curve(exposed / headroom) * headroom"));
    }

    fn hdr_caps() -> wgpu::SurfaceCapabilities {
        wgpu::SurfaceCapabilities {
            formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
            format_capabilities: vec![
                wgpu::SurfaceFormatCapabilities {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    color_spaces: wgpu::SurfaceColorSpaces::SRGB,
                },
                wgpu::SurfaceFormatCapabilities {
                    format: wgpu::TextureFormat::Rgba16Float,
                    color_spaces: wgpu::SurfaceColorSpaces::EXTENDED_SRGB_LINEAR,
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn hdr_auto_picks_extended_linear_surface() {
        let display = wgpu::DisplayHdrInfo {
            luminance: Some(wgpu::DisplayLuminance {
                max_nits: Some(800.0),
                sdr_white_nits: Some(200.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        let selected = choose_surface_selection(&hdr_caps(), &display, HdrMode::Auto, true);
        assert_eq!(selected.format, wgpu::TextureFormat::Rgba16Float);
        assert_eq!(
            selected.color_space,
            wgpu::SurfaceColorSpace::ExtendedSrgbLinear
        );
        assert!(selected.status.active);
        assert_eq!(selected.status.headroom, 4.0);
        assert_eq!(selected.status.peak_nits, Some(800.0));
    }

    #[test]
    fn hdr_off_keeps_sdr_but_reports_support() {
        let selected = choose_surface_selection(
            &hdr_caps(),
            &wgpu::DisplayHdrInfo::default(),
            HdrMode::Off,
            true,
        );
        assert_eq!(selected.format, wgpu::TextureFormat::Bgra8UnormSrgb);
        assert!(selected.status.supported);
        assert!(!selected.status.active);
        assert_eq!(selected.status.fallback, Some(HdrFallback::Disabled));
    }

    #[test]
    fn hdr_falls_back_when_float_scene_path_missing() {
        let selected = choose_surface_selection(
            &hdr_caps(),
            &wgpu::DisplayHdrInfo::default(),
            HdrMode::On,
            false,
        );
        assert!(!selected.status.supported);
        assert!(!selected.status.active);
        assert_eq!(
            selected.status.fallback,
            Some(HdrFallback::SurfaceUnsupported)
        );
    }
}
