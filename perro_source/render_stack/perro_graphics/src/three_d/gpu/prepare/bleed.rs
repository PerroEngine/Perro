use super::*;

pub(in super::super) struct BleedEmitter {
    batch_index: usize,
    center: Vec3,
    radius_sq: f32,
    color: Vec3,
}

pub(in super::super) struct BleedOccluder {
    batch_index: usize,
    center: Vec3,
    radius: f32,
}

#[inline]
pub(super) fn unpack_unorm8_lane(packed: u32, shift: u32) -> f32 {
    ((packed >> shift) & 0xff) as f32 / 255.0
}

// Bit layout matches decode_local_bleed in the prelude WGSL:
// r5 g5 b5 strength5 oct_x6 oct_y6.
#[inline]
pub(super) fn pack_local_bleed(color: Vec3, strength: f32, dir: Vec3) -> u32 {
    #[inline]
    fn quant5(v: f32) -> u32 {
        (v.clamp(0.0, 1.0) * 31.0 + 0.5) as u32
    }
    #[inline]
    fn quant6(v: f32) -> u32 {
        (v.clamp(0.0, 1.0) * 63.0 + 0.5) as u32
    }
    let d = dir.normalize_or_zero();
    let sum = (d.x.abs() + d.y.abs() + d.z.abs()).max(1.0e-6);
    let mut ox = d.x / sum;
    let mut oy = d.y / sum;
    if d.z < 0.0 {
        let old_x = ox;
        ox = (1.0 - oy.abs()) * old_x.signum();
        oy = (1.0 - old_x.abs()) * oy.signum();
    }
    quant5(color.x)
        | (quant5(color.y) << 5)
        | (quant5(color.z) << 10)
        | (quant5(strength) << 15)
        | (quant6(ox * 0.5 + 0.5) << 20)
        | (quant6(oy * 0.5 + 0.5) << 26)
}

// Multimesh emitters index past regular draw batches to stay unique.
pub(super) const BLEED_MULTIMESH_INDEX_BASE: usize = 1 << 20;

// Shared gather: weighted sum of visible emitters around a receiver center.
// Returns the packed tint or None when nothing contributes.
pub(super) fn gather_local_bleed(
    center: Vec3,
    self_index: usize,
    emitters: &[BleedEmitter],
    occluders: &[BleedOccluder],
) -> Option<u32> {
    let mut sum = Vec3::ZERO;
    let mut sum_dir = Vec3::ZERO;
    let mut total_w = 0.0f32;
    for emitter in emitters {
        if emitter.batch_index == self_index {
            continue;
        }
        let d_sq = emitter.center.distance_squared(center);
        let range_fade = (1.0 - d_sq / (BLEED_RANGE * BLEED_RANGE)).clamp(0.0, 1.0);
        if range_fade <= 0.0 {
            continue;
        }
        let mut w = (emitter.radius_sq / (d_sq + emitter.radius_sq)) * range_fade;
        if bleed_segment_occluded(
            center,
            emitter.center,
            occluders,
            self_index,
            emitter.batch_index,
        ) {
            w *= BLEED_OCCLUDED_FACTOR;
        }
        sum += emitter.color * w;
        sum_dir += (emitter.center - center).normalize_or_zero() * w;
        total_w += w;
    }
    if total_w <= 1.0e-3 {
        return None;
    }
    let tint = (sum / total_w).clamp(Vec3::ZERO, Vec3::ONE);
    Some(pack_local_bleed(
        tint,
        total_w.clamp(0.0, 1.0),
        sum_dir.normalize_or_zero(),
    ))
}

