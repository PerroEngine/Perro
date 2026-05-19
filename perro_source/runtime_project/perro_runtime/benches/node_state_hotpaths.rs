use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_ids::{NodeID, ScriptMemberID, TagID};
use perro_nodes::{Node2D, Node3D};
use perro_runtime::Runtime;
use perro_runtime::api::scripts::{BenchScriptState, bench_insert_state_script};
use perro_runtime_api::sub_apis::{NodeAPI, NodeCreationTemplate, ScriptAPI};
use perro_structs::{Transform2D, Transform3D, Vector2, Vector3};
use perro_variant::Variant;

fn build_chain_2d(count: usize) -> (Runtime, Vec<NodeID>) {
    let mut runtime = Runtime::new();
    let root = NodeAPI::create::<Node2D>(&mut runtime);
    let mut ids = Vec::with_capacity(count);
    ids.push(root);
    let mut parent = root;
    for i in 1..count {
        let id = NodeAPI::create::<Node2D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, parent, id));
        let _ = NodeAPI::with_base_node_mut::<Node2D, _, _>(&mut runtime, id, |node| {
            node.transform = Transform2D::new(
                Vector2::new((i % 17) as f32 * 0.25, (i % 11) as f32 * 0.5),
                (i % 31) as f32 * 0.001,
                Vector2::ONE,
            );
        });
        ids.push(id);
        parent = id;
    }
    (runtime, ids)
}

fn build_chain_3d(count: usize) -> (Runtime, Vec<NodeID>) {
    let mut runtime = Runtime::new();
    let root = NodeAPI::create::<Node3D>(&mut runtime);
    let mut ids = Vec::with_capacity(count);
    ids.push(root);
    let mut parent = root;
    for i in 1..count {
        let id = NodeAPI::create::<Node3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, parent, id));
        let _ = NodeAPI::with_base_node_mut::<Node3D, _, _>(&mut runtime, id, |node| {
            node.transform = Transform3D::new(
                Vector3::new((i % 17) as f32 * 0.25, (i % 11) as f32 * 0.5, 0.5),
                node.transform.rotation,
                Vector3::ONE,
            );
        });
        ids.push(id);
        parent = id;
    }
    (runtime, ids)
}

fn build_wide_tree_3d(branches: usize, depth: usize) -> (Runtime, NodeID, Vec<NodeID>) {
    let mut runtime = Runtime::new();
    let root = NodeAPI::create::<Node3D>(&mut runtime);
    let mut level = vec![root];
    let mut all = vec![root];
    for _ in 0..depth {
        let mut next = Vec::with_capacity(level.len() * branches);
        for &parent in &level {
            for _ in 0..branches {
                let id = NodeAPI::create::<Node3D>(&mut runtime);
                assert!(NodeAPI::reparent(&mut runtime, parent, id));
                next.push(id);
                all.push(id);
            }
        }
        level = next;
    }
    (runtime, root, all)
}

fn bench_transform_propagation(c: &mut Criterion) {
    let mut group = c.benchmark_group("node_state/transform_propagation");

    for count in [256usize, 2_048, 8_192] {
        group.bench_with_input(
            BenchmarkId::new("chain_2d_cold_leaf", count),
            &count,
            |b, &count| {
                b.iter_batched(
                    || build_chain_2d(count),
                    |(mut runtime, ids)| {
                        let root = ids[0];
                        let leaf = ids[ids.len() - 1];
                        Runtime::mark_transform_dirty_recursive(&mut runtime, root);
                        black_box(NodeAPI::get_global_transform_2d(&mut runtime, leaf))
                    },
                    criterion::BatchSize::LargeInput,
                )
            },
        );

        group.bench_with_input(
            BenchmarkId::new("chain_3d_cold_leaf", count),
            &count,
            |b, &count| {
                b.iter_batched(
                    || build_chain_3d(count),
                    |(mut runtime, ids)| {
                        let root = ids[0];
                        let leaf = ids[ids.len() - 1];
                        Runtime::mark_transform_dirty_recursive(&mut runtime, root);
                        black_box(NodeAPI::get_global_transform_3d(&mut runtime, leaf))
                    },
                    criterion::BatchSize::LargeInput,
                )
            },
        );

        group.bench_with_input(
            BenchmarkId::new("chain_3d_cached_leaf", count),
            &count,
            |b, &count| {
                let (mut runtime, ids) = build_chain_3d(count);
                let leaf = ids[ids.len() - 1];
                black_box(NodeAPI::get_global_transform_3d(&mut runtime, leaf));
                b.iter(|| black_box(NodeAPI::get_global_transform_3d(&mut runtime, leaf)))
            },
        );
    }

    group.bench_function("wide_tree_3d_dirty_root_refresh", |b| {
        b.iter_batched(
            || build_wide_tree_3d(8, 5),
            |(mut runtime, root, all)| {
                Runtime::mark_transform_dirty_recursive(&mut runtime, root);
                Runtime::bench_refresh_dirty_global_transforms(&mut runtime);
                black_box((all.len(), runtime.dirty_node_count()))
            },
            criterion::BatchSize::LargeInput,
        )
    });

    group.finish();
}

