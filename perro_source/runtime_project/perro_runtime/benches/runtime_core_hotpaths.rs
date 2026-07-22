use ahash::AHashMap;
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_ids::{NodeID, TextureID};
use perro_nodes::{
    AnimatedSprite, AnimatedSprite2D, Node2D, Node3D, SceneNode, SceneNodeData, Sprite2D,
};
use perro_render_bridge::{Command2D, RenderCommand};
use perro_runtime::{NodeArena, Runtime};
use perro_runtime_api::sub_apis::{NodeAPI, NodeSpec};
use std::sync::Arc;

fn bench_node_arena_len_hotloop(c: &mut Criterion) {
    let mut arena = NodeArena::with_capacity(200_000);
    for _ in 0..200_000 {
        let _ = arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    }

    c.bench_function("runtime_core/node_arena_len_hotloop", |b| {
        b.iter(|| black_box(arena.len()))
    });
}

fn bench_child_topology_scan(c: &mut Criterion) {
    let mut arena = NodeArena::with_capacity(100_000);
    let mut ids = Vec::with_capacity(100_000);
    for index in 0..100_000 {
        let id = arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
        if index > 0 {
            let parent = ids[(index - 1) / 4];
            arena.get_mut(parent).unwrap().add_child(id);
        }
        ids.push(id);
    }
    arena.rebuild_packed_children();

    let mut group = c.benchmark_group("runtime_core/child_topology_scan_100k");
    group.bench_function("per_node_vec", |b| {
        b.iter(|| {
            let mut edges = 0usize;
            for &id in black_box(&ids) {
                edges += arena.get(id).unwrap().children_slice().len();
            }
            black_box(edges)
        })
    });
    group.bench_function("packed_cache", |b| {
        b.iter(|| {
            let mut edges = 0usize;
            for &id in black_box(&ids) {
                edges += arena.children(id).unwrap().len();
            }
            black_box(edges)
        })
    });
    group.finish();
}

fn bench_internal_schedule_unregister(c: &mut Criterion) {
    c.bench_function(
        "runtime_core/internal_schedule_unregister_remove_nodes",
        |b| {
            b.iter_batched(
                || {
                    let mut runtime = Runtime::new();
                    let ids: Vec<_> = (0..20_000)
                        .map(|_| NodeAPI::create::<Node3D>(&mut runtime))
                        .collect();
                    (runtime, ids)
                },
                |(mut runtime, ids)| {
                    for id in ids {
                        black_box(NodeAPI::remove_node(&mut runtime, id));
                    }
                    black_box(runtime.nodes.len())
                },
                criterion::BatchSize::LargeInput,
            )
        },
    );
}

fn bench_dirty_indices_snapshot_compare(c: &mut Criterion) {
    let dirty_indices: Vec<u32> = (0..200_000).collect();
    let mut group = c.benchmark_group("runtime_core/dirty_indices_snapshot_compare");
    group.bench_function("to_vec", |b| {
        b.iter(|| black_box(black_box(&dirty_indices).to_vec()))
    });
    group.bench_function("scratch_extend", |b| {
        let mut scratch = Vec::<u32>::new();
        b.iter(|| {
            scratch.clear();
            scratch.extend_from_slice(black_box(&dirty_indices));
            black_box(scratch.len())
        })
    });
    group.finish();
}

fn bench_transform_dirty_propagate_and_refresh(c: &mut Criterion) {
    c.bench_function("runtime_core/transform_dirty_propagate_and_refresh", |b| {
        b.iter_batched(
            || {
                let mut runtime = Runtime::new();
                let root = NodeAPI::create::<Node3D>(&mut runtime);
                let mut last = root;
                for _ in 1..10_000 {
                    let id = NodeAPI::create::<Node3D>(&mut runtime);
                    assert!(NodeAPI::reparent(&mut runtime, last, id));
                    last = id;
                }
                (runtime, root, last)
            },
            |(mut runtime, root, leaf)| {
                Runtime::mark_transform_dirty_recursive(&mut runtime, root);
                black_box(NodeAPI::get_global_transform_3d(&mut runtime, leaf))
            },
            criterion::BatchSize::LargeInput,
        )
    });
}

