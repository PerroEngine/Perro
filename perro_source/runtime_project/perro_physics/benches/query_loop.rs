use criterion::{Criterion, black_box, criterion_group, criterion_main};
use perro_ids::NodeID;
use perro_nodes::Shape2D;
use perro_physics::{BodyDesc2D, BodyKind, PhysicsSystem, ShapeDesc2D, ShapeKind2D};
use perro_runtime_api::sub_apis::PhysicsQueryFilter;
use perro_structs::{BitMask, Transform2D, Vector2};

fn static_body_2d(i: u32) -> BodyDesc2D {
    BodyDesc2D {
        id: NodeID::new(i + 1),
        kind: BodyKind::Static,
        enabled: true,
        global: Transform2D::new(
            Vector2::new((i % 64) as f32 * 2.0, (i / 64) as f32 * 2.0),
            0.0,
            Vector2::ONE,
        ),
        rigid: None,
        sync_signature: (i + 1) as u64,
        shape_signature: 1,
        shapes: vec![ShapeDesc2D {
            local: Transform2D::IDENTITY,
            shape: ShapeKind2D::Primitive(Shape2D::Circle { radius: 0.4 }),
            sensor: false,
            collision_layers: BitMask::ALL,
            collision_mask: BitMask::NONE,
            friction: 0.7,
            restitution: 0.1,
            density: 1.0,
        }],
    }
}

fn build_system(body_count: u32) -> PhysicsSystem {
    let bodies: Vec<_> = (0..body_count).map(static_body_2d).collect();
    let mut system = PhysicsSystem::new();
    system.sync_world_2d(&bodies, |_, _| {});
    system
}

fn cast_100(system: &mut PhysicsSystem, filter: &PhysicsQueryFilter) -> u32 {
    let mut hits = 0u32;
    for i in 0..100u32 {
        let origin = Vector2::new((i % 32) as f32 * 2.0, -5.0);
        if system
            .raycast_2d(origin, Vector2::new(0.0, 1.0), black_box(200.0), filter)
            .is_some()
        {
            hits += 1;
        }
    }
    hits
}

fn bench_query_loop(c: &mut Criterion) {
    let filter = PhysicsQueryFilter::default();

    // new path: pipeline refit 1x, casts reuse
    let mut system = build_system(1024);
    c.bench_function("raycast_2d_x100_static_world", |b| {
        b.iter(|| black_box(cast_100(&mut system, &filter)))
    });

    // old path sim: force refit b4 every cast (pre-fix behavior)
    let mut forced = build_system(1024);
    c.bench_function("raycast_2d_x100_forced_refit", |b| {
        b.iter(|| {
            let mut hits = 0u32;
            for i in 0..100u32 {
                forced.query_pipeline_dirty_2d = true;
                let origin = Vector2::new((i % 32) as f32 * 2.0, -5.0);
                if forced
                    .raycast_2d(origin, Vector2::new(0.0, 1.0), black_box(200.0), &filter)
                    .is_some()
                {
                    hits += 1;
                }
            }
            black_box(hits)
        })
    });
}

criterion_group!(benches, bench_query_loop);
criterion_main!(benches);
