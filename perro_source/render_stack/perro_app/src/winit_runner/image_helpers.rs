//! Project image loading helpers for window icons and startup splash sizing.

use perro_asset_formats::ptex::{
    FLAG_FORMAT_MASK as PTEX_FLAG_FORMAT_MASK, FLAG_FORMAT_R8 as PTEX_FLAG_FORMAT_R8,
    FLAG_FORMAT_RGB8 as PTEX_FLAG_FORMAT_RGB8, FLAG_FORMAT_RGBA8 as PTEX_FLAG_FORMAT_RGBA8,
    FLAG_PAYLOAD_RAW as PTEX_FLAG_PAYLOAD_RAW, MAGIC as PTEX_MAGIC, VERSION as PTEX_VERSION,
};
use perro_io::decompress_zlib;
use std::{
    fs,
    path::{Path, PathBuf},
};
use winit::window::Icon;

pub(super) fn load_project_window_icon(project: &perro_runtime::RuntimeProject) -> Option<Icon> {
    let bytes = load_project_icon_bytes(project)?;
    let (rgba, width, height) = decode_image_rgba(&bytes)?;
    Icon::from_rgba(rgba, width, height).ok()
}

fn load_project_icon_bytes(project: &perro_runtime::RuntimeProject) -> Option<Vec<u8>> {
    load_project_image_bytes(
        project,
        project.config.icon.trim(),
        project.config.icon_hash,
    )
}

fn load_project_image_bytes(
    project: &perro_runtime::RuntimeProject,
    source: &str,
    source_hash: Option<u64>,
) -> Option<Vec<u8>> {
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

pub(super) fn load_image_size(
    project: &perro_runtime::RuntimeProject,
    source: &str,
    source_hash: Option<u64>,
) -> Option<(u32, u32)> {
    let bytes = load_project_image_bytes(project, source, source_hash)?;
    decode_image_size(&bytes)
}

fn decode_image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if let Some((width, height)) = decode_ptex_dimensions(bytes) {
        return Some((width.max(1), height.max(1)));
    }
    let decoded = ::image::load_from_memory(bytes).ok()?;
    Some((decoded.width().max(1), decoded.height().max(1)))
}

fn decode_image_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    if let Some(decoded) = decode_ptex_rgba(bytes) {
        return Some(decoded);
    }
    let img = ::image::load_from_memory(bytes).ok()?;
    let rgba = img.into_rgba8();
    let (width, height) = rgba.dimensions();
    Some((rgba.into_raw(), width, height))
}

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

fn decode_ptex_payload(flags: u32, payload: &[u8]) -> Option<Vec<u8>> {
    if (flags & PTEX_FLAG_PAYLOAD_RAW) != 0 {
        Some(payload.to_vec())
    } else {
        decompress_zlib(payload).ok()
    }
}

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
    use super::{decode_image_rgba, decode_image_size};

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
}
