use perro_api::prelude::*;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

type SelfNodeType = UiPanel;

const ACTIVE_PROJECT: &str = "user://perro_editor_active_project.txt";
const MAX_SCENE_ROWS: usize = 200;
const MAX_RESOURCE_ROWS: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Variant)]
enum SceneViewerMode {
    #[default]
    Empty,
    Ui,
    TwoD,
    ThreeD,
    Mixed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Variant)]
enum EditorViewMode {
    #[default]
    Auto,
    Ui,
    TwoD,
    ThreeD,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Variant)]
enum ActivePreview {
    #[default]
    Placeholder,
    LiveUi(NodeID),
}

#[State]
#[derive(Clone)]
struct EditorState {
    project_dir: String,
    main_scene: String,
    script_path: String,
    live_scene_path: String,
    scene_mode: SceneViewerMode,
    view_mode: EditorViewMode,
    project_label: NodeID,
    scene_label: NodeID,
    mode_label: NodeID,
    preview_status: NodeID,
    preview_stage: NodeID,
    inspector_body: NodeID,
    status_label: NodeID,
    tab_auto_button: NodeID,
    tab_ui_button: NodeID,
    tab_2d_button: NodeID,
    tab_3d_button: NodeID,
    node_details: Vec<String>,
    resource_paths: Vec<String>,
    active_preview: ActivePreview,
    live_root: NodeID,
    run_pid: u32,
}