fn bench_create_nodes_10k_batch_transform_and_render(c: &mut Criterion) {
    c.bench_function(
        "runtime_core/create_nodes_10k_batch_transform_and_render",
        |b| {
            let templates_2d = vec![NodeSpec::new(Node2D::new()); 10_000];
            let templates_sprite = vec![NodeSpec::new(Sprite2D::new()); 10_000];
            b.iter_batched(
                Runtime::new,
                |mut runtime| {
                    let parent = NodeAPI::create::<Node2D>(&mut runtime);
                    let ids = NodeAPI::create_nodes(&mut runtime, &templates_2d, parent);
                    let _ = NodeAPI::get_global_transform_2d(&mut runtime, ids[ids.len() - 1]);

                    let sprite_ids =
                        NodeAPI::create_nodes(&mut runtime, &templates_sprite, NodeID::nil());
                    let texture = TextureID::from_parts(77, 0);
                    for id in sprite_ids {
                        let _ =
                            NodeAPI::with_node_mut::<Sprite2D, _, _>(&mut runtime, id, |sprite| {
                                sprite.texture = texture;
                            });
                    }
                    Runtime::extract_render_2d_commands(&mut runtime);
                    let mut commands = Vec::new();
                    Runtime::drain_render_commands(&mut runtime, &mut commands);
                    black_box((ids.len(), commands.len()))
                },
                criterion::BatchSize::LargeInput,
            )
        },
    );
}

fn bench_render_command_drain_hotloop(c: &mut Criterion) {
    c.bench_function("runtime_core/render_command_drain_hotloop", |b| {
        let mut runtime = Runtime::new();
        let mut out = Vec::with_capacity(4);
        b.iter(|| {
            for i in 0..4 {
                let node = NodeID::from_parts((i + 1) as u32, 0);
                Runtime::queue_render_command(
                    &mut runtime,
                    RenderCommand::TwoD(Command2D::RemoveNode { node }),
                );
            }
            Runtime::drain_render_commands(&mut runtime, &mut out);
            let len = out.len();
            out.clear();
            black_box(len)
        })
    });
}

fn bench_extract_moving_sprite2d_nodes(c: &mut Criterion) {
    let mut group = c.benchmark_group("runtime_core/extract_moving_sprite2d_nodes");
    for count in [500usize, 2_000, 10_000] {
        group.bench_with_input(
            BenchmarkId::new("mutate_extract", count),
            &count,
            |b, &count| {
                b.iter_batched(
                    || {
                        let mut runtime = Runtime::new();
                        let templates = vec![NodeSpec::new(Sprite2D::new()); count];
                        let ids = NodeAPI::create_nodes(&mut runtime, &templates, NodeID::nil());
                        let texture = TextureID::from_parts(77, 0);
                        for (i, &id) in ids.iter().enumerate() {
                            let _ = NodeAPI::with_node_mut::<Sprite2D, _, _>(
                                &mut runtime,
                                id,
                                |sprite| {
                                    sprite.texture = texture;
                                    sprite.transform.position.x = (i % 64) as f32;
                                    sprite.transform.position.y = (i / 64) as f32;
                                },
                            );
                        }
                        runtime.extract_render_2d_commands();
                        let mut commands = Vec::new();
                        runtime.drain_render_commands(&mut commands);
                        (runtime, ids, commands)
                    },
                    |(mut runtime, ids, mut commands)| {
                        for (i, &id) in ids.iter().enumerate() {
                            let _ = NodeAPI::with_node_mut::<Sprite2D, _, _>(
                                &mut runtime,
                                id,
                                |sprite| {
                                    sprite.transform.position.x += ((i % 3) as f32) * 0.25 + 0.1;
                                },
                            );
                        }
                        runtime.extract_render_2d_commands();
                        runtime.drain_render_commands(&mut commands);
                        black_box(commands.len())
                    },
                    criterion::BatchSize::LargeInput,
                )
            },
        );

        group.bench_with_input(
            BenchmarkId::new("extract_only", count),
            &count,
            |b, &count| {
                b.iter_batched(
                    || {
                        let mut runtime = Runtime::new();
                        let templates = vec![NodeSpec::new(Sprite2D::new()); count];
                        let ids = NodeAPI::create_nodes(&mut runtime, &templates, NodeID::nil());
                        let texture = TextureID::from_parts(77, 0);
                        for (i, &id) in ids.iter().enumerate() {
                            let _ = NodeAPI::with_node_mut::<Sprite2D, _, _>(
                                &mut runtime,
                                id,
                                |sprite| {
                                    sprite.texture = texture;
                                    sprite.transform.position.x = (i % 64) as f32;
                                    sprite.transform.position.y = (i / 64) as f32;
                                },
                            );
                        }
                        runtime.extract_render_2d_commands();
                        let mut commands = Vec::new();
                        runtime.drain_render_commands(&mut commands);
                        for (i, &id) in ids.iter().enumerate() {
                            if let Some(mut node) = runtime.nodes.get_mut(id)
                                && let perro_nodes::SceneNodeData::Sprite2D(sprite) = &mut node.data
                            {
                                sprite.transform.position.x += ((i % 3) as f32) * 0.25 + 0.1;
                            }
                            runtime.mark_needs_rerender(id);
                            runtime.mark_transform_dirty_recursive(id);
                        }
                        (runtime, commands)
                    },
                    |(mut runtime, mut commands)| {
                        runtime.extract_render_2d_commands();
                        runtime.drain_render_commands(&mut commands);
                        black_box(commands.len())
                    },
                    criterion::BatchSize::LargeInput,
                )
            },
        );
    }
    group.finish();
}

