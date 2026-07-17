mod interpolation {
    use super::*;

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
    fn hidden_ui_viewport_disables_local_physics_body() {
        let mut runtime = Runtime::new();
        let viewport = NodeAPI::create::<UiViewport>(&mut runtime);
        let body = NodeAPI::create::<RigidBody3D>(&mut runtime);
        assert!(runtime.reparent(viewport, body));

        let descs = runtime.collect_body_descs_3d();
        assert!(descs.iter().any(|desc| desc.id == body && desc.enabled));
        runtime.physics_body_descs_3d = descs;

        if let Some(mut node) = runtime.nodes.get_mut(viewport)
            && let SceneNodeData::UiViewport(viewport) = &mut node.data
        {
            viewport.visible = false;
        }
        let descs = runtime.collect_body_descs_3d();
        assert!(descs.iter().any(|desc| desc.id == body && !desc.enabled));
        runtime.physics_body_descs_3d = descs;
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

        if let Some(mut node) = runtime.nodes.get_mut(body_id)
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

        if let Some(mut node) = runtime.nodes.get_mut(body_id)
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
        let mut shape = CollisionShape3D {
            shape: Shape3D::TriMesh {
                source: "res://models/one_sided.pmesh".to_string(),
            },
            flip_x: true,
            flip_z: true,
            ..Default::default()
        };
        shape.transform.scale = Vector3::new(2.0, 3.0, 4.0);

        let desc = shape_desc_3d(&shape, 0.7, 0.0);

        assert_eq!(desc.local.scale, Vector3::new(-2.0, 3.0, -4.0));
        assert!(matches!(desc.shape, ShapeKind3D::TriMesh { .. }));
    }

    #[test]
    fn collision_shape_3d_flip_changes_shape_signature() {
        let a = CollisionShape3D::default();
        let b = CollisionShape3D {
            flip_x: true,
            ..Default::default()
        };

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

        if let Some(mut node) = runtime.nodes.get_mut(water_id)
            && let SceneNodeData::WaterBody3D(water) = &mut node.data
        {
            water.water.physics.buoyancy = 2.0;
            water.water.physics.drag = 0.0;
        }

        if let Some(mut node) = runtime.nodes.get_mut(body_id)
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

        if let Some(mut node) = runtime.nodes.get_mut(water_id)
            && let SceneNodeData::WaterBody3D(water) = &mut node.data
        {
            water.water.shape = WaterShape::box_volume(Vector3::new(12.0, 4.0, 12.0));
            water.water.depth = 4.0;
            water.water.physics.buoyancy = 4.0;
            water.water.physics.drag = 0.0;
        }

        if let Some(mut node) = runtime.nodes.get_mut(body_id)
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

        if let Some(mut node) = runtime.nodes.get_mut(water_id)
            && let SceneNodeData::WaterBody2D(water) = &mut node.data
        {
            water.transform.rotation = std::f32::consts::FRAC_PI_2;
            water.water.shape = WaterShape::rect(Vector2::new(2.0, 10.0));
            water.water.physics.buoyancy = 1.0;
            water.water.physics.drag = 0.0;
        }

        if let Some(mut node) = runtime.nodes.get_mut(body_id)
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

        if let Some(mut node) = runtime.nodes.get_mut(water_id)
            && let SceneNodeData::WaterBody3D(water) = &mut node.data
        {
            water.transform.rotation =
                Quaternion::from_euler_xyz(0.0, 0.0, std::f32::consts::FRAC_PI_2);
            water.water.shape = WaterShape::box_volume(Vector3::new(4.0, 6.0, 4.0));
            water.water.physics.buoyancy = 1.0;
            water.water.physics.drag = 0.0;
        }

        if let Some(mut node) = runtime.nodes.get_mut(body_id)
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

        if let Some(mut node) = runtime.nodes.get_mut(water_a)
            && let SceneNodeData::WaterBody2D(water) = &mut node.data
        {
            water.water.shape = WaterShape::rect(Vector2::new(16.0, 16.0));
            water.water.physics.buoyancy = 2.0;
            water.water.physics.drag = 0.0;
        }
        if let Some(mut node) = runtime.nodes.get_mut(water_b)
            && let SceneNodeData::WaterBody2D(water) = &mut node.data
        {
            water.transform.position.x = 4.0;
            water.transform.position.y = 2.0;
            water.water.shape = WaterShape::rect(Vector2::new(16.0, 16.0));
            water.water.physics.buoyancy = 2.0;
            water.water.physics.drag = 0.0;
        }
        if let Some(mut node) = runtime.nodes.get_mut(body_id)
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

        if let Some(mut node) = runtime.nodes.get_mut(water_a)
            && let SceneNodeData::WaterBody3D(water) = &mut node.data
        {
            water.water.shape = WaterShape::box_volume(Vector3::new(16.0, 4.0, 16.0));
            water.water.depth = 4.0;
            water.water.physics.buoyancy = 2.0;
            water.water.physics.drag = 0.0;
        }
        if let Some(mut node) = runtime.nodes.get_mut(water_b)
            && let SceneNodeData::WaterBody3D(water) = &mut node.data
        {
            water.transform.position.x = 4.0;
            water.transform.position.y = 2.0;
            water.water.shape = WaterShape::box_volume(Vector3::new(16.0, 4.0, 16.0));
            water.water.depth = 4.0;
            water.water.physics.buoyancy = 2.0;
            water.water.physics.drag = 0.0;
        }
        if let Some(mut node) = runtime.nodes.get_mut(body_id)
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
            if let Some(mut node) = runtime.nodes.get_mut(id)
                && let SceneNodeData::WaterBody2D(water) = &mut node.data
            {
                water.transform.position.x = x;
                water.water.shape = WaterShape::rect(Vector2::new(16.0, 16.0));
                water.water.physics.buoyancy = 1.5;
                water.water.physics.drag = 0.0;
            }
        }
        if let Some(mut node) = runtime.nodes.get_mut(body_id)
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
        if let Some(mut node) = runtime.nodes.get_mut(body_id)
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

}