// Approximate world bounds of a multimesh draw from sampled instances.
pub(in super::super) fn multimesh_world_bounds(
    batch: &MultiMeshBatch,
    draw_params: &[MultiMeshDrawParamGpu],
    instances: &[MultiMeshInstanceGpu],
) -> Option<(Vec3, f32)> {
    let param = draw_params.get(batch.draw_param_index as usize)?;
    let cols = [
        [
            param.model_row_0[0],
            param.model_row_1[0],
            param.model_row_2[0],
            0.0,
        ],
        [
            param.model_row_0[1],
            param.model_row_1[1],
            param.model_row_2[1],
            0.0,
        ],
        [
            param.model_row_0[2],
            param.model_row_1[2],
            param.model_row_2[2],
            0.0,
        ],
        [
            param.model_row_0[3],
            param.model_row_1[3],
            param.model_row_2[3],
            1.0,
        ],
    ];
    let model = Mat4::from_cols_array_2d(&cols);
    if !model.is_finite() {
        return None;
    }
    let start = batch.instance_start as usize;
    let end = (start + batch.instance_count as usize).min(instances.len());
    if end <= start {
        return None;
    }
    let count = end - start;
    let step = (count / 64).max(1);
    let mut sum = Vec3::ZERO;
    // step>=2 whenever count>=128, so an integer-floored step leaves step=1 only
    // for count<128; the sampled count never exceeds 127. A fixed stack array
    // holds every sample with no heap alloc; the length guard is defensive.
    let mut samples = [Vec3::ZERO; 128];
    let mut sample_count = 0usize;
    let mut i = start;
    while i < end && sample_count < samples.len() {
        let world = (model * Vec3::from(instances[i].position).extend(1.0)).truncate();
        if world.is_finite() {
            sum += world;
            samples[sample_count] = world;
            sample_count += 1;
        }
        i += step;
    }
    if sample_count == 0 {
        return None;
    }
    let center = sum / sample_count as f32;
    let radius = samples[..sample_count]
        .iter()
        .map(|p| p.distance(center))
        .fold(0.0f32, f32::max)
        + 1.0;
    Some((center, radius.clamp(0.5, 100.0)))
}

// True when a third batch sphere blocks the segment between two centers.
#[inline]
pub(super) fn bleed_segment_occluded(
    from: Vec3,
    to: Vec3,
    occluders: &[BleedOccluder],
    skip_a: usize,
    skip_b: usize,
) -> bool {
    let seg = to - from;
    let len_sq = seg.length_squared();
    if len_sq <= 1.0e-6 {
        return false;
    }
    for occ in occluders {
        if occ.batch_index == skip_a || occ.batch_index == skip_b {
            continue;
        }
        let t = ((occ.center - from).dot(seg) / len_sq).clamp(0.0, 1.0);
        // Endpoints touching the occluder are contact, not blockage.
        if t <= 0.05 || t >= 0.95 {
            continue;
        }
        let closest = from + seg * t;
        let r = occ.radius * BLEED_OCCLUDER_SHRINK;
        if occ.center.distance_squared(closest) < r * r {
            return true;
        }
    }
    false
}

