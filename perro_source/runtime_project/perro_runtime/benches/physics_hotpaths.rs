use criterion::{Criterion, black_box, criterion_group, criterion_main};
use perro_ids::NodeID;
use perro_runtime::Runtime;
use perro_runtime_api::sub_apis::{NodeAPI, PhysicsQueryFilter};
use perro_structs::{Transform2D, Transform3D, Vector2, Vector3};

fn bench_physics_scan_ids_clone_vs_scratch(c: &mut Criterion) {
    let ids: Vec<NodeID> = (1..=200_000).map(|i| NodeID::from_parts(i, 0)).collect();
    let mut group = c.benchmark_group("physics/scan_ids_clone_vs_scratch");
    group.bench_function("clone", |b| b.iter(|| black_box(black_box(&ids).clone())));
    group.bench_function("scratch_extend", |b| {
        let mut scratch = Vec::<NodeID>::new();
        b.iter(|| {
            scratch.clear();
            scratch.extend_from_slice(black_box(&ids));
            black_box(scratch.len())
        })
    });
    group.finish();
}

fn bench_physics_children_clone_vs_slice_scan(c: &mut Criterion) {
    let body_count = 100_000usize;
    let children_per_body = 4usize;
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

    let mut group = c.benchmark_group("physics/children_clone_vs_slice_scan");
    group.bench_function("clone_then_scan", |b| {
        b.iter(|| {
            let mut acc = 0u64;
            for ids in black_box(&children) {
                let copied = ids.to_vec();
                for id in &copied {
                    acc = acc.wrapping_add(id.as_u64());
                }
            }
            black_box(acc)
        })
    });
    group.bench_function("slice_scan", |b| {
        b.iter(|| {
            let mut acc = 0u64;
            for ids in black_box(&children) {
                for &id in ids {
                    acc = acc.wrapping_add(id.as_u64());
                }
            }
            black_box(acc)
        })
    });
    group.finish();
}

fn bench_physics_raycast_2d_query_filter(c: &mut Criterion) {
    c.bench_function("physics/raycast_2d_query_filter", |b| {
        let mut runtime = Runtime::new();
        for i in 0..256 {
            let body = NodeAPI::create::<perro_nodes::StaticBody2D>(&mut runtime);
            let shape = NodeAPI::create::<perro_nodes::CollisionShape2D>(&mut runtime);
            assert!(NodeAPI::reparent(&mut runtime, body, shape));
            let _ = NodeAPI::set_global_transform_2d(
                &mut runtime,
                body,
                Transform2D::new(Vector2::new(i as f32 * 2.0, 0.0), 0.0, Vector2::ONE),
            );
        }
        let filter = PhysicsQueryFilter::default();
        b.iter(|| {
            black_box(Runtime::physics_raycast_2d(
                &mut runtime,
                Vector2::new(-10.0, 0.0),
                Vector2::new(1.0, 0.0),
                1_000.0,
                &filter,
            ))
        })
    });
}

fn bench_physics_shape_cast_3d(c: &mut Criterion) {
    c.bench_function("physics/shape_cast_3d", |b| {
        let mut runtime = Runtime::new();
        for i in 0..128 {
            let body = NodeAPI::create::<perro_nodes::StaticBody3D>(&mut runtime);
            let shape = NodeAPI::create::<perro_nodes::CollisionShape3D>(&mut runtime);
            assert!(NodeAPI::reparent(&mut runtime, body, shape));
            let _ = NodeAPI::set_global_transform_3d(
                &mut runtime,
                body,
                Transform3D::new(
                    Vector3::new(0.0, 0.0, i as f32 * 2.0),
                    perro_structs::Quaternion::IDENTITY,
                    Vector3::ONE,
                ),
            );
        }
        let filter = PhysicsQueryFilter::default();
        b.iter(|| {
            black_box(Runtime::physics_shape_cast_3d(
                &mut runtime,
                perro_nodes::Shape3D::Sphere { radius: 0.25 },
                Vector3::new(0.0, 0.0, -5.0),
                Vector3::new(0.0, 0.0, 1.0),
                1_000.0,
                &filter,
            ))
        })
    });
}

fn benches(c: &mut Criterion) {
    bench_physics_scan_ids_clone_vs_scratch(c);
    bench_physics_children_clone_vs_slice_scan(c);
    bench_physics_raycast_2d_query_filter(c);
    bench_physics_shape_cast_3d(c);
}

criterion_group! {
    name = physics_hotpaths;
    config = Criterion::default().sample_size(10);
    targets = benches
}
criterion_main!(physics_hotpaths);
