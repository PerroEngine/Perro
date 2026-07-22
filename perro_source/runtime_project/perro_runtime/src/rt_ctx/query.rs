use crate::cns::NodeArena;
use ahash::{AHashMap, AHashSet};
use perro_ids::NodeID;
use perro_ids::TagID;
use perro_nodes::{Node2D, Node3D, NodeType, SceneNode};
use perro_runtime_api::sub_apis::{
    NodeQueryView, QueryBounds, QueryExpr, QueryScope, QueryTypeMask,
};
use perro_structs::{BitMask, Vector2, Vector3};
use rayon::prelude::*;
#[cfg(feature = "profile")]
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(feature = "profile")]
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

const PARALLEL_MIN_NODES: usize = 10_000;
const PARALLEL_MIN_WORK_UNITS: u64 = 30_000;

/// Slot-indexed snapshot of global node positions, built only when a query
/// contains a [`QueryExpr::Within`] clause.
pub(crate) struct QuerySpatialIndex {
    pub pos_2d: Vec<Option<Vector2>>,
    pub pos_3d: Vec<Option<Vector3>>,
}

// Only exercised directly by tests now; the live call sites in `nodes.rs`
// hoist candidate computation and go through `query_node_ids_with_candidates`.
#[cfg(test)]
pub(super) fn query_node_ids(
    arena: &NodeArena,
    query: NodeQueryView<'_>,
    spatial: Option<&QuerySpatialIndex>,
    tag_index: Option<&AHashMap<TagID, AHashSet<NodeID>>>,
) -> Vec<NodeID> {
    query_node_ids_with_worker_override(arena, query, spatial, None, tag_index, None)
}

/// Same as [`query_node_ids`] but reuses a candidate set the caller already
/// computed (e.g. to size the spatial-index fill), so the tag-index
/// intersection doesn't run twice per query.
pub(super) fn query_node_ids_with_candidates(
    arena: &NodeArena,
    query: NodeQueryView<'_>,
    spatial: Option<&QuerySpatialIndex>,
    tag_index: Option<&AHashMap<TagID, AHashSet<NodeID>>>,
    candidates: Option<QueryCandidates>,
) -> Vec<NodeID> {
    query_node_ids_with_worker_override(
        arena,
        query,
        spatial,
        None,
        tag_index,
        Some(PrecomputedCandidates { candidates }),
    )
}

// Only exercised directly by tests now; see `query_node_ids` above.
#[cfg(test)]
pub(super) fn query_first_node_id(
    arena: &NodeArena,
    query: NodeQueryView<'_>,
    spatial: Option<&QuerySpatialIndex>,
    tag_index: Option<&AHashMap<TagID, AHashSet<NodeID>>>,
) -> Option<NodeID> {
    query_first_node_id_with_candidates(arena, query, spatial, tag_index, None)
}

/// Same as [`query_first_node_id`] but reuses a candidate set the caller
/// already computed, mirroring [`query_node_ids_with_candidates`].
pub(super) fn query_first_node_id_with_candidates(
    arena: &NodeArena,
    query: NodeQueryView<'_>,
    spatial: Option<&QuerySpatialIndex>,
    tag_index: Option<&AHashMap<TagID, AHashSet<NodeID>>>,
    precomputed: Option<Option<QueryCandidates>>,
) -> Option<NodeID> {
    let slot_count = arena.slot_count();
    if slot_count <= 1 {
        return None;
    }

    let plan = QueryPlan::from_query(query.expr);
    if plan.exact_type_mask.is_empty() || plan.base_type_mask.is_empty() {
        return None;
    }

    match query.scope {
        QueryScope::Root => {
            let candidates = match precomputed {
                Some(candidates) => candidates,
                None => candidate_ids_from_index(query.expr, tag_index, slot_count),
            };
            if let Some(candidates) = candidates {
                if candidates.exact {
                    candidates.ids.into_iter().next()
                } else {
                    first_in_candidates(arena, candidates.ids, &plan, spatial)
                }
            } else {
                first_in_range(arena, 1, slot_count, &plan, spatial)
            }
        }
        QueryScope::Subtree(root_id) => {
            if root_id.is_nil() {
                None
            } else {
                first_in_subtree(arena, root_id, &plan, spatial)
            }
        }
    }
}

/// Precomputed candidate set handed in from the caller (e.g. `nodes.rs`,
/// which also uses it to restrict the spatial-index fill). Passing it in
/// avoids recomputing the same tag-index intersection twice per query.
pub(super) struct PrecomputedCandidates {
    pub(super) candidates: Option<QueryCandidates>,
}

