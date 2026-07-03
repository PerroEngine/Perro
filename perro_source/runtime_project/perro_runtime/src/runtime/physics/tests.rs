use super::*;
use crate::runtime::render_2d::{
    ParsedTile2D, ParsedTileCollisionShape2D, ParsedTileset2D, TileSetShape2D,
};
use perro_nodes::{
    Area2D, Area3D, CollisionShape2D, CollisionShape3D, FixedJoint2D, FixedJoint3D, RigidBody2D,
    RigidBody3D, Sprite2D, StaticBody2D, StaticBody3D, WaterBody2D, WaterBody3D, WaterIdleMode,
    WaterShape, WaterSurfaceParams,
};
use perro_runtime_api::sub_apis::PhysicsAPI;
use perro_structs::CollisionPolicy;

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1.0e-4
}

fn quat_y(angle: f32) -> Quaternion {
    Quaternion::from_euler_xyz(0.0, angle, 0.0)
}

#[test]
fn physics_interp_2d_uses_prev_curr_alpha_and_keeps_scale() {
    let mut runtime = Runtime::new();
    let body_id = NodeAPI::create::<RigidBody2D>(&mut runtime);
    let prev = Transform2D::new(Vector2::new(0.0, 0.0), 0.0, Vector2::new(2.0, 3.0));
    let curr = Transform2D::new(
        Vector2::new(10.0, 0.0),
        std::f32::consts::PI,
        Vector2::new(2.0, 3.0),
    );

    runtime.record_physics_pose_2d(body_id, NodeID::nil(), prev, prev);
    runtime.record_physics_pose_2d(body_id, NodeID::nil(), prev, curr);
    runtime.set_physics_render_alpha(0.5);

    let render = runtime
        .get_render_global_transform_2d(body_id)
        .expect("render transform");
    assert!(approx(render.position.x, 5.0));
    assert!(approx(render.rotation, std::f32::consts::FRAC_PI_2));
    assert_eq!(render.scale, curr.scale);
}

#[test]
fn physics_interp_3d_uses_prev_curr_alpha_and_keeps_scale() {
    let mut runtime = Runtime::new();
    let body_id = NodeAPI::create::<RigidBody3D>(&mut runtime);
    let prev = Transform3D::new(
        Vector3::new(0.0, 0.0, 0.0),
        Quaternion::IDENTITY,
        Vector3::new(2.0, 3.0, 4.0),
    );
    let curr = Transform3D::new(
        Vector3::new(0.0, 0.0, 10.0),
        quat_y(std::f32::consts::PI),
        Vector3::new(2.0, 3.0, 4.0),
    );

    runtime.record_physics_pose_3d(body_id, NodeID::nil(), prev, prev);
    runtime.record_physics_pose_3d(body_id, NodeID::nil(), prev, curr);
    runtime.set_physics_render_alpha(0.5);

    let render = runtime
        .get_render_global_transform_3d(body_id)
        .expect("render transform");
    let forward = render.rotation.rotate_vector3(Vector3::new(0.0, 0.0, -1.0));
    assert!(approx(render.position.z, 5.0));
    assert!(approx(forward.x.abs(), 1.0));
    assert_eq!(render.scale, curr.scale);
}

#[test]
fn physics_interp_external_teleport_snaps_without_smear() {
    let mut runtime = Runtime::new();
    let body_id = NodeAPI::create::<RigidBody2D>(&mut runtime);
    let old = Transform2D::new(Vector2::new(0.0, 0.0), 0.0, Vector2::ONE);
    let teleported_scene = Transform2D::new(Vector2::new(100.0, 0.0), 0.0, Vector2::ONE);
    let curr = Transform2D::new(Vector2::new(110.0, 0.0), 0.0, Vector2::ONE);

    runtime.record_physics_pose_2d(body_id, NodeID::nil(), old, old);
    runtime.record_physics_pose_2d(body_id, NodeID::nil(), teleported_scene, curr);
    runtime.set_physics_render_alpha(0.5);

    let render = runtime
        .get_render_global_transform_2d(body_id)
        .expect("render transform");
    assert!(approx(render.position.x, 110.0));
}

#[test]
fn physics_interp_child_uses_interpolated_parent() {
    let mut runtime = Runtime::new();
    let body_id = NodeAPI::create::<RigidBody2D>(&mut runtime);
    let child_id = NodeAPI::create::<Sprite2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, body_id, child_id));
    runtime
        .with_node_mut::<Sprite2D, _, _>(child_id, |sprite| {
            sprite.transform.position = Vector2::new(2.0, 0.0);
        })
        .expect("child exists");

    let prev = Transform2D::new(Vector2::new(0.0, 0.0), 0.0, Vector2::ONE);
    let curr = Transform2D::new(Vector2::new(10.0, 0.0), 0.0, Vector2::ONE);
    runtime.record_physics_pose_2d(body_id, NodeID::nil(), prev, prev);
    runtime.record_physics_pose_2d(body_id, NodeID::nil(), prev, curr);
    runtime.set_physics_render_alpha(0.5);

    let render = runtime
        .get_render_global_transform_2d(child_id)
        .expect("child render transform");
    assert!(approx(render.position.x, 7.0));
}

#[test]
fn physics_interp_alpha_one_matches_auth_transform() {
    let mut runtime = Runtime::new();
    let body_id = NodeAPI::create::<RigidBody2D>(&mut runtime);
    let prev = Transform2D::new(Vector2::new(0.0, 0.0), 0.0, Vector2::ONE);
    let curr = Transform2D::new(Vector2::new(8.0, 0.0), 0.0, Vector2::ONE);

    runtime.record_physics_pose_2d(body_id, NodeID::nil(), prev, prev);
    runtime.record_physics_pose_2d(body_id, NodeID::nil(), prev, curr);
    runtime.set_physics_render_alpha(1.0);

    let render = runtime
        .get_render_global_transform_2d(body_id)
        .expect("render transform");
    assert!(approx(render.position.x, 8.0));
}

#[test]
fn physics_2d_body_desc_carries_mass_and_density() {
    let mut runtime = Runtime::new();
    let body_id = NodeAPI::create::<RigidBody2D>(&mut runtime);
    let shape_id = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, body_id, shape_id));

    if let Some(node) = runtime.nodes.get_mut(body_id)
        && let SceneNodeData::RigidBody2D(body) = &mut node.data
    {
        body.mass = 7.0;
        body.density = 0.25;
    }

    let descs = runtime.collect_body_descs_2d();
    let desc = descs
        .iter()
        .find(|desc| desc.id == body_id)
        .expect("body desc should exist");
    let rigid = desc.rigid.expect("rigid props should exist");
    assert_eq!(rigid.mass, 7.0);
    assert_eq!(rigid.density, 0.25);
    assert_eq!(desc.shapes[0].density, 0.25);
}

#[test]
fn physics_3d_body_desc_carries_mass_and_density() {
    let mut runtime = Runtime::new();
    let body_id = NodeAPI::create::<RigidBody3D>(&mut runtime);
    let shape_id = NodeAPI::create::<CollisionShape3D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, body_id, shape_id));

    if let Some(node) = runtime.nodes.get_mut(body_id)
        && let SceneNodeData::RigidBody3D(body) = &mut node.data
    {
        body.mass = 9.0;
        body.density = 0.5;
    }

    let descs = runtime.collect_body_descs_3d();
    let desc = descs
        .iter()
        .find(|desc| desc.id == body_id)
        .expect("body desc should exist");
    let rigid = desc.rigid.expect("rigid props should exist");
    assert_eq!(rigid.mass, 9.0);
    assert_eq!(rigid.density, 0.5);
    assert_eq!(desc.shapes[0].density, 0.5);
}

