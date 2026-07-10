// F1 microbench: `effective_self_modulate` folds the modulate product up a
// node's ancestor chain. This isolates that ancestor walk over deep hierarchies
// (before: one heap `Vec` alloc + a reverse pass per call; after: a single
// zero-alloc upward fold).

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_nodes::MeshInstance3D;
use perro_runtime::Runtime;
use perro_runtime_api::sub_apis::NodeAPI;
use perro_structs::{Color, NodeModulate};

fn tinted() -> NodeModulate {
    // Non-white on every channel so `color_modulate` never short-circuits and
    // the full per-node multiply runs.
    NodeModulate::new(
        Color::new(0.99, 0.98, 0.97, 1.0),
        Color::new(0.96, 0.95, 0.94, 0.99),
        Color::new(0.97, 0.96, 0.95, 0.98),
    )
}

// `width` independent chains, each `depth` Node3D-based nodes deep. Returns the
// leaf id of every chain (the node whose effective modulate walks the full depth).
fn build_chains(runtime: &mut Runtime, width: usize, depth: usize) -> Vec<perro_ids::NodeID> {
    let tint = tinted();
    let mut leaves = Vec::with_capacity(width);
    for _ in 0..width {
        let mut current = NodeAPI::create::<MeshInstance3D>(runtime);
        runtime.bench_set_node3d_modulate(current, tint);
        for _ in 1..depth {
            let child = NodeAPI::create::<MeshInstance3D>(runtime);
            runtime.bench_set_node3d_modulate(child, tint);
            NodeAPI::reparent(runtime, current, child);
            current = child;
        }
        leaves.push(current);
    }
    leaves
}

fn bench_modulate_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("effective_self_modulate/deep_chain");
    let width = 1_024usize;
    for depth in [4usize, 8, 16] {
        let mut runtime = Runtime::new();
        let leaves = build_chains(&mut runtime, width, depth);
        group.bench_with_input(
            BenchmarkId::new("width_1024", format!("depth_{depth}")),
            &leaves,
            |b, leaves| {
                b.iter(|| black_box(runtime.bench_effective_self_modulate_sum(black_box(leaves))));
            },
        );
    }
    group.finish();
}

criterion_group! {
    name = modulate_chain;
    config = Criterion::default().sample_size(30);
    targets = bench_modulate_chain
}
criterion_main!(modulate_chain);