fn query_node_ids_with_worker_override(
    arena: &NodeArena,
    query: NodeQueryView<'_>,
    spatial: Option<&QuerySpatialIndex>,
    worker_override: Option<usize>,
    tag_index: Option<&AHashMap<TagID, AHashSet<NodeID>>>,
    precomputed: Option<PrecomputedCandidates>,
) -> Vec<NodeID> {
    #[cfg(feature = "profile")]
    let start = Instant::now();
    let slot_count = arena.slot_count();
    if slot_count <= 1 {
        #[cfg(feature = "profile")]
        {
            print_query_timing(
                query,
                0,
                slot_count,
                start.elapsed().as_secs_f64() * 1_000_000.0,
            );
        }
        return Vec::new();
    }

    let plan = QueryPlan::from_query(query.expr);
    if plan.exact_type_mask.is_empty() || plan.base_type_mask.is_empty() {
        #[cfg(feature = "profile")]
        {
            print_query_timing(
                query,
                0,
                slot_count,
                start.elapsed().as_secs_f64() * 1_000_000.0,
            );
        }
        return Vec::new();
    }
    let out = match query.scope {
        QueryScope::Root => {
            let candidates = match precomputed {
                Some(precomputed) => precomputed.candidates,
                None => candidate_ids_from_index(query.expr, tag_index, slot_count),
            };
            if let Some(candidates) = candidates {
                if candidates.exact {
                    candidates.ids
                } else {
                    scan_candidates(arena, candidates.ids, &plan, spatial)
                }
            } else {
                let worker_count = worker_override.unwrap_or_else(|| {
                    recommended_workers(slot_count, plan.estimated_cost_per_node)
                });
                if worker_count <= 1 {
                    scan_range(arena, 1, slot_count, &plan, spatial)
                } else {
                    let chunk_size = slot_count.div_ceil(worker_count);
                    let mut ranges = Vec::with_capacity(worker_count);
                    for start in (1..slot_count).step_by(chunk_size) {
                        let end = (start + chunk_size).min(slot_count);
                        ranges.push((start, end));
                    }
                    let mut partials = ranges
                        .into_par_iter()
                        .map(|(start, end)| scan_range(arena, start, end, &plan, spatial))
                        .collect::<Vec<_>>();
                    let total: usize = partials.iter().map(Vec::len).sum();
                    let mut out = Vec::with_capacity(total);
                    for mut local in partials.drain(..) {
                        out.append(&mut local);
                    }
                    out
                }
            }
        }
        QueryScope::Subtree(root_id) => {
            if root_id.is_nil() {
                Vec::new()
            } else {
                scan_subtree(arena, root_id, &plan, spatial)
            }
        }
    };

    #[cfg(feature = "profile")]
    {
        print_query_timing(
            query,
            out.len(),
            slot_count,
            start.elapsed().as_secs_f64() * 1_000_000.0,
        );
    }
    out
}

pub(super) struct QueryCandidates {
    pub(super) ids: Vec<NodeID>,
    pub(super) exact: bool,
}

pub(super) fn candidate_ids_from_index<'a>(
    expr: &'a Option<QueryExpr>,
    tag_index: Option<&'a AHashMap<TagID, AHashSet<NodeID>>>,
    slot_count: usize,
) -> Option<QueryCandidates> {
    let query_expr = expr.as_ref()?;
    let index = tag_index?;
    candidate_ids_for_expr(query_expr, TagClauseContext::Any, index, slot_count)
}

fn candidate_ids_for_expr(
    expr: &QueryExpr,
    tag_ctx: TagClauseContext,
    tag_index: &AHashMap<TagID, AHashSet<NodeID>>,
    slot_count: usize,
) -> Option<QueryCandidates> {
    match expr {
        QueryExpr::Tags(tags) => match tag_ctx {
            TagClauseContext::All => Some(QueryCandidates {
                ids: tag_intersection_candidates(tags, tag_index),
                exact: true,
            }),
            TagClauseContext::Any => Some(QueryCandidates {
                ids: tag_union_candidates(tags, tag_index, slot_count),
                exact: true,
            }),
        },
        QueryExpr::All(children) => candidate_ids_for_all(children, tag_index, slot_count),
        QueryExpr::Any(children) => candidate_ids_for_any(children, tag_index, slot_count),
        QueryExpr::Not(_)
        | QueryExpr::Name(_)
        | QueryExpr::IsType(_)
        | QueryExpr::BaseType(_)
        | QueryExpr::IsTypeMask(_)
        | QueryExpr::BaseTypeMask(_)
        | QueryExpr::Layers(_)
        | QueryExpr::Mask(_)
        | QueryExpr::Within(_) => None,
    }
}

