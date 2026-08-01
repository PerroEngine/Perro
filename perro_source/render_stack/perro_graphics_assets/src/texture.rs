use perro_asset_formats::ptex::{
    FLAG_FORMAT_MASK as PTEX_FLAG_FORMAT_MASK, FLAG_FORMAT_R8 as PTEX_FLAG_FORMAT_R8,
    FLAG_FORMAT_RGB8 as PTEX_FLAG_FORMAT_RGB8, FLAG_FORMAT_RGBA8 as PTEX_FLAG_FORMAT_RGBA8,
    FLAG_PAYLOAD_RAW as PTEX_FLAG_PAYLOAD_RAW, MAGIC as PTEX_MAGIC,
    MAX_COMPRESSED_BYTES as PTEX_MAX_COMPRESSED_BYTES, MAX_RAW_BYTES as PTEX_MAX_RAW_BYTES,
    VERSION as PTEX_VERSION,
};
use perro_io::{decompress_zlib_limited, load_asset};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    io::Cursor,
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};

// 2x logical size: retina-grade zoom headroom at 1/4 the pixel memory of the
// old 4x (rgba bytes scale with the square; a 512px logo is 4MB at 2x, 16MB
// at 4x, and every raster is held cpu-side + gpu-side).
pub const SVG_RASTER_SCALE: u32 = 2;
const SVG_MAX_RASTER_DIM: u32 = 8192;
const SVG_CACHE_LIMIT: usize = 32;
const SVG_RGBA_CACHE_MAX_BYTES: usize = 64 * 1024 * 1024;

pub fn encode_rgba_image(
    rgba: &[u8],
    width: u32,
    height: u32,
    path: &str,
) -> Result<Vec<u8>, String> {
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4));
    if width == 0 || height == 0 || expected != Some(rgba.len()) {
        return Err(format!(
            "invalid rgba image {width}x{height} len={}",
            rgba.len()
        ));
    }
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .ok_or_else(|| format!("image path has no extension: {path}"))?;
    let format = image::ImageFormat::from_extension(extension)
        .ok_or_else(|| format!("unsupported image extension: {extension}"))?;
    // encode straight from the caller's slice: no owned RgbaImage copy.
    let mut out = Cursor::new(Vec::new());
    image::write_buffer_with_format(
        &mut out,
        rgba,
        width,
        height,
        image::ExtendedColorType::Rgba8,
        format,
    )
    .map_err(|error| format!("image encode failed: {error}"))?;
    Ok(out.into_inner())
}

pub fn save_rgba_image(rgba: &[u8], width: u32, height: u32, path: &str) -> Result<(), String> {
    let encoded = encode_rgba_image(rgba, width, height, path)?;
    perro_io::save_asset(path, &encoded).map_err(|error| format!("image save failed: {error}"))
}

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

/// `load_texture_rgba` returning `Arc<[u8]>`. SVG cache hits hand out the
/// cached Arc by refcount (no full-buffer copy); consumers that store the
/// pixels behind an Arc (`DecodedTextureRgba`) adopt it without converting.
pub fn load_texture_rgba_arc(source: &str) -> Option<(Arc<[u8]>, u32, u32)> {
    let (path, fragment) = split_source_fragment(source);
    if (path.ends_with(".glb") || path.ends_with(".gltf"))
        && let Some(texture_index) = parse_fragment_index(fragment, "tex")
            .or_else(|| parse_fragment_index(fragment, "texture"))
            .or_else(|| parse_fragment_index(fragment, "img"))
    {
        return decode_gltf_texture(path, texture_index as usize)
            .map(|(rgba, width, height)| (rgba.into(), width, height));
    }

    let bytes = load_asset(source).ok()?;
    if source.ends_with(".ptex") {
        return decode_ptex(&bytes).map(|(rgba, width, height)| (rgba.into(), width, height));
    }
    decode_image_rgba_arc(&bytes)
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
        return decode_svg_rgba(bytes)
            .map(|(rgba, width, height)| (rgba.as_ref().to_vec(), width, height));
    }
    if bytes.starts_with(PTEX_MAGIC) {
        return decode_ptex(bytes);
    }
    let image = image::load_from_memory(bytes).ok()?;
    // into_rgba8 consumes the DynamicImage: no source + RGBA copy alive at once.
    let rgba = image.into_rgba8();
    let (w, h) = rgba.dimensions();
    Some((rgba.into_raw(), w.max(1), h.max(1)))
}

