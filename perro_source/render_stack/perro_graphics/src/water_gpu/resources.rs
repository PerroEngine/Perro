use super::*;

pub(super) fn decode_water_readback(
    cells: &[[f32; 4]],
    nodes: &[NodeID],
    water_sample_count: usize,
    queries: &[WaterReadbackQuery],
    samples: &mut Vec<WaterSampleState>,
    body_samples: &mut Vec<WaterBodySampleState>,
) {
    samples.clear();
    body_samples.clear();
    for (idx, node) in nodes.iter().take(water_sample_count).enumerate() {
        let cell = cells.get(idx).copied().unwrap_or([0.0; 4]);
        samples.push(WaterSampleState {
            node: *node,
            height: cell[0],
            velocity: [cell[1], 0.0],
            foam: cell[2],
        });
    }
    let mut query_base = water_sample_count;
    for sample in queries {
        let c00 = cells.get(query_base).copied().unwrap_or([0.0; 4]);
        let c10 = cells.get(query_base + 1).copied().unwrap_or(c00);
        let c01 = cells.get(query_base + 2).copied().unwrap_or(c00);
        let c11 = cells.get(query_base + 3).copied().unwrap_or(c10);
        query_base += 4;
        let cell = water_lerp_cell(c00, c10, c01, c11, sample.frac);
        let query = sample.query;
        body_samples.push(WaterBodySampleState {
            water: query.water,
            body: query.body,
            point: query.point,
            local: query.local,
            height: cell[0],
            velocity: [cell[1], 0.0],
            foam: cell[2],
        });
    }
}

pub(super) fn empty_buffer(
    device: &wgpu::Device,
    label: &str,
    count: usize,
    water: bool,
) -> wgpu::Buffer {
    let elem = if water {
        std::mem::size_of::<WaterGpu>()
    } else {
        std::mem::size_of::<[f32; 4]>()
    };
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (count.max(1) * elem) as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

pub(super) fn readback_buffer(device: &wgpu::Device, cell_count: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("perro_water_gpu_readback"),
        size: (cell_count.max(1) * std::mem::size_of::<[f32; 4]>()) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    })
}

pub(super) fn readback_interval_seconds(rate_hz: f32) -> f32 {
    if !rate_hz.is_finite() || rate_hz <= 0.0 {
        return 0.0;
    }
    1.0 / rate_hz.clamp(1.0, 240.0)
}

pub(super) fn water_adaptive_readback_interval(
    base_rate_hz: f32,
    ripple_blend: f32,
    has_queries: bool,
    has_impacts: bool,
) -> f32 {
    let active_scale = if has_queries || has_impacts || ripple_blend >= 0.85 {
        1.0
    } else if ripple_blend >= 0.45 {
        0.5
    } else {
        0.25
    };
    readback_interval_seconds(base_rate_hz * active_scale)
}

pub(super) fn make_compute_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffers: ComputeBindGroupBuffers<'_>,
    label: &'static str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buffers.waters.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: buffers.cells.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: buffers.params.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: buffers.coastline.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: buffers.next_cells.as_entire_binding(),
            },
        ],
    })
}

pub(super) struct ComputeBindGroupBuffers<'a> {
    pub(super) waters: &'a wgpu::Buffer,
    pub(super) cells: &'a wgpu::Buffer,
    pub(super) next_cells: &'a wgpu::Buffer,
    pub(super) coastline: &'a wgpu::Buffer,
    pub(super) params: &'a wgpu::Buffer,
}

pub(super) struct RenderBindGroupBuffers<'a> {
    pub(super) waters: &'a wgpu::Buffer,
    pub(super) cells: &'a wgpu::Buffer,
    pub(super) coastline: &'a wgpu::Buffer,
    pub(super) render_chunks: &'a wgpu::Buffer,
    pub(super) params: &'a wgpu::Buffer,
}

pub(super) fn make_render_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffers: RenderBindGroupBuffers<'_>,
    label: &'static str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buffers.waters.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: buffers.cells.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: buffers.params.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: buffers.coastline.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: buffers.render_chunks.as_entire_binding(),
            },
        ],
    })
}

pub(super) fn make_depth_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    scene_depth_view: &wgpu::TextureView,
    scene_color_view: &wgpu::TextureView,
    label: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(scene_depth_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(scene_color_view),
            },
        ],
    })
}

// Fullscreen-triangle downsample: linear-samples the full-res scene into the
// reduced-resolution refraction copy. Replaces the 1:1 copy_texture_to_texture
// on the non-MSAA path so the copy target can stay at half render resolution.
pub(super) const WATER_SCENE_COLOR_BLIT_WGSL: &str = r#"
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
    return textureSample(src_tex, src_sampler, in.uv);
}
"#;

pub(super) struct SceneColorBlit {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) bgl: wgpu::BindGroupLayout,
    pub(super) sampler: wgpu::Sampler,
}

pub(super) fn create_scene_color_blit(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
) -> SceneColorBlit {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_water_scene_color_blit_shader"),
        source: wgpu::ShaderSource::Wgsl(WATER_SCENE_COLOR_BLIT_WGSL.into()),
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("perro_water_scene_color_blit_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("perro_water_scene_color_blit_bgl"),
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
        label: Some("perro_water_scene_color_blit_layout"),
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_water_scene_color_blit_pipeline"),
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
                format: color_format,
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
    SceneColorBlit {
        pipeline,
        bgl,
        sampler,
    }
}

pub(super) fn create_scene_color_texture(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_water_scene_color"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