#[test]
fn collision_shape_3d_flip_signs_local_mesh_scale() {
    let mut shape = CollisionShape3D::new();
    shape.shape = Shape3D::TriMesh {
        source: "res://models/one_sided.pmesh".to_string(),
    };
    shape.transform.scale = Vector3::new(2.0, 3.0, 4.0);
    shape.flip_x = true;
    shape.flip_z = true;

    let desc = shape_desc_3d(&shape, 0.7, 0.0);

    assert_eq!(desc.local.scale, Vector3::new(-2.0, 3.0, -4.0));
    assert!(matches!(desc.shape, ShapeKind3D::TriMesh { .. }));
}

#[test]
fn collision_shape_3d_flip_changes_shape_signature() {
    let a = CollisionShape3D::new();
    let mut b = CollisionShape3D::new();
    b.flip_x = true;

    let base = body_signature_seed(BodyKind::Static);
    let sig_a = hash_collision_shape_3d(base, &a, BodyKind::Static, Vector3::ONE);
    let sig_b = hash_collision_shape_3d(base, &b, BodyKind::Static, Vector3::ONE);

    assert_ne!(sig_a, sig_b);
}

#[test]
fn water_3d_buoyancy_uses_density() {
    let mut runtime = Runtime::new();
    let water_id = NodeAPI::create::<WaterBody3D>(&mut runtime);
    let body_id = NodeAPI::create::<RigidBody3D>(&mut runtime);

    if let Some(node) = runtime.nodes.get_mut(water_id)
        && let SceneNodeData::WaterBody3D(water) = &mut node.data
    {
        water.water.physics.buoyancy = 2.0;
        water.water.physics.drag = 0.0;
    }

    if let Some(node) = runtime.nodes.get_mut(body_id)
        && let SceneNodeData::RigidBody3D(body) = &mut node.data
    {
        body.transform.position.y = -1.5;
        body.mass = 9.0;
        body.density = 0.25;
    }

    runtime.queue_water_forces_3d();

    let force = runtime
        .physics
        .pending_forces_3d
        .iter()
        .find(|pending| pending.id == body_id)
        .expect("water force should be queued")
        .force;
    assert!(force.y > 50.0);
}

#[test]
fn water_3d_buoyancy_recovers_body_below_depth() {
    let mut runtime = Runtime::new();
    let water_id = NodeAPI::create::<WaterBody3D>(&mut runtime);
    let body_id = NodeAPI::create::<RigidBody3D>(&mut runtime);

    if let Some(node) = runtime.nodes.get_mut(water_id)
        && let SceneNodeData::WaterBody3D(water) = &mut node.data
    {
        water.water.shape = WaterShape::box_volume(Vector3::new(12.0, 4.0, 12.0));
        water.water.depth = 4.0;
        water.water.physics.buoyancy = 4.0;
        water.water.physics.drag = 0.0;
    }

    if let Some(node) = runtime.nodes.get_mut(body_id)
        && let SceneNodeData::RigidBody3D(body) = &mut node.data
    {
        body.transform.position = Vector3::new(0.0, -6.0, 0.0);
        body.density = 1.0;
    }

    runtime.queue_water_forces_3d();

    let force = runtime
        .physics
        .pending_forces_3d
        .iter()
        .find(|pending| pending.id == body_id)
        .expect("deep body should still get recovery force")
        .force;
    assert!(force.y > 20.0);
}

#[test]
fn rotated_2d_water_uses_local_top_for_buoyancy() {
    let mut runtime = Runtime::new();
    let water_id = NodeAPI::create::<WaterBody2D>(&mut runtime);
    let body_id = NodeAPI::create::<RigidBody2D>(&mut runtime);

    if let Some(node) = runtime.nodes.get_mut(water_id)
        && let SceneNodeData::WaterBody2D(water) = &mut node.data
    {
        water.transform.rotation = std::f32::consts::FRAC_PI_2;
        water.water.shape = WaterShape::rect(Vector2::new(2.0, 10.0));
        water.water.physics.buoyancy = 1.0;
        water.water.physics.drag = 0.0;
    }

    if let Some(node) = runtime.nodes.get_mut(body_id)
        && let SceneNodeData::RigidBody2D(body) = &mut node.data
    {
        body.transform.position = Vector2::new(4.0, 0.0);
        body.density = 1.0;
    }

    runtime.queue_water_forces_2d();

    let force = runtime
        .physics
        .pending_forces_2d
        .iter()
        .find(|pending| pending.id == body_id)
        .expect("rotated water force should be queued")
        .force;
    assert!(force.x < -3.9);
    assert!(force.y.abs() < 0.01);
}

#[test]
fn rotated_3d_water_uses_local_top_for_buoyancy() {
    let mut runtime = Runtime::new();
    let water_id = NodeAPI::create::<WaterBody3D>(&mut runtime);
    let body_id = NodeAPI::create::<RigidBody3D>(&mut runtime);

    if let Some(node) = runtime.nodes.get_mut(water_id)
        && let SceneNodeData::WaterBody3D(water) = &mut node.data
    {
        water.transform.rotation =
            Quaternion::from_euler_xyz(0.0, 0.0, std::f32::consts::FRAC_PI_2);
        water.water.shape = WaterShape::box_volume(Vector3::new(4.0, 6.0, 4.0));
        water.water.physics.buoyancy = 1.0;
        water.water.physics.drag = 0.0;
    }

    if let Some(node) = runtime.nodes.get_mut(body_id)
        && let SceneNodeData::RigidBody3D(body) = &mut node.data
    {
        body.transform.position = Vector3::new(2.0, 0.0, 0.0);
        body.density = 1.0;
    }

    runtime.queue_water_forces_3d();

    let force = runtime
        .physics
        .pending_forces_3d
        .iter()
        .find(|pending| pending.id == body_id)
        .expect("rotated water force should be queued")
        .force;
    assert!(force.x < -1.9);
    assert!(force.y.abs() < 0.01);
}

#[test]
fn overlapping_2d_waters_blend_buoyancy_once() {
    let mut runtime = Runtime::new();
    let water_a = NodeAPI::create::<WaterBody2D>(&mut runtime);
    let water_b = NodeAPI::create::<WaterBody2D>(&mut runtime);
    let body_id = NodeAPI::create::<RigidBody2D>(&mut runtime);

    if let Some(node) = runtime.nodes.get_mut(water_a)
        && let SceneNodeData::WaterBody2D(water) = &mut node.data
    {
        water.water.shape = WaterShape::rect(Vector2::new(16.0, 16.0));
        water.water.physics.buoyancy = 2.0;
        water.water.physics.drag = 0.0;
    }
    if let Some(node) = runtime.nodes.get_mut(water_b)
        && let SceneNodeData::WaterBody2D(water) = &mut node.data
    {
        water.transform.position.x = 4.0;
        water.transform.position.y = 2.0;
        water.water.shape = WaterShape::rect(Vector2::new(16.0, 16.0));
        water.water.physics.buoyancy = 2.0;
        water.water.physics.drag = 0.0;
    }
    if let Some(node) = runtime.nodes.get_mut(body_id)
        && let SceneNodeData::RigidBody2D(body) = &mut node.data
    {
        body.transform.position = Vector2::new(2.0, -1.0);
        body.density = 1.0;
    }

    runtime.queue_water_forces_2d();

    let total: f32 = runtime
        .physics
        .pending_forces_2d
        .iter()
        .filter(|pending| pending.id == body_id)
        .map(|pending| pending.force.y)
        .sum();
    assert!(total > 2.0);
    assert!(total < 64.0);
}

