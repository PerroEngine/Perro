use crate::cns::NodeArena;
use ahash::{AHashMap, AHashSet};
use perro_ids::NodeID;
use perro_ids::TagID;
use perro_nodes::{NodeType, SceneNode};
use perro_runtime_context::sub_apis::{QueryExpr, QueryScope, TagQuery};
use std::time::Instant;

const PARALLEL_MIN_NODES: usize = 10_000;
const PARALLEL_MIN_WORK_UNITS: u64 = 250_000;

pub(super) fn query_node_ids(
    arena: &NodeArena,
    query: TagQuery,
    tag_index: Option<&AHashMap<TagID, AHashSet<NodeID>>>,
) -> Vec<NodeID> {
    query_node_ids_with_worker_override(arena, query, None, tag_index)
}

fn query_node_ids_with_worker_override(
    arena: &NodeArena,
    query: TagQuery,
    worker_override: Option<usize>,
    tag_index: Option<&AHashMap<TagID, AHashSet<NodeID>>>,
) -> Vec<NodeID> {
    let start = Instant::now();
    let slot_count = arena.slot_count();
    if slot_count <= 1 {
        print_query_timing(
            &query,
            0,
            slot_count,
            start.elapsed().as_secs_f64() * 1_000_000.0,
        );
        return Vec::new();
    }

    let plan = QueryPlan::from_query(&query.expr);
    if plan.exact_type_mask == 0 || plan.base_type_mask == 0 {
        print_query_timing(
            &query,
            0,
            slot_count,
            start.elapsed().as_secs_f64() * 1_000_000.0,
        );
        return Vec::new();
    }
    let out = match query.scope {
        QueryScope::Root => {
            if let Some(candidates) = candidate_ids_from_tag_index(&query.expr, tag_index) {
                scan_candidates(arena, candidates, &plan)
            } else {
                let worker_count = worker_override.unwrap_or_else(|| {
                    recommended_workers(slot_count, plan.estimated_cost_per_node)
                });
                if worker_count <= 1 {
                    scan_range(arena, 1, slot_count, &plan)
                } else {
                    let chunk_size = slot_count.div_ceil(worker_count);
                    std::thread::scope(|scope| {
                        let mut handles = Vec::with_capacity(worker_count);
                        for start in (1..slot_count).step_by(chunk_size) {
                            let end = (start + chunk_size).min(slot_count);
                            let plan_ref = &plan;
                            handles
                                .push(scope.spawn(move || scan_range(arena, start, end, plan_ref)));
                        }
                        let mut out = Vec::new();
                        for handle in handles {
                            if let Ok(mut local) = handle.join() {
                                out.append(&mut local);
                            }
                        }
                        out
                    })
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

    print_query_timing(
        &query,
        out.len(),
        slot_count,
        start.elapsed().as_secs_f64() * 1_000_000.0,
    );
    out
}

fn candidate_ids_from_tag_index<'a>(
    expr: &'a Option<QueryExpr>,
    tag_index: Option<&'a AHashMap<TagID, AHashSet<NodeID>>>,
) -> Option<Vec<NodeID>> {
    let query_expr = expr.as_ref()?;
    let index = tag_index?;
    let required = required_all_tags_root(query_expr)?;
    if required.is_empty() {
        return None;
    }

    let mut sets: Vec<&AHashSet<NodeID>> = Vec::with_capacity(required.len());
    for tag in required {
        let set = index.get(&tag)?;
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
    Some(out)
}

fn required_all_tags_root(expr: &QueryExpr) -> Option<Vec<TagID>> {
    match expr {
        QueryExpr::All(children) => {
            for child in children {
                if let QueryExpr::Tags(tags) = child
                    && !tags.is_empty()
                {
                    return Some(tags.clone());
                }
            }
            None
        }
        _ => None,
    }
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
        QueryExpr::Tags(tags) => tags.iter().any(|tag| node.tags_slice().contains(tag)),
        QueryExpr::IsType(types) => types.contains(&node.node_type()),
        QueryExpr::BaseType(base_types) => base_types
            .iter()
            .any(|base_type| node.node_type().is_a(*base_type)),
    }
}

#[derive(Clone, Copy)]
enum TagClauseContext {
    Any,
    All,
}

fn eval_expr_in_context(expr: &QueryExpr, node: &SceneNode, tag_ctx: TagClauseContext) -> bool {
    match expr {
        QueryExpr::Tags(tags) => match tag_ctx {
            TagClauseContext::Any => tags.iter().any(|tag| node.tags_slice().contains(tag)),
            TagClauseContext::All => tags.iter().all(|tag| node.tags_slice().contains(tag)),
        },
        _ => eval_expr(expr, node),
    }
}

fn eval_not_expr(expr: &QueryExpr, node: &SceneNode) -> bool {
    match expr {
        QueryExpr::Tags(tags) => !tags.iter().any(|tag| node.tags_slice().contains(tag)),
        _ => !eval_expr(expr, node),
    }
}

struct QueryPlan {
    optimized_expr: Option<QueryExpr>,
    estimated_cost_per_node: u32,
    exact_type_mask: u64,
    base_type_mask: u64,
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
    fn type_in_mask(&self, node_type: NodeType, mask: u64) -> bool {
        let bit = 1_u64 << (node_type as u8);
        (mask & bit) != 0
    }
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
    }
}

fn expr_cost(expr: &QueryExpr) -> u32 {
    match expr {
        QueryExpr::IsType(_) => 1,
        QueryExpr::BaseType(_) => 2,
        QueryExpr::Name(names) => 4 + names.len() as u32,
        QueryExpr::Tags(tags) => 8 + (tags.len() as u32 * 2),
        QueryExpr::Not(inner) => 1 + expr_cost(inner),
        QueryExpr::All(children) | QueryExpr::Any(children) => {
            2 + children.iter().map(expr_cost).sum::<u32>()
        }
    }
}

fn all_types_mask() -> u64 {
    let mut mask = 0_u64;
    for &ty in NodeType::ALL {
        mask |= 1_u64 << (ty as u8);
    }
    mask
}

fn mask_from_types(kind: TypeFilterKind, types: &[NodeType]) -> u64 {
    match kind {
        TypeFilterKind::Exact => {
            let mut mask = 0_u64;
            for &ty in types {
                mask |= 1_u64 << (ty as u8);
            }
            mask
        }
        TypeFilterKind::Base => {
            if types.is_empty() {
                return all_types_mask();
            }
            let mut mask = 0_u64;
            for &ty in NodeType::ALL {
                if types.iter().any(|base| ty.is_a(*base)) {
                    mask |= 1_u64 << (ty as u8);
                }
            }
            mask
        }
    }
}

fn allowed_type_mask(expr: Option<&QueryExpr>, kind: TypeFilterKind) -> u64 {
    let Some(expr) = expr else {
        return all_types_mask();
    };
    allowed_type_mask_inner(expr, kind)
}

fn allowed_type_mask_inner(expr: &QueryExpr, kind: TypeFilterKind) -> u64 {
    match expr {
        QueryExpr::All(children) => children.iter().fold(all_types_mask(), |acc, child| {
            acc & allowed_type_mask_inner(child, kind)
        }),
        QueryExpr::Any(children) => children.iter().fold(0_u64, |acc, child| {
            acc | allowed_type_mask_inner(child, kind)
        }),
        QueryExpr::Not(_) => all_types_mask(),
        QueryExpr::Name(_) | QueryExpr::Tags(_) => all_types_mask(),
        QueryExpr::IsType(types) => match kind {
            TypeFilterKind::Exact => mask_from_types(TypeFilterKind::Exact, types),
            TypeFilterKind::Base => all_types_mask(),
        },
        QueryExpr::BaseType(types) => match kind {
            TypeFilterKind::Exact => all_types_mask(),
            TypeFilterKind::Base => mask_from_types(TypeFilterKind::Base, types),
        },
    }
}

fn print_query_timing(query: &TagQuery, matches: usize, slot_count: usize, elapsed_us: f64) {
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
