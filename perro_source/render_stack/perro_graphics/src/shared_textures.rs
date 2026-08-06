//! Gpu-level shared texture uploads.
//!
//! One decoded image used by several consumer caches (main/late-overlay 2D,
//! per-camera-stream 2D, UI images, main + per-stream 3D material slots) used
//! to be uploaded once per consumer, multiplying VRAM by the consumer count.
//! This store keys uploads by (source hash, color space, mip presence) and
//! hands out `Arc<SharedGpuTexture>` handles; consumers keep only their own
//! samplers + bind groups (bind group layouts are per-pipeline).

use crate::backend::StaticTextureLookup;
use crate::texture_mips::{
    rgba_mip_level_count, write_rgba_texture_streaming, write_texture_base_level,
};
use ahash::AHashMap;
use perro_structs::TextureFilterMode;
use std::sync::Arc;

/// Sweeps an entry must stay unreferenced (strong_count == 1, store only)
/// before the periodic sweep evicts it.
const SHARED_TEXTURE_EVICT_SWEEPS: u32 = 2;

/// Texel bytes (base level + generated mips) one frame may upload before the
/// store defers further first-time uploads to later frames.
///
/// Decode and file IO already fan out across the rayon pool, but every decoded
/// texture still pays its `write_texture` + CPU mip chain on the render thread
/// at first use. A scene that reveals hundreds of textures at once paid that
/// whole burst in one frame; the budget spreads it while leaving normal loads
/// (a handful of textures, or one big one) same-frame.
const UPLOAD_BUDGET_BYTES_PER_FRAME: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SharedTextureColorSpace {
    Srgb,
    Linear,
}

impl SharedTextureColorSpace {
    pub(crate) fn format(self) -> wgpu::TextureFormat {
        match self {
            Self::Srgb => wgpu::TextureFormat::Rgba8UnormSrgb,
            Self::Linear => wgpu::TextureFormat::Rgba8Unorm,
        }
    }
}

/// Identity of one GPU upload. Two consumers requesting the same source with
/// the same color space and the same mip decision share one `wgpu::Texture`.
/// The exact filter mode is NOT part of the key: the baked texel contents only
/// differ by whether a mip chain exists (`TextureFilterMode::uses_mipmaps`);
/// mag/min/mip filtering lives in per-consumer samplers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SharedTextureKey {
    pub source_hash: u64,
    pub color_space: SharedTextureColorSpace,
    pub mips: bool,
}

impl SharedTextureKey {
    pub(crate) fn from_source(
        source: &str,
        color_space: SharedTextureColorSpace,
        filter: TextureFilterMode,
    ) -> Self {
        Self {
            source_hash: shared_texture_source_hash(source),
            color_space,
            mips: filter.uses_mipmaps(),
        }
    }
}

/// Same normalization the 3D custom-material slot map uses, so `res://`-style
/// hashed URIs and plain paths land on one identity.
pub(crate) fn shared_texture_source_hash(source: &str) -> u64 {
    perro_ids::parse_hashed_source_uri(source).unwrap_or_else(|| perro_ids::string_to_u64(source))
}

pub(crate) struct SharedGpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

struct SharedTextureSlot {
    texture: Arc<SharedGpuTexture>,
    idle_sweeps: u32,
}

#[derive(Default)]
pub(crate) struct SharedTextureStore {
    entries: AHashMap<SharedTextureKey, SharedTextureSlot>,
    upload_bytes_this_frame: usize,
    deferred_uploads: bool,
}

/// One texture upload request: the decoded base level plus where its mip chain
/// can be adopted from.
pub(crate) struct TextureUpload<'a> {
    pub rgba: &'a [u8],
    pub width: u32,
    pub height: u32,
    /// Asset the pixels came from, for adopting a baked `.ptex` chain. `None`
    /// (runtime-created pixels, fallbacks) always generates.
    pub mip_source: Option<(&'a str, Option<StaticTextureLookup>)>,
}

impl<'a> TextureUpload<'a> {
    #[cfg(test)]
    pub(crate) fn new(rgba: &'a [u8], width: u32, height: u32) -> Self {
        Self {
            rgba,
            width,
            height,
            mip_source: None,
        }
    }

    pub(crate) fn from_source(
        rgba: &'a [u8],
        width: u32,
        height: u32,
        source: &'a str,
        lookup: Option<StaticTextureLookup>,
    ) -> Self {
        Self {
            rgba,
            width,
            height,
            mip_source: Some((source, lookup)),
        }
    }
}

/// Texel bytes one upload costs: base level, plus ~1/3 again when the key
/// carries a mip chain (sum of the quarter-size levels).
fn upload_cost_bytes(mips: bool, width: u32, height: u32) -> usize {
    let base = width.max(1) as usize * height.max(1) as usize * 4;
    if mips { base + base / 3 } else { base }
}