#[test]
fn overlapping_3d_waters_blend_buoyancy_once() {
    let mut runtime = Runtime::new();
    let water_a = NodeAPI::create::<WaterBody3D>(&mut runtime);
    let water_b = NodeAPI::create::<WaterBody3D>(&mut runtime);
    let body_id = NodeAPI::create::<RigidBody3D>(&mut runtime);

    if let Some(node) = runtime.nodes.get_mut(water_a)
        && let SceneNodeData::WaterBody3D(water) = &mut node.data
    {
        water.water.shape = WaterShape::box_volume(Vector3::new(16.0, 4.0, 16.0));
        water.water.depth = 4.0;
        water.water.physics.buoyancy = 2.0;
        water.water.physics.drag = 0.0;
    }
    if let Some(node) = runtime.nodes.get_mut(water_b)
        && let SceneNodeData::WaterBody3D(water) = &mut node.data
    {
        water.transform.position.x = 4.0;
        water.transform.position.y = 2.0;
        water.water.shape = WaterShape::box_volume(Vector3::new(16.0, 4.0, 16.0));
        water.water.depth = 4.0;
        water.water.physics.buoyancy = 2.0;
        water.water.physics.drag = 0.0;
    }
    if let Some(node) = runtime.nodes.get_mut(body_id)
        && let SceneNodeData::RigidBody3D(body) = &mut node.data
    {
        body.transform.position = Vector3::new(2.0, -1.0, 0.0);
        body.density = 1.0;
    }

    runtime.queue_water_forces_3d();

    let total: f32 = runtime
        .physics
        .pending_forces_3d
        .iter()
        .filter(|pending| pending.id == body_id)
        .map(|pending| pending.force.y)
        .sum();
    assert!(total > 2.0);
    assert!(total < 96.0);
}

#[test]
fn rigid_body_crossing_2d_link_boundary_keeps_water_force() {
    let mut runtime = Runtime::new();
    let water_a = NodeAPI::create::<WaterBody2D>(&mut runtime);
    let water_b = NodeAPI::create::<WaterBody2D>(&mut runtime);
    let body_id = NodeAPI::create::<RigidBody2D>(&mut runtime);

    for (id, x) in [(water_a, 0.0), (water_b, 14.0)] {
        if let Some(node) = runtime.nodes.get_mut(id)
            && let SceneNodeData::WaterBody2D(water) = &mut node.data
        {
            water.transform.position.x = x;
            water.water.shape = WaterShape::rect(Vector2::new(16.0, 16.0));
            water.water.physics.buoyancy = 1.5;
            water.water.physics.drag = 0.0;
        }
    }
    if let Some(node) = runtime.nodes.get_mut(body_id)
        && let SceneNodeData::RigidBody2D(body) = &mut node.data
    {
        body.transform.position = Vector2::new(7.0, -1.0);
        body.density = 1.0;
    }
    runtime.queue_water_forces_2d();
    let boundary_force: f32 = runtime
        .physics
        .pending_forces_2d
        .iter()
        .filter(|pending| pending.id == body_id)
        .map(|pending| pending.force.y)
        .sum();
    runtime.physics.pending_forces_2d.clear();
    if let Some(node) = runtime.nodes.get_mut(body_id)
        && let SceneNodeData::RigidBody2D(body) = &mut node.data
    {
        body.transform.position.x = 14.0;
    }
    runtime.queue_water_forces_2d();
    let after_cross_force: f32 = runtime
        .physics
        .pending_forces_2d
        .iter()
        .filter(|pending| pending.id == body_id)
        .map(|pending| pending.force.y)
        .sum();

    assert!(boundary_force > 0.0);
    assert!(after_cross_force > 0.0);
}

#[test]
fn floating_body_tracks_wave_vertical_velocity() {
    let mut surface = WaterSurfaceParams {
        shape: WaterShape::rect(Vector2::new(16.0, 16.0)),
        idle_mode: WaterIdleMode::Sine,
        ..WaterSurfaceParams::default()
    };
    surface.wave.speed = 20.0;
    surface.wave.scale = 4.0;
    surface.physics.drag = 0.0;
    surface.physics.buoyancy = 1.0;
    surface.physics.wake_strength = 1.0;

    let local = Vector2::ZERO;
    let elapsed = 0.5;
    let sample = water_physics_sample_for_body(&surface, local, elapsed);
    assert!(sample.velocity.y.abs() > 0.01);

    let water = RuntimeWater2D {
        id: NodeID::from_parts(1, 0),
        half: Vector2::new(8.0, 8.0),
        transform: Mat3::IDENTITY,
        inv_transform: Mat3::IDENTITY,
        normal: Vector2::new(0.0, 1.0),
        min_x: -8.0,
        max_x: 8.0,
        surface,
    };
    let water_index = RuntimeWaterIndex2D::new(vec![water]);
    let submerged = water_target_submerged(1.0);
    let body_base = RuntimeWaterBody2D {
        id: NodeID::from_parts(90, 0),
        pos: Vector2::new(0.0, sample.height - submerged),
        velocity: Vector2::ZERO,
        mass: 1.0,
        density: 1.0,
        float_radius: 0.0,
        sleeping: false,
        collision_layers: BitMask::ALL,
        collision_mask: BitMask::NONE,
    };
    let still_force = water_forces_for_body_2d(
        body_base,
        &water_index,
        &AHashMap::new(),
        &AHashMap::new(),
        elapsed,
        Vector2::ZERO,
    )[0]
    .force;
    let matched_force = water_forces_for_body_2d(
        RuntimeWaterBody2D {
            velocity: Vector2::new(0.0, sample.velocity.y),
            ..body_base
        },
        &water_index,
        &AHashMap::new(),
        &AHashMap::new(),
        elapsed,
        Vector2::ZERO,
    )[0]
    .force;

    assert!((still_force.y - matched_force.y).abs() > 0.25);
}

#[test]
fn water_physics_uses_cached_visual_height_offset() {
    let surface = WaterSurfaceParams {
        shape: WaterShape::rect(Vector2::new(16.0, 16.0)),
        idle_mode: WaterIdleMode::Sine,
        ..WaterSurfaceParams::default()
    };
    let local = Vector2::new(2.0, -1.0);
    let elapsed = 0.25;
    let analytic = water_physics_sample_for_body(&surface, local, elapsed);
    let cached = perro_nodes::WaterPhysicsSample {
        height: 0.75,
        velocity: Vector2::ZERO,
        foam: 0.6,
    };
    let synced = water_physics_sample_for_body_cached(&surface, local, elapsed, None, Some(cached));

    assert!((synced.height - analytic.height - 0.75).abs() < 0.001);
    assert_eq!(synced.foam, 0.6);
}