fn candidate_ids_for_all(
    children: &[QueryExpr],
    tag_index: &AHashMap<TagID, AHashSet<NodeID>>,
    slot_count: usize,
) -> Option<QueryCandidates> {
    if children.is_empty() {
        return None;
    }

    let mut indexed = Vec::new();
    let mut all_children_indexed = true;
    for child in children {
        if let Some(candidates) =
            candidate_ids_for_expr(child, TagClauseContext::All, tag_index, slot_count)
        {
            indexed.push(candidates);
        } else {
            all_children_indexed = false;
        }
    }
    if indexed.is_empty() {
        return None;
    }

    indexed.sort_by_key(|candidates| candidates.ids.len());
    let ids =
        intersect_candidate_vectors(indexed.iter().map(|candidates| &candidates.ids), slot_count);
    Some(QueryCandidates {
        ids,
        exact: all_children_indexed && indexed.iter().all(|candidates| candidates.exact),
    })
}

fn candidate_ids_for_any(
    children: &[QueryExpr],
    tag_index: &AHashMap<TagID, AHashSet<NodeID>>,
    slot_count: usize,
) -> Option<QueryCandidates> {
    if children.is_empty() {
        return None;
    }

    let mut indexed = Vec::with_capacity(children.len());
    for child in children {
        indexed.push(candidate_ids_for_expr(
            child,
            TagClauseContext::Any,
            tag_index,
            slot_count,
        )?);
    }

    indexed.sort_by_key(|candidates| candidates.ids.len());
    let total_candidates = indexed.iter().fold(0usize, |sum, candidates| {
        sum.saturating_add(candidates.ids.len())
    });
    let mut ids = Vec::with_capacity(total_candidates.min(slot_count));
    let bit_words = slot_count.max(1).div_ceil(64);
    let mut marks = vec![0u64; bit_words];
    for candidates in &indexed {
        push_unique_ids(&candidates.ids, &mut marks, &mut ids);
    }
    Some(QueryCandidates {
        ids,
        exact: indexed.iter().all(|candidates| candidates.exact),
    })
}

fn tag_intersection_candidates(
    tags: &[TagID],
    tag_index: &AHashMap<TagID, AHashSet<NodeID>>,
) -> Vec<NodeID> {
    if tags.is_empty() {
        return Vec::new();
    }

    let mut sets = Vec::with_capacity(tags.len());
    for tag in tags {
        let Some(set) = tag_index.get(tag) else {
            return Vec::new();
        };
        sets.push(set);
    }
    sets.sort_by_key(|set| set.len());

    let mut out = Vec::new();
    if let Some(seed) = sets.first().copied() {
        out.reserve(seed.len());
        'outer: for &id in seed {
            for set in sets.iter().skip(1) {
                if !set.contains(&id) {
                    continue 'outer;
                }
            }
            out.push(id);
        }
    }
    out
}

fn tag_union_candidates(
    tags: &[TagID],
    tag_index: &AHashMap<TagID, AHashSet<NodeID>>,
    slot_count: usize,
) -> Vec<NodeID> {
    if tags.is_empty() {
        return Vec::new();
    }

    let mut sets = Vec::with_capacity(tags.len());
    for tag in tags {
        if let Some(set) = tag_index.get(tag) {
            sets.push(set);
        }
    }
    sets.sort_by_key(|set| set.len());

    let total_candidates = sets
        .iter()
        .fold(0usize, |sum, set| sum.saturating_add(set.len()));
    let mut out = Vec::with_capacity(total_candidates.min(slot_count));
    let bit_words = slot_count.max(1).div_ceil(64);
    let mut marks = vec![0u64; bit_words];
    for set in sets {
        for &id in set {
            push_unique_id(id, &mut marks, &mut out);
        }
    }
    out
}

