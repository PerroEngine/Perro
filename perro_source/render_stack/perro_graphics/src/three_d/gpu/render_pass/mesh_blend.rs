use super::*;

pub(super) fn mesh_blend_relevant_sphere_changed(
    batches: &[DrawBatch],
    sources: &[usize],
    prev: &[Option<(Vec3, f32)>],
    cur: &[Option<(Vec3, f32)>],
) -> bool {
    for &source_i in sources {
        if prev.get(source_i) != cur.get(source_i) {
            return true;
        }
    }
    for (i, batch) in batches.iter().enumerate() {
        // Skip batches excluded as receiver targets (mesh_blend_receiver_matches).
        if batch.draw_on_top || batch.alpha_mode != 0 || batch.mesh_blend {
            continue;
        }
        if prev.get(i) != cur.get(i) {
            return true;
        }
    }
    false
}

pub(super) fn mesh_blend_receiver_matches(
    source_index: usize,
    source: &DrawBatch,
    source_sphere: Option<(Vec3, f32)>,
    target_index: usize,
    target: &DrawBatch,
    target_sphere: Option<(Vec3, f32)>,
) -> bool {
    if source_index == target_index
        || target.draw_on_top
        || target.alpha_mode != 0
        || target.mesh_blend
    {
        return false;
    }
    let source_accepts_target = target.blend_layers & !source.blend_mask != 0;
    let target_accepts_source = source.blend_layers & !target.blend_mask != 0;
    if !source_accepts_target || !target_accepts_source {
        return false;
    }
    mesh_blend_batches_overlap(source_sphere, target_sphere)
}

// Per-layer caster cull: keep only shadow batches whose world sphere touches
// this light's frustum, preserving draw order. Multi-instance batches use the
// merged sphere over all instances; batches with no usable bound survive.