#[test]
fn floating_body_gets_mass_scaled_wave_drive() {
    let mut surface = WaterSurfaceParams {
        shape: WaterShape::rect(Vector2::new(16.0, 16.0)),
        flow: Vector2::new(2.0, 0.0),
        ..WaterSurfaceParams::default()
    };
    surface.physics.drag = 0.0;
    surface.physics.buoyancy = 1.0;
    surface.physics.wake_strength = 2.0;

    let water = RuntimeWater2D {
        id: NodeID::from_parts(1, 0),
        half: Vector2::new(8.0, 8.0),
        transform: Mat3::IDENTITY,
        inv_transform: Mat3::IDENTITY,
        normal: Vector2::new(0.0, 1.0),
        min_x: -8.0,
        max_x: 8.0,
        surface,
    };
    let water_index = RuntimeWaterIndex2D::new(vec![water]);
    let sample = water_physics_sample_for_body(&surface, Vector2::ZERO, 0.0);
    let submerged = water_target_submerged(1.0);
    let body = RuntimeWaterBody2D {
        id: NodeID::from_parts(91, 0),
        pos: Vector2::new(0.0, sample.height - submerged),
        velocity: Vector2::ZERO,
        mass: 1.0,
        density: 1.0,
        float_radius: 0.0,
        sleeping: false,
        collision_layers: BitMask::ALL,
        collision_mask: BitMask::NONE,
    };
    let light = water_forces_for_body_2d(
        body,
        &water_index,
        &AHashMap::new(),
        &AHashMap::new(),
        0.0,
        Vector2::ZERO,
    )[0]
    .force;
    let heavy = water_forces_for_body_2d(
        RuntimeWaterBody2D { mass: 4.0, ..body },
        &water_index,
        &AHashMap::new(),
        &AHashMap::new(),
        0.0,
        Vector2::ZERO,
    )[0]
    .force;

    assert!(light.x > 0.0);
    assert!(heavy.x > light.x);
}

#[test]
fn deeply_submerged_3d_body_gets_enough_lift_to_leave_bed() {
    let mut surface = WaterSurfaceParams {
        shape: WaterShape::rect(Vector2::new(16.0, 16.0)),
        depth: 4.0,
        ..WaterSurfaceParams::default()
    };
    surface.physics.buoyancy = 4.0;
    surface.physics.drag = 0.55;

    let water = RuntimeWater3D {
        id: NodeID::from_parts(1, 0),
        half: Vector2::new(8.0, 8.0),
        transform: Mat4::IDENTITY,
        inv_transform: Mat4::IDENTITY,
        normal: Vector3::new(0.0, 1.0, 0.0),
        min_x: -8.0,
        max_x: 8.0,
        surface,
    };
    let water_index = RuntimeWaterIndex3D::new(vec![water]);
    let body = RuntimeWaterBody3D {
        id: NodeID::from_parts(92, 0),
        pos: Vector3::new(0.0, -1.2, 0.0),
        velocity: Vector3::ZERO,
        mass: 2.0,
        density: 1.0,
        float_radius: 0.45,
        sleeping: false,
        collision_layers: BitMask::ALL,
        collision_mask: BitMask::NONE,
    };

    let total_force: f32 = water_forces_for_body_3d(
        body,
        &water_index,
        &AHashMap::new(),
        &AHashMap::new(),
        0.0,
        Vector2::ZERO,
    )
    .iter()
    .map(|effect| effect.force.y)
    .sum();

    assert!(total_force > body.mass * 9.81 * 4.0);
}

#[test]
fn downward_surface_entry_creates_water_splash() {
    let mut surface = WaterSurfaceParams {
        shape: WaterShape::rect(Vector2::new(16.0, 16.0)),
        ..WaterSurfaceParams::default()
    };
    surface.physics.wake_strength = 2.0;

    let water = RuntimeWater2D {
        id: NodeID::from_parts(1, 0),
        half: Vector2::new(8.0, 8.0),
        transform: Mat3::IDENTITY,
        inv_transform: Mat3::IDENTITY,
        normal: Vector2::new(0.0, 1.0),
        min_x: -8.0,
        max_x: 8.0,
        surface,
    };
    let water_index = RuntimeWaterIndex2D::new(vec![water]);
    let body = RuntimeWaterBody2D {
        id: NodeID::from_parts(92, 0),
        pos: Vector2::new(0.0, -0.05),
        velocity: Vector2::new(0.0, -3.0),
        mass: 2.0,
        density: 1.0,
        float_radius: 0.5,
        sleeping: false,
        collision_layers: BitMask::ALL,
        collision_mask: BitMask::NONE,
    };

    let impacts = water_body_splashes_2d(&[body], &water_index, &AHashMap::new(), 0.0);
    assert_eq!(impacts.len(), 1);
    assert!(impacts[0].strength > 0.0);
    assert!(impacts[0].cavitation > 0.0);

    let floating = RuntimeWaterBody2D {
        velocity: Vector2::ZERO,
        ..body
    };
    assert!(water_body_splashes_2d(&[floating], &water_index, &AHashMap::new(), 0.0).is_empty());
}

#[test]
fn water_link_mask_blocks_2d_blend() {
    let mut a = WaterSurfaceParams::default();
    let mut b = WaterSurfaceParams::default();
    a.link.link_mask = BitMask::with([1]);
    b.link.link_layers = BitMask::with([1]);

    let wa = RuntimeWater2D {
        id: NodeID::from_parts(1, 0),
        half: a.shape.surface_size() * 0.5,
        transform: glam::Mat3::IDENTITY,
        inv_transform: glam::Mat3::IDENTITY,
        normal: Vector2::new(0.0, 1.0),
        min_x: -a.shape.surface_size().x * 0.5,
        max_x: a.shape.surface_size().x * 0.5,
        surface: a,
    };
    let wb = RuntimeWater2D {
        id: NodeID::from_parts(2, 0),
        half: b.shape.surface_size() * 0.5,
        transform: glam::Mat3::from_translation(glam::Vec2::new(4.0, 0.0)),
        inv_transform: glam::Mat3::from_translation(glam::Vec2::new(4.0, 0.0)).inverse(),
        normal: Vector2::new(0.0, 1.0),
        min_x: 4.0 - b.shape.surface_size().x * 0.5,
        max_x: 4.0 + b.shape.surface_size().x * 0.5,
        surface: b,
    };

    assert!(!water_linked_2d(wa, wb));
}

#[test]
fn smoothstep_midpoint_is_cubic_half() {
    assert_eq!(smoothstep(0.0), 0.0);
    assert_eq!(smoothstep(1.0), 1.0);
    assert!((smoothstep(0.5) - 0.5).abs() < 0.001);
}

#[test]
fn apply_force_2d_uses_world_space_vector() {
    let mut runtime = Runtime::new();
    let body_id = NodeAPI::create::<RigidBody2D>(&mut runtime);
    let shape_id = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, body_id, shape_id));

    if let Some(node) = runtime.nodes.get_mut(body_id)
        && let SceneNodeData::RigidBody2D(body) = &mut node.data
    {
        body.gravity_scale = 0.0;
        body.transform.rotation = std::f32::consts::FRAC_PI_2;
    }

    assert!(PhysicsAPI::apply_force_2d(
        &mut runtime,
        body_id,
        Vector2::new(60.0, 0.0)
    ));
    runtime.physics_fixed_step();

    let velocity = runtime
        .nodes
        .get(body_id)
        .and_then(|node| {
            let SceneNodeData::RigidBody2D(body) = &node.data else {
                return None;
            };
            Some(body.linear_velocity)
        })
        .expect("body should exist");
    assert!(velocity.x > 0.0);
    assert!(velocity.y.abs() < 0.001);
}

