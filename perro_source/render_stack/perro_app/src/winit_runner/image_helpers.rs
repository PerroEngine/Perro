//! Project image loading helpers for window icons and startup splash sizing.

#[cfg(not(target_arch = "wasm32"))]
use perro_asset_formats::ptex::{
    FLAG_FORMAT_MASK as PTEX_FLAG_FORMAT_MASK, FLAG_FORMAT_R8 as PTEX_FLAG_FORMAT_R8,
    FLAG_FORMAT_RGB8 as PTEX_FLAG_FORMAT_RGB8, FLAG_FORMAT_RGBA8 as PTEX_FLAG_FORMAT_RGBA8,
    FLAG_PAYLOAD_RAW as PTEX_FLAG_PAYLOAD_RAW,
};
#[cfg(not(target_arch = "wasm32"))]
use perro_asset_formats::ptex::{MAGIC as PTEX_MAGIC, VERSION as PTEX_VERSION};
#[cfg(not(target_arch = "wasm32"))]
use perro_graphics_assets::{
    decode_image_logical_size as decode_source_image_logical_size,
    decode_image_rgba as decode_source_image_rgba, decode_image_size as decode_source_image_size,
};
#[cfg(not(target_arch = "wasm32"))]
use perro_io::decompress_zlib;
#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs,
    path::{Path, PathBuf},
};
#[cfg(not(target_arch = "wasm32"))]
use winit::window::Icon;

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn load_project_window_icon(project: &perro_runtime::RuntimeProject) -> Option<Icon> {
    let bytes = load_project_icon_bytes(project)?;
    let (rgba, width, height) = decode_image_rgba(&bytes)?;
    Icon::from_rgba(rgba, width, height).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn load_project_icon_bytes(project: &perro_runtime::RuntimeProject) -> Option<Vec<u8>> {
    load_project_image_bytes(
        project,
        project.config.icon.trim(),
        project.config.icon_hash,
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn load_project_image_bytes(
    project: &perro_runtime::RuntimeProject,
    source: &str,
    source_hash: Option<u64>,
) -> Option<Vec<u8>> {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = resolve_project_asset_path(project, source)
        && let Ok(bytes) = fs::read(path)
    {
        return Some(bytes);
    }
    if let Some(lookup) = project.static_icon_lookup {
        let hash = source_hash
            .or_else(|| perro_ids::parse_hashed_source_uri(source))
            .or_else(|| {
                source
                    .starts_with("res://")
                    .then(|| perro_ids::string_to_u64(source))
            });
        if let Some(hash) = hash {
            let bytes = lookup(hash);
            if !bytes.is_empty() {
                return Some(bytes.to_vec());
            }
        }
    }
    None
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy)]
pub(crate) struct LoadedImageSizes {
    pub display: (u32, u32),
    pub texture: (u32, u32),
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn load_image_sizes(
    project: &perro_runtime::RuntimeProject,
    source: &str,
    source_hash: Option<u64>,
) -> Option<LoadedImageSizes> {
    let bytes = load_project_image_bytes(project, source, source_hash)?;
    Some(LoadedImageSizes {
        display: decode_image_logical_size(&bytes)?,
        texture: decode_image_size(&bytes)?,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if let Some((width, height)) = decode_ptex_dimensions(bytes) {
        return Some((width.max(1), height.max(1)));
    }
    decode_source_image_size(bytes)
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_image_logical_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if let Some((width, height)) = decode_ptex_dimensions(bytes) {
        return Some((width.max(1), height.max(1)));
    }
    decode_source_image_logical_size(bytes)
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_image_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    if let Some(decoded) = decode_ptex_rgba(bytes) {
        return Some(decoded);
    }
    decode_source_image_rgba(bytes)
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_ptex_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 16 || &bytes[0..4] != PTEX_MAGIC {
        return None;
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    if version != PTEX_VERSION {
        return None;
    }
    let width = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
    let height = u32::from_le_bytes(bytes[12..16].try_into().ok()?);
    if width == 0 || height == 0 {
        return None;
    }
    Some((width, height))
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_ptex_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    if bytes.len() < 24 || &bytes[0..4] != PTEX_MAGIC {
        return None;
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    if version != PTEX_VERSION {
        return None;
    }
    let width = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
    let height = u32::from_le_bytes(bytes[12..16].try_into().ok()?);
    if width == 0 || height == 0 {
        return None;
    }
    let flags = u32::from_le_bytes(bytes[16..20].try_into().ok()?);
    if flags & !(PTEX_FLAG_FORMAT_MASK | PTEX_FLAG_PAYLOAD_RAW) != 0 {
        return None;
    }
    let raw_len = u32::from_le_bytes(bytes[20..24].try_into().ok()?);
    let pixel_count = width.checked_mul(height)? as usize;
    let expected_raw_len = match flags & PTEX_FLAG_FORMAT_MASK {
        PTEX_FLAG_FORMAT_RGBA8 => pixel_count.checked_mul(4)?,
        PTEX_FLAG_FORMAT_RGB8 => pixel_count.checked_mul(3)?,
        PTEX_FLAG_FORMAT_R8 => pixel_count,
        _ => return None,
    };
    if raw_len as usize != expected_raw_len {
        return None;
    }
    let raw = decode_ptex_payload(flags, &bytes[24..])?;
    if raw.len() != expected_raw_len {
        return None;
    }

    let rgba = match flags & PTEX_FLAG_FORMAT_MASK {
        PTEX_FLAG_FORMAT_RGBA8 => raw,
        PTEX_FLAG_FORMAT_RGB8 => {
            let mut out = Vec::with_capacity(pixel_count * 4);
            for px in raw.chunks_exact(3) {
                out.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            out
        }
        PTEX_FLAG_FORMAT_R8 => {
            let mut out = Vec::with_capacity(pixel_count * 4);
            for &v in &raw {
                out.extend_from_slice(&[v, v, v, 255]);
            }
            out
        }
        _ => return None,
    };
    Some((rgba, width, height))
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_ptex_payload(flags: u32, payload: &[u8]) -> Option<Vec<u8>> {
    if (flags & PTEX_FLAG_PAYLOAD_RAW) != 0 {
        Some(payload.to_vec())
    } else {
        decompress_zlib(payload).ok()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_project_asset_path(
    project: &perro_runtime::RuntimeProject,
    source: &str,
) -> Option<PathBuf> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }

    if let Some(rel) = source.strip_prefix("res://") {
        let rel = rel.trim_start_matches('/');
        return Some(project.root.join("res").join(rel));
    }

    let path = Path::new(source);
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }

    Some(project.root.join(path))
}

#[cfg(test)]
mod tests {
    use super::{decode_image_logical_size, decode_image_rgba, decode_image_size};

    #[test]
    fn decode_image_rgba_supports_ptex_v1_rgb() {
        let raw_rgb = vec![10u8, 20, 30, 40, 50, 60];
        let compressed = perro_io::compress_zlib_best(&raw_rgb).expect("compress");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PTEX");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&(raw_rgb.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&compressed);

        let (rgba, width, height) = decode_image_rgba(&bytes).expect("decode rgba");
        assert_eq!((width, height), (2, 1));
        assert_eq!(rgba, vec![10u8, 20, 30, 255, 40, 50, 60, 255]);
    }

    #[test]
    fn decode_image_size_supports_ptex_v1() {
        let raw = vec![1u8, 2, 3, 4];
        let compressed = perro_io::compress_zlib_best(&raw).expect("compress");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PTEX");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(&compressed);

        assert_eq!(decode_image_size(&bytes), Some((1, 1)));
    }

    #[test]
    fn decode_image_helpers_support_svg_icon_and_splash_size() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="3" height="2"><rect width="3" height="2" fill="red"/></svg>"#;
        let (rgba, width, height) = decode_image_rgba(svg).expect("decode svg");
        assert_eq!((width, height), (12, 8));
        assert_eq!(rgba.len(), 12 * 8 * 4);
        assert_eq!(decode_image_size(svg), Some((12, 8)));
        assert_eq!(decode_image_logical_size(svg), Some((3, 2)));
    }
}
