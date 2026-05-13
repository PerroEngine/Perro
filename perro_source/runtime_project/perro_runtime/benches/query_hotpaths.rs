use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_ids::TagID;
use perro_nodes::{MeshInstance3D, Node3D, NodeType};
use perro_runtime::Runtime;
use perro_runtime_api::sub_apis::{NodeAPI, QueryExpr, QueryScope, QueryTypeMask, TagQuery};

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
    let selective = TagQuery {
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
    let broad = TagQuery {
        expr: Some(QueryExpr::Any(vec![QueryExpr::Tags(vec![
            TagID::from_string("enemy"),
            TagID::from_string("alive"),
        ])])),
        scope: QueryScope::Root,
    };

    let mut group = c.benchmark_group("query/rt_ctx_queries");
    for count in [2_500usize, 10_000, 50_000] {
        group.bench_with_input(BenchmarkId::new("selective", count), &count, |b, &count| {
            b.iter_batched(
                || build_query_runtime(count),
                |mut runtime| black_box(NodeAPI::query_nodes(&mut runtime, selective.clone())),
                criterion::BatchSize::LargeInput,
            )
        });
        group.bench_with_input(BenchmarkId::new("broad", count), &count, |b, &count| {
            b.iter_batched(
                || build_query_runtime(count),
                |mut runtime| black_box(NodeAPI::query_nodes(&mut runtime, broad.clone())),
                criterion::BatchSize::LargeInput,
            )
        });
    }
    group.finish();
}

fn bench_compile_repr_queries(c: &mut Criterion) {
    let mesh_type_mask = QueryTypeMask::NONE.with_type(NodeType::MeshInstance3D);
    let node3d_base_mask = QueryTypeMask::NONE.with_type(NodeType::Node3D);
    let enemy = TagID::from_string("enemy");
    let alive = TagID::from_string("alive");
    let boss = TagID::from_string("boss");

    let selective_vec = TagQuery {
        expr: Some(QueryExpr::All(vec![
            QueryExpr::IsType(vec![NodeType::MeshInstance3D]),
            QueryExpr::Name(vec!["enemy".to_string()]),
            QueryExpr::Tags(vec![enemy, alive, boss]),
        ])),
        scope: QueryScope::Root,
    };
    let selective_mask = TagQuery {
        expr: Some(QueryExpr::All(vec![
            QueryExpr::IsTypeMask(mesh_type_mask),
            QueryExpr::Name(vec!["enemy".to_string()]),
            QueryExpr::Tags(vec![enemy, alive, boss]),
        ])),
        scope: QueryScope::Root,
    };
    let type_vec = TagQuery {
        expr: Some(QueryExpr::All(vec![
            QueryExpr::IsType(vec![NodeType::MeshInstance3D]),
            QueryExpr::BaseType(vec![NodeType::Node3D]),
        ])),
        scope: QueryScope::Root,
    };
    let type_mask_query = TagQuery {
        expr: Some(QueryExpr::All(vec![
            QueryExpr::IsTypeMask(mesh_type_mask),
            QueryExpr::BaseTypeMask(node3d_base_mask),
        ])),
        scope: QueryScope::Root,
    };
    let not_type_vec = TagQuery {
        expr: Some(QueryExpr::Not(Box::new(QueryExpr::IsType(vec![
            NodeType::MeshInstance3D,
        ])))),
        scope: QueryScope::Root,
    };
    let not_type_mask = TagQuery {
        expr: Some(QueryExpr::Not(Box::new(QueryExpr::IsTypeMask(
            mesh_type_mask,
        )))),
        scope: QueryScope::Root,
    };

    let mut group = c.benchmark_group("query/compile_repr");
    for count in [2_500usize, 10_000, 50_000] {
        group.bench_with_input(
            BenchmarkId::new("selective_vec", count),
            &count,
            |b, &count| {
                let mut runtime = build_query_runtime(count);
                b.iter(|| black_box(NodeAPI::query_nodes(&mut runtime, selective_vec.clone())))
            },
        );
        group.bench_with_input(
            BenchmarkId::new("selective_mask", count),
            &count,
            |b, &count| {
                let mut runtime = build_query_runtime(count);
                b.iter(|| black_box(NodeAPI::query_nodes(&mut runtime, selective_mask.clone())))
            },
        );
        group.bench_with_input(BenchmarkId::new("type_vec", count), &count, |b, &count| {
            let mut runtime = build_query_runtime(count);
            b.iter(|| black_box(NodeAPI::query_nodes(&mut runtime, type_vec.clone())))
        });
        group.bench_with_input(BenchmarkId::new("type_mask", count), &count, |b, &count| {
            let mut runtime = build_query_runtime(count);
            b.iter(|| black_box(NodeAPI::query_nodes(&mut runtime, type_mask_query.clone())))
        });
        group.bench_with_input(
            BenchmarkId::new("not_type_vec", count),
            &count,
            |b, &count| {
                let mut runtime = build_query_runtime(count);
                b.iter(|| black_box(NodeAPI::query_nodes(&mut runtime, not_type_vec.clone())))
            },
        );
        group.bench_with_input(
            BenchmarkId::new("not_type_mask", count),
            &count,
            |b, &count| {
                let mut runtime = build_query_runtime(count);
                b.iter(|| black_box(NodeAPI::query_nodes(&mut runtime, not_type_mask.clone())))
            },
        );
    }
    group.finish();
}

fn benches(c: &mut Criterion) {
    bench_rt_ctx_queries(c);
    bench_compile_repr_queries(c);
}

criterion_group! {
    name = query_hotpaths;
    config = Criterion::default().sample_size(10);
    targets = benches
}
criterion_main!(query_hotpaths);
