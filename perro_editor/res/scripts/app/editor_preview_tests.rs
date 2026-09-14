#[cfg(all(test, not(feature = "dynamic-scripts")))]
mod tests {
    use crate::scripts::editor::main::EditorState;
    use perro_api::prelude::*;
    use perro_api::runtime_api::RuntimeWindow;
    use perro_render_bridge::{
        CameraStreamCommand, CameraStreamDraw3DState, CameraStreamSourceState, RenderCommand,
        RenderEvent, RenderRequestID, ResourceCommand,
    };
    use perro_runtime::{ProviderMode, Runtime, RuntimeProject};
    use std::path::{Path, PathBuf};

    struct PreviewFixture {
        path: PathBuf,
        res_path: String,
    }

    impl PreviewFixture {
        fn create(root: &Path) -> Self {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos();
            let name = format!("__editor_preview_test_{}_{}.scn", std::process::id(), stamp);
            let path = root.join("res/scenes").join(&name);
            let text = r#"$root = @PreviewRoot

[PreviewRoot]
[Node3D]
[/Node3D]
[/PreviewRoot]

[PreviewMesh]
parent = @PreviewRoot
[MeshInstance3D]
    mesh = "__cube__"
    position = (0, 0, 0)
[/MeshInstance3D]
[/PreviewMesh]
"#;
            std::fs::write(&path, text).expect("write preview scene");
            Self {
                path,
                res_path: format!("res://scenes/{name}"),
            }
        }
    }