impl Gpu3D {
    // Approximate one-bounce GI: tint each standard-material batch with the
    // distance-weighted albedo/emissive of nearby batches. The tint rides in
    // packed_pbr_params_1 (free unless mesh blend owns it) and the
    // MATERIAL_FLAG_LOCAL_BLEED bit tells the shader the lane is valid.
    pub(super) fn apply_local_color_bleed(&mut self) {
        if self.draw_batches.len() > BLEED_MAX_BATCHES {
            return;
        }
        let mut emitters = std::mem::take(&mut self.bleed_emitters_scratch);
        emitters.clear();
        let mut occluders = std::mem::take(&mut self.bleed_occluders_scratch);
        occluders.clear();
        for (batch_index, batch) in self.draw_batches.iter().enumerate() {
            if emitters.len() >= BLEED_MAX_EMITTERS {
                break;
            }
            // Multi-instance batches stay out of the bleed emitter/occluder set:
            // their first-instance transform does not stand in for the whole
            // batch, and pre-merge behavior excluded them via the 1e9 sentinel.
            if batch.draw_on_top || batch.instance_count != 1 || batch.local_radius >= 1.0e8 {
                continue;
            }
            let Some(inst) = self
                .staged_instance_transforms
                .get(batch.instance_start as usize)
            else {
                continue;
            };
            let model = Mat4::from_cols_array_2d(&model_cols_from_affine_rows(inst));
            if !model.is_finite() {
                continue;
            }
            let center = (model * Vec3::from(batch.local_center).extend(1.0)).truncate();
            if !center.is_finite() {
                continue;
            }
            let sx = model.x_axis.truncate().length();
            let sy = model.y_axis.truncate().length();
            let sz = model.z_axis.truncate().length();
            let radius = (batch.local_radius.max(0.0) * sx.max(sy).max(sz)).clamp(0.05, 50.0);
            if occluders.len() < BLEED_MAX_OCCLUDERS && radius >= 0.75 && batch.alpha_mode != 2 {
                occluders.push(BleedOccluder {
                    batch_index,
                    center,
                    radius,
                });
            }
            let Some(meta) = self
                .staged_rigid_instance_meta
                .get(batch.instance_start as usize)
            else {
                continue;
            };
            let packed = meta.material.packed_color;
            let albedo = Vec3::new(
                unpack_unorm8_lane(packed, 0),
                unpack_unorm8_lane(packed, 8),
                unpack_unorm8_lane(packed, 16),
            );
            let em = meta.material.packed_emissive;
            let em_scale = unpack_unorm8_lane(em, 24) * 16.0;
            let emissive = Vec3::new(
                unpack_unorm8_lane(em, 0),
                unpack_unorm8_lane(em, 8),
                unpack_unorm8_lane(em, 16),
            ) * em_scale;
            let color = albedo * 0.8 + emissive;
            if color.max_element() <= 1.0e-3 {
                continue;
            }
            emitters.push(BleedEmitter {
                batch_index,
                center,
                radius_sq: radius * radius,
                color,
            });
        }
        // Multimesh draws join as emitters too (grass fields tint neighbors).
        let mut multimesh_bounds = std::mem::take(&mut self.bleed_multimesh_bounds_scratch);
        multimesh_bounds.clear();
        multimesh_bounds.reserve(self.multimesh_batches.len());
        for (mm_index, batch) in self.multimesh_batches.iter().enumerate() {
            let bounds = multimesh_world_bounds(
                batch,
                &self.staged_multimesh_draw_params,
                &self.staged_multimesh_instances,
            );
            if let Some((center, radius)) = bounds
                && emitters.len() < BLEED_MAX_EMITTERS
                && let Some(param) = self
                    .staged_multimesh_draw_params
                    .get(batch.draw_param_index as usize)
            {
                let albedo = Vec3::new(
                    unpack_unorm8_lane(param.packed_color, 0),
                    unpack_unorm8_lane(param.packed_color, 8),
                    unpack_unorm8_lane(param.packed_color, 16),
                );
                let em_scale = unpack_unorm8_lane(param.packed_emissive, 24) * 16.0;
                let emissive = Vec3::new(
                    unpack_unorm8_lane(param.packed_emissive, 0),
                    unpack_unorm8_lane(param.packed_emissive, 8),
                    unpack_unorm8_lane(param.packed_emissive, 16),
                ) * em_scale;
                let color = albedo * 0.8 + emissive;
                if color.max_element() > 1.0e-3 {
                    emitters.push(BleedEmitter {
                        batch_index: BLEED_MULTIMESH_INDEX_BASE + mm_index,
                        center,
                        radius_sq: radius * radius,
                        color,
                    });
                }
            }
            multimesh_bounds.push(bounds);
        }
        if emitters.is_empty() {
            self.bleed_emitters_scratch = emitters;
            self.bleed_occluders_scratch = occluders;
            self.bleed_multimesh_bounds_scratch = multimesh_bounds;
            return;
        }
        for batch_index in 0..self.draw_batches.len() {
            let (instance_start, instance_count, local_center) = {
                let batch = &self.draw_batches[batch_index];
                if batch.draw_on_top
                    || batch.mesh_blend
                    || batch.instance_count == 0
                    || matches!(batch.material_kind, MaterialPipelineKind::Unlit)
                {
                    continue;
                }
                (
                    batch.instance_start as usize,
                    batch.instance_count as usize,
                    batch.local_center,
                )
            };
            let Some(inst) = self.staged_instance_transforms.get(instance_start) else {
                continue;
            };
            let model = Mat4::from_cols_array_2d(&model_cols_from_affine_rows(inst));
            if !model.is_finite() {
                continue;
            }
            let center = (model * Vec3::from(local_center).extend(1.0)).truncate();
            if !center.is_finite() {
                continue;
            }
            let Some(packed) = gather_local_bleed(center, batch_index, &emitters, &occluders)
            else {
                continue;
            };
            let end = instance_start + instance_count;
            for meta in self
                .staged_rigid_instance_meta
                .get_mut(instance_start..end)
                .unwrap_or(&mut [])
            {
                meta.material.packed_pbr_params_1 = packed;
                meta.material.packed_material_params |= MATERIAL_FLAG_LOCAL_BLEED << 3;
            }
            for meta in self
                .staged_skinned_instance_meta
                .get_mut(instance_start..end)
                .unwrap_or(&mut [])
            {
                meta.material.packed_pbr_params_1 = packed;
                meta.material.packed_material_params |= MATERIAL_FLAG_LOCAL_BLEED << 3;
            }
        }
        // Multimesh receivers: one tint per draw param, read in the vertex
        // stage since instances share the draw's material.
        for (mm_index, bounds) in multimesh_bounds.iter().enumerate() {
            let Some((center, _)) = *bounds else {
                continue;
            };
            let (draw_param_index, unlit) = {
                let batch = &self.multimesh_batches[mm_index];
                (
                    batch.draw_param_index as usize,
                    matches!(batch.material_kind, MaterialPipelineKind::Unlit),
                )
            };
            if unlit {
                continue;
            }
            let self_index = BLEED_MULTIMESH_INDEX_BASE + mm_index;
            let Some(packed) = gather_local_bleed(center, self_index, &emitters, &occluders) else {
                continue;
            };
            if let Some(param) = self.staged_multimesh_draw_params.get_mut(draw_param_index) {
                param.packed_bleed = packed;
            }
        }
        self.bleed_emitters_scratch = emitters;
        self.bleed_occluders_scratch = occluders;
        self.bleed_multimesh_bounds_scratch = multimesh_bounds;
    }
}

