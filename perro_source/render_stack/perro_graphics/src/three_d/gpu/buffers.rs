use super::*;

pub(super) struct ShadowMultimeshBgArgs<'a> {
    pub(super) device: &'a wgpu::Device,
    pub(super) multimesh_bgl: &'a wgpu::BindGroupLayout,
    pub(super) shadow_camera_buffers: &'a [wgpu::Buffer],
    pub(super) multimesh_draw_params_buffer: &'a wgpu::Buffer,
    pub(super) mesh_blend_depth_view: &'a wgpu::TextureView,
    pub(super) blend_shape_delta_buffer: &'a wgpu::Buffer,
    pub(super) blend_shape_weight_buffer: &'a wgpu::Buffer,
    pub(super) blend_shape_instance_meta_buffer: &'a wgpu::Buffer,
    pub(super) custom_params_meta_buffer: &'a wgpu::Buffer,
    pub(super) custom_params_values_buffer: &'a wgpu::Buffer,
    pub(super) shadow_identity_buffer: &'a wgpu::Buffer,
    pub(super) multimesh_instance_buffer: &'a wgpu::Buffer,
    pub(super) decal_buffer: &'a wgpu::Buffer,
    pub(super) decal_texture_view: &'a wgpu::TextureView,
    pub(super) decal_sampler: &'a wgpu::Sampler,
    pub(super) ssao_view: &'a wgpu::TextureView,
}

// One multimesh draw bind group per shadow layer: identical to multimesh_bgl
// except binding 0 = that layer's scene uniform (light view-proj) and binding 8
// = the dedicated identity index buffer, so vs_depth draws the full instance set
// projected into the light's view regardless of the camera cull output.
pub(super) fn build_shadow_multimesh_bind_groups(
    args: ShadowMultimeshBgArgs<'_>,
) -> Vec<wgpu::BindGroup> {
    args.shadow_camera_buffers
        .iter()
        .map(|scene_buffer| {
            args.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("perro_shadow_multimesh_bg"),
                layout: args.multimesh_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: scene_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: args.multimesh_draw_params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(args.mesh_blend_depth_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: args.blend_shape_delta_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: args.blend_shape_weight_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: args.blend_shape_instance_meta_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: args.custom_params_meta_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: args.custom_params_values_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 8,
                        resource: args.shadow_identity_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 9,
                        resource: args.multimesh_instance_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 10,
                        resource: args.decal_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 11,
                        resource: wgpu::BindingResource::TextureView(args.decal_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 12,
                        resource: wgpu::BindingResource::Sampler(args.decal_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 13,
                        resource: wgpu::BindingResource::TextureView(args.ssao_view),
                    },
                ],
            })
        })
        .collect()
}

struct AppendPackedLodDataArgs<'a> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    vertices: &'a [MeshVertex],
    mesh_indices: &'a [u32],
    base_vertex: u32,
    decoded_lods: &'a [DecodedLod],
    decoded_surfaces: &'a [MeshRange],
}

fn material_texture_key_uses_slot(key: &MaterialTextureKey, slot: u32) -> bool {
    let source_slot = material_texture_source_slot(slot);
    source_slot != MATERIAL_TEXTURE_NONE
        && key
            .slots
            .iter()
            .copied()
            .any(|key_slot| material_texture_source_slot(key_slot) == source_slot)
}

fn material_texture_key_survives_slot_evict(key: &MaterialTextureKey, slot: u32) -> bool {
    !material_texture_key_uses_slot(key, slot)
}

#[path = "buffers/capacities.rs"]
mod capacities;
#[path = "buffers/materials.rs"]
mod materials;
#[path = "buffers/mesh.rs"]
mod mesh;
#[path = "buffers/occlusion.rs"]
mod occlusion;

fn bounded_growth_capacity(current: usize, needed: usize, max: usize) -> Option<usize> {
    if needed > max {
        return None;
    }
    let mut capacity = current.max(1).min(max);
    while capacity < needed {
        capacity = capacity.saturating_mul(2).min(max);
    }
    Some(capacity)
}

fn packed_lod_param(
    vertices: &[MeshVertex],
    uploaded_indices: &[u32],
    base_vertex: u32,
) -> Option<PackedLodParamGpu> {
    let mut pos_min = [f32::INFINITY; 3];
    let mut pos_max = [f32::NEG_INFINITY; 3];
    let mut uv_min = [f32::INFINITY; 2];
    let mut uv_max = [f32::NEG_INFINITY; 2];
    let mut any = false;
    for &uploaded_index in uploaded_indices {
        let local_index = uploaded_index.saturating_sub(base_vertex);
        let Some(vertex) = vertices.get(local_index as usize) else {
            continue;
        };
        any = true;
        for axis in 0..3 {
            pos_min[axis] = pos_min[axis].min(vertex.pos[axis]);
            pos_max[axis] = pos_max[axis].max(vertex.pos[axis]);
        }
        for axis in 0..2 {
            uv_min[axis] = uv_min[axis].min(vertex.uv[axis]);
            uv_max[axis] = uv_max[axis].max(vertex.uv[axis]);
        }
    }
    if !any {
        return None;
    }
    let pos_extent = [
        (pos_max[0] - pos_min[0]).max(1.0e-9),
        (pos_max[1] - pos_min[1]).max(1.0e-9),
        (pos_max[2] - pos_min[2]).max(1.0e-9),
        0.0,
    ];
    Some(PackedLodParamGpu {
        pos_min: [pos_min[0], pos_min[1], pos_min[2], 0.0],
        pos_extent,
        uv_min_extent: [
            uv_min[0],
            uv_min[1],
            (uv_max[0] - uv_min[0]).max(1.0e-9),
            (uv_max[1] - uv_min[1]).max(1.0e-9),
        ],
    })
}

