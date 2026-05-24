use perro_asset_formats::ptex::{
    FLAG_FORMAT_MASK as PTEX_FLAG_FORMAT_MASK, FLAG_FORMAT_R8 as PTEX_FLAG_FORMAT_R8,
    FLAG_FORMAT_RGB8 as PTEX_FLAG_FORMAT_RGB8, FLAG_FORMAT_RGBA8 as PTEX_FLAG_FORMAT_RGBA8,
    FLAG_PAYLOAD_RAW as PTEX_FLAG_PAYLOAD_RAW, MAGIC as PTEX_MAGIC, VERSION as PTEX_VERSION,
};
use perro_io::{decompress_zlib, load_asset};

pub fn load_texture_rgba(source: &str) -> Option<(Vec<u8>, u32, u32)> {
    let (path, fragment) = split_source_fragment(source);
    if (path.ends_with(".glb") || path.ends_with(".gltf"))
        && let Some(texture_index) = parse_fragment_index(fragment, "tex")
            .or_else(|| parse_fragment_index(fragment, "texture"))
            .or_else(|| parse_fragment_index(fragment, "img"))
    {
        return decode_gltf_texture(path, texture_index as usize);
    }

    let bytes = load_asset(source).ok()?;
    if source.ends_with(".ptex") {
        return decode_ptex(&bytes);
    }
    decode_image_rgba(&bytes)
}

pub fn gltf_texture_source_from_mesh_source(mesh_source: &str, slot: u32) -> Option<String> {
    let (path, _) = split_source_fragment(mesh_source);
    if !(path.ends_with(".glb") || path.ends_with(".gltf")) {
        return None;
    }
    Some(format!("{path}:tex[{slot}]"))
}

pub fn decode_image_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    if looks_like_svg(bytes) {
        return decode_svg_rgba(bytes);
    }
    let image = image::load_from_memory(bytes).ok()?;
    let rgba = image.to_rgba8();
    let (w, h) = rgba.dimensions();
    Some((rgba.into_raw(), w.max(1), h.max(1)))
}

pub fn decode_image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if looks_like_svg(bytes) {
        return svg_target_size(bytes);
    }
    let image = image::load_from_memory(bytes).ok()?;
    Some((image.width().max(1), image.height().max(1)))
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let Ok(src) = std::str::from_utf8(bytes.get(..bytes.len().min(512)).unwrap_or(bytes)) else {
        return false;
    };
    let src = src.trim_start_matches('\u{feff}').trim_start();
    src.starts_with("<svg") || src.starts_with("<?xml") && src.contains("<svg")
}

fn decode_svg_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(bytes, &options).ok()?;
    let tree_size = tree.size();
    let tree_width = tree_size.width().max(1.0);
    let tree_height = tree_size.height().max(1.0);
    let (width, height) = svg_target_size(bytes).unwrap_or_else(|| {
        (
            tree_width.round().max(1.0) as u32,
            tree_height.round().max(1.0) as u32,
        )
    });
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    let transform = resvg::tiny_skia::Transform::from_scale(
        width as f32 / tree_width,
        height as f32 / tree_height,
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let mut rgba = Vec::with_capacity(
        (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?,
    );
    for pixel in pixmap.pixels() {
        rgba.extend_from_slice(&[pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]);
    }
    Some((rgba, width, height))
}

fn svg_target_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let src = std::str::from_utf8(bytes).ok()?;
    let tag = svg_start_tag(src)?;
    if let (Some(width), Some(height)) = (
        svg_attr_number(tag, "width"),
        svg_attr_number(tag, "height"),
    ) {
        return Some((width, height));
    }
    if let Some((width, height)) = svg_viewbox_size(tag) {
        return Some((width, height));
    }
    Some((256, 256))
}

fn svg_start_tag(src: &str) -> Option<&str> {
    let start = src.find("<svg")?;
    let rest = &src[start..];
    let end = rest.find('>')?;
    Some(&rest[..end])
}

fn svg_attr_number(tag: &str, name: &str) -> Option<u32> {
    let value = svg_attr_value(tag, name)?;
    parse_svg_number(value)
}

fn svg_attr_value<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag;
    loop {
        let idx = rest.find(name)?;
        let after_name = &rest[idx + name.len()..];
        let after_eq = after_name.trim_start();
        if !after_eq.starts_with('=') {
            rest = after_name.get(1..)?;
            continue;
        }
        let value = after_eq[1..].trim_start();
        let quote = value.chars().next()?;
        if quote == '"' || quote == '\'' {
            let value = &value[quote.len_utf8()..];
            let end = value.find(quote)?;
            return Some(&value[..end]);
        }
        let end = value
            .find(|ch: char| ch.is_ascii_whitespace() || ch == '>')
            .unwrap_or(value.len());
        return Some(&value[..end]);
    }
}

