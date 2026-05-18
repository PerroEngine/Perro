use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use perro_ids::NodeID;
use perro_nodes::{Shape2D, Shape3D};
use perro_physics::{
    BodyDesc2D, BodyDesc3D, BodyKind, PhysicsAssetContext, PhysicsProviderMode, PhysicsSystem,
    RigidProps2D, RigidProps3D, ShapeDesc2D, ShapeDesc3D, ShapeKind2D, ShapeKind3D,
};
use perro_structs::{BitMask, Quaternion, Transform2D, Transform3D, Vector2, Vector3};

const DT: f32 = 1.0 / 60.0;

fn asset_context() -> PhysicsAssetContext {
    PhysicsAssetContext {
        provider_mode: PhysicsProviderMode::Dynamic,
        static_mesh_lookup: None,
        static_collision_trimesh_lookup: None,
    }
}

fn rigid_props_2d(i: u32) -> RigidProps2D {
    RigidProps2D {
        enabled: true,
        can_sleep: false,
        lock_rotation: false,
        mass: 1.0,
        density: 1.0,
        continuous_collision_detection: false,
        linear_velocity: Vector2::new((i % 7) as f32 * 0.05, 0.0),
        angular_velocity: 0.0,
        gravity_scale: 1.0,
        linear_damping: 0.01,
        angular_damping: 0.01,
    }
}

fn rigid_props_3d(i: u32) -> RigidProps3D {
    RigidProps3D {
        enabled: true,
        can_sleep: false,
        mass: 1.0,
        density: 1.0,
        continuous_collision_detection: false,
        linear_velocity: Vector3::new((i % 7) as f32 * 0.05, 0.0, (i % 5) as f32 * -0.03),
        angular_velocity: Vector3::ZERO,
        gravity_scale: 1.0,
        linear_damping: 0.01,
        angular_damping: 0.01,
    }
}

fn shape_2d() -> ShapeDesc2D {
    ShapeDesc2D {
        local: Transform2D::IDENTITY,
        shape: ShapeKind2D::Primitive(Shape2D::Circle { radius: 0.35 }),
        sensor: false,
        collision_layers: BitMask::ALL,
        collision_mask: BitMask::NONE,
        friction: 0.7,
        restitution: 0.1,
        density: 1.0,
    }
}

fn shape_3d() -> ShapeDesc3D {
    ShapeDesc3D {
        local: Transform3D::new(Vector3::ZERO, Quaternion::IDENTITY, Vector3::ONE),
        shape: ShapeKind3D::Primitive(Shape3D::Sphere { radius: 0.35 }),
        sensor: false,
        collision_layers: BitMask::ALL,
        collision_mask: BitMask::NONE,
        friction: 0.7,
        restitution: 0.1,
        density: 1.0,
    }
}

fn bodies_2d(count: u32) -> Vec<BodyDesc2D> {
    let mut bodies = Vec::with_capacity(count as usize + 1);
    bodies.push(BodyDesc2D {
        id: NodeID::new(1),
        kind: BodyKind::Static,
        enabled: true,
        global: Transform2D::new(Vector2::new(0.0, -8.0), 0.0, Vector2::ONE),
        rigid: None,
        shape_signature: 1,
        shapes: vec![ShapeDesc2D {
            shape: ShapeKind2D::Primitive(Shape2D::Quad {
                width: count as f32,
                height: 1.0,
            }),
            ..shape_2d()
        }],
    });
    for i in 0..count {
        bodies.push(BodyDesc2D {
            id: NodeID::new(i + 2),
            kind: BodyKind::Rigid,
            enabled: true,
            global: Transform2D::new(
                Vector2::new((i % 64) as f32 - 32.0, 2.0 + (i / 64) as f32 * 1.25),
                0.0,
                Vector2::ONE,
            ),
            rigid: Some(rigid_props_2d(i)),
            shape_signature: 2,
            shapes: vec![shape_2d()],
        });
    }
    bodies
}