fn pack_packed_lod_vertex(vertex: &MeshVertex, param: &PackedLodParamGpu) -> PackedRigidLodVertex {
    PackedRigidLodVertex {
        pos: [
            pack_unorm16_local(vertex.pos[0], param.pos_min[0], param.pos_extent[0]),
            pack_unorm16_local(vertex.pos[1], param.pos_min[1], param.pos_extent[1]),
            pack_unorm16_local(vertex.pos[2], param.pos_min[2], param.pos_extent[2]),
            0,
        ],
        normal: pack_normal_snorm8x4(vertex.normal),
        uv: [
            pack_unorm16_local(vertex.uv[0], param.uv_min_extent[0], param.uv_min_extent[2]),
            pack_unorm16_local(vertex.uv[1], param.uv_min_extent[1], param.uv_min_extent[3]),
        ],
        paint_uv: vertex.paint_uv,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CUSTOM_MATERIAL_TEXTURE_SLOT_BASE, MATERIAL_TEXTURE_NONE, MaterialTextureKey,
        bounded_growth_capacity, linear_material_texture_slot, material_texture_is_linear,
        material_texture_key_survives_slot_evict, material_texture_source_slot,
    };
    use perro_render_bridge::StandardMaterial3D;

    #[test]
    fn buffer_growth_stops_at_device_limit() {
        assert_eq!(
            bounded_growth_capacity(2_139_648, 4_000_000, 4_194_304),
            Some(4_194_304)
        );
        assert_eq!(
            bounded_growth_capacity(2_139_648, 4_194_305, 4_194_304),
            None
        );
    }

    #[test]
    fn material_texture_slot_evict_targets_matching_keys() {
        let slot = CUSTOM_MATERIAL_TEXTURE_SLOT_BASE + 3;
        let other_slot = CUSTOM_MATERIAL_TEXTURE_SLOT_BASE + 4;
        let affected = MaterialTextureKey {
            slots: [
                MATERIAL_TEXTURE_NONE,
                MATERIAL_TEXTURE_NONE,
                MATERIAL_TEXTURE_NONE,
                MATERIAL_TEXTURE_NONE,
                slot,
                MATERIAL_TEXTURE_NONE,
                MATERIAL_TEXTURE_NONE,
                MATERIAL_TEXTURE_NONE,
                MATERIAL_TEXTURE_NONE,
            ],
            standard: false,
        };
        let unaffected = MaterialTextureKey {
            slots: [
                MATERIAL_TEXTURE_NONE,
                MATERIAL_TEXTURE_NONE,
                MATERIAL_TEXTURE_NONE,
                MATERIAL_TEXTURE_NONE,
                other_slot,
                MATERIAL_TEXTURE_NONE,
                MATERIAL_TEXTURE_NONE,
                MATERIAL_TEXTURE_NONE,
                MATERIAL_TEXTURE_NONE,
            ],
            standard: false,
        };

        assert!(!material_texture_key_survives_slot_evict(&affected, slot));
        assert!(material_texture_key_survives_slot_evict(&unaffected, slot));
    }

    #[test]
    fn standard_texture_key_marks_only_gltf_data_maps_linear() {
        let material = StandardMaterial3D {
            base_color_texture: 10,
            metallic_roughness_texture: 11,
            normal_texture: 12,
            occlusion_texture: 13,
            emissive_texture: 14,
            ..Default::default()
        };
        let key = MaterialTextureKey::from_standard(&material);

        assert!(key.standard);
        assert_eq!(key.slots[0], 10);
        assert_eq!(key.slots[1], linear_material_texture_slot(11));
        assert_eq!(key.slots[2], linear_material_texture_slot(12));
        assert_eq!(key.slots[3], linear_material_texture_slot(13));
        assert_eq!(key.slots[4], 14);
        assert!(!material_texture_is_linear(key.slots[0]));
        assert!(material_texture_is_linear(key.slots[1]));
        assert!(material_texture_is_linear(key.slots[2]));
        assert!(material_texture_is_linear(key.slots[3]));
        assert!(!material_texture_is_linear(key.slots[4]));
        assert_eq!(material_texture_source_slot(key.slots[2]), 12);
    }
}
