mod bodies {
    use super::*;

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
    fn water_3d_entry_splash_rearms_only_after_clear_time() {
        let mut surface = WaterSurfaceParams {
            shape: WaterShape::rect(Vector2::new(16.0, 16.0)),
            ..WaterSurfaceParams::default()
        };
        surface.physics.wake_strength = 2.0;
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
        let wet = RuntimeWaterBody3D {
            id: NodeID::from_parts(93, 0),
            pos: Vector3::new(0.0, -0.05, 0.0),
            velocity: Vector3::new(0.0, -3.0, 0.0),
            mass: 2.0,
            density: 1.0,
            float_radius: 0.5,
            sleeping: false,
            collision_layers: BitMask::ALL,
            collision_mask: BitMask::NONE,
        };
        let dry = RuntimeWaterBody3D {
            pos: Vector3::new(0.0, 2.0, 0.0),
            ..wet
        };
        let mut states = AHashMap::new();

        assert_eq!(
            water_body_splashes_3d(&[wet], &water_index, &AHashMap::new(), 0.0, &mut states).len(),
            1
        );
        assert!(
            water_body_splashes_3d(&[wet], &water_index, &AHashMap::new(), 0.1, &mut states).is_empty()
        );
        assert!(
            water_body_splashes_3d(&[dry], &water_index, &AHashMap::new(), 0.2, &mut states).is_empty()
        );
        assert!(
            water_body_splashes_3d(&[wet], &water_index, &AHashMap::new(), 0.3, &mut states).is_empty()
        );
        let _ = water_body_splashes_3d(&[dry], &water_index, &AHashMap::new(), 0.4, &mut states);
        assert_eq!(
            water_body_splashes_3d(&[wet], &water_index, &AHashMap::new(), 0.8, &mut states).len(),
            1
        );
    }

    #[test]
    fn water_3d_hull_contact_does_not_rearm_at_center_crossing() {
        let mut surface = WaterSurfaceParams {
            shape: WaterShape::rect(Vector2::new(16.0, 16.0)),
            ..WaterSurfaceParams::default()
        };
        surface.physics.wake_strength = 2.0;
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
            id: NodeID::from_parts(94, 0),
            pos: Vector3::new(0.0, -0.05, 0.0),
            velocity: Vector3::new(0.0, -3.0, 0.0),
            mass: 2.0,
            density: 1.0,
            float_radius: 0.5,
            sleeping: false,
            collision_layers: BitMask::ALL,
            collision_mask: BitMask::NONE,
        };
        let mut states = AHashMap::new();
        assert_eq!(
            water_body_splashes_3d(&[body], &water_index, &AHashMap::new(), 0.0, &mut states).len(),
            1
        );