fn intersect_candidate_vectors<'a>(
    mut candidates: impl Iterator<Item = &'a Vec<NodeID>>,
    slot_count: usize,
) -> Vec<NodeID> {
    let Some(seed) = candidates.next() else {
        return Vec::new();
    };
    let bit_words = slot_count.max(1).div_ceil(64);
    let rest_marks = candidates
        .map(|candidate| {
            let mut marks = vec![0u64; bit_words];
            for &id in candidate {
                mark_id(id, &mut marks);
            }
            marks
        })
        .collect::<Vec<_>>();
    if rest_marks.is_empty() {
        return seed.clone();
    }

    let mut out = Vec::with_capacity(seed.len());
    'outer: for &id in seed {
        for marks in &rest_marks {
            if !is_marked(id, marks) {
                continue 'outer;
            }
        }
        out.push(id);
    }
    out
}

fn push_unique_ids(ids: &[NodeID], marks: &mut [u64], out: &mut Vec<NodeID>) {
    for &id in ids {
        push_unique_id(id, marks, out);
    }
}

fn push_unique_id(id: NodeID, marks: &mut [u64], out: &mut Vec<NodeID>) {
    if is_marked(id, marks) {
        return;
    }
    mark_id(id, marks);
    out.push(id);
}

fn mark_id(id: NodeID, marks: &mut [u64]) {
    let slot = id.index() as usize;
    let word = slot / 64;
    if word >= marks.len() {
        return;
    }
    let bit = 1u64 << (slot & 63);
    marks[word] |= bit;
}

fn is_marked(id: NodeID, marks: &[u64]) -> bool {
    let slot = id.index() as usize;
    let word = slot / 64;
    if word >= marks.len() {
        return false;
    }
    let bit = 1u64 << (slot & 63);
    (marks[word] & bit) != 0
}

fn recommended_workers(total_nodes: usize, estimated_cost_per_node: u32) -> usize {
    if total_nodes < PARALLEL_MIN_NODES {
        return 1;
    }
    let estimated_work = total_nodes as u64 * estimated_cost_per_node as u64;
    if estimated_work < PARALLEL_MIN_WORK_UNITS {
        return 1;
    }

    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Cheap slot-lane type reject. The mirror value is always accurate for
/// occupied slots; free slots may hold a stale type, but a false pass just
/// falls through to `slot_get`/`get` which rejects on occupancy/generation.
/// Never falsely rejects a live node. Callers gate on `plan.has_type_filter`
/// so unconstrained queries skip the lane read entirely.
#[inline]
fn slot_type_rejects(node_type: NodeType, plan: &QueryPlan) -> bool {
    !type_in_mask(node_type, plan.exact_type_mask) || !type_in_mask(node_type, plan.base_type_mask)
}

fn scan_range(
    arena: &NodeArena,
    start: usize,
    end: usize,
    plan: &QueryPlan,
    spatial: Option<&QuerySpatialIndex>,
) -> Vec<NodeID> {
    let mut out = Vec::with_capacity((end.saturating_sub(start)) / 4);
    let types = arena.node_type_slots();
    let type_gate = plan.has_type_filter;
    // index feeds types[], slot_get, + matches_query; enumerate rewrite loses clarity
    #[allow(clippy::needless_range_loop)]
    for index in start..end {
        // Type lane runs first so rejected slots never touch the wide node Vec.
        if type_gate && slot_type_rejects(types[index], plan) {
            continue;
        }
        let Some((id, node)) = arena.slot_get(index) else {
            continue;
        };
        if matches_query(node, index, plan, spatial) {
            out.push(id);
        }
    }
    out
}

fn scan_subtree(
    arena: &NodeArena,
    root_id: NodeID,
    plan: &QueryPlan,
    spatial: Option<&QuerySpatialIndex>,
) -> Vec<NodeID> {
    let mut out = Vec::new();
    let mut stack = vec![root_id];
    let mut seen = AHashSet::default();
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        let Some(node) = arena.get(id) else {
            continue;
        };
        if matches_query(node, id.index() as usize, plan, spatial) {
            out.push(id);
        }
        if let Some(children) = arena.children(id) {
            stack.extend(children.iter().copied());
        }
    }
    out
}

fn scan_candidates(
    arena: &NodeArena,
    candidates: Vec<NodeID>,
    plan: &QueryPlan,
    spatial: Option<&QuerySpatialIndex>,
) -> Vec<NodeID> {
    let mut out = Vec::with_capacity(candidates.len());
    let type_gate = plan.has_type_filter;
    for id in candidates {
        if type_gate
            && arena
                .slot_node_type(id.index() as usize)
                .is_none_or(|node_type| slot_type_rejects(node_type, plan))
        {
            continue;
        }
        let Some(node) = arena.get(id) else {
            continue;
        };
        if matches_query(node, id.index() as usize, plan, spatial) {
            out.push(id);
        }
    }
    out
}