    impl Drop for PreviewFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn project_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("editor project root")
    }

    fn collect_snapshot(runtime: &mut Runtime) -> Vec<RenderCommand> {
        let mut commands = Vec::new();
        runtime.extract_render_snapshot_commands(&mut commands);
        commands
    }

    fn synthetic_mesh_id(request: RenderRequestID) -> MeshID {
        MeshID::from_parts(0x7000 | (request.0 as u32 & 0x0fff), 1)
    }

    fn synthetic_material_id(request: RenderRequestID) -> MaterialID {
        MaterialID::from_parts(0x7000 | (request.0 as u32 & 0x0fff), 1)
    }

    /// Simulate the backend resource ack used by the app's render thread.
    /// Builtin meshes still wait on this ack before stream extraction emits a
    /// draw, even though their CPU data resolves synchronously.
    fn ack_resource_loads(runtime: &mut Runtime, commands: &[RenderCommand]) -> bool {
        let mut events = Vec::new();
        for command in commands {
            let RenderCommand::Resource(resource) = command else {
                continue;
            };
            match resource.as_ref() {
                ResourceCommand::CreateMesh { request, id, .. } => {
                    events.push(RenderEvent::MeshCreated {
                        request: *request,
                        id: if id.is_nil() {
                            synthetic_mesh_id(*request)
                        } else {
                            *id
                        },
                        mesh: None,
                    });
                }
                ResourceCommand::CreateMaterial { request, id, .. } => {
                    events.push(RenderEvent::MaterialCreated {
                        request: *request,
                        id: if id.is_nil() {
                            synthetic_material_id(*request)
                        } else {
                            *id
                        },
                    });
                }
                _ => {}
            }
        }
        let acked = !events.is_empty();
        for event in events {
            runtime.apply_render_event(event);
        }
        acked
    }

    fn settle_preview_resources(runtime: &mut Runtime) -> Vec<RenderCommand> {
        let mut commands = collect_snapshot(runtime);
        for _ in 0..4 {
            if !ack_resource_loads(runtime, &commands) {
                break;
            }
            runtime.update(1.0 / 60.0);
            commands = collect_snapshot(runtime);
        }
        commands
    }

    fn has_3d_stream_upsert(commands: &[RenderCommand]) -> bool {
        commands.iter().any(|command| {
            matches!(
                command,
                RenderCommand::CameraStream(CameraStreamCommand::Upsert { state, .. })
                    if matches!(&state.source, CameraStreamSourceState::ThreeD(_))
            )
        })
    }

    #[test]
    fn editor_3d_preview_assigns_camera_and_extracts_mesh() {
        let root = project_root();
        let fixture = PreviewFixture::create(&root);
        perro_api::modules::file::set_project_root_disk(
            root.to_string_lossy().as_ref(),
            "Perro Editor",
        );
        let project = RuntimeProject::from_project_dir(&root).expect("load editor project");
        let mut runtime = Runtime::from_project_with_script_registry_deferred_boot(
            project,
            ProviderMode::Static,
            Some(crate::SCRIPT_REGISTRY),
        );
        runtime.set_viewport_size(1280, 720);

        let editor = RuntimeWindow::new(&mut runtime)
            .Scene()
            .load("res://scenes/manager/project_manager.scn")
            .expect("load project manager");
        runtime.update(1.0 / 60.0);

        {
            let mut run = RuntimeWindow::new(&mut runtime);
            let shell = run
                .Scene()
                .load("res://scenes/editor/shell.scn")
                .expect("load editor shell");
            assert!(run.Nodes().reparent(editor, shell));
            with_state_mut!(run, EditorState, editor, |state| {
                state.editor_shell_root = shell.as_u64();
                state.project_root = root.to_string_lossy().to_string();
                state.viewport_mode = "2D".into();
                state.file_paths = vec!["res://scenes/".into(), fixture.res_path.clone()];
                state.file_expanded_paths = vec!["res://".into(), "res://scenes/".into()];
                state.file_filter.clear();
                state.file_scope.clear();
                state.sidebar_mode = "files".into();
                state.activity_mode = "scene".into();
            })
            .expect("init editor state");

            let manager = run
                .Nodes()
                .find_node_by_name(editor, "project_manager")
                .expect("project manager node");
            with_node_mut!(run, UiVLayout, manager, |node| node.visible = false);

            let mode = run
                .Nodes()
                .find_node_by_name(editor, "mode_3d_button")
                .expect("3D mode button");
            assert_eq!(
                run.Signals()
                    .emit(signal!("editor_mode_3d"), &[Variant::from(mode)]),
                1
            );

            // `filtered_file_paths` inserts the synthetic root before the
            // expanded scene folder and its scene file.
            let file_tree = run
                .Nodes()
                .find_node_by_name(editor, "file_rows")
                .expect("file tree");
            assert_eq!(
                run.Signals().emit(
                    signal!("editor_file_tree_selected"),
                    &[
                        Variant::from(file_tree),
                        Variant::from(2_i32),
                        Variant::from(fixture.res_path.clone()),
                    ],
                ),
                1
            );

            let state = with_state!(run, EditorState, editor, |state| {
                (
                    state.active_asset_path.clone(),
                    state.viewport_mode.clone(),
                    state.preview_root,
                    state.preview_camera_3d,
                )
            })
            .expect("editor state after scene open");
            assert_eq!(state.0, fixture.res_path, "file signal must open scene");
            assert_eq!(state.1, "3D", "3D scene root picks 3D viewport");
            assert_ne!(state.2, 0, "preview root must load");
            assert_ne!(state.3, 0, "3D preview must create camera");

            let stream = run
                .Nodes()
                .find_node_by_name(editor, "viewport_stream_3d")
                .expect("3D preview stream");
            let stream_state = with_node!(run, UiCameraStream, stream, |node| {
                (node.stream.camera, node.visible, node.stream.enabled)
            })
            .expect("3D preview stream data");
            assert_eq!(stream_state.0.as_u64(), state.3);
            assert!(stream_state.1, "3D stream must show for 3D mode");
            assert!(stream_state.2, "3D stream must stay enabled");

            let frame = run
                .Nodes()
                .find_node_by_name(editor, "viewport_frame_button")
                .expect("frame button");
            assert_eq!(
                run.Signals()
                    .emit(signal!("editor_viewport_frame"), &[Variant::from(frame)]),
                1
            );
            let framed = with_state!(run, EditorState, editor, |state| {
                (state.cam_x, state.cam_y, state.cam_z, state.log.clone())
            })
            .expect("framed editor state");
            assert_eq!(framed.0, 0.0);
            assert_eq!(framed.1, 3.0);
            assert_eq!(framed.2, 8.0);
            assert!(
                framed.3.starts_with("frame 3d\n"),
                "frame log: {}",
                framed.3
            );
        }

        // Update + snapshot extract exercise the same bridge path used by the
        // app frame, including the UI stream's 3D source world.
        runtime.update(1.0 / 60.0);
        let commands = settle_preview_resources(&mut runtime);
        let stream = RuntimeWindow::new(&mut runtime)
            .Nodes()
            .find_node_by_name(editor, "viewport_stream_3d")
            .expect("3D preview stream after extraction");
        let stream_state = commands
            .iter()
            .find_map(|command| match command {
                RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                    if *node == stream =>
                {
                    Some(state.as_ref())
                }
                _ => None,
            })
            .expect("3D preview stream render state");
        assert!(
            matches!(&stream_state.source, CameraStreamSourceState::ThreeD(_)),
            "preview stream source must be 3D"
        );
        assert!(
            stream_state.draws_3d.iter().any(|draw| matches!(
                draw,
                CameraStreamDraw3DState::Draw { mesh, .. } if !mesh.is_nil()
            )),
            "3D stream must contain a mesh draw"
        );

        // Idle frames keep the settled stream stable before a live resize.
        for _ in 0..3 {
            runtime.update(1.0 / 60.0);
            let _ = collect_snapshot(&mut runtime);
        }

        runtime.set_viewport_size(1024, 768);
        let mut resized_commands = Vec::new();
        for _ in 0..3 {
            runtime.update(1.0 / 60.0);
            let frame_commands = collect_snapshot(&mut runtime);
            if ack_resource_loads(&mut runtime, &frame_commands) {
                runtime.update(1.0 / 60.0);
                let ack_commands = collect_snapshot(&mut runtime);
                if has_3d_stream_upsert(&ack_commands) {
                    resized_commands = ack_commands;
                }
            } else if has_3d_stream_upsert(&frame_commands) {
                resized_commands = frame_commands;
            }
        }
        let (panel_size, stream_resolution, stream_visible, stream) = {
            let mut run = RuntimeWindow::new(&mut runtime);
            let panel = run
                .Nodes()
                .find_node_by_name(editor, "viewport_panel")
                .expect("viewport panel after resize");
            let stream = run
                .Nodes()
                .find_node_by_name(editor, "viewport_stream_3d")
                .expect("3D stream after resize");
            let panel_size = run
                .Nodes()
                .get_ui_rect_pixels(panel)
                .expect("viewport panel bounds after resize")
                .size;
            let stream_data = with_node!(run, UiCameraStream, stream, |node| {
                (
                    [
                        node.stream.resolution.x as f32,
                        node.stream.resolution.y as f32,
                    ],
                    node.visible,
                )
            })
            .expect("3D stream data after resize");
            (panel_size, stream_data.0, stream_data.1, stream)
        };
        assert!(stream_visible, "3D stream stays visible after resize");
        assert!((stream_resolution[0] - panel_size.x).abs() <= 1.0);
        assert!((stream_resolution[1] - panel_size.y).abs() <= 1.0);
        let resized_stream = resized_commands
            .iter()
            .find_map(|command| match command {
                RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
                    if *node == stream =>
                {
                    Some(state.as_ref())
                }
                _ => None,
            })
            .expect("resized 3D stream render state");
        assert!(resized_stream.draws_3d.iter().any(|draw| matches!(
            draw,
            CameraStreamDraw3DState::Draw { mesh, .. } if !mesh.is_nil()
        )));
    }
}
