//! AnimationTree pose-apply hot path.
//!
//! Drives a full `Runtime::update` frame over one `AnimationTree` node whose
//! single slot animates N independent `Node3D` position tracks. Each frame the
//! internal update samples the slot clip into a pose and applies every pose
//! track (`eval_tree_pose` -> `apply_pose` in `perro_internal_updates`), which
//! is the path this bench isolates: the runtime is otherwise empty, so frame
//! cost is dominated by pose evaluation + application.
//!
//! Run: cargo bench -p perro_runtime --features bench --bench animation_tree_pose

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_nodes::{AnimationTree, Node3D};
use perro_runtime::Runtime;
use perro_runtime_api::sub_apis::NodeAPI;
use std::fmt::Write as _;

/// A clip with `object_count` Node3D objects, each carrying a position track
/// keyed at frames 0 and 59 so every frame samples a linearly interpolated
/// Transform3D value.
fn make_panim(object_count: usize) -> String {
    let mut src = String::new();
    src.push_str("[Animation]\nname = \"PoseBench\"\nfps = 60\n[/Animation]\n\n[Objects]\n");
    for i in 0..object_count {
        let _ = writeln!(src, "Obj{i} = Node3D");
    }
    src.push_str("[/Objects]\n\n[Frame0]\n");
    for i in 0..object_count {
        let _ = writeln!(src, "@Obj{i} {{ position = (0.0, 0.0, 0.0) }}");
    }
    src.push_str("[/Frame0]\n\n[Frame59]\n");
    for i in 0..object_count {
        let _ = writeln!(
            src,
            "@Obj{i} {{ position = ({}.0, 2.0, 3.0) }}",
            (i % 7) + 1
        );
    }
    src.push_str("[/Frame59]\n");
    src
}

/// One slot, output wired straight to it — the graph itself stays trivial so
/// the measured work is pose sampling + application, not blending.
const PANIMTREE_SRC: &str = r#"
[AnimationTree]
name = "PoseBenchTree"
[/AnimationTree]
[AnimationSlots]
Base
[/AnimationSlots]
[Output]
input = @Base
[/Output]
"#;

fn build_runtime(object_count: usize) -> Runtime {
    let mut runtime = Runtime::new();
    let targets: Vec<_> = (0..object_count)
        .map(|_| NodeAPI::create::<Node3D>(&mut runtime))
        .collect();
    let tree_node = NodeAPI::create::<AnimationTree>(&mut runtime);

    let panim = make_panim(object_count);
    let (animation, tree_asset) = runtime.bench_with_script_context(tree_node, |ctx| {
        (
            ctx.res.Animations().create_from_bytes(panim.as_bytes()),
            ctx.res
                .AnimationTrees()
                .create_from_bytes(PANIMTREE_SRC.as_bytes()),
        )
    });
    assert!(!animation.is_nil(), "bench panim must parse");
    assert!(!tree_asset.is_nil(), "bench panimtree must parse");

    let bound = NodeAPI::with_node_mut::<AnimationTree, _, _>(&mut runtime, tree_node, |tree| {
        tree.set_tree(tree_asset);
        tree.set_clip_by_index(0, animation);
        for (i, &node) in targets.iter().enumerate() {
            tree.set_slot_binding(0, &format!("Obj{i}"), node);
        }
    });
    assert!(bound.is_some(), "animation tree node must exist");
    runtime
}

fn bench_animation_tree_pose(c: &mut Criterion) {
    let mut group = c.benchmark_group("animation_tree/pose_apply_frame");
    for &track_count in &[25usize, 100] {
        let mut runtime = build_runtime(track_count);
        // Warm-up frame builds the slot playback state and applies frame 0.
        runtime.update(1.0 / 60.0);
        group.bench_with_input(
            BenchmarkId::new("tracks", track_count),
            &track_count,
            |b, _| {
                b.iter(|| {
                    runtime.update(black_box(1.0 / 60.0));
                })
            },
        );
    }
    group.finish();
}

criterion_group! {
    name = animation_tree_pose;
    config = Criterion::default().sample_size(60);
    targets = bench_animation_tree_pose
}
criterion_main!(animation_tree_pose);
