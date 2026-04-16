use super::*;
use ahash::AHashMap;
use perro_ids::NodeID;
use perro_nodes::{Node3D, SceneNode, SceneNodeData};
use perro_render_bridge::{Command2D, RenderCommand};
use std::hint::black_box;
use std::sync::Arc;

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
                .resize(slot + 1, u32::MAX);
        }
        if runtime.internal_updates.internal_fixed_update_pos.len() <= slot {
            runtime
                .internal_updates
                .internal_fixed_update_pos
                .resize(slot + 1, u32::MAX);
        }
        runtime.internal_updates.internal_update_pos[slot] =
            runtime.internal_updates.internal_update_nodes.len() as u32;
        runtime.internal_updates.internal_update_nodes.push(id);
        runtime.internal_updates.internal_fixed_update_pos[slot] =
            runtime.internal_updates.internal_fixed_update_nodes.len() as u32;
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

#[test]
#[ignore]
fn bench_dirty_indices_snapshot_compare() {
    let count = 200_000usize;
    let rounds = 2_000usize;
    let dirty_indices: Vec<u32> = (0..count as u32).collect();

    let start_legacy = std::time::Instant::now();
    let mut acc_legacy = 0usize;
    for _ in 0..rounds {
        let copied = dirty_indices.to_vec();
        acc_legacy = acc_legacy.wrapping_add(copied.len());
    }
    let legacy_us = start_legacy.elapsed().as_micros();

    let mut scratch = Vec::<u32>::new();
    let start_scratch = std::time::Instant::now();
    let mut acc_scratch = 0usize;
    for _ in 0..rounds {
        scratch.clear();
        scratch.extend_from_slice(&dirty_indices);
        acc_scratch = acc_scratch.wrapping_add(scratch.len());
    }
    let scratch_us = start_scratch.elapsed().as_micros();
    let speedup = legacy_us as f64 / scratch_us.max(1) as f64;

    println!(
        "bench_dirty_indices_snapshot_compare: count={} rounds={} legacy_us={} scratch_us={} speedup={:.3}x acc_legacy={} acc_scratch={}",
        count, rounds, legacy_us, scratch_us, speedup, acc_legacy, acc_scratch
    );
}

#[test]
#[ignore]
fn bench_physics_scan_ids_clone_vs_scratch() {
    let count = 200_000usize;
    let rounds = 2_000usize;
    let ids: Vec<perro_ids::NodeID> = (1..=count as u32)
        .map(|i| perro_ids::NodeID::from_parts(i, 0))
        .collect();

    let start_clone = std::time::Instant::now();
    let mut acc_clone = 0usize;
    for _ in 0..rounds {
        let copied = ids.clone();
        acc_clone = acc_clone.wrapping_add(copied.len());
    }
    let clone_us = start_clone.elapsed().as_micros();

    let mut scratch = Vec::<perro_ids::NodeID>::new();
    let start_scratch = std::time::Instant::now();
    let mut acc_scratch = 0usize;
    for _ in 0..rounds {
        scratch.clear();
        scratch.extend_from_slice(&ids);
        acc_scratch = acc_scratch.wrapping_add(scratch.len());
    }
    let scratch_us = start_scratch.elapsed().as_micros();
    let speedup = clone_us as f64 / scratch_us.max(1) as f64;

    println!(
        "bench_physics_scan_ids_clone_vs_scratch: count={} rounds={} clone_us={} scratch_us={} speedup={:.3}x acc_clone={} acc_scratch={}",
        count, rounds, clone_us, scratch_us, speedup, acc_clone, acc_scratch
    );
}

#[test]
#[ignore]
fn bench_transform_dirty_propagate_and_refresh() {
    let mut runtime = Runtime::new();
    let count = 40_000usize;
    let rounds = 120usize;

    let mut ids = Vec::with_capacity(count);
    for _ in 0..count {
        let id = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
        ids.push(id);
        if ids.len() > 1 {
            let parent = ids[ids.len() - 2];
            if let Some(parent_node) = runtime.nodes.get_mut(parent) {
                parent_node.add_child(id);
            }
            if let Some(child_node) = runtime.nodes.get_mut(id) {
                child_node.parent = parent;
            }
        }
    }
    let root = ids[0];
    let leaf = ids[count - 1];

    for _ in 0..4 {
        runtime.mark_transform_dirty_recursive(root);
        runtime.propagate_pending_transform_dirty();
        runtime.refresh_dirty_global_transforms();
        let _ = runtime.get_global_transform_3d(leaf);
    }

    let start = std::time::Instant::now();
    let mut acc = 0.0f32;
    for _ in 0..rounds {
        runtime.mark_transform_dirty_recursive(root);
        runtime.propagate_pending_transform_dirty();
        runtime.refresh_dirty_global_transforms();
        if let Some(global) = runtime.get_global_transform_3d(leaf) {
            acc += black_box(global.position.x + global.position.y + global.position.z);
        }
    }
    let elapsed_us = start.elapsed().as_micros();
    let per_round_us = elapsed_us as f64 / rounds as f64;
    println!(
        "bench_transform_dirty_propagate_and_refresh: nodes={} rounds={} total_us={} per_round_us={:.3} acc={}",
        count, rounds, elapsed_us, per_round_us, acc
    );
}