fn bench_map_and_schedule_scans(c: &mut Criterion) {
    let count = 200_000usize;
    let ids: Vec<NodeID> = (1..=count as u32)
        .map(|i| NodeID::from_parts(i, 0))
        .collect();
    let mut body_map = AHashMap::<NodeID, (u64, u8)>::default();
    for i in 1..=count as u32 {
        body_map.insert(NodeID::from_parts(i, 0), (i as u64, (i % 2) as u8));
    }

    let mut group = c.benchmark_group("runtime_core/map_and_schedule_scans");
    group.bench_function("physics_sync_world_keys_collect_then_get", |b| {
        b.iter(|| {
            let keys: Vec<NodeID> = black_box(&body_map).keys().copied().collect();
            let mut acc = 0u64;
            for id in keys {
                if let Some((opaque, kind)) = body_map.get(&id) {
                    acc = acc.wrapping_add(*opaque + *kind as u64);
                }
            }
            black_box(acc)
        })
    });
    group.bench_function("physics_sync_world_direct_iter", |b| {
        b.iter(|| {
            let mut acc = 0u64;
            for (_id, (opaque, kind)) in black_box(&body_map) {
                acc = acc.wrapping_add(*opaque + *kind as u64);
            }
            black_box(acc)
        })
    });
    group.bench_function("internal_schedule_take_scan", |b| {
        let mut legacy_schedule = ids.clone();
        b.iter(|| {
            let schedule = std::mem::take(&mut legacy_schedule);
            let mut acc = 0u64;
            for id in schedule.iter().copied() {
                acc = acc.wrapping_add(id.as_u64());
            }
            legacy_schedule = schedule;
            black_box(acc)
        })
    });
    group.bench_function("internal_schedule_index_scan", |b| {
        b.iter(|| {
            let mut acc = 0u64;
            for id in black_box(&ids) {
                acc = acc.wrapping_add(id.as_u64());
            }
            black_box(acc)
        })
    });
    group.bench_function("physics_scan_ids_copy_then_iter", |b| {
        let mut scratch = Vec::new();
        b.iter(|| {
            scratch.clear();
            scratch.extend_from_slice(black_box(&ids));
            let mut acc = 0u64;
            for id in scratch.iter().copied() {
                acc = acc.wrapping_add(id.as_u64());
            }
            black_box(acc)
        })
    });
    group.bench_function("physics_scan_ids_direct_iter", |b| {
        b.iter(|| {
            let mut acc = 0u64;
            for id in black_box(&ids) {
                acc = acc.wrapping_add(id.as_u64());
            }
            black_box(acc)
        })
    });
    group.finish();
}

