use super::*;

#[inline]
pub(in super::super) fn compare_draw_batch_keys(a: &DrawBatch, b: &DrawBatch) -> Ordering {
    a.render_state
        .batch_kind
        .cmp(&b.render_state.batch_kind)
        .then_with(|| match a.render_state.batch_kind {
            RenderBatchKind::Opaque => a
                .render_state
                .pipeline_key
                .cmp(&b.render_state.pipeline_key)
                .then_with(|| {
                    a.render_state
                        .texture_slot
                        .cmp(&b.render_state.texture_slot)
                })
                .then_with(|| {
                    a.render_state
                        .mesh_index_start
                        .cmp(&b.render_state.mesh_index_start)
                })
                .then_with(|| {
                    a.render_state
                        .mesh_base_vertex
                        .cmp(&b.render_state.mesh_base_vertex)
                })
                .then_with(|| a.instance_start.cmp(&b.instance_start)),
            RenderBatchKind::Alpha | RenderBatchKind::MeshBlend | RenderBatchKind::Overlay => a
                .order_index
                .cmp(&b.order_index)
                .then_with(|| a.instance_start.cmp(&b.instance_start)),
        })
}

#[inline]
pub(in super::super) fn draw_batches_sorted(batches: &[DrawBatch]) -> bool {
    batches
        .windows(2)
        .all(|pair| compare_draw_batch_keys(&pair[0], &pair[1]) != Ordering::Greater)
}

#[inline]
pub(in super::super) fn multimesh_batch_sort_key(
    batch: &MultiMeshBatch,
) -> (bool, bool, bool, bool, u8, u32, u64, u32, u32) {
    let custom_token = match batch.material_kind {
        MaterialPipelineKind::Custom(token) => token,
        _ => 0,
    };
    (
        batch.mesh_blend,
        batch.mesh_blend_screen,
        batch.casts_shadows,
        batch.double_sided,
        material_pipeline_kind_rank(&batch.material_kind),
        custom_token,
        batch.material_texture_key.state_hash(),
        batch.mesh.index_start,
        batch.draw_param_index,
    )
}

#[inline]
pub(in super::super) fn multimesh_batches_sorted(batches: &[MultiMeshBatch]) -> bool {
    batches
        .windows(2)
        .all(|pair| multimesh_batch_sort_key(&pair[0]) <= multimesh_batch_sort_key(&pair[1]))
}

#[inline]
pub(in super::super) fn material_pipeline_kind_rank(kind: &MaterialPipelineKind) -> u8 {
    match kind {
        MaterialPipelineKind::Standard => 0,
        MaterialPipelineKind::Unlit => 1,
        MaterialPipelineKind::Toon => 2,
        MaterialPipelineKind::Custom(_) => 3,
    }
}

#[inline]
pub(in super::super) fn draw_batch_state_key(
    path: RenderPath3D,
    draw_on_top: bool,
    double_sided: bool,
    alpha_mode: u8,
    packed_lod: bool,
    material_kind: &MaterialPipelineKind,
) -> u64 {
    let path_bits = match path {
        RenderPath3D::Rigid => 0u64,
        RenderPath3D::Skinned => 1u64,
        RenderPath3D::MultiMesh => 2u64,
    };
    let top_bits = u64::from(draw_on_top) << 1;
    let sided_bits = u64::from(double_sided) << 2;
    let alpha_bits = u64::from(alpha_mode == 2) << 3;
    let rank_bits = (material_pipeline_kind_rank(material_kind) as u64) << 4;
    let packed_bits = u64::from(packed_lod) << 8;
    let custom_bits = match material_kind {
        MaterialPipelineKind::Custom(token) => (*token as u64) << 9,
        _ => 0u64,
    };
    path_bits | top_bits | sided_bits | alpha_bits | rank_bits | packed_bits | custom_bits
}

#[inline]
pub(in super::super) fn render_state_key(
    pipeline_key: u64,
    texture_slot: u64,
    mesh_index_start: u32,
    mesh_base_vertex: i32,
    draw_on_top: bool,
    alpha_mode: u8,
    mesh_blend: bool,
) -> RenderStateKey {
    let batch_kind = if draw_on_top {
        RenderBatchKind::Overlay
    } else if mesh_blend {
        RenderBatchKind::MeshBlend
    } else if alpha_mode != 0 {
        RenderBatchKind::Alpha
    } else {
        RenderBatchKind::Opaque
    };
    RenderStateKey {
        pipeline_key,
        texture_slot,
        mesh_index_start,
        mesh_base_vertex,
        batch_kind,
    }
}

#[inline]
pub(in super::super) fn same_draw_except_model(a: &Draw3DInstance, b: &Draw3DInstance) -> bool {
    a.node == b.node
        && a.kind == b.kind
        && a.surfaces == b.surfaces
        && a.skeleton == b.skeleton
        && a.blend_shape_weights == b.blend_shape_weights
        && a.dense_multimesh == b.dense_multimesh
        && a.meshlet_override == b.meshlet_override
        && a.lod == b.lod
        && a.blend == b.blend
        && a.cast_shadows == b.cast_shadows
        && a.receive_shadows == b.receive_shadows
}