/// `decode_image_rgba` returning `Arc<[u8]>`. SVG cache hits share the cached
/// raster by refcount instead of copying it; raster/ptex decodes pay the same
/// single Vec -> Arc conversion their Arc-storing callers already paid.
pub fn decode_image_rgba_arc(bytes: &[u8]) -> Option<(Arc<[u8]>, u32, u32)> {
    if looks_like_svg(bytes) {
        return decode_svg_rgba(bytes);
    }
    decode_image_rgba(bytes).map(|(rgba, width, height)| (rgba.into(), width, height))
}

pub fn decode_image_rgba_max_size(bytes: &[u8], max_dim: u32) -> Option<(Vec<u8>, u32, u32)> {
    if looks_like_svg(bytes) {
        return decode_svg_rgba_max_size(bytes, max_dim)
            .map(|(rgba, width, height)| (rgba.as_ref().to_vec(), width, height));
    }
    if bytes.starts_with(PTEX_MAGIC) {
        let (rgba, width, height) = decode_ptex(bytes)?;
        return resize_rgba_to_max(rgba, width, height, max_dim);
    }
    let image = image::load_from_memory(bytes).ok()?;
    let (width, height) = (image.width().max(1), image.height().max(1));
    let target = fit_size((width, height), max_dim.max(1));
    let rgba = if target == (width, height) {
        image.into_rgba8()
    } else {
        let resized = image.resize_exact(target.0, target.1, image::imageops::FilterType::Lanczos3);
        // drop full-size source before the RGBA conversion of the resized copy.
        drop(image);
        resized.into_rgba8()
    };
    Some((rgba.into_raw(), target.0, target.1))
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

fn decode_svg_rgba(bytes: &[u8]) -> Option<(Arc<[u8]>, u32, u32)> {
    let cache_key = svg_cache_key(bytes);
    let (logical_size, raster_size) = svg_sizes(bytes)?;
    decode_svg_rgba_sized(bytes, cache_key, logical_size, raster_size)
}

fn decode_svg_rgba_max_size(bytes: &[u8], max_dim: u32) -> Option<(Arc<[u8]>, u32, u32)> {
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
) -> Option<(Arc<[u8]>, u32, u32)> {
    if let Some(rgba) = load_svg_rgba_cache_entry(cache_key, raster_size) {
        // refcount share of the cached raster: hits copy nothing.
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

    let _ = logical_size;
    // pixmap buffer already holds premultiplied RGBA in that byte order, so the
    // old per-pixel 4-byte push loop was an identity copy. take it whole.
    let rgba: Arc<[u8]> = pixmap.take().into();
    store_svg_rgba_cache_entry(cache_key, raster_size, Arc::clone(&rgba));
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

// ahash over the full source: runs per decode + per size probe, and SipHash
// (DefaultHasher) dominated both on multi-100KB svg sources.
fn svg_cache_key(bytes: &[u8]) -> u64 {
    let mut hasher = ahash::AHasher::default();
    bytes.len().hash(&mut hasher);
    hasher.write(bytes);
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

fn load_svg_rgba_cache_entry(key: u64, size: (u32, u32)) -> Option<Arc<[u8]>> {
    svg_rgba_cache().lock().ok()?.get(&(key, size))
}

fn store_svg_rgba_cache_entry(key: u64, size: (u32, u32), rgba: Arc<[u8]>) {
    let Ok(mut cache) = svg_rgba_cache().lock() else {
        return;
    };
    cache.insert((key, size), rgba);
}

fn svg_size_cache() -> &'static Mutex<HashMap<u64, SvgSizeCacheEntry>> {
    static SVG_SIZE_CACHE: OnceLock<Mutex<HashMap<u64, SvgSizeCacheEntry>>> = OnceLock::new();
    SVG_SIZE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

type SvgRgbaCacheKey = (u64, (u32, u32));

struct SvgRgbaCacheEntry {
    rgba: Arc<[u8]>,
    last_used: u64,
}

struct SvgRgbaCache {
    entries: HashMap<SvgRgbaCacheKey, SvgRgbaCacheEntry>,
    // single slot 4 rasters over the whole lru budget. w/o it such a raster is
    // never cached, so every decode re-parses + re-rasters the svg.
    oversized: Option<(SvgRgbaCacheKey, Arc<[u8]>)>,
    bytes: usize,
    clock: u64,
    max_bytes: usize,
}

impl SvgRgbaCache {
    fn new(max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            oversized: None,
            bytes: 0,
            clock: 0,
            max_bytes,
        }
    }

    fn get(&mut self, key: &SvgRgbaCacheKey) -> Option<Arc<[u8]>> {
        self.clock = self.clock.wrapping_add(1);
        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_used = self.clock;
            return Some(Arc::clone(&entry.rgba));
        }
        match &self.oversized {
            Some((slot_key, rgba)) if slot_key == key => Some(Arc::clone(rgba)),
            _ => None,
        }
    }

    fn insert(&mut self, key: SvgRgbaCacheKey, rgba: Arc<[u8]>) {
        let item_bytes = rgba.len();
        if item_bytes > self.max_bytes {
            // 1 entry only: a different oversized asset evicts the last one.
            self.oversized = Some((key, rgba));
            return;
        }
        if let Some(old) = self.entries.remove(&key) {
            self.bytes = self.bytes.saturating_sub(old.rgba.len());
        }
        while self.entries.len() >= SVG_CACHE_LIMIT
            || self.bytes.saturating_add(item_bytes) > self.max_bytes
        {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| *key)
            else {
                break;
            };
            if let Some(old) = self.entries.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(old.rgba.len());
            }
        }
        self.clock = self.clock.wrapping_add(1);
        self.bytes = self.bytes.saturating_add(item_bytes);
        self.entries.insert(
            key,
            SvgRgbaCacheEntry {
                rgba,
                last_used: self.clock,
            },
        );
    }
}