fn bench_node_api(c: &mut Criterion) {
    let mut group = c.benchmark_group("node_state/node_api");

    for count in [1_000usize, 10_000, 50_000] {
        group.bench_with_input(
            BenchmarkId::new("create_nodes_3d_batch", count),
            &count,
            |b, &count| {
                let requests = vec![NodeCreationTemplate::new::<Node3D>(); count];
                b.iter_batched(
                    Runtime::new,
                    |mut runtime| {
                        black_box(NodeAPI::create_nodes(
                            &mut runtime,
                            &requests,
                            NodeID::nil(),
                        ))
                    },
                    criterion::BatchSize::LargeInput,
                )
            },
        );

        group.bench_with_input(
            BenchmarkId::new("create_nodes_3d_batch_parented", count),
            &count,
            |b, &count| {
                let requests = vec![NodeCreationTemplate::new::<Node3D>(); count];
                b.iter_batched(
                    || {
                        let mut runtime = Runtime::new();
                        let parent = NodeAPI::create::<Node3D>(&mut runtime);
                        (runtime, parent)
                    },
                    |(mut runtime, parent)| {
                        black_box(NodeAPI::create_nodes(&mut runtime, &requests, parent))
                    },
                    criterion::BatchSize::LargeInput,
                )
            },
        );

        group.bench_with_input(
            BenchmarkId::new("with_base_node_read", count),
            &count,
            |b, &count| {
                let (mut runtime, ids) = build_chain_3d(count);
                b.iter(|| {
                    let mut sum = 0.0f32;
                    for &id in &ids {
                        sum += NodeAPI::with_base_node::<Node3D, _, _>(&mut runtime, id, |node| {
                            node.transform.position.x
                        })
                        .unwrap_or_default();
                    }
                    black_box(sum)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("with_base_node_mut_transform", count),
            &count,
            |b, &count| {
                let (mut runtime, ids) = build_chain_3d(count);
                b.iter(|| {
                    for (i, &id) in ids.iter().enumerate() {
                        let _ =
                            NodeAPI::with_base_node_mut::<Node3D, _, _>(&mut runtime, id, |node| {
                                node.transform.position.x += (i % 3) as f32 * 0.001;
                            });
                    }
                    black_box(runtime.dirty_node_count())
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("tag_add_remove", count),
            &count,
            |b, &count| {
                let mut runtime = Runtime::new();
                let ids: Vec<_> = (0..count)
                    .map(|_| NodeAPI::create::<Node3D>(&mut runtime))
                    .collect();
                let tag = TagID::from_string("bench_tag");
                b.iter(|| {
                    for &id in &ids {
                        black_box(NodeAPI::add_node_tag(&mut runtime, id, tag));
                    }
                    for &id in &ids {
                        black_box(NodeAPI::remove_node_tag(&mut runtime, id, tag));
                    }
                })
            },
        );
    }

    group.bench_function("remove_deep_subtree_10k", |b| {
        b.iter_batched(
            || build_chain_3d(10_000),
            |(mut runtime, ids)| black_box(NodeAPI::remove_node(&mut runtime, ids[0])),
            criterion::BatchSize::LargeInput,
        )
    });

    group.finish();
}

fn bench_script_state(c: &mut Criterion) {
    let mut group = c.benchmark_group("node_state/script_state");

    for count in [1_000usize, 10_000, 50_000] {
        group.bench_with_input(
            BenchmarkId::new("insert_state_scripts", count),
            &count,
            |b, &count| {
                b.iter_batched(
                    Runtime::new,
                    |mut runtime| {
                        for i in 0..count {
                            bench_insert_state_script(&mut runtime, NodeID::new((i + 1) as u32));
                        }
                        black_box(count)
                    },
                    criterion::BatchSize::LargeInput,
                )
            },
        );

        group.bench_with_input(
            BenchmarkId::new("with_state_read", count),
            &count,
            |b, &count| {
                let mut runtime = Runtime::new();
                let ids: Vec<_> = (0..count)
                    .map(|i| {
                        let id = NodeID::new((i + 1) as u32);
                        bench_insert_state_script(&mut runtime, id);
                        id
                    })
                    .collect();
                b.iter(|| {
                    let mut sum = 0u64;
                    for &id in &ids {
                        sum = sum.wrapping_add(ScriptAPI::with_state::<BenchScriptState, _, _>(
                            &mut runtime,
                            id,
                            |state| state.frame,
                        ));
                    }
                    black_box(sum)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("with_state_mut", count),
            &count,
            |b, &count| {
                let mut runtime = Runtime::new();
                let ids: Vec<_> = (0..count)
                    .map(|i| {
                        let id = NodeID::new((i + 1) as u32);
                        bench_insert_state_script(&mut runtime, id);
                        id
                    })
                    .collect();
                b.iter(|| {
                    for &id in &ids {
                        let _ = ScriptAPI::with_state_mut::<BenchScriptState, _, _>(
                            &mut runtime,
                            id,
                            |state| {
                                state.frame = state.frame.wrapping_add(1);
                                state.hp += 1;
                                state.pos[0] += 0.25;
                            },
                        );
                    }
                    black_box(ids.len())
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("get_set_var", count),
            &count,
            |b, &count| {
                let mut runtime = Runtime::new();
                let ids: Vec<_> = (0..count)
                    .map(|i| {
                        let id = NodeID::new((i + 1) as u32);
                        bench_insert_state_script(&mut runtime, id);
                        id
                    })
                    .collect();
                let frame = ScriptMemberID(1);
                b.iter(|| {
                    for (i, &id) in ids.iter().enumerate() {
                        ScriptAPI::set_var(&mut runtime, id, frame, Variant::from(i as i64));
                        black_box(ScriptAPI::get_var(&mut runtime, id, frame));
                    }
                })
            },
        );
    }

    group.finish();
}

fn benches(c: &mut Criterion) {
    bench_transform_propagation(c);
    bench_node_api(c);
    bench_script_state(c);
}

criterion_group! {
    name = node_state_hotpaths;
    config = Criterion::default().sample_size(10);
    targets = benches
}
criterion_main!(node_state_hotpaths);
