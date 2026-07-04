use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_ids::TagID;
use perro_nodes::{MeshInstance3D, Node3D, NodeType};
use perro_runtime::Runtime;
use perro_runtime_api::sub_apis::{
    NodeAPI, NodeQuery, QueryBounds, QueryExpr, QueryScope, QueryTypeMask,
};
use perro_structs::{Transform3D, Vector3};

fn build_query_runtime(count: usize) -> Runtime {
    let mut runtime = Runtime::new();
    let enemy = TagID::from_string("enemy");
    let alive = TagID::from_string("alive");
    let boss = TagID::from_string("boss");
    for i in 0..count {
        let id = if i % 5 == 0 {
            NodeAPI::create::<MeshInstance3D>(&mut runtime)
        } else {
            NodeAPI::create::<Node3D>(&mut runtime)
        };
        let _ = NodeAPI::set_node_name(&mut runtime, id, if i % 4 == 0 { "enemy" } else { "npc" });
        if i % 3 == 0 {
            let _ = NodeAPI::add_node_tag(&mut runtime, id, enemy);
        }
        if i % 7 == 0 {
            let _ = NodeAPI::add_node_tag(&mut runtime, id, alive);
        }
        if i % 19 == 0 {
            let _ = NodeAPI::add_node_tag(&mut runtime, id, boss);
        }
    }
    runtime
}

fn bench_rt_ctx_queries(c: &mut Criterion) {
    let selective = NodeQuery {
        expr: Some(QueryExpr::All(vec![
            QueryExpr::IsType(vec![NodeType::MeshInstance3D]),
            QueryExpr::Name(vec!["enemy".to_string()]),
            QueryExpr::Tags(vec![
                TagID::from_string("enemy"),
                TagID::from_string("alive"),
                TagID::from_string("boss"),
            ]),
        ])),
        scope: QueryScope::Root,
    };
    let broad = NodeQuery {
        expr: Some(QueryExpr::Any(vec![QueryExpr::Tags(vec![
            TagID::from_string("enemy"),
            TagID::from_string("alive"),
        ])])),
        scope: QueryScope::Root,
    };
    let rare_tag_name = NodeQuery {
        expr: Some(QueryExpr::All(vec![
            QueryExpr::Tags(vec![TagID::from_string("boss")]),
            QueryExpr::Name(vec!["enemy".to_string()]),
        ])),
        scope: QueryScope::Root,
    };

    let mut group = c.benchmark_group("query/rt_ctx_queries");
    for count in [100usize, 2_500, 10_000, 50_000, 100_000] {
        group.bench_with_input(BenchmarkId::new("selective", count), &count, |b, &count| {
            b.iter_batched(
                || build_query_runtime(count),
                |mut runtime| black_box(NodeAPI::query_nodes(&mut runtime, selective.as_view())),
                criterion::BatchSize::LargeInput,
            )
        });
        group.bench_with_input(BenchmarkId::new("broad", count), &count, |b, &count| {
            b.iter_batched(
                || build_query_runtime(count),
                |mut runtime| black_box(NodeAPI::query_nodes(&mut runtime, broad.as_view())),
                criterion::BatchSize::LargeInput,
            )
        });
        group.bench_with_input(
            BenchmarkId::new("rare_tag_name", count),
            &count,
            |b, &count| {
                b.iter_batched(
                    || build_query_runtime(count),
                    |mut runtime| {
                        black_box(NodeAPI::query_nodes(&mut runtime, rare_tag_name.as_view()))
                    },
                    criterion::BatchSize::LargeInput,
                )
            },
        );
    }
    group.finish();
}

