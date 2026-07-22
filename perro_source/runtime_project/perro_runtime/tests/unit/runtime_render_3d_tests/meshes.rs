mod meshes {
    use super::*;

    #[test]
    fn mesh_blend_options_reach_draw_command() {
        let mut runtime = Runtime::new();
        let mut mesh = MeshInstance3D::new();
        mesh.mesh = MeshID::from_parts(7, 0);
        set_primary_material(&mut mesh, MaterialID::from_parts(9, 0));
        mesh.blend.enabled = true;
        mesh.blend.screen_blending = false;
        mesh.blend.normal_blending = true;
        mesh.blend.blend_layers = BitMask::with([3]);
        mesh.blend.blend_mask = BitMask::with([2, 4]);
        mesh.blend.distance = 0.75;
        mesh.blend.min_distance = 0.125;
        mesh.blend.noise_factor = 0.5;
        mesh.blend.noise_scale = 12.0;
        let node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        let blend = commands
            .iter()
            .find_map(|command| match command {
                RenderCommand::ThreeD(command) => match command.as_ref() {
                    Command3D::Draw {
                        node: got, blend, ..
                    } if *got == node => Some(*blend),
                    _ => None,
                },
                _ => None,
            })
            .expect("mesh draw command");

        assert!(blend.enabled);
        assert!(!blend.screen_blending);
        assert!(blend.normal_blending);
        assert_eq!(blend.blend_layers, BitMask::with([3]));
        assert_eq!(blend.blend_mask, BitMask::with([2, 4]));
        assert_eq!(blend.distance, 0.75);
        assert_eq!(blend.min_distance, 0.125);
        assert_eq!(blend.noise_factor, 0.5);
        assert_eq!(blend.noise_scale, 12.0);
    }

    #[test]
    fn multimesh_blend_options_reach_dense_draw_command() {
        let mut runtime = Runtime::new();
        let mut multi = MultiMeshInstance3D::new();
        multi.mesh = MeshID::from_parts(8, 0);
        set_primary_material_multi(&mut multi, MaterialID::from_parts(10, 0));
        multi
            .instances
            .push(perro_nodes::MultiMeshInstancePose::from_pos_rot(
                Vector3::ZERO,
                Quaternion::IDENTITY,
            ));
        multi.blend.enabled = true;
        multi.blend.screen_blending = false;
        multi.blend.normal_blending = true;
        multi.blend.blend_layers = BitMask::with([5]);
        multi.blend.blend_mask = BitMask::with([1, 5]);
        multi.blend.distance = 0.25;
        let node = runtime.nodes.insert(SceneNode::new(multi.into()));

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        let blend = commands
            .iter()
            .find_map(|command| match command {
                RenderCommand::ThreeD(command) => match command.as_ref() {
                    Command3D::DrawMultiDense {
                        node: got, blend, ..
                    } if *got == node => Some(*blend),
                    _ => None,
                },
                _ => None,
            })
            .expect("multimesh draw command");

        assert!(blend.enabled);
        assert!(!blend.screen_blending);
        assert!(blend.normal_blending);
        assert_eq!(blend.blend_layers, BitMask::with([5]));
        assert_eq!(blend.blend_mask, BitMask::with([1, 5]));
        assert_eq!(blend.distance, 0.25);
    }

    #[test]
    fn mesh_instance_flip_x_mirrors_model_about_local_origin() {
        let mut runtime = Runtime::new();
        let mut mesh = MeshInstance3D::new();
        mesh.mesh = MeshID::from_parts(11, 0);
        set_primary_material(&mut mesh, MaterialID::from_parts(13, 0));
        mesh.flip_x = true;
        mesh.transform.position = Vector3::new(3.0, 4.0, 5.0);
        let node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        let model = commands
            .iter()
            .find_map(|command| match command {
                RenderCommand::ThreeD(command) => match command.as_ref() {
                    Command3D::Draw {
                        node: got, model, ..
                    } if *got == node => Some(*model),
                    _ => None,
                },
                _ => None,
            })
            .expect("mesh draw command");

        assert_eq!(model[0], [-1.0, 0.0, 0.0, 0.0]);
        assert_eq!(model[1], [0.0, 1.0, 0.0, 0.0]);
        assert_eq!(model[2], [0.0, 0.0, 1.0, 0.0]);
        assert_eq!(model[3], [3.0, 4.0, 5.0, 1.0]);
    }

    #[test]
    fn multimesh_flip_xy_mirrors_node_model_about_local_origin() {
        let mut runtime = Runtime::new();
        let mut multi = MultiMeshInstance3D::new();
        multi.mesh = MeshID::from_parts(12, 0);
        set_primary_material_multi(&mut multi, MaterialID::from_parts(14, 0));
        multi
            .instances
            .push(perro_nodes::MultiMeshInstancePose::from_pos_rot(
                Vector3::ZERO,
                Quaternion::IDENTITY,
            ));
        multi.flip_x = true;
        multi.flip_y = true;
        multi.transform.position = Vector3::new(1.0, 2.0, 3.0);
        let node = runtime.nodes.insert(SceneNode::new(multi.into()));

        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);
        let model = commands
            .iter()
            .find_map(|command| match command {
                RenderCommand::ThreeD(command) => match command.as_ref() {
                    Command3D::DrawMultiDense {
                        node: got,
                        node_model,
                        ..
                    } if *got == node => Some(*node_model),
                    _ => None,
                },
                _ => None,
            })
            .expect("multimesh draw command");

        assert_eq!(model[0], [-1.0, 0.0, 0.0, 0.0]);
        assert_eq!(model[1], [0.0, -1.0, 0.0, 0.0]);
        assert_eq!(model[2], [0.0, 0.0, 1.0, 0.0]);
        assert_eq!(model[3], [1.0, 2.0, 3.0, 1.0]);
    }

    #[test]
    fn mesh_instance_without_mesh_source_requests_nothing() {
        let mut runtime = Runtime::new();
        let mut mesh = MeshInstance3D::new();
        mesh.mesh = MeshID::nil();
        set_primary_material(&mut mesh, MaterialID::nil());
        runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        assert!(first.is_empty());
    }

    #[test]
    fn mesh_instance_requests_missing_assets_once_until_events_arrive() {
        let mut runtime = Runtime::new();
        let mut mesh = MeshInstance3D::new();
        mesh.mesh = MeshID::nil();
        set_primary_material(&mut mesh, MaterialID::nil());
        let expected_node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));
        runtime
            .render_3d
            .mesh_sources
            .insert(expected_node, "__cube__".to_string());

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        assert_eq!(first.len(), 1);
        assert!(matches!(
            &first[0],
            RenderCommand::Resource(ResourceCommand::CreateMesh { source, .. })
                if source == "__cube__"
        ));

        runtime.extract_render_3d_commands();
        let second = collect_commands(&mut runtime);
        assert!(second.is_empty());
    }

    #[test]
    fn mesh_instance_emits_draw_after_mesh_created_and_inline_material_allocated() {
        let mut runtime = Runtime::new();
        let expected_node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
                MeshInstance3D::new(),
            )));
        runtime
            .render_3d
            .mesh_sources
            .insert(expected_node, "__cube__".to_string());

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        let mesh_request = match &first[0] {
            RenderCommand::Resource(ResourceCommand::CreateMesh { request, .. }) => *request,
            _ => panic!("expected mesh create request"),
        };

        let expected_mesh = MeshID::from_parts(9, 1);
        runtime.apply_render_event(RenderEvent::MeshCreated {
            request: mesh_request,
            id: expected_mesh,
            mesh: None,
        });
        runtime.extract_render_3d_commands();
        let second = collect_commands(&mut runtime);
        let expected_material = second
            .iter()
            .find_map(|command| match command {
                RenderCommand::Resource(ResourceCommand::CreateMaterial { id, .. }) => Some(*id),
                _ => None,
            })
            .expect("expected material create command");
        assert!(!expected_material.is_nil());
        let drew_expected = second.iter().any(|command| match command {
            RenderCommand::ThreeD(command) => matches!(
                command.as_ref(),
                Command3D::Draw {
                    node,
                    mesh,
                    surfaces,
                    ..
                } if *node == expected_node
                    && *mesh == expected_mesh
                    && surfaces
                        .first()
                        .and_then(|surface| surface.material)
                        .is_some_and(|id| id == expected_material)
            ),
            _ => false,
        });
        assert!(drew_expected);
    }

    #[test]
    fn node_3d_effective_modulate_inherits_to_child() {
        let mut runtime = Runtime::new();
        let parent = NodeAPI::create::<Node3D>(&mut runtime);
        let child = NodeAPI::create::<MeshInstance3D>(&mut runtime);

        if let Some(mut node) = runtime.nodes.get_mut(parent)
            && let SceneNodeData::Node3D(data) = &mut node.data
        {
            data.modulate.children_modulate = Color::new(0.5, 1.0, 1.0, 1.0);
            data.modulate.self_modulate = Color::RED;
            node.add_child(child);
        }
        if let Some(mut node) = runtime.nodes.get_mut(child)
            && let SceneNodeData::MeshInstance3D(data) = &mut node.data
        {
            data.modulate.self_modulate = Color::new(1.0, 0.25, 1.0, 1.0);
            node.parent = parent;
        }

        let expected = Runtime::color_modulate(
            Color::new(0.5, 1.0, 1.0, 1.0),
            Color::new(1.0, 0.25, 1.0, 1.0),
        );
        assert_eq!(runtime.effective_self_modulate(child), expected);
        assert_eq!(runtime.effective_self_modulate(parent), Color::RED);
    }

    #[test]
    fn effective_modulate_combines_deep_chain_roles() {
        let mut runtime = Runtime::new();
        let root = NodeAPI::create::<Node3D>(&mut runtime);
        let mid = NodeAPI::create::<Node3D>(&mut runtime);
        let leaf = NodeAPI::create::<MeshInstance3D>(&mut runtime);
        let sibling = NodeAPI::create::<MeshInstance3D>(&mut runtime);

        if let Some(mut node) = runtime.nodes.get_mut(root)
            && let SceneNodeData::Node3D(data) = &mut node.data
        {
            data.modulate.modulate = Color::new(0.8, 1.0, 1.0, 1.0);
            data.modulate.self_modulate = Color::new(1.0, 0.1, 0.1, 1.0);
            data.modulate.children_modulate = Color::new(1.0, 0.7, 1.0, 1.0);
            node.add_child(mid);
        }
        if let Some(mut node) = runtime.nodes.get_mut(mid)
            && let SceneNodeData::Node3D(data) = &mut node.data
        {
            data.modulate.modulate = Color::new(1.0, 0.9, 1.0, 1.0);
            data.modulate.self_modulate = Color::new(0.1, 1.0, 0.1, 1.0);
            data.modulate.children_modulate = Color::new(1.0, 1.0, 0.6, 1.0);
            node.parent = root;
            node.add_child(leaf);
            node.add_child(sibling);
        }
        if let Some(mut node) = runtime.nodes.get_mut(leaf)
            && let SceneNodeData::MeshInstance3D(data) = &mut node.data
        {
            data.modulate.modulate = Color::new(1.0, 1.0, 0.5, 1.0);
            data.modulate.self_modulate = Color::new(0.5, 1.0, 1.0, 1.0);
            data.modulate.children_modulate = Color::RED;
            node.parent = mid;
        }
        if let Some(mut node) = runtime.nodes.get_mut(sibling)
            && let SceneNodeData::MeshInstance3D(data) = &mut node.data
        {
            data.modulate.self_modulate = Color::new(1.0, 0.5, 1.0, 1.0);
            node.parent = mid;
        }

        assert_eq!(
            runtime.effective_self_modulate(root),
            Runtime::color_modulate(
                Color::new(0.8, 1.0, 1.0, 1.0),
                Color::new(1.0, 0.1, 0.1, 1.0)
            )
        );
        let inherited_to_mid = Runtime::color_modulate(
            Runtime::color_modulate(
                Color::new(0.8, 1.0, 1.0, 1.0),
                Color::new(1.0, 0.7, 1.0, 1.0),
            ),
            Color::new(1.0, 0.9, 1.0, 1.0),
        );
        assert_eq!(
            runtime.effective_self_modulate(mid),
            Runtime::color_modulate(inherited_to_mid, Color::new(0.1, 1.0, 0.1, 1.0))
        );
        let inherited_to_leaf =
            Runtime::color_modulate(inherited_to_mid, Color::new(1.0, 1.0, 0.6, 1.0));
        assert_eq!(
            runtime.effective_self_modulate(leaf),
            Runtime::color_modulate(
                Runtime::color_modulate(inherited_to_leaf, Color::new(1.0, 1.0, 0.5, 1.0)),
                Color::new(0.5, 1.0, 1.0, 1.0)
            )
        );
        assert_eq!(
            runtime.effective_self_modulate(sibling),
            Runtime::color_modulate(inherited_to_leaf, Color::new(1.0, 0.5, 1.0, 1.0))
        );
    }

    #[test]
    fn mesh_instance_redraws_when_script_assigned_pending_mesh_load_finishes() {
        let mut runtime = Runtime::new();
        let pending_mesh = MeshAPI::load_mesh(
            runtime.resource_api.as_ref(),
            "res://avatars/face/noses.glb:mesh[3]",
        );
        let mesh_request =
            collect_commands(&mut runtime)
                .into_iter()
                .find_map(|command| match command {
                    RenderCommand::Resource(ResourceCommand::CreateMesh {
                        request, id, ..
                    }) if id == pending_mesh => Some(request),
                    _ => None,
                })
                .expect("expected mesh create command");

        let node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
                MeshInstance3D::new(),
            )));
        NodeAPI::with_node_mut::<MeshInstance3D, _, _>(&mut runtime, node, |mesh| {
            mesh.mesh = pending_mesh;
        });

        runtime.extract_render_3d_commands();
        assert!(collect_commands(&mut runtime).is_empty());

        runtime.apply_render_event(RenderEvent::MeshCreated {
            request: mesh_request,
            id: pending_mesh,
            mesh: Some(Mesh3D {
                vertices: Vec::new(),
                indices: Vec::new(),
                surface_ranges: Vec::new(),
                blend_shapes: Vec::new(),
            }),
        });
        runtime.extract_render_3d_commands();
        let commands = collect_commands(&mut runtime);

        assert!(commands.iter().any(|command| matches!(
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
    fn mesh_instance_ready_waits_for_mesh_and_material_backend_ack() {
        let mut runtime = Runtime::new();
        let node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
                MeshInstance3D::new(),
            )));
        runtime
            .render_3d
            .mesh_sources
            .insert(node, "__cube__".to_string());

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        let mesh_request = match first.first() {
            Some(RenderCommand::Resource(ResourceCommand::CreateMesh { request, .. })) => *request,
            _ => panic!("expected mesh create request"),
        };
        assert!(!NodeAPI::is_mesh_instance_ready(&mut runtime, node));

        let mesh = MeshID::from_parts(99, 0);
        runtime.apply_render_event(RenderEvent::MeshCreated {
            request: mesh_request,
            id: mesh,
            mesh: Some(Mesh3D {
                vertices: Vec::new(),
                indices: Vec::new(),
                surface_ranges: Vec::new(),
                blend_shapes: Vec::new(),
            }),
        });
        runtime.extract_render_3d_commands();
        let second = collect_commands(&mut runtime);
        let (material_request, material) = second
            .iter()
            .find_map(|command| match command {
                RenderCommand::Resource(ResourceCommand::CreateMaterial {
                    request, id, ..
                }) => Some((*request, *id)),
                _ => None,
            })
            .expect("expected material create command");
        assert!(!NodeAPI::is_mesh_instance_ready(&mut runtime, node));

        runtime.apply_render_event(RenderEvent::MaterialCreated {
            request: material_request,
            id: material,
        });
        assert!(NodeAPI::is_mesh_instance_ready(&mut runtime, node));
    }

    #[test]
    fn mesh_instance_ready_ignores_default_nil_mesh_and_material() {
        let mut runtime = Runtime::new();
        let node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
                MeshInstance3D::new(),
            )));

        assert!(NodeAPI::is_mesh_instance_ready(&mut runtime, node));
    }

    #[test]
    fn mesh_instance_ready_ignores_nil_surface_material() {
        let mut runtime = Runtime::new();
        let mesh_id = MeshAPI::create_mesh_data(
            runtime.resource_api.as_ref(),
            Mesh3D {
                vertices: Vec::new(),
                indices: Vec::new(),
                surface_ranges: Vec::new(),
                blend_shapes: Vec::new(),
            },
        );
        let request = collect_commands(&mut runtime)
            .into_iter()
            .find_map(|command| match command {
                RenderCommand::Resource(ResourceCommand::CreateRuntimeMesh { request, .. }) => {
                    Some(request)
                }
                _ => None,
            })
            .expect("expected runtime mesh create command");
        runtime.apply_render_event(RenderEvent::MeshCreated {
            request,
            id: mesh_id,
            mesh: None,
        });
        let mut mesh = MeshInstance3D::new();
        mesh.mesh = mesh_id;
        set_primary_material(&mut mesh, MaterialID::nil());
        let node = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(mesh)));

        assert!(NodeAPI::is_mesh_instance_ready(&mut runtime, node));
    }

    #[test]
    fn mesh_instance_can_request_mesh_and_material_in_separate_frames() {
        let mut runtime = Runtime::new();
        let inserted = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::MeshInstance3D(
                MeshInstance3D::new(),
            )));
        runtime
            .render_3d
            .mesh_sources
            .insert(inserted, "__cube__".to_string());

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        let mesh_request = match first.first() {
            Some(RenderCommand::Resource(ResourceCommand::CreateMesh { request, .. })) => *request,
            _ => panic!("expected mesh create request"),
        };

        runtime.apply_render_event(RenderEvent::MeshCreated {
            request: mesh_request,
            id: MeshID::from_parts(10, 0),
            mesh: None,
        });
        runtime.extract_render_3d_commands();
        let second = collect_commands(&mut runtime);
        assert!(second.iter().any(|command| matches!(
            command,
            RenderCommand::Resource(ResourceCommand::CreateMaterial { .. })
        )));
        assert!(second.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command)
                if matches!(command.as_ref(), Command3D::Draw { node, .. } if *node == inserted)
        )));
    }

    #[test]
    fn mesh_instances_share_default_material() {
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

        runtime.extract_render_3d_commands();
        let first = collect_commands(&mut runtime);
        for (request, mesh) in first
            .iter()
            .filter_map(|command| match command {
                RenderCommand::Resource(ResourceCommand::CreateMesh { request, .. }) => {
                    Some(*request)
                }
                _ => None,
            })
            .zip([MeshID::from_parts(20, 0), MeshID::from_parts(21, 0)])
        {
            runtime.apply_render_event(RenderEvent::MeshCreated {
                request,
                id: mesh,
                mesh: None,
            });
        }

        runtime.extract_render_3d_commands();
        let second = collect_commands(&mut runtime);
        let default_materials: Vec<MaterialID> =
            second
                .iter()
                .filter_map(|command| match command {
                    RenderCommand::Resource(ResourceCommand::CreateMaterial {
                        id, source, ..
                    }) if source.is_none() => Some(*id),
                    _ => None,
                })
                .collect();
        assert_eq!(default_materials.len(), 1);
        let default_material = default_materials[0];
        let draws_using_default = second
            .iter()
            .filter(|command| {
                matches!(
                    command,
                    RenderCommand::ThreeD(command)
                        if matches!(
                            command.as_ref(),
                            Command3D::Draw { surfaces, .. }
                                if surfaces
                                    .first()
                                    .and_then(|surface| surface.material)
                                    == Some(default_material)
                        )
                )
            })
            .count();
        assert_eq!(draws_using_default, 2);
    }
}
