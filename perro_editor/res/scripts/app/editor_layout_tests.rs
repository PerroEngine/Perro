#[cfg(all(test, not(feature = "dynamic-scripts")))]
mod tests {
    use crate::scripts::editor::main::EditorState;
    use perro_api::prelude::*;
    use perro_api::runtime_api::RuntimeWindow;
    use perro_runtime::{ProviderMode, Runtime, RuntimeProject};

    fn editor(width: u32, height: u32) -> (Runtime, NodeID) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        perro_api::modules::file::set_project_root_disk(
            root.to_string_lossy().as_ref(),
            "Perro Editor",
        );
        let project = RuntimeProject::from_project_dir(&root).unwrap();
        let mut runtime = Runtime::from_project_with_script_registry_deferred_boot(
            project,
            ProviderMode::Static,
            Some(crate::SCRIPT_REGISTRY),
        );
        runtime.set_viewport_size(width, height);
        let editor = RuntimeWindow::new(&mut runtime)
            .Scene()
            .load("res://scenes/manager/project_manager.scn")
            .unwrap();
        runtime.update(1.0 / 60.0);
        {
            let mut run = RuntimeWindow::new(&mut runtime);
            let shell = run.Scene().load("res://scenes/editor/shell.scn").unwrap();
            assert!(run.Nodes().reparent(editor, shell));
            with_state_mut!(run, EditorState, editor, |state| {
                state.editor_shell_root = shell.as_u64();
                state.viewport_mode = "2D".into();
                state.file_paths = vec![
                    "res://scenes/".into(),
                    "res://scenes/main.scn".into(),
                    "res://scripts/".into(),
                    "res://scripts/player.rs".into(),
                ];
                state.file_expanded_paths = vec!["res://".into(), "res://scenes/".into()];
                state.sidebar_mode = "files".into();
                state.doc_text = "$root = @root\n[root]\n[Node3D]\nposition = (1.0, 2.0, 3.0)\n[/Node3D]\n[/root]\n".into();
                state.selected_key = Some(0);
                state.selected_keys = vec![0];
            })
            .unwrap();
            let manager = run
                .Nodes()
                .find_node_by_name(editor, "project_manager")
                .unwrap();
            with_node_mut!(run, UiVLayout, manager, |node| node.visible = false);
            let mode = run
                .Nodes()
                .find_node_by_name(editor, "mode_2d_button")
                .unwrap();
            assert_eq!(
                run.Signals()
                    .emit(signal!("editor_mode_2d"), &[Variant::from(mode)]),
                1
            );
            assert_eq!(
                with_state!(run, EditorState, editor, |s| s.log.clone()).as_deref(),
                Some("mode 2D")
            );
        }
        for _ in 0..3 {
            runtime.begin_input_frame();
            runtime.update(1.0 / 60.0);
            runtime.extract_render_ui_commands();
            runtime.drain_render_commands(&mut Vec::new());
            runtime.clear_dirty_flags();
        }
        (runtime, editor)
    }

    fn bounds(runtime: &mut Runtime, root: NodeID, name: &str) -> (Vector2, Vector2) {
        let mut run = RuntimeWindow::new(runtime);
        let id = run
            .Nodes()
            .find_node_by_name(root, name)
            .unwrap_or_else(|| panic!("missing {name}"));
        let rect = run
            .Nodes()
            .get_ui_rect_pixels(id)
            .unwrap_or_else(|| panic!("no visible bounds for {name}"));
        (rect.min(), rect.max())
    }

    #[test]
    fn editor_panels_fit_without_overlap_at_common_window_sizes() {
        for (width, height) in [(1024, 768), (1280, 720), (1920, 1080)] {
            let (mut runtime, root) = editor(width, height);
            let mut previous_right = -(width as f32) * 0.5;
            for name in ["left_panel", "center_stack", "inspector_panel"] {
                let (min, max) = bounds(&mut runtime, root, name);
                assert!(
                    min.x >= previous_right - 1.0,
                    "{width}x{height}: {name} overlaps previous pane: {min:?} < {previous_right}"
                );
                assert!(max.x <= width as f32 * 0.5 + 1.0, "{name} exceeds window");
                assert!(
                    min.y >= -(height as f32) * 0.5 - 1.0 && max.y <= height as f32 * 0.5 + 1.0,
                    "{name} exceeds window height"
                );
                assert!(max.x - min.x > 120.0, "{name} is too narrow");
                previous_right = max.x;
            }
            let (viewport_min, viewport_max) = bounds(&mut runtime, root, "viewport_panel");
            assert!(
                viewport_max.x - viewport_min.x >= 380.0,
                "viewport too narrow at {width}x{height}"
            );
            assert!(
                viewport_max.y - viewport_min.y >= 350.0,
                "viewport too short at {width}x{height}"
            );
            let rows = runtime
                .nodes
                .iter()
                .filter_map(|(id, node)| match node.data {
                    perro_api::nodes::SceneNodeData::UiHLayout(_) => {
                        Some((id, node.name.to_string(), false))
                    }
                    perro_api::nodes::SceneNodeData::UiVLayout(_) => {
                        Some((id, node.name.to_string(), true))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let field_labels = runtime
                .nodes
                .iter()
                .filter_map(|(id, node)| {
                    let perro_api::nodes::SceneNodeData::UiLabel(label) = &node.data else {
                        return None;
                    };
                    let name = node.name.to_string();
                    (name.starts_with("inspector_var_")
                        && name.ends_with("_name")
                        && !label.text.is_empty())
                    .then_some((id, name, label.font_size, label.text_size_ratio))
                })
                .collect::<Vec<_>>();
            let mut run = RuntimeWindow::new(&mut runtime);
            let mut errors = Vec::new();
            for axis in ["v", "h"] {
                for idx in 0..33 {
                    assert!(
                        run.Nodes()
                            .find_node_by_name(root, format!("canvas_{axis}_{idx}"))
                            .is_some(),
                        "grid pool must have real nodes"
                    );
                }
            }
            let mut visible_fields = 0;
            for (id, name, font_size, ratio) in field_labels {
                let Some(rect) = run.Nodes().get_ui_rect_pixels(id) else {
                    continue;
                };
                let text_px = if ratio > 0.0 {
                    rect.size.y * ratio
                } else {
                    font_size
                };
                if text_px < 12.0 || text_px > rect.size.y + 1.0 {
                    errors.push(format!(
                        "{name} unreadable: font {text_px}px in {}px row",
                        rect.size.y
                    ));
                }
                visible_fields += 1;
            }
            assert!(
                visible_fields >= 3,
                "selected Node3D must populate inspector"
            );
            for (id, row_name, vertical) in rows {
                let Some(row) = run.Nodes().get_ui_rect_pixels(id) else {
                    continue;
                };
                let mut children = run
                    .Nodes()
                    .get_children(id)
                    .into_iter()
                    .filter_map(|child| {
                        run.Nodes()
                            .get_ui_rect_pixels(child)
                            .map(|rect| (child, rect))
                    })
                    .collect::<Vec<_>>();
                if vertical {
                    children.sort_by(|a, b| b.1.max().y.total_cmp(&a.1.max().y));
                    let mut bottom = f32::INFINITY;
                    for (child, rect) in children {
                        let name = run.Nodes().get_node_name(child).unwrap_or_default();
                        if rect.max().y > bottom + 1.0 {
                            errors.push(format!("{row_name}/{name} overlaps vertical sibling"));
                        }
                        bottom = rect.min().y;
                    }
                    continue;
                }
                children.sort_by(|a, b| a.1.min().x.total_cmp(&b.1.min().x));
                let mut right = row.min().x;
                for (child, rect) in children {
                    let name = run.Nodes().get_node_name(child).unwrap_or_default();
                    if rect.min().x < right - 1.0 {
                        errors.push(format!("{row_name}/{name} overlaps sibling"));
                    }
                    if rect.max().x > row.max().x + 1.0 {
                        errors.push(format!("{row_name}/{name} exceeds row"));
                    }
                    right = rect.max().x;
                }
            }
            assert!(errors.is_empty(), "{width}x{height}: {}", errors.join("\n"));
        }
    }

    #[test]
    #[ignore = "manual geometry diagnostic; emits SVG from actual computed bounds"]
    fn dump_editor_layout_geometry() {
        let (mut runtime, _) = editor(1280, 720);
        let nodes = runtime
            .nodes
            .iter()
            .map(|(id, node)| (id, node.name.to_string()))
            .collect::<Vec<_>>();
        let mut svg = String::from(
            "<svg xmlns='http://www.w3.org/2000/svg' width='1280' height='720'><rect width='100%' height='100%' fill='#171a1f'/>",
        );
        let mut run = RuntimeWindow::new(&mut runtime);
        for (id, name) in nodes {
            let Some(rect) = run.Nodes().get_ui_rect_pixels(id) else {
                continue;
            };
            if rect.size.x < 15.0 || rect.size.y < 12.0 {
                continue;
            }
            let min = rect.min();
            let x = min.x + 640.0;
            let y = 360.0 - rect.max().y;
            let label = name.replace('&', "&amp;").replace('<', "&lt;");
            svg.push_str(&format!("<rect x='{x}' y='{y}' width='{}' height='{}' fill='none' stroke='#738096' stroke-width='0.5'/><text x='{}' y='{}' fill='#d5dce6' font-family='sans-serif' font-size='9'>{label}</text>", rect.size.x, rect.size.y, x+2.0, y+10.0));
        }
        svg.push_str("</svg>");
        let output =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/editor-layout.svg");
        std::fs::write(&output, svg).unwrap();
        eprintln!("layout geometry: {}", output.display());
    }
}
