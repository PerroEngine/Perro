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
