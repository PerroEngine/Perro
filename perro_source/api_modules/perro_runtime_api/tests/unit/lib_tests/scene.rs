mod scene {
    use super::*;

    #[test]
    fn node_collection_scene_patch_and_script_are_stored() {
        let collection = node_collection![{
            scene = {
                path = res_path!("res://scenes/player.scn"),
                patch = Node2D {
                    transform: Transform2D {
                        position: Vector2::new(10.0, 3.0),
                    },
                },
            },
            script = res_path!("res://scripts/player.rs"),
        }];

        assert_eq!(collection.scenes.len(), 1);
        let scene = &collection.scenes[0];
        assert_eq!(scene.path.as_ref(), "res://scenes/player.scn");
        assert_eq!(
            scene.script.as_ref().map(|script| script.path.as_ref()),
            Some("res://scripts/player.rs")
        );
        assert_eq!(
            scene.patches.first().map(|patch| patch.node_type()),
            Some(Node2D::NODE_TYPE)
        );
    }

    #[test]
    fn node_collection_script_vars_are_stored() {
        let collection = node_collection![{
            node = Node2D,
            script = {
                path = res_path!("res://scripts/player.rs"),
                vars = {
                    hp: 100_i32,
                    "title": {"Player".to_string()},
                },
            },
        }];

        let script = collection.specs[0].script.as_ref().expect("script");
        assert_eq!(script.path.as_ref(), "res://scripts/player.rs");
        assert_eq!(script.vars.len(), 2);
        assert_eq!(
            script.vars[0].0,
            perro_ids::ScriptMemberID::from_string("hp")
        );
        assert_eq!(
            script.vars[0].1,
            NodeScriptVar::Value(Variant::from(100_i32))
        );
        assert_eq!(
            script.vars[1].0,
            perro_ids::ScriptMemberID::from_string("title")
        );
        assert_eq!(
            script.vars[1].1,
            NodeScriptVar::Value(Variant::from("Player".to_string()))
        );
    }

    #[test]
    fn node_collection_key_vars_root_and_patch_list_are_stored() {
        let collection = node_collection![
            root: { node = Node2D },
            follower: {
                parent = @root,
                node = Node2D,
                script = {
                    path = res_path!("res://scripts/follower.rs"),
                    vars = {
                        target: @root,
                    },
                },
            },
            {
                scene = {
                    path = res_path!("res://scenes/player.scn"),
                    patch = [
                        Node2D {
                            transform: Transform2D {
                                position: Vector2::new(1.0, 2.0),
                            },
                        },
                    ],
                },
            },
            root = @follower,
        ];

        assert_eq!(collection.root, Some(1));
        let script = collection.specs[1].script.as_ref().expect("script");
        assert_eq!(
            script.vars[0],
            (
                perro_ids::ScriptMemberID::from_string("target"),
                NodeScriptVar::NodeRef(0),
            )
        );
        assert_eq!(collection.scenes[0].patches.len(), 1);
    }

    #[test]
    fn node_collection_key_defaults_name_and_parent_refs() {
        let collection = node_collection![
            root: { node = Node2D },
            sprite: {
                parent = @root,
                node = Node2D,
            },
            {
                parent = @root,
                node = Node2D,
            }
        ];

        assert_eq!(collection.specs.len(), 3);
        assert_eq!(collection.specs[0].name.as_deref(), Some("root"));
        assert_eq!(collection.specs[1].name.as_deref(), Some("sprite"));
        assert_eq!(collection.specs[2].name, None);
        assert_eq!(collection.specs[0].parent, None);
        assert_eq!(collection.specs[1].parent, Some(0));
        assert_eq!(collection.specs[2].parent, Some(0));
    }

    #[test]
    fn node_collection_key_name_can_override_default() {
        let collection = node_collection![
            player: {
                name = "PlayerRoot",
                node = Node2D,
                children = [
                    sprite: { node = Node2D },
                    { node = Node2D }
                ],
            }
        ];

        assert_eq!(collection.specs.len(), 3);
        assert_eq!(collection.specs[0].name.as_deref(), Some("PlayerRoot"));
        assert_eq!(collection.specs[1].name.as_deref(), Some("sprite"));
        assert_eq!(collection.specs[1].parent, Some(0));
        assert_eq!(collection.specs[2].parent, Some(0));
    }

    #[test]
    fn script_macros_typecheck_and_forward() {
        let mut rt = DummyRuntime {
            state: Box::new(5_i32),
            gravity: -9.81,
            coefficient: 1.0,
        };
        let mut ctx = RuntimeWindow::new(&mut rt);
        let id = NodeID::new(42);

        let initial = with_state!(&mut ctx, i32, id, |state| *state);
        assert_eq!(initial, Some(5));

        let _ = with_state_mut!(&mut ctx, i32, id, |state| {
            *state += 7;
        });
        let updated = with_state!(&mut ctx, i32, id, |state| *state);
        assert_eq!(updated, Some(12));

        let _new_node = create_node!(&mut ctx, Node2D);
        let _root_nodes = create_nodes!(
            &mut ctx,
            node_collection![
                { node = Node2D::new() },
                { name = "root", node = Node2D::new() },
            ]
        );
        let _new_nodes = create_nodes!(
            &mut ctx,
            node_collection![{
                name = "child",
                tags = tags!["spawned"],
                node = Node2D::new()
            }],
            id
        );
        with_node_mut!(&mut ctx, Node2D, id, |_node| {});
        let value = with_node!(&mut ctx, Node2D, id, |_node| 99_i32);
        assert_eq!(value, None);
        let _ = with_base_node!(&mut ctx, Node2D, id, |_node| 1_i32);
        let _ = with_base_node_mut!(&mut ctx, Node2D, id, |_node| 2_i32);
        assert_eq!(get_node_name!(&mut ctx, id), None);
        assert!(!set_node_name!(&mut ctx, id, "player"));
        assert!(!set_ui_rotation!(&mut ctx, id, 0.5));
        assert_eq!(get_node_parent_id!(&mut ctx, id), None);
        assert_eq!(get_node_children_ids!(&mut ctx, id), None);
        assert_eq!(get_node_type!(&mut ctx, id), None);
        assert_eq!(get_node_tags!(&mut ctx, id), None);
        assert!(!crate::set_tags!(&mut ctx, id, tags!["player", "enemy"]));
        assert!(!crate::set_tags!(&mut ctx, id));
        assert!(!tag_add!(&mut ctx, id, "player"));
        assert!(!tag_remove!(&mut ctx, id, "player"));
        assert!(query!(&mut ctx, all(tags["player"], not(tags["enemy"]))).is_empty());
        let player_tag = "player".to_string();
        assert!(query!(&mut ctx, all(tags[player_tag.as_str()])).is_empty());
        assert!(query!(&mut ctx, all(node_type[Node2D], base_type[Node3D])).is_empty());
        assert!(query!(&mut ctx, all(layers[1], mask[2, 3])).is_empty());
        let layer = 4usize;
        assert!(query!(&mut ctx, all(layers[layer], mask[layer])).is_empty());
        let expr = query_expr!(all(tags["player"], not(tags["enemy"])));
        assert!(matches!(expr, QueryExpr::All(_)));
        let reusable_query = query_builder!(all(tags["player"]), in_subtree(id));
        assert_eq!(reusable_query.scope, QueryScope::Subtree(id));
        assert!(query!(&mut ctx, &reusable_query).is_empty());
        let original_scope = reusable_query.scope;
        assert!(query!(&mut ctx, &reusable_query, in_subtree(NodeID::new(7))).is_empty());
        assert_eq!(reusable_query.scope, original_scope);
        assert!(query!(&mut ctx, query_builder!(all(tags["player"]))).is_empty());
        assert!(query_first!(&mut ctx, &reusable_query).is_none());
        let direct_query = NodeQuery::new().where_expr(QueryExpr::Name(vec!["Player".to_string()]));
        assert!(ctx.NodeQuery().query(&direct_query).is_empty());
        assert!(!reparent!(&mut ctx, NodeID::new(1), id));
        assert_eq!(reparent_multi!(&mut ctx, NodeID::new(1), [id]), 0);
        assert!(!remove_node!(&mut ctx, id));
        assert_eq!(get_global_transform_2d!(&mut ctx, id), None);
        assert_eq!(get_global_transform_3d!(&mut ctx, id), None);
        assert_eq!(get_local_transform_2d!(&mut ctx, id), None);
        assert_eq!(get_local_transform_3d!(&mut ctx, id), None);
        assert!(!set_global_transform_2d!(
            &mut ctx,
            id,
            Transform2D::new(Vector2::new(1.0, 2.0), 0.5, Vector2::ONE)
        ));
        assert!(!set_global_transform_3d!(
            &mut ctx,
            id,
            Transform3D::new(
                Vector3::new(1.0, 2.0, 3.0),
                Quaternion::IDENTITY,
                Vector3::ONE
            )
        ));
        assert!(!set_local_transform_2d!(
            &mut ctx,
            id,
            Transform2D::new(Vector2::new(1.0, 2.0), 0.5, Vector2::ONE)
        ));
        assert!(!set_local_transform_3d!(
            &mut ctx,
            id,
            Transform3D::new(
                Vector3::new(1.0, 2.0, 3.0),
                Quaternion::IDENTITY,
                Vector3::ONE
            )
        ));
        assert_eq!(get_local_pos_2d!(&mut ctx, id), None);
        assert_eq!(get_local_pos_3d!(&mut ctx, id), None);
        assert!(!set_local_pos_2d!(&mut ctx, id, Vector2::new(1.0, 2.0)));
        assert!(!set_local_pos_3d!(
            &mut ctx,
            id,
            Vector3::new(1.0, 2.0, 3.0)
        ));
        assert_eq!(get_global_pos_2d!(&mut ctx, id), None);
        assert_eq!(get_global_pos_3d!(&mut ctx, id), None);
        assert!(!set_global_pos_2d!(&mut ctx, id, Vector2::new(1.0, 2.0)));
        assert!(!set_global_pos_3d!(
            &mut ctx,
            id,
            Vector3::new(1.0, 2.0, 3.0)
        ));
        assert_eq!(get_local_rot_2d!(&mut ctx, id), None);
        assert_eq!(get_local_rot_3d!(&mut ctx, id), None);
        assert!(!set_local_rot_2d!(&mut ctx, id, 0.5));
        assert!(!set_local_rot_3d!(&mut ctx, id, Quaternion::IDENTITY));
        assert_eq!(get_global_rot_2d!(&mut ctx, id), None);
        assert_eq!(get_global_rot_3d!(&mut ctx, id), None);
        assert!(!set_global_rot_2d!(&mut ctx, id, 0.5));
        assert!(!set_global_rot_3d!(&mut ctx, id, Quaternion::IDENTITY));
        assert_eq!(get_local_scale_2d!(&mut ctx, id), None);
        assert_eq!(get_local_scale_3d!(&mut ctx, id), None);
        assert!(!set_local_scale_2d!(&mut ctx, id, Vector2::ONE));
        assert!(!set_local_scale_3d!(&mut ctx, id, Vector3::ONE));
        assert_eq!(get_global_scale_2d!(&mut ctx, id), None);
        assert_eq!(get_global_scale_3d!(&mut ctx, id), None);
        assert!(!set_global_scale_2d!(&mut ctx, id, Vector2::ONE));
        assert!(!set_global_scale_3d!(&mut ctx, id, Vector3::ONE));
        assert_eq!(
            to_global_point_2d!(&mut ctx, id, Vector2::new(1.0, 0.0)),
            None
        );
        assert_eq!(
            to_local_point_2d!(&mut ctx, id, Vector2::new(1.0, 0.0)),
            None
        );
        assert_eq!(
            to_global_point_3d!(&mut ctx, id, Vector3::new(1.0, 0.0, 0.0)),
            None
        );
        assert_eq!(
            to_local_point_3d!(&mut ctx, id, Vector3::new(1.0, 0.0, 0.0)),
            None
        );
        assert_eq!(
            to_global_transform_2d!(
                &mut ctx,
                id,
                Transform2D::new(Vector2::new(1.0, 2.0), 0.5, Vector2::ONE)
            ),
            None
        );
        assert_eq!(
            to_local_transform_2d!(
                &mut ctx,
                id,
                Transform2D::new(Vector2::new(1.0, 2.0), 0.5, Vector2::ONE)
            ),
            None
        );
        assert_eq!(
            to_global_transform_3d!(
                &mut ctx,
                id,
                Transform3D::new(
                    Vector3::new(1.0, 2.0, 3.0),
                    Quaternion::IDENTITY,
                    Vector3::ONE
                )
            ),
            None
        );
        assert_eq!(
            to_local_transform_3d!(
                &mut ctx,
                id,
                Transform3D::new(
                    Vector3::new(1.0, 2.0, 3.0),
                    Quaternion::IDENTITY,
                    Vector3::ONE
                )
            ),
            None
        );
        assert_eq!(
            mesh_instance_surface_at_global_point_3d!(&mut ctx, id, Vector3::new(0.0, 0.0, 0.0)),
            None
        );
        assert_eq!(
            mesh_instance_surface_global_point_3d!(&mut ctx, id, 0, Vector3::new(0.5, 0.25, 0.25)),
            None
        );
        assert_eq!(
            mesh_instance_surface_on_global_ray_3d!(
                &mut ctx,
                id,
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::new(0.0, -1.0, 0.0),
                100.0
            ),
            None
        );
        assert_eq!(
            mesh_instance_surfaces_on_global_rays_3d!(
                &mut ctx,
                id,
                &[MeshSurfaceRay3D {
                    origin: Vector3::new(0.0, 1.0, 0.0),
                    direction: Vector3::new(0.0, -1.0, 0.0),
                    max_distance: 100.0,
                }],
                false
            ),
            vec![None]
        );
        let direct_hits = ctx.MeshQuery().instance_surfaces_on_global_rays(
            id,
            &[MeshSurfaceRay3D {
                origin: Vector3::new(0.0, 1.0, 0.0),
                direction: Vector3::new(0.0, -1.0, 0.0),
                max_distance: 100.0,
            }],
            false,
        );
        assert_eq!(direct_hits, vec![None]);
        assert!(
            mesh_instance_material_regions_3d!(&mut ctx, id, perro_ids::MaterialID::new(1)).is_empty()
        );
        assert!(apply_force!(&mut ctx, id, Vector2::new(8.0, 0.0)));
        assert!(apply_force!(&mut ctx, id, Vector3::new(0.0, 3.5, 0.0)));
        assert!(apply_impulse!(&mut ctx, id, Vector2::new(0.0, 1.25)));
        assert!(apply_impulse!(&mut ctx, id, Vector3::new(2.75, 0.0, 0.0)));
        assert_eq!(physics_predict_body_2d!(&mut ctx, id, 1.0), None);
        assert_eq!(
            physics_predict_body_3d!(&mut ctx, id, 1.0, Vector3::new(0.5, 0.0, 0.0)),
            None
        );
        assert_eq!(physics_get_body_gravity_scale!(&mut ctx, id), None);
        assert!(!physics_set_body_gravity_scale!(&mut ctx, id, 0.5));
        assert_eq!(
            physics_raycast_3d!(
                &mut ctx,
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::new(0.0, -1.0, 0.0),
                100.0
            ),
            None
        );
        assert_eq!(
            physics_raycast_3d_with_areas!(
                &mut ctx,
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::new(0.0, -1.0, 0.0),
                100.0
            ),
            None
        );
        assert_eq!(
            physics_raycast_3d_without_areas!(
                &mut ctx,
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::new(0.0, -1.0, 0.0),
                100.0
            ),
            None
        );
        physics_pause!(&mut ctx, true);
        assert!(!physics_is_paused!(&mut ctx));
        assert!(!script_attach!(&mut ctx, id, "res://scripts/a.rs"));
        assert!(!script_detach!(&mut ctx, id));
        assert!(script_set_update_enabled!(&mut ctx, id, false));
        assert!(script_set_fixed_update_enabled!(&mut ctx, id, true));
        let member = var!("x");
        let member_alias = sid!("x");
        let var_member = var!("x");
        let method_member = method!("x");
        let func_member = func!("x");
        let signal_member = signal!("on_test");
        assert_eq!(member, member_alias);
        assert_eq!(member, var_member);
        assert_eq!(member, method_member);
        assert_eq!(member, func_member);
        assert_eq!(signal_member, perro_ids::SignalID::from_string("on_test"));
        timer_start!(&mut ctx, std::time::Duration::from_secs(1), "literal_wait");
        let timer_name = String::from("dynamic_wait");
        timer_start!(
            &mut ctx,
            std::time::Duration::from_millis(2),
            timer_name.as_str()
        );
        assert!(!timer_is_active!(&mut ctx, timer_name.as_str()));
        assert_eq!(timer_remaining!(&mut ctx, timer_name.as_str()), None);
        assert!(timer_cancel!(&mut ctx, timer_name.as_str()));
        assert_eq!(
            timer_finished!(timer_name.as_str()),
            perro_ids::SignalID::from_string("dynamic_wait_finished")
        );
        let _value = get_var!(&mut ctx, id, member);
        set_var!(&mut ctx, id, member, variant!(perro_variant::Variant::Null));
        set_var!(&mut ctx, id, member, variant!(77_i32));
        let _result = call_method!(&mut ctx, id, method_member, &[]);
        let _result2 = call_method!(&mut ctx, id, member, params![1_i32, "abc"]);
        assert!(signal_connect!(
            &mut ctx,
            id,
            signal!("on_test"),
            method!("handle")
        ));
        assert!(signal_connect!(
            &mut ctx,
            id,
            signal!("on_test_with_params"),
            method!("handle"),
            params!["button_a"]
        ));
        assert_eq!(
            signal_connect_many!(
                &mut ctx,
                id,
                &[signal!("on_a"), signal!("on_b")],
                [func!("handle_many")]
            ),
            2
        );
        assert_eq!(
            signal_connect_many!(
                &mut ctx,
                id,
                [signal!("on_c")],
                &[func!("handle_c"), func!("handle_c_extra")],
                params!["button_b"]
            ),
            2
        );
        assert_eq!(
            ctx.Signals().connect_many(
                id,
                vec![signal!("on_d"), signal!("on_e")],
                vec![func!("handle_d"), func!("handle_e")],
                &[]
            ),
            4
        );
        assert!(
            ctx.Signals()
                .connect(id, signal!("on_direct"), func!("handle_direct"), &[])
        );
        assert!(signal_disconnect!(
            &mut ctx,
            id,
            signal!("on_test"),
            method!("handle")
        ));
        assert_eq!(
            signal_disconnect_many!(
                &mut ctx,
                id,
                &[signal!("on_a"), signal!("on_b")],
                [func!("handle_many")]
            ),
            2
        );
        assert_eq!(
            signal_disconnect_many!(
                &mut ctx,
                id,
                [signal!("on_c")],
                &[func!("handle_c"), func!("handle_c_extra")]
            ),
            2
        );
        assert_eq!(
            ctx.Signals().disconnect_many(
                id,
                vec![signal!("on_d"), signal!("on_e")],
                vec![func!("handle_d"), func!("handle_e")]
            ),
            4
        );
        assert!(
            ctx.Signals()
                .disconnect(id, signal!("on_direct"), func!("handle_direct"))
        );
        assert_eq!(
            signal_emit!(&mut ctx, signal!("on_test"), params![1_i32]),
            1
        );
        assert_eq!(signal_emit!(&mut ctx, signal!("on_test")), 1);
        assert_eq!(ctx.Signals().emit(signal!("on_test"), &[]), 1);
        assert_eq!(
            scene_load!(&mut ctx, "res://scenes/a.scene"),
            Ok(NodeID::new(7))
        );
        assert_eq!(
            scene_load!(&mut ctx, String::from("res://scenes/b.scene")),
            Ok(NodeID::new(7))
        );
        let cow_path = std::borrow::Cow::Borrowed("res://scenes/c.scene");
        assert_eq!(scene_load!(&mut ctx, cow_path), Ok(NodeID::new(7)));
        assert_eq!(
            ctx.Scene()
                .load_typed("res://scenes/typed.scene")
                .expect("typed scene load"),
            NodeID::new(7)
        );
        let preloaded = scene_preload!(&mut ctx, "res://scenes/preloaded.scene")
            .expect("preload should return deterministic id");
        assert_eq!(preloaded, PreloadedSceneID::from_u64(11));
        assert_eq!(scene_load!(&mut ctx, preloaded), Ok(NodeID::new(8)));
        assert_eq!(
            ctx.Scene()
                .load_preloaded_typed(PreloadedSceneID::from_u64(99))
                .expect_err("test call must fail"),
            LoadError::Legacy("bad preloaded scene id".to_string())
        );
        assert!(scene_drop_preloaded!(&mut ctx, preloaded));
        assert!(scene_drop_preloaded!(
            &mut ctx,
            "res://scenes/preloaded.scene"
        ));

        let dt = delta_time!(&mut ctx);
        let dt_capped = delta_time_capped!(&mut ctx, 0.010);
        let dt_clamped = delta_time_clamped!(&mut ctx, 0.020, 0.030);
        let fdt = fixed_delta_time!(&mut ctx);
        let elapsed = elapsed_time!(&mut ctx);
        let sim = simulation_time!(&mut ctx);
        let gfx = graphics_time!(&mut ctx);
        let frame = frame_time!(&mut ctx);
        let fps_value = fps!(&mut ctx);
        let profile = profiling!(&mut ctx);
        assert_eq!(dt, 0.016);
        assert_eq!(dt_capped, 0.010);
        assert_eq!(dt_clamped, 0.020);
        assert_eq!(fdt, 0.016);
        assert_eq!(elapsed, 1.0);
        assert_eq!(sim, Duration::from_micros(1_000));
        assert_eq!(gfx, Duration::from_micros(2_000));
        assert_eq!(frame, Duration::from_micros(16_000));
        assert_eq!(fps_value, 60.0);
        assert_eq!(
            profile,
            ProfilingSnapshot {
                simulation_time: Duration::from_micros(1_000),
                graphics_time: Duration::from_micros(2_000),
                frame_time: Duration::from_micros(16_000),
                fps: 60.0,
                draw_gpu_prepare_3d: Duration::ZERO,
                draw_gpu_prepare_3d_frustum: Duration::ZERO,
                draw_gpu_prepare_3d_hiz: Duration::ZERO,
                draw_gpu_prepare_3d_indirect: Duration::ZERO,
                draw_gpu_prepare_3d_cull_inputs: Duration::ZERO,
                draw_calls_2d: 0,
                draw_calls_3d: 0,
                draw_calls_total: 0,
                sprite_batches_2d: 0,
                sprite_bind_group_switches_2d: 0,
                draw_batches_3d: 0,
                pipeline_switches_3d: 0,
                texture_bind_group_switches_3d: 0,
                draw_instances_3d: 0,
                draw_material_refs_3d: 0,
                skip_prepare_3d: 0,
                skip_prepare_3d_frustum: 0,
                skip_prepare_3d_hiz: 0,
                skip_prepare_3d_indirect: 0,
                skip_prepare_3d_cull_inputs: 0,
            }
        );
    }

}
