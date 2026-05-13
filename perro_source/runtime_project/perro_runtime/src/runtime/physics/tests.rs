use super::*;
use crate::runtime::render_2d::{
    ParsedTile2D, ParsedTileCollisionShape2D, ParsedTileset2D, TileSetShape2D,
};
use perro_nodes::{
    Area2D, Area3D, CollisionShape2D, CollisionShape3D, FixedJoint2D, FixedJoint3D, RigidBody2D,
    RigidBody3D, StaticBody2D, StaticBody3D, WaterBody3D,
};
use perro_structs::CollisionPolicy;

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
    assert!((force.y - 0.75).abs() < 0.001);
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
                mask: CollisionPolicy::layer(4),
                ..PhysicsQueryFilter::default()
            },
        )
        .expect("ray should skip layer 3 and hit layer 4");

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
                mask: BitMask::from_bits(4),
                include_areas: false,
                exclude_nodes: Vec::new(),
            },
        )
        .expect("query mask should use collider layer without collider mask coupling");
    assert_eq!(hit.node, static_body);
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
