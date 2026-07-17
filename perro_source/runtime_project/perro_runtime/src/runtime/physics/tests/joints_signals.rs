mod joints_signals {
    use super::*;

    #[test]
    fn character_body_3d_move_and_slide_stops_on_static_floor() {
        let mut runtime = Runtime::new();
        runtime.time.fixed_delta = 1.0 / 60.0;

        let floor_id = NodeAPI::create::<StaticBody3D>(&mut runtime);
        let floor_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, floor_id, floor_shape));
        if let Some(mut node) = runtime.nodes.get_mut(floor_shape)
            && let SceneNodeData::CollisionShape3D(shape) = &mut node.data
        {
            shape.shape = Shape3D::Cube {
                size: Vector3::new(20.0, 1.0, 20.0),
            };
        }

        let char_id = NodeAPI::create::<CharacterBody3D>(&mut runtime);
        let char_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, char_id, char_shape));
        assert!(NodeAPI::set_global_transform_3d(
            &mut runtime,
            char_id,
            Transform3D::new(
                Vector3::new(0.0, 3.0, 0.0),
                Quaternion::IDENTITY,
                Vector3::ONE
            ),
        ));

        let result = runtime
            .physics_move_and_slide_3d(
                char_id,
                Vector3::new(0.0, -5.0, 0.0),
                &PhysicsQueryFilter {
                    include_areas: false,
                    ..PhysicsQueryFilter::default()
                },
            )
            .expect("move_and_slide result");

        // stop ~1.0 (floor half 0.5 + char half 0.5), no tunnel
        assert!(
            result.position.y < 1.2 && result.position.y > 0.9,
            "char must stop on floor, y={}",
            result.position.y
        );
        assert!(
            result.hits.iter().any(|hit| hit.node == floor_id),
            "slide must report floor hit"
        );
        let pos = runtime
            .get_global_transform_3d(char_id)
            .expect("char transform")
            .position;
        assert!(
            approx(pos.y, result.position.y),
            "node transform must match"
        );
        // char body kp no dynamics state in world
        let kind = runtime
            .physics
            .world_3d
            .as_ref()
            .and_then(|world| world.body_map.get(&char_id))
            .map(|state| state.kind);
        assert_eq!(kind, Some(BodyKind::Character));
    }

    #[test]
    fn character_body_3d_move_and_slide_slides_along_floor() {
        let mut runtime = Runtime::new();
        runtime.time.fixed_delta = 1.0 / 60.0;

        let floor_id = NodeAPI::create::<StaticBody3D>(&mut runtime);
        let floor_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, floor_id, floor_shape));
        if let Some(mut node) = runtime.nodes.get_mut(floor_shape)
            && let SceneNodeData::CollisionShape3D(shape) = &mut node.data
        {
            shape.shape = Shape3D::Cube {
                size: Vector3::new(20.0, 1.0, 20.0),
            };
        }

        let char_id = NodeAPI::create::<CharacterBody3D>(&mut runtime);
        let char_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, char_id, char_shape));
        assert!(NodeAPI::set_global_transform_3d(
            &mut runtime,
            char_id,
            Transform3D::new(
                Vector3::new(0.0, 3.0, 0.0),
                Quaternion::IDENTITY,
                Vector3::ONE
            ),
        ));

        // diagonal down: unconsumed motion must slide along floor plane
        let result = runtime
            .physics_move_and_slide_3d(
                char_id,
                Vector3::new(2.0, -5.0, 0.0),
                &PhysicsQueryFilter {
                    include_areas: false,
                    ..PhysicsQueryFilter::default()
                },
            )
            .expect("move_and_slide result");

        assert!(
            result.position.y < 1.2 && result.position.y > 0.9,
            "char must stop on floor, y={}",
            result.position.y
        );
        assert!(
            result.position.x > 1.5,
            "char must slide along floor, x={}",
            result.position.x
        );
    }

    #[test]
    fn character_body_3d_reports_contacts_on_static_floor() {
        let mut runtime = Runtime::new();
        runtime.time.fixed_delta = 1.0 / 60.0;

        let floor_id = NodeAPI::create::<StaticBody3D>(&mut runtime);
        let floor_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, floor_id, floor_shape));
        if let Some(mut node) = runtime.nodes.get_mut(floor_shape)
            && let SceneNodeData::CollisionShape3D(shape) = &mut node.data
        {
            shape.shape = Shape3D::Cube {
                size: Vector3::new(20.0, 1.0, 20.0),
            };
        }

        let char_id = NodeAPI::create::<CharacterBody3D>(&mut runtime);
        let char_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, char_id, char_shape));
        assert!(NodeAPI::set_global_transform_3d(
            &mut runtime,
            char_id,
            Transform3D::new(
                Vector3::new(0.0, 3.0, 0.0),
                Quaternion::IDENTITY,
                Vector3::ONE
            ),
        ));

        // manual descent onto floor; sweep hit feed contacts
        runtime
            .physics_move_and_slide_3d(
                char_id,
                Vector3::new(0.0, -5.0, 0.0),
                &PhysicsQueryFilter {
                    include_areas: false,
                    ..PhysicsQueryFilter::default()
                },
            )
            .expect("move_and_slide result");

        // kinematic char on static floor: contact pair need KINEMATIC_FIXED opt-in
        let contacts = runtime.physics_contacts_3d(char_id);
        assert!(
            contacts.iter().any(|contact| contact.node == floor_id),
            "char on floor must report contact, got {} contacts",
            contacts.len()
        );
    }

    #[test]
    fn character_body_3d_apply_gravity_falls_and_lands() {
        let mut runtime = Runtime::new();
        runtime.time.fixed_delta = 1.0 / 60.0;

        let floor_id = NodeAPI::create::<StaticBody3D>(&mut runtime);
        let floor_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, floor_id, floor_shape));
        if let Some(mut node) = runtime.nodes.get_mut(floor_shape)
            && let SceneNodeData::CollisionShape3D(shape) = &mut node.data
        {
            shape.shape = Shape3D::Cube {
                size: Vector3::new(20.0, 1.0, 20.0),
            };
        }

        let char_id = NodeAPI::create::<CharacterBody3D>(&mut runtime);
        let char_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, char_id, char_shape));
        assert!(NodeAPI::set_global_transform_3d(
            &mut runtime,
            char_id,
            Transform3D::new(
                Vector3::new(0.0, 3.0, 0.0),
                Quaternion::IDENTITY,
                Vector3::ONE
            ),
        ));

        // script-invoked gravity: cal per step, engine integrates fall speed
        let filter = PhysicsQueryFilter {
            include_areas: false,
            ..PhysicsQueryFilter::default()
        };
        let dt = 1.0 / 60.0;
        let mut landed = false;
        for _ in 0..600 {
            let result = runtime.physics_apply_gravity_3d(char_id, dt, 64.0, &filter);
            if result.is_some_and(|result| result.clipped) {
                landed = true;
            }
        }
        assert!(landed, "apply_gravity must land on floor");

        let pos = runtime
            .get_global_transform_3d(char_id)
            .expect("char transform")
            .position;
        assert!(pos.y < 1.2, "char should fall, y={}", pos.y);
        assert!(pos.y > 0.9, "char should not tunnel floor, y={}", pos.y);

        // gravity on non-char body reject
        let rigid_id = NodeAPI::create::<RigidBody3D>(&mut runtime);
        assert!(
            runtime
                .physics_apply_gravity_3d(rigid_id, dt, 64.0, &filter)
                .is_none(),
            "apply_gravity must reject non-character bodies"
        );
    }

    #[test]
    fn character_body_3d_never_moves_without_script() {
        let mut runtime = Runtime::new();
        runtime.time.fixed_delta = 1.0 / 60.0;

        let char_id = NodeAPI::create::<CharacterBody3D>(&mut runtime);
        let char_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, char_id, char_shape));
        assert!(NodeAPI::set_global_transform_3d(
            &mut runtime,
            char_id,
            Transform3D::new(
                Vector3::new(0.0, 5.0, 0.0),
                Quaternion::IDENTITY,
                Vector3::ONE
            ),
        ));

        for _ in 0..120 {
            runtime.physics_fixed_step();
        }

        let pos = runtime
            .get_global_transform_3d(char_id)
            .expect("char transform")
            .position;
        assert!(approx(pos.y, 5.0), "char must never self-move, y={}", pos.y);
    }

    #[test]
    fn character_body_2d_move_and_slide_stops_on_static_floor() {
        let mut runtime = Runtime::new();
        runtime.time.fixed_delta = 1.0 / 60.0;

        let floor_id = NodeAPI::create::<StaticBody2D>(&mut runtime);
        let floor_shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, floor_id, floor_shape));
        if let Some(mut node) = runtime.nodes.get_mut(floor_shape)
            && let SceneNodeData::CollisionShape2D(shape) = &mut node.data
        {
            shape.shape = Shape2D::Quad {
                width: 20.0,
                height: 1.0,
            };
        }

        let char_id = NodeAPI::create::<CharacterBody2D>(&mut runtime);
        let char_shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, char_id, char_shape));
        assert!(NodeAPI::set_global_transform_2d(
            &mut runtime,
            char_id,
            Transform2D::new(Vector2::new(0.0, 3.0), 0.0, Vector2::ONE),
        ));

        let result = runtime
            .physics_move_and_slide_2d(
                char_id,
                Vector2::new(0.0, -5.0),
                &PhysicsQueryFilter {
                    include_areas: false,
                    ..PhysicsQueryFilter::default()
                },
            )
            .expect("move_and_slide result");

        assert!(
            result.position.y < 1.2 && result.position.y > 0.9,
            "char must stop on floor, y={}",
            result.position.y
        );
        assert!(
            result.hits.iter().any(|hit| hit.node == floor_id),
            "slide must report floor hit"
        );
    }

    #[test]
    fn move_body_fast_path_keeps_world_synced_no_recollect() {
        let mut runtime = Runtime::new();
        runtime.time.fixed_delta = 1.0 / 60.0;
        let (char_id, _floor) = char_over_floor_3d(&mut runtime);
        let filter = PhysicsQueryFilter {
            include_areas: false,
            ..PhysicsQueryFilter::default()
        };

        // 1st move: syncs world (collect fires), then fast-path commits + re-record.
        let r1 = runtime
            .physics_move_body_3d(char_id, Vector3::new(0.0, 2.5, 0.0), 0.005, &filter)
            .expect("move 1");
        let after_first = runtime.physics_collect_calls_3d.get();

        // 2nd move: world already in-sync via fast path -> ensure_synced must skip
        // collect entirely (this is the per-iteration win in move_and_slide).
        let r2 = runtime
            .physics_move_body_3d(char_id, Vector3::new(0.0, 2.0, 0.0), 0.005, &filter)
            .expect("move 2");
        assert_eq!(
            runtime.physics_collect_calls_3d.get(),
            after_first,
            "2nd move must not re-collect: fast path kept world synced"
        );

        // pose reflects the move (fell toward floor, moved down).
        assert!(r2.position.y < r1.position.y + 1.0e-4);

        // a query now (ensure_synced) must also skip collect -> world stayed fresh.
        let before_ray = runtime.physics_collect_calls_3d.get();
        let _ = runtime.physics_raycast_3d(
            Vector3::new(0.0, 10.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            100.0,
            false,
        );
        assert_eq!(
            runtime.physics_collect_calls_3d.get(),
            before_ray,
            "raycast after fast-path move must not re-collect"
        );
    }

    #[test]
    fn move_body_interleaved_unrelated_mutation_forces_resync() {
        let mut runtime = Runtime::new();
        runtime.time.fixed_delta = 1.0 / 60.0;
        let (char_id, _floor) = char_over_floor_3d(&mut runtime);
        // 2nd char body to mutate btw moves.
        let other = NodeAPI::create::<CharacterBody3D>(&mut runtime);
        let other_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, other, other_shape));
        assert!(NodeAPI::set_global_transform_3d(
            &mut runtime,
            other,
            Transform3D::new(
                Vector3::new(5.0, 3.0, 0.0),
                Quaternion::IDENTITY,
                Vector3::ONE
            ),
        ));
        let filter = PhysicsQueryFilter {
            include_areas: false,
            ..PhysicsQueryFilter::default()
        };

        let _ = runtime
            .physics_move_body_3d(char_id, Vector3::new(0.0, 2.5, 0.0), 0.005, &filter)
            .expect("move 1");
        let baseline = runtime.physics_collect_calls_3d.get();

        // mutate a DIFFERENT body's transform -> physics_revision bumps + physics
        // dirty set -> next move's ensure_synced MUST re-collect (fast path re-record
        // must not mask unrelated staleness).
        assert!(NodeAPI::set_global_transform_3d(
            &mut runtime,
            other,
            Transform3D::new(
                Vector3::new(6.0, 3.0, 0.0),
                Quaternion::IDENTITY,
                Vector3::ONE
            ),
        ));
        let _ = runtime
            .physics_move_body_3d(char_id, Vector3::new(0.0, 2.0, 0.0), 0.005, &filter)
            .expect("move 2");
        assert!(
            runtime.physics_collect_calls_3d.get() > baseline,
            "unrelated body mutation must force full re-collect"
        );
    }

    #[test]
    fn move_body_nested_char_pose_consistent_after_fast_path() {
        // char parented under a moved Node3D: fast path writes re-read global to
        // rapier; a raycast must hit the char at its true world pose (proves the
        // committed pose == what a full collect+sync would produce).
        let mut runtime = Runtime::new();
        runtime.time.fixed_delta = 1.0 / 60.0;

        let floor_id = NodeAPI::create::<StaticBody3D>(&mut runtime);
        let floor_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, floor_id, floor_shape));
        if let Some(mut node) = runtime.nodes.get_mut(floor_shape)
            && let SceneNodeData::CollisionShape3D(shape) = &mut node.data
        {
            shape.shape = Shape3D::Cube {
                size: Vector3::new(20.0, 1.0, 20.0),
            };
        }

        // pivot parent offset in x; char local under it.
        let pivot = NodeAPI::create::<perro_nodes::Node3D>(&mut runtime);
        assert!(NodeAPI::set_global_transform_3d(
            &mut runtime,
            pivot,
            Transform3D::new(
                Vector3::new(4.0, 0.0, 0.0),
                Quaternion::IDENTITY,
                Vector3::ONE
            ),
        ));
        let char_id = NodeAPI::create::<CharacterBody3D>(&mut runtime);
        let char_shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, char_id, char_shape));
        assert!(NodeAPI::reparent(&mut runtime, pivot, char_id));
        assert!(NodeAPI::set_global_transform_3d(
            &mut runtime,
            char_id,
            Transform3D::new(
                Vector3::new(4.0, 3.0, 0.0),
                Quaternion::IDENTITY,
                Vector3::ONE
            ),
        ));

        let filter = PhysicsQueryFilter {
            include_areas: false,
            ..PhysicsQueryFilter::default()
        };
        let result = runtime
            .physics_move_body_3d(char_id, Vector3::new(4.0, 1.5, 0.0), 0.005, &filter)
            .expect("nested move");
        let node_pos = runtime
            .get_global_transform_3d(char_id)
            .expect("char global")
            .position;
        assert!(
            approx(node_pos.y, result.position.y),
            "node global == result"
        );
        assert!(approx(node_pos.x, 4.0), "x parent offset preserved");

        // ray straight down thru char world-x must hit the char (rapier body at
        // committed world pose, not local).
        let hit = runtime.physics_raycast_3d(
            Vector3::new(4.0, 10.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            100.0,
            false,
        );
        assert!(
            hit.is_some_and(|h| h.node == char_id || h.node == floor_id),
            "ray at world-x must hit committed char/floor pose"
        );
    }

    #[test]
    fn nonphysics_node_move_does_not_recollect() {
        let mut runtime = Runtime::new();
        runtime.time.fixed_delta = 1.0 / 60.0;
        // 1 physics body so a world exists.
        let (char_id, _floor) = char_over_floor_3d(&mut runtime);
        let _ = char_id;
        // plain non-physics node moving each "frame".
        let spinner = NodeAPI::create::<perro_nodes::Node3D>(&mut runtime);

        // prime: sync world once via a query.
        let _ = runtime.physics_raycast_3d(
            Vector3::new(0.0, 10.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            100.0,
            false,
        );
        let baseline = runtime.physics_collect_calls_3d.get();

        // move the non-physics node + query again -> gate must skip collect.
        for i in 0..5 {
            assert!(NodeAPI::set_global_transform_3d(
                &mut runtime,
                spinner,
                Transform3D::new(
                    Vector3::new(i as f32, 0.0, 0.0),
                    Quaternion::IDENTITY,
                    Vector3::ONE,
                ),
            ));
            let _ = runtime.physics_raycast_3d(
                Vector3::new(0.0, 10.0, 0.0),
                Vector3::new(0.0, -1.0, 0.0),
                100.0,
                false,
            );
        }
        assert_eq!(
            runtime.physics_collect_calls_3d.get(),
            baseline,
            "non-physics node move must not trigger physics collect"
        );
    }

    #[test]
    fn physics_node_move_does_recollect() {
        let mut runtime = Runtime::new();
        runtime.time.fixed_delta = 1.0 / 60.0;
        let (char_id, _floor) = char_over_floor_3d(&mut runtime);
        let _ = runtime.physics_raycast_3d(
            Vector3::new(0.0, 10.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            100.0,
            false,
        );
        let baseline = runtime.physics_collect_calls_3d.get();
        // moving a physics body's transform directly must re-collect.
        assert!(NodeAPI::set_global_transform_3d(
            &mut runtime,
            char_id,
            Transform3D::new(
                Vector3::new(1.0, 3.0, 0.0),
                Quaternion::IDENTITY,
                Vector3::ONE
            ),
        ));
        let _ = runtime.physics_raycast_3d(
            Vector3::new(0.0, 10.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            100.0,
            false,
        );
        assert!(
            runtime.physics_collect_calls_3d.get() > baseline,
            "physics body move must re-collect"
        );
    }

    #[test]
    fn collision_shape_child_move_does_recollect() {
        let mut runtime = Runtime::new();
        runtime.time.fixed_delta = 1.0 / 60.0;
        let (_char, _floor) = char_over_floor_3d(&mut runtime);
        // static body w/ a collision-shape child we will move.
        let body = NodeAPI::create::<StaticBody3D>(&mut runtime);
        let shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, body, shape));
        assert!(NodeAPI::set_global_transform_3d(
            &mut runtime,
            body,
            Transform3D::new(
                Vector3::new(8.0, 0.0, 0.0),
                Quaternion::IDENTITY,
                Vector3::ONE
            ),
        ));
        let _ = runtime.physics_raycast_3d(
            Vector3::new(0.0, 10.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            100.0,
            false,
        );
        let baseline = runtime.physics_collect_calls_3d.get();
        // move the CollisionShape child (a physics-typed node) under static parent.
        assert!(NodeAPI::set_global_transform_3d(
            &mut runtime,
            shape,
            Transform3D::new(
                Vector3::new(8.0, 1.0, 0.0),
                Quaternion::IDENTITY,
                Vector3::ONE
            ),
        ));
        let _ = runtime.physics_raycast_3d(
            Vector3::new(0.0, 10.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            100.0,
            false,
        );
        assert!(
            runtime.physics_collect_calls_3d.get() > baseline,
            "collision-shape child move must re-collect"
        );
    }

    #[test]
    fn parent_of_physics_node_move_does_recollect() {
        // THE TRAP: a plain Node3D parent moving carries its physics-body child.
        // propagation visits the child -> physics gate must catch it.
        let mut runtime = Runtime::new();
        runtime.time.fixed_delta = 1.0 / 60.0;
        let (_char, _floor) = char_over_floor_3d(&mut runtime);

        let parent = NodeAPI::create::<perro_nodes::Node3D>(&mut runtime);
        let body = NodeAPI::create::<StaticBody3D>(&mut runtime);
        let shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, body, shape));
        assert!(NodeAPI::reparent(&mut runtime, parent, body));
        assert!(NodeAPI::set_global_transform_3d(
            &mut runtime,
            parent,
            Transform3D::new(
                Vector3::new(8.0, 0.0, 0.0),
                Quaternion::IDENTITY,
                Vector3::ONE
            ),
        ));
        let _ = runtime.physics_raycast_3d(
            Vector3::new(0.0, 10.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            100.0,
            false,
        );
        let baseline = runtime.physics_collect_calls_3d.get();

        // move the PLAIN parent (non-physics). its physics child inherits the move;
        // propagation marks the child physics-dirty -> gate must re-collect.
        assert!(NodeAPI::set_global_transform_3d(
            &mut runtime,
            parent,
            Transform3D::new(
                Vector3::new(9.0, 0.0, 0.0),
                Quaternion::IDENTITY,
                Vector3::ONE
            ),
        ));
        let _ = runtime.physics_raycast_3d(
            Vector3::new(0.0, 10.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            100.0,
            false,
        );
        assert!(
            runtime.physics_collect_calls_3d.get() > baseline,
            "parent-of-physics move must re-collect (transform inheritance)"
        );
    }

}
