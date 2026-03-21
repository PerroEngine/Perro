use super::*;
use perro_nodes::{Node3D, SceneNode, SceneNodeData};
use std::hint::black_box;

#[test]
fn node_arena_len_tracks_active_nodes() {
    let mut arena = NodeArena::new();
    assert_eq!(arena.len(), 0);
    assert!(arena.is_empty());

    let a = arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let b = arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    assert_eq!(arena.len(), 2);
    assert!(!arena.is_empty());

    let _ = arena.remove(a);
    assert_eq!(arena.len(), 1);
    let _ = arena.remove(b);
    assert_eq!(arena.len(), 0);
    assert!(arena.is_empty());
}

#[test]
#[ignore]
fn bench_node_arena_len_hotloop() {
    let mut arena = NodeArena::with_capacity(200_000);
    for _ in 0..200_000 {
        let _ = arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    }

    let rounds = 5_000_000usize;
    let start = std::time::Instant::now();
    let mut acc = 0usize;
    for _ in 0..rounds {
        acc = acc.wrapping_add(black_box(arena.len()));
    }
    let elapsed_us = start.elapsed().as_micros();
    println!(
        "bench_node_arena_len_hotloop: rounds={} elapsed={}us (acc={})",
        rounds, elapsed_us, acc
    );
}

#[test]
#[ignore]
fn bench_internal_schedule_unregister() {
    let mut runtime = Runtime::new();
    let count = 100_000usize;

    runtime.internal_updates.internal_update_nodes.clear();
    runtime.internal_updates.internal_fixed_update_nodes.clear();
    runtime.internal_updates.internal_update_pos.clear();
    runtime.internal_updates.internal_fixed_update_pos.clear();

    let mut ids = Vec::with_capacity(count);
    for _ in 0..count {
        let id = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
        ids.push(id);
        let slot = id.index() as usize;
        if runtime.internal_updates.internal_update_pos.len() <= slot {
            runtime
                .internal_updates
                .internal_update_pos
                .resize(slot + 1, None);
        }
        if runtime.internal_updates.internal_fixed_update_pos.len() <= slot {
            runtime
                .internal_updates
                .internal_fixed_update_pos
                .resize(slot + 1, None);
        }
        runtime.internal_updates.internal_update_pos[slot] =
            Some(runtime.internal_updates.internal_update_nodes.len());
        runtime.internal_updates.internal_update_nodes.push(id);
        runtime.internal_updates.internal_fixed_update_pos[slot] =
            Some(runtime.internal_updates.internal_fixed_update_nodes.len());
        runtime
            .internal_updates
            .internal_fixed_update_nodes
            .push(id);
    }

    let start = std::time::Instant::now();
    for id in ids {
        runtime.unregister_internal_node_schedules(id);
    }
    let elapsed_us = start.elapsed().as_micros();
    println!(
        "bench_internal_schedule_unregister: removed={} in {}us",
        count, elapsed_us
    );

    assert!(runtime.internal_updates.internal_update_nodes.is_empty());
    assert!(
        runtime
            .internal_updates
            .internal_fixed_update_nodes
            .is_empty()
    );
}
