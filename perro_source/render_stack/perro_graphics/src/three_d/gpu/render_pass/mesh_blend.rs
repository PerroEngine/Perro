use super::*;

// True when any instance of `batch` sits inside a patched span. `spans` is
// sorted + disjoint, but `instance_start` is NOT monotonic across batches
// (compaction can repoint a later batch at an earlier region), so binary-search
// per batch instead of sweeping.
pub(super) fn batch_overlaps_dirty_spans(batch: &DrawBatch, spans: &[Range<u32>]) -> bool {
    let start = batch.instance_start;
    let end = start.saturating_add(batch.instance_count);
    if start >= end {
        return false;
    }
    let candidate = spans.partition_point(|span| span.end <= start);
    spans.get(candidate).is_some_and(|span| span.start < end)
}

// What the per-source blend loop knows about the depth currently sitting in
// `mesh_blend_depth_view`. The receiver depth a source needs is a pure function
// of the batch list rendered into that texture, and nothing between two source
// passes writes it, so a source whose list matches what is already resident can
// skip its own depth pass entirely.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum BlendDepthResident {
    // Contents not attributable to a known batch list: every source renders.
    Unknown,
    // Output of this frame's global mesh-blend depth pass: the batches in
    // `mesh_blend_depth_batch_indices`, same Clear(1.0), same depth pipelines.
    Global,
    // Output of an earlier source's depth pass: this range of
    // `mesh_blend_receiver_indices`.
    Receivers(Range<usize>),
}

// True when the resident depth is exactly what re-rendering `receivers` would
// produce. Both lists are built in ascending batch order, so equal sets are
// equal slices; the compare is element-wise (no hashing), so a hit is exact
// rather than probable. O(receivers) against a whole render pass.
pub(super) fn blend_depth_resident_matches(
    resident: &BlendDepthResident,
    receiver_indices: &[usize],
    global_indices: &[usize],
    receivers: Range<usize>,
) -> bool {
    match resident {
        BlendDepthResident::Unknown => false,
        BlendDepthResident::Global => receiver_indices[receivers] == *global_indices,
        BlendDepthResident::Receivers(prev) => {
            receiver_indices[prev.clone()] == receiver_indices[receivers]
        }
    }
}

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