fn bench_compile_repr_queries(c: &mut Criterion) {
    let mesh_type_mask = QueryTypeMask::NONE.with_type(NodeType::MeshInstance3D);
    let node3d_base_mask = QueryTypeMask::NONE.with_type(NodeType::Node3D);
    let enemy = TagID::from_string("enemy");
    let alive = TagID::from_string("alive");
    let boss = TagID::from_string("boss");

    let selective_vec = NodeQuery {
        expr: Some(QueryExpr::All(vec![
            QueryExpr::IsType(vec![NodeType::MeshInstance3D]),
            QueryExpr::Name(vec!["enemy".to_string()]),
            QueryExpr::Tags(vec![enemy, alive, boss]),
        ])),
        scope: QueryScope::Root,
    };
    let selective_mask = NodeQuery {
        expr: Some(QueryExpr::All(vec![
            QueryExpr::IsTypeMask(mesh_type_mask),
            QueryExpr::Name(vec!["enemy".to_string()]),
            QueryExpr::Tags(vec![enemy, alive, boss]),
        ])),
        scope: QueryScope::Root,
    };
    let type_vec = NodeQuery {
        expr: Some(QueryExpr::All(vec![
            QueryExpr::IsType(vec![NodeType::MeshInstance3D]),
            QueryExpr::BaseType(vec![NodeType::Node3D]),
        ])),
        scope: QueryScope::Root,
    };
    let type_mask_query = NodeQuery {
        expr: Some(QueryExpr::All(vec![
            QueryExpr::IsTypeMask(mesh_type_mask),
            QueryExpr::BaseTypeMask(node3d_base_mask),
        ])),
        scope: QueryScope::Root,
    };
    let not_type_vec = NodeQuery {
        expr: Some(QueryExpr::Not(Box::new(QueryExpr::IsType(vec![
            NodeType::MeshInstance3D,
        ])))),
        scope: QueryScope::Root,
    };
    let not_type_mask = NodeQuery {
        expr: Some(QueryExpr::Not(Box::new(QueryExpr::IsTypeMask(
            mesh_type_mask,
        )))),
        scope: QueryScope::Root,
    };
    let rare_tag_name = NodeQuery {
        expr: Some(QueryExpr::All(vec![
            QueryExpr::Tags(vec![boss]),
            QueryExpr::Name(vec!["enemy".to_string()]),
        ])),
        scope: QueryScope::Root,
    };

    let mut group = c.benchmark_group("query/compile_repr");
    for count in [100usize, 2_500, 10_000, 50_000, 100_000] {
        group.bench_with_input(
            BenchmarkId::new("selective_vec", count),
            &count,
            |b, &count| {
                let mut runtime = build_query_runtime(count);
                b.iter(|| black_box(NodeAPI::query_nodes(&mut runtime, selective_vec.as_view())))
            },
        );
        group.bench_with_input(
            BenchmarkId::new("selective_mask", count),
            &count,
            |b, &count| {
                let mut runtime = build_query_runtime(count);
                b.iter(|| black_box(NodeAPI::query_nodes(&mut runtime, selective_mask.as_view())))
            },
        );
        group.bench_with_input(BenchmarkId::new("type_vec", count), &count, |b, &count| {
            let mut runtime = build_query_runtime(count);
            b.iter(|| black_box(NodeAPI::query_nodes(&mut runtime, type_vec.as_view())))
        });
        group.bench_with_input(BenchmarkId::new("type_mask", count), &count, |b, &count| {
            let mut runtime = build_query_runtime(count);
            b.iter(|| {
                black_box(NodeAPI::query_nodes(
                    &mut runtime,
                    type_mask_query.as_view(),
                ))
            })
        });
        group.bench_with_input(
            BenchmarkId::new("not_type_vec", count),
            &count,
            |b, &count| {
                let mut runtime = build_query_runtime(count);
                b.iter(|| black_box(NodeAPI::query_nodes(&mut runtime, not_type_vec.as_view())))
            },
        );
        group.bench_with_input(
            BenchmarkId::new("not_type_mask", count),
            &count,
            |b, &count| {
                let mut runtime = build_query_runtime(count);
                b.iter(|| black_box(NodeAPI::query_nodes(&mut runtime, not_type_mask.as_view())))
            },
        );
        group.bench_with_input(
            BenchmarkId::new("rare_tag_name", count),
            &count,
            |b, &count| {
                let mut runtime = build_query_runtime(count);
                b.iter(|| black_box(NodeAPI::query_nodes(&mut runtime, rare_tag_name.as_view())))
            },
        );
    }
    group.finish();
}

fn build_spatial_runtime(count: usize) -> Runtime {
    let mut runtime = build_query_runtime(count);
    let ids = NodeAPI::query_nodes(
        &mut runtime,
        NodeQuery {
            expr: None,
            scope: QueryScope::Root,
        }
        .as_view(),
    );
    for (i, id) in ids.into_iter().enumerate() {
        let _ = NodeAPI::set_global_transform_3d(
            &mut runtime,
            id,
            Transform3D {
                position: Vector3::new(i as f32, 0.0, 0.0),
                ..Transform3D::IDENTITY
            },
        );
    }
    runtime
}

fn bench_spatial_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("query/spatial");
    for count in [100usize, 2_500, 10_000, 50_000, 100_000] {
        // Box around the middle of the position range: ~200 nodes match.
        let origin = Vector3::new(count as f32 * 0.5, 0.0, 0.0);
        let size = Vector3::new(200.0, 10.0, 10.0);
        let within_only = NodeQuery {
            expr: Some(QueryExpr::Within(QueryBounds::Box3D { origin, size })),
            scope: QueryScope::Root,
        };
        let within_rare_tag = NodeQuery {
            expr: Some(QueryExpr::All(vec![
                QueryExpr::Tags(vec![TagID::from_string("boss")]),
                QueryExpr::Within(QueryBounds::Box3D { origin, size }),
            ])),
            scope: QueryScope::Root,
        };
        let within_broad_tag = NodeQuery {
            expr: Some(QueryExpr::All(vec![
                QueryExpr::Tags(vec![TagID::from_string("enemy")]),
                QueryExpr::Within(QueryBounds::Box3D { origin, size }),
            ])),
            scope: QueryScope::Root,
        };

        group.bench_with_input(
            BenchmarkId::new("within_only", count),
            &count,
            |b, &count| {
                let mut runtime = build_spatial_runtime(count);
                b.iter(|| black_box(NodeAPI::query_nodes(&mut runtime, within_only.as_view())))
            },
        );
        group.bench_with_input(
            BenchmarkId::new("within_rare_tag", count),
            &count,
            |b, &count| {
                let mut runtime = build_spatial_runtime(count);
                b.iter(|| {
                    black_box(NodeAPI::query_nodes(
                        &mut runtime,
                        within_rare_tag.as_view(),
                    ))
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("within_broad_tag", count),
            &count,
            |b, &count| {
                let mut runtime = build_spatial_runtime(count);
                b.iter(|| {
                    black_box(NodeAPI::query_nodes(
                        &mut runtime,
                        within_broad_tag.as_view(),
                    ))
                })
            },
        );
    }
    group.finish();
}

fn benches(c: &mut Criterion) {
    bench_rt_ctx_queries(c);
    bench_compile_repr_queries(c);
    bench_spatial_queries(c);
}

criterion_group! {
    name = query_hotpaths;
    config = Criterion::default().sample_size(10);
    targets = benches
}
criterion_main!(query_hotpaths);
