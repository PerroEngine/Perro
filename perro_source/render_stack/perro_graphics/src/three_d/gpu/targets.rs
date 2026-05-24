use super::*;

pub(super) fn create_hiz_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (
    wgpu::Texture,
    Vec<wgpu::TextureView>,
    wgpu::TextureView,
    u32,
    (u32, u32),
) {
    let width = width.max(1);
    let height = height.max(1);
    let max_dim = width.max(height);
    let mip_count = (u32::BITS - max_dim.leading_zeros()).max(1);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_hiz_texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: mip_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
        view_formats: &[],
    });
    let mut mip_views = Vec::with_capacity(mip_count as usize);
    for mip in 0..mip_count {
        mip_views.push(texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("perro_hiz_mip_view"),
            format: Some(wgpu::TextureFormat::R32Float),
            dimension: Some(wgpu::TextureViewDimension::D2),
            usage: Some(
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            ),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: mip,
            mip_level_count: Some(1),
            base_array_layer: 0,
            array_layer_count: Some(1),
        }));
    }
    let sample_view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("perro_hiz_sample_view"),
        format: Some(wgpu::TextureFormat::R32Float),
        dimension: Some(wgpu::TextureViewDimension::D2),
        usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
        aspect: wgpu::TextureAspect::All,
        base_mip_level: 0,
        mip_level_count: Some(mip_count),
        base_array_layer: 0,
        array_layer_count: Some(1),
    });
    (texture, mip_views, sample_view, mip_count, (width, height))
}

pub(super) fn compute_builtin_mesh_bounds(
    vertices: &[MeshVertex],
    indices: &[u32],
    ranges: &AHashMap<&'static str, MeshRange>,
) -> AHashMap<&'static str, ([f32; 3], f32)> {
    let mut out = AHashMap::new();
    for (name, range) in ranges {
        let start = range.index_start as usize;
        let end = start
            .saturating_add(range.index_count as usize)
            .min(indices.len());
        let mut pts = Vec::with_capacity(end.saturating_sub(start));
        for idx in &indices[start..end] {
            let vertex_index = range.base_vertex as i64 + *idx as i64;
            if vertex_index < 0 {
                continue;
            }
            let Some(v) = vertices.get(vertex_index as usize) else {
                continue;
            };
            pts.push(v.pos);
        }
        if let Some((c, r)) = mesh_bounds_from_positions(&pts) {
            out.insert(*name, (c, r));
        }
    }
    out
}

pub(super) fn mesh_bounds_from_vertices(vertices: &[MeshVertex]) -> Option<([f32; 3], f32)> {
    let positions: Vec<[f32; 3]> = vertices.iter().map(|v| v.pos).collect();
    mesh_bounds_from_positions(&positions)
}

pub(super) fn mesh_bounds_from_positions(positions: &[[f32; 3]]) -> Option<([f32; 3], f32)> {
    let mut it = positions.iter().copied();
    let first = it.next()?;
    let mut min = Vec3::from(first);
    let mut max = Vec3::from(first);
    for p in it {
        let v = Vec3::from(p);
        min = min.min(v);
        max = max.max(v);
    }
    let center = (min + max) * 0.5;
    let mut radius = 0.0f32;
    for p in positions {
        let d = (Vec3::from(*p) - center).length();
        if d > radius {
            radius = d;
        }
    }
    Some(([center.x, center.y, center.z], radius))
}

pub(super) fn create_depth_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    sample_count: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_depth3d"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
    (depth_texture, depth_view)
}

pub(super) fn create_depth_prepass_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_depth_prepass"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_PREPASS_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

pub(super) fn create_shadow_map_array_texture(
    device: &wgpu::Device,
    label: &'static str,
    size: u32,
    layers: u32,
) -> (wgpu::Texture, wgpu::TextureView, Vec<wgpu::TextureView>) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size.max(1),
            height: size.max(1),
            depth_or_array_layers: layers.max(1),
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: SHADOW_DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let array_view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some(label),
        format: Some(SHADOW_DEPTH_FORMAT),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
        aspect: wgpu::TextureAspect::DepthOnly,
        base_mip_level: 0,
        mip_level_count: Some(1),
        base_array_layer: 0,
        array_layer_count: Some(layers.max(1)),
    });
    let layer_views = (0..layers.max(1))
        .map(|layer| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(label),
                format: Some(SHADOW_DEPTH_FORMAT),
                dimension: Some(wgpu::TextureViewDimension::D2),
                usage: Some(wgpu::TextureUsages::RENDER_ATTACHMENT),
                aspect: wgpu::TextureAspect::DepthOnly,
                base_mip_level: 0,
                mip_level_count: Some(1),
                base_array_layer: layer,
                array_layer_count: Some(1),
            })
        })
        .collect();
    (texture, array_view, layer_views)
}

pub(super) fn create_sky_pipeline(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    sample_count: u32,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_sky3d_pipeline"),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}

pub(super) fn create_sky_noise_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
    const SIZE: u32 = 128;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let i = ((y * SIZE + x) * 4) as usize;
            rgba[i] = sky_noise_hash(x, y, 0x8da6_b343);
            rgba[i + 1] = sky_noise_hash(x, y, 0xd816_3841);
            rgba[i + 2] = sky_noise_hash(x, y, 0xcb1a_3c6d);
            rgba[i + 3] = sky_noise_hash(x, y, 0x1656_67b1);
        }
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_sky_noise_cache"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
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
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(SIZE * 4),
            rows_per_image: Some(SIZE),
        },
        wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("perro_sky_noise_sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    (texture, view, sampler)
}

fn sky_noise_hash(x: u32, y: u32, seed: u32) -> u8 {
    let mut n = x
        .wrapping_mul(0x9e37_79b1)
        .wrapping_add(y.wrapping_mul(0x85eb_ca6b))
        .wrapping_add(seed);
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb_352d);
    n ^= n >> 15;
    n = n.wrapping_mul(0x846c_a68b);
    n ^= n >> 16;
    (n & 0xff) as u8
}
