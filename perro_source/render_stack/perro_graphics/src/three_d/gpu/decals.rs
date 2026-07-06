//! Lit decals: projected boxes patched into the material shaders.
//!
//! Decal records live in a group(0) storage buffer read by every lit material
//! pass (skinned/rigid/multimesh + toon); textures live in one shared
//! `texture_2d_array` so the per-material texture bind group stays untouched.
//! Layers store the source bytes unmodified (linear view); the shader decodes
//! sRGB for albedo/emission and reads normal maps raw.

use super::*;
use crate::texture_mips::{RgbaMipLevel, build_rgba_levels_for_filter_owned, rgba_mip_level_count};
use perro_ids::{NodeID, TextureID};
use perro_render_bridge::Decal3DState;
use perro_structs::TextureFilterMode;

pub(super) const DECAL_LAYER_SIZE: u32 = 512;
pub(super) const DECAL_MAX_LAYERS: u32 = 64;
pub(super) const DECAL_INITIAL_LAYERS: u32 = 4;
pub(super) const DECAL_MAX_DECALS: usize = 64;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct DecalGpu {
    // World -> unit-box ([-0.5, 0.5]^3) transform rows.
    pub(super) inv_row_0: [f32; 4],
    pub(super) inv_row_1: [f32; 4],
    pub(super) inv_row_2: [f32; 4],
    // rgb tint, a = opacity.
    pub(super) tint: [f32; 4],
    // rgb = emission color * energy, w = normal strength.
    pub(super) emission: [f32; 4],
    // x = albedo layer (-1 none), y = normal layer, z = emission layer,
    // w = normal fade threshold.
    pub(super) params0: [f32; 4],
    // x = albedo mix, y = distance fade begin (0 = off),
    // z = 1 / distance fade length, w = unused.
    pub(super) params1: [f32; 4],
}

pub(super) fn create_decal_texture_array(
    device: &wgpu::Device,
    layers: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_decal_texture_array"),
        size: wgpu::Extent3d {
            width: DECAL_LAYER_SIZE,
            height: DECAL_LAYER_SIZE,
            depth_or_array_layers: layers,
        },
        mip_level_count: rgba_mip_level_count(DECAL_LAYER_SIZE, DECAL_LAYER_SIZE),
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("perro_decal_texture_array_view"),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    (texture, view)
}

pub(super) fn create_decal_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("perro_decal_buffer"),
        size: (16 + capacity * std::mem::size_of::<DecalGpu>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

// Bilinear resample into the fixed layer size so every decal texture fits one
// array layer regardless of source dimensions.
fn resize_rgba_bilinear(rgba: &[u8], width: u32, height: u32, out_size: u32) -> Vec<u8> {
    let width = width.max(1) as usize;
    let height = height.max(1) as usize;
    let out = out_size as usize;
    if width == out && height == out && rgba.len() >= out * out * 4 {
        return rgba[..out * out * 4].to_vec();
    }
    let mut dst = vec![0u8; out * out * 4];
    let sx = width as f32 / out as f32;
    let sy = height as f32 / out as f32;
    for y in 0..out {
        let fy = ((y as f32 + 0.5) * sy - 0.5).max(0.0);
        let y0 = (fy as usize).min(height - 1);
        let y1 = (y0 + 1).min(height - 1);
        let ty = fy - y0 as f32;
        for x in 0..out {
            let fx = ((x as f32 + 0.5) * sx - 0.5).max(0.0);
            let x0 = (fx as usize).min(width - 1);
            let x1 = (x0 + 1).min(width - 1);
            let tx = fx - x0 as f32;
            let d = (y * out + x) * 4;
            for c in 0..4 {
                let p00 = rgba[(y0 * width + x0) * 4 + c] as f32;
                let p10 = rgba[(y0 * width + x1) * 4 + c] as f32;
                let p01 = rgba[(y1 * width + x0) * 4 + c] as f32;
                let p11 = rgba[(y1 * width + x1) * 4 + c] as f32;
                let top = p00 + (p10 - p00) * tx;
                let bottom = p01 + (p11 - p01) * tx;
                dst[d + c] = (top + (bottom - top) * ty).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    dst
}

fn write_decal_layer(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    layer: u32,
    levels: &[RgbaMipLevel],
) {
    for (mip_level, level) in levels.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: mip_level as u32,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &level.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * level.width),
                rows_per_image: Some(level.height),
            },
            wgpu::Extent3d {
                width: level.width,
                height: level.height,
                depth_or_array_layers: 1,
            },
        );
    }
}

impl Gpu3D {
    /// True while a decal references a texture that has not finished decoding.
    /// The frame loop keeps re-preparing until this clears so the decal appears
    /// as soon as its async texture arrives (no reload needed).
    pub fn decals_pending(&self) -> bool {
        self.decal_sources_pending
    }