impl SharedTextureStore {
    pub(crate) fn get(&self, key: &SharedTextureKey) -> Option<Arc<SharedGpuTexture>> {
        self.entries.get(key).map(|slot| slot.texture.clone())
    }

    /// Reset the per-frame upload budget. Called once per rendered frame.
    pub(crate) fn begin_frame_uploads(&mut self) {
        self.upload_bytes_this_frame = 0;
        self.deferred_uploads = false;
    }

    /// Whether an upload was pushed to a later frame, so the caller keeps the
    /// frame pump awake until the backlog drains (an otherwise-static scene
    /// would never ask for the frame that finishes its textures).
    pub(crate) fn deferred_uploads_pending(&self) -> bool {
        self.deferred_uploads
    }

    /// [`Self::ensure_rgba`] under the per-frame upload budget: `None` means
    /// "not this frame", and the caller falls through its existing
    /// texture-not-ready path and retries next frame.
    ///
    /// The first upload of a frame always goes through, so one texture larger
    /// than the whole budget can never stall forever.
    pub(crate) fn try_ensure_rgba(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        key: SharedTextureKey,
        upload: TextureUpload<'_>,
    ) -> Option<Arc<SharedGpuTexture>> {
        if let Some(existing) = self.get(&key) {
            return Some(existing);
        }
        let cost = upload_cost_bytes(key.mips, upload.width, upload.height);
        if self.upload_bytes_this_frame > 0
            && self.upload_bytes_this_frame.saturating_add(cost) > UPLOAD_BUDGET_BYTES_PER_FRAME
        {
            self.deferred_uploads = true;
            return None;
        }
        Some(self.ensure_rgba_from(device, queue, key, upload))
    }

