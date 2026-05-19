use crate::cns::NodeArena;
use ahash::{AHashMap, AHashSet};
use perro_ids::NodeID;
use perro_ids::TagID;
use perro_nodes::{Node2D, Node3D, NodeType, SceneNode};
use perro_runtime_api::sub_apis::{NodeQueryView, QueryExpr, QueryScope, QueryTypeMask};
use perro_structs::BitMask;
use rayon::prelude::*;
#[cfg(feature = "profile")]
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(feature = "profile")]
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

const PARALLEL_MIN_NODES: usize = 10_000;
const PARALLEL_MIN_WORK_UNITS: u64 = 30_000;

pub(super) fn query_node_ids(
    arena: &NodeArena,
    query: NodeQueryView<'_>,
    tag_index: Option<&AHashMap<TagID, AHashSet<NodeID>>>,
) -> Vec<NodeID> {
    query_node_ids_with_worker_override(arena, query, None, tag_index)
}

fn query_node_ids_with_worker_override(
    arena: &NodeArena,
    query: NodeQueryView<'_>,
    worker_override: Option<usize>,
    tag_index: Option<&AHashMap<TagID, AHashSet<NodeID>>>,
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
            if let Some(candidates) = candidate_ids_from_index(query.expr, tag_index, slot_count) {
                if candidates.exact {
                    candidates.ids
                } else {
                    scan_candidates(arena, candidates.ids, &plan)
                }
            } else {
                let worker_count = worker_override.unwrap_or_else(|| {
                    recommended_workers(slot_count, plan.estimated_cost_per_node)
                });
                if worker_count <= 1 {
                    scan_range(arena, 1, slot_count, &plan)
                } else {
                    let chunk_size = slot_count.div_ceil(worker_count);
                    let mut ranges = Vec::with_capacity(worker_count);
                    for start in (1..slot_count).step_by(chunk_size) {
                        let end = (start + chunk_size).min(slot_count);
                        ranges.push((start, end));
                    }
                    let mut partials = ranges
                        .into_par_iter()
                        .map(|(start, end)| scan_range(arena, start, end, &plan))
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
                scan_subtree(arena, root_id, &plan)
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

struct QueryCandidates {
    ids: Vec<NodeID>,
    exact: bool,
}

fn candidate_ids_from_index<'a>(
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
        | QueryExpr::Mask(_) => None,
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
    let mut ids = Vec::new();
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

    let mut out = Vec::new();
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
    let rest = candidates.collect::<Vec<_>>();
    if rest.is_empty() {
        return seed.clone();
    }

    let bit_words = slot_count.max(1).div_ceil(64);
    let rest_marks = rest
        .iter()
        .map(|candidate| {
            let mut marks = vec![0u64; bit_words];
            for &id in candidate.iter() {
                mark_id(id, &mut marks);
            }
            marks
        })
        .collect::<Vec<_>>();

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

fn scan_range(arena: &NodeArena, start: usize, end: usize, plan: &QueryPlan) -> Vec<NodeID> {
    let mut out = Vec::with_capacity((end.saturating_sub(start)) / 4);
    for index in start..end {
        let Some((id, node)) = arena.slot_get(index) else {
            continue;
        };
        if matches_query(node, plan) {
            out.push(id);
        }
    }
    out
}

fn scan_subtree(arena: &NodeArena, root_id: NodeID, plan: &QueryPlan) -> Vec<NodeID> {
    let mut out = Vec::new();
    let mut stack = vec![root_id];
    while let Some(id) = stack.pop() {
        let Some(node) = arena.get(id) else {
            continue;
        };
        if matches_query(node, plan) {
            out.push(id);
        }
        stack.extend(node.children_slice().iter().copied());
    }
    out
}

fn scan_candidates(arena: &NodeArena, candidates: Vec<NodeID>, plan: &QueryPlan) -> Vec<NodeID> {
    let mut out = Vec::with_capacity(candidates.len());
    for id in candidates {
        let Some(node) = arena.get(id) else {
            continue;
        };
        if matches_query(node, plan) {
            out.push(id);
        }
    }
    out
}

fn matches_query(node: &SceneNode, plan: &QueryPlan) -> bool {
    let node_type = node.node_type();
    if !plan.type_in_mask(node_type, plan.exact_type_mask) {
        return false;
    }

    if !plan.type_in_mask(node_type, plan.base_type_mask) {
        return false;
    }

    match &plan.optimized_expr {
        Some(expr) => eval_expr(expr, node),
        None => true,
    }
}

fn eval_expr(expr: &QueryExpr, node: &SceneNode) -> bool {
    match expr {
        QueryExpr::All(children) => children
            .iter()
            .all(|child| eval_expr_in_context(child, node, TagClauseContext::All)),
        QueryExpr::Any(children) => children
            .iter()
            .any(|child| eval_expr_in_context(child, node, TagClauseContext::Any)),
        QueryExpr::Not(inner) => eval_not_expr(inner, node),
        QueryExpr::Name(names) => names.iter().any(|name| node.get_name() == name),
        QueryExpr::Tags(tags) => tags.iter().any(|tag| node.has_tag(*tag)),
        QueryExpr::IsType(types) => types.contains(&node.node_type()),
        QueryExpr::BaseType(base_types) => base_types
            .iter()
            .any(|base_type| node.node_type().is_a(*base_type)),
        QueryExpr::IsTypeMask(mask) | QueryExpr::BaseTypeMask(mask) => {
            type_in_mask(node.node_type(), *mask)
        }
        QueryExpr::Layers(mask) => {
            node_render_layers(node).is_some_and(|layers| layers.intersects(*mask))
        }
        QueryExpr::Mask(mask) => {
            node_render_layers(node).is_none_or(|layers| !layers.intersects(*mask))
        }
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

fn eval_expr_in_context(expr: &QueryExpr, node: &SceneNode, tag_ctx: TagClauseContext) -> bool {
    match expr {
        QueryExpr::Tags(tags) => match tag_ctx {
            TagClauseContext::Any => tags.iter().any(|tag| node.has_tag(*tag)),
            TagClauseContext::All => tags.iter().all(|tag| node.has_tag(*tag)),
        },
        _ => eval_expr(expr, node),
    }
}

fn eval_not_expr(expr: &QueryExpr, node: &SceneNode) -> bool {
    match expr {
        QueryExpr::Tags(tags) => !tags.iter().any(|tag| node.has_tag(*tag)),
        _ => !eval_expr(expr, node),
    }
}

struct QueryPlan {
    optimized_expr: Option<QueryExpr>,
    estimated_cost_per_node: u32,
    exact_type_mask: QueryTypeMask,
    base_type_mask: QueryTypeMask,
}

impl QueryPlan {
    fn from_query(expr: &Option<QueryExpr>) -> Self {
        let optimized_expr = expr.as_ref().map(optimize_expr);
        let estimated_cost_per_node = optimized_expr.as_ref().map(expr_cost).unwrap_or(1);
        let exact_type_mask = allowed_type_mask(optimized_expr.as_ref(), TypeFilterKind::Exact);
        let base_type_mask = allowed_type_mask(optimized_expr.as_ref(), TypeFilterKind::Base);
        Self {
            optimized_expr,
            estimated_cost_per_node,
            exact_type_mask,
            base_type_mask,
        }
    }

    #[inline]
    fn type_in_mask(&self, node_type: NodeType, mask: QueryTypeMask) -> bool {
        type_in_mask(node_type, mask)
    }
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
    }
}

fn expr_cost(expr: &QueryExpr) -> u32 {
    match expr {
        QueryExpr::IsTypeMask(_) => 0,
        QueryExpr::IsType(_) => 1,
        QueryExpr::BaseTypeMask(_) => 1,
        QueryExpr::BaseType(_) => 2,
        QueryExpr::Layers(_) | QueryExpr::Mask(_) => 3,
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
        QueryExpr::Name(_) | QueryExpr::Tags(_) | QueryExpr::Layers(_) | QueryExpr::Mask(_) => None,
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