    /// Rebuild the decal storage buffer + texture layers. Skips work when the
    /// retained set is unchanged and no texture source is still pending.
    pub(super) fn prepare_decals(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &ResourceStore,
        decals: &[(NodeID, Decal3DState)],
        revision: u64,
    ) {
        if revision == self.last_decals_revision && !self.decal_sources_pending {
            return;
        }
        self.last_decals_revision = revision;
        self.decal_sources_pending = false;

        let mut sorted: Vec<&Decal3DState> = decals.iter().map(|(_, d)| d).collect();
        // Low priority first: later decals blend over earlier ones in-shader.
        sorted.sort_by_key(|d| d.sort_priority);
        sorted.truncate(DECAL_MAX_DECALS);

        let mut records: Vec<DecalGpu> = Vec::with_capacity(sorted.len());
        for decal in sorted {
            let albedo_layer =
                self.decal_layer_for_texture(device, queue, resources, decal.albedo_texture);
            let normal_layer =
                self.decal_layer_for_texture(device, queue, resources, decal.normal_texture);
            let emission_layer =
                self.decal_layer_for_texture(device, queue, resources, decal.emission_texture);
            // Hide the decal until every assigned texture finishes decoding.
            // A missing albedo layer otherwise falls through to the flat-color
            // path, flashing the modulate color (white box) on first load until
            // the async texture arrives.
            let waiting = (!decal.albedo_texture.is_nil() && albedo_layer.is_none())
                || (!decal.normal_texture.is_nil() && normal_layer.is_none())
                || (!decal.emission_texture.is_nil() && emission_layer.is_none());
            if waiting {
                continue;
            }
            let rot = glam::Quat::from(decal.rotation).normalize();
            let model = glam::Mat4::from_scale_rotation_translation(
                glam::Vec3::from(decal.size).max(glam::Vec3::splat(0.001)),
                rot,
                decal.position.into(),
            );
            let inv = model.inverse();
            let layer_or = |layer: Option<u32>| layer.map(|l| l as f32).unwrap_or(-1.0);
            let modulate: [f32; 4] = decal.modulate.into();
            records.push(DecalGpu {
                inv_row_0: inv.row(0).to_array(),
                inv_row_1: inv.row(1).to_array(),
                inv_row_2: inv.row(2).to_array(),
                tint: [
                    modulate[0],
                    modulate[1],
                    modulate[2],
                    modulate[3].clamp(0.0, 1.0),
                ],
                emission: [
                    modulate[0] * decal.emission_energy,
                    modulate[1] * decal.emission_energy,
                    modulate[2] * decal.emission_energy,
                    decal.normal_strength,
                ],
                params0: [
                    layer_or(albedo_layer),
                    layer_or(normal_layer),
                    layer_or(emission_layer),
                    decal.normal_fade.clamp(0.0, 0.99),
                ],
                params1: [
                    decal.albedo_mix.clamp(0.0, 1.0),
                    decal.distance_fade_begin.max(0.0),
                    1.0 / decal.distance_fade_length.max(0.001),
                    0.0,
                ],
            });
        }

        if records.len() > self.decal_buffer_capacity {
            let mut capacity = self.decal_buffer_capacity.max(8);
            while capacity < records.len() {
                capacity *= 2;
            }
            self.decal_buffer = create_decal_buffer(device, capacity);
            self.decal_buffer_capacity = capacity;
            self.rebuild_camera_bind_groups(device);
        }
        let header: [u32; 4] = [records.len() as u32, 0, 0, 0];
        queue.write_buffer(&self.decal_buffer, 0, bytemuck::cast_slice(&header));
        if !records.is_empty() {
            queue.write_buffer(&self.decal_buffer, 16, bytemuck::cast_slice(&records));
        }
        self.decal_count = records.len() as u32;
    }

    // TextureID -> array layer; decodes + uploads on first sight. None = nil
    // id, not yet decodable (pending resource), or out of layers.
    fn decal_layer_for_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &ResourceStore,
        id: TextureID,
    ) -> Option<u32> {
        if id.is_nil() {
            return None;
        }
        if let Some(layer) = self.decal_layer_by_texture.get(&id) {
            return Some(*layer);
        }
        let Some((rgba, width, height)) = decal_texture_rgba(resources, id) else {
            // Registered but not decoded yet; retry next prepare.
            self.decal_sources_pending = true;
            return None;
        };
        let layer = self.decal_layer_by_texture.len() as u32;
        if layer >= DECAL_MAX_LAYERS {
            return None;
        }
        if layer >= self.decal_texture_layers {
            self.grow_decal_texture(device, queue, resources);
        }
        write_decal_layer(
            queue,
            &self.decal_texture,
            layer,
            &decal_layer_levels(rgba, width, height),
        );
        self.decal_layer_by_texture.insert(id, layer);
        Some(layer)
    }

    // Double the layer count and re-upload every cached texture into the new
    // array (grow is rare; re-decoding keeps the cache pixel-free).
    fn grow_decal_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &ResourceStore,
    ) {
        let new_layers = (self.decal_texture_layers * 2).min(DECAL_MAX_LAYERS);
        let (texture, view) = create_decal_texture_array(device, new_layers);
        self.decal_texture = texture;
        self.decal_texture_view = view;
        self.decal_texture_layers = new_layers;
        let existing: Vec<(TextureID, u32)> = self
            .decal_layer_by_texture
            .iter()
            .map(|(id, layer)| (*id, *layer))
            .collect();
        for (id, layer) in existing {
            let Some((rgba, width, height)) = decal_texture_rgba(resources, id) else {
                continue;
            };
            write_decal_layer(
                queue,
                &self.decal_texture,
                layer,
                &decal_layer_levels(rgba, width, height),
            );
        }
        self.rebuild_camera_bind_groups(device);
    }
}

fn decal_texture_rgba(resources: &ResourceStore, id: TextureID) -> Option<(Vec<u8>, u32, u32)> {
    let source = resources.texture_source(id)?;
    resources
        .decoded_texture_data_by_source(source)
        .map(|decoded| (decoded.rgba.clone(), decoded.width, decoded.height))
        .or_else(|| load_texture_rgba(source))
}

fn decal_layer_levels(rgba: Vec<u8>, width: u32, height: u32) -> Vec<RgbaMipLevel> {
    let resized = resize_rgba_bilinear(&rgba, width, height, DECAL_LAYER_SIZE);
    build_rgba_levels_for_filter_owned(
        resized,
        DECAL_LAYER_SIZE,
        DECAL_LAYER_SIZE,
        TextureFilterMode::LinearMipmap,
    )
}
