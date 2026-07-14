//! 3D material texture decode and GPU cache allocation.

use crate::texture_mips::{
    build_rgba_levels_for_filter_owned, sampler_descriptor, write_rgba_mip_chain,
    write_texture_base_level,
};
use perro_structs::TextureFilterMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MaterialTextureColorSpace {
    Srgb,
    Linear,
}

impl MaterialTextureColorSpace {
    fn format(self) -> wgpu::TextureFormat {
        match self {
            Self::Srgb => wgpu::TextureFormat::Rgba8UnormSrgb,
            Self::Linear => wgpu::TextureFormat::Rgba8Unorm,
        }
    }
}

pub(super) struct CachedMaterialTexture {
    pub(super) source: String,
    pub(super) texture: Option<wgpu::Texture>,
    pub(super) view: wgpu::TextureView,
    pub(super) sampler: wgpu::Sampler,
    #[allow(dead_code)]
    pub(super) bind_group: wgpu::BindGroup,
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
        let Some(texture) = self.texture.as_ref() else {
            return false;
        };
        if self.width != width || self.height != height || texture.mip_level_count() != 1 {
            return false;
        }
        write_texture_base_level(queue, texture, width, height, rgba);
        true
    }
}

pub(super) struct CachedMaterialTextureInput {
    pub(super) rgba: Vec<u8>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) source: String,
    pub(super) filter: TextureFilterMode,
    pub(super) color_space: MaterialTextureColorSpace,
}

pub(super) fn create_cached_material_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    input: CachedMaterialTextureInput,
) -> CachedMaterialTexture {
    let width = input.width.max(1);
    let height = input.height.max(1);
    let mips = build_rgba_levels_for_filter_owned(input.rgba, width, height, input.filter);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_material_texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: mips.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: input.color_space.format(),
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    write_rgba_mip_chain(queue, &texture, &mips);
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("perro_material_texture_view"),
        ..Default::default()
    });
    let sampler = device.create_sampler(&sampler_descriptor(
        "perro_material_texture_sampler",
        input.filter,
        wgpu::AddressMode::Repeat,
    ));
    let bind_group = create_material_texture_bind_group(device, layout, &sampler, &view, &[]);
    CachedMaterialTexture {
        source: input.source,
        texture: Some(texture),
        view,
        sampler,
        bind_group,
        width,
        height,
    }
}

pub(super) fn create_external_material_texture(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
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
    let bind_group = create_material_texture_bind_group(device, layout, &sampler, view, &[]);
    CachedMaterialTexture {
        source,
        texture: None,
        view: view.clone(),
        sampler,
        bind_group,
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