lifecycle!({
    fn on_init(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self_id: NodeID,
    ) {
    }

    fn on_all_init(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {
        let top_bar = get_child!(ctx, self_id, "top_bar").unwrap_or_default();
        let top_row = get_child!(ctx, top_bar, "top_bar_row").unwrap_or_default();
        let workspace = get_child!(ctx, self_id, "workspace").unwrap_or_default();
        let left_panel = get_child!(ctx, workspace, "left_panel").unwrap_or_default();
        let left_stack = get_child!(ctx, left_panel, "left_vlayout").unwrap_or_default();
        let scene_section = get_child!(ctx, left_stack, "scene_section").unwrap_or_default();
        let scene_stack = get_child!(ctx, scene_section, "scene_rows").unwrap_or_default();
        let fs_section = get_child!(ctx, left_stack, "fs_section").unwrap_or_default();
        let fs_stack = get_child!(ctx, fs_section, "fs_rows").unwrap_or_default();
        let preview_panel = get_child!(ctx, workspace, "preview_panel").unwrap_or_default();
        let preview_stack = get_child!(ctx, preview_panel, "preview_vlayout").unwrap_or_default();
        let preview_header = get_child!(ctx, preview_stack, "preview_header").unwrap_or_default();
        let preview_canvas = get_child!(ctx, preview_stack, "preview_canvas").unwrap_or_default();
        let inspector_panel = get_child!(ctx, workspace, "inspector_panel").unwrap_or_default();
        let inspector_stack =
            get_child!(ctx, inspector_panel, "inspector_vlayout").unwrap_or_default();
        let inspector_body_panel =
            get_child!(ctx, inspector_stack, "inspector_body").unwrap_or_default();
        let bottom_bar = get_child!(ctx, self_id, "bottom_bar").unwrap_or_default();

        let project_label = get_child!(ctx, top_row, "project_label").unwrap_or_default();
        let scene_label = get_child!(ctx, top_row, "scene_label").unwrap_or_default();
        let mode_label = get_child!(ctx, preview_header, "preview_mode_label").unwrap_or_default();
        let preview_status = get_child!(ctx, preview_canvas, "preview_status").unwrap_or_default();
        let preview_stage = get_child!(ctx, preview_canvas, "preview_stage").unwrap_or_default();
        let inspector_body =
            get_child!(ctx, inspector_body_panel, "inspector_body_text").unwrap_or_default();
        let status_label = get_child!(ctx, bottom_bar, "status_label").unwrap_or_default();

        let tab_auto_button = get_child!(ctx, preview_header, "tab_auto_button").unwrap_or_default();
        let tab_ui_button = get_child!(ctx, preview_header, "tab_ui_button").unwrap_or_default();
        let tab_2d_button = get_child!(ctx, preview_header, "tab_2d_button").unwrap_or_default();
        let tab_3d_button = get_child!(ctx, preview_header, "tab_3d_button").unwrap_or_default();

        let project_dir = FileMod::load_string(ACTIVE_PROJECT).unwrap_or_default();
        let project_dir = project_dir.trim().to_string();
        if !project_dir.is_empty() {
            FileMod::set_project_root_disk(&project_dir, "perro_editor_live_project");
        }

        let main_scene =
            read_main_scene(&project_dir).unwrap_or_else(|| "res://main.scn".to_string());
        let resource_paths = resource_rows(&project_dir);
        let mut script_path = "res://scripts/script.rs".to_string();
        let mut node_details = Vec::new();
        let mut scene_mode = SceneViewerMode::Empty;
        let mut live_scene_path = String::new();

        set_label(ctx, project_label, project_title(&project_dir));
        set_label(ctx, scene_label, &format!("Scene {main_scene}"));
        create_resource_rows(ctx, self_id, fs_stack, &resource_paths);
        match scene_load_doc!(res, main_scene.clone()) {
            Ok(doc) => {
                let rows = scene_graph_rows(&doc);
                create_scene_rows(ctx, self_id, scene_stack, &rows);
                node_details = rows.into_iter().map(|(_, detail)| detail).collect();
                script_path = doc
                    .scene
                    .nodes
                    .iter()
                    .find_map(|node| node.script.as_ref().map(|s| s.to_string()))
                    .unwrap_or(script_path);
                scene_mode = scene_viewer_mode(&doc);
                set_text_block(ctx, inspector_body, &inspector_summary(&doc));
                if scene_mode == SceneViewerMode::Ui {
                    live_scene_path = write_live_scene_doc(&doc).unwrap_or_default();
                }
            }
            Err(err) => {
                set_text_block(ctx, inspector_body, "No scene doc");
                set_text_block(ctx, preview_status, &format!("Scene load fail\n{err}"));
                set_label(ctx, status_label, &format!("Scene load fail: {err}"));
            }
        }

        let _ = with_state_mut!(ctx, EditorState, self_id, |state| {
            state.project_dir = project_dir;
            state.main_scene = main_scene;
            state.script_path = script_path;
            state.live_scene_path = live_scene_path;
            state.scene_mode = scene_mode;
            state.view_mode = EditorViewMode::Auto;
            state.project_label = project_label;
            state.scene_label = scene_label;
            state.mode_label = mode_label;
            state.preview_status = preview_status;
            state.preview_stage = preview_stage;
            state.inspector_body = inspector_body;
            state.status_label = status_label;
            state.tab_auto_button = tab_auto_button;
            state.tab_ui_button = tab_ui_button;
            state.tab_2d_button = tab_2d_button;
            state.tab_3d_button = tab_3d_button;
            state.node_details = node_details;
            state.resource_paths = resource_paths;
            state.active_preview = ActivePreview::Placeholder;
            state.live_root = NodeID::default();
            state.run_pid = 0;
        });

        signal_connect!(
            ctx,
            self_id,
            signal!("script_button_click"),
            func!("on_open_script")
        );
        signal_connect!(
            ctx,
            self_id,
            signal!("main_scene_button_click"),
            func!("on_open_main_scene")
        );
        signal_connect!(ctx, self_id, signal!("play_button_click"), func!("on_play"));
        signal_connect!(ctx, self_id, signal!("stop_button_click"), func!("on_stop"));
        signal_connect!(
            ctx,
            self_id,
            signal!("tab_auto_button_click"),
            func!("on_tab_auto")
        );
        signal_connect!(ctx, self_id, signal!("tab_ui_button_click"), func!("on_tab_ui"));
        signal_connect!(ctx, self_id, signal!("tab_2d_button_click"), func!("on_tab_2d"));
        signal_connect!(ctx, self_id, signal!("tab_3d_button_click"), func!("on_tab_3d"));

        set_view_mode(ctx, self_id, EditorViewMode::Auto);
    }

    fn on_update(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self_id: NodeID,
    ) {
    }

    fn on_fixed_update(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self_id: NodeID,
    ) {
    }

    fn on_removal(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self_id: NodeID,
    ) {
    }
});

methods!({
    fn on_open_script(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
        _button: NodeID,
    ) {
        let (project_dir, script_path, status_label) =
            with_state!(ctx, EditorState, self_id, |state| {
                (
                    state.project_dir.clone(),
                    state.script_path.clone(),
                    state.status_label,
                )
            });
        match open_code(&res_path_to_disk(&project_dir, &script_path)) {
            Ok(()) => set_label(ctx, status_label, "VS Code open script"),
            Err(err) => set_label(ctx, status_label, &format!("VS Code fail: {err}")),
        }
    }

    fn on_open_main_scene(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
        _button: NodeID,
    ) {
        let (project_dir, main_scene, status_label) =
            with_state!(ctx, EditorState, self_id, |state| {
                (
                    state.project_dir.clone(),
                    state.main_scene.clone(),
                    state.status_label,
                )
            });
        match open_code(&res_path_to_disk(&project_dir, &main_scene)) {
            Ok(()) => set_label(ctx, status_label, "VS Code open scene"),
            Err(err) => set_label(ctx, status_label, &format!("VS Code fail: {err}")),
        }
    }

    fn on_scene_node_row(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
        _button: NodeID,
        index: u64,
    ) {
        inspect_node(ctx, self_id, index as usize);
    }

    fn on_resource_row(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
        _button: NodeID,
        index: u64,
    ) {
        let (project_dir, status_label, path) = with_state!(ctx, EditorState, self_id, |state| {
            (
                state.project_dir.clone(),
                state.status_label,
                state.resource_paths.get(index as usize).cloned(),
            )
        });
        if let Some(path) = path {
            match open_code(&res_path_to_disk(&project_dir, &path)) {
                Ok(()) => set_label(ctx, status_label, &format!("VS Code open {path}")),
                Err(err) => set_label(ctx, status_label, &format!("VS Code fail: {err}")),
            }
        }
    }

    fn on_play(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
        _button: NodeID,
    ) {
        let (project_dir, status_label, current_pid) =
            with_state!(ctx, EditorState, self_id, |state| {
                (state.project_dir.clone(), state.status_label, state.run_pid)
            });
        if current_pid != 0 {
            set_label(ctx, status_label, "Game already running");
            return;
        }
        match spawn_game(&project_dir) {
            Ok(pid) => {
                let _ = with_state_mut!(ctx, EditorState, self_id, |state| state.run_pid = pid);
                set_label(ctx, status_label, &format!("Game run pid {pid}"));
            }
            Err(err) => set_label(ctx, status_label, &format!("Game run fail: {err}")),
        }
    }

    fn on_stop(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
        _button: NodeID,
    ) {
        let (status_label, pid) = with_state!(ctx, EditorState, self_id, |state| (
            state.status_label,
            state.run_pid
        ));
        if pid == 0 {
            set_label(ctx, status_label, "No game process");
            return;
        }
        match stop_game(pid) {
            Ok(()) => {
                let _ = with_state_mut!(ctx, EditorState, self_id, |state| state.run_pid = 0);
                set_label(ctx, status_label, "Game stop");
            }
            Err(err) => set_label(ctx, status_label, &format!("Game stop fail: {err}")),
        }
    }

    fn on_tab_auto(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
        _button: NodeID,
    ) {
        set_view_mode(ctx, self_id, EditorViewMode::Auto);
    }

    fn on_tab_ui(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
        _button: NodeID,
    ) {
        set_view_mode(ctx, self_id, EditorViewMode::Ui);
    }

    fn on_tab_2d(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
        _button: NodeID,
    ) {
        set_view_mode(ctx, self_id, EditorViewMode::TwoD);
    }

    fn on_tab_3d(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
        _button: NodeID,
    ) {
        set_view_mode(ctx, self_id, EditorViewMode::ThreeD);
    }
});

fn set_view_mode<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    self_id: NodeID,
    mode: EditorViewMode,
) {
    let mut prev_active = ActivePreview::Placeholder;
    let mut scene_mode = SceneViewerMode::Empty;
    let mut live_scene_path = String::new();
    let mut preview_stage = NodeID::default();
    let mut preview_status = NodeID::default();
    let mut mode_label = NodeID::default();
    let mut status_label = NodeID::default();
    let mut script_path = String::new();
    let mut tab_auto = NodeID::default();
    let mut tab_ui = NodeID::default();
    let mut tab_2d = NodeID::default();
    let mut tab_3d = NodeID::default();

    let _ = with_state_mut!(ctx, EditorState, self_id, |state| {
        prev_active = state.active_preview;
        scene_mode = state.scene_mode;
        live_scene_path = state.live_scene_path.clone();
        preview_stage = state.preview_stage;
        preview_status = state.preview_status;
        mode_label = state.mode_label;
        status_label = state.status_label;
        script_path = state.script_path.clone();
        tab_auto = state.tab_auto_button;
        tab_ui = state.tab_ui_button;
        tab_2d = state.tab_2d_button;
        tab_3d = state.tab_3d_button;
        state.view_mode = mode;
    });

    if let ActivePreview::LiveUi(root) = prev_active {
        if !root.is_nil() {
            let _ = remove_node!(ctx, root);
        }
    }

    let resolved = resolve_mode(mode, scene_mode);
    set_tab_visual(ctx, tab_auto, mode == EditorViewMode::Auto);
    set_tab_visual(ctx, tab_ui, mode == EditorViewMode::Ui);
    set_tab_visual(ctx, tab_2d, mode == EditorViewMode::TwoD);
    set_tab_visual(ctx, tab_3d, mode == EditorViewMode::ThreeD);

    let mut active_preview = ActivePreview::Placeholder;
    let mut mode_text = format!("{} ({})", mode.label(), scene_mode.label());
    let status_text: String;

    match resolved {
        SceneViewerMode::Ui => {
            if live_scene_path.is_empty() {
                set_text_block(
                    ctx,
                    preview_status,
                    "UI preview unavailable\nscene not UI-only or live doc build fail",
                );
                show_ui_box(ctx, preview_status);
                status_text = "UI preview fallback".to_string();
            } else {
                match scene_load!(ctx, live_scene_path.clone()) {
                    Ok(root) => {
                        if reparent!(ctx, preview_stage, root) {
                            apply_live_root(ctx, root);
                            disable_physics(ctx, root);
                            hide_ui_box(ctx, preview_status);
                            active_preview = ActivePreview::LiveUi(root);
                            let _ = with_state_mut!(ctx, EditorState, self_id, |state| {
                                state.live_root = root;
                            });
                            status_text = "UI preview live".to_string();
                        } else {
                            let _ = remove_node!(ctx, root);
                            set_text_block(ctx, preview_status, "UI preview parent fail");
                            show_ui_box(ctx, preview_status);
                            status_text = "UI preview fallback".to_string();
                        }
                    }
                    Err(err) => {
                        set_text_block(ctx, preview_status, &format!("UI preview load fail\n{err}"));
                        show_ui_box(ctx, preview_status);
                        status_text = "UI preview fallback".to_string();
                    }
                }
            }
        }
        SceneViewerMode::TwoD => {
            set_text_block(ctx, preview_status, "2D preview not wired yet");
            show_ui_box(ctx, preview_status);
            status_text = "2D preview placeholder".to_string();
        }
        SceneViewerMode::ThreeD => {
            set_text_block(ctx, preview_status, "3D preview not wired yet");
            show_ui_box(ctx, preview_status);
            status_text = "3D preview placeholder".to_string();
        }
        SceneViewerMode::Mixed => {
            set_text_block(
                ctx,
                preview_status,
                "Mixed scene\npick UI/2D/3D tab\n2D+3D preview not wired yet",
            );
            show_ui_box(ctx, preview_status);
            status_text = "Mixed preview placeholder".to_string();
        }
        SceneViewerMode::Empty => {
            set_text_block(ctx, preview_status, "No scene nodes");
            show_ui_box(ctx, preview_status);
            status_text = "No scene nodes".to_string();
        }
    }

    if mode == EditorViewMode::Ui && scene_mode != SceneViewerMode::Ui {
        mode_text = "UI (forced, incompatible scene)".to_string();
    }
    if mode == EditorViewMode::Auto {
        mode_text = format!("Auto -> {}", resolved.label());
    }

    set_label(ctx, mode_label, &mode_text);
    set_label(ctx, status_label, &format!("{status_text}; script {script_path}"));
    let _ = with_state_mut!(ctx, EditorState, self_id, |state| {
        state.active_preview = active_preview;
        state.live_root = match active_preview {
            ActivePreview::LiveUi(id) => id,
            ActivePreview::Placeholder => NodeID::default(),
        };
    });
}

