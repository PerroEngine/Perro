// F2 microbench: a skeleton animates every frame, so it is dirty every frame.
// Extraction must re-collect the mesh instances bound to that skeleton. Before:
// a full O(all-nodes) arena scan per frame. After: an O(skinned) pull from a
// skeleton->mesh reverse index. Extraction time should grow with the number of
// static mesh nodes before the fix and stay flat after it.

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_ids::NodeID;
use perro_nodes::{MeshInstance3D, Skeleton3D};
use perro_runtime::Runtime;
use perro_runtime_api::sub_apis::NodeAPI;

// `static_meshes` unskinned mesh nodes + one skeleton with one skinned mesh
// bound to it. Returns the skeleton id (the per-frame dirty node).
fn build_scene(runtime: &mut Runtime, static_meshes: usize) -> NodeID {
    for _ in 0..static_meshes {
        let _ = NodeAPI::create::<MeshInstance3D>(runtime);
    }
    let skeleton = NodeAPI::create::<Skeleton3D>(runtime);
    let skinned = NodeAPI::create::<MeshInstance3D>(runtime);
    runtime.bench_bind_mesh_skeleton(skinned, skeleton);
    skeleton
}

fn bench_skinned_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("extract_3d/dirty_skeleton");
    for static_meshes in [512usize, 4_096, 16_384] {
        let mut runtime = Runtime::new();
        let skeleton = build_scene(&mut runtime, static_meshes);

        // Bootstrap extraction (full scan) builds the reverse index / retained
        // state so the timed loop measures steady-state per-frame cost.
        let mut commands = Vec::new();
        runtime.extract_render_snapshot_commands(&mut commands);

        group.bench_with_input(
            BenchmarkId::from_parameter(static_meshes),
            &static_meshes,
            |b, _| {
                b.iter(|| {
                    runtime.bench_touch_node(skeleton);
                    commands.clear();
                    runtime.extract_render_snapshot_commands(&mut commands);
                    black_box(commands.len())
                });
            },
        );
    }
    group.finish();
}

criterion_group! {
    name = skinned_extraction;
    config = Criterion::default().sample_size(30);
    targets = bench_skinned_extraction
}
criterion_main!(skinned_extraction);
