#[cfg(all(test, not(feature = "dynamic-scripts")))]
mod tests {
    use crate::scripts::editor::main::EditorState;
    use perro_api::prelude::*;
    use perro_api::runtime_api::RuntimeWindow;
    use perro_runtime::{ProviderMode, Runtime, RuntimeProject};

    #[test]
    fn source_project_is_browsable_without_generated_cache() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("perro-editor-open-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(root.join("res/scenes")).unwrap();
        std::fs::write(root.join("project.toml"), "[project]\nname = 'New Game'\n").unwrap();
        std::fs::write(
            root.join("res/scenes/main.scn"),
            "$root = @root\n[root]\n[Node2D]\n[/Node2D]\n[/root]\n",
        )
        .unwrap();
        let result = crate::scripts::app::editor_manager::canonical_project_root(&root);
        let paths = crate::scripts::assets::editor_assets::scan_res_paths(&root);
        assert!(!root.join(".perro").exists());
        std::fs::remove_dir_all(&root).unwrap();
        assert!(result.is_ok(), "{result:?}");
        assert!(
            paths
                .unwrap()
                .contains(&"res://scenes/main.scn".to_string())
        );
    }

    #[test]
    fn manager_scene_connects_button_signals_to_editor_state() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let project = RuntimeProject::from_project_dir(&root).unwrap();
        perro_api::modules::file::set_project_root_disk(
            root.to_string_lossy().as_ref(),
            "Perro Editor",
        );
        let mut runtime = Runtime::from_project_with_script_registry_deferred_boot(
            project,
            ProviderMode::Static,
            Some(crate::SCRIPT_REGISTRY),
        );
        let editor = RuntimeWindow::new(&mut runtime)
            .Scene()
            .load("res://scenes/manager/project_manager.scn")
            .unwrap();
        runtime.set_viewport_size(1280, 720);
        runtime.update(1.0 / 60.0);
        let mut run = RuntimeWindow::new(&mut runtime);
        assert_eq!(
            with_state!(run, EditorState, editor, |s| s.log.clone()).as_deref(),
            Some("project manager")
        );
        let button = run
            .Nodes()
            .find_node_by_name(editor, "manager_create_button")
            .unwrap();
        let calls = run
            .Signals()
            .emit(signal!("editor_manager_create"), &[Variant::from(button)]);
        assert_eq!(calls, 1, "manager button must reach editor method");
        let log = with_state!(run, EditorState, editor, |s| s.log.clone()).unwrap();
        assert!(log.contains("pick location first"), "manager log: {log}");

        let path_button = run
            .Nodes()
            .find_node_by_name(editor, "manager_open_path_button")
            .unwrap();
        run.Signals().emit(
            signal!("editor_manager_open_path"),
            &[Variant::from(path_button)],
        );
        let log = with_state!(run, EditorState, editor, |s| s.log.clone()).unwrap();
        assert!(
            log.contains("enter project folder path"),
            "manager log: {log}"
        );
        let status = run
            .Nodes()
            .find_node_by_name(editor, "manager_status_label")
            .unwrap();
        assert_eq!(
            with_node!(run, UiLabel, status, |node| node.text.to_string()).as_deref(),
            Some(log.as_str()),
            "manager must show its error before the editor shell exists"
        );

        let path_box = run
            .Nodes()
            .find_node_by_name(editor, "manager_open_path_box")
            .unwrap();
        run.Signals().emit(
            signal!("editor_inspector_focus"),
            &[Variant::from(path_box)],
        );
        assert_eq!(
            with_state!(run, EditorState, editor, |s| s
                .focused_inspector_box
                .clone())
            .as_deref(),
            Some("manager_open_path_box")
        );
    }
}