#[test]
#[ignore]
fn bench_render_command_drain_hotloop() {
    let mut runtime = Runtime::new();
    let rounds = 80_000usize;
    let commands_per_round = 4usize;
    let mut out = Vec::with_capacity(commands_per_round);

    for _ in 0..512 {
        for i in 0..commands_per_round {
            let node = NodeID::from_parts((i + 1) as u32, 0);
            runtime.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
        }
        runtime.drain_render_commands(&mut out);
        out.clear();
    }

    let start = std::time::Instant::now();
    let mut acc = 0usize;
    for round in 0..rounds {
        for i in 0..commands_per_round {
            let node = NodeID::from_parts((i + round + 1) as u32, 0);
            runtime.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
        }
        runtime.drain_render_commands(&mut out);
        acc = acc.wrapping_add(black_box(out.len()));
        out.clear();
    }
    let elapsed_us = start.elapsed().as_micros();
    let per_round_us = elapsed_us as f64 / rounds as f64;
    println!(
        "bench_render_command_drain_hotloop: rounds={} commands_per_round={} total_us={} per_round_us={:.3} acc={}",
        rounds, commands_per_round, elapsed_us, per_round_us, acc
    );
}

#[test]
#[ignore]
fn bench_physics_children_clone_vs_slice_scan() {
    let body_count = 250_000usize;
    let children_per_body = 4usize;
    let rounds = 120usize;

    let mut children = Vec::with_capacity(body_count);
    for body in 0..body_count {
        let mut ids = Vec::with_capacity(children_per_body);
        for i in 0..children_per_body {
            ids.push(NodeID::from_parts(
                (body * children_per_body + i + 1) as u32,
                0,
            ));
        }
        children.push(ids);
    }

    let t0 = std::time::Instant::now();
    let mut legacy_acc = 0u64;
    for _ in 0..rounds {
        for ids in &children {
            let copied = ids.to_vec();
            for id in &copied {
                legacy_acc = legacy_acc.wrapping_add(id.as_u64());
            }
        }
    }
    let legacy_us = t0.elapsed().as_micros();

    let t1 = std::time::Instant::now();
    let mut now_acc = 0u64;
    for _ in 0..rounds {
        for ids in &children {
            for &id in ids {
                now_acc = now_acc.wrapping_add(id.as_u64());
            }
        }
    }
    let now_us = t1.elapsed().as_micros();

    println!(
        "bench_physics_children_clone_vs_slice_scan: bodies={} children_per_body={} rounds={} legacy_us={} now_us={} speedup={:.3}x acc={}/{}",
        body_count,
        children_per_body,
        rounds,
        legacy_us,
        now_us,
        legacy_us as f64 / now_us.max(1) as f64,
        legacy_acc,
        now_acc
    );
}

#[test]
#[ignore]
fn bench_physics_sync_world_map_scan_legacy_vs_direct_iter() {
    let body_count = 250_000u32;
    let rounds = 180usize;

    let mut body_map = AHashMap::<NodeID, (u64, u8)>::default();
    for i in 1..=body_count {
        body_map.insert(NodeID::from_parts(i, 0), (i as u64, (i % 2) as u8));
    }

    let t0 = std::time::Instant::now();
    let mut legacy_acc = 0u64;
    for _ in 0..rounds {
        let ids: Vec<NodeID> = body_map.keys().copied().collect();
        for id in ids {
            if let Some((opaque, kind)) = body_map.get(&id) {
                legacy_acc = legacy_acc.wrapping_add(*opaque + *kind as u64);
            }
        }
    }
    let legacy_us = t0.elapsed().as_micros();

    let t1 = std::time::Instant::now();
    let mut now_acc = 0u64;
    for _ in 0..rounds {
        for (_id, (opaque, kind)) in &body_map {
            now_acc = now_acc.wrapping_add(*opaque + *kind as u64);
        }
    }
    let now_us = t1.elapsed().as_micros();

    println!(
        "bench_physics_sync_world_map_scan_legacy_vs_direct_iter: bodies={} rounds={} legacy_us={} now_us={} speedup={:.3}x acc={}/{}",
        body_count,
        rounds,
        legacy_us,
        now_us,
        legacy_us as f64 / now_us.max(1) as f64,
        legacy_acc,
        now_acc
    );
}