fn first_in_range(
    arena: &NodeArena,
    start: usize,
    end: usize,
    plan: &QueryPlan,
    spatial: Option<&QuerySpatialIndex>,
) -> Option<NodeID> {
    let types = arena.node_type_slots();
    let type_gate = plan.has_type_filter;
    // index feeds types[], slot_get, + matches_query; enumerate rewrite loses clarity
    #[allow(clippy::needless_range_loop)]
    for index in start..end {
        if type_gate && slot_type_rejects(types[index], plan) {
            continue;
        }
        let Some((id, node)) = arena.slot_get(index) else {
            continue;
        };
        if matches_query(node, index, plan, spatial) {
            return Some(id);
        }
    }
    None
}

fn first_in_subtree(
    arena: &NodeArena,
    root_id: NodeID,
    plan: &QueryPlan,
    spatial: Option<&QuerySpatialIndex>,
) -> Option<NodeID> {
    let mut stack = vec![root_id];
    let mut seen = AHashSet::default();
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        let Some(node) = arena.get(id) else {
            continue;
        };
        if matches_query(node, id.index() as usize, plan, spatial) {
            return Some(id);
        }
        if let Some(children) = arena.children(id) {
            stack.extend(children.iter().copied());
        }
    }
    None
}

fn first_in_candidates(
    arena: &NodeArena,
    candidates: Vec<NodeID>,
    plan: &QueryPlan,
    spatial: Option<&QuerySpatialIndex>,
) -> Option<NodeID> {
    let type_gate = plan.has_type_filter;
    for id in candidates {
        if type_gate
            && arena
                .slot_node_type(id.index() as usize)
                .is_none_or(|node_type| slot_type_rejects(node_type, plan))
        {
            continue;
        }
        let Some(node) = arena.get(id) else {
            continue;
        };
        if matches_query(node, id.index() as usize, plan, spatial) {
            return Some(id);
        }
    }
    None
}

fn matches_query(
    node: &SceneNode,
    slot: usize,
    plan: &QueryPlan,
    spatial: Option<&QuerySpatialIndex>,
) -> bool {
    let node_type = node.node_type();
    if !type_in_mask(node_type, plan.exact_type_mask) {
        return false;
    }

    if !type_in_mask(node_type, plan.base_type_mask) {
        return false;
    }

    match &plan.optimized_expr {
        Some(expr) => eval_expr_with_type(expr, node, node_type, slot, spatial),
        None => true,
    }
}

#[cfg(test)]
fn eval_expr(expr: &QueryExpr, node: &SceneNode) -> bool {
    eval_expr_with_type(expr, node, node.node_type(), 0, None)
}

fn eval_expr_with_type(
    expr: &QueryExpr,
    node: &SceneNode,
    node_type: NodeType,
    slot: usize,
    spatial: Option<&QuerySpatialIndex>,
) -> bool {
    match expr {
        QueryExpr::All(children) => children.iter().all(|child| {
            eval_expr_in_context(child, node, node_type, slot, spatial, TagClauseContext::All)
        }),
        QueryExpr::Any(children) => children.iter().any(|child| {
            eval_expr_in_context(child, node, node_type, slot, spatial, TagClauseContext::Any)
        }),
        QueryExpr::Not(inner) => eval_not_expr(inner, node, node_type, slot, spatial),
        QueryExpr::Name(names) => names.iter().any(|name| node.get_name() == name),
        QueryExpr::Tags(tags) => tags.iter().any(|tag| node.has_tag(*tag)),
        QueryExpr::IsType(types) => types.contains(&node_type),
        QueryExpr::BaseType(base_types) => base_types
            .iter()
            .any(|base_type| node_type.is_a(*base_type)),
        QueryExpr::IsTypeMask(mask) | QueryExpr::BaseTypeMask(mask) => {
            type_in_mask(node_type, *mask)
        }
        QueryExpr::Layers(mask) => {
            node_render_layers(node).is_some_and(|layers| layers.intersects(*mask))
        }
        QueryExpr::Mask(mask) => {
            node_render_layers(node).is_none_or(|layers| !layers.intersects(*mask))
        }
        QueryExpr::Within(bounds) => spatial.is_some_and(|index| match bounds {
            QueryBounds::Box2D { .. } => index
                .pos_2d
                .get(slot)
                .copied()
                .flatten()
                .is_some_and(|position| bounds.contains_2d(position)),
            QueryBounds::Box3D { .. } => index
                .pos_3d
                .get(slot)
                .copied()
                .flatten()
                .is_some_and(|position| bounds.contains_3d(position)),
        }),
    }
}

