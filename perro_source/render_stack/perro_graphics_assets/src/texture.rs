use perro_asset_formats::ptex::{
    FLAG_FORMAT_MASK as PTEX_FLAG_FORMAT_MASK, FLAG_FORMAT_R8 as PTEX_FLAG_FORMAT_R8,
    FLAG_FORMAT_RGB8 as PTEX_FLAG_FORMAT_RGB8, FLAG_FORMAT_RGBA8 as PTEX_FLAG_FORMAT_RGBA8,
    FLAG_PAYLOAD_RAW as PTEX_FLAG_PAYLOAD_RAW, MAGIC as PTEX_MAGIC, VERSION as PTEX_VERSION,
};
use perro_io::{decompress_zlib, load_asset};
use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::{Mutex, OnceLock},
};

const SVG_RASTER_SCALE: u32 = 4;
const SVG_MAX_RASTER_DIM: u32 = 8192;
const SVG_CACHE_LIMIT: usize = 32;

#[derive(Clone)]
struct SvgSizeCacheEntry {
    logical_size: (u32, u32),
    raster_size: (u32, u32),
}

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

pub fn decode_image_rgba_max_size(bytes: &[u8], max_dim: u32) -> Option<(Vec<u8>, u32, u32)> {
    if looks_like_svg(bytes) {
        return decode_svg_rgba_max_size(bytes, max_dim);
    }
    decode_image_rgba(bytes)
}

pub fn decode_image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if looks_like_svg(bytes) {
        return svg_target_size(bytes);
    }
    let image = image::load_from_memory(bytes).ok()?;
    Some((image.width().max(1), image.height().max(1)))
}

pub fn decode_image_logical_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if looks_like_svg(bytes) {
        return svg_logical_size(bytes);
    }
    decode_image_size(bytes)
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let Ok(src) = std::str::from_utf8(bytes.get(..bytes.len().min(512)).unwrap_or(bytes)) else {
        return false;
    };
    let src = src.trim_start_matches('\u{feff}').trim_start();
    src.starts_with("<svg") || src.starts_with("<?xml") && src.contains("<svg")
}

fn decode_svg_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let cache_key = svg_cache_key(bytes);
    let (logical_size, raster_size) = svg_sizes(bytes)?;
    decode_svg_rgba_sized(bytes, cache_key, logical_size, raster_size)
}

fn decode_svg_rgba_max_size(bytes: &[u8], max_dim: u32) -> Option<(Vec<u8>, u32, u32)> {
    let cache_key = svg_cache_key(bytes);
    let (logical_size, _) = svg_sizes(bytes)?;
    let raster_size = fit_size(logical_size, max_dim.max(1));
    decode_svg_rgba_sized(bytes, cache_key, logical_size, raster_size)
}

fn decode_svg_rgba_sized(
    bytes: &[u8],
    cache_key: u64,
    logical_size: (u32, u32),
    raster_size: (u32, u32),
) -> Option<(Vec<u8>, u32, u32)> {
    if let Some(rgba) = load_svg_rgba_cache_entry(cache_key, raster_size) {
        return Some((rgba, raster_size.0, raster_size.1));
    }
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(bytes, &options).ok()?;
    let tree_size = tree.size();
    let tree_width = tree_size.width().max(1.0);
    let tree_height = tree_size.height().max(1.0);
    let (width, height) = raster_size;
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
    let _ = logical_size;
    store_svg_rgba_cache_entry(cache_key, raster_size, rgba.clone());
    Some((rgba, width, height))
}

fn svg_target_size(bytes: &[u8]) -> Option<(u32, u32)> {
    svg_sizes(bytes).map(|(_, raster)| raster)
}

fn svg_logical_size(bytes: &[u8]) -> Option<(u32, u32)> {
    svg_sizes(bytes).map(|(logical, _)| logical)
}

