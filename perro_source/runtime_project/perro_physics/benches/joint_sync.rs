use criterion::{Criterion, black_box, criterion_group, criterion_main};
use perro_ids::NodeID;
use perro_physics::{
    BodyDesc2D, BodyDesc3D, BodyKind, JointDesc2D, JointDesc3D, JointKind2D, JointKind3D,
    PhysicsAssetContext, PhysicsProviderMode, PhysicsSystem, joint_signature_2d,
    joint_signature_3d,
};
use perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};

fn body_2d(id: u32) -> BodyDesc2D {
    BodyDesc2D {
        id: NodeID::new(id),
        kind: BodyKind::Rigid,
        enabled: true,
        global: Transform2D::IDENTITY,
        rigid: None,
        shape_signature: 0,
        shapes: Vec::new(),
    }
}

fn body_3d(id: u32) -> BodyDesc3D {
    BodyDesc3D {
        id: NodeID::new(id),
        kind: BodyKind::Rigid,
        enabled: true,
        global: Transform3D::new(Vector3::ZERO, Quaternion::IDENTITY, Vector3::ONE),
        rigid: None,
        shape_signature: 0,
        shapes: Vec::new(),
    }
}

fn asset_context() -> PhysicsAssetContext {
    PhysicsAssetContext {
        provider_mode: PhysicsProviderMode::Dynamic,
        static_mesh_lookup: None,
        static_collision_trimesh_lookup: None,
    }
}

fn joint_2d(id: u32, body_a: u32, body_b: u32) -> JointDesc2D {
    let id = NodeID::new(id);
    let body_a = NodeID::new(body_a);
    let body_b = NodeID::new(body_b);
    let anchor_a = Vector2::ZERO;
    let anchor_b = Vector2::ZERO;
    let kind = JointKind2D::Fixed;
    JointDesc2D {
        id,
        body_a,
        body_b,
        anchor_a,
        anchor_b,
        enabled: true,
        collide_connected: false,
        kind,
        signature: joint_signature_2d(body_a, body_b, anchor_a, anchor_b, true, false, kind),
    }
}

fn joint_3d(id: u32, body_a: u32, body_b: u32) -> JointDesc3D {
    let id = NodeID::new(id);
    let body_a = NodeID::new(body_a);
    let body_b = NodeID::new(body_b);
    let anchor_a = Vector3::ZERO;
    let anchor_b = Vector3::ZERO;
    let kind = JointKind3D::Fixed;
    JointDesc3D {
        id,
        body_a,
        body_b,
        anchor_a,
        anchor_b,
        enabled: true,
        collide_connected: false,
        kind,
        signature: joint_signature_3d(body_a, body_b, anchor_a, anchor_b, true, false, kind),
    }
}

fn bench_joint_sync(c: &mut Criterion) {
    let bodies_2d = (0..1025).map(|i| body_2d(i + 1)).collect::<Vec<_>>();
    let joints_2d = (0..1024)
        .map(|i| joint_2d(10_000 + i, i + 1, i + 2))
        .collect::<Vec<_>>();
    let mut system_2d = PhysicsSystem::new();
    system_2d.sync_world_2d(&bodies_2d, |_, _| {});
    system_2d.sync_joints_2d(&joints_2d);

    c.bench_function("physics_joint_sync_2d_1024_hot", |b| {
        b.iter(|| {
            system_2d.sync_joints_2d(black_box(&joints_2d));
            black_box(system_2d.world_2d.as_ref().unwrap().joint_map.len())
        });
    });

    let bodies_3d = (0..1025).map(|i| body_3d(i + 1)).collect::<Vec<_>>();
    let joints_3d = (0..1024)
        .map(|i| joint_3d(10_000 + i, i + 1, i + 2))
        .collect::<Vec<_>>();
    let mut system_3d = PhysicsSystem::new();
    system_3d.sync_world_3d(&bodies_3d, asset_context(), |_, _| {});
    system_3d.sync_joints_3d(&joints_3d);

    c.bench_function("physics_joint_sync_3d_1024_hot", |b| {
        b.iter(|| {
            system_3d.sync_joints_3d(black_box(&joints_3d));
            black_box(system_3d.world_3d.as_ref().unwrap().joint_map.len())
        });
    });
}

criterion_group!(benches, bench_joint_sync);
criterion_main!(benches);