fn resolve_mode(mode: EditorViewMode, scene_mode: SceneViewerMode) -> SceneViewerMode {
    match mode {
        EditorViewMode::Auto => scene_mode,
        EditorViewMode::Ui => SceneViewerMode::Ui,
        EditorViewMode::TwoD => SceneViewerMode::TwoD,
        EditorViewMode::ThreeD => SceneViewerMode::ThreeD,
    }
}

impl EditorViewMode {
    fn label(self) -> &'static str {
        match self {
            EditorViewMode::Auto => "Auto",
            EditorViewMode::Ui => "UI",
            EditorViewMode::TwoD => "2D",
            EditorViewMode::ThreeD => "3D",
        }
    }
}

fn set_tab_visual<RT: RuntimeAPI + ?Sized>(ctx: &mut RuntimeContext<'_, RT>, id: NodeID, on: bool) {
    if id.is_nil() {
        return;
    }
    let (fill, stroke) = if on {
        (color("#182436"), color("#426384"))
    } else {
        (color("#202733"), color("#323D4B"))
    };
    let _ = with_node_mut!(ctx, UiButton, id, |btn| {
        btn.style.fill = fill;
        btn.style.stroke = stroke;
        btn.style.stroke_width = 1.0;
        btn.style.corner_radius = 3.0;
    });
}