fn node_render_layers(node: &SceneNode) -> Option<BitMask> {
    node.with_base_ref::<Node2D, _>(|node| node.render_layers)
        .or_else(|| node.with_base_ref::<Node3D, _>(|node| node.render_layers))
}

#[derive(Clone, Copy)]
enum TagClauseContext {
    Any,
    All,
}

fn eval_expr_in_context(
    expr: &QueryExpr,
    node: &SceneNode,
    node_type: NodeType,
    slot: usize,
    spatial: Option<&QuerySpatialIndex>,
    tag_ctx: TagClauseContext,
) -> bool {
    match expr {
        QueryExpr::Tags(tags) => match tag_ctx {
            TagClauseContext::Any => tags.iter().any(|tag| node.has_tag(*tag)),
            TagClauseContext::All => tags.iter().all(|tag| node.has_tag(*tag)),
        },
        _ => eval_expr_with_type(expr, node, node_type, slot, spatial),
    }
}

fn eval_not_expr(
    expr: &QueryExpr,
    node: &SceneNode,
    node_type: NodeType,
    slot: usize,
    spatial: Option<&QuerySpatialIndex>,
) -> bool {
    match expr {
        QueryExpr::Tags(tags) => !tags.iter().any(|tag| node.has_tag(*tag)),
        _ => !eval_expr_with_type(expr, node, node_type, slot, spatial),
    }
}

struct QueryPlan {
    optimized_expr: Option<QueryExpr>,
    estimated_cost_per_node: u32,
    exact_type_mask: QueryTypeMask,
    base_type_mask: QueryTypeMask,
    /// True when either mask actually constrains types; unconstrained queries
    /// skip the slot type-lane pre-check (it would be pure overhead).
    has_type_filter: bool,
}

impl QueryPlan {
    fn from_query(expr: &Option<QueryExpr>) -> Self {
        // Masks + cost don't depend on child order, so compute frm the
        // original (borrowed) expr -- no need to wait on optimize's clone.
        let exact_type_mask = allowed_type_mask(expr.as_ref(), TypeFilterKind::Exact);
        let base_type_mask = allowed_type_mask(expr.as_ref(), TypeFilterKind::Base);
        // Reorder only possible 4 All/Any w/ >1 child; single/no-clause exprs
        // skip the recursive rebuild+sort entirely (was an unconditional
        // clone-heavy walk even when there was nothing to reorder).
        let optimized: Option<QueryExpr> = expr.as_ref().map(|e| {
            if expr_may_reorder(e) {
                optimize_expr(e)
            } else {
                e.clone()
            }
        });
        // Move (not clone) into strip; it hands the same value back untouched
        // when nothing gets stripped, so the common case pays 1 clone total
        // (the line above) instead of 2.
        let optimized_expr = optimized.and_then(strip_redundant_type_filters_owned);
        let estimated_cost_per_node = optimized_expr.as_ref().map(expr_cost).unwrap_or(1);
        let has_type_filter =
            exact_type_mask != QueryTypeMask::all() || base_type_mask != QueryTypeMask::all();
        Self {
            optimized_expr,
            estimated_cost_per_node,
            exact_type_mask,
            base_type_mask,
            has_type_filter,
        }
    }
}

/// True only when reordering could actually change eval order (All/Any w/
/// >1 child). Guards the expensive recursive rebuild+sort in `optimize_expr`.
#[inline]
fn expr_may_reorder(expr: &QueryExpr) -> bool {
    matches!(expr, QueryExpr::All(children) | QueryExpr::Any(children) if children.len() > 1)
}