fn svg_viewbox_size(tag: &str) -> Option<(u32, u32)> {
    let value = svg_attr_value(tag, "viewBox").or_else(|| svg_attr_value(tag, "viewbox"))?;
    let nums: Vec<f32> = value
        .split(|ch: char| ch.is_ascii_whitespace() || ch == ',')
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<f32>().ok())
        .collect();
    if nums.len() < 4 {
        return None;
    }
    Some((size_component(nums[2])?, size_component(nums[3])?))
}

fn parse_svg_number(value: &str) -> Option<u32> {
    let trimmed = value.trim();
    let number_len = trimmed
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .unwrap_or(trimmed.len());
    let unit = trimmed.get(number_len..)?.trim();
    if unit.starts_with('%') {
        return None;
    }
    let parsed = trimmed.get(..number_len)?.parse::<f32>().ok()?;
    size_component(parsed)
}

fn size_component(value: f32) -> Option<u32> {
    (value.is_finite() && value > 0.0).then(|| value.round().max(1.0) as u32)
}

pub fn decode_gltf_texture(source_path: &str, texture_index: usize) -> Option<(Vec<u8>, u32, u32)> {
    let bytes = load_asset(source_path).ok()?;
    let (doc, _buffers, images) = gltf::import_slice(&bytes).ok()?;
    let texture = doc.textures().nth(texture_index)?;
    let image_index = texture.source().index();
    let image = images.get(image_index)?;
    let (width, height) = (image.width.max(1), image.height.max(1));
    let rgba = match image.format {
        gltf::image::Format::R8G8B8A8 => image.pixels.clone(),
        gltf::image::Format::R8G8B8 => {
            let mut out = Vec::with_capacity((width * height * 4) as usize);
            for px in image.pixels.chunks_exact(3) {
                out.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            out
        }
        gltf::image::Format::R8G8 => {
            let mut out = Vec::with_capacity((width * height * 4) as usize);
            for px in image.pixels.chunks_exact(2) {
                out.extend_from_slice(&[px[0], px[1], 0, 255]);
            }
            out
        }
        gltf::image::Format::R8 => {
            let mut out = Vec::with_capacity((width * height * 4) as usize);
            for &v in &image.pixels {
                out.extend_from_slice(&[v, v, v, 255]);
            }
            out
        }
        _ => return None,
    };
    Some((rgba, width, height))
}

pub fn decode_ptex(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
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
    let raw_len = u32::from_le_bytes(bytes[20..24].try_into().ok()?);
    if flags & !(PTEX_FLAG_FORMAT_MASK | PTEX_FLAG_PAYLOAD_RAW) != 0 {
        return None;
    }
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
    let raw = decode_texture_payload(flags, &bytes[24..])?;
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

fn decode_texture_payload(flags: u32, payload: &[u8]) -> Option<Vec<u8>> {
    if (flags & PTEX_FLAG_PAYLOAD_RAW) != 0 {
        Some(payload.to_vec())
    } else {
        decompress_zlib(payload).ok()
    }
}

fn split_source_fragment(source: &str) -> (&str, Option<&str>) {
    let Some((path, selector)) = source.rsplit_once(':') else {
        return (source, None);
    };
    if path.is_empty() || selector.contains('/') || selector.contains('\\') {
        return (source, None);
    }
    if selector.contains('[') && selector.ends_with(']') {
        return (path, Some(selector));
    }
    (source, None)
}

fn parse_fragment_index(fragment: Option<&str>, key: &str) -> Option<u32> {
    let fragment = fragment?;
    let (name, rest) = fragment.split_once('[')?;
    if name.trim() != key {
        return None;
    }
    let value = rest.strip_suffix(']')?.trim();
    value.parse::<u32>().ok()
}

#[cfg(test)]
mod tests {
    use super::{decode_image_rgba, decode_image_size};

    #[test]
    fn decode_image_rgba_supports_svg_with_intrinsic_size() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="3"><rect width="2" height="3" fill="red"/></svg>"#;
        let (rgba, width, height) = decode_image_rgba(svg).expect("decode svg");
        assert_eq!((width, height), (2, 3));
        assert_eq!(rgba.len(), 2 * 3 * 4);
        assert_eq!(decode_image_size(svg), Some((2, 3)));
    }

    #[test]
    fn decode_image_rgba_supports_svg_viewbox_and_fallback_size() {
        let viewbox = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 5"><rect width="4" height="5" fill="red"/></svg>"#;
        let (_, width, height) = decode_image_rgba(viewbox).expect("decode viewbox svg");
        assert_eq!((width, height), (4, 5));

        let percent = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%" viewBox="0 0 6 7"><rect width="6" height="7" fill="red"/></svg>"#;
        let (_, width, height) = decode_image_rgba(percent).expect("decode percent svg");
        assert_eq!((width, height), (6, 7));

        let fallback = br#"<svg xmlns="http://www.w3.org/2000/svg"><rect width="4" height="5" fill="red"/></svg>"#;
        let (_, width, height) = decode_image_rgba(fallback).expect("decode fallback svg");
        assert_eq!((width, height), (256, 256));
    }
}