        // Center clears the surface, but hull remains in contact: no re-arm.
        let bob = RuntimeWaterBody3D {
            pos: Vector3::new(0.0, 0.35, 0.0),
            ..body
        };
        assert!(
            water_body_splashes_3d(&[bob], &water_index, &AHashMap::new(), 0.5, &mut states).is_empty()
        );
        assert!(
            water_body_splashes_3d(&[body], &water_index, &AHashMap::new(), 0.9, &mut states)
                .is_empty()
        );
    }

    #[test]
    fn buoyancy_filters_cell_velocity_spikes() {
        assert!((water_relative_normal_velocity(0.0, 100.0) + 0.7).abs() < 0.001);
        assert!((water_relative_normal_velocity(-1.0, 1.0) + 1.28).abs() < 0.001);
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

        if let Some(mut node) = runtime.nodes.get_mut(body_id)
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

        if let Some(mut node) = runtime.nodes.get_mut(body_id)
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
    fn physics_api_rejects_nonfinite_forces_and_impulses() {
        let mut runtime = Runtime::new();
        let body_2d = NodeAPI::create::<RigidBody2D>(&mut runtime);
        let body_3d = NodeAPI::create::<RigidBody3D>(&mut runtime);

        assert!(!PhysicsAPI::apply_force_2d(
            &mut runtime,
            body_2d,
            Vector2::new(f32::NAN, 0.0)
        ));
        assert!(!PhysicsAPI::apply_impulse_2d(
            &mut runtime,
            body_2d,
            Vector2::new(0.0, f32::INFINITY)
        ));
        assert!(!PhysicsAPI::apply_force_3d(
            &mut runtime,
            body_3d,
            Vector3::new(0.0, f32::NEG_INFINITY, 0.0)
        ));
        assert!(!PhysicsAPI::apply_impulse_3d(
            &mut runtime,
            body_3d,
            Vector3::new(0.0, 0.0, f32::NAN)
        ));
        assert!(runtime.physics.pending_forces_2d.is_empty());
        assert!(runtime.physics.pending_forces_3d.is_empty());
        assert!(runtime.physics.pending_impulses_2d.is_empty());
        assert!(runtime.physics.pending_impulses_3d.is_empty());
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
    fn soa_writeback_nested_rigid_body_2d_keeps_parent_offset() {
        let mut runtime = Runtime::new();
        let holder = NodeAPI::create::<Sprite2D>(&mut runtime);
        let body_id = NodeAPI::create::<RigidBody2D>(&mut runtime);
        let shape_id = NodeAPI::create::<CollisionShape2D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, holder, body_id));
        assert!(NodeAPI::reparent(&mut runtime, body_id, shape_id));
        let _ = NodeAPI::set_global_transform_2d(
            &mut runtime,
            holder,
            Transform2D::new(Vector2::new(5.0, 0.0), 0.0, Vector2::ONE),
        );

        runtime.time.fixed_delta = 1.0 / 60.0;
        for _ in 0..4 {
            runtime.physics_fixed_step();
        }

        let global = runtime
            .get_global_transform_2d(body_id)
            .expect("body global");
        // no horizontal force -> stays under holder x; gravity -> fell
        assert!(approx(global.position.x, 5.0));
        assert!(global.position.y < -0.001);

        let local = runtime
            .nodes
            .get(body_id)
            .and_then(|node| match &node.data {
                SceneNodeData::RigidBody2D(body) => Some(body.transform),
                _ => None,
            })
            .expect("body local");
        // stored local = parent-relative: x back-solved to ~0, not global 5
        assert!(approx(local.position.x, 0.0));
        assert!(approx(local.position.y, global.position.y));
    }

    #[test]
    fn soa_writeback_nested_rigid_body_3d_keeps_parent_offset() {
        let mut runtime = Runtime::new();
        let holder = NodeAPI::create::<MeshInstance3D>(&mut runtime);
        let body_id = NodeAPI::create::<RigidBody3D>(&mut runtime);
        let shape_id = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, holder, body_id));
        assert!(NodeAPI::reparent(&mut runtime, body_id, shape_id));
        let _ = NodeAPI::set_global_transform_3d(
            &mut runtime,
            holder,
            Transform3D::new(
                Vector3::new(0.0, 0.0, 7.0),
                Quaternion::IDENTITY,
                Vector3::ONE,
            ),
        );

        runtime.time.fixed_delta = 1.0 / 60.0;
        for _ in 0..4 {
            runtime.physics_fixed_step();
        }

        let global = runtime
            .get_global_transform_3d(body_id)
            .expect("body global");
        assert!(approx(global.position.z, 7.0));
        assert!(global.position.y < -0.001);

        let local = runtime
            .nodes
            .get(body_id)
            .and_then(|node| match &node.data {
                SceneNodeData::RigidBody3D(body) => Some(body.transform),
                _ => None,
            })
            .expect("body local");
        assert!(approx(local.position.z, 0.0));
        assert!(approx(local.position.y, global.position.y));
    }

    #[test]
    fn soa_writeback_multi_body_keeps_per_body_identity_2d() {
        let mut runtime = Runtime::new();
        let mut bodies = Vec::new();
        for i in 0..8u32 {
            let body_id = NodeAPI::create::<RigidBody2D>(&mut runtime);
            let shape_id = NodeAPI::create::<CollisionShape2D>(&mut runtime);
            assert!(NodeAPI::reparent(&mut runtime, body_id, shape_id));
            if let Some(mut node) = runtime.nodes.get_mut(body_id)
                && let SceneNodeData::RigidBody2D(body) = &mut node.data
            {
                body.gravity_scale = 0.0;
                body.can_sleep = false;
                body.linear_velocity = Vector2::new(1.0 + i as f32, 0.0);
            }
            // spacing thru dirtying API so global cache picks it up (no overlap)
            let _ = NodeAPI::set_global_transform_2d(
                &mut runtime,
                body_id,
                Transform2D::new(Vector2::new(i as f32 * 10.0, 0.0), 0.0, Vector2::ONE),
            );
            bodies.push((body_id, 1.0 + i as f32));
        }

        runtime.time.fixed_delta = 1.0 / 60.0;
        runtime.physics_fixed_step();

        for (i, &(body_id, expected_vx)) in bodies.iter().enumerate() {
            let (vel, pos) = runtime
                .nodes
                .get(body_id)
                .and_then(|node| match &node.data {
                    SceneNodeData::RigidBody2D(body) => {
                        Some((body.linear_velocity, body.transform.position))
                    }
                    _ => None,
                })
                .expect("body exists");
            // per-body identity: vx stays near its OWN target (damping ok),
            // never swapped w/ another body's -> tol < half the 1.0 spacing
            assert!((vel.x - expected_vx).abs() < 0.1, "body {i} vx {}", vel.x);
            assert!(vel.y.abs() < 0.001);
            // moved right frm its own start by its own vx
            assert!(pos.x > i as f32 * 10.0, "body {i} pos {}", pos.x);
        }
    }

    #[test]
    fn custom_force_vectors_interpolate_by_radius() {
        let emitter = perro_nodes::PhysicsForceEmitter2D {
            profile: perro_nodes::PhysicsForceProfile::Custom,
            radius: 10.0,
            strength: 2.0,
            vectors: vec![
                Vector2::new(0.0, 20.0),
                Vector2::new(4.0, 15.0),
                Vector2::new(8.0, 0.0),
            ],
            ..Default::default()
        };

        let force = force_emitter_force_2d(&emitter, Vector2::new(5.0, 0.0), 5.0);
        assert_eq!(force, Vector2::new(8.0, 30.0));
    }

    #[test]
    fn force_emitter_2d_lift_queues_upward_force() {
        let mut runtime = Runtime::new();
        let emitter_id = NodeAPI::create::<perro_nodes::PhysicsForceEmitter2D>(&mut runtime);
        let body_id = NodeAPI::create::<RigidBody2D>(&mut runtime);

        if let Some(mut node) = runtime.nodes.get_mut(emitter_id)
            && let SceneNodeData::PhysicsForceEmitter2D(emitter) = &mut node.data
        {
            emitter.profile = perro_nodes::PhysicsForceProfile::Lift;
            emitter.radius = 10.0;
            emitter.strength = 12.0;
        }
        if let Some(mut node) = runtime.nodes.get_mut(body_id)
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

        if let Some(mut node) = runtime.nodes.get_mut(water_id)
            && let SceneNodeData::WaterBody2D(water) = &mut node.data
        {
            water.water.shape = WaterShape::rect(Vector2::new(16.0, 16.0));
        }
        if let Some(mut node) = runtime.nodes.get_mut(emitter_id)
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

        if let Some(mut node) = runtime.nodes.get_mut(water_id)
            && let SceneNodeData::WaterBody2D(water) = &mut node.data
        {
            water.water.shape = WaterShape::rect(Vector2::new(16.0, 16.0));
        }
        if let Some(mut node) = runtime.nodes.get_mut(body_id)
            && let SceneNodeData::RigidBody2D(body) = &mut node.data
        {
            body.transform.position = Vector2::new(2.0, 0.0);
            body.gravity_scale = 0.0;
        }

        let mut emitter = perro_nodes::PhysicsForceEmitter2D {
            profile: perro_nodes::PhysicsForceProfile::Current,
            radius: 6.0,
            strength: 20.0,
            vectors: vec![Vector2::new(1.0, 0.0)],
            ..Default::default()
        };
        emitter.transform.position = Vector2::new(0.0, 0.0);

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

}
