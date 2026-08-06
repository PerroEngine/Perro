use core::sync::atomic::{AtomicU64, Ordering};
use perro_structs::TextureFilterMode;
pub(crate) use perro_graphics_assets::mip::{RgbaMipLevel, downsample_rgba_into, rgba_mip_level_count};
#[cfg(test)]
pub(crate) use perro_graphics_assets::mip::build_rgba_levels_for_filter;
pub(crate) use perro_graphics_assets::mip::build_rgba_levels_for_filter_owned;

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
/// Mip levels baked into the source `.ptex`, when it has them.
///
/// Skips the CPU downsample at the cost of inflating the chain payload (~1/3
/// of the base). The pipeline bakes with the same filter as
/// [`build_rgba_levels_for_filter`], so an adopted chain is identical to a
/// generated one. Dimensions are checked against the texture actually being
/// uploaded: a stale or mismatched bake falls back to generating.
pub(crate) fn baked_mip_levels(
    source: &str,
    static_texture_lookup: Option<crate::backend::StaticTextureLookup>,
    width: u32,
    height: u32,
) -> Option<Vec<RgbaMipLevel>> {
    let bytes = match static_texture_lookup {
        Some(lookup) => {
            let hash = perro_ids::parse_hashed_source_uri(source)
                .unwrap_or_else(|| perro_ids::string_to_u64(source));
            let bytes = lookup(hash);
            (!bytes.is_empty()).then_some(std::borrow::Cow::Borrowed(bytes))
        }
        None => None,
    }
    .or_else(|| {
        // Disk / DLC `.ptex`: only these can carry a chain, so nothing else is
        // worth a read.
        if source.ends_with(".ptex") {
            perro_io::load_asset_cow(source).ok()
        } else {
            None
        }
    })?;

    let levels = perro_graphics_assets::decode_ptex_mip_levels(&bytes)?;
    let expected = rgba_mip_level_count(width, height).saturating_sub(1) as usize;
    let first_matches = levels
        .first()
        .is_some_and(|level| level.width == (width / 2).max(1) && level.height == (height / 2).max(1));
    (levels.len() == expected && first_matches).then_some(levels)
}

pub(crate) fn write_rgba_texture_streaming(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    rgba: &[u8],
    width: u32,
    height: u32,
    baked: Option<&[RgbaMipLevel]>,
) {
    write_texture_base_level(queue, texture, width, height, rgba);
    let mip_count = texture.mip_level_count();
    if mip_count <= 1 {
        return;
    }
    if let Some(levels) = baked.filter(|levels| levels.len() as u32 + 1 == mip_count) {
        for (index, level) in levels.iter().enumerate() {
            write_rgba_mip_level(
                queue,
                texture,
                index as u32 + 1,
                &level.rgba,
                level.width,
                level.height,
            );
        }
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