/// Owned variant of the strip pass: consumes `expr` + hands it right back
/// when nothing needs stripping (the overwhelming common case), so the
/// caller pays 0 extra clones instead of 1. Only `All` children actually
/// ever get removed; every other shape is either dropped whole (all-filter)
/// or passed through unchanged w/o rebuild.
fn strip_redundant_type_filters_owned(expr: QueryExpr) -> Option<QueryExpr> {
    if type_filter_only(&expr) {
        return None;
    }

    match expr {
        QueryExpr::All(children) => {
            let mut stripped = Vec::with_capacity(children.len());
            for child in children {
                if let Some(child) = strip_redundant_type_filters_owned(child) {
                    stripped.push(child);
                }
            }
            match stripped.len() {
                0 => None,
                1 => stripped.pop(),
                _ => Some(QueryExpr::All(stripped)),
            }
        }
        // Do not strip mixed `Any` branches. A type filter inside one branch is
        // branch-local, not a global mask constraint.
        other => Some(other),
    }
}

fn type_filter_only(expr: &QueryExpr) -> bool {
    type_mask_only(expr, TypeFilterKind::Exact).is_some()
        || type_mask_only(expr, TypeFilterKind::Base).is_some()
}

#[inline]
fn type_in_mask(node_type: NodeType, mask: QueryTypeMask) -> bool {
    mask.contains_type(node_type)
}

#[derive(Clone, Copy)]
enum TypeFilterKind {
    Exact,
    Base,
}

fn optimize_expr(expr: &QueryExpr) -> QueryExpr {
    match expr {
        QueryExpr::All(children) => {
            let mut optimized: Vec<QueryExpr> = children.iter().map(optimize_expr).collect();
            optimized.sort_by_key(expr_cost);
            QueryExpr::All(optimized)
        }
        QueryExpr::Any(children) => {
            let mut optimized: Vec<QueryExpr> = children.iter().map(optimize_expr).collect();
            optimized.sort_by_key(expr_cost);
            QueryExpr::Any(optimized)
        }
        QueryExpr::Not(inner) => QueryExpr::Not(Box::new(optimize_expr(inner))),
        QueryExpr::Name(names) => QueryExpr::Name(names.clone()),
        QueryExpr::Tags(tags) => QueryExpr::Tags(tags.clone()),
        QueryExpr::IsType(types) => QueryExpr::IsType(types.clone()),
        QueryExpr::BaseType(types) => QueryExpr::BaseType(types.clone()),
        QueryExpr::IsTypeMask(mask) => QueryExpr::IsTypeMask(*mask),
        QueryExpr::BaseTypeMask(mask) => QueryExpr::BaseTypeMask(*mask),
        QueryExpr::Layers(mask) => QueryExpr::Layers(*mask),
        QueryExpr::Mask(mask) => QueryExpr::Mask(*mask),
        QueryExpr::Within(bounds) => QueryExpr::Within(*bounds),
    }
}

fn expr_cost(expr: &QueryExpr) -> u32 {
    match expr {
        QueryExpr::IsTypeMask(_) => 0,
        QueryExpr::IsType(_) => 1,
        QueryExpr::BaseTypeMask(_) => 1,
        QueryExpr::BaseType(_) => 2,
        QueryExpr::Layers(_) | QueryExpr::Mask(_) | QueryExpr::Within(_) => 3,
        QueryExpr::Name(names) => 4 + names.len() as u32,
        QueryExpr::Tags(tags) => 8 + (tags.len() as u32 * 2),
        QueryExpr::Not(inner) => 1 + expr_cost(inner),
        QueryExpr::All(children) | QueryExpr::Any(children) => {
            2 + children.iter().map(expr_cost).sum::<u32>()
        }
    }
}

fn all_types_mask() -> QueryTypeMask {
    QueryTypeMask::all()
}

fn mask_from_types(kind: TypeFilterKind, types: &[NodeType]) -> QueryTypeMask {
    match kind {
        TypeFilterKind::Exact => {
            let mut mask = QueryTypeMask::NONE;
            for &ty in types {
                mask = mask.with_type(ty);
            }
            mask
        }
        TypeFilterKind::Base => {
            if types.is_empty() {
                return all_types_mask();
            }
            let mut mask = QueryTypeMask::NONE;
            for &ty in NodeType::ALL {
                if types.iter().any(|base| ty.is_a(*base)) {
                    mask = mask.with_type(ty);
                }
            }
            mask
        }
    }
}

fn allowed_type_mask(expr: Option<&QueryExpr>, kind: TypeFilterKind) -> QueryTypeMask {
    let Some(expr) = expr else {
        return all_types_mask();
    };
    allowed_type_mask_inner(expr, kind)
}

