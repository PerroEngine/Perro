use core::sync::atomic::{AtomicU64, Ordering};
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

#[cfg(test)]
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
        let mut next = Vec::new();
        let (next_width, next_height) =
            downsample_rgba_into(&prev.rgba, prev.width, prev.height, &mut next);
        levels.push(RgbaMipLevel {
            rgba: next,
            width: next_width,
            height: next_height,
        });
    }

    levels
}

/// 2x2 box-downsample one RGBA level into `dst` (cleared + resized in place so
/// scratch buffers reuse their allocation). Color averages in linear space,
/// alpha averages directly. Returns the downsampled dimensions.
fn downsample_rgba_into(src: &[u8], width: u32, height: u32, dst: &mut Vec<u8>) -> (u32, u32) {
    let next_width = (width / 2).max(1);
    let next_height = (height / 2).max(1);
    dst.clear();
    dst.resize(next_width as usize * next_height as usize * 4, 0);

    for y in 0..next_height {
        for x in 0..next_width {
            let sx = x * 2;
            let sy = y * 2;
            let x1 = (sx + 1).min(width - 1);
            let y1 = (sy + 1).min(height - 1);
            let samples = [(sx, sy), (x1, sy), (sx, y1), (x1, y1)];
            let dst_at = ((y * next_width + x) * 4) as usize;

            let alpha_sum = samples.iter().fold(0u32, |acc, &(px, py)| {
                let src_at = ((py * width + px) * 4) as usize + 3;
                acc + src[src_at] as u32
            });
            let alpha = ((alpha_sum + 2) / 4) as u8;
            for c in 0..3 {
                let sum = samples.iter().fold(0.0f32, |acc, &(px, py)| {
                    let src_at = ((py * width + px) * 4) as usize + c;
                    acc + srgb_u8_to_linear(src[src_at])
                });
                dst[dst_at + c] = linear_to_srgb_u8(sum * 0.25);
            }
            dst[dst_at + 3] = alpha;
        }
    }

    (next_width, next_height)
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

fn write_rgba_mip_level(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    mip_level: u32,
    rgba: &[u8],
    width: u32,
    height: u32,
) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level,
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

std::thread_local! {
    // Ping-pong downsample scratch for the streaming mip upload below; reused
    // across uploads so mip generation allocates nothing after warmup. Peak
    // retained size is 1/4 + 1/16 of the largest base level seen.
    static MIP_STREAM_SCRATCH: std::cell::RefCell<(Vec<u8>, Vec<u8>)> =
        const { std::cell::RefCell::new((Vec::new(), Vec::new())) };
}

/// Streaming texture upload: writes the base level straight from the caller's
/// borrowed slice, then generates each mip level into a ping-pong scratch pair
/// and uploads it before the next level overwrites it. The full mip chain
/// (~1.33x the base) is never materialized: peak extra CPU memory is the two
/// scratch levels (base/4 + base/16). Mip count comes from the texture itself.
pub(crate) fn write_rgba_texture_streaming(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    rgba: &[u8],
    width: u32,
    height: u32,
) {
    write_texture_base_level(queue, texture, width, height, rgba);
    let mip_count = texture.mip_level_count();
    if mip_count <= 1 {
        return;
    }
    MIP_STREAM_SCRATCH.with(|scratch| {
        let mut scratch = scratch.borrow_mut();
        let (ping, pong) = &mut *scratch;
        let (mut src_width, mut src_height) = (width, height);
        for level in 1..mip_count {
            let (next_width, next_height) = if level == 1 {
                let dims = downsample_rgba_into(rgba, src_width, src_height, ping);
                write_rgba_mip_level(queue, texture, level, ping, dims.0, dims.1);
                dims
            } else if level % 2 == 0 {
                let dims = downsample_rgba_into(ping, src_width, src_height, pong);
                write_rgba_mip_level(queue, texture, level, pong, dims.0, dims.1);
                dims
            } else {
                let dims = downsample_rgba_into(pong, src_width, src_height, ping);
                write_rgba_mip_level(queue, texture, level, ping, dims.0, dims.1);
                dims
            };
            src_width = next_width;
            src_height = next_height;
        }
    });
}

std::thread_local! {
    /// Active stream fan-out dedupe scope, see [`StreamWriteDedupe`].
    static STREAM_WRITE_DEDUPE: std::cell::RefCell<Option<StreamDedupeState>> =
        const { std::cell::RefCell::new(None) };
}

#[derive(Default)]
struct StreamDedupeState {
    // Handles already written at base level during this scope. Tiny (one entry
    // per distinct shared texture reachable from the consumer caches), so a
    // linear scan beats hashing.
    seen: Vec<wgpu::Texture>,
    written: u32,
    skipped: u32,
}

/// RAII dedupe scope for the camera-stream / webcam texture fan-out.
///
/// A single decoded RGBA frame is pushed at every consumer cache (2D, late
/// overlay, per-stream 2D, UI, 3D material by id and by source, per-stream 3D
/// ×2). Those caches routinely resolve to the *same* `SharedGpuTexture`, so the
/// naive fan-out uploads the identical bytes up to eight times per frame. While
/// this scope is alive, `write_texture_base_level` performs at most one upload
/// per distinct `wgpu::Texture`; the consumers still run their own bookkeeping
/// (residency checks, UI supersample invalidation) and still report success, so
/// no caller's residency assumption changes.
pub(crate) struct StreamWriteDedupe {
    _not_send: core::marker::PhantomData<*const ()>,
}

impl StreamWriteDedupe {
    pub(crate) fn begin() -> Self {
        STREAM_WRITE_DEDUPE.with(|slot| {
            *slot.borrow_mut() = Some(StreamDedupeState::default());
        });
        Self {
            _not_send: core::marker::PhantomData,
        }
    }
}

impl Drop for StreamWriteDedupe {
    fn drop(&mut self) {
        STREAM_WRITE_DEDUPE.with(|slot| {
            let Some(state) = slot.borrow_mut().take() else {
                return;
            };
            STREAM_WRITES.fetch_add(state.written as u64, Ordering::Relaxed);
            STREAM_WRITES_ELIDED.fetch_add(state.skipped as u64, Ordering::Relaxed);
        });
    }
}

static STREAM_WRITES: AtomicU64 = AtomicU64::new(0);
static STREAM_WRITES_ELIDED: AtomicU64 = AtomicU64::new(0);

/// `(distinct base-level uploads issued, redundant uploads elided)` totalled
/// over every completed [`StreamWriteDedupe`] scope.
pub(crate) fn stream_write_totals() -> (u64, u64) {
    (
        STREAM_WRITES.load(Ordering::Relaxed),
        STREAM_WRITES_ELIDED.load(Ordering::Relaxed),
    )
}

/// Returns false when an active dedupe scope already uploaded this texture.
fn stream_dedupe_admit(texture: &wgpu::Texture) -> bool {
    STREAM_WRITE_DEDUPE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(state) = slot.as_mut() else {
            return true;
        };
        if state.seen.iter().any(|seen| seen == texture) {
            state.skipped = state.skipped.saturating_add(1);
            return false;
        }
        state.seen.push(texture.clone());
        state.written = state.written.saturating_add(1);
        true
    })
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
    if !stream_dedupe_admit(texture) {
        return;
    }
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

    async fn dedupe_test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .ok()?;
        adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("perro_stream_dedupe_test_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::default(),
            })
            .await
            .ok()
    }

    fn dedupe_test_texture(device: &wgpu::Device, label: &str) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    #[test]
    fn stream_write_scope_uploads_once_per_distinct_texture() {
        let Some((device, queue)) = pollster::block_on(dedupe_test_device()) else {
            // No adapter available in this environment.
            return;
        };
        let shared = dedupe_test_texture(&device, "perro_stream_dedupe_shared");
        let other = dedupe_test_texture(&device, "perro_stream_dedupe_other");
        let rgba = [255u8; 4 * 4 * 4];
        let (base_writes, base_elided) = stream_write_totals();

        {
            let _scope = StreamWriteDedupe::begin();
            // The camera-stream fan-out reaches the same SharedGpuTexture from
            // up to eight consumer caches; only two distinct textures here.
            for _ in 0..7 {
                write_texture_base_level(&queue, &shared, 4, 4, &rgba);
            }
            write_texture_base_level(&queue, &other, 4, 4, &rgba);
        }
        let (writes, elided) = stream_write_totals();
        assert_eq!(writes - base_writes, 2);
        assert_eq!(elided - base_elided, 6);

        // Outside a scope every call still uploads (mip/decal paths rely on it).
        write_texture_base_level(&queue, &shared, 4, 4, &rgba);
        write_texture_base_level(&queue, &shared, 4, 4, &rgba);
        let (after, after_elided) = stream_write_totals();
        assert_eq!(after, writes);
        assert_eq!(after_elided, elided);
        queue.submit(std::iter::empty());
    }
}
