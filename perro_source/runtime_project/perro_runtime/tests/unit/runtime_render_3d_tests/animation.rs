mod animation {
    use super::*;

    #[test]
    fn skinned_mesh_palette_uses_bone_pose_not_rest() {
        let mut runtime = Runtime::new();

        let mut skeleton = Skeleton3D::default();
        skeleton.bones = vec![Bone3D {
            rest: Transform3D::IDENTITY,
            pose: Transform3D::new(
                Vector3::new(2.0, 0.0, 0.0),
                Quaternion::IDENTITY,
                Vector3::ONE,
            ),
            inv_bind: Transform3D::IDENTITY,
            ..Bone3D::new()
        }];
        // Populate the derived inv-bind lane like a real scene load so the palette
        // builder takes the cached (non-fallback) path.
        skeleton.refresh_inv_bind_cache();
        let skeleton_id = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Skeleton3D(skeleton)));

        let mut mesh = MeshInstance3D::new();
        mesh.mesh = MeshID::from_parts(340, 0);
        mesh.skeleton = skeleton_id;
        set_primary_material(&mut mesh, MaterialID::from_parts(341, 0));
        runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(
                    command_3d.as_ref(),
                    Command3D::Draw {
                        skeleton: Some(palette),
                        ..
                        // Palette rows are affine (row-major); translation.x is row0[3].
                    } if palette.matrices.first().is_some_and(|m| m[0][3] == 2.0)
                )
        )));
    }

    #[test]
    fn dirty_skeleton_refreshes_sibling_skinned_mesh_draw() {
        let mut runtime = Runtime::new();

        let mut skeleton = Skeleton3D::default();
        skeleton.bones = vec![Bone3D {
            pose: Transform3D::IDENTITY,
            ..Bone3D::new()
        }];
        let skeleton_id = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Skeleton3D(skeleton)));

        let mut mesh = MeshInstance3D::new();
        mesh.mesh = MeshID::from_parts(350, 0);
        mesh.skeleton = skeleton_id;
        set_primary_material(&mut mesh, MaterialID::from_parts(351, 0));
        let mesh_id = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

        runtime.extract_render_3d_commands();
        let _ = collect_commands(&mut runtime);

        if let Some(mut node) = runtime.nodes.get_mut(skeleton_id)
            && let SceneNodeData::Skeleton3D(skeleton) = &mut node.data
        {
            skeleton.bones[0].pose.position.x = 3.0;
        }
        runtime.mark_needs_rerender(skeleton_id);

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(
                    command_3d.as_ref(),
                    Command3D::Draw {
                        node,
                        skeleton: Some(palette),
                        ..
                    } if *node == mesh_id
                        && palette.matrices.first().is_some_and(|m| m[0][3] == 3.0)
                )
        )));
    }

    #[test]
    fn active_camera_3d_emits_set_camera_command() {
        let mut runtime = Runtime::new();
        let mut camera = Camera3D {
            active: true,
            projection: CameraProjection::Orthographic {
                size: 24.0,
                near: 0.2,
                far: 600.0,
            },
            ..Default::default()
        };
        camera.transform.position.x = 6.0;
        camera.transform.position.y = 7.0;
        camera.transform.position.z = 8.0;
        camera.transform.rotation.x = 0.1;
        camera.transform.rotation.y = 0.2;
        camera.transform.rotation.z = 0.3;
        camera.transform.rotation.w = 0.9;
        runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Camera3D(camera)));

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(
                    command_3d.as_ref(),
                    Command3D::SetCamera { camera }
                        if camera.position == [6.0, 7.0, 8.0]
                            && camera.rotation == [0.1, 0.2, 0.3, 0.9]
                            && matches!(
                                camera.projection,
                                CameraProjectionState::Orthographic { size, near, far }
                                    if size == 24.0 && near == 0.2 && far == 600.0
                            )
                )
        )));
    }

    #[test]
    fn deactivating_last_camera_3d_resets_renderer_camera() {
        let mut runtime = Runtime::new();
        let mut camera = Camera3D {
            active: true,
            ..Default::default()
        };
        camera.transform.position.x = 12.0;
        let camera_node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Camera3D(camera)));

        runtime.extract_render_3d_commands();
        let _ = collect_commands(&mut runtime);

        NodeAPI::with_node_mut::<Camera3D, _, _>(&mut runtime, camera_node, |camera| {
            camera.active = false;
        });
        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);

        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(
                    command_3d.as_ref(),
                    Command3D::SetCamera { camera } if camera.position == [0.0, 0.0, 6.0]
                )
        )));
    }

    #[test]
    fn newly_activated_camera_3d_wins_over_higher_slot_old_camera() {
        let mut runtime = Runtime::new();
        let dummy = NodeAPI::create::<Node3D>(&mut runtime);
        let old_camera = NodeAPI::create::<Camera3D>(&mut runtime);
        NodeAPI::with_node_mut::<Camera3D, _, _>(&mut runtime, old_camera, |camera| {
            camera.active = true;
            camera.transform.position.x = 1.0;
        });
        let _ = NodeAPI::remove_node(&mut runtime, dummy);
        let new_camera = NodeAPI::create::<Camera3D>(&mut runtime);
        NodeAPI::with_node_mut::<Camera3D, _, _>(&mut runtime, new_camera, |camera| {
            camera.active = true;
            camera.transform.position.x = 9.0;
        });

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);

        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(
                    command_3d.as_ref(),
                    Command3D::SetCamera { camera } if camera.position[0] == 9.0
                )
        )));
    }

    #[test]
    fn camera_3d_render_mask_filters_meshes() {
        let mut runtime = Runtime::new();
        let camera = Camera3D {
            active: true,
            render_mask: BitMask::with([2]),
            ..Default::default()
        };
        let camera_node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Camera3D(camera)));

        let mut mesh = MeshInstance3D::new();
        mesh.mesh = MeshID::from_parts(92, 0);
        mesh.render_layers = BitMask::with([2]);
        set_primary_material(&mut mesh, MaterialID::from_parts(93, 0));
        let mesh_node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        assert!(!first.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(command_3d.as_ref(), Command3D::Draw { node, .. } if *node == mesh_node)
        )));

        if let Some(mut node) = runtime.nodes.get_mut(camera_node)
            && let SceneNodeData::Camera3D(camera) = &mut node.data
        {
            camera.render_mask = BitMask::with([1]);
        }
        runtime.mark_needs_rerender(camera_node);

        runtime.extract_render_3d_commands();
        let second = collect_commands(&mut runtime);
        assert!(second.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(command_3d.as_ref(), Command3D::Draw { node, .. } if *node == mesh_node)
        )));
    }

    #[test]
    fn camera_3d_move_does_not_rewalk_mesh_render_layers() {
        let mut runtime = Runtime::new();
        let camera = Camera3D {
            active: true,
            ..Default::default()
        };
        let camera_node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Camera3D(camera)));

        let mut mesh = MeshInstance3D::new();
        mesh.mesh = MeshID::from_parts(95, 0);
        set_primary_material(&mut mesh, MaterialID::from_parts(96, 0));
        let mesh_node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

        runtime.extract_render_3d_commands();
        let _ = collect_commands(&mut runtime);

        if let Some(mut node) = runtime.nodes.get_mut(camera_node)
            && let SceneNodeData::Camera3D(camera) = &mut node.data
        {
            camera.transform.position.x = 10.0;
        }
        runtime.mark_transform_dirty_recursive(camera_node);

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(command_3d.as_ref(), Command3D::SetCamera { .. })
        )));
        assert!(!commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(command_3d.as_ref(), Command3D::Draw { node, .. } if *node == mesh_node)
        )));
    }

    #[test]
    fn active_ray_light_3d_emits_set_ray_light_command() {
        let mut runtime = Runtime::new();
        let mut light = RayLight3D::new();
        light.color = Color::new(0.8, 0.7, 0.6, 1.0);
        light.intensity = 2.5;
        light.shadow_strength = 0.55;
        light.shadow_depth_bias = 0.001;
        light.shadow_normal_bias = 0.12;
        light.active = true;
        runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::RayLight3D(light)));

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(
                    command_3d.as_ref(),
                    Command3D::SetRayLight { light, .. }
                        if light.color == Color::new(0.8, 0.7, 0.6, 1.0).to_rgb()
                            && light.intensity == 2.5
                            && light.shadow_strength == 0.55
                            && light.shadow_depth_bias == 0.001
                            && light.shadow_normal_bias == 0.12
                )
        )));
    }

    #[test]
    fn active_ambient_light_3d_emits_set_ambient_light_command() {
        let mut runtime = Runtime::new();
        let mut light = AmbientLight3D::new();
        light.color = Color::new(0.25, 0.3, 0.4, 1.0);
        light.intensity = 0.2;
        light.active = true;
        runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::AmbientLight3D(light)));

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(
                    command_3d.as_ref(),
                    Command3D::SetAmbientLight { light, .. }
                        if light.color == Color::new(0.25, 0.3, 0.4, 1.0).to_rgb()
                            && light.intensity == 0.2
                )
        )));
    }

    #[test]
    fn active_sky_3d_emits_set_sky_command() {
        let mut runtime = Runtime::new();
        let mut sky = Sky3D::default();
        sky.palette.day_colors = vec![[0.4, 0.6, 0.9], [0.9, 0.95, 1.0]];
        sky.palette.evening_colors = vec![[0.95, 0.45, 0.22], [0.7, 0.2, 0.35]];
        sky.palette.night_colors = vec![[0.01, 0.02, 0.05], [0.04, 0.08, 0.18]];
        sky.time.time_of_day = 0.67;
        sky.time.paused = true;
        sky.time.scale = 0.25;
        sky.active = true;
        runtime.nodes.insert(SceneNode::new(sky.into()));

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(
                    command_3d.as_ref(),
                    Command3D::SetSky { sky, .. }
                        if sky.time.time_of_day == 0.67
                            && sky.time.paused
                            && sky.time.scale == 0.25
                            && sky.day_colors.len() == 2
                            && sky.evening_colors.len() == 2
                            && sky.night_colors.len() == 2
                )
        )));
    }

    #[test]
    fn unchanged_sky_3d_does_not_reemit_set_sky_command() {
        let mut runtime = Runtime::new();
        let mut sky = Sky3D::default();
        sky.palette.day_colors = vec![[0.4, 0.6, 0.9], [0.9, 0.95, 1.0]];
        sky.active = true;
        let node = runtime.nodes.insert(SceneNode::new(sky.into()));

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, RenderCommand::ThreeD(command_3d) if matches!(command_3d.as_ref(), Command3D::SetSky { .. })))
        );

        // Re-mark the node dirty (via a transform touch) without changing any
        // sky data, so the retained-state comparison runs again on revisit.
        runtime.mark_transform_dirty_recursive(node);
        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        assert!(
            !commands
                .iter()
                .any(|command| matches!(command, RenderCommand::ThreeD(command_3d) if matches!(command_3d.as_ref(), Command3D::SetSky { .. })))
        );
    }

    #[test]
    fn sky_3d_state_matches_compares_all_fields() {
        let mut sky = Sky3D::default();
        sky.palette.day_colors = vec![[0.1, 0.2, 0.3]];
        sky.palette.evening_colors = vec![[0.4, 0.5, 0.6]];
        sky.palette.night_colors = vec![[0.7, 0.8, 0.9]];
        sky.palette.horizon_colors = vec![[0.2, 0.2, 0.2]];
        sky.time.time_of_day = 0.5;
        sky.time.paused = false;
        sky.time.scale = 1.0;
        sky.shaders
            .push(perro_nodes::sky_3d::SkyShaderPass::new("shader_a"));
        sky.environment = Some(perro_nodes::SkyEnvironment::new("res://studio.png"));

        let retained = super::super::Sky3DState {
            day_colors: Arc::from(sky.palette.day_colors.as_slice()),
            evening_colors: Arc::from(sky.palette.evening_colors.as_slice()),
            night_colors: Arc::from(sky.palette.night_colors.as_slice()),
            horizon_colors: Arc::from(sky.palette.horizon_colors.as_slice()),
            time: super::super::SkyTime3DState {
                time_of_day: sky.time.time_of_day,
                paused: sky.time.paused,
                scale: sky.time.scale,
            },
            shaders: Arc::from(
                sky.shaders
                    .iter()
                    .map(|shader| super::super::SkyShaderPass3DState {
                        path: shader.path.clone(),
                        params: Arc::from(shader.params.as_slice()),
                    })
                    .collect::<Vec<_>>(),
            ),
            environment: sky.environment.as_ref().map(|environment| {
                perro_render_bridge::EnvironmentMap3DState {
                    source: environment.source.clone(),
                    intensity: environment.intensity,
                    rotation_degrees: environment.rotation_degrees,
                }
            }),
        };

        assert!(super::super::sky_3d_state_matches(&retained, &sky));

        sky.environment.as_mut().expect("test or bench setup must succeed").intensity = 2.0;
        assert!(!super::super::sky_3d_state_matches(&retained, &sky));
        sky.environment.as_mut().expect("test or bench setup must succeed").intensity = 1.0;

        sky.environment.as_mut().expect("test or bench setup must succeed").rotation_degrees = 90.0;
        assert!(!super::super::sky_3d_state_matches(&retained, &sky));
        sky.environment.as_mut().expect("test or bench setup must succeed").rotation_degrees = 0.0;

        sky.environment.as_mut().expect("test or bench setup must succeed").source = "res://other.png".into();
        assert!(!super::super::sky_3d_state_matches(&retained, &sky));
        sky.environment.as_mut().expect("test or bench setup must succeed").source = "res://studio.png".into();

        sky.time.time_of_day = 0.75;
        assert!(!super::super::sky_3d_state_matches(&retained, &sky));
        sky.time.time_of_day = 0.5;

        sky.shaders
            .push(perro_nodes::sky_3d::SkyShaderPass::new("shader_b"));
        assert!(!super::super::sky_3d_state_matches(&retained, &sky));
    }

    #[test]
    fn mesh_under_parent_uses_global_transform() {
        let mut runtime = Runtime::new();

        let mut parent_node = Node3D::new();
        parent_node.transform.position.x = 15.0;
        let parent = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Node3D(parent_node)));

        let mut mesh = MeshInstance3D::new();
        mesh.mesh = MeshID::from_parts(41, 0);
        set_primary_material(&mut mesh, MaterialID::from_parts(42, 0));
        mesh.transform.position.x = 1.0;
        let child = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

        if let Some(mut parent_node) = runtime.nodes.get_mut(parent) {
            parent_node.add_child(child);
        }
        if let Some(mut child_node) = runtime.nodes.get_mut(child) {
            child_node.parent = parent;
        }
        runtime.mark_transform_dirty_recursive(parent);

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(
                    command_3d.as_ref(),
                    Command3D::Draw { node, model, .. }
                        if *node == child
                            && model[3][0] == 16.0
                            && model[3][1] == 0.0
                            && model[3][2] == 0.0
                )
        )));
    }

    #[test]
    fn mesh_instance_passes_meshlet_override_to_draw_command() {
        let mut runtime = Runtime::new();
        let mut mesh = MeshInstance3D::new();
        mesh.mesh = MeshID::from_parts(420, 0);
        mesh.meshlet_override = Some(false);
        set_primary_material(&mut mesh, MaterialID::from_parts(421, 0));
        let node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(
                    command_3d.as_ref(),
                    Command3D::Draw {
                        node: draw_node,
                        meshlet_override,
                        ..
                    } if *draw_node == node && *meshlet_override == Some(false)
                )
        )));
    }

    #[test]
    fn multi_mesh_instance_passes_meshlet_override_to_draw_command() {
        let mut runtime = Runtime::new();
        let mut multi = MultiMeshInstance3D::new();
        multi.mesh = MeshID::from_parts(430, 0);
        multi.meshlet_override = Some(true);
        set_primary_material_multi(&mut multi, MaterialID::from_parts(431, 0));
        multi.instances = vec![perro_nodes::MultiMeshInstancePose::from_pos_rot(
            Vector3::new(0.0, 0.0, 0.0),
            Quaternion::IDENTITY,
        )];
        let node = runtime.nodes.insert(SceneNode::new(multi.into()));

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        assert!(commands.iter().any(|command| {
            matches!(
                command,
                RenderCommand::ThreeD(command_3d)
                    if matches!(
                        command_3d.as_ref(),
                        Command3D::DrawMulti {
                            node: draw_node,
                            meshlet_override,
                            ..
                        } if *draw_node == node && *meshlet_override == Some(true)
                    )
                    || matches!(
                        command_3d.as_ref(),
                        Command3D::DrawMultiDense {
                            node: draw_node,
                            meshlet_override,
                            ..
                        } if *draw_node == node && *meshlet_override == Some(true)
                    )
            )
        }));
    }

    #[test]
    fn collision_shape_debug_rebuilds_when_parent_moves() {
        let mut runtime = Runtime::new();

        let mut parent_node = Node3D::new();
        parent_node.transform.position.x = 2.0;
        let parent = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Node3D(parent_node)));

        let collision = perro_nodes::CollisionShape3D {
            debug: true,
            shape: Shape3D::Cube {
                size: Vector3::new(2.0, 2.0, 2.0),
            },
            ..Default::default()
        };
        let child = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::CollisionShape3D(collision)));

        if let Some(mut parent_node) = runtime.nodes.get_mut(parent) {
            parent_node.add_child(child);
        }
        if let Some(mut child_node) = runtime.nodes.get_mut(child) {
            child_node.parent = parent;
        }
        runtime.mark_transform_dirty_recursive(parent);

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        let first_x = first
            .iter()
            .find_map(|command| match command {
                RenderCommand::ThreeD(command_3d) => match command_3d.as_ref() {
                    Command3D::DrawDebugLine3D { start, .. } => Some(start[0]),
                    _ => None,
                },
                _ => None,
            })
            .expect("expected collision debug line draw");

        if let Some(mut node) = runtime.nodes.get_mut(parent)
            && let SceneNodeData::Node3D(parent_node) = &mut node.data
        {
            parent_node.transform.position.x = 8.0;
        }
        runtime.mark_transform_dirty_recursive(parent);

        runtime.extract_render_3d_commands();
        let second = collect_commands(&mut runtime);
        let second_x = second
            .iter()
            .find_map(|command| match command {
                RenderCommand::ThreeD(command_3d) => match command_3d.as_ref() {
                    Command3D::DrawDebugLine3D { start, .. } => Some(start[0]),
                    _ => None,
                },
                _ => None,
            })
            .expect("expected collision debug line draw after move");

        assert_ne!(first_x, second_x);
    }
}