fn bench_trimesh_vertices_clone_vs_arc_share(c: &mut Criterion) {
    let vertices: Vec<[f32; 3]> = (0..10_000)
        .map(|i| [i as f32 * 0.01, (i % 13) as f32, (i % 7) as f32])
        .collect();
    let shared: Arc<[[f32; 3]]> = Arc::from(vertices.clone());
    let layers = 6usize;

    let mut group = c.benchmark_group("runtime_core/trimesh_vertices_clone_vs_arc_share");
    group.bench_function("clone_vertices", |b| {
        b.iter(|| {
            let mut copies = Vec::with_capacity(layers);
            for _ in 0..layers {
                copies.push(black_box(&vertices).clone());
            }
            black_box(copies.len() * copies[0].len())
        })
    });
    group.bench_function("arc_share", |b| {
        b.iter(|| {
            let mut refs = Vec::with_capacity(layers);
            for _ in 0..layers {
                refs.push(black_box(&shared).clone());
            }
            black_box(refs.len() * refs[0].len())
        })
    });
    group.finish();
}

fn bench_animated_sprite_2d_hotpaths(c: &mut Criterion) {
    let mut sprite = AnimatedSprite2D::new();
    sprite.current_animation = "run".into();
    for i in 0..32 {
        let mut animation = AnimatedSprite::new(format!("anim_{i}"));
        animation.frame_size = [16.0, 16.0];
        animation.frame_count = 16;
        animation.columns = 4;
        animation.fps = 12.0 + i as f32;
        sprite.animations.push(animation);
    }
    let mut run = AnimatedSprite::new("run");
    run.start = [32.0, 16.0];
    run.frame_size = [16.0, 16.0];
    run.frame_count = 24;
    run.columns = 6;
    run.fps = 24.0;
    sprite.animations.push(run);

    let mut group = c.benchmark_group("runtime_core/animated_sprite_2d_hotpaths");
    group.bench_function("current_animation_data", |b| {
        b.iter(|| {
            let animation = black_box(&sprite)
                .current_animation_data()
                .expect("animation");
            black_box(animation.frame_count)
        })
    });
    group.bench_function("current_texture_region", |b| {
        b.iter(|| black_box(&sprite).current_texture_region())
    });
    group.bench_function("step_like_update", |b| {
        b.iter_batched(
            || sprite.clone(),
            |mut sprite| {
                for _ in 0..120 {
                    let Some(animation) = sprite.current_animation_data() else {
                        return black_box(sprite.current_frame);
                    };
                    let frame_count = animation.frame_count.max(1);
                    let fps = animation.fps.max(0.0) * sprite.fps_scale.max(0.0);
                    sprite.current_frame = sprite.current_frame.min(frame_count.saturating_sub(1));
                    if sprite.playing && fps > 0.0 && frame_count > 1 {
                        sprite.frame_accum += 1.0 / 60.0 * fps;
                        let steps = sprite.frame_accum.floor() as u32;
                        if steps > 0 {
                            sprite.frame_accum -= steps as f32;
                            sprite.current_frame = (sprite.current_frame + steps) % frame_count;
                        }
                    }
                }
                black_box(sprite.current_frame)
            },
            criterion::BatchSize::SmallInput,
        )
    });
    group.bench_function("leaf_force_rerender", |b| {
        b.iter_batched(
            || {
                let mut runtime = Runtime::new();
                let ids = (0..10_000)
                    .map(|_| {
                        runtime
                            .nodes
                            .insert(SceneNode::new(SceneNodeData::AnimatedSprite2D(
                                AnimatedSprite2D::new(),
                            )))
                    })
                    .collect::<Vec<_>>();
                (runtime, ids)
            },
            |(mut runtime, ids)| {
                for id in ids {
                    runtime.mark_needs_rerender(id);
                }
                black_box(runtime.dirty_node_count())
            },
            criterion::BatchSize::LargeInput,
        )
    });
    group.finish();
}

fn benches(c: &mut Criterion) {
    bench_node_arena_len_hotloop(c);
    bench_child_topology_scan(c);
    bench_internal_schedule_unregister(c);
    bench_dirty_indices_snapshot_compare(c);
    bench_transform_dirty_propagate_and_refresh(c);
    bench_create_nodes_10k_batch_transform_and_render(c);
    bench_render_command_drain_hotloop(c);
    bench_extract_moving_sprite2d_nodes(c);
    bench_map_and_schedule_scans(c);
    bench_trimesh_vertices_clone_vs_arc_share(c);
    bench_animated_sprite_2d_hotpaths(c);
}

criterion_group! {
    name = runtime_core_hotpaths;
    config = Criterion::default().sample_size(10);
    targets = benches
}
criterion_main!(runtime_core_hotpaths);
