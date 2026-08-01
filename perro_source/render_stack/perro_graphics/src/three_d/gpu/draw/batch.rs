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
        MaterialPipelineKind::StandardVariant(features)
        | MaterialPipelineKind::UnlitVariant(features)
        | MaterialPipelineKind::ToonVariant(features) => features.bits() as u32,
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
        MaterialPipelineKind::Standard | MaterialPipelineKind::StandardVariant(_) => 0,
        MaterialPipelineKind::Unlit | MaterialPipelineKind::UnlitVariant(_) => 1,
        MaterialPipelineKind::Toon | MaterialPipelineKind::ToonVariant(_) => 2,
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
        MaterialPipelineKind::StandardVariant(features) => (features.bits() as u64) << 9,
        MaterialPipelineKind::UnlitVariant(features)
        | MaterialPipelineKind::ToonVariant(features) => (features.bits() as u64) << 9,
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

/// Which animation lanes of an otherwise-identical draw carry new values.
///
/// Both lanes are laid out row-per-joint / row-per-weight in staging vectors
/// whose row counts are fixed by the draw's topology, so "same shape, new
/// numbers" is patchable in place: no restage, no batch sort, no compaction.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(in super::super) struct AnimationDelta {
    pub skeleton: bool,
    pub blend_weights: bool,
}

impl AnimationDelta {
    #[inline]
    pub(in super::super) fn any(self) -> bool {
        self.skeleton || self.blend_weights
    }
}

/// `Some(changed)` when both draws agree on skeleton *shape* (both unskinned,
/// or both skinned with the same joint count), `None` when the shape itself
/// moved and the palette rows have to be restaged.
///
/// A differing `Arc` is reported as changed without a deep compare: the patch
/// step compares against the staged rows it is about to overwrite anyway, so a
/// producer that reallocates identical values still uploads nothing.
#[inline]
fn skeleton_animation_delta(
    a: Option<&perro_render_bridge::SkeletonPalette>,
    b: Option<&perro_render_bridge::SkeletonPalette>,
) -> Option<bool> {
    match (a, b) {
        (None, None) => Some(false),
        (Some(a), Some(b)) => {
            if a.matrices.len() != b.matrices.len() {
                return None;
            }
            // An empty palette stages no rows, so it can never be a delta.
            Some(!a.matrices.is_empty() && !Arc::ptr_eq(&a.matrices, &b.matrices))
        }
        _ => None,
    }
}

/// Same contract as [`skeleton_animation_delta`] for the blend-shape weights:
/// the staged weight run of an instance is `min(len, mesh target count)` long,
/// so an equal-length lane keeps every row start and count in place.
#[inline]
fn blend_weights_delta(a: &Arc<[f32]>, b: &Arc<[f32]>) -> Option<bool> {
    if a.len() != b.len() {
        return None;
    }
    // An empty lane stages no rows, and the retained producer commonly hands
    // out a fresh empty `Arc` per frame; that must not read as a delta.
    Some(!a.is_empty() && !Arc::ptr_eq(a, b))
}

/// `Some(delta)` when `a`/`b` are the same draw except possibly its model row
/// and the animation lanes named by `delta`; `None` when anything structural
/// (topology, material, instance count, joint count, weight count) differs.
#[inline]
pub(in super::super) fn same_draw_except_model_and_animation(
    a: &Draw3DInstance,
    b: &Draw3DInstance,
) -> Option<AnimationDelta> {
    let same_shape = a.node == b.node
        && a.kind == b.kind
        && a.surfaces == b.surfaces
        && a.dense_multimesh == b.dense_multimesh
        && a.meshlet_override == b.meshlet_override
        && a.lod == b.lod
        && a.blend == b.blend
        && a.cast_shadows == b.cast_shadows
        && a.receive_shadows == b.receive_shadows;
    if !same_shape {
        return None;
    }
    Some(AnimationDelta {
        skeleton: skeleton_animation_delta(a.skeleton.as_ref(), b.skeleton.as_ref())?,
        blend_weights: blend_weights_delta(&a.blend_shape_weights, &b.blend_shape_weights)?,
    })
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

/// Per-draw shape classification for the transform-only fast path.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in super::super) enum TransformOnlyDrawKind {
    /// Single-instance regular draw; only its model row may differ.
    RegularSingle,
    /// Dense multimesh w/ unchanged poses; only its node_model may differ.
    Multimesh,
}

/// One draw's full fast-path classification: its shape plus which animation
/// lanes changed. A frame is free to mix the two (a walking character moves
/// AND re-poses), so the patch step handles the union.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in super::super) struct TransformOnlyDrawClass {
    pub kind: TransformOnlyDrawKind,
    pub anim: AnimationDelta,
}

impl TransformOnlyDrawClass {
    #[inline]
    pub(in super::super) fn transform(kind: TransformOnlyDrawKind) -> Self {
        Self {
            kind,
            anim: AnimationDelta::default(),
        }
    }
}

/// Classify one draw pair for the transform-only path, or `None` if the pair
/// forces a full rebuild (topology/material/instance-count change).
#[inline]
pub(in super::super) fn classify_transform_only_draw(
    prev: &Draw3DInstance,
    next: &Draw3DInstance,
) -> Option<TransformOnlyDrawClass> {
    if next.dense_multimesh.is_some() {
        if same_multimesh_except_node_model(prev, next) {
            return Some(TransformOnlyDrawClass::transform(
                TransformOnlyDrawKind::Multimesh,
            ));
        }
        return None;
    }
    // Multimesh cannot flip to a regular draw and stay transform-only.
    if prev.dense_multimesh.is_some() {
        return None;
    }
    if prev.instance_mats.len() != 1 || next.instance_mats.len() != 1 {
        return None;
    }
    let anim = same_draw_except_model_and_animation(prev, next)?;
    Some(TransformOnlyDrawClass {
        kind: TransformOnlyDrawKind::RegularSingle,
        anim,
    })
}

/// Whole-scene decision: every draw pair must classify, and at least one draw
/// must actually be present. Returns per-draw classes when the transform-only
/// path is valid.
pub(in super::super) fn classify_transform_only_scene(
    prev: &[Draw3DInstance],
    next: &[Draw3DInstance],
    out: &mut Vec<TransformOnlyDrawClass>,
) -> bool {
    out.clear();
    if prev.len() != next.len() || next.is_empty() {
        return false;
    }
    for (p, n) in prev.iter().zip(next.iter()) {
        match classify_transform_only_draw(p, n) {
            Some(class) => out.push(class),
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
