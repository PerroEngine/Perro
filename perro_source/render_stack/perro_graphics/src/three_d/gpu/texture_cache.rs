//! 3D material texture decode and GPU cache allocation.
//!
//! The actual texel uploads live in the Gpu-level `SharedTextureStore`; each
//! Gpu3D keeps per-slot handles (`Arc<SharedGpuTexture>`) plus its own
//! samplers and bind groups. Two material slots resolving to the same source
//! (same color space + mip decision) therefore share one `wgpu::Texture`,
//! as do the 2D/UI consumers and every per-camera-stream Gpu3D.

use crate::shared_textures::{
    SharedGpuTexture, SharedTextureColorSpace, SharedTextureKey, SharedTextureStore,
};
use crate::texture_mips::{sampler_descriptor, write_texture_base_level};
use perro_structs::TextureFilterMode;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MaterialTextureColorSpace {
    Srgb,
    Linear,
}

impl MaterialTextureColorSpace {
    fn shared(self) -> SharedTextureColorSpace {
        match self {
            Self::Srgb => SharedTextureColorSpace::Srgb,
            Self::Linear => SharedTextureColorSpace::Linear,
        }
    }
}

pub(super) struct CachedMaterialTexture {
    pub(super) source: String,
    // shared upload handle; None for external render-target views.
    pub(super) shared: Option<Arc<SharedGpuTexture>>,
    pub(super) view: wgpu::TextureView,
    pub(super) sampler: wgpu::Sampler,
    pub(super) width: u32,
    pub(super) height: u32,
}

impl CachedMaterialTexture {
    /// In-place base-level upload for a resident stream material texture (built
    /// single-level). Returns false when dims mismatch / not CPU-owned / built
    /// with mips (base-only write leaves stale mips; caller rebuilds instead).
    pub(super) fn write_stream_base_level(
        &self,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> bool {
        let Some(shared) = self.shared.as_ref() else {
            return false;
        };
        if self.width != width || self.height != height || shared.texture.mip_level_count() != 1 {
            return false;
        }
        write_texture_base_level(queue, &shared.texture, width, height, rgba);
        true
    }
}

pub(super) struct CachedMaterialTextureInput {
    // Arc: resident decoded buffers are shared in by refcount; the mip
    // builder borrows and copies only the base level.
    pub(super) rgba: Arc<[u8]>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) source: String,
    pub(super) filter: TextureFilterMode,
    pub(super) color_space: MaterialTextureColorSpace,
    /// Static lookup for the baked mip chain; `None` falls back to generating.
    pub(super) static_texture_lookup: Option<crate::backend::StaticTextureLookup>,
}

pub(super) fn material_shared_texture_key(
    source: &str,
    color_space: MaterialTextureColorSpace,
    filter: TextureFilterMode,
) -> SharedTextureKey {
    SharedTextureKey::from_source(source, color_space.shared(), filter)
}

/// Wrap an already-uploaded shared texture in a per-consumer cache entry
/// (own sampler; the view + texture stay shared).
pub(super) fn cached_material_texture_from_shared(
    device: &wgpu::Device,
    shared: Arc<SharedGpuTexture>,
    source: String,
    filter: TextureFilterMode,
) -> CachedMaterialTexture {
    let view = shared.view.clone();
    let sampler = device.create_sampler(&sampler_descriptor(
        "perro_material_texture_sampler",
        filter,
        wgpu::AddressMode::Repeat,
    ));
    let (width, height) = (shared.width, shared.height);
    CachedMaterialTexture {
        source,
        shared: Some(shared),
        view,
        sampler,
        width,
        height,
    }
}

/// Unbudgeted upload. Reserved for the tiny fallback textures every material
/// binds against: deferring those would leave slots with nothing to sample.
pub(super) fn create_cached_material_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    shared_textures: &mut SharedTextureStore,
    input: CachedMaterialTextureInput,
) -> CachedMaterialTexture {
    let key = material_shared_texture_key(&input.source, input.color_space, input.filter);
    let shared = shared_textures.ensure_rgba_from(
        device,
        queue,
        key,
        crate::shared_textures::TextureUpload::from_source(
            &input.rgba,
            input.width,
            input.height,
            input.source.as_str(),
            input.static_texture_lookup,
        ),
    );
    cached_material_texture_from_shared(device, shared, input.source, input.filter)
}

/// `create_cached_material_texture` under the per-frame upload budget. `None`
/// means the slot keeps its current binding (the fallback on a first load) and
/// retries next frame.
pub(super) fn try_create_cached_material_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    shared_textures: &mut SharedTextureStore,
    input: CachedMaterialTextureInput,
) -> Option<CachedMaterialTexture> {
    let key = material_shared_texture_key(&input.source, input.color_space, input.filter);
    let shared = shared_textures.try_ensure_rgba(
        device,
        queue,
        key,
        crate::shared_textures::TextureUpload::from_source(
            &input.rgba,
            input.width,
            input.height,
            input.source.as_str(),
            input.static_texture_lookup,
        ),
    )?;
    Some(cached_material_texture_from_shared(
        device,
        shared,
        input.source,
        input.filter,
    ))
}

pub(super) fn create_external_material_texture(
    device: &wgpu::Device,
    view: &wgpu::TextureView,
    source: String,
) -> CachedMaterialTexture {
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("perro_external_material_texture_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    CachedMaterialTexture {
        source,
        shared: None,
        view: view.clone(),
        sampler,
        width: 0,
        height: 0,
    }
}

pub(super) fn create_material_texture_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    base_view: &wgpu::TextureView,
    custom_views: &[&wgpu::TextureView],
) -> wgpu::BindGroup {
    let mut entries = Vec::with_capacity(super::MATERIAL_TEXTURE_SET_SIZE + 1);
    entries.push(wgpu::BindGroupEntry {
        binding: 0,
        resource: wgpu::BindingResource::Sampler(sampler),
    });
    entries.push(wgpu::BindGroupEntry {
        binding: 1,
        resource: wgpu::BindingResource::TextureView(base_view),
    });
    for i in 0..super::CUSTOM_MATERIAL_IMAGE_COUNT {
        let view = custom_views.get(i).copied().unwrap_or(base_view);
        entries.push(wgpu::BindGroupEntry {
            binding: 2 + i as u32,
            resource: wgpu::BindingResource::TextureView(view),
        });
    }
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("perro_material_texture_bg"),
        layout,
        entries: &entries,
    })
}
