use perro_structs::TextureFilterMode;

pub(crate) struct RgbaMipLevel {
    pub(crate) rgba: Vec<u8>,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[inline]
pub(crate) fn rgba_mip_level_count(width: u32, height: u32) -> u32 {
    let max_dim = width.max(height).max(1);
    u32::BITS - max_dim.leading_zeros()
}

pub(crate) fn build_rgba_levels_for_filter(
    rgba: &[u8],
    width: u32,
    height: u32,
    filter: TextureFilterMode,
) -> Vec<RgbaMipLevel> {
    let width = width.max(1);
    let height = height.max(1);
    let base_len = width as usize * height as usize * 4;
    if rgba.len() < base_len {
        return fallback_mip_chain();
    }
    let base = rgba[..base_len].to_vec();
    if filter.uses_mipmaps() {
        build_rgba_mip_chain_from_base(base, width, height)
    } else {
        vec![RgbaMipLevel {
            rgba: base,
            width,
            height,
        }]
    }
}

pub(crate) fn build_rgba_levels_for_filter_owned(
    mut rgba: Vec<u8>,
    width: u32,
    height: u32,
    filter: TextureFilterMode,
) -> Vec<RgbaMipLevel> {
    let width = width.max(1);
    let height = height.max(1);
    let base_len = width as usize * height as usize * 4;
    if rgba.len() < base_len {
        return fallback_mip_chain();
    }
    rgba.truncate(base_len);
    if filter.uses_mipmaps() {
        build_rgba_mip_chain_from_base(rgba, width, height)
    } else {
        vec![RgbaMipLevel {
            rgba,
            width,
            height,
        }]
    }
}

fn build_rgba_mip_chain_from_base(rgba: Vec<u8>, width: u32, height: u32) -> Vec<RgbaMipLevel> {
    let mut levels = Vec::with_capacity(rgba_mip_level_count(width, height) as usize);
    levels.push(RgbaMipLevel {
        rgba,
        width,
        height,
    });

    while let Some(prev) = levels
        .last()
        .filter(|level| level.width > 1 || level.height > 1)
    {
        let next_width = (prev.width / 2).max(1);
        let next_height = (prev.height / 2).max(1);
        let mut next = vec![0u8; next_width as usize * next_height as usize * 4];

        for y in 0..next_height {
            for x in 0..next_width {
                let sx = x * 2;
                let sy = y * 2;
                let x1 = (sx + 1).min(prev.width - 1);
                let y1 = (sy + 1).min(prev.height - 1);
                let samples = [(sx, sy), (x1, sy), (sx, y1), (x1, y1)];
                let dst = ((y * next_width + x) * 4) as usize;

                let alpha_sum = samples.iter().fold(0u32, |acc, &(px, py)| {
                    let src = ((py * prev.width + px) * 4) as usize + 3;
                    acc + prev.rgba[src] as u32
                });
                let alpha = ((alpha_sum + 2) / 4) as u8;
                for c in 0..3 {
                    let sum = samples.iter().fold(0.0f32, |acc, &(px, py)| {
                        let src = ((py * prev.width + px) * 4) as usize + c;
                        acc + srgb_u8_to_linear(prev.rgba[src])
                    });
                    next[dst + c] = linear_to_srgb_u8(sum * 0.25);
                }
                next[dst + 3] = alpha;
            }
        }

        levels.push(RgbaMipLevel {
            rgba: next,
            width: next_width,
            height: next_height,
        });
    }

    levels
}

#[inline]
fn srgb_u8_to_linear(v: u8) -> f32 {
    let c = v as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
fn linear_to_srgb_u8(v: f32) -> u8 {
    let c = if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (c.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn fallback_mip_chain() -> Vec<RgbaMipLevel> {
    vec![RgbaMipLevel {
        rgba: vec![255, 255, 255, 255],
        width: 1,
        height: 1,
    }]
}

pub(crate) fn write_rgba_mip_chain(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    levels: &[RgbaMipLevel],
) {
    for (mip_level, level) in levels.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: mip_level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &level.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * level.width),
                rows_per_image: Some(level.height),
            },
            wgpu::Extent3d {
                width: level.width,
                height: level.height,
                depth_or_array_layers: 1,
            },
        );
    }
}

/// Upload rgba into mip level 0 of an existing texture (no mip regen). Used by
/// the stream-texture in-place path so per-frame webcam/video writes reuse the
/// resident GPU texture + bind group instead of recreating them.
pub(crate) fn write_texture_base_level(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    rgba: &[u8],
) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
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
}

pub(crate) fn sampler_descriptor(
    label: &'static str,
    filter: TextureFilterMode,
    address_mode: wgpu::AddressMode,
) -> wgpu::SamplerDescriptor<'static> {
    match filter {
        TextureFilterMode::Nearest => wgpu::SamplerDescriptor {
            label: Some(label),
            address_mode_u: address_mode,
            address_mode_v: address_mode,
            address_mode_w: address_mode,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        },
        TextureFilterMode::Linear => wgpu::SamplerDescriptor {
            label: Some(label),
            address_mode_u: address_mode,
            address_mode_v: address_mode,
            address_mode_w: address_mode,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        },
        TextureFilterMode::LinearMipmap => wgpu::SamplerDescriptor {
            label: Some(label),
            address_mode_u: address_mode,
            address_mode_v: address_mode,
            address_mode_w: address_mode,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        },
        TextureFilterMode::Anisotropic => wgpu::SamplerDescriptor {
            label: Some(label),
            address_mode_u: address_mode,
            address_mode_v: address_mode,
            address_mode_w: address_mode,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_mip_level_count_tracks_max_dim() {
        assert_eq!(rgba_mip_level_count(1, 1), 1);
        assert_eq!(rgba_mip_level_count(2, 1), 2);
        assert_eq!(rgba_mip_level_count(3, 5), 3);
        assert_eq!(rgba_mip_level_count(256, 128), 9);
    }

    #[test]
    fn build_rgba_mip_chain_halves_until_one() {
        let rgba = vec![128u8; 4 * 4 * 2];
        let levels = build_rgba_levels_for_filter(&rgba, 4, 2, TextureFilterMode::LinearMipmap);
        let dims: Vec<(u32, u32)> = levels
            .iter()
            .map(|level| (level.width, level.height))
            .collect();
        assert_eq!(dims, vec![(4, 2), (2, 1), (1, 1)]);
    }

    #[test]
    fn build_rgba_mip_chain_averages_pixels() {
        let rgba = vec![0, 0, 0, 0, 100, 0, 0, 100, 200, 0, 0, 200, 255, 0, 0, 255];
        let levels = build_rgba_levels_for_filter(&rgba, 2, 2, TextureFilterMode::LinearMipmap);
        assert_eq!(levels.len(), 2);
        assert_eq!(levels[1].rgba, vec![175, 0, 0, 139]);
    }

    #[test]
    fn linear_filter_keeps_single_level() {
        let rgba = vec![128u8; 4 * 4 * 4];
        let levels = build_rgba_levels_for_filter(&rgba, 4, 4, TextureFilterMode::Linear);
        assert_eq!(levels.len(), 1);
        assert_eq!((levels[0].width, levels[0].height), (4, 4));
    }
}
