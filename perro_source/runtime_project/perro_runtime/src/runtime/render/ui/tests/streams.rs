mod streams {
    use super::*;

    #[test]
    fn sub_view_2d_renders_local_3d_children() {
        let mut runtime = Runtime::new();
        let view = NodeAPI::create::<SubView2D>(&mut runtime);
        let mesh = NodeAPI::create::<MeshInstance3D>(&mut runtime);
        assert!(runtime.reparent(view, mesh));

        if let Some(mut node) = runtime.nodes.get_mut(view)
            && let SceneNodeData::SubView2D(view) = &mut node.data
        {
            view.transform = Transform2D::new(Vector2::new(40.0, 50.0), 0.0, Vector2::ONE);
            view.size = Vector2::new(32.0, 18.0);
        }
        if let Some(mut node) = runtime.nodes.get_mut(mesh)
            && let SceneNodeData::MeshInstance3D(mesh) = &mut node.data
        {
            mesh.transform.position = Vector3::new(2.0, 3.0, 4.0);
            mesh.mesh = perro_ids::MeshID::from_parts(31, 0);
            mesh.set_surface_material(0, Some(perro_ids::MaterialID::from_parts(32, 0)));
        }

        runtime.extract_render_2d_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        let state = commands.iter().find_map(|command| match command {
            RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                if *node == view => Some(state.as_ref()),
            _ => None,
        }).expect("SubView2D stream state");
        assert!(!state.tone_map_output);
        let model = state.draws_3d.iter().find_map(|draw| match draw {
            perro_render_bridge::CameraStreamDraw3DState::Draw { node, model, .. }
                if *node == mesh => Some(model),
            _ => None,
        }).expect("local 3D mesh draw");
        assert_eq!(model[3][0..3], [2.0, 3.0, 4.0]);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::TwoD(Command2D::UpsertCameraStream { node, .. }) if *node == view
        )));
    }

    #[test]
    fn sub_view_3d_renders_local_2d_children() {
        let mut runtime = Runtime::new();
        let view = NodeAPI::create::<SubView3D>(&mut runtime);
        let particles = NodeAPI::create::<ParticleEmitter2D>(&mut runtime);
        assert!(runtime.reparent(view, particles));

        if let Some(mut node) = runtime.nodes.get_mut(particles)
            && let SceneNodeData::ParticleEmitter2D(particles) = &mut node.data
        {
            particles.transform.position = Vector2::new(7.0, 9.0);
        }

        runtime.extract_render_3d_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        let state = commands.iter().find_map(|command| match command {
            RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                if *node == view => Some(state.as_ref()),
            _ => None,
        }).expect("SubView3D stream state");
        assert!(!state.tone_map_output);
        let (_, particles_state) = state
            .point_particles_2d
            .iter()
            .find(|(node, _)| *node == particles)
            .expect("local 2D particle draw");
        assert_eq!(particles_state.model[2][0..2], [7.0, 9.0]);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command)
                if matches!(command.as_ref(), Command3D::UpsertCameraStream { node, .. } if *node == view)
        )));
    }

    #[test]
    fn ui_sub_view_uses_active_local_cameras_for_each_lane() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let view = NodeAPI::create::<UiSubView>(&mut runtime);
        let camera_2d = NodeAPI::create::<Camera2D>(&mut runtime);
        let camera_3d = NodeAPI::create::<Camera3D>(&mut runtime);
        assert!(runtime.reparent(view, camera_2d));
        assert!(runtime.reparent(view, camera_3d));

        if let Some(mut node) = runtime.nodes.get_mut(view)
            && let SceneNodeData::UiSubView(view) = &mut node.data
        {
            view.layout.size = UiVector2::pixels(320.0, 180.0);
            view.post_processing
                .add_unnamed(PostProcessEffect::Saturate { amount: 0.75 });
        }
        if let Some(mut node) = runtime.nodes.get_mut(camera_2d)
            && let SceneNodeData::Camera2D(camera) = &mut node.data
        {
            camera.active = true;
            camera.transform =
                Transform2D::new(Vector2::new(12.0, 13.0), 0.25, Vector2::ONE);
            camera.zoom = 2.0;
            camera.render_mask = BitMask::with([2]);
            camera.audio_options.audio_mask = BitMask::with([3]);
            camera
                .post_processing
                .add_unnamed(PostProcessEffect::Blur { strength: 0.5 });
        }
        if let Some(mut node) = runtime.nodes.get_mut(camera_3d)
            && let SceneNodeData::Camera3D(camera) = &mut node.data
        {
            camera.active = true;
            camera.transform = Transform3D::new(
                Vector3::new(1.0, 2.0, 3.0),
                Quaternion::IDENTITY,
                Vector3::ONE,
            );
            camera.projection = CameraProjection::orthographic(7.0, 0.1, 50.0);
            camera.render_mask = BitMask::with([4]);
            camera.audio_options.audio_mask = BitMask::with([5]);
            camera
                .post_processing
                .add_unnamed(PostProcessEffect::Pixelate { size: 3.0 });
        }

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        let state = commands
            .iter()
            .find_map(|command| match command {
                RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                    if *node == view =>
                {
                    Some(state.as_ref())
                }
                _ => None,
            })
            .expect("UiSubView stream state");

        let CameraStreamSourceState::ThreeD(camera_3d_state) = &state.source else {
            panic!("3D SubView source");
        };
        assert_eq!(camera_3d_state.position, [1.0, 2.0, 3.0]);
        assert_eq!(camera_3d_state.render_mask, BitMask::with([4]));
        assert_eq!(
            camera_3d_state.audio_options.audio_mask,
            BitMask::with([5])
        );
        assert!(matches!(
            camera_3d_state.projection,
            perro_render_bridge::CameraProjectionState::Orthographic { size: 7.0, .. }
        ));

        let camera_2d_state = state
            .overlay_camera_2d
            .as_ref()
            .expect("2D SubView overlay camera");
        assert_eq!(camera_2d_state.position, [12.0, 13.0]);
        assert_eq!(camera_2d_state.rotation_radians, 0.25);
        assert_eq!(camera_2d_state.zoom, 2.0);
        assert_eq!(camera_2d_state.render_mask, BitMask::with([2]));
        assert_eq!(
            camera_2d_state.audio_options.audio_mask,
            BitMask::with([3])
        );
        assert!(matches!(
            state.post_processing.as_ref(),
            [
                PostProcessEffect::Pixelate { size: 3.0 },
                PostProcessEffect::Blur { strength: 0.5 },
                PostProcessEffect::Saturate { amount: 0.75 }
            ]
        ));
    }

    #[test]
    fn sub_view_active_camera_uses_local_space_and_nearest_owner() {
        let mut runtime = Runtime::new();
        let view = NodeAPI::create::<SubView3D>(&mut runtime);
        let other_camera = NodeAPI::create::<Camera3D>(&mut runtime);
        let camera = NodeAPI::create::<Camera3D>(&mut runtime);
        let nested_view = NodeAPI::create::<SubView3D>(&mut runtime);
        let nested_camera = NodeAPI::create::<Camera3D>(&mut runtime);
        assert!(runtime.reparent(view, other_camera));
        assert!(runtime.reparent(view, camera));
        assert!(runtime.reparent(view, nested_view));
        assert!(runtime.reparent(nested_view, nested_camera));

        if let Some(mut node) = runtime.nodes.get_mut(view)
            && let SceneNodeData::SubView3D(view) = &mut node.data
        {
            view.transform.position = Vector3::new(40.0, 50.0, 60.0);
        }
        if let Some(mut node) = runtime.nodes.get_mut(camera)
            && let SceneNodeData::Camera3D(camera) = &mut node.data
        {
            camera.active = true;
            camera.transform.position = Vector3::new(2.0, 3.0, 4.0);
        }
        if let Some(mut node) = runtime.nodes.get_mut(other_camera)
            && let SceneNodeData::Camera3D(camera) = &mut node.data
        {
            camera.active = true;
            camera.transform.position = Vector3::new(6.0, 7.0, 8.0);
        }
        if let Some(mut node) = runtime.nodes.get_mut(nested_camera)
            && let SceneNodeData::Camera3D(camera) = &mut node.data
        {
            camera.active = true;
            camera.transform.position = Vector3::new(9.0, 9.0, 9.0);
        }
        runtime.note_camera_3d_activated(other_camera);
        runtime.note_camera_3d_activated(camera);

        runtime.extract_render_3d_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        let state = commands
            .iter()
            .find_map(|command| match command {
                RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                    if *node == view =>
                {
                    Some(state.as_ref())
                }
                _ => None,
            })
            .expect("outer SubView stream state");
        assert!(matches!(
            &state.source,
            CameraStreamSourceState::ThreeD(camera) if camera.position == [2.0, 3.0, 4.0]
        ));

        if let Some(mut node) = runtime.nodes.get_mut(camera)
            && let SceneNodeData::Camera3D(camera) = &mut node.data
        {
            camera.visible = false;
        }
        runtime.mark_needs_rerender(camera);
        runtime.extract_render_3d_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                if *node == view
                    && matches!(
                        &state.source,
                        CameraStreamSourceState::ThreeD(camera)
                            if camera.position == [6.0, 7.0, 8.0]
                    )
        )));
    }

    #[test]
    fn webcam_node_does_not_open_without_stream_or_api_use() {
        let mut runtime = Runtime::new();
        let _webcam = NodeAPI::create::<Webcam>(&mut runtime);

        runtime.extract_render_snapshot_commands(&mut Vec::new());
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(!has_external_texture_create(&commands));
    }

    #[test]
    fn ui_camera_stream_opens_referenced_webcam_node() {
        let mut runtime = Runtime::new();
        let webcam = NodeAPI::create::<Webcam>(&mut runtime);
        let stream = NodeAPI::create::<UiCameraStream>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(stream)
            && let SceneNodeData::UiCameraStream(data) = &mut node.data
        {
            data.stream.camera = webcam;
        }

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(has_external_texture_create(&commands));
        let stream_texture = Runtime::camera_stream_texture_id(stream);
        assert!(commands.iter().any(|command| {
            matches!(
                command,
                RenderCommand::Ui(UiCommand::UpsertImage { texture, .. })
                    if !texture.is_nil() && *texture != stream_texture
            )
        }));
    }

    #[test]
    fn webcam_chroma_key_uses_processed_camera_stream_target() {
        let mut runtime = Runtime::new();
        let webcam = NodeAPI::create::<Webcam>(&mut runtime);
        let stream = NodeAPI::create::<UiCameraStream>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(stream)
            && let SceneNodeData::UiCameraStream(data) = &mut node.data
        {
            data.stream.camera = webcam;
            data.stream
                .post_processing
                .add_unnamed(perro_structs::PostProcessEffect::ChromaKey {
                    color: Color::GREEN,
                    tolerance: 0.1,
                    softness: 0.05,
                });
        }

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        let output_texture = Runtime::camera_stream_texture_id(stream);

        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                if *node == stream
                    && state.output_texture == output_texture
                    && matches!(state.post_processing.as_ref(), [perro_structs::PostProcessEffect::ChromaKey { .. }])
        )));
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::UpsertImage { node, texture, .. })
                if *node == stream && *texture == output_texture
        )));
    }

    #[test]
    fn ui_camera_stream_uses_native_webcam_frame_aspect() {
        let mut runtime = Runtime::new();
        let webcam = NodeAPI::create::<Webcam>(&mut runtime);
        let stream = NodeAPI::create::<UiCameraStream>(&mut runtime);
        let config = if let Some(node) = runtime.nodes.get(webcam)
            && let SceneNodeData::Webcam(data) = &node.data
        {
            data.config.clone()
        } else {
            panic!("expected webcam node");
        };
        let webcam_id = runtime.resource_api.ensure_webcam_node_slot(webcam, config);
        assert!(runtime.resource_api.queue_webcam_frame(
            webcam_id,
            WebcamFrame {
                width: 4,
                height: 2,
                rgba: vec![255; 4 * 2 * 4],
            },
        ));
        if let Some(mut node) = runtime.nodes.get_mut(stream)
            && let SceneNodeData::UiCameraStream(data) = &mut node.data
        {
            data.stream.camera = webcam;
            data.stream.aspect_ratio = 0.0;
        }

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::UpsertImage { node, aspect_ratio, .. })
                if *node == stream && *aspect_ratio == 2.0
        )));
    }

    #[test]
    fn camera_stream_3d_opens_referenced_webcam_node() {
        let mut runtime = Runtime::new();
        let camera = NodeAPI::create::<Camera3D>(&mut runtime);
        let webcam = NodeAPI::create::<Webcam>(&mut runtime);
        let stream = NodeAPI::create::<CameraStream3D>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(camera)
            && let SceneNodeData::Camera3D(data) = &mut node.data
        {
            data.active = true;
        }
        if let Some(mut node) = runtime.nodes.get_mut(stream)
            && let SceneNodeData::CameraStream3D(data) = &mut node.data
        {
            data.stream.camera = webcam;
        }

        let mut commands = Vec::new();
        runtime.extract_render_snapshot_commands(&mut commands);

        assert!(has_external_texture_create(&commands));
    }

    #[test]
    fn webcam_api_default_opens_without_node() {
        let mut runtime = Runtime::new();
        let id = WebcamAPI::webcam_default(runtime.resource_api.as_ref()).expect("webcam id");

        assert!(WebcamAPI::webcam_is_open(runtime.resource_api.as_ref(), id));

        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(has_external_texture_create(&commands));
    }

    #[test]
    fn ui_camera_stream_refreshes_when_source_camera_moves() {
        let mut runtime = Runtime::new();
        let camera = NodeAPI::create::<Camera3D>(&mut runtime);
        let stream = NodeAPI::create::<UiCameraStream>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(stream)
            && let SceneNodeData::UiCameraStream(data) = &mut node.data
        {
            data.stream.camera = camera;
            data.stream.resolution = [320, 180].into();
        }

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());

        if let Some(mut node) = runtime.nodes.get_mut(camera)
            && let SceneNodeData::Camera3D(data) = &mut node.data
        {
            data.transform = Transform3D::new(
                Vector3::new(4.0, 5.0, 6.0),
                Quaternion::IDENTITY,
                Vector3::ONE,
            );
        }
        runtime.mark_transform_dirty_recursive(camera);
        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                if *node == stream
                    && matches!(&state.source, CameraStreamSourceState::ThreeD(camera) if camera.position == [4.0, 5.0, 6.0])
        )));
    }

    #[test]
    fn ui_camera_stream_3d_captures_sky_from_source_camera() {
        let mut runtime = Runtime::new();
        let camera = NodeAPI::create::<Camera3D>(&mut runtime);
        let _sky = NodeAPI::create::<Sky3D>(&mut runtime);
        let stream = NodeAPI::create::<UiCameraStream>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(stream)
            && let SceneNodeData::UiCameraStream(data) = &mut node.data
        {
            data.stream.camera = camera;
            data.stream.resolution = [320, 180].into();
        }

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|command| matches!(
            command,
                RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                if *node == stream
                    && matches!(state.source, CameraStreamSourceState::ThreeD(_))
                    && state.transparent_background
                    && state.lighting_3d.sky.is_some()
        )));
    }

    #[test]
    fn ui_camera_stream_emits_image_corner_radius() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let camera = NodeAPI::create::<Camera3D>(&mut runtime);
        let stream = NodeAPI::create::<UiCameraStream>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(stream)
            && let SceneNodeData::UiCameraStream(data) = &mut node.data
        {
            data.layout.size = UiVector2::pixels(320.0, 180.0);
            data.stream.camera = camera;
            data.stream.resolution = [320, 180].into();
            data.corner_radius = 0.25;
        }

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::UpsertImage { node, corner_radii, .. })
                if *node == stream && corner_radii.tl == 0.25 && corner_radii.tr == 0.25
        )));
    }

    #[test]
    fn ui_viewport_renders_only_local_spatial_descendants() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let viewport = NodeAPI::create::<UiSubView>(&mut runtime);
        let local_light = NodeAPI::create::<AmbientLight3D>(&mut runtime);
        let local_mesh = NodeAPI::create::<MeshInstance3D>(&mut runtime);
        let world_light = NodeAPI::create::<AmbientLight3D>(&mut runtime);
        let local_light_2d = NodeAPI::create::<AmbientLight2D>(&mut runtime);
        let _world_light_2d = NodeAPI::create::<AmbientLight2D>(&mut runtime);
        assert!(runtime.reparent(viewport, local_light));
        assert!(runtime.reparent(viewport, local_mesh));
        assert!(runtime.reparent(viewport, local_light_2d));
        if let Some(mut node) = runtime.nodes.get_mut(local_mesh)
            && let SceneNodeData::MeshInstance3D(data) = &mut node.data
        {
            data.mesh = perro_ids::MeshID::from_parts(7, 0);
            data.set_surface_material(0, Some(perro_ids::MaterialID::from_parts(9, 0)));
        }
        if let Some(mut node) = runtime.nodes.get_mut(local_light)
            && let SceneNodeData::AmbientLight3D(data) = &mut node.data
        {
            data.intensity = 2.0;
        }
        if let Some(mut node) = runtime.nodes.get_mut(world_light)
            && let SceneNodeData::AmbientLight3D(data) = &mut node.data
        {
            data.intensity = 4.0;
        }
        if let Some(mut node) = runtime.nodes.get_mut(local_light_2d)
            && let SceneNodeData::AmbientLight2D(data) = &mut node.data
        {
            data.intensity = 3.0;
        }
        if let Some(mut node) = runtime.nodes.get_mut(viewport)
            && let SceneNodeData::UiSubView(data) = &mut node.data
        {
            data.layout.size = UiVector2::pixels(320.0, 180.0);
        }
        // Exercise the packed path used after scene-load cache rebuilds.
        runtime.nodes.rebuild_packed_children();

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        let state = commands.iter().find_map(|command| match command {
            RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                if *node == viewport =>
            {
                Some(state.as_ref())
            }
            _ => None,
        });
        let state = state.expect("viewport stream state");
        assert!(state.tone_map_output);
        assert_eq!(state.resolution, [640, 360]);
        assert!(state.transparent_background);
        assert!(matches!(
            &state.source,
            CameraStreamSourceState::ThreeD(camera) if camera.position == [0.0, 0.0, 5.0]
        ));
        assert_eq!(
            state
                .lighting_3d
                .ambient_light
                .expect("local ambient light")
                .intensity,
            2.0
        );
        assert_eq!(state.lights_2d.len(), 1);
        assert!(matches!(
            state.lights_2d[0],
            Light2DState::Ambient(light) if light.intensity == 3.0
        ));
        assert!(state.overlay_camera_2d.is_some());
        assert!(state.draws_3d.iter().any(|draw| matches!(
            draw,
            perro_render_bridge::CameraStreamDraw3DState::Draw { node, .. }
                if *node == local_mesh
        )));
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::UpsertImage { node, .. }) if *node == viewport
        )));

        runtime.extract_render_3d_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);
        assert!(!commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command)
                if matches!(command.as_ref(), Command3D::SetAmbientLight { node, .. } if *node == local_light)
        )));
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::ThreeD(command)
                if matches!(command.as_ref(), Command3D::SetAmbientLight { node, .. } if *node == world_light)
        )));

        if let Some(mut node) = runtime.nodes.get_mut(viewport)
            && let SceneNodeData::UiSubView(data) = &mut node.data
        {
            data.visible = false;
        }
        assert!(runtime.is_suspended_by_sub_view(local_light));
        if let Some(mut node) = runtime.nodes.get_mut(viewport)
            && let SceneNodeData::UiSubView(data) = &mut node.data
        {
            data.suspend_when_hidden = false;
        }
        assert!(!runtime.is_suspended_by_sub_view(local_light));
    }

    #[test]
    fn ui_viewport_rebuilds_draws_after_mesh_resource_event() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let viewport = NodeAPI::create::<UiSubView>(&mut runtime);
        let local_mesh = NodeAPI::create::<MeshInstance3D>(&mut runtime);
        assert!(runtime.reparent(viewport, local_mesh));

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        let first = commands.iter().find_map(|command| match command {
            RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                if *node == viewport =>
            {
                Some(state.as_ref())
            }
            _ => None,
        });
        assert!(first.is_some_and(|state| state.draws_3d.is_empty()));

        runtime.clear_dirty_flags();
        let mesh = perro_ids::MeshID::from_parts(17, 0);
        if let Some(node) = runtime.nodes.get_mut_untracked_non_physics(local_mesh)
            && let SceneNodeData::MeshInstance3D(data) = &mut node.data
        {
            data.mesh = mesh;
            data.set_surface_material(0, Some(perro_ids::MaterialID::from_parts(19, 0)));
        }
        runtime.clear_dirty_flags();
        runtime.apply_render_event(RenderEvent::MeshCreated {
            request: perro_render_bridge::RenderRequestID::new(71),
            id: mesh,
            mesh: None,
        });

        runtime.extract_render_ui_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                if *node == viewport
                    && state.draws_3d.iter().any(|draw| matches!(
                        draw,
                        perro_render_bridge::CameraStreamDraw3DState::Draw { node, .. }
                            if *node == local_mesh
                    ))
        )));
    }

    #[test]
    fn ui_viewport_tracks_scaled_parent_rect() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut parent = UiPanel::new();
        parent.layout.size = UiVector2::pixels(400.0, 200.0);
        parent.transform.scale = Vector2::new(0.5, 0.5);
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(parent)));

        let viewport = NodeAPI::create::<UiSubView>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(viewport)
            && let SceneNodeData::UiSubView(data) = &mut node.data
        {
            data.layout.size = UiVector2::ratio(1.0, 1.0);
        }
        assert!(runtime.reparent(parent, viewport));

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                if *node == viewport && state.resolution == [400, 200]
        )));
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::UpsertImage { node, rect, aspect_ratio, .. })
                if *node == viewport && rect.size == [200.0, 100.0] && *aspect_ratio == 2.0
        )));
    }

    #[test]
    fn ui_viewport_opaque_background_keeps_local_sky_path() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let viewport = NodeAPI::create::<UiSubView>(&mut runtime);
        if let Some(mut node) = runtime.nodes.get_mut(viewport)
            && let SceneNodeData::UiSubView(data) = &mut node.data
        {
            data.layout.size = UiVector2::pixels(100.0, 50.0);
            data.background = Color::BLACK;
        }

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                if *node == viewport
                    && !state.transparent_background
                    && state.resolution == [200, 100]
        )));
    }

    #[test]
    fn unchanged_ui_skips_redundant_upsert() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let node = insert_panel(&mut runtime, [120.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert_eq!(commands.iter().filter(|cmd| matches!(cmd, RenderCommand::Ui(UiCommand::UpsertPanel { node: n, .. }) if *n == node)).count(), 1);

        runtime.clear_dirty_flags();
        runtime.extract_render_ui_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.is_empty());
    }

    #[test]
    fn ui_animated_image_emits_current_frame_region() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut image = UiAnimatedImage::new();
        image.texture = TextureID::from_parts(42, 0);
        image.layout.size = UiVector2::pixels(64.0, 64.0);
        image.current_frame = 1;
        image.animations.push(UiAnimatedImageFrameSet {
            name: Cow::Borrowed("default"),
            start: [0.0, 0.0],
            frame_size: [16.0, 16.0],
            frame_count: 4,
            columns: 2,
            fps: 12.0,
        });
        let node = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiAnimatedImage(Box::new(image)),
        );

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertImage { node: n, uv_min, uv_max, .. })
                if *n == node && *uv_min == [16.0, 0.0] && *uv_max == [32.0, 16.0]
        )));
    }

    #[test]
    fn ui_image_button_emits_image_command_with_state_tint() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut button = perro_ui::UiImageButton::new();
        button.texture = TextureID::from_parts(43, 0);
        button.layout.size = UiVector2::pixels(64.0, 64.0);
        button.tint = Color::new(0.1, 0.2, 0.3, 1.0);
        button.hover_tint = Color::new(0.4, 0.5, 0.6, 1.0);
        button.pressed_tint = Color::new(0.7, 0.8, 0.9, 1.0);
        button.scale_mode = perro_ui::UiImageScaleMode::Fit;
        let node = insert_ui_node(&mut runtime, SceneNodeData::UiImageButton(Box::new(button)));

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.begin_input_frame();
        runtime.set_mouse_position(400.0, 300.0);
        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertImage { node: n, tint, scale_mode, .. })
                if *n == node
                    && *tint == Color::new(0.4, 0.5, 0.6, 1.0)
                    && *scale_mode == UiImageScaleState::Fit
        )));
    }

    #[test]
    fn ui_image_uses_inherited_ui_modulate() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut parent = UiPanel::new();
        parent.layout.size = UiVector2::pixels(120.0, 80.0);
        parent.modulate.children_modulate = Color::new(1.0, 0.5, 1.0, 1.0);
        parent.modulate.self_modulate = Color::RED;
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(parent)));

        let mut image = perro_ui::UiImage::new();
        image.texture = TextureID::from_parts(44, 0);
        image.layout.size = UiVector2::pixels(32.0, 32.0);
        image.tint = Color::new(0.5, 1.0, 1.0, 1.0);
        let child = insert_ui_node(&mut runtime, SceneNodeData::UiImage(Box::new(image)));
        attach_child(&mut runtime, parent, child);

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        let expected = Runtime::color_modulate(
            Color::new(1.0, 0.5, 1.0, 1.0),
            Color::new(0.5, 1.0, 1.0, 1.0),
        );
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertImage { node: n, tint, .. })
                if *n == child && *tint == expected
        )));
    }

    #[test]
    fn ui_nine_slice_emits_nine_slice_command() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut node_data = perro_ui::UiNineSlice::new();
        node_data.texture = TextureID::from_parts(64, 0);
        node_data.layout.size = UiVector2::pixels(120.0, 40.0);
        node_data.texture_region = Some([1.0, 2.0, 30.0, 20.0]);
        node_data.margins = [5.0, 6.0, 7.0, 8.0];
        let node = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiNineSlice(Box::new(node_data)),
        );

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertNineSlice {
                node: n,
                texture,
                rect,
                uv_min,
                uv_max,
                margins,
                ..
            }) if *n == node
                && *texture == TextureID::from_parts(64, 0)
                && rect.size == [120.0, 40.0]
                && *uv_min == [1.0, 2.0]
                && *uv_max == [31.0, 22.0]
                && *margins == [5.0, 6.0, 7.0, 8.0]
        )));
    }

    #[test]
    fn ui_image_keeps_retained_texture_while_replacement_texture_is_pending() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let old_texture = TextureID::from_parts(61, 0);
        let mut image = perro_ui::UiImage::new();
        image.texture = old_texture;
        image.layout.size = UiVector2::pixels(64.0, 64.0);
        let node = insert_ui_node(&mut runtime, SceneNodeData::UiImage(Box::new(image)));

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertImage { node: n, texture, .. })
                if *n == node && *texture == old_texture
        )));

        let pending_texture = runtime
            .resource_api
            .load_texture("res://textures/ui_tool_version_b.png");
        let pending_request = collect_resource_texture_request(&mut runtime, pending_texture);
        if let Some(mut scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::UiImage(image) = &mut scene_node.data
        {
            image.texture = pending_texture;
        }
        runtime.mark_ui_dirty(node, Runtime::UI_DIRTY_COMMANDS);

        runtime.extract_render_ui_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);
        assert!(!commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::RemoveNode { node: n }) if *n == node
        )));
        assert!(!commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertImage { node: n, .. }) if *n == node
        )));
        assert!(
            runtime
                .render_ui
                .retained_commands
                .get(&node)
                .is_some_and(|cmd| {
                    matches!(cmd, UiCommand::UpsertImage { texture, .. } if *texture == old_texture)
                })
        );

        runtime.apply_render_event(RenderEvent::TextureCreated {
            request: pending_request,
            id: pending_texture,
        });
        runtime.mark_ui_dirty(node, Runtime::UI_DIRTY_COMMANDS);
        runtime.extract_render_ui_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertImage { node: n, texture, .. })
                if *n == node && *texture == pending_texture
        )));
    }

    #[test]
    fn viewport_resize_recomputes_percent_ui_rects() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut panel = UiPanel::new();
        panel.layout.anchor = UiAnchor::TopRight;
        panel.layout.size = UiVector2::ratio(0.5, 0.25);
        panel.style.fill = Color::new(0.1, 0.2, 0.3, 1.0);
        let node = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(panel)));

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.set_viewport_size(1200, 900);
        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertPanel { node: n, rect, .. })
                if *n == node
                    && rect.size == [600.0, 225.0]
                    && rect.center == [300.0, 337.5]
        )));
    }

    #[test]
    fn ui_panel_without_position_field_centers_in_parent() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let node = insert_panel(&mut runtime, [100.0, 50.0], Color::new(0.1, 0.2, 0.3, 1.0));

        runtime.extract_render_ui_commands();

        let rect = runtime
            .render_ui
            .computed_rects
            .get(&node)
            .copied()
            .expect("computed rect");
        assert_eq!(rect.center, Vector2::ZERO);
        assert_eq!(rect.size, Vector2::new(100.0, 50.0));
    }

    #[test]
    fn ui_bottom_anchor_places_rect_on_bottom_edge_without_position() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let node = insert_panel(&mut runtime, [100.0, 50.0], Color::new(0.1, 0.2, 0.3, 1.0));
        if let Some(mut scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.layout.anchor = UiAnchor::Bottom;
        }

        runtime.extract_render_ui_commands();

        let rect = runtime
            .render_ui
            .computed_rects
            .get(&node)
            .copied()
            .expect("computed rect");
        assert_eq!(rect.center, Vector2::new(0.0, -275.0));
        assert_eq!(rect.min().y, -300.0);
    }

    #[test]
    fn ui_translation_ratio_moves_after_anchor_by_parent_size() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let node = insert_panel(&mut runtime, [100.0, 80.0], Color::new(0.1, 0.2, 0.3, 1.0));
        if let Some(mut scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.transform.translation = Vector2::new(0.25, -0.5);
        }

        runtime.extract_render_ui_commands();

        let rect = runtime
            .render_ui
            .computed_rects
            .get(&node)
            .copied()
            .expect("computed rect");
        assert_eq!(rect.center, Vector2::new(200.0, -300.0));
    }

    #[test]
    fn ui_self_translation_ratio_moves_after_anchor_by_own_size() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let node = insert_panel(&mut runtime, [100.0, 80.0], Color::new(0.1, 0.2, 0.3, 1.0));
        if let Some(mut scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.transform.self_translation = Vector2::new(0.25, -0.5);
        }

        runtime.extract_render_ui_commands();

        let rect = runtime
            .render_ui
            .computed_rects
            .get(&node)
            .copied()
            .expect("computed rect");
        assert_eq!(rect.center, Vector2::new(25.0, -40.0));
    }

    #[test]
    fn ui_bottom_anchor_keeps_edge_placed_while_pivot_moves_origin() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let node = insert_panel(&mut runtime, [100.0, 100.0], Color::new(0.1, 0.2, 0.3, 1.0));
        if let Some(mut scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.layout.anchor = UiAnchor::Bottom;
            panel.transform.pivot = UiVector2::ratio(0.5, 1.0);
        }

        runtime.extract_render_ui_commands();

        let rect = runtime
            .render_ui
            .computed_rects
            .get(&node)
            .copied()
            .expect("computed rect");
        assert_eq!(rect.center, Vector2::new(0.0, -250.0));
        assert_eq!(rect.min().y, -300.0);
        assert_eq!(rect.max().y, -200.0);

        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertPanel { node: n, rect, .. })
                if *n == node && rect.pivot == [0.5, 1.0]
        )));
    }

    #[test]
    fn ui_pivot_changes_render_pivot_without_changing_anchor_layout() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let centered = insert_panel(&mut runtime, [100.0, 50.0], Color::new(0.1, 0.2, 0.3, 1.0));
        if let Some(mut scene_node) = runtime.nodes.get_mut(centered)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.layout.anchor = UiAnchor::Bottom;
            panel.transform.pivot = UiVector2::ratio(0.5, 0.5);
        }

        let top_pivot = insert_panel(&mut runtime, [100.0, 50.0], Color::new(0.1, 0.2, 0.3, 1.0));
        if let Some(mut scene_node) = runtime.nodes.get_mut(top_pivot)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.layout.anchor = UiAnchor::Bottom;
            panel.transform.pivot = UiVector2::ratio(0.5, 1.0);
        }

        runtime.extract_render_ui_commands();

        let centered_rect = runtime
            .render_ui
            .computed_rects
            .get(&centered)
            .copied()
            .expect("centered rect");
        let top_pivot_rect = runtime
            .render_ui
            .computed_rects
            .get(&top_pivot)
            .copied()
            .expect("top pivot rect");
        assert_eq!(top_pivot_rect.center, centered_rect.center);

        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertPanel { node, rect, .. })
                if *node == top_pivot && rect.pivot == [0.5, 1.0]
        )));
    }

    #[test]
    fn ui_center_and_right_anchor_translation_can_reach_same_parent_point() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let center = insert_panel(&mut runtime, [200.0, 80.0], Color::new(0.1, 0.2, 0.3, 1.0));
        if let Some(mut scene_node) = runtime.nodes.get_mut(center)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.layout.anchor = UiAnchor::Center;
            panel.transform.translation = Vector2::new(0.25, 0.0);
        }

        let right = insert_panel(&mut runtime, [200.0, 80.0], Color::new(0.1, 0.2, 0.3, 1.0));
        if let Some(mut scene_node) = runtime.nodes.get_mut(right)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.layout.anchor = UiAnchor::Right;
            panel.transform.translation = Vector2::new(-0.125, 0.0);
        }

        runtime.extract_render_ui_commands();

        let center_rect = runtime
            .render_ui
            .computed_rects
            .get(&center)
            .copied()
            .expect("center rect");
        let right_rect = runtime
            .render_ui
            .computed_rects
            .get(&right)
            .copied()
            .expect("right rect");
        assert_eq!(center_rect.center, Vector2::new(200.0, 0.0));
        assert_eq!(right_rect.center, center_rect.center);
    }

    #[test]
    fn ui_center_and_top_anchor_translation_can_reach_same_parent_point() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let center = insert_panel(&mut runtime, [100.0, 150.0], Color::new(0.1, 0.2, 0.3, 1.0));
        if let Some(mut scene_node) = runtime.nodes.get_mut(center)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.layout.anchor = UiAnchor::Center;
            panel.transform.translation = Vector2::new(0.0, 0.25);
        }

        let top = insert_panel(&mut runtime, [100.0, 150.0], Color::new(0.1, 0.2, 0.3, 1.0));
        if let Some(mut scene_node) = runtime.nodes.get_mut(top)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.layout.anchor = UiAnchor::Top;
            panel.transform.translation = Vector2::new(0.0, -0.125);
        }

        runtime.extract_render_ui_commands();

        let center_rect = runtime
            .render_ui
            .computed_rects
            .get(&center)
            .copied()
            .expect("center rect");
        let top_rect = runtime
            .render_ui
            .computed_rects
            .get(&top)
            .copied()
            .expect("top rect");
        assert_eq!(center_rect.center, Vector2::new(0.0, 150.0));
        assert_eq!(top_rect.center, center_rect.center);
    }

    #[test]
    fn ui_parent_scale_preserves_child_virtual_layout_size() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut parent = UiPanel::new();
        parent.layout.size = UiVector2::pixels(200.0, 100.0);
        parent.transform.scale = Vector2::new(0.5, 0.5);
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(parent)));

        let mut child = UiPanel::new();
        child.layout.size = UiVector2::ratio(1.0, 1.0);
        let child = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(child)));
        attach_child(&mut runtime, parent, child);

        runtime.extract_render_ui_commands();

        let parent_rect = runtime
            .render_ui
            .computed_rects
            .get(&parent)
            .expect("parent rect exists");
        let child_rect = runtime
            .render_ui
            .computed_rects
            .get(&child)
            .expect("child rect exists");

        assert_eq!(parent_rect.size, Vector2::new(100.0, 50.0));
        assert_eq!(child_rect.center, parent_rect.center);
        assert_eq!(child_rect.size, parent_rect.size);
    }

    #[test]
    fn dirty_ui_node_emits_changed_upsert_only() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let node = insert_panel(&mut runtime, [120.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        runtime.clear_dirty_flags();

        if let Some(mut scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.style.fill = Color::new(0.8, 0.1, 0.1, 1.0);
        }
        runtime.mark_needs_rerender(node);
        runtime.extract_render_ui_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);

        assert_eq!(commands.len(), 1);
        assert!(
            matches!(&commands[0], RenderCommand::Ui(UiCommand::UpsertPanel { node: n, fill, .. }) if *n == node && *fill == rgba(0.8, 0.1, 0.1, 1.0))
        );
    }

    #[test]
    fn ui_reparent_marks_layout_dirty_without_resize() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut parent_a = UiPanel::new();
        parent_a.layout.size = UiVector2::pixels(200.0, 200.0);
        parent_a.transform.translation.x = -0.125;
        let parent_a = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(parent_a)));

        let mut parent_b = UiPanel::new();
        parent_b.layout.size = UiVector2::pixels(200.0, 200.0);
        parent_b.transform.translation.x = 0.125;
        let parent_b = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(parent_b)));

        let child = insert_panel(&mut runtime, [40.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        attach_child(&mut runtime, parent_a, child);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        assert!(runtime.reparent(parent_b, child));
        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertPanel { node, rect, .. })
                if *node == child && rect.center == [100.0, 0.0]
        )));
    }

    #[test]
    fn ui_descendant_reparented_via_non_ui_wrapper_recomputes_parent_space() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut preview = UiPanel::new();
        preview.layout.size = UiVector2::ratio(1.0, 1.0);
        preview.transform.scale = Vector2::new(0.5, 0.5);
        let preview = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(preview)));

        let wrapper = runtime.create::<perro_nodes::Node2D>();
        let mut ui_root = UiPanel::new();
        ui_root.layout.size = UiVector2::ratio(1.0, 1.0);
        let ui_root = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(ui_root)));
        attach_child(&mut runtime, wrapper, ui_root);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        assert!(runtime.reparent(preview, wrapper));
        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertPanel { node, rect, .. })
                if *node == ui_root && rect.size == [400.0, 300.0]
        )));
    }

    #[test]
    fn ui_descendant_under_node3d_wrapper_resolves_against_closest_ui_parent() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut root = UiPanel::new();
        root.layout.size = UiVector2::ratio(0.5, 0.5);
        let root = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(root)));

        let wrapper = runtime.create::<perro_nodes::Node3D>();
        let mut child = UiPanel::new();
        child.layout.size = UiVector2::ratio(1.0, 1.0);
        let child = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(child)));
        attach_child(&mut runtime, wrapper, child);
        attach_child(&mut runtime, root, wrapper);

        runtime.extract_render_ui_commands();

        let root_rect = runtime
            .render_ui
            .computed_rects
            .get(&root)
            .copied()
            .expect("root rect exists");
        let child_rect = runtime
            .render_ui
            .computed_rects
            .get(&child)
            .copied()
            .expect("child rect exists");
        assert_eq!(child_rect.size, root_rect.size);
        assert_eq!(child_rect.center, root_rect.center);
    }

    #[test]
    fn ui_descendant_under_animation_player_wrapper_resolves_against_closest_ui_parent() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut root = UiPanel::new();
        root.layout.size = UiVector2::ratio(0.5, 0.5);
        let root = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(root)));

        let wrapper = runtime.create::<perro_nodes::AnimationPlayer>();
        let mut child = UiPanel::new();
        child.layout.size = UiVector2::ratio(1.0, 1.0);
        let child = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(child)));
        attach_child(&mut runtime, wrapper, child);
        attach_child(&mut runtime, root, wrapper);

        runtime.extract_render_ui_commands();

        let root_rect = runtime
            .render_ui
            .computed_rects
            .get(&root)
            .copied()
            .expect("root rect exists");
        let child_rect = runtime
            .render_ui
            .computed_rects
            .get(&child)
            .copied()
            .expect("child rect exists");
        assert_eq!(child_rect.size, root_rect.size);
        assert_eq!(child_rect.center, root_rect.center);
    }

}
