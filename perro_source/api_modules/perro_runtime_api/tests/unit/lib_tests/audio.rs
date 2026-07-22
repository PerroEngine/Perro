mod audio {
    use super::*;

    #[test]
    fn runtime_api_getter_aliases_forward() {
        let mut rt = DummyRuntime {
            state: Box::new(5_i32),
            gravity: -9.81,
            coefficient: 0.7,
        };
        let mut ctx = RuntimeWindow::new(&mut rt);
        let id = NodeID::new(42);

        assert_eq!(ctx.Time().delta(), 0.016);
        assert_eq!(ctx.Time().fixed_delta(), 0.016);
        assert_eq!(ctx.Time().elapsed(), 1.0);
        assert_eq!(ctx.Time().simulation_time(), Duration::from_micros(1_000));
        assert_eq!(ctx.Time().graphics_time(), Duration::from_micros(2_000));
        assert_eq!(ctx.Time().frame_time(), Duration::from_micros(16_000));
        assert_eq!(ctx.Time().fps(), 60.0);
        assert_eq!(ctx.Time().profiling().fps, 60.0);
        assert_eq!(ctx.Window().active_refresh_rate(), Some(60.0));
        assert_eq!(ctx.Nodes().name(id), None);
        assert_eq!(ctx.Nodes().children_ids(id), None);
        assert_eq!(ctx.Physics().gravity(), -9.81);
        assert_eq!(ctx.Physics().coefficient(), 0.7);
        ctx.Physics().set_paused(true);
        assert!(!ctx.Physics().paused());
    }

    #[test]
    fn node_query_iterator_macros_typecheck_and_forward() {
        let ids = vec![NodeID::new(1), NodeID::new(2), NodeID::new(3)];
        let mut rt = DummyRuntime {
            state: Box::new(ids.clone()),
            gravity: -9.81,
            coefficient: 1.0,
        };
        let mut ctx = RuntimeWindow::new(&mut rt);
        let parent = NodeID::new(99);

        let iter_hits = query_iter!(&mut ctx, all(tags["enemy"])).collect::<Vec<_>>();
        assert_eq!(iter_hits, ids);

        let subtree_hits =
            query_iter!(&mut ctx, all(tags["enemy"]), in_subtree(parent)).collect::<Vec<_>>();
        assert_eq!(subtree_hits, ids);

        let reusable_query = query_builder!(all(tags["enemy"]));
        let reusable_hits = query_iter!(&mut ctx, &reusable_query).collect::<Vec<_>>();
        assert_eq!(reusable_hits, ids);

        let reusable_subtree_hits =
            query_iter!(&mut ctx, &reusable_query, in_subtree(parent)).collect::<Vec<_>>();
        assert_eq!(reusable_subtree_hits, ids);

        let module_hits = ctx
            .NodeQuery()
            .query_iter(&reusable_query)
            .collect::<Vec<_>>();
        assert_eq!(module_hits, ids);

        let mut each_count = 0;
        query_each!(&mut ctx, all(tags["enemy"]), |id| {
            let _ = get_node_name!(&mut ctx, id);
            each_count += 1;
        });
        assert_eq!(each_count, ids.len());

        let mapped = query_map!(&mut ctx, all(tags["enemy"]), |id| id.index());
        assert_eq!(mapped, vec![1, 2, 3]);
    }

    #[test]
    fn close_app_macro_queues_window_close_request() {
        let mut rt = dummy_runtime();
        let mut ctx = RuntimeWindow::new(&mut rt);

        close_app!(&mut ctx);

        assert_eq!(
            rt.state.downcast_ref::<WindowRequest>(),
            Some(&WindowRequest::CloseApp)
        );
    }

    #[test]
    fn physics_solve_velocity_to_target_2d_hits_target() {
        let mut rt = dummy_runtime();
        let mut ctx = RuntimeWindow::new(&mut rt);
        let origin = Vector2::new(0.0, 0.0);
        let target = Vector2::new(12.0, 3.0);
        let time = 1.5;

        let velocity = physics_solve_velocity_to_target_2d!(&mut ctx, origin, target, time).expect("test setup must succeed");
        let hit = simulate_2d(origin, velocity, Vector2::ZERO, -9.81, time);

        assert_vec2_close(hit, target, 1.0e-4);
    }

    #[test]
    fn physics_solve_velocity_to_target_3d_hits_target_with_drift() {
        let mut rt = dummy_runtime();
        let mut ctx = RuntimeWindow::new(&mut rt);
        let origin = Vector3::new(0.0, 1.0, 0.0);
        let target = Vector3::new(8.0, 2.0, -4.0);
        let drift = Vector3::new(1.0, 0.0, -0.5);
        let time = 1.25;

        let velocity =
            physics_solve_velocity_to_target_3d!(&mut ctx, origin, target, time, drift).expect("test setup must succeed");
        let hit = simulate_3d(origin, velocity, drift, -9.81, time);

        assert_vec3_close(hit, target, 1.0e-4);
    }

    #[test]
    fn physics_solve_launch_velocity_2d_returns_low_and_high_arcs() {
        let mut rt = dummy_runtime();
        let mut ctx = RuntimeWindow::new(&mut rt);
        let origin = Vector2::new(0.0, 0.0);
        let target = Vector2::new(10.0, 0.0);
        let speed = 12.0;

        let solution = physics_solve_launch_velocity_2d!(&mut ctx, origin, target, speed, 5.0).expect("test setup must succeed");
        let low_time = 10.0 / solution.low.x;
        let high_time = 10.0 / solution.high.x;
        let low_hit = simulate_2d(origin, solution.low, Vector2::ZERO, -9.81, low_time);
        let high_hit = simulate_2d(origin, solution.high, Vector2::ZERO, -9.81, high_time);

        assert!(
            low_time < high_time,
            "low_time={low_time} high_time={high_time}"
        );
        assert_vec2_close(low_hit, target, 2.0e-3);
        assert_vec2_close(high_hit, target, 2.0e-3);
        assert!((solution.low.length() - speed).abs() < 2.0e-3);
        assert!((solution.high.length() - speed).abs() < 2.0e-3);
    }

    #[test]
    fn physics_solve_launch_velocity_3d_returns_none_when_unreachable() {
        let mut rt = dummy_runtime();
        let mut ctx = RuntimeWindow::new(&mut rt);

        let solution = physics_solve_launch_velocity_3d!(
            &mut ctx,
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(100.0, 0.0, 0.0),
            1.0,
            4.0
        );

        assert_eq!(solution, None);
    }

    #[test]
    fn physics_trajectory_solver_rejects_invalid_inputs() {
        let mut rt = dummy_runtime();
        let mut ctx = RuntimeWindow::new(&mut rt);

        assert_eq!(
            physics_solve_velocity_to_target_2d!(&mut ctx, Vector2::ZERO, Vector2::new(1.0, 0.0), 0.0),
            None
        );
        assert_eq!(
            physics_solve_velocity_to_target_3d!(&mut ctx, Vector3::ZERO, Vector3::ZERO, 1.0),
            None
        );
        assert_eq!(
            physics_solve_launch_velocity_2d!(
                &mut ctx,
                Vector2::ZERO,
                Vector2::new(1.0, 0.0),
                0.0,
                1.0
            ),
            None
        );
        assert_eq!(
            physics_solve_launch_velocity_3d!(
                &mut ctx,
                Vector3::ZERO,
                Vector3::new(1.0, 0.0, 0.0),
                1.0,
                0.0
            ),
            None
        );
    }

    #[test]
    fn physics_trajectory_solver_uses_gravity_coefficient() {
        let mut rt = dummy_runtime();
        rt.gravity = -5.0;
        rt.coefficient = 2.0;
        let mut ctx = RuntimeWindow::new(&mut rt);

        let velocity =
            physics_solve_velocity_to_target_2d!(&mut ctx, Vector2::ZERO, Vector2::new(10.0, 0.0), 1.0)
                .expect("test setup must succeed");

        assert_vec2_close(velocity, Vector2::new(10.0, 5.0), 1.0e-6);
    }

    #[test]
    fn collect_subtree_ids_walks_root_and_descendants() {
        use crate::sub_apis::collect_subtree_ids;
        use std::collections::HashMap;

        let n = |v: u32| NodeID::new(v);
        let mut children: HashMap<NodeID, Vec<NodeID>> = HashMap::new();
        children.insert(n(1), vec![n(2), n(3)]);
        children.insert(n(2), vec![n(4), NodeID::nil()]);
        children.insert(n(3), vec![n(5)]);

        let mut ids = collect_subtree_ids(n(1), |id| children.get(&id).cloned().unwrap_or_default());
        ids.sort_by_key(|id| id.index());

        // Root included, all descendants collected once, nil children skipped.
        assert_eq!(ids, vec![n(1), n(2), n(3), n(4), n(5)]);
    }

    #[test]
    fn collect_subtree_ids_nil_root_is_empty() {
        use crate::sub_apis::collect_subtree_ids;

        let ids = collect_subtree_ids(NodeID::nil(), |_| vec![NodeID::new(9)]);
        assert!(ids.is_empty());
    }

}