#[cfg(test)]
mod tests {
    use super::builtin_flat_mesh_double_sided;
    use super::{
        BleedOccluder, Vec3, bleed_segment_occluded, pack_local_bleed, prepare_fast_path_eligible,
    };

    #[test]
    fn resource_dirty_forces_full_prepare_path() {
        assert!(!prepare_fast_path_eligible(true, true));
        assert!(prepare_fast_path_eligible(false, true));
    }

    #[test]
    fn local_bleed_pack_lanes_match_shader_layout() {
        let packed = pack_local_bleed(Vec3::new(1.0, 0.5, 0.0), 0.5, Vec3::Y);
        assert_eq!(packed & 0x1f, 31, "r lane");
        assert_eq!((packed >> 5) & 0x1f, 16, "g lane");
        assert_eq!((packed >> 10) & 0x1f, 0, "b lane");
        assert_eq!((packed >> 15) & 0x1f, 16, "strength lane");
        // +Y maps to octahedral (0, 1) -> quantized (32, 63).
        assert_eq!((packed >> 20) & 0x3f, 32, "oct x");
        assert_eq!((packed >> 26) & 0x3f, 63, "oct y");
    }

    #[test]
    fn bleed_occlusion_blocks_midpoint_sphere_only() {
        let occ = [BleedOccluder {
            batch_index: 7,
            center: Vec3::new(0.0, 0.0, 5.0),
            radius: 1.5,
        }];
        let a = Vec3::ZERO;
        let b = Vec3::new(0.0, 0.0, 10.0);
        assert!(bleed_segment_occluded(a, b, &occ, 0, 1));
        // Skipped when the occluder is one of the endpoints' batches.
        assert!(!bleed_segment_occluded(a, b, &occ, 7, 1));
        // Off-axis sphere does not block.
        let off = [BleedOccluder {
            batch_index: 7,
            center: Vec3::new(4.0, 0.0, 5.0),
            radius: 1.5,
        }];
        assert!(!bleed_segment_occluded(a, b, &off, 0, 1));
    }

    #[test]
    fn flat_builtin_meshes_default_double_sided() {
        assert!(builtin_flat_mesh_double_sided(
            perro_builtin_meshes::PLANE_SOURCE
        ));
        assert!(builtin_flat_mesh_double_sided(
            perro_builtin_meshes::QUAD_SOURCE
        ));
        assert!(!builtin_flat_mesh_double_sided(
            perro_builtin_meshes::CUBE_SOURCE
        ));
    }
}
