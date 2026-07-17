mod materials {
    use super::*;

    #[test]
    fn mesh_instances_share_identical_inline_material() {
        let mut runtime = Runtime::new();
        let first_node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
                MeshInstance3D::new(),
            )));
        let second_node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
                MeshInstance3D::new(),
            )));
        runtime
            .render_3d
            .mesh_sources
            .insert(first_node, "__cube__".to_string());
        runtime
            .render_3d
            .mesh_sources
            .insert(second_node, "__cube__".to_string());
        let standard = StandardMaterial3D {
            base_color_factor: [0.2, 0.4, 0.8, 1.0],
            ..Default::default()
        };
        let material = Material3D::Standard(standard);
        runtime
            .render_3d
            .material_surface_overrides
            .insert(first_node, vec![Some(material.clone())]);
        runtime
            .render_3d
            .material_surface_overrides
            .insert(second_node, vec![Some(material)]);

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        for (request, mesh) in first
            .iter()
            .filter_map(|command| match command {
                RenderCommand::Resource(ResourceCommand::CreateMesh { request, .. }) => Some(*request),
                _ => None,
            })
            .zip([MeshID::from_parts(22, 0), MeshID::from_parts(23, 0)])
        {
            runtime.apply_render_event(RenderEvent::MeshCreated {
                request,
                id: mesh,
                mesh: None,
            });
        }

        runtime.extract_render_3d_commands();
        let second = collect_commands(&mut runtime);
        let inline_materials: Vec<MaterialID> = second
            .iter()
            .filter_map(|command| match command {
                RenderCommand::Resource(ResourceCommand::CreateMaterial { id, source, .. })
                    if source.is_none() =>
                {
                    Some(*id)
                }
                _ => None,
            })
            .collect();
        assert_eq!(inline_materials.len(), 1);
    }

    #[test]
    fn mesh_instance_keeps_retained_mesh_while_replacement_mesh_is_pending() {
        let mut runtime = Runtime::new();
        let old_mesh = MeshID::from_parts(41, 0);
        let old_material = MaterialID::from_parts(42, 0);
        let mut mesh = MeshInstance3D::new();
        mesh.mesh = old_mesh;
        set_primary_material(&mut mesh, old_material);
        let node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        assert!(first.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command)
                if matches!(
                    command.as_ref(),
                    Command3D::Draw { node: draw_node, mesh, .. }
                        if *draw_node == node && *mesh == old_mesh
                )
        )));

        let pending_mesh = runtime
            .resource_api
            .load_mesh("res://meshes/tool_version_b.glb:mesh[0]");
        let pending_request = collect_commands(&mut runtime)
            .into_iter()
            .find_map(|command| match command {
                RenderCommand::Resource(ResourceCommand::CreateMesh { request, id, .. })
                    if id == pending_mesh =>
                {
                    Some(request)
                }
                _ => None,
            })
            .expect("expected pending mesh create request");
        if let Some(mut scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::MeshInstance3D(mesh) = &mut scene_node.data
        {
            mesh.mesh = pending_mesh;
        }
        runtime.mark_needs_rerender(node);

        runtime.extract_render_3d_commands();
        let pending = collect_commands(&mut runtime);
        assert!(!pending.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command)
                if matches!(command.as_ref(), Command3D::RemoveNode { node: removed } if *removed == node)
        )));
        assert!(!pending.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command)
                if matches!(command.as_ref(), Command3D::Draw { node: draw_node, .. } if *draw_node == node)
        )));
        assert_eq!(
            runtime
                .render_3d
                .retained_mesh_draws
                .get(&node)
                .map(|draw| draw.mesh),
            Some(old_mesh)
        );

        runtime.apply_render_event(RenderEvent::MeshCreated {
            request: pending_request,
            id: pending_mesh,
            mesh: None,
        });
        runtime.mark_needs_rerender(node);
        runtime.extract_render_3d_commands();
        let ready = collect_commands(&mut runtime);
        assert!(ready.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command)
                if matches!(
                    command.as_ref(),
                    Command3D::Draw { node: draw_node, mesh, .. }
                        if *draw_node == node && *mesh == pending_mesh
                )
        )));
    }

    #[test]
    fn mesh_instance_keeps_retained_material_while_replacement_material_is_pending() {
        let mut runtime = Runtime::new();
        let mesh_id = MeshID::from_parts(51, 0);
        let old_material = MaterialID::from_parts(52, 0);
        let mut mesh = MeshInstance3D::new();
        mesh.mesh = mesh_id;
        set_primary_material(&mut mesh, old_material);
        let node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        assert!(first.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command)
                if matches!(
                    command.as_ref(),
                    Command3D::Draw { node: draw_node, surfaces, .. }
                        if *draw_node == node
                            && surfaces
                                .first()
                                .and_then(|surface| surface.material)
                                .is_some_and(|material| material == old_material)
                )
        )));

        let pending_material = runtime
            .resource_api
            .load_material_source("res://materials/tool_version_b.pmat");
        let pending_request =
            collect_commands(&mut runtime)
                .into_iter()
                .find_map(|command| match command {
                    RenderCommand::Resource(ResourceCommand::CreateMaterial {
                        request, id, ..
                    }) if id == pending_material => Some(request),
                    _ => None,
                })
                .expect("expected pending material create request");
        if let Some(mut scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::MeshInstance3D(mesh) = &mut scene_node.data
        {
            set_primary_material(mesh, pending_material);
        }
        runtime.mark_needs_rerender(node);

        runtime.extract_render_3d_commands();
        let pending = collect_commands(&mut runtime);
        assert!(!pending.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command)
                if matches!(command.as_ref(), Command3D::RemoveNode { node: removed } if *removed == node)
        )));
        assert!(!pending.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command)
                if matches!(command.as_ref(), Command3D::Draw { node: draw_node, .. } if *draw_node == node)
        )));
        assert_eq!(
            runtime
                .render_3d
                .retained_mesh_draws
                .get(&node)
                .and_then(|draw| draw.surfaces.first())
                .and_then(|surface| surface.material),
            Some(old_material)
        );

        runtime.apply_render_event(RenderEvent::MaterialCreated {
            request: pending_request,
            id: pending_material,
        });
        runtime.mark_needs_rerender(node);
        runtime.extract_render_3d_commands();
        let ready = collect_commands(&mut runtime);
        assert!(ready.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command)
                if matches!(
                    command.as_ref(),
                    Command3D::Draw { node: draw_node, surfaces, .. }
                        if *draw_node == node
                            && surfaces
                                .first()
                                .and_then(|surface| surface.material)
                                .is_some_and(|material| material == pending_material)
                )
        )));
    }

    #[test]
    fn material_loaded_event_reemits_mesh_draw_using_material() {
        let mut runtime = Runtime::new();
        let mesh_id = MeshID::from_parts(61, 0);
        let material_id = MaterialID::from_parts(62, 0);
        let mut mesh = MeshInstance3D::new();
        mesh.mesh = mesh_id;
        set_primary_material(&mut mesh, material_id);
        let node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        assert!(first.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command)
                if matches!(command.as_ref(), Command3D::Draw { node: draw_node, .. } if *draw_node == node)
        )));

        runtime.apply_render_event(RenderEvent::MaterialLoaded { id: material_id });
        runtime.extract_render_3d_commands();
        let second = collect_commands(&mut runtime);
        assert!(second.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command)
                if matches!(
                    command.as_ref(),
                    Command3D::Draw { node: draw_node, surfaces, .. }
                        if *draw_node == node
                            && surfaces
                                .first()
                                .and_then(|surface| surface.material)
                                .is_some_and(|material| material == material_id)
                )
        )));
    }

    #[test]
    fn animation_player_keeps_old_clip_while_replacement_clip_is_pending() {
        let mut runtime = Runtime::new();
        let target = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
        let player = NodeAPI::create::<AnimationPlayer>(&mut runtime);
        let old_clip = runtime
            .resource_api
            .test_create_animation(node3d_position_clip(&[(0, 1.0), (1, 2.0), (2, 3.0)]), true);
        let pending_clip = runtime
            .resource_api
            .test_create_animation(node3d_position_clip(&[(0, 100.0), (1, 200.0)]), false);

        assert!(runtime.animation_set_clip(player, old_clip));
        assert!(runtime.animation_bind(player, "Tool", target));
        runtime.update(1.0);
        let x_after_old = runtime
            .nodes
            .get(target)
            .and_then(|node| match &node.data {
                SceneNodeData::Node3D(node) => Some(node.transform.position.x),
                _ => None,
            })
            .expect("target node");
        assert_eq!(x_after_old, 1.0);

        assert!(runtime.animation_set_clip(player, pending_clip));
        assert!(runtime.animation_seek_frame(player, 1));
        runtime.update(1.0);
        let x_while_pending = runtime
            .nodes
            .get(target)
            .and_then(|node| match &node.data {
                SceneNodeData::Node3D(node) => Some(node.transform.position.x),
                _ => None,
            })
            .expect("target node");
        assert_eq!(x_while_pending, 2.0);

        runtime
            .resource_api
            .test_mark_animation_loaded(pending_clip);
        runtime.update(1.0);
        let x_after_ready = runtime
            .nodes
            .get(target)
            .and_then(|node| match &node.data {
                SceneNodeData::Node3D(node) => Some(node.transform.position.x),
                _ => None,
            })
            .expect("target node");
        assert!(x_after_ready >= 100.0);
    }

    #[test]
    fn mesh_under_invisible_parent_emits_remove_node() {
        let mut runtime = Runtime::new();
        let parent = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
        let child = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
                MeshInstance3D::new(),
            )));
        if let Some(mut parent_node) = runtime.nodes.get_mut(parent) {
            parent_node.add_child(child);
        }
        if let Some(mut child_node) = runtime.nodes.get_mut(child) {
            child_node.parent = parent;
        }

        let mesh = MeshID::from_parts(20, 0);
        let material = MaterialID::from_parts(21, 0);
        if let Some(mut node) = runtime.nodes.get_mut(child)
            && let SceneNodeData::MeshInstance3D(mesh_instance) = &mut node.data
        {
            mesh_instance.mesh = mesh;
            set_primary_material(mesh_instance, material);
        }

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        assert!(first.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(command_3d.as_ref(), Command3D::Draw { node, .. } if *node == child)
        )));

        if let Some(mut node) = runtime.nodes.get_mut(parent)
            && let SceneNodeData::Node3D(parent_node) = &mut node.data
        {
            parent_node.visible = false;
        }
        runtime.mark_needs_rerender(parent);
        runtime.extract_render_3d_commands();
        let second = collect_commands(&mut runtime);
        assert!(second.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(command_3d.as_ref(), Command3D::RemoveNode { node } if *node == child)
        )));
        assert_eq!(runtime.scene_mesh_refs_cache.get(&mesh), Some(&vec![child]));
        assert_eq!(
            runtime.scene_material_refs_cache.get(&material),
            Some(&vec![child])
        );
    }

    #[test]
    fn removed_water_3d_emits_remove_node() {
        let mut runtime = Runtime::new();
        let water = NodeAPI::create::<WaterBody3D>(&mut runtime);

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        assert!(first.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(command_3d.as_ref(), Command3D::UpsertWater { node, .. } if *node == water)
        )));

        assert!(NodeAPI::remove_node(&mut runtime, water));
        let second = collect_commands(&mut runtime);
        assert!(second.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(command_3d.as_ref(), Command3D::RemoveNode { node } if *node == water)
        )));
    }

    #[test]
    fn physics_pause_keeps_water_3d_visual_state_live() {
        let mut runtime = Runtime::new();
        let water = NodeAPI::create::<WaterBody3D>(&mut runtime);

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        assert!(!water_3d_command(&first, water).paused);
        runtime.clear_dirty_flags();

        runtime.extract_render_3d_commands();
        assert!(collect_commands(&mut runtime).is_empty());

        runtime.set_physics_paused(true);
        runtime.extract_render_3d_commands();
        let paused = collect_commands(&mut runtime);
        assert!(!water_3d_command(&paused, water).paused);
    }

    #[test]
    fn unchanged_mesh_instance_emits_draw() {
        let mut runtime = Runtime::new();
        let node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
                MeshInstance3D::new(),
            )));
        let mesh = MeshID::from_parts(30, 0);
        let material = MaterialID::from_parts(31, 0);
        if let Some(mut scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::MeshInstance3D(mesh_instance) = &mut scene_node.data
        {
            mesh_instance.mesh = mesh;
            set_primary_material(mesh_instance, material);
        }

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(
                    command_3d.as_ref(),
                    Command3D::Draw {
                        node: draw_node, ..
                    } if *draw_node == node
                )
        )));
    }

    #[test]
    fn multi_mesh_instance_emits_draw_multi_with_instance_mats() {
        let mut runtime = Runtime::new();
        let mut multi = MultiMeshInstance3D::new();
        multi.mesh = MeshID::from_parts(330, 0);
        set_primary_material_multi(&mut multi, MaterialID::from_parts(331, 0));

        multi.instance_scale = 1.0;
        multi.instances = vec![
            perro_nodes::MultiMeshInstancePose::from_pos_rot(
                Vector3::new(1.0, 0.0, 0.0),
                Quaternion::IDENTITY,
            ),
            perro_nodes::MultiMeshInstancePose::from_pos_rot(
                Vector3::new(3.0, 0.0, 0.0),
                Quaternion::IDENTITY,
            ),
        ];

        let node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MultiMeshInstance3D(multi)));

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
                            instance_mats,
                            ..
                        } if *draw_node == node
                            && instance_mats.len() == 2
                            && instance_mats[0][3][0] == 1.0
                            && instance_mats[1][3][0] == 3.0
                    )
                    || matches!(
                        command_3d.as_ref(),
                        Command3D::DrawMultiDense {
                            node: draw_node,
                            instances,
                            ..
                        } if *draw_node == node
                            && instances.len() == 2
                            && instances[0].position[0] == 1.0
                            && instances[1].position[0] == 3.0
                    )
            )
        }));
    }

    #[test]
    fn multi_mesh_instance_default_scale_is_one() {
        let multi = MultiMeshInstance3D::default();
        assert_eq!(multi.instance_scale, 1.0);
    }

    #[test]
    fn multi_mesh_instance_passes_instance_scale_to_dense_draw() {
        let mut runtime = Runtime::new();
        let mut multi = MultiMeshInstance3D::new();
        multi.mesh = MeshID::from_parts(332, 0);
        set_primary_material_multi(&mut multi, MaterialID::from_parts(333, 0));
        multi.instances = vec![perro_nodes::MultiMeshInstancePose::new(Transform3D::new(
            Vector3::new(1.0, 2.0, 3.0),
            Quaternion::IDENTITY,
            Vector3::new(2.0, 3.0, 4.0),
        ))];

        let node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MultiMeshInstance3D(multi)));

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command_3d)
                if matches!(
                    command_3d.as_ref(),
                    Command3D::DrawMultiDense {
                        node: draw_node,
                        instances,
                        ..
                    } if *draw_node == node && instances[0].scale == [2.0, 3.0, 4.0]
                )
        )));
    }

}