#[test]
fn apply_force_3d_uses_world_space_vector() {
    let mut runtime = Runtime::new();
    let body_id = NodeAPI::create::<RigidBody3D>(&mut runtime);
    let shape_id = NodeAPI::create::<CollisionShape3D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, body_id, shape_id));

    if let Some(node) = runtime.nodes.get_mut(body_id)
        && let SceneNodeData::RigidBody3D(body) = &mut node.data
    {
        body.gravity_scale = 0.0;
        body.transform.rotation = Quaternion::from_euler_xyz(0.0, 0.0, std::f32::consts::FRAC_PI_2);
    }

    assert!(PhysicsAPI::apply_force_3d(
        &mut runtime,
        body_id,
        Vector3::new(60.0, 0.0, 0.0)
    ));
    runtime.physics_fixed_step();

    let velocity = runtime
        .nodes
        .get(body_id)
        .and_then(|node| {
            let SceneNodeData::RigidBody3D(body) = &node.data else {
                return None;
            };
            Some(body.linear_velocity)
        })
        .expect("body should exist");
    assert!(velocity.x > 0.0);
    assert!(velocity.y.abs() < 0.001);
    assert!(velocity.z.abs() < 0.001);
}

#[test]
fn sleeping_rigidbody_2d_sync_settles_once_then_skips() {
    let mut runtime = Runtime::new();
    let body_id = NodeAPI::create::<RigidBody2D>(&mut runtime);
    let shape_id = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, body_id, shape_id));

    runtime.propagate_pending_transform_dirty();
    runtime.refresh_dirty_global_transforms();
    let bodies = runtime.collect_body_descs_2d();
    runtime.sync_world_2d(&bodies);

    {
        let world = runtime.physics.world_2d.as_mut().expect("2d world");
        let state = world.body_map.get(&body_id).expect("2d body state");
        world.bodies.get_mut(state.handle).expect("2d body").sleep();
    }

    assert!(runtime.sync_world_to_nodes_2d());
    let pose = runtime.transforms.physics_pose_2d[body_id.index() as usize];
    assert_eq!(pose.prev, pose.curr);

    let idle_frames = runtime
        .physics
        .world_2d
        .as_ref()
        .and_then(|world| world.body_map.get(&body_id))
        .map(|state| state.idle_sync_frames)
        .expect("2d idle sync state");
    assert_eq!(idle_frames, 1);

    assert!(!runtime.sync_world_to_nodes_2d());
}

#[test]
fn sleeping_rigidbody_3d_sync_settles_once_then_skips() {
    let mut runtime = Runtime::new();
    let body_id = NodeAPI::create::<RigidBody3D>(&mut runtime);
    let shape_id = NodeAPI::create::<CollisionShape3D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, body_id, shape_id));

    runtime.propagate_pending_transform_dirty();
    runtime.refresh_dirty_global_transforms();
    let bodies = runtime.collect_body_descs_3d();
    runtime.sync_world_3d(&bodies);

    {
        let world = runtime.physics.world_3d.as_mut().expect("3d world");
        let state = world.body_map.get(&body_id).expect("3d body state");
        world.bodies.get_mut(state.handle).expect("3d body").sleep();
    }

    assert!(runtime.sync_world_to_nodes_3d());
    let pose = runtime.transforms.physics_pose_3d[body_id.index() as usize];
    assert_eq!(pose.prev, pose.curr);

    let idle_frames = runtime
        .physics
        .world_3d
        .as_ref()
        .and_then(|world| world.body_map.get(&body_id))
        .map(|state| state.idle_sync_frames)
        .expect("3d idle sync state");
    assert_eq!(idle_frames, 1);

    assert!(!runtime.sync_world_to_nodes_3d());
}

#[test]
fn custom_force_vectors_interpolate_by_radius() {
    let mut emitter = perro_nodes::PhysicsForceEmitter2D::new();
    emitter.profile = perro_nodes::PhysicsForceProfile::Custom;
    emitter.radius = 10.0;
    emitter.strength = 2.0;
    emitter.vectors = vec![
        Vector2::new(0.0, 20.0),
        Vector2::new(4.0, 15.0),
        Vector2::new(8.0, 0.0),
    ];

    let force = force_emitter_force_2d(&emitter, Vector2::new(5.0, 0.0), 5.0);
    assert_eq!(force, Vector2::new(8.0, 30.0));
}

#[test]
fn force_emitter_2d_lift_queues_upward_force() {
    let mut runtime = Runtime::new();
    let emitter_id = NodeAPI::create::<perro_nodes::PhysicsForceEmitter2D>(&mut runtime);
    let body_id = NodeAPI::create::<RigidBody2D>(&mut runtime);

    if let Some(node) = runtime.nodes.get_mut(emitter_id)
        && let SceneNodeData::PhysicsForceEmitter2D(emitter) = &mut node.data
    {
        emitter.profile = perro_nodes::PhysicsForceProfile::Lift;
        emitter.radius = 10.0;
        emitter.strength = 12.0;
    }
    if let Some(node) = runtime.nodes.get_mut(body_id)
        && let SceneNodeData::RigidBody2D(body) = &mut node.data
    {
        body.transform.position = Vector2::new(0.0, 2.0);
    }

    runtime.queue_physics_force_emitters_2d();

    let force = runtime
        .physics
        .pending_forces_2d
        .iter()
        .find(|pending| pending.id == body_id)
        .expect("lift force should queue")
        .force;
    assert!(force.y > 0.0);
    assert_eq!(force.x, 0.0);
}

#[test]
fn force_emitter_near_water_creates_cavitation_impact() {
    let mut runtime = Runtime::new();
    let water_id = NodeAPI::create::<WaterBody2D>(&mut runtime);
    let emitter_id = NodeAPI::create::<perro_nodes::PhysicsForceEmitter2D>(&mut runtime);

    if let Some(node) = runtime.nodes.get_mut(water_id)
        && let SceneNodeData::WaterBody2D(water) = &mut node.data
    {
        water.water.shape = WaterShape::rect(Vector2::new(16.0, 16.0));
    }
    if let Some(node) = runtime.nodes.get_mut(emitter_id)
        && let SceneNodeData::PhysicsForceEmitter2D(emitter) = &mut node.data
    {
        emitter.profile = perro_nodes::PhysicsForceProfile::Explosion;
        emitter.radius = 6.0;
        emitter.strength = 32.0;
        emitter.affect_bodies = false;
    }

    runtime.queue_physics_force_emitters_2d();

    assert!(
        runtime
            .force_water_impacts_2d
            .iter()
            .any(|impact| impact.cavitation > 0.0)
    );
}