fn set_label<RT: RuntimeAPI + ?Sized>(ctx: &mut RuntimeContext<'_, RT>, id: NodeID, text: &str) {
    if id.is_nil() {
        return;
    }
    let _ = with_node_mut!(ctx, UiLabel, id, |label| {
        label.text = Cow::Owned(text.to_string());
    });
}

fn set_text_block<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    id: NodeID,
    text: &str,
) {
    if id.is_nil() {
        return;
    }
    let mut ok = false;
    let _ = with_node_mut!(ctx, UiTextBlock, id, |block| {
        block.set_text(text.to_string());
        ok = true;
    });
    if ok {
        return;
    }
    let _ = with_node_mut!(ctx, UiLabel, id, |label| {
        label.text = Cow::Owned(text.to_string());
    });
}

fn project_title(project_dir: &str) -> &str {
    if project_dir.is_empty() {
        "No project"
    } else {
        project_dir
    }
}

fn read_main_scene(project_dir: &str) -> Option<String> {
    let text = std::fs::read_to_string(Path::new(project_dir).join("project.toml")).ok()?;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("main_scene") {
            let (_, value) = rest.split_once('=')?;
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn inspector_summary(doc: &perro_scene::SceneDoc) -> String {
    let Some(root) = doc.scene.nodes.first() else {
        return "Empty scene".to_string();
    };
    node_detail(root)
}

impl SceneViewerMode {
    fn label(self) -> &'static str {
        match self {
            SceneViewerMode::Ui => "UI",
            SceneViewerMode::TwoD => "2D",
            SceneViewerMode::ThreeD => "3D",
            SceneViewerMode::Mixed => "Mixed",
            SceneViewerMode::Empty => "Empty",
        }
    }
}

fn scene_viewer_mode(doc: &perro_scene::SceneDoc) -> SceneViewerMode {
    let mut has_ui = false;
    let mut has_2d = false;
    let mut has_3d = false;

    for node in doc.scene.nodes.iter() {
        collect_scene_mode_flags(&node.data, &mut has_ui, &mut has_2d, &mut has_3d);
    }

    match (has_ui, has_2d, has_3d) {
        (false, false, false) => SceneViewerMode::Empty,
        (true, false, false) => SceneViewerMode::Ui,
        (false, true, false) => SceneViewerMode::TwoD,
        (false, false, true) => SceneViewerMode::ThreeD,
        _ => SceneViewerMode::Mixed,
    }
}

fn collect_scene_mode_flags(
    data: &perro_scene::SceneNodeData,
    has_ui: &mut bool,
    has_2d: &mut bool,
    has_3d: &mut bool,
) {
    let ty = data.ty.as_ref();
    if ty.starts_with("Ui") {
        *has_ui = true;
    }
    if ty.ends_with("2D") || ty.contains("2D") {
        *has_2d = true;
    }
    if ty.ends_with("3D") || ty.contains("3D") {
        *has_3d = true;
    }
    if let Some(base) = data.base_ref() {
        collect_scene_mode_flags(base, has_ui, has_2d, has_3d);
    }
}

fn scene_graph_rows(doc: &perro_scene::SceneDoc) -> Vec<(String, String)> {
    let mut by_parent = BTreeMap::<String, Vec<usize>>::new();
    let root_key = doc
        .scene
        .root
        .as_ref()
        .map(|root| root.as_ref().to_string());

    for (index, node) in doc.scene.nodes.iter().enumerate() {
        if let Some(parent) = &node.parent {
            by_parent
                .entry(parent.as_ref().to_string())
                .or_default()
                .push(index);
        }
    }

    let mut rows = Vec::new();
    let mut seen = HashSet::new();
    if let Some(root_key) = root_key {
        if let Some(index) = doc
            .scene
            .nodes
            .iter()
            .position(|node| node.key.as_ref() == root_key)
        {
            push_scene_row(doc, index, 0, &by_parent, &mut seen, &mut rows);
        }
    }
    for index in 0..doc.scene.nodes.len() {
        if !seen.contains(&index) {
            push_scene_row(doc, index, 0, &by_parent, &mut seen, &mut rows);
        }
    }
    rows
}

fn push_scene_row(
    doc: &perro_scene::SceneDoc,
    index: usize,
    depth: usize,
    by_parent: &BTreeMap<String, Vec<usize>>,
    seen: &mut HashSet<usize>,
    rows: &mut Vec<(String, String)>,
) {
    if !seen.insert(index) {
        return;
    }
    let node = &doc.scene.nodes[index];
    let name = node
        .name
        .as_ref()
        .map(|name| name.as_ref())
        .unwrap_or(node.key.as_ref());
    rows.push((
        format!("{}{} : {}", "  ".repeat(depth.min(5)), name, node.data.ty),
        node_detail(node),
    ));
    if let Some(children) = by_parent.get(node.key.as_ref()) {
        for child in children {
            push_scene_row(doc, *child, depth + 1, by_parent, seen, rows);
        }
    }
}

fn node_detail(node: &perro_scene::SceneNodeEntry) -> String {
    let name = node
        .name
        .as_ref()
        .map(|name| name.as_ref())
        .unwrap_or(node.key.as_ref());
    let parent = node.parent.as_ref().map(|p| p.as_ref()).unwrap_or("none");
    let script = node.script.as_ref().map(|s| s.as_ref()).unwrap_or("none");
    let root_of = node.root_of.as_ref().map(|s| s.as_ref()).unwrap_or("none");
    let fields = node
        .data
        .fields
        .iter()
        .map(|(name, value)| format!("  {} = {}", name, scene_value_summary(value)))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "selected: {name}\ntype: {}\nkey: {}\nparent: {parent}\nscript: {script}\nroot_of: {root_of}\nfields:\n{}",
        node.data.ty,
        node.key.as_ref(),
        if fields.is_empty() { "  -" } else { &fields }
    )
}

