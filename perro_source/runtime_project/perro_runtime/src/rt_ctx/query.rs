use crate::cns::NodeArena;
use perro_ids::{NodeID, TagID};
use perro_nodes::NodeType;
use perro_runtime_context::sub_apis::TagQuery;
use std::time::Instant;

const PARALLEL_MIN_NODES: usize = 10_000;

pub(super) fn query_node_ids(arena: &NodeArena, query: TagQuery) -> Vec<NodeID> {
    let start = Instant::now();
    let slot_count = arena.slot_count();
    if slot_count <= 1 {
        print_query_timing(&query, 0, slot_count, start.elapsed().as_secs_f64() * 1_000_000.0);
        return Vec::new();
    }

    let plan = QueryPlan::from_query(&query);
    let worker_count = recommended_workers(slot_count);
    let out = if worker_count <= 1 {
        scan_range(arena, 1, slot_count, &query, &plan)
    } else {
        let chunk_size = slot_count.div_ceil(worker_count);
        std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(worker_count);
            for start in (1..slot_count).step_by(chunk_size) {
                let end = (start + chunk_size).min(slot_count);
                let query_ref = &query;
                let plan_ref = &plan;
                handles.push(scope.spawn(move || scan_range(arena, start, end, query_ref, plan_ref)));
            }
            let mut out = Vec::new();
            for handle in handles {
                if let Ok(mut local) = handle.join() {
                    out.append(&mut local);
                }
            }
            out
        })
    };

    print_query_timing(
        &query,
        out.len(),
        slot_count,
        start.elapsed().as_secs_f64() * 1_000_000.0,
    );
    out
}

fn recommended_workers(total_nodes: usize) -> usize {
    if total_nodes < PARALLEL_MIN_NODES {
        return 1;
    }

    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

fn scan_range(
    arena: &NodeArena,
    start: usize,
    end: usize,
    query: &TagQuery,
    plan: &QueryPlan,
) -> Vec<NodeID> {
    let mut out = Vec::new();
    out.reserve((end.saturating_sub(start)) / 4);
    for index in start..end {
        let Some((id, node)) = arena.slot_get(index) else {
            continue;
        };
        if matches_query(node.tags_slice(), node.node_type(), query, plan) {
            out.push(id);
        }
    }
    out
}

fn matches_query(tags: &[TagID], node_type: NodeType, query: &TagQuery, plan: &QueryPlan) -> bool {
    if plan.exact_type_mask != 0 && !plan.type_in_mask(node_type, plan.exact_type_mask) {
        return false;
    }

    if plan.base_type_mask != 0 && !plan.type_in_mask(node_type, plan.base_type_mask) {
        return false;
    }

    if !query.has.is_empty() && !query.has.iter().all(|tag| tags.contains(tag)) {
        return false;
    }

    if !query.any.is_empty() && !query.any.iter().any(|tag| tags.contains(tag)) {
        return false;
    }

    if !query.not.is_empty() && query.not.iter().any(|tag| tags.contains(tag)) {
        return false;
    }

    true
}

struct QueryPlan {
    exact_type_mask: u64,
    base_type_mask: u64,
}

impl QueryPlan {
    fn from_query(query: &TagQuery) -> Self {
        let exact_type_mask = mask_from_exact_types(&query.is_node_types);
        let base_type_mask = mask_from_base_types(&query.base_node_types);
        Self {
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

fn mask_from_exact_types(types: &[NodeType]) -> u64 {
    let mut mask = 0_u64;
    for &ty in types {
        mask |= 1_u64 << (ty as u8);
    }
    mask
}

fn mask_from_base_types(bases: &[NodeType]) -> u64 {
    if bases.is_empty() {
        return 0;
    }

    let mut mask = 0_u64;
    for &ty in NodeType::ALL {
        if bases.iter().any(|base| ty.is_a(*base)) {
            mask |= 1_u64 << (ty as u8);
        }
    }
    mask
}

fn print_query_timing(query: &TagQuery, matches: usize, slot_count: usize, elapsed_us: f64) {
    println!(
        "[node_query] {:.2}us matches={} slots={} clauses: has={} any={} not={} is_type={} base_type={}",
        elapsed_us,
        matches,
        slot_count.saturating_sub(1),
        query.has.len(),
        query.any.len(),
        query.not.len(),
        query.is_node_types.len(),
        query.base_node_types.len(),
    );
}