    /// Upload-or-reuse. On a hit the decoded bytes are ignored (the resident
    /// texture already holds them); on a miss the texture + mip chain (per
    /// `key.mips`) is created and written once.
    #[cfg(test)]
    pub(crate) fn ensure_rgba(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        key: SharedTextureKey,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> Arc<SharedGpuTexture> {
        self.ensure_rgba_from(device, queue, key, TextureUpload::new(rgba, width, height))
    }

    /// `ensure_rgba` that can adopt the mip chain baked into the source
    /// `.ptex` instead of downsampling on this thread. `mip_source` is only
    /// consulted on a miss for a mipped key.
    pub(crate) fn ensure_rgba_from(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        key: SharedTextureKey,
        upload: TextureUpload<'_>,
    ) -> Arc<SharedGpuTexture> {
        if let Some(existing) = self.get(&key) {
            return existing;
        }
        let TextureUpload {
            rgba,
            width,
            height,
            mip_source,
        } = upload;
        let width = width.max(1);
        let height = height.max(1);
        self.upload_bytes_this_frame = self
            .upload_bytes_this_frame
            .saturating_add(upload_cost_bytes(key.mips, width, height));
        let base_len = width as usize * height as usize * 4;
        let valid = rgba.len() >= base_len;
        // Mip contents depend only on the mips bit (mag/min/mip filtering lives
        // in per-consumer samplers). Undersized input keeps the old fallback:
        // a single-level texture with 1x1 white written into level 0.
        let mip_level_count = if key.mips && valid {
            rgba_mip_level_count(width, height)
        } else {
            1
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("perro_shared_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: key.color_space.format(),
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        if valid {
            // Streaming upload: base level straight from the borrowed slice,
            // mips generated level-by-level into reused scratch. The full mip
            // chain is never materialized on the CPU.
            // A baked chain skips the CPU downsample; without one the streaming
            // path generates each level into reused scratch, so neither route
            // materializes the whole chain.
            let baked = if mip_level_count > 1 {
                mip_source.and_then(|(source, lookup)| {
                    crate::texture_mips::baked_mip_levels(source, lookup, width, height)
                })
            } else {
                None
            };
            write_rgba_texture_streaming(
                queue,
                &texture,
                &rgba[..base_len],
                width,
                height,
                baked.as_deref(),
            );
        } else {
            write_texture_base_level(queue, &texture, 1, 1, &[255, 255, 255, 255]);
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("perro_shared_texture_view"),
            ..Default::default()
        });
        let shared = Arc::new(SharedGpuTexture {
            texture,
            view,
            width,
            height,
        });
        self.entries.insert(
            key,
            SharedTextureSlot {
                texture: shared.clone(),
                idle_sweeps: 0,
            },
        );
        shared
    }

    /// Drop every color-space/mip variant uploaded for `source`. Consumers
    /// must drop their handles + bind groups too (the existing per-consumer
    /// invalidate fan-out does exactly that); the next demand re-uploads once.
    pub(crate) fn invalidate_source(&mut self, source: &str) {
        let hash = shared_texture_source_hash(source);
        self.entries.retain(|key, _| key.source_hash != hash);
    }

    /// One in-place base-level write per resident single-level variant of
    /// `source` (stream textures upload without mips, so a matching write
    /// updates every consumer at once - that is the point of sharing).
    /// Returns whether any variant accepted the write.
    pub(crate) fn write_stream_base_level(
        &mut self,
        queue: &wgpu::Queue,
        source: &str,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> bool {
        let hash = shared_texture_source_hash(source);
        let mut wrote = false;
        for (key, slot) in self.entries.iter() {
            if key.source_hash != hash {
                continue;
            }
            let shared = slot.texture.as_ref();
            if shared.width != width
                || shared.height != height
                || shared.texture.mip_level_count() != 1
            {
                continue;
            }
            write_texture_base_level(queue, &shared.texture, width, height, rgba);
            wrote = true;
        }
        wrote
    }

    /// Refcount eviction: entries whose only strong reference is the store
    /// itself for `SHARED_TEXTURE_EVICT_SWEEPS` consecutive sweeps are freed.
    /// A handle re-appearing between sweeps resets the counter.
    pub(crate) fn sweep(&mut self) {
        self.entries.retain(|_, slot| {
            if Arc::strong_count(&slot.texture) > 1 {
                slot.idle_sweeps = 0;
                return true;
            }
            slot.idle_sweeps += 1;
            slot.idle_sweeps < SHARED_TEXTURE_EVICT_SWEEPS
        });
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        pollster::block_on(async {
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
                    label: Some("perro_shared_texture_test_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    experimental_features: wgpu::ExperimentalFeatures::disabled(),
                    memory_hints: wgpu::MemoryHints::Performance,
                    trace: wgpu::Trace::default(),
                })
                .await
                .ok()
        })
    }

    fn key(source: &str, filter: TextureFilterMode) -> SharedTextureKey {
        SharedTextureKey::from_source(source, SharedTextureColorSpace::Srgb, filter)
    }

    #[test]
    fn key_filter_only_distinguishes_mip_presence() {
        let base = key("res://a.png", TextureFilterMode::Linear);
        assert_eq!(base, key("res://a.png", TextureFilterMode::Nearest));
        let mipped = key("res://a.png", TextureFilterMode::LinearMipmap);
        assert_eq!(mipped, key("res://a.png", TextureFilterMode::Anisotropic));
        assert_ne!(base, mipped);
        assert_ne!(base, key("res://b.png", TextureFilterMode::Linear));
        assert_ne!(
            SharedTextureKey::from_source(
                "res://a.png",
                SharedTextureColorSpace::Linear,
                TextureFilterMode::Linear,
            ),
            base
        );
    }

    #[test]
    fn two_consumers_share_one_upload() {
        let Some((device, queue)) = test_device() else {
            eprintln!("skip shared texture test: no wgpu adapter");
            return;
        };
        let mut store = SharedTextureStore::default();
        let rgba = vec![255u8; 2 * 2 * 4];
        let k = key("res://shared.png", TextureFilterMode::LinearMipmap);
        let first = store.ensure_rgba(&device, &queue, k, &rgba, 2, 2);
        let second = store.ensure_rgba(&device, &queue, k, &rgba, 2, 2);
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(store.len(), 1);
        assert_eq!(first.texture.mip_level_count(), 2);
    }

    #[test]
    fn different_mip_modes_get_distinct_entries() {
        let Some((device, queue)) = test_device() else {
            eprintln!("skip shared texture test: no wgpu adapter");
            return;
        };
        let mut store = SharedTextureStore::default();
        let rgba = vec![255u8; 2 * 2 * 4];
        let mipped = store.ensure_rgba(
            &device,
            &queue,
            key("res://x.png", TextureFilterMode::LinearMipmap),
            &rgba,
            2,
            2,
        );
        let flat = store.ensure_rgba(
            &device,
            &queue,
            key("res://x.png", TextureFilterMode::Linear),
            &rgba,
            2,
            2,
        );
        assert!(!Arc::ptr_eq(&mipped, &flat));
        assert_eq!(store.len(), 2);
        assert_eq!(mipped.texture.mip_level_count(), 2);
        assert_eq!(flat.texture.mip_level_count(), 1);
    }

    #[test]
    fn invalidate_drops_all_variants_and_refetch_reuploads() {
        let Some((device, queue)) = test_device() else {
            eprintln!("skip shared texture test: no wgpu adapter");
            return;
        };
        let mut store = SharedTextureStore::default();
        let rgba = vec![128u8; 4];
        let k_mip = key("res://inv.png", TextureFilterMode::LinearMipmap);
        let k_flat = key("res://inv.png", TextureFilterMode::Linear);
        let before_a = store.ensure_rgba(&device, &queue, k_mip, &rgba, 1, 1);
        let before_b = store.ensure_rgba(&device, &queue, k_flat, &rgba, 1, 1);
        store.invalidate_source("res://inv.png");
        assert_eq!(store.len(), 0);
        let after_a = store.ensure_rgba(&device, &queue, k_mip, &rgba, 1, 1);
        let after_b = store.ensure_rgba(&device, &queue, k_flat, &rgba, 1, 1);
        assert!(!Arc::ptr_eq(&before_a, &after_a));
        assert!(!Arc::ptr_eq(&before_b, &after_b));
    }

    #[test]
    fn sweep_evicts_only_unreferenced_entries_after_grace() {
        let Some((device, queue)) = test_device() else {
            eprintln!("skip shared texture test: no wgpu adapter");
            return;
        };
        let mut store = SharedTextureStore::default();
        let rgba = vec![0u8; 4];
        let held_key = key("res://held.png", TextureFilterMode::Linear);
        let dropped_key = key("res://dropped.png", TextureFilterMode::Linear);
        let held = store.ensure_rgba(&device, &queue, held_key, &rgba, 1, 1);
        drop(store.ensure_rgba(&device, &queue, dropped_key, &rgba, 1, 1));
        store.sweep();
        assert_eq!(store.len(), 2, "first idle sweep keeps the grace entry");
        store.sweep();
        assert_eq!(store.len(), 1);
        assert!(store.get(&held_key).is_some());
        assert!(store.get(&dropped_key).is_none());
        drop(held);
    }

    #[test]
    fn upload_budget_defers_after_first_frame_burst() {
        let Some((device, queue)) = test_device() else {
            eprintln!("skip shared texture test: no wgpu adapter");
            return;
        };
        let mut store = SharedTextureStore::default();
        // 2048^2 rgba = 16 MiB base; two fit the 32 MiB frame budget, the third
        // (mips push each past 16 MiB) does not.
        let dim = 2048u32;
        let rgba = vec![0u8; dim as usize * dim as usize * 4];
        store.begin_frame_uploads();
        for index in 0..2 {
            assert!(
                store
                    .try_ensure_rgba(
                        &device,
                        &queue,
                        key(&format!("res://budget{index}.png"), TextureFilterMode::Linear),
                        TextureUpload::new(&rgba, dim, dim),
                    )
                    .is_some(),
                "upload {index} must fit the frame budget"
            );
        }
        assert!(
            store
                .try_ensure_rgba(
                    &device,
                    &queue,
                    key("res://budget_over.png", TextureFilterMode::Linear),
                    TextureUpload::new(&rgba, dim, dim),
                )
                .is_none()
        );
        assert!(store.deferred_uploads_pending());

        // Next frame resets the budget, so the deferred upload lands.
        store.begin_frame_uploads();
        assert!(!store.deferred_uploads_pending());
        assert!(
            store
                .try_ensure_rgba(
                    &device,
                    &queue,
                    key("res://budget_over.png", TextureFilterMode::Linear),
                    TextureUpload::new(&rgba, dim, dim),
                )
                .is_some()
        );
    }

    #[test]
    fn upload_budget_never_stalls_a_single_oversized_texture() {
        let Some((device, queue)) = test_device() else {
            eprintln!("skip shared texture test: no wgpu adapter");
            return;
        };
        let mut store = SharedTextureStore::default();
        // 4096^2 rgba = 64 MiB: bigger than the whole frame budget on its own.
        let dim = 4096u32;
        let rgba = vec![0u8; dim as usize * dim as usize * 4];
        store.begin_frame_uploads();
        assert!(
            store
                .try_ensure_rgba(
                    &device,
                    &queue,
                    key("res://huge.png", TextureFilterMode::Linear),
                    TextureUpload::new(&rgba, dim, dim),
                )
                .is_some(),
            "first upload of a frame always goes through"
        );
        assert!(!store.deferred_uploads_pending());
    }

    #[test]
    fn stream_write_targets_matching_single_level_entries() {
        let Some((device, queue)) = test_device() else {
            eprintln!("skip shared texture test: no wgpu adapter");
            return;
        };
        let mut store = SharedTextureStore::default();
        let rgba = vec![10u8; 2 * 2 * 4];
        store.ensure_rgba(
            &device,
            &queue,
            key("res://stream.png", TextureFilterMode::Linear),
            &rgba,
            2,
            2,
        );
        assert!(store.write_stream_base_level(&queue, "res://stream.png", 2, 2, &rgba));
        // dim mismatch and unknown sources refuse the in-place path.
        assert!(!store.write_stream_base_level(&queue, "res://stream.png", 4, 4, &rgba));
        assert!(!store.write_stream_base_level(&queue, "res://other.png", 2, 2, &rgba));
    }
}