fn scene_value_summary(value: &perro_scene::SceneValue) -> String {
    match value {
        perro_scene::SceneValue::Bool(v) => v.to_string(),
        perro_scene::SceneValue::I32(v) => v.to_string(),
        perro_scene::SceneValue::F32(v) => v.to_string(),
        perro_scene::SceneValue::Vec2 { x, y } => format!("({x}, {y})"),
        perro_scene::SceneValue::Vec3 { x, y, z } => format!("({x}, {y}, {z})"),
        perro_scene::SceneValue::Vec4 { x, y, z, w } => format!("({x}, {y}, {z}, {w})"),
        perro_scene::SceneValue::Str(v) => format!("\"{v}\""),
        perro_scene::SceneValue::Hashed(v) => v.to_string(),
        perro_scene::SceneValue::Key(v) => v.to_string(),
        perro_scene::SceneValue::Object(fields) => format!("{{{} fields}}", fields.len()),
        perro_scene::SceneValue::Array(items) => format!("[{} items]", items.len()),
    }
}

fn resource_rows(project_dir: &str) -> Vec<String> {
    if project_dir.is_empty() {
        return vec!["res://".to_string()];
    }
    let res_root = Path::new(project_dir).join("res");
    let mut rows = vec!["res://".to_string()];
    collect_resource_rows(&res_root, &res_root, &mut rows);
    rows
}