/// Cheap identity check for a dense multimesh's instance pose list. The retained
/// producer reuses the same `Arc` when poses do not change, so `Arc::ptr_eq`
/// hits the fast path; the deep compare stays as a correctness fallback.
#[inline]
pub(in super::super) fn same_dense_instances(
    a: &DenseMultiMeshDraw3D,
    b: &DenseMultiMeshDraw3D,
) -> bool {
    a.instance_scale == b.instance_scale
        && (Arc::ptr_eq(&a.instances, &b.instances) || a.instances == b.instances)
}

/// True when `a`/`b` are the same multimesh draw except possibly `node_model`.
/// Instances (poses + scale) and material/blend must be unchanged; only the
/// draw's world transform may differ. Such a draw is patchable in the
/// transform-only path (instances are relative to the draw model in the shader,
/// so only the `MultiMeshDrawParamGpu` model rows need a rewrite).
#[inline]
pub(in super::super) fn same_multimesh_except_node_model(
    a: &Draw3DInstance,
    b: &Draw3DInstance,
) -> bool {
    let (Some(dense_a), Some(dense_b)) = (a.dense_multimesh.as_ref(), b.dense_multimesh.as_ref())
    else {
        return false;
    };
    a.node == b.node
        && a.kind == b.kind
        && a.surfaces == b.surfaces
        && a.skeleton == b.skeleton
        && a.blend_shape_weights == b.blend_shape_weights
        && a.meshlet_override == b.meshlet_override
        && a.lod == b.lod
        && a.blend == b.blend
        && a.cast_shadows == b.cast_shadows
        && a.receive_shadows == b.receive_shadows
        && same_dense_instances(dense_a, dense_b)
}

/// Per-draw classification for the transform-only fast path.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in super::super) enum TransformOnlyDrawKind {
    /// Single-instance regular draw; only its model row may differ.
    RegularSingle,
    /// Dense multimesh w/ unchanged poses; only its node_model may differ.
    Multimesh,
}

/// Classify one draw pair for the transform-only path, or `None` if the pair
/// forces a full rebuild (topology/material/instance-count change).
#[inline]
pub(in super::super) fn classify_transform_only_draw(
    prev: &Draw3DInstance,
    next: &Draw3DInstance,
) -> Option<TransformOnlyDrawKind> {
    if next.dense_multimesh.is_some() {
        if same_multimesh_except_node_model(prev, next) {
            return Some(TransformOnlyDrawKind::Multimesh);
        }
        return None;
    }
    // Multimesh cannot flip to a regular draw and stay transform-only.
    if prev.dense_multimesh.is_some() {
        return None;
    }
    if prev.instance_mats.len() == 1
        && next.instance_mats.len() == 1
        && same_draw_except_model(prev, next)
    {
        return Some(TransformOnlyDrawKind::RegularSingle);
    }
    None
}

/// Whole-scene decision: every draw pair must classify, and at least one draw
/// must actually be present. Returns per-draw kinds when the transform-only
/// path is valid.
pub(in super::super) fn classify_transform_only_scene(
    prev: &[Draw3DInstance],
    next: &[Draw3DInstance],
    out: &mut Vec<TransformOnlyDrawKind>,
) -> bool {
    out.clear();
    if prev.len() != next.len() || next.is_empty() {
        return false;
    }
    for (p, n) in prev.iter().zip(next.iter()) {
        match classify_transform_only_draw(p, n) {
            Some(kind) => out.push(kind),
            None => {
                out.clear();
                return false;
            }
        }
    }
    true
}

#[inline]
pub(in super::super) fn draws_semantically_unchanged(
    prev_revision: u64,
    next_revision: u64,
    prev: &[Draw3DInstance],
    next: &[Draw3DInstance],
) -> bool {
    prev_revision == next_revision || (prev_revision != u64::MAX && prev == next)
}

// Depth-safety of one batch for the shared depth-only shaders (shadow depth +
// depth prepass + mesh-blend depth). Built-in materials replicate exactly:
// standard vertex transforms, plus the mode-1 base-texture cutout discard.
// A custom material is replicated only when it has NO shade_vertex hook (the
// hook's displacement never runs in the depth-only vertex stage) AND is fully
// opaque (a custom fragment's alpha can diverge from the base-texture cutout
// the shared mode-1 depth shaders apply). Tokens without a recorded hook flag
// (pipeline not ensured yet) stay excluded, matching the old conservative
// behavior.
pub(in super::super) fn batch_depth_safe(
    batch: &DrawBatch,
    custom_vertex_hooks: &AHashMap<u32, bool>,
) -> bool {
    match &batch.material_kind {
        MaterialPipelineKind::Custom(token) => {
            batch.alpha_mode == 0 && custom_vertex_hooks.get(token).copied() == Some(false)
        }
        _ => true,
    }
}

// Shadow-caster gate for one rigid/skinned batch (membership in
// shadow_batch_indices).
pub(in super::super) fn batch_casts_into_shadow_map(
    batch: &DrawBatch,
    custom_vertex_hooks: &AHashMap<u32, bool>,
) -> bool {
    batch_depth_safe(batch, custom_vertex_hooks)
        && !batch.draw_on_top
        && batch.casts_shadows
        && batch.alpha_mode != 2
}
