use super::*;

#[inline]
pub(super) fn next_generation(current: u32) -> u32 {
    let next = current.wrapping_add(1);
    if next == 0 { 1 } else { next }
}

#[inline]
pub(super) fn lut_hash_2d(effect: &PostProcessEffect) -> u64 {
    match effect {
        PostProcessEffect::Lut2D {
            texture_path, size, ..
        } => lut_key(texture_path.as_ref(), *size),
        _ => 0,
    }
}

#[inline]
pub(super) fn lut_hash_3d(effect: &PostProcessEffect) -> u64 {
    match effect {
        PostProcessEffect::Lut3D {
            texture_path, size, ..
        } => lut_key(texture_path.as_ref(), *size),
        _ => 0,
    }
}

#[inline]
pub(super) fn align_up_uniform(value: u64, alignment: u64) -> u64 {
    if alignment <= 1 {
        return value.max(1);
    }
    value.div_ceil(alignment) * alignment
}

pub(super) fn create_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_post_pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

pub(super) fn create_color_target(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        // COPY_SRC: the mesh-blend seam pass copies the scene aside before
        // rewriting it in place.
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

pub(super) fn create_default_lut_2d(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::Texture, wgpu::TextureView) {
    let mut rgba = Vec::with_capacity(2 * 2 * 2 * 4);
    for g in 0..2u8 {
        for b in 0..2u8 {
            for r in 0..2u8 {
                rgba.extend_from_slice(&[r * 255, g * 255, b * 255, 255]);
            }
        }
    }
    let cached = create_post_lut_2d(device, queue, rgba, 4, 2);
    (cached.texture, cached.view)
}

pub(super) fn create_default_lut_3d(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::Texture, wgpu::TextureView) {
    let mut rgba = Vec::with_capacity(2 * 2 * 2 * 4);
    for b in 0..2u8 {
        for g in 0..2u8 {
            for r in 0..2u8 {
                rgba.extend_from_slice(&[r * 255, g * 255, b * 255, 255]);
            }
        }
    }
    let cached = create_post_lut_3d(device, queue, rgba, 2);
    (cached.texture, cached.view)
}

pub(super) fn create_post_lut_2d(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
) -> CachedPostTexture {
    let width = width.max(1);
    let height = height.max(1);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_post_lut_2d"),
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
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2),
        ..Default::default()
    });
    CachedPostTexture { texture, view }
}

pub(super) fn create_post_lut_3d(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    rgba: Vec<u8>,
    size: u32,
) -> CachedPostTexture {
    let size = size.max(1);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_post_lut_3d"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: size,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
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
            bytes_per_row: Some(4 * size),
            rows_per_image: Some(size),
        },
        wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: size,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D3),
        ..Default::default()
    });
    CachedPostTexture { texture, view }
}

pub(super) fn load_post_texture_rgba(
    source: &str,
    static_texture_lookup: Option<StaticTextureLookup>,
) -> Option<(Vec<u8>, u32, u32)> {
    if let Some(lookup) = static_texture_lookup {
        let source_hash = perro_ids::parse_hashed_source_uri(source)
            .unwrap_or_else(|| perro_ids::string_to_u64(source));
        let bytes = lookup(source_hash);
        if !bytes.is_empty() {
            if let Some(decoded) = decode_ptex(bytes) {
                return Some(decoded);
            }
            return decode_image_rgba(bytes);
        }
    }
    let bytes = load_asset(source).ok()?;
    decode_image_rgba(&bytes)
}

pub(super) fn flattened_lut_to_3d(
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    requested_size: u32,
) -> Option<(Vec<u8>, u32)> {
    let size = if requested_size > 0 {
        requested_size
    } else if width == height.saturating_mul(height) {
        height
    } else if height == width.saturating_mul(width) {
        width
    } else {
        return None;
    };
    if size == 0 {
        return None;
    }
    let expected = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    if rgba.len() < expected {
        return None;
    }
    let voxel_count = (size as usize)
        .checked_mul(size as usize)?
        .checked_mul(size as usize)?;
    let mut out = vec![0u8; voxel_count.checked_mul(4)?];
    if width == size.saturating_mul(size) && height == size {
        for z in 0..size {
            for y in 0..size {
                for x in 0..size {
                    copy_lut_texel(&rgba, &mut out, width, (x + z * size, y), (x, y, z), size);
                }
            }
        }
        Some((out, size))
    } else if width == size && height == size.saturating_mul(size) {
        for z in 0..size {
            for y in 0..size {
                for x in 0..size {
                    copy_lut_texel(&rgba, &mut out, width, (x, y + z * size), (x, y, z), size);
                }
            }
        }
        Some((out, size))
    } else {
        None
    }
}

pub(super) fn copy_lut_texel(
    src: &[u8],
    dst: &mut [u8],
    src_width: u32,
    src_pos: (u32, u32),
    dst_pos: (u32, u32, u32),
    size: u32,
) {
    let (src_x, src_y) = src_pos;
    let (dst_x, dst_y, dst_z) = dst_pos;
    let src_index = ((src_y * src_width + src_x) * 4) as usize;
    let dst_index = (((dst_z * size + dst_y) * size + dst_x) * 4) as usize;
    dst[dst_index..dst_index + 4].copy_from_slice(&src[src_index..src_index + 4]);
}

pub(super) fn post_shader_key(path: &str) -> u64 {
    perro_ids::parse_hashed_source_uri(path).unwrap_or_else(|| perro_ids::string_to_u64(path))
}

pub(super) fn lut_key(path: &str, size: u32) -> u64 {
    post_shader_key(path) ^ (u64::from(size) << 32) ^ 0x517c_c1b7_2722_0a95
}

pub(super) fn projection_uniform_params(camera: &Camera3DState) -> (u32, f32, f32) {
    match camera.projection {
        CameraProjectionState::Perspective { near, far, .. } => (0, near, far),
        CameraProjectionState::Orthographic { near, far, .. } => (1, near, far),
        CameraProjectionState::Frustum { near, far, .. } => (2, near, far),
    }
}

// Refactored to return the new struct, simplifying the return type
