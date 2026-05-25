//! 3D material texture decode and GPU cache allocation.

use crate::texture_mips::{
    build_rgba_levels_for_filter_owned, sampler_descriptor, write_rgba_mip_chain,
};
use perro_structs::TextureFilterMode;

const MATERIAL_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

pub(super) struct CachedMaterialTexture {
    pub(super) source: String,
    pub(super) _texture: Option<wgpu::Texture>,
    pub(super) _view: wgpu::TextureView,
    pub(super) _sampler: wgpu::Sampler,
    pub(super) bind_group: wgpu::BindGroup,
}

pub(super) struct CachedMaterialTextureInput {
    pub(super) rgba: Vec<u8>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) source: String,
    pub(super) filter: TextureFilterMode,
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
        format: MATERIAL_TEXTURE_FORMAT,
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
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("perro_material_texture_bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
        ],
    });
    CachedMaterialTexture {
        source: input.source,
        _texture: Some(texture),
        _view: view,
        _sampler: sampler,
        bind_group,
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
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("perro_external_material_texture_bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(view),
            },
        ],
    });
    CachedMaterialTexture {
        source,
        _texture: None,
        _view: view.clone(),
        _sampler: sampler,
        bind_group,
    }
}