#[test]
fn emitted_force_2d_affects_nearby_body_and_water() {
    let mut runtime = Runtime::new();
    let water_id = NodeAPI::create::<WaterBody2D>(&mut runtime);
    let body_id = NodeAPI::create::<RigidBody2D>(&mut runtime);

    if let Some(node) = runtime.nodes.get_mut(water_id)
        && let SceneNodeData::WaterBody2D(water) = &mut node.data
    {
        water.water.shape = WaterShape::rect(Vector2::new(16.0, 16.0));
    }
    if let Some(node) = runtime.nodes.get_mut(body_id)
        && let SceneNodeData::RigidBody2D(body) = &mut node.data
    {
        body.transform.position = Vector2::new(2.0, 0.0);
        body.gravity_scale = 0.0;
    }

    let mut emitter = perro_nodes::PhysicsForceEmitter2D::new();
    emitter.profile = perro_nodes::PhysicsForceProfile::Current;
    emitter.transform.position = Vector2::new(0.0, 0.0);
    emitter.radius = 6.0;
    emitter.strength = 20.0;
    emitter.vectors = vec![Vector2::new(1.0, 0.0)];

    assert!(PhysicsAPI::emit_force_2d(&mut runtime, emitter));
    runtime.queue_physics_force_emitters_2d();

    assert!(
        runtime
            .physics
            .pending_forces_2d
            .iter()
            .any(|pending| pending.id == body_id && pending.force.x > 0.0)
    );
    assert!(
        runtime
            .force_water_impacts_2d
            .iter()
            .any(|impact| impact.strength > 0.0 && impact.cavitation > 0.0)
    );
}

#[test]
fn emitted_force_3d_affects_nearby_body_and_water() {
    let mut runtime = Runtime::new();
    let _water_id = NodeAPI::create::<WaterBody3D>(&mut runtime);
    let body_id = NodeAPI::create::<RigidBody3D>(&mut runtime);

    if let Some(node) = runtime.nodes.get_mut(body_id)
        && let SceneNodeData::RigidBody3D(body) = &mut node.data
    {
        body.transform.position = Vector3::new(2.0, -1.0, 0.0);
        body.gravity_scale = 0.0;
    }

    let mut emitter = perro_nodes::PhysicsForceEmitter3D::new();
    emitter.profile = perro_nodes::PhysicsForceProfile::Current;
    emitter.transform.position = Vector3::new(0.0, -1.0, 0.0);
    emitter.radius = 6.0;
    emitter.strength = 20.0;
    emitter.pulse = false;
    emitter.vectors = vec![Vector3::new(1.0, 0.0, 0.0)];

    assert!(PhysicsAPI::emit_force_3d(&mut runtime, emitter));
    runtime.queue_physics_force_emitters_3d();

    assert!(
        runtime
            .physics
            .pending_forces_3d
            .iter()
            .any(|pending| pending.id == body_id && pending.force.x > 0.0)
    );
    assert!(
        runtime
            .force_water_impacts_3d
            .iter()
            .any(|impact| impact.strength > 0.0 && impact.cavitation > 0.0)
    );
}

#[test]
fn physics_raycast_3d_hits_static_body() {
    let mut runtime = Runtime::new();
    let body = NodeAPI::create::<StaticBody3D>(&mut runtime);
    let shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, body, shape));

    let hit = runtime
        .physics_raycast_3d(
            Vector3::new(0.0, 0.0, -5.0),
            Vector3::new(0.0, 0.0, 1.0),
            10.0,
            false,
        )
        .expect("ray should hit cube");

    assert_eq!(hit.node, body);
    assert!((hit.distance - 4.5).abs() < 0.001);
    assert!((hit.point.z + 0.5).abs() < 0.001);
    assert!(hit.normal.z < -0.9);
}

#[test]
fn physics_raycast_3d_hits_area_with_collision_shape() {
    let mut runtime = Runtime::new();

    let static_body = NodeAPI::create::<StaticBody3D>(&mut runtime);
    let static_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, static_body, static_shape));

    let area = NodeAPI::create::<Area3D>(&mut runtime);
    let area_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, area, area_shape));
    let _ = <Runtime as NodeAPI>::set_global_transform_3d(
        &mut runtime,
        area,
        Transform3D::new(
            Vector3::new(0.0, 0.0, -2.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        ),
    );

    let area_hit = runtime
        .physics_raycast_3d(
            Vector3::new(0.0, 0.0, -5.0),
            Vector3::new(0.0, 0.0, 1.0),
            10.0,
            true,
        )
        .expect("ray should hit area first");
    assert_eq!(area_hit.node, area);
    assert!((area_hit.distance - 2.5).abs() < 0.001);

    let no_area_hit = runtime
        .physics_raycast_3d(
            Vector3::new(0.0, 0.0, -5.0),
            Vector3::new(0.0, 0.0, 1.0),
            10.0,
            false,
        )
        .expect("ray should skip area and hit static body");
    assert_eq!(no_area_hit.node, static_body);
}

#[test]
fn physics_raycast_3d_filter_uses_collision_policy() {
    let mut runtime = Runtime::new();

    let body_a = NodeAPI::create::<StaticBody3D>(&mut runtime);
    let shape_a = NodeAPI::create::<CollisionShape3D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, body_a, shape_a));
    let _ = <Runtime as NodeAPI>::set_global_transform_3d(
        &mut runtime,
        body_a,
        Transform3D::new(
            Vector3::new(0.0, 0.0, -2.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        ),
    );

    let body_b = NodeAPI::create::<StaticBody3D>(&mut runtime);
    let shape_b = NodeAPI::create::<CollisionShape3D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, body_b, shape_b));

    let mask_a = CollisionPolicy::new(CollisionPolicy::layer(3), CollisionPolicy::layer(4));
    let mask_b = CollisionPolicy::new(CollisionPolicy::layer(4), CollisionPolicy::layer(3));
    assert!(!mask_a.can_collide(mask_b));

    if let Some(node) = runtime.nodes.get_mut(body_a)
        && let SceneNodeData::StaticBody3D(body) = &mut node.data
    {
        body.set_collision_policy(mask_a);
    }
    if let Some(node) = runtime.nodes.get_mut(body_b)
        && let SceneNodeData::StaticBody3D(body) = &mut node.data
    {
        body.set_collision_policy(mask_b);
    }

    let hit = runtime
        .physics_raycast_3d_filtered(
            Vector3::new(0.0, 0.0, -5.0),
            Vector3::new(0.0, 0.0, 1.0),
            10.0,
            &PhysicsQueryFilter {
                layers: CollisionPolicy::layer(4),
                ..PhysicsQueryFilter::default()
            },
        )
        .expect("ray should include layer 4");

    assert_eq!(hit.node, body_b);
}