fn svg_rgba_cache() -> &'static Mutex<SvgRgbaCache> {
    static SVG_RGBA_CACHE: OnceLock<Mutex<SvgRgbaCache>> = OnceLock::new();
    SVG_RGBA_CACHE.get_or_init(|| Mutex::new(SvgRgbaCache::new(SVG_RGBA_CACHE_MAX_BYTES)))
}

/// Drop every cached SVG raster + size entry. The rasters are process-global
/// (keyed by content hash) and otherwise only shrink via LRU pressure; callers
/// with a scene-teardown hook can reclaim them eagerly here.
pub fn clear_svg_caches() {
    if let Ok(mut cache) = svg_size_cache().lock() {
        cache.clear();
    }
    if let Ok(mut cache) = svg_rgba_cache().lock() {
        cache.entries.clear();
        cache.oversized = None;
        cache.bytes = 0;
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

fn resize_rgba_to_max(
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    max_dim: u32,
) -> Option<(Vec<u8>, u32, u32)> {
    let target = fit_size((width, height), max_dim.max(1));
    if target == (width, height) {
        return Some((rgba, width, height));
    }
    let source = image::RgbaImage::from_raw(width, height, rgba)?;
    let resized = image::imageops::resize(
        &source,
        target.0,
        target.1,
        image::imageops::FilterType::Lanczos3,
    );
    Some((resized.into_raw(), target.0, target.1))
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
        if idx > 0
            && rest[..idx]
                .chars()
                .next_back()
                .is_some_and(is_svg_attr_name_char)
        {
            rest = &rest[idx + name.len()..];
            continue;
        }
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

fn is_svg_attr_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ':'
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
    // this runs once per material texture slot on the same .glb, so decode
    // ONLY the target image; import_slice would re-decode every embedded
    // image on each call.
    let gltf::Gltf {
        document: doc,
        blob,
    } = gltf::Gltf::from_slice(&bytes).ok()?;
    let texture = doc.textures().nth(texture_index)?;
    match texture.source().source() {
        gltf::image::Source::View { view, mime_type } => {
            let buffers = gltf::import_buffers(&doc, None, blob).ok()?;
            let data = buffers.get(view.buffer().index())?;
            let start = view.offset();
            let end = start.checked_add(view.length())?;
            decode_gltf_image_rgba(data.0.get(start..end)?, Some(mime_type))
        }
        gltf::image::Source::Uri { .. } => {
            // rare data-URI image path: keep the old full import (external
            // file URIs already failed here with base = None).
            let (doc, _buffers, mut images) = gltf::import_slice(&bytes).ok()?;
            let texture = doc.textures().nth(texture_index)?;
            let image = images.get_mut(texture.source().index())?;
            let (width, height) = (image.width.max(1), image.height.max(1));
            // images is owned and dropped right after: move the RGBA8 pixel
            // buffer out instead of cloning it.
            let rgba = match image.format {
                gltf::image::Format::R8G8B8A8 => std::mem::take(&mut image.pixels),
                gltf::image::Format::R8G8B8 => expand_rgb8(&image.pixels, width, height),
                gltf::image::Format::R8G8 => expand_rg8(&image.pixels, width, height),
                gltf::image::Format::R8 => expand_r8(&image.pixels, width, height),
                _ => return None,
            };
            Some((rgba, width, height))
        }
    }
}

// mirrors the gltf importer's DynamicImage -> Format mapping + the old
// Format -> RGBA8 expansion above, so output bytes stay identical.
fn decode_gltf_image_rgba(bytes: &[u8], mime_type: Option<&str>) -> Option<(Vec<u8>, u32, u32)> {
    let format = match mime_type {
        Some("image/png") => Some(image::ImageFormat::Png),
        Some("image/jpeg") => Some(image::ImageFormat::Jpeg),
        Some("image/webp") => Some(image::ImageFormat::WebP),
        _ => None,
    };
    let decoded = match format {
        Some(format) => image::load_from_memory_with_format(bytes, format).ok()?,
        None => image::load_from_memory(bytes).ok()?,
    };
    let (width, height) = (decoded.width().max(1), decoded.height().max(1));
    let rgba = match decoded {
        image::DynamicImage::ImageRgba8(img) => img.into_raw(),
        image::DynamicImage::ImageRgb8(img) => expand_rgb8(&img.into_raw(), width, height),
        image::DynamicImage::ImageLumaA8(img) => expand_rg8(&img.into_raw(), width, height),
        image::DynamicImage::ImageLuma8(img) => expand_r8(&img.into_raw(), width, height),
        _ => return None,
    };
    Some((rgba, width, height))
}

fn expand_rgb8(pixels: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for px in pixels.chunks_exact(3) {
        out.extend_from_slice(&[px[0], px[1], px[2], 255]);
    }
    out
}

fn expand_rg8(pixels: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for px in pixels.chunks_exact(2) {
        out.extend_from_slice(&[px[0], px[1], 0, 255]);
    }
    out
}

fn expand_r8(pixels: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for &v in pixels {
        out.extend_from_slice(&[v, v, v, 255]);
    }
    out
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
    if expected_raw_len > PTEX_MAX_RAW_BYTES {
        return None;
    }
    let raw = decode_texture_payload(flags, &bytes[24..], expected_raw_len)?;
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

fn decode_texture_payload(flags: u32, payload: &[u8], expected_raw_len: usize) -> Option<Vec<u8>> {
    if (flags & PTEX_FLAG_PAYLOAD_RAW) != 0 {
        (payload.len() == expected_raw_len).then(|| payload.to_vec())
    } else {
        if payload.len() > PTEX_MAX_COMPRESSED_BYTES {
            return None;
        }
        decompress_zlib_limited(payload, expected_raw_len).ok()
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
        SvgRgbaCache, clear_svg_caches, decode_image_logical_size, decode_image_rgba,
        decode_image_rgba_max_size, decode_image_size, encode_rgba_image,
    };
    use std::{io::Cursor, sync::Arc, time::Instant};

    #[test]
    fn encode_rgba_image_writes_png_bytes() {
        let bytes =
            encode_rgba_image(&[255, 0, 0, 255], 1, 1, "user://shot.png").expect("encode png");
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(encode_rgba_image(&[0; 3], 1, 1, "user://bad.png").is_err());
        assert!(encode_rgba_image(&[0; 4], 1, 1, "user://bad.nope").is_err());
    }

    #[test]
    fn decode_image_rgba_supports_svg_with_intrinsic_size() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="3"><rect width="2" height="3" fill="red"/></svg>"#;
        let (rgba, width, height) = decode_image_rgba(svg).expect("decode svg");
        assert_eq!((width, height), (4, 6));
        assert_eq!(rgba.len(), 4 * 6 * 4);
        assert_eq!(decode_image_size(svg), Some((4, 6)));
        assert_eq!(decode_image_logical_size(svg), Some((2, 3)));
    }

    #[test]
    fn decode_image_rgba_ignores_svg_attr_name_substrings() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" stroke-width="99" data-height="88" width="2" height="3"><rect width="2" height="3" fill="red"/></svg>"#;
        let (_, width, height) = decode_image_rgba(svg).expect("decode svg");
        assert_eq!((width, height), (4, 6));
        assert_eq!(decode_image_logical_size(svg), Some((2, 3)));
    }

    #[test]
    fn decode_image_rgba_supports_svg_viewbox_and_fallback_size() {
        let viewbox = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 5"><rect width="4" height="5" fill="red"/></svg>"#;
        let (_, width, height) = decode_image_rgba(viewbox).expect("decode viewbox svg");
        assert_eq!((width, height), (8, 10));

        let percent = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%" viewBox="0 0 6 7"><rect width="6" height="7" fill="red"/></svg>"#;
        let (_, width, height) = decode_image_rgba(percent).expect("decode percent svg");
        assert_eq!((width, height), (12, 14));

        let fallback = br#"<svg xmlns="http://www.w3.org/2000/svg"><rect width="4" height="5" fill="red"/></svg>"#;
        let (_, width, height) = decode_image_rgba(fallback).expect("decode fallback svg");
        assert_eq!((width, height), (512, 512));
    }

    #[test]
    fn decode_image_rgba_caps_large_svg_raster_size() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="5000" height="100"><rect width="5000" height="100" fill="red"/></svg>"#;
        let (_, width, height) = decode_image_rgba(svg).expect("decode large svg");
        assert_eq!((width, height), (8192, 164));
        assert_eq!(decode_image_size(svg), Some((8192, 164)));
        assert_eq!(decode_image_logical_size(svg), Some((5000, 100)));
    }

    #[test]
    fn decode_image_rgba_max_size_caps_svg_raster_size() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="1000"><rect width="1200" height="1000" fill="red"/></svg>"#;
        let (_, width, height) = decode_image_rgba_max_size(svg, 256).expect("decode icon svg");
        assert_eq!((width, height), (256, 213));
    }

    #[test]
    fn decode_image_rgba_max_size_downscales_raster_and_ptex() {
        let raster = image::RgbaImage::from_pixel(8, 4, image::Rgba([255, 0, 0, 255]));
        let mut png = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(raster)
            .write_to(&mut png, image::ImageFormat::Png)
            .expect("encode png");
        let (_, width, height) =
            decode_image_rgba_max_size(png.get_ref(), 2).expect("decode resized png");
        assert_eq!((width, height), (2, 1));

        let mut ptex = Vec::new();
        ptex.extend_from_slice(b"PTEX");
        ptex.extend_from_slice(&1u32.to_le_bytes());
        ptex.extend_from_slice(&8u32.to_le_bytes());
        ptex.extend_from_slice(&4u32.to_le_bytes());
        ptex.extend_from_slice(&(1u32 << 31).to_le_bytes());
        ptex.extend_from_slice(&(8u32 * 4 * 4).to_le_bytes());
        ptex.extend(std::iter::repeat_n(255, 8 * 4 * 4));
        let (_, width, height) = decode_image_rgba_max_size(&ptex, 2).expect("decode resized ptex");
        assert_eq!((width, height), (2, 1));
    }

    #[test]
    fn ptex_rejects_inflate_beyond_declared_exact_size() {
        let compressed = perro_io::compress_zlib_best(&vec![0u8; 4096]).expect("compress");
        let mut ptex = Vec::new();
        ptex.extend_from_slice(b"PTEX");
        ptex.extend_from_slice(&1u32.to_le_bytes());
        ptex.extend_from_slice(&1u32.to_le_bytes());
        ptex.extend_from_slice(&1u32.to_le_bytes());
        ptex.extend_from_slice(&0u32.to_le_bytes());
        ptex.extend_from_slice(&4u32.to_le_bytes());
        ptex.extend_from_slice(&compressed);

        assert!(super::decode_ptex(&ptex).is_none());
    }

    #[test]
    fn svg_rgba_cache_enforces_byte_lru_and_uses_arc_hits() {
        let mut cache = SvgRgbaCache::new(10);
        let first: Arc<[u8]> = vec![1; 6].into();
        let second: Arc<[u8]> = vec![2; 6].into();
        cache.insert((1, (1, 1)), Arc::clone(&first));
        assert!(Arc::ptr_eq(
            &cache.get(&(1, (1, 1))).expect("first hit"),
            &first
        ));
        cache.insert((2, (1, 1)), second);

        assert!(cache.bytes <= cache.max_bytes);
        assert!(cache.get(&(1, (1, 1))).is_none());
        assert!(cache.get(&(2, (1, 1))).is_some());
        cache.insert((3, (1, 1)), vec![3; 11].into());
        assert!(!cache.entries.contains_key(&(3, (1, 1))));
    }

    #[test]
    fn svg_rgba_cache_keeps_one_oversized_entry() {
        let mut cache = SvgRgbaCache::new(10);
        let big: Arc<[u8]> = vec![1; 32].into();
        cache.insert((1, (4, 2)), Arc::clone(&big));
        assert!(cache.entries.is_empty(), "oversized stays out of the lru");
        assert!(Arc::ptr_eq(
            &cache.get(&(1, (4, 2))).expect("oversized hit"),
            &big
        ));
        // budget untouched by the oversized slot.
        assert_eq!(cache.bytes, 0);
        // a different oversized raster evicts the last one.
        cache.insert((2, (4, 2)), vec![2; 32].into());
        assert!(cache.get(&(1, (4, 2))).is_none());
        assert!(cache.get(&(2, (4, 2))).is_some());
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
