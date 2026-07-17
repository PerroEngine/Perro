mod water_overlays {
    use super::*;

    #[test]
    fn linked_3d_water_mirrors_wake_across_overlap() {
        let mut runtime = Runtime::new();
        let water_a = NodeAPI::create::<WaterBody3D>(&mut runtime);
        let water_b = NodeAPI::create::<WaterBody3D>(&mut runtime);
        for (id, x) in [(water_a, 0.0), (water_b, 12.0)] {
            if let Some(mut node) = runtime.nodes.get_mut(id)
                && let SceneNodeData::WaterBody3D(water) = &mut node.data
            {
                water.transform.position.x = x;
                water.water.shape = perro_nodes::WaterShape::box_volume(Vector3::new(16.0, 4.0, 16.0));
                water.water.depth = 4.0;
            }
        }
        runtime
            .force_water_impacts_3d
            .push(crate::runtime::ForceWaterImpact3D {
                position: Vector3::new(8.4, 0.0, 0.0),
                force: Vector3::new(12.0, 0.0, 0.0),
                strength: 10.0,
                radius: 0.25,
                cavitation: 0.5,
            });

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        let water = water_3d_command(&commands, water_a);

        assert_eq!(water.links.len(), 1);
        assert_eq!(water.impacts.len(), 1);
        assert!(water.impacts[0].strength > 0.0);
        assert!(water.impacts[0].strength < 10.0);
    }

    #[test]
    fn sprite_3d_and_label_3d_emit_projected_ui_commands() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let camera = NodeAPI::create::<Camera3D>(&mut runtime);
        let sprite = NodeAPI::create::<Sprite3D>(&mut runtime);
        let label = NodeAPI::create::<Label3D>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(camera)
            && let SceneNodeData::Camera3D(data) = &mut node.data
        {
            data.active = true;
        }
        if let Some(mut node) = runtime.nodes.get_mut(sprite)
            && let SceneNodeData::Sprite3D(data) = &mut node.data
        {
            data.texture = TextureID::from_parts(12, 0);
            data.transform.position = Vector3::new(0.0, 0.0, -5.0);
        }
        if let Some(mut node) = runtime.nodes.get_mut(label)
            && let SceneNodeData::Label3D(data) = &mut node.data
        {
            data.text = "Name".into();
            data.transform.position = Vector3::new(0.0, 1.0, -5.0);
            data.backdrop_color = perro_structs::Color::new(0.1, 0.2, 0.3, 1.0);
            data.corner_radii = perro_ui::UiCornerRadii::all(0.25);
            data.padding = perro_ui::UiRect::new(0.1, 0.2, 0.1, 0.2);
        }

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);

        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::UpsertImage { node, texture, .. })
                if *node == sprite && *texture == TextureID::from_parts(12, 0)
        )));
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::UpsertLabel {
                node,
                text,
                wrap_width,
                font_size,
                rect,
                backdrop_color,
                corner_radii,
                padding,
                ..
            }) if *node == label
                && text.as_ref() == "Name"
                && wrap_width.is_some_and(|width| width > 0.0 && width < rect.size[0])
                && *font_size <= rect.size[1]
                && *backdrop_color == perro_structs::Color::new(0.1, 0.2, 0.3, 1.0)
                && corner_radii.tl == 0.25
                && *padding == [0.1, 0.2, 0.1, 0.2]
        )));
    }

    #[test]
    fn label_3d_stays_visible_when_rotated_edge_crosses_camera_plane() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let camera = NodeAPI::create::<Camera3D>(&mut runtime);
        let label = NodeAPI::create::<Label3D>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(camera)
            && let SceneNodeData::Camera3D(data) = &mut node.data
        {
            data.active = true;
            data.projection = CameraProjection::Perspective {
                fov_y_degrees: 60.0,
                near: 0.1,
                far: 100.0,
            };
        }
        if let Some(mut node) = runtime.nodes.get_mut(label)
            && let SceneNodeData::Label3D(data) = &mut node.data
        {
            data.text = "Near".into();
            data.size.x = 2.0;
            data.transform.position = Vector3::new(0.0, 0.0, -0.2);
            data.transform.rotation = Quaternion::from_euler_xyz(0.0, 1.2, 0.0);
        }

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);

        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::UpsertLabel { node, .. }) if *node == label
        )));
    }

    #[test]
    fn label_3d_lock_orientation_projects_transform_rotation() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let camera = NodeAPI::create::<Camera3D>(&mut runtime);
        let label = NodeAPI::create::<Label3D>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(camera)
            && let SceneNodeData::Camera3D(data) = &mut node.data
        {
            data.active = true;
        }
        if let Some(mut node) = runtime.nodes.get_mut(label)
            && let SceneNodeData::Label3D(data) = &mut node.data
        {
            data.text = "Turned".into();
            data.lock_orientation = true;
            data.transform.position = Vector3::new(0.0, 0.0, -5.0);
            data.transform.rotation = Quaternion::from_euler_xyz(0.0, 0.0, 0.5);
        }

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);

        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::UpsertLabel { node, rect, projected_quad: Some(quad), .. })
                if *node == label
                    && rect.rotation_radians == 0.0
                    && (quad[1][1] - quad[0][1]).abs() > 0.001
                    && quad.iter().all(|corner| corner[3] > 0.0)
        )));
    }

    #[test]
    fn label_3d_billboard_ignores_world_rotation() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let camera = NodeAPI::create::<Camera3D>(&mut runtime);
        let label = NodeAPI::create::<Label3D>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(camera)
            && let SceneNodeData::Camera3D(data) = &mut node.data
        {
            data.active = true;
        }
        if let Some(mut node) = runtime.nodes.get_mut(label)
            && let SceneNodeData::Label3D(data) = &mut node.data
        {
            data.text = "Billboard".into();
            data.transform.position = Vector3::new(0.0, 0.0, -5.0);
            data.transform.rotation = Quaternion::from_euler_xyz(0.7, 1.1, 0.9);
        }

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);

        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::UpsertLabel { node, rect, .. })
                if *node == label && rect.rotation_radians == 0.0 && rect.size[0] > rect.size[1]
        )));
    }

    #[test]
    fn label_3d_locked_plane_collapses_edge_on() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let camera = NodeAPI::create::<Camera3D>(&mut runtime);
        let label = NodeAPI::create::<Label3D>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(camera)
            && let SceneNodeData::Camera3D(data) = &mut node.data
        {
            data.active = true;
        }
        if let Some(mut node) = runtime.nodes.get_mut(label)
            && let SceneNodeData::Label3D(data) = &mut node.data
        {
            data.text = "Edge".into();
            data.lock_orientation = true;
            data.backface_cull = false;
            data.transform.position = Vector3::new(0.0, 0.0, -5.0);
            data.transform.rotation = Quaternion::from_euler_xyz(0.0, std::f32::consts::FRAC_PI_2, 0.0);
        }

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);

        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::UpsertLabel { node, projected_quad: Some(quad), .. })
                if *node == label
                    && ((quad[1][0] / quad[1][3]) - (quad[0][0] / quad[0][3])).abs() <= 0.0001
        )));
    }

    #[test]
    fn label_3d_locked_layout_size_ignores_projected_rect_size() {
        let rect = perro_render_bridge::UiRectState {
            center: [0.0, 0.0],
            size: [1800.0, 900.0],
            pivot: [0.5, 0.5],
            rotation_radians: 1.0,
            z_index: 0,
        };
        let layout =
            super::super::label_3d_stable_layout_rect(
                rect,
                perro_structs::Vector2::new(2.0, 0.5),
                20.0,
            );

        assert_eq!(layout.size, [80.0, 20.0]);
        assert_eq!(layout.rotation_radians, 0.0);
    }

    #[test]
    fn label_3d_backface_cull_hides_locked_label_facing_away() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let camera = NodeAPI::create::<Camera3D>(&mut runtime);
        let label = NodeAPI::create::<Label3D>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(camera)
            && let SceneNodeData::Camera3D(data) = &mut node.data
        {
            data.active = true;
        }
        if let Some(mut node) = runtime.nodes.get_mut(label)
            && let SceneNodeData::Label3D(data) = &mut node.data
        {
            data.text = "Back".into();
            data.lock_orientation = true;
            data.transform.position = Vector3::new(0.0, 0.0, -5.0);
            data.transform.rotation = Quaternion::from_euler_xyz(0.0, std::f32::consts::PI, 0.0);
        }

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);

        assert!(!commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::UpsertLabel { node, .. }) if *node == label
        )));
    }

    #[test]
    fn sprite_3d_emits_after_async_texture_create_without_other_dirty_work() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let camera = NodeAPI::create::<Camera3D>(&mut runtime);
        let sprite = NodeAPI::create::<Sprite3D>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(camera)
            && let SceneNodeData::Camera3D(data) = &mut node.data
        {
            data.active = true;
        }

        let texture = runtime
            .resource_api
            .load_texture("res://textures/floating_prompt.png");
        let request = collect_resource_texture_request(&mut runtime, texture);
        if let Some(mut node) = runtime.nodes.get_mut(sprite)
            && let SceneNodeData::Sprite3D(data) = &mut node.data
        {
            data.texture = texture;
            data.transform.position = Vector3::new(0.0, 0.0, -5.0);
        }

        runtime.extract_render_3d_commands();
        assert!(
            !collect_commands(&mut runtime)
                .iter()
                .any(|command| matches!(
                    command,
                    RenderCommand::Ui(UiCommand::UpsertImage { node, .. }) if *node == sprite
                ))
        );

        runtime.apply_render_event(RenderEvent::TextureCreated {
            request,
            id: texture,
        });
        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);

        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::UpsertImage { node, texture: id, .. })
                if *node == sprite && *id == texture
        )));
    }

    #[test]
    fn sprite_3d_and_label_3d_hide_when_mesh_blocks_center() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let camera = NodeAPI::create::<Camera3D>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(camera)
            && let SceneNodeData::Camera3D(data) = &mut node.data
        {
            data.active = true;
        }

        let mut blocker = MeshInstance3D::new();
        blocker.mesh = MeshID::from_parts(31, 0);
        blocker.transform.position = Vector3::new(0.0, 0.0, -2.5);
        let blocker = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(blocker)));
        runtime
            .render_3d
            .mesh_sources
            .insert(blocker, "__cube__".to_string());

        let sprite = NodeAPI::create::<Sprite3D>(&mut runtime);
        let label = NodeAPI::create::<Label3D>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(sprite)
            && let SceneNodeData::Sprite3D(data) = &mut node.data
        {
            data.texture = TextureID::from_parts(12, 0);
            data.transform.position = Vector3::new(0.0, 0.0, -5.0);
        }
        if let Some(mut node) = runtime.nodes.get_mut(label)
            && let SceneNodeData::Label3D(data) = &mut node.data
        {
            data.text = "Hidden".into();
            data.transform.position = Vector3::new(0.0, 0.0, -5.0);
        }

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);

        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::RemoveNode { node }) if *node == sprite
        )));
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::RemoveNode { node }) if *node == label
        )));
        assert!(!commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::UpsertImage { node, .. }) if *node == sprite
        )));
        assert!(!commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::UpsertLabel { node, .. }) if *node == label
        )));
    }

    #[test]
    fn sprite_3d_hides_behind_mesh_with_orthographic_camera() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let camera = NodeAPI::create::<Camera3D>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(camera)
            && let SceneNodeData::Camera3D(data) = &mut node.data
        {
            data.active = true;
            data.projection = CameraProjection::Orthographic {
                size: 10.0,
                near: 0.1,
                far: 100.0,
            };
        }

        let mut blocker = MeshInstance3D::new();
        blocker.mesh = MeshID::from_parts(31, 0);
        blocker.transform.position = Vector3::new(2.0, 0.0, -2.5);
        let blocker = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(blocker)));
        runtime
            .render_3d
            .mesh_sources
            .insert(blocker, "__cube__".to_string());

        let sprite = NodeAPI::create::<Sprite3D>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(sprite)
            && let SceneNodeData::Sprite3D(data) = &mut node.data
        {
            data.texture = TextureID::from_parts(12, 0);
            data.transform.position = Vector3::new(2.0, 0.0, -5.0);
        }

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);

        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::RemoveNode { node }) if *node == sprite
        )));
        assert!(!commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::UpsertImage { node, .. }) if *node == sprite
        )));
    }

    #[test]
    fn linked_3d_waters_both_collect_shared_coastline_shape() {
        let mut runtime = Runtime::new();
        let water_a = NodeAPI::create::<WaterBody3D>(&mut runtime);
        let water_b = NodeAPI::create::<WaterBody3D>(&mut runtime);
        for (id, x) in [(water_a, 0.0), (water_b, 12.0)] {
            if let Some(mut node) = runtime.nodes.get_mut(id)
                && let SceneNodeData::WaterBody3D(water) = &mut node.data
            {
                water.transform.position.x = x;
                water.water.shape = perro_nodes::WaterShape::box_volume(Vector3::new(16.0, 4.0, 16.0));
                water.water.depth = 4.0;
            }
        }
        let body = NodeAPI::create::<StaticBody3D>(&mut runtime);
        let shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, body, shape));
        if let Some(mut node) = runtime.nodes.get_mut(shape)
            && let SceneNodeData::CollisionShape3D(shape) = &mut node.data
        {
            shape.transform.position = Vector3::new(6.0, -1.0, 0.0);
            shape.shape = Shape3D::Cube {
                size: Vector3::new(2.0, 2.0, 4.0),
            };
        }

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);

        assert_eq!(
            water_3d_command(&commands, water_a).coastline_shapes.len(),
            1
        );
        assert_eq!(
            water_3d_command(&commands, water_b).coastline_shapes.len(),
            1
        );
    }

    #[test]
    fn water_3d_impacts_use_live_body_pos_not_stale_cached_sample() {
        let mut runtime = Runtime::new();
        let water = NodeAPI::create::<WaterBody3D>(&mut runtime);
        let body = NodeAPI::create::<RigidBody3D>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(body)
            && let SceneNodeData::RigidBody3D(rigid) = &mut node.data
        {
            rigid.transform.position = Vector3::new(1.5, -0.4, -0.75);
            rigid.linear_velocity = Vector3::new(0.0, -2.8, 0.0);
            rigid.mass = 4.0;
            rigid.density = 1.0;
        }
        runtime.time.elapsed = 1.0;
        runtime.apply_render_event(RenderEvent::WaterBodySamples {
            samples: Arc::from([perro_render_bridge::WaterBodySampleState {
                water,
                body,
                point: 0,
                local: [6.0, 4.0],
                height: 2.0,
                velocity: [0.0, 0.0],
                foam: 1.0,
            }]),
        });

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        let water = water_3d_command(&commands, water);

        assert_eq!(water.impacts.len(), 1);
        assert!((water.impacts[0].position[0] - 1.5).abs() < 0.01);
        assert!((water.impacts[0].position[1] + 0.4).abs() < 0.01);
        assert!((water.impacts[0].position[2] + 0.75).abs() < 0.01);
    }

}