#[test]
fn physics_raycast_2d_filters_areas_and_nodes() {
    let mut runtime = Runtime::new();

    let static_body = NodeAPI::create::<StaticBody2D>(&mut runtime);
    let static_shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, static_body, static_shape));

    let area = NodeAPI::create::<Area2D>(&mut runtime);
    let area_shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, area, area_shape));
    let _ = <Runtime as NodeAPI>::set_global_transform_2d(
        &mut runtime,
        area,
        Transform2D::new(Vector2::new(-2.0, 0.0), 0.0, Vector2::ONE),
    );

    let hit = runtime
        .physics_raycast_2d(
            Vector2::new(-5.0, 0.0),
            Vector2::new(1.0, 0.0),
            10.0,
            &PhysicsQueryFilter::default(),
        )
        .expect("ray should hit area first");
    assert_eq!(hit.node, area);

    let hit = runtime
        .physics_raycast_2d(
            Vector2::new(-5.0, 0.0),
            Vector2::new(1.0, 0.0),
            10.0,
            &PhysicsQueryFilter {
                include_areas: false,
                ..PhysicsQueryFilter::default()
            },
        )
        .expect("ray should skip area");
    assert_eq!(hit.node, static_body);

    let hit = runtime.physics_raycast_2d(
        Vector2::new(-5.0, 0.0),
        Vector2::new(1.0, 0.0),
        10.0,
        &PhysicsQueryFilter {
            include_areas: false,
            exclude_nodes: vec![static_body],
            ..PhysicsQueryFilter::default()
        },
    );
    assert!(hit.is_none());

    if let Some(node) = runtime.nodes.get_mut(static_body)
        && let SceneNodeData::StaticBody2D(body) = &mut node.data
    {
        body.collision_layers = BitMask::from_bits(4);
        body.collision_mask = BitMask::NONE;
    }
    let hit = runtime
        .physics_raycast_2d(
            Vector2::new(-5.0, 0.0),
            Vector2::new(1.0, 0.0),
            10.0,
            &PhysicsQueryFilter {
                layers: BitMask::from_bits(4),
                include_areas: false,
                exclude_nodes: Vec::new(),
                ..PhysicsQueryFilter::default()
            },
        )
        .expect("query layers should include collider layer without collider mask coupling");
    assert_eq!(hit.node, static_body);

    let hit = runtime.physics_raycast_2d(
        Vector2::new(-5.0, 0.0),
        Vector2::new(1.0, 0.0),
        10.0,
        &PhysicsQueryFilter {
            mask: BitMask::from_bits(4),
            include_areas: false,
            ..PhysicsQueryFilter::default()
        },
    );
    assert!(
        hit.is_none(),
        "query mask should skip matching collider layer"
    );
}

#[test]
fn physics_shape_cast_2d_and_3d_hit_static_bodies() {
    let mut runtime = Runtime::new();

    let body_2d = NodeAPI::create::<StaticBody2D>(&mut runtime);
    let shape_2d = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, body_2d, shape_2d));
    let hit_2d = runtime
        .physics_shape_cast_2d(
            Shape2D::Circle { radius: 0.25 },
            Vector2::new(-5.0, 0.0),
            Vector2::new(1.0, 0.0),
            10.0,
            &PhysicsQueryFilter::default(),
        )
        .expect("2d shape cast should hit");
    assert_eq!(hit_2d.node, body_2d);
    assert!(hit_2d.distance > 3.0 && hit_2d.distance < 5.0);

    let body_3d = NodeAPI::create::<StaticBody3D>(&mut runtime);
    let shape_3d = NodeAPI::create::<CollisionShape3D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, body_3d, shape_3d));
    let _ = <Runtime as NodeAPI>::set_global_transform_3d(
        &mut runtime,
        body_3d,
        Transform3D::new(
            Vector3::new(0.0, 0.0, 4.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        ),
    );
    let hit_3d = runtime
        .physics_shape_cast_3d(
            Shape3D::Sphere { radius: 0.25 },
            Vector3::new(0.0, 0.0, -5.0),
            Vector3::new(0.0, 0.0, 1.0),
            20.0,
            &PhysicsQueryFilter::default(),
        )
        .expect("3d shape cast should hit");
    assert_eq!(hit_3d.node, body_3d);
}

#[test]
fn physics_move_body_2d_clips_before_static_body() {
    let mut runtime = Runtime::new();

    let mover = NodeAPI::create::<RigidBody2D>(&mut runtime);
    let mover_shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, mover, mover_shape));
    let wall = NodeAPI::create::<StaticBody2D>(&mut runtime);
    let wall_shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, wall, wall_shape));
    assert!(<Runtime as NodeAPI>::set_global_transform_2d(
        &mut runtime,
        mover,
        Transform2D::new(Vector2::new(-5.0, 0.0), 0.0, Vector2::ONE),
    ));

    let result = PhysicsAPI::move_body_2d(
        &mut runtime,
        mover,
        Vector2::new(0.0, 0.0),
        0.01,
        PhysicsQueryFilter::default(),
    )
    .expect("move should return result");

    assert!(result.clipped);
    assert_eq!(result.hit.expect("hit").node, wall);
    assert!(approx(result.position.x, -1.01));
    assert!(approx(
        runtime
            .get_global_transform_2d(mover)
            .expect("mover transform")
            .position
            .x,
        -1.01
    ));
}

#[test]
fn physics_move_body_3d_clips_before_static_body() {
    let mut runtime = Runtime::new();

    let mover = NodeAPI::create::<RigidBody3D>(&mut runtime);
    let mover_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, mover, mover_shape));
    let wall = NodeAPI::create::<StaticBody3D>(&mut runtime);
    let wall_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, wall, wall_shape));
    assert!(<Runtime as NodeAPI>::set_global_transform_3d(
        &mut runtime,
        mover,
        Transform3D::new(
            Vector3::new(0.0, 0.0, -5.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        ),
    ));

    let result = PhysicsAPI::move_body_3d(
        &mut runtime,
        mover,
        Vector3::new(0.0, 0.0, 0.0),
        0.01,
        PhysicsQueryFilter::default(),
    )
    .expect("move should return result");

    assert!(result.clipped);
    assert_eq!(result.hit.expect("hit").node, wall);
    assert!(approx(result.position.z, -1.01));
    assert!(approx(
        runtime
            .get_global_transform_3d(mover)
            .expect("mover transform")
            .position
            .z,
        -1.01
    ));
}

#[test]
fn physics_contacts_return_other_node_and_points() {
    let mut runtime = Runtime::new();

    let body_a = NodeAPI::create::<RigidBody2D>(&mut runtime);
    let shape_a = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, body_a, shape_a));
    let body_b = NodeAPI::create::<StaticBody2D>(&mut runtime);
    let shape_b = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, body_b, shape_b));
    if let Some(node) = runtime.nodes.get_mut(body_a)
        && let SceneNodeData::RigidBody2D(body) = &mut node.data
    {
        body.gravity_scale = 0.0;
    }

    runtime.physics_fixed_step();
    let contacts = runtime.physics_contacts_2d(body_a);
    assert!(contacts.iter().any(|contact| contact.node == body_b));
}

#[test]
fn tilemap_explicit_collision_shapes_do_not_merge_with_auto() {
    let tilemap = TileMap2D {
        width: 2,
        height: 1,
        tiles: vec![1, 2],
        collision_enabled: true,
        ..TileMap2D::new()
    };
    let tiles = vec![
        ParsedTile2D {
            id: 1,
            atlas: [0, 0],
            collision: true,
            collision_shape: ParsedTileCollisionShape2D::Auto,
        },
        ParsedTile2D {
            id: 2,
            atlas: [1, 0],
            collision: true,
            collision_shape: ParsedTileCollisionShape2D::Shape {
                shape: TileSetShape2D::Circle { radius: 3.0 },
                offset: [1.0, -1.0],
            },
        },
    ];
    let tileset = ParsedTileset2D {
        texture: "res://tiles.png".into(),
        tile_size: [16.0, 16.0],
        columns: 2,
        rows: 1,
        tiles: tiles.into(),
    };

    let shapes = tilemap_shape_descs_2d(
        &tilemap,
        BitMask::with([1]),
        BitMask::ALL,
        0.7,
        0.0,
        1.0,
        Some(&tileset),
    );
    assert_eq!(shapes.len(), 2);
    assert!(matches!(
        shapes[0].shape,
        ShapeKind2D::Primitive(Shape2D::Quad { .. })
    ));
    assert!(matches!(
        shapes[1].shape,
        ShapeKind2D::Primitive(Shape2D::Circle { radius }) if radius == 3.0
    ));
    assert_eq!(shapes[1].local.position, Vector2::new(25.0, -7.0));
}