fn collect_resource_rows(root: &Path, dir: &Path, rows: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries = entries.flatten().map(|entry| entry.path()).collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_resource_rows(root, &path, rows);
            continue;
        }
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        rows.push(format!("res://{}", rel.to_string_lossy().replace('\\', "/")));
    }
}

fn create_scene_rows<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    script_id: NodeID,
    parent: NodeID,
    rows: &[(String, String)],
) {
    if parent.is_nil() {
        return;
    }
    for (index, (label, _)) in rows.iter().take(MAX_SCENE_ROWS).enumerate() {
        let name = format!("scene_node_row_{index}");
        create_button_row(ctx, parent, &name, label, "#213343", "#2C4155", "#EEF6FF");
        let sig = signal!(&format!("{name}_click"));
        let params = [Variant::from(index as u64)];
        signal_connect!(ctx, script_id, sig, func!("on_scene_node_row"), &params);
    }
}

fn create_resource_rows<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    script_id: NodeID,
    parent: NodeID,
    rows: &[String],
) {
    if parent.is_nil() {
        return;
    }
    for (index, path) in rows.iter().take(MAX_RESOURCE_ROWS).enumerate() {
        let name = format!("resource_row_{index}");
        create_button_row(ctx, parent, &name, path, "#1F2834", "#273544", "#9FAABD");
        let sig = signal!(&format!("{name}_click"));
        let params = [Variant::from(index as u64)];
        signal_connect!(ctx, script_id, sig, func!("on_resource_row"), &params);
    }
}