fn svg_sizes(bytes: &[u8]) -> Option<((u32, u32), (u32, u32))> {
    let cache_key = svg_cache_key(bytes);
    if let Some(entry) = load_svg_size_cache_entry(cache_key) {
        return Some((entry.logical_size, entry.raster_size));
    }

    let src = std::str::from_utf8(bytes).ok()?;
    let tag = svg_start_tag(src)?;
    let logical_size = if let (Some(width), Some(height)) = (
        svg_attr_number(tag, "width"),
        svg_attr_number(tag, "height"),
    ) {
        (width, height)
    } else if let Some((width, height)) = svg_viewbox_size(tag) {
        (width, height)
    } else {
        (256, 256)
    };
    let raster_size = scaled_svg_raster_size(logical_size.0, logical_size.1);
    store_svg_size_cache_entry(
        cache_key,
        SvgSizeCacheEntry {
            logical_size,
            raster_size,
        },
    );
    Some((logical_size, raster_size))
}

fn svg_cache_key(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.len().hash(&mut hasher);
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn load_svg_size_cache_entry(key: u64) -> Option<SvgSizeCacheEntry> {
    svg_size_cache().lock().ok()?.get(&key).cloned()
}

fn store_svg_size_cache_entry(key: u64, entry: SvgSizeCacheEntry) {
    let Ok(mut cache) = svg_size_cache().lock() else {
        return;
    };
    if !cache.contains_key(&key) && cache.len() >= SVG_CACHE_LIMIT {
        cache.clear();
    }
    cache.insert(key, entry);
}

fn load_svg_rgba_cache_entry(key: u64, size: (u32, u32)) -> Option<Vec<u8>> {
    svg_rgba_cache().lock().ok()?.get(&(key, size)).cloned()
}

fn store_svg_rgba_cache_entry(key: u64, size: (u32, u32), rgba: Vec<u8>) {
    let Ok(mut cache) = svg_rgba_cache().lock() else {
        return;
    };
    let cache_key = (key, size);
    if !cache.contains_key(&cache_key) && cache.len() >= SVG_CACHE_LIMIT {
        cache.clear();
    }
    cache.insert(cache_key, rgba);
}

fn svg_size_cache() -> &'static Mutex<HashMap<u64, SvgSizeCacheEntry>> {
    static SVG_SIZE_CACHE: OnceLock<Mutex<HashMap<u64, SvgSizeCacheEntry>>> = OnceLock::new();
    SVG_SIZE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

type SvgRgbaCacheKey = (u64, (u32, u32));
type SvgRgbaCache = HashMap<SvgRgbaCacheKey, Vec<u8>>;

fn svg_rgba_cache() -> &'static Mutex<SvgRgbaCache> {
    static SVG_RGBA_CACHE: OnceLock<Mutex<SvgRgbaCache>> = OnceLock::new();
    SVG_RGBA_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
fn clear_svg_caches() {
    if let Ok(mut cache) = svg_size_cache().lock() {
        cache.clear();
    }
    if let Ok(mut cache) = svg_rgba_cache().lock() {
        cache.clear();
    }
}

fn scaled_svg_raster_size(width: u32, height: u32) -> (u32, u32) {
    let scaled_width = (width as u64)
        .saturating_mul(SVG_RASTER_SCALE as u64)
        .max(1);
    let scaled_height = (height as u64)
        .saturating_mul(SVG_RASTER_SCALE as u64)
        .max(1);
    let max_dim = scaled_width.max(scaled_height);
    if max_dim <= SVG_MAX_RASTER_DIM as u64 {
        return (scaled_width as u32, scaled_height as u32);
    }
    let ratio = SVG_MAX_RASTER_DIM as f64 / max_dim as f64;
    (
        ((scaled_width as f64 * ratio).round() as u32).max(1),
        ((scaled_height as f64 * ratio).round() as u32).max(1),
    )
}

fn fit_size(size: (u32, u32), max_dim: u32) -> (u32, u32) {
    let (width, height) = size;
    let largest = width.max(height).max(1);
    if largest <= max_dim {
        return (width.max(1), height.max(1));
    }
    let scale = max_dim as f64 / largest as f64;
    (
        ((width as f64 * scale).round() as u32).max(1),
        ((height as f64 * scale).round() as u32).max(1),
    )
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
    use super::{
        clear_svg_caches, decode_image_logical_size, decode_image_rgba, decode_image_rgba_max_size,
        decode_image_size,
    };
    use std::time::Instant;

    #[test]
    fn decode_image_rgba_supports_svg_with_intrinsic_size() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="3"><rect width="2" height="3" fill="red"/></svg>"#;
        let (rgba, width, height) = decode_image_rgba(svg).expect("decode svg");
        assert_eq!((width, height), (8, 12));
        assert_eq!(rgba.len(), 8 * 12 * 4);
        assert_eq!(decode_image_size(svg), Some((8, 12)));
        assert_eq!(decode_image_logical_size(svg), Some((2, 3)));
    }

    #[test]
    fn decode_image_rgba_supports_svg_viewbox_and_fallback_size() {
        let viewbox = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 5"><rect width="4" height="5" fill="red"/></svg>"#;
        let (_, width, height) = decode_image_rgba(viewbox).expect("decode viewbox svg");
        assert_eq!((width, height), (16, 20));

        let percent = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%" viewBox="0 0 6 7"><rect width="6" height="7" fill="red"/></svg>"#;
        let (_, width, height) = decode_image_rgba(percent).expect("decode percent svg");
        assert_eq!((width, height), (24, 28));

        let fallback = br#"<svg xmlns="http://www.w3.org/2000/svg"><rect width="4" height="5" fill="red"/></svg>"#;
        let (_, width, height) = decode_image_rgba(fallback).expect("decode fallback svg");
        assert_eq!((width, height), (1024, 1024));
    }

    #[test]
    fn decode_image_rgba_caps_large_svg_raster_size() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="2593" height="100"><rect width="2593" height="100" fill="red"/></svg>"#;
        let (_, width, height) = decode_image_rgba(svg).expect("decode large svg");
        assert_eq!((width, height), (8192, 316));
        assert_eq!(decode_image_size(svg), Some((8192, 316)));
        assert_eq!(decode_image_logical_size(svg), Some((2593, 100)));
    }

    #[test]
    fn decode_image_rgba_max_size_caps_svg_raster_size() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="1000"><rect width="1200" height="1000" fill="red"/></svg>"#;
        let (_, width, height) = decode_image_rgba_max_size(svg, 256).expect("decode icon svg");
        assert_eq!((width, height), (256, 213));
    }

    #[test]
    #[ignore = "bench probe; run with --release --ignored --nocapture"]
    fn bench_svg_decode_release_probe() {
        let svg = include_bytes!("../../../api_modules/perro_api/src/assets/perro.svg");

        clear_svg_caches();
        let start = Instant::now();
        let full = decode_image_rgba(svg).expect("full svg decode");
        eprintln!(
            "bench_svg_decode full_cold size={}x{} ms={:.3}",
            full.1,
            full.2,
            start.elapsed().as_secs_f64() * 1000.0
        );

        let start = Instant::now();
        let full_warm = decode_image_rgba(svg).expect("full svg decode warm");
        eprintln!(
            "bench_svg_decode full_warm size={}x{} ms={:.3}",
            full_warm.1,
            full_warm.2,
            start.elapsed().as_secs_f64() * 1000.0
        );

        clear_svg_caches();
        let start = Instant::now();
        let icon = decode_image_rgba_max_size(svg, 256).expect("icon svg decode");
        eprintln!(
            "bench_svg_decode icon_cold size={}x{} ms={:.3}",
            icon.1,
            icon.2,
            start.elapsed().as_secs_f64() * 1000.0
        );

        let start = Instant::now();
        let icon_warm = decode_image_rgba_max_size(svg, 256).expect("icon svg decode warm");
        eprintln!(
            "bench_svg_decode icon_warm size={}x{} ms={:.3}",
            icon_warm.1,
            icon_warm.2,
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
}
