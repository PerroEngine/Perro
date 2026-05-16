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

impl PresentProcessor {
    pub(super) fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_present_shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;

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
    let c = textureSample(input_tex, input_sampler, in.uv);
    return vec4<f32>(c.rgb, 1.0);
}
"#
                .into(),
            ),
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
        Self {
            sampler,
            bgl,
            pipeline,
        }
    }

    pub(super) fn create_bind_group(
        &self,
        device: &wgpu::Device,
        input_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
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
            ],
        })
    }

    pub(super) fn apply(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        bind_group: &wgpu::BindGroup,
        output_view: &wgpu::TextureView,
    ) {
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
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

pub(super) fn linear_render_format(surface_format: wgpu::TextureFormat) -> wgpu::TextureFormat {
    match surface_format {
        wgpu::TextureFormat::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8Unorm,
        _ => surface_format,
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
            return *mode;
        }
    }
    modes.first().copied().unwrap_or(wgpu::PresentMode::Fifo)
}

pub(super) fn choose_max_frame_latency(_vsync_enabled: bool) -> u32 {
    #[cfg(target_arch = "wasm32")]
    let default = 1;
    #[cfg(not(target_arch = "wasm32"))]
    let default = if _vsync_enabled { 3 } else { 1 };
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