fn create_button_row<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    parent: NodeID,
    name: &str,
    text: &str,
    fill: &str,
    hover: &str,
    text_color: &str,
) -> NodeID {
    let button = create_node!(ctx, UiButton, name.to_string(), tags![], parent);
    let _ = with_node_mut!(ctx, UiButton, button, |node| {
        node.layout.size = UiVector2::pixels(100.0, 24.0);
        node.layout.h_size = UiSizeMode::Fill;
        node.style.fill = color(fill);
        node.style.stroke = color("#323D4B");
        node.style.stroke_width = 1.0;
        node.style.corner_radius = 3.0;
        node.hover_style.fill = color(hover);
        node.hover_style.stroke = color("#7FA4CB");
        node.hover_style.stroke_width = 1.0;
        node.hover_style.corner_radius = 3.0;
        node.pressed_style.fill = color("#11161D");
        node.pressed_style.stroke = color("#7FA4CB");
        node.pressed_style.stroke_width = 1.0;
        node.pressed_style.corner_radius = 3.0;
        node.layout.z_index = 45;
    });
    let label = create_node!(ctx, UiLabel, format!("{name}_label"), tags![], button);
    let _ = with_node_mut!(ctx, UiLabel, label, |node| {
        node.text = Cow::Owned(text.to_string());
        node.font_size = 12.0;
        node.color = color(text_color);
        node.layout.size = UiVector2::ratio(1.0, 1.0);
        node.layout.h_size = UiSizeMode::Fill;
        node.h_align = UiTextAlign::Start;
        node.layout.margin.left = 8.0;
        node.layout.z_index = 46;
    });
    button
}

fn color(hex: &str) -> Color {
    Color::from_hex(hex).unwrap_or(Color::WHITE)
}