fn bodies_3d(count: u32) -> Vec<BodyDesc3D> {
    let mut bodies = Vec::with_capacity(count as usize + 1);
    bodies.push(BodyDesc3D {
        id: NodeID::new(1),
        kind: BodyKind::Static,
        enabled: true,
        global: Transform3D::new(
            Vector3::new(0.0, -8.0, 0.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        ),
        rigid: None,
        shape_signature: 1,
        shapes: vec![ShapeDesc3D {
            shape: ShapeKind3D::Primitive(Shape3D::Cube {
                size: Vector3::new(count as f32, 1.0, count as f32),
            }),
            ..shape_3d()
        }],
    });
    for i in 0..count {
        bodies.push(BodyDesc3D {
            id: NodeID::new(i + 2),
            kind: BodyKind::Rigid,
            enabled: true,
            global: Transform3D::new(
                Vector3::new(
                    (i % 16) as f32 - 8.0,
                    2.0 + (i / 256) as f32 * 1.25,
                    ((i / 16) % 16) as f32 - 8.0,
                ),
                Quaternion::IDENTITY,
                Vector3::ONE,
            ),
            rigid: Some(rigid_props_3d(i)),
            shape_signature: 2,
            shapes: vec![shape_3d()],
        });
    }
    bodies
}

fn system_2d(count: u32) -> PhysicsSystem {
    let mut system = PhysicsSystem::new();
    system.sync_world_2d(&bodies_2d(count), |_, _| {});
    system
}

fn system_3d(count: u32) -> PhysicsSystem {
    let mut system = PhysicsSystem::new();
    system.sync_world_3d(&bodies_3d(count), asset_context(), |_, _| {});
    system
}

fn bench_rapier_step_2d(c: &mut Criterion) {
    c.bench_function("rapier_core/step_2d_512", |b| {
        b.iter_batched(
            || system_2d(512),
            |mut system| {
                for i in 0..512 {
                    system.queue_force_2d(NodeID::new(i + 2), Vector2::new(0.4, 0.1));
                    system.queue_impulse_2d(NodeID::new(i + 2), Vector2::new(0.02, 0.01));
                }
                system.apply_pending_forces_2d(1.0, DT);
                system.apply_pending_impulses_2d(1.0);
                system.step_world_2d(-9.81, DT);
                black_box(system.world_2d.as_ref().unwrap().bodies.len())
            },
            BatchSize::LargeInput,
        )
    });
}

fn bench_rapier_step_3d(c: &mut Criterion) {
    c.bench_function("rapier_core/step_3d_512", |b| {
        b.iter_batched(
            || system_3d(512),
            |mut system| {
                for i in 0..512 {
                    system.queue_force_3d(NodeID::new(i + 2), Vector3::new(0.4, 0.1, -0.2));
                    system.queue_impulse_3d(NodeID::new(i + 2), Vector3::new(0.02, 0.01, 0.03));
                }
                system.apply_pending_forces_3d(1.0, DT);
                system.apply_pending_impulses_3d(1.0);
                system.step_world_3d(-9.81, DT);
                black_box(system.world_3d.as_ref().unwrap().bodies.len())
            },
            BatchSize::LargeInput,
        )
    });
}

fn bench_apply_pending_hot(c: &mut Criterion) {
    let mut group = c.benchmark_group("rapier_core/apply_pending");
    group.bench_function("forces_impulses_2d_4096", |b| {
        b.iter_batched(
            || {
                let mut system = system_2d(4096);
                for i in 0..4096 {
                    system.queue_force_2d(NodeID::new(i + 2), Vector2::new(0.4, 0.1));
                    system.queue_impulse_2d(NodeID::new(i + 2), Vector2::new(0.02, 0.01));
                }
                system
            },
            |mut system| {
                system.apply_pending_forces_2d(1.0, DT);
                system.apply_pending_impulses_2d(1.0);
                black_box(system.pending_forces_2d.len() + system.pending_impulses_2d.len())
            },
            BatchSize::LargeInput,
        )
    });
    group.bench_function("forces_impulses_3d_4096", |b| {
        b.iter_batched(
            || {
                let mut system = system_3d(4096);
                for i in 0..4096 {
                    system.queue_force_3d(NodeID::new(i + 2), Vector3::new(0.4, 0.1, -0.2));
                    system.queue_impulse_3d(NodeID::new(i + 2), Vector3::new(0.02, 0.01, 0.03));
                }
                system
            },
            |mut system| {
                system.apply_pending_forces_3d(1.0, DT);
                system.apply_pending_impulses_3d(1.0);
                black_box(system.pending_forces_3d.len() + system.pending_impulses_3d.len())
            },
            BatchSize::LargeInput,
        )
    });
    group.finish();
}

criterion_group! {
    name = rapier_core;
    config = Criterion::default().sample_size(10);
    targets = bench_rapier_step_2d, bench_rapier_step_3d, bench_apply_pending_hot
}
criterion_main!(rapier_core);