fn allowed_type_mask_inner(expr: &QueryExpr, kind: TypeFilterKind) -> QueryTypeMask {
    match expr {
        QueryExpr::All(children) => children.iter().fold(all_types_mask(), |acc, child| {
            acc.intersection(allowed_type_mask_inner(child, kind))
        }),
        QueryExpr::Any(children) => children.iter().fold(QueryTypeMask::NONE, |acc, child| {
            acc.union(allowed_type_mask_inner(child, kind))
        }),
        QueryExpr::Not(inner) => match type_mask_only(inner, kind) {
            Some(mask) => mask.complement(),
            None => all_types_mask(),
        },
        QueryExpr::Name(_) | QueryExpr::Tags(_) | QueryExpr::Layers(_) | QueryExpr::Mask(_) => {
            all_types_mask()
        }
        // Spatial bounds can only match nodes of the matching dimensionality,
        // so a `Within` clause narrows the base-type mask for free.
        QueryExpr::Within(bounds) => match kind {
            TypeFilterKind::Exact => all_types_mask(),
            TypeFilterKind::Base => match bounds {
                QueryBounds::Box2D { .. } => {
                    mask_from_types(TypeFilterKind::Base, &[NodeType::Node2D])
                }
                QueryBounds::Box3D { .. } => {
                    mask_from_types(TypeFilterKind::Base, &[NodeType::Node3D])
                }
            },
        },
        QueryExpr::IsType(types) => match kind {
            TypeFilterKind::Exact => mask_from_types(TypeFilterKind::Exact, types),
            TypeFilterKind::Base => all_types_mask(),
        },
        QueryExpr::BaseType(types) => match kind {
            TypeFilterKind::Exact => all_types_mask(),
            TypeFilterKind::Base => mask_from_types(TypeFilterKind::Base, types),
        },
        QueryExpr::IsTypeMask(mask) => match kind {
            TypeFilterKind::Exact => *mask,
            TypeFilterKind::Base => all_types_mask(),
        },
        QueryExpr::BaseTypeMask(mask) => match kind {
            TypeFilterKind::Exact => all_types_mask(),
            TypeFilterKind::Base => *mask,
        },
    }
}

fn type_mask_only(expr: &QueryExpr, kind: TypeFilterKind) -> Option<QueryTypeMask> {
    match expr {
        QueryExpr::All(children) => {
            let mut mask = all_types_mask();
            for child in children {
                mask = mask.intersection(type_mask_only(child, kind)?);
            }
            Some(mask)
        }
        QueryExpr::Any(children) => {
            let mut mask = QueryTypeMask::NONE;
            for child in children {
                mask = mask.union(type_mask_only(child, kind)?);
            }
            Some(mask)
        }
        QueryExpr::Not(inner) => type_mask_only(inner, kind).map(QueryTypeMask::complement),
        QueryExpr::Name(_)
        | QueryExpr::Tags(_)
        | QueryExpr::Layers(_)
        | QueryExpr::Mask(_)
        | QueryExpr::Within(_) => None,
        QueryExpr::IsType(types) => match kind {
            TypeFilterKind::Exact => Some(mask_from_types(TypeFilterKind::Exact, types)),
            TypeFilterKind::Base => None,
        },
        QueryExpr::BaseType(types) => match kind {
            TypeFilterKind::Exact => None,
            TypeFilterKind::Base => Some(mask_from_types(TypeFilterKind::Base, types)),
        },
        QueryExpr::IsTypeMask(mask) => match kind {
            TypeFilterKind::Exact => Some(*mask),
            TypeFilterKind::Base => None,
        },
        QueryExpr::BaseTypeMask(mask) => match kind {
            TypeFilterKind::Exact => None,
            TypeFilterKind::Base => Some(*mask),
        },
    }
}

#[cfg(feature = "profile")]
fn print_query_timing(
    query: NodeQueryView<'_>,
    matches: usize,
    slot_count: usize,
    elapsed_us: f64,
) {
    #[cfg(not(debug_assertions))]
    {
        let _ = (query, matches, slot_count, elapsed_us);
        return;
    }

    #[cfg(debug_assertions)]
    {
        if std::env::var_os("PERRO_QUERY_TIMING").is_none() {
            return;
        }
        println!(
            "[node_query] {:.2}us matches={} slots={} scope={}",
            elapsed_us,
            matches,
            slot_count.saturating_sub(1),
            match query.scope {
                QueryScope::Root => "root",
                QueryScope::Subtree(_) => "subtree",
            },
        );
    }
}

#[cfg(test)]
#[path = "../../tests/unit/rt_ctx_query_tests.rs"]
mod tests;