fn write_live_scene_doc(doc: &perro_scene::SceneDoc) -> Result<String, String> {
    let mut live_doc = doc.clone();
    let root_key = live_doc.scene.root.clone();
    for node in live_doc.scene.nodes.to_mut() {
        node.script = None;
        node.script_hash = None;
        node.clear_script = true;
        node.root_of = None;
        node.root_of_hash = None;
        if let Some(root_key) = &root_key {
            if node.parent.is_none() && node.key.as_ref() != root_key.as_ref() {
                node.parent = Some(root_key.clone());
            }
        }
    }
    live_doc.normalize_links();
    let path = std::env::temp_dir().join("perro_editor_live_scene.scn");
    std::fs::write(&path, live_doc.to_text()).map_err(|err| err.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

fn apply_live_root<RT: RuntimeAPI + ?Sized>(ctx: &mut RuntimeContext<'_, RT>, root: NodeID) {
    let _ = with_base_node_mut!(ctx, UiBox, root, |node| {
        node.layout.anchor = UiAnchor::Center;
        node.layout.size = UiVector2::ratio(1.0, 1.0);
        node.layout.h_size = UiSizeMode::Fill;
        node.layout.v_size = UiSizeMode::Fill;
        node.transform.position = UiVector2::ratio(0.5, 0.5);
        node.transform.pivot = UiVector2::ratio(0.5, 0.5);
        node.transform.translation = Vector2::ZERO;
        node.transform.rotation = 0.0;
        node.transform.scale = Vector2::ONE;
    });
    disable_ui_node_input(ctx, root);
    for id in query!(ctx, base[UiBox], in_subtree(root)) {
        disable_ui_node_input(ctx, id);
    }
}

fn disable_ui_node_input<RT: RuntimeAPI + ?Sized>(ctx: &mut RuntimeContext<'_, RT>, id: NodeID) {
    let _ = with_base_node_mut!(ctx, UiBox, id, |node| {
        node.input_enabled = false;
        node.mouse_filter = UiMouseFilter::Ignore;
    });
}

fn hide_ui_box<RT: RuntimeAPI + ?Sized>(ctx: &mut RuntimeContext<'_, RT>, id: NodeID) {
    let _ = with_base_node_mut!(ctx, UiBox, id, |node| {
        node.visible = false;
        node.input_enabled = false;
        node.mouse_filter = UiMouseFilter::Ignore;
    });
}

fn show_ui_box<RT: RuntimeAPI + ?Sized>(ctx: &mut RuntimeContext<'_, RT>, id: NodeID) {
    let _ = with_base_node_mut!(ctx, UiBox, id, |node| {
        node.visible = true;
        node.input_enabled = false;
        node.mouse_filter = UiMouseFilter::Ignore;
    });
}

fn inspect_node<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    self_id: NodeID,
    index: usize,
) {
    let (inspector_body, detail) = with_state!(ctx, EditorState, self_id, |state| {
        (
            state.inspector_body,
            state
                .node_details
                .get(index)
                .cloned()
                .unwrap_or_else(|| "No node at slot".to_string()),
        )
    });
    set_text_block(ctx, inspector_body, &detail);
}

fn disable_physics<RT: RuntimeAPI + ?Sized>(ctx: &mut RuntimeContext<'_, RT>, root: NodeID) {
    let ids = query!(
        ctx,
        any(
            is[StaticBody2D, RigidBody2D, Area2D, StaticBody3D, RigidBody3D, Area3D]
        ),
        in_subtree(root)
    );
    for id in ids {
        let _ = with_node_mut!(ctx, StaticBody2D, id, |node| node.enabled = false);
        let _ = with_node_mut!(ctx, RigidBody2D, id, |node| node.enabled = false);
        let _ = with_node_mut!(ctx, Area2D, id, |node| node.enabled = false);
        let _ = with_node_mut!(ctx, StaticBody3D, id, |node| node.enabled = false);
        let _ = with_node_mut!(ctx, RigidBody3D, id, |node| node.enabled = false);
        let _ = with_node_mut!(ctx, Area3D, id, |node| node.enabled = false);
    }
}

fn res_path_to_disk(project_dir: &str, path: &str) -> PathBuf {
    if let Some(rel) = path.strip_prefix("res://") {
        Path::new(project_dir).join("res").join(rel)
    } else {
        Path::new(project_dir).join(path)
    }
}

fn open_code(path: &Path) -> Result<(), String> {
    Command::new("code")
        .arg(path)
        .spawn()
        .or_else(|_| Command::new("code.cmd").arg(path).spawn())
        .map(|_| ())
        .map_err(|err| err.to_string())
}

fn spawn_game(project_dir: &str) -> Result<u32, String> {
    let child = Command::new("cargo")
        .arg("run")
        .arg("--manifest-path")
        .arg(repo_root().join("Cargo.toml"))
        .arg("-p")
        .arg("perro_cli")
        .arg("--")
        .arg("dev")
        .arg("--path")
        .arg(project_dir)
        .spawn()
        .map_err(|err| err.to_string())?;
    Ok(child.id())
}

fn stop_game(pid: u32) -> Result<(), String> {
    let mut cmd = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("taskkill");
        cmd.arg("/PID").arg(pid.to_string()).arg("/T").arg("/F");
        cmd
    } else {
        let mut cmd = Command::new("kill");
        cmd.arg(pid.to_string());
        cmd
    };
    let status = cmd.status().map_err(|err| err.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("exit {status}"))
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
}