#[test]
fn physics_2d_layers_and_masks_filter_area_overlaps() {
    let mut runtime = Runtime::new();

    let static_body = NodeAPI::create::<RigidBody2D>(&mut runtime);
    let static_shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, static_body, static_shape));

    let area = NodeAPI::create::<Area2D>(&mut runtime);
    let area_shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, area, area_shape));

    if let Some(node) = runtime.nodes.get_mut(static_body)
        && let SceneNodeData::RigidBody2D(body) = &mut node.data
    {
        body.collision_layers = BitMask::from_bits(1);
        body.collision_mask = BitMask::from_bits(2);
        body.gravity_scale = 0.0;
    }
    if let Some(node) = runtime.nodes.get_mut(area)
        && let SceneNodeData::Area2D(body) = &mut node.data
    {
        body.collision_layers = BitMask::from_bits(2);
        body.collision_mask = BitMask::from_bits(1);
    }

    runtime.physics_fixed_step();
    assert!(runtime.physics.active_area_overlaps_2d.is_empty());

    if let Some(node) = runtime.nodes.get_mut(area)
        && let SceneNodeData::Area2D(body) = &mut node.data
    {
        body.collision_mask = BitMask::NONE;
    }
    if let Some(node) = runtime.nodes.get_mut(static_body)
        && let SceneNodeData::RigidBody2D(body) = &mut node.data
    {
        body.collision_mask = BitMask::NONE;
    }

    runtime.physics_fixed_step();
    assert!(
        runtime
            .physics
            .active_area_overlaps_2d
            .contains(&AreaOverlap {
                area,
                other: static_body
            })
    );
}

#[test]
fn physics_3d_layers_and_masks_filter_area_overlaps() {
    let mut runtime = Runtime::new();

    let static_body = NodeAPI::create::<RigidBody3D>(&mut runtime);
    let static_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, static_body, static_shape));

    let area = NodeAPI::create::<Area3D>(&mut runtime);
    let area_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
    assert!(NodeAPI::reparent(&mut runtime, area, area_shape));

    if let Some(node) = runtime.nodes.get_mut(static_body)
        && let SceneNodeData::RigidBody3D(body) = &mut node.data
    {
        body.collision_layers = BitMask::from_bits(1);
        body.collision_mask = BitMask::from_bits(4);
        body.gravity_scale = 0.0;
    }
    if let Some(node) = runtime.nodes.get_mut(area)
        && let SceneNodeData::Area3D(body) = &mut node.data
    {
        body.collision_layers = BitMask::from_bits(4);
        body.collision_mask = BitMask::from_bits(1);
    }

    runtime.physics_fixed_step();
    assert!(runtime.physics.active_area_overlaps_3d.is_empty());

    if let Some(node) = runtime.nodes.get_mut(area)
        && let SceneNodeData::Area3D(body) = &mut node.data
    {
        body.collision_mask = BitMask::NONE;
    }
    if let Some(node) = runtime.nodes.get_mut(static_body)
        && let SceneNodeData::RigidBody3D(body) = &mut node.data
    {
        body.collision_mask = BitMask::NONE;
    }

    runtime.physics_fixed_step();
    assert!(
        runtime
            .physics
            .active_area_overlaps_3d
            .contains(&AreaOverlap {
                area,
                other: static_body
            })
    );
}

#[test]
fn physics_2d_fixed_joint_syncs_and_disables() {
    let mut runtime = Runtime::new();

    let body_a = NodeAPI::create::<RigidBody2D>(&mut runtime);
    let body_b = NodeAPI::create::<RigidBody2D>(&mut runtime);
    let joint = NodeAPI::create::<FixedJoint2D>(&mut runtime);

    if let Some(node) = runtime.nodes.get_mut(body_a)
        && let SceneNodeData::RigidBody2D(body) = &mut node.data
    {
        body.gravity_scale = 0.0;
    }
    if let Some(node) = runtime.nodes.get_mut(body_b)
        && let SceneNodeData::RigidBody2D(body) = &mut node.data
    {
        body.gravity_scale = 0.0;
    }
    if let Some(node) = runtime.nodes.get_mut(joint)
        && let SceneNodeData::FixedJoint2D(joint_data) = &mut node.data
    {
        joint_data.body_a = body_a;
        joint_data.body_b = body_b;
    }

    runtime.physics_fixed_step();
    assert!(
        runtime
            .physics
            .world_2d
            .as_ref()
            .is_some_and(|world| world.joint_map.contains_key(&joint))
    );

    if let Some(node) = runtime.nodes.get_mut(joint)
        && let SceneNodeData::FixedJoint2D(joint_data) = &mut node.data
    {
        joint_data.enabled = false;
    }

    runtime.physics_fixed_step();
    assert!(
        runtime
            .physics
            .world_2d
            .as_ref()
            .is_none_or(|world| !world.joint_map.contains_key(&joint))
    );
}

#[test]
fn physics_2d_distance_joint_enforces_min_and_max_limits() {
    let joint = JointDesc2D {
        id: NodeID::new(1),
        body_a: NodeID::new(2),
        body_b: NodeID::new(3),
        anchor_a: Vector2::new(-1.0, 0.0),
        anchor_b: Vector2::new(1.0, 0.0),
        enabled: true,
        collide_connected: false,
        kind: JointKind2D::Distance { min: 2.0, max: 5.0 },
        signature: 0,
    };

    let data = build_joint_2d(&joint);
    let limits = data
        .limits(r2::JointAxis::LinX)
        .expect("distance joint should set linear limits");

    assert_eq!(limits.min, 2.0);
    assert_eq!(limits.max, 5.0);
    assert_eq!(data.coupled_axes, r2::JointAxesMask::LIN_AXES);
}

#[test]
fn physics_3d_fixed_joint_syncs_and_disables() {
    let mut runtime = Runtime::new();

    let body_a = NodeAPI::create::<RigidBody3D>(&mut runtime);
    let body_b = NodeAPI::create::<RigidBody3D>(&mut runtime);
    let joint = NodeAPI::create::<FixedJoint3D>(&mut runtime);

    if let Some(node) = runtime.nodes.get_mut(body_a)
        && let SceneNodeData::RigidBody3D(body) = &mut node.data
    {
        body.gravity_scale = 0.0;
    }
    if let Some(node) = runtime.nodes.get_mut(body_b)
        && let SceneNodeData::RigidBody3D(body) = &mut node.data
    {
        body.gravity_scale = 0.0;
    }
    if let Some(node) = runtime.nodes.get_mut(joint)
        && let SceneNodeData::FixedJoint3D(joint_data) = &mut node.data
    {
        joint_data.body_a = body_a;
        joint_data.body_b = body_b;
    }

    runtime.physics_fixed_step();
    assert!(
        runtime
            .physics
            .world_3d
            .as_ref()
            .is_some_and(|world| world.joint_map.contains_key(&joint))
    );

    if let Some(node) = runtime.nodes.get_mut(joint)
        && let SceneNodeData::FixedJoint3D(joint_data) = &mut node.data
    {
        joint_data.enabled = false;
    }

    runtime.physics_fixed_step();
    assert!(
        runtime
            .physics
            .world_3d
            .as_ref()
            .is_none_or(|world| !world.joint_map.contains_key(&joint))
    );
}