#[test]
#[ignore]
fn bench_internal_schedule_take_vs_index_scan() {
    let count = 200_000usize;
    let rounds = 400usize;
    let ids: Vec<NodeID> = (1..=count as u32)
        .map(|i| NodeID::from_parts(i, 0))
        .collect();

    let mut legacy_schedule = ids.clone();
    let t0 = std::time::Instant::now();
    let mut legacy_acc = 0u64;
    for _ in 0..rounds {
        let schedule = std::mem::take(&mut legacy_schedule);
        for id in schedule.iter().copied() {
            legacy_acc = legacy_acc.wrapping_add(id.as_u64());
        }
        legacy_schedule = schedule;
    }
    let legacy_us = t0.elapsed().as_micros();

    let index_schedule = ids;
    let t1 = std::time::Instant::now();
    let mut now_acc = 0u64;
    for _ in 0..rounds {
        for i in 0..index_schedule.len() {
            now_acc = now_acc.wrapping_add(index_schedule[i].as_u64());
        }
    }
    let now_us = t1.elapsed().as_micros();

    println!(
        "bench_internal_schedule_take_vs_index_scan: count={} rounds={} legacy_us={} now_us={} speedup={:.3}x acc={}/{}",
        count,
        rounds,
        legacy_us,
        now_us,
        legacy_us as f64 / now_us.max(1) as f64,
        legacy_acc,
        now_acc
    );
}

#[test]
#[ignore]
fn bench_physics_scan_ids_copy_vs_direct_iter() {
    let count = 250_000usize;
    let rounds = 220usize;
    let ids: Vec<NodeID> = (1..=count as u32)
        .map(|i| NodeID::from_parts(i, 0))
        .collect();

    let mut scratch = Vec::new();
    let t0 = std::time::Instant::now();
    let mut legacy_acc = 0u64;
    for _ in 0..rounds {
        scratch.clear();
        scratch.extend_from_slice(&ids);
        for id in scratch.iter().copied() {
            legacy_acc = legacy_acc.wrapping_add(id.as_u64());
        }
    }
    let legacy_us = t0.elapsed().as_micros();

    let t1 = std::time::Instant::now();
    let mut now_acc = 0u64;
    for _ in 0..rounds {
        for i in 0..ids.len() {
            now_acc = now_acc.wrapping_add(ids[i].as_u64());
        }
    }
    let now_us = t1.elapsed().as_micros();

    println!(
        "bench_physics_scan_ids_copy_vs_direct_iter: count={} rounds={} legacy_us={} now_us={} speedup={:.3}x acc={}/{}",
        count,
        rounds,
        legacy_us,
        now_us,
        legacy_us as f64 / now_us.max(1) as f64,
        legacy_acc,
        now_acc
    );
}

#[test]
#[ignore]
fn bench_trimesh_vertices_clone_vs_arc_share() {
    let vertices_len = 10_000usize;
    let layers = 6usize;
    let rounds = 2_000usize;

    let vertices: Vec<[f32; 3]> = (0..vertices_len)
        .map(|i| [i as f32 * 0.01, (i % 13) as f32, (i % 7) as f32])
        .collect();

    let t0 = std::time::Instant::now();
    let mut legacy_acc = 0usize;
    for _ in 0..rounds {
        let mut copies = Vec::with_capacity(layers);
        for _ in 0..layers {
            copies.push(vertices.clone());
        }
        legacy_acc = legacy_acc.wrapping_add(copies.len() * copies[0].len());
    }
    let legacy_us = t0.elapsed().as_micros();

    let shared: Arc<[[f32; 3]]> = Arc::from(vertices);
    let t1 = std::time::Instant::now();
    let mut now_acc = 0usize;
    for _ in 0..rounds {
        let mut refs = Vec::with_capacity(layers);
        for _ in 0..layers {
            refs.push(shared.clone());
        }
        now_acc = now_acc.wrapping_add(refs.len() * refs[0].len());
    }
    let now_us = t1.elapsed().as_micros();

    println!(
        "bench_trimesh_vertices_clone_vs_arc_share: verts={} layers={} rounds={} legacy_us={} now_us={} speedup={:.3}x acc={}/{}",
        vertices_len,
        layers,
        rounds,
        legacy_us,
        now_us,
        legacy_us as f64 / now_us.max(1) as f64,
        legacy_acc,
        now_acc
    );
}
