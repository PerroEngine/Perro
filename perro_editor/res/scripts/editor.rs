use perro_api::prelude::*;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

type SelfNodeType = UiPanel;

const ACTIVE_PROJECT: &str = "user://perro_editor_active_project.txt";
const LIVE_VIEWPORT_WIDTH: f32 = 1920.0;
const LIVE_VIEWPORT_HEIGHT: f32 = 1080.0;
const LIVE_VIEWPORT_MIN_SCALE: f32 = 0.33;
const LIVE_VIEWPORT_MAX_SCALE: f32 = 1.5;
const LIVE_VIEWPORT_Z_FALLBACK_PARENT: i32 = 29;
const EDITOR_TOP_BAR_HEIGHT: f32 = 40.0;
const EDITOR_BOTTOM_BAR_HEIGHT: f32 = 26.0;
const EDITOR_WORKSPACE_PADDING_X: f32 = 8.0;
const EDITOR_WORKSPACE_PADDING_Y: f32 = 8.0;
const EDITOR_WORKSPACE_SPACING: f32 = 4.0;
const EDITOR_SIDE_PANEL_WIDTH: f32 = 240.0;
const EDITOR_VIEWPORT_TAB_HEIGHT: f32 = 36.0;
const EDITOR_VIEWPORT_DIVIDER_HEIGHT: f32 = 1.0;
const EDITOR_VIEWPORT_CANVAS_PADDING_X: f32 = 12.0;
const EDITOR_VIEWPORT_CANVAS_PADDING_Y: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SceneViewerMode {
    Ui,
    TwoD,
    ThreeD,
    Mixed,
    Empty,
}

#[State]
#[derive(Clone)]
struct EditorState {
    project_dir: String,
    main_scene: String,
    script_path: String,
    project_label: NodeID,
    scene_label: NodeID,
    viewport_status: NodeID,
    inspector_body: NodeID,
    status_label: NodeID,
    node_details: Vec<String>,
    resource_paths: Vec<String>,
    live_root: NodeID,
    live_scale: f32,
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
        let top_bar = child(ctx, self_id, "top_bar");
        let top_bar_row = child(ctx, top_bar, "top_bar_row");
        let workspace = child(ctx, self_id, "workspace");
        let scene_panel = child(ctx, workspace, "scene_panel");
        let scene_stack = child(ctx, scene_panel, "scene_stack");
        let resource_panel = child(ctx, scene_panel, "resource_panel");
        let resource_stack = child(ctx, resource_panel, "resource_stack");
        let viewport_panel = child(ctx, workspace, "viewport_panel");
        let viewport_vlayout = child(ctx, viewport_panel, "viewport_vlayout");
        let viewport_canvas_wrap = child(ctx, viewport_vlayout, "viewport_canvas_wrap");
        let viewport_canvas = child(ctx, viewport_canvas_wrap, "viewport_canvas");
        let inspector_panel = child(ctx, workspace, "inspector_panel");
        let inspector_stack = child(ctx, inspector_panel, "inspector_stack");
        let bottom_bar = child(ctx, self_id, "bottom_bar");

        let project_label = child(ctx, top_bar_row, "project_label");
        let scene_label = child(ctx, top_bar_row, "scene_label");
        let viewport_status = child(ctx, viewport_panel, "viewport_status");
        let inspector_body = child(ctx, inspector_stack, "inspector_body_text");
        let status_label = child(ctx, bottom_bar, "status_label");

        let project_dir = FileMod::load_string(ACTIVE_PROJECT).unwrap_or_default();
        if !project_dir.trim().is_empty() {
            FileMod::set_project_root_disk(project_dir.trim(), "perro_editor_live_project");
        }
        let main_scene =
            read_main_scene(project_dir.trim()).unwrap_or_else(|| "res://main.scn".to_string());
        let mut script_path = String::new();
        let mut live_root = NodeID::default();
        let mut live_scene_path = String::new();
        let mut node_details = Vec::new();
        let resource_paths = resource_rows(project_dir.trim());
        let mut viewer_mode = SceneViewerMode::Empty;

        set_label(ctx, project_label, project_dir.trim());
        set_label(ctx, scene_label, &format!("Scene: {main_scene}"));
        create_resource_rows(ctx, self_id, resource_stack, &resource_paths);

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
                    .unwrap_or_else(|| "res://scripts/script.rs".to_string());
                viewer_mode = scene_viewer_mode(&doc);
                let summary = format!(
                    "Viewport\nmode: {}\nnodes: {}\nroot: {}\nscript: {}\npreview: {}",
                    viewer_mode.label(),
                    doc.scene.nodes.len(),
                    doc.scene
                        .root
                        .as_ref()
                        .map(|r| r.as_ref())
                        .unwrap_or("none"),
                    script_path,
                    viewer_mode.preview_status(),
                );
                set_text_block(ctx, viewport_status, &summary);
                set_text_block(ctx, inspector_body, &inspector_summary(&doc));
                if viewer_mode == SceneViewerMode::Ui {
                    live_scene_path = write_live_scene_doc(&doc).unwrap_or_default();
                }
            }
            Err(err) => {
                set_text_block(ctx, viewport_status, &format!("Doc load fail\n{err}"));
                create_scene_rows(ctx, self_id, scene_stack, &[]);
            }
        }

        if live_scene_path.is_empty() {
            set_label(
                ctx,
                status_label,
                &format!("Viewport mode {}", viewer_mode.label()),
            );
        } else {
            match scene_load!(ctx, live_scene_path.clone()) {
                Ok(root) => {
                    live_root = root;
                    let _ = reparent!(ctx, viewport_canvas, root);
                    let live_scale = live_viewport_scale(res.viewport_size());
                    apply_live_viewport_transform(ctx, root, live_scale);
                    hide_viewport_status(ctx, viewport_status);
                    disable_physics(ctx, root);
                    set_label(
                        ctx,
                        status_label,
                        "Live edit scene loaded in viewport; scaled; scripts stripped; physics disabled",
                    );
                }
                Err(err) => set_label(ctx, status_label, &format!("Live scene load fail: {err}")),
            }
        }

        let _ = with_state_mut!(ctx, EditorState, self_id, |state| {
            state.project_dir = project_dir.trim().to_string();
            state.main_scene = main_scene;
            state.script_path = script_path;
            state.project_label = project_label;
            state.scene_label = scene_label;
            state.viewport_status = viewport_status;
            state.inspector_body = inspector_body;
            state.status_label = status_label;
            state.node_details = node_details;
            state.resource_paths = resource_paths;
            state.live_root = live_root;
            state.live_scale = live_viewport_scale(res.viewport_size());
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
    }

    fn on_update(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {
        let target_scale = live_viewport_scale(res.viewport_size());
        let mut scale_update = None;
        let _ = with_state_mut!(ctx, EditorState, self_id, |state| {
            if !state.live_root.is_nil() && (state.live_scale - target_scale).abs() > 0.001 {
                scale_update = Some((state.live_root, state.live_scale, target_scale));
                state.live_scale = target_scale;
            }
        });
        if let Some((root, old_scale, new_scale)) = scale_update {
            rescale_live_viewport(ctx, root, old_scale, new_scale);
        }
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
        let path = res_path_to_disk(&project_dir, &script_path);
        match open_code(&path) {
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
        let path = res_path_to_disk(&project_dir, &main_scene);
        match open_code(&path) {
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
            let disk = res_path_to_disk(&project_dir, &path);
            match open_code(&disk) {
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
});

fn child<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    parent: NodeID,
    name: &str,
) -> NodeID {
    if parent.is_nil() {
        return NodeID::default();
    }
    ctx.Nodes()
        .get_child_by_name(parent, name)
        .unwrap_or_default()
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
    let _ = with_node_mut!(ctx, UiTextBlock, id, |block| {
        block.set_text(text.to_string());
    });
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
    let name = root
        .name
        .as_ref()
        .map(|n| n.as_ref())
        .unwrap_or(root.key.as_ref());
    let parent = root.parent.as_ref().map(|p| p.as_ref()).unwrap_or("none");
    let script = root.script.as_ref().map(|s| s.as_ref()).unwrap_or("none");
    format!(
        "selected: {name}\ntype: {}\nkey: {}\nparent: {parent}\nscript: {script}\nfields: {}",
        root.data.ty,
        root.key.as_ref(),
        root.data.fields.len()
    )
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

    fn preview_status(self) -> &'static str {
        match self {
            SceneViewerMode::Ui => "live UI in panel",
            SceneViewerMode::TwoD => "doc preview; scoped 2D target pending",
            SceneViewerMode::ThreeD => "doc preview; scoped 3D target pending",
            SceneViewerMode::Mixed => "doc preview; mixed scoped target pending",
            SceneViewerMode::Empty => "no nodes",
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

    let mut rows = Vec::<(String, String)>::new();
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
        .map(|n| n.as_ref())
        .unwrap_or(node.key.as_ref());
    let indent = "  ".repeat(depth.min(4));
    rows.push((
        format!("{indent}{name} : {}", node.data.ty),
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
        .map(|n| n.as_ref())
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
    let res_root = Path::new(project_dir).join("res");
    let mut rows = vec!["res://".to_string()];
    collect_resource_rows(&res_root, &res_root, &mut rows);
    rows
}

fn collect_resource_rows(root: &Path, dir: &Path, rows: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries = entries
        .flatten()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_resource_rows(root, &path, rows);
            continue;
        }
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        rows.push(format!(
            "res://{}",
            rel.to_string_lossy().replace('\\', "/")
        ));
    }
}

fn create_scene_rows<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    _script_id: NodeID,
    parent: NodeID,
    rows: &[(String, String)],
) {
    if parent.is_nil() {
        return;
    }
    for (index, (label, _)) in rows.iter().enumerate() {
        create_scene_button_row(ctx, parent, &format!("scene_node_row_{index}"), label);
    }
}

fn create_resource_rows<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    _script_id: NodeID,
    parent: NodeID,
    rows: &[String],
) {
    if parent.is_nil() {
        return;
    }
    for (index, path) in rows.iter().enumerate() {
        create_label_row(ctx, parent, &format!("resource_row_{index}"), path);
    }
}

fn create_label_row<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    parent: NodeID,
    name: &str,
    text: &str,
) -> NodeID {
    let label = create_node!(ctx, UiLabel, name.to_string(), tags![], parent);
    let _ = with_node_mut!(ctx, UiLabel, label, |node| {
        node.text = Cow::Owned(text.to_string());
        node.font_size = 16.0;
        node.color = color("#DDE6F2");
        node.layout.size = UiVector2::pixels(100.0, 24.0);
        node.layout.h_size = UiSizeMode::Fill;
        node.h_align = UiTextAlign::Start;
        node.layout.z_index = 43;
    });
    label
}

fn create_scene_button_row<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    parent: NodeID,
    name: &str,
    text: &str,
) -> NodeID {
    let button = create_node!(ctx, UiButton, name.to_string(), tags![], parent);
    let _ = with_node_mut!(ctx, UiButton, button, |node| {
        node.layout.size = UiVector2::pixels(100.0, 34.0);
        node.layout.h_size = UiSizeMode::Fill;
        node.style.fill = color("#2C4155");
        node.hover_style.fill = color("#36506A");
        node.pressed_style.fill = color("#213343");
        node.style.stroke = color("#739ABD");
        node.style.corner_radius = 4.0;
        node.layout.z_index = 43;
    });
    let label_name = format!("{name}_label");
    let lbl = create_node!(ctx, UiLabel, label_name, tags![], button);
    let _ = with_node_mut!(ctx, UiLabel, lbl, |node| {
        node.text = Cow::Owned(text.to_string());
        node.font_size = 14.0;
        node.color = color("#EEF6FF");
        node.layout.size = UiVector2::ratio(1.0, 1.0);
        node.layout.h_size = UiSizeMode::Fill;
        node.h_align = UiTextAlign::Start;
        node.layout.margin.left = 6.0;
        node.layout.z_index = 44;
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

fn live_viewport_scale(viewport_size: Vector2) -> f32 {
    let canvas_width = viewport_size.x
        - EDITOR_WORKSPACE_PADDING_X
        - EDITOR_SIDE_PANEL_WIDTH * 2.0
        - EDITOR_WORKSPACE_SPACING * 2.0
        - EDITOR_VIEWPORT_CANVAS_PADDING_X;
    let canvas_height = viewport_size.y
        - EDITOR_TOP_BAR_HEIGHT
        - EDITOR_BOTTOM_BAR_HEIGHT
        - EDITOR_WORKSPACE_PADDING_Y
        - EDITOR_VIEWPORT_TAB_HEIGHT
        - EDITOR_VIEWPORT_DIVIDER_HEIGHT
        - EDITOR_VIEWPORT_CANVAS_PADDING_Y;
    let fit_scale = (canvas_width / LIVE_VIEWPORT_WIDTH).min(canvas_height / LIVE_VIEWPORT_HEIGHT);
    fit_scale.clamp(LIVE_VIEWPORT_MIN_SCALE, LIVE_VIEWPORT_MAX_SCALE)
}

fn apply_live_viewport_transform<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    root: NodeID,
    scale: f32,
) {
    let root_parent_z = get_node_parent_id!(ctx, root)
        .and_then(|parent| ui_z_index(ctx, parent))
        .unwrap_or(LIVE_VIEWPORT_Z_FALLBACK_PARENT);

    apply_ui_root_fit::<RT, UiPanel>(ctx, root, scale);
    apply_ui_root_fit::<RT, UiButton>(ctx, root, scale);
    apply_ui_root_fit::<RT, UiLabel>(ctx, root, scale);
    apply_ui_root_fit::<RT, UiTextBox>(ctx, root, scale);
    apply_ui_root_fit::<RT, UiTextBlock>(ctx, root, scale);
    apply_ui_root_fit::<RT, UiLayout>(ctx, root, scale);
    apply_ui_root_fit::<RT, UiHLayout>(ctx, root, scale);
    apply_ui_root_fit::<RT, UiVLayout>(ctx, root, scale);
    apply_ui_root_fit::<RT, UiGrid>(ctx, root, scale);

    let nodes = query!(
        ctx,
        any(is[
            UiPanel,
            UiButton,
            UiLabel,
            UiTextBox,
            UiTextBlock,
            UiLayout,
            UiHLayout,
            UiVLayout,
            UiGrid
        ]),
        in_subtree(root)
    );
    for id in nodes {
        if id == root {
            continue;
        }
        apply_ui_node_scale::<RT, UiPanel>(ctx, id, scale);
        apply_ui_node_scale::<RT, UiButton>(ctx, id, scale);
        apply_ui_node_scale::<RT, UiLabel>(ctx, id, scale);
        apply_ui_node_scale::<RT, UiTextBox>(ctx, id, scale);
        apply_ui_node_scale::<RT, UiTextBlock>(ctx, id, scale);
        apply_ui_node_scale::<RT, UiLayout>(ctx, id, scale);
        apply_ui_node_scale::<RT, UiHLayout>(ctx, id, scale);
        apply_ui_node_scale::<RT, UiVLayout>(ctx, id, scale);
        apply_ui_node_scale::<RT, UiGrid>(ctx, id, scale);
        apply_label_scale(ctx, id, scale);
        apply_text_edit_scale::<RT, UiTextBox>(ctx, id, scale);
        apply_text_edit_scale::<RT, UiTextBlock>(ctx, id, scale);
        apply_layout_spacing_scale(ctx, id, scale);
    }

    apply_live_viewport_z_tree(ctx, root, root_parent_z);
}

fn apply_ui_root_fit<RT, T>(ctx: &mut RuntimeContext<'_, RT>, root: NodeID, scale: f32)
where
    RT: RuntimeAPI + ?Sized,
    T: UiNodeBase + NodeTypeDispatch + 'static,
{
    let _ = with_node_mut!(ctx, T, root, |node| {
        let base = node.ui_base_mut();
        base.layout.anchor = UiAnchor::Center;
        base.layout.size = UiVector2::pixels(LIVE_VIEWPORT_WIDTH, LIVE_VIEWPORT_HEIGHT);
        base.transform.position = UiVector2::ratio(0.5, 0.5);
        base.transform.pivot = UiVector2::ratio(0.5, 0.5);
        base.transform.scale = Vector2::new(scale, scale);
        base.input_enabled = false;
        base.mouse_filter = UiMouseFilter::Ignore;
    });
}

fn rescale_live_viewport<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    root: NodeID,
    old_scale: f32,
    new_scale: f32,
) {
    let ratio = new_scale / old_scale.max(0.0001);
    set_ui_root_scale::<RT, UiPanel>(ctx, root, new_scale);
    set_ui_root_scale::<RT, UiButton>(ctx, root, new_scale);
    set_ui_root_scale::<RT, UiLabel>(ctx, root, new_scale);
    set_ui_root_scale::<RT, UiTextBox>(ctx, root, new_scale);
    set_ui_root_scale::<RT, UiTextBlock>(ctx, root, new_scale);
    set_ui_root_scale::<RT, UiLayout>(ctx, root, new_scale);
    set_ui_root_scale::<RT, UiHLayout>(ctx, root, new_scale);
    set_ui_root_scale::<RT, UiVLayout>(ctx, root, new_scale);
    set_ui_root_scale::<RT, UiGrid>(ctx, root, new_scale);

    let nodes = query!(
        ctx,
        any(is[
            UiPanel,
            UiButton,
            UiLabel,
            UiTextBox,
            UiTextBlock,
            UiLayout,
            UiHLayout,
            UiVLayout,
            UiGrid
        ]),
        in_subtree(root)
    );
    for id in nodes {
        if id == root {
            continue;
        }
        apply_ui_node_scale::<RT, UiPanel>(ctx, id, ratio);
        apply_ui_node_scale::<RT, UiButton>(ctx, id, ratio);
        apply_ui_node_scale::<RT, UiLabel>(ctx, id, ratio);
        apply_ui_node_scale::<RT, UiTextBox>(ctx, id, ratio);
        apply_ui_node_scale::<RT, UiTextBlock>(ctx, id, ratio);
        apply_ui_node_scale::<RT, UiLayout>(ctx, id, ratio);
        apply_ui_node_scale::<RT, UiHLayout>(ctx, id, ratio);
        apply_ui_node_scale::<RT, UiVLayout>(ctx, id, ratio);
        apply_ui_node_scale::<RT, UiGrid>(ctx, id, ratio);
        apply_label_scale(ctx, id, ratio);
        apply_text_edit_scale::<RT, UiTextBox>(ctx, id, ratio);
        apply_text_edit_scale::<RT, UiTextBlock>(ctx, id, ratio);
        apply_layout_spacing_scale(ctx, id, ratio);
    }
}

fn set_ui_root_scale<RT, T>(ctx: &mut RuntimeContext<'_, RT>, root: NodeID, scale: f32)
where
    RT: RuntimeAPI + ?Sized,
    T: UiNodeBase + NodeTypeDispatch + 'static,
{
    let _ = with_node_mut!(ctx, T, root, |node| {
        let base = node.ui_base_mut();
        base.layout.anchor = UiAnchor::Center;
        base.transform.position = UiVector2::ratio(0.5, 0.5);
        base.transform.pivot = UiVector2::ratio(0.5, 0.5);
        base.transform.scale = Vector2::new(scale, scale);
    });
}

fn apply_ui_node_scale<RT, T>(ctx: &mut RuntimeContext<'_, RT>, id: NodeID, scale: f32)
where
    RT: RuntimeAPI + ?Sized,
    T: UiNodeBase + NodeTypeDispatch + 'static,
{
    let _ = with_node_mut!(ctx, T, id, |node| {
        let base = node.ui_base_mut();
        base.input_enabled = false;
        base.mouse_filter = UiMouseFilter::Ignore;
        scale_ui_vector2(&mut base.layout.size, scale);
        base.layout.min_size *= scale;
        base.layout.max_size *= scale;
        base.transform.translation *= scale;
        base.layout.margin.left *= scale;
        base.layout.margin.top *= scale;
        base.layout.margin.right *= scale;
        base.layout.margin.bottom *= scale;
        base.layout.padding.left *= scale;
        base.layout.padding.top *= scale;
        base.layout.padding.right *= scale;
        base.layout.padding.bottom *= scale;
    });
}

fn apply_live_viewport_z_tree<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    node: NodeID,
    parent_z: i32,
) {
    let own_z = ui_z_index(ctx, node).unwrap_or(0);
    let z = parent_z.saturating_add(1).saturating_add(own_z);
    set_ui_z_index(ctx, node, z);
    for child in get_children!(ctx, node) {
        apply_live_viewport_z_tree(ctx, child, z);
    }
}

fn ui_z_index<RT: RuntimeAPI + ?Sized>(ctx: &mut RuntimeContext<'_, RT>, id: NodeID) -> Option<i32> {
    with_node!(ctx, UiPanel, id, |node| Some(node.ui_base().layout.z_index))
        .or_else(|| with_node!(ctx, UiButton, id, |node| Some(node.ui_base().layout.z_index)))
        .or_else(|| with_node!(ctx, UiLabel, id, |node| Some(node.ui_base().layout.z_index)))
        .or_else(|| with_node!(ctx, UiTextBox, id, |node| Some(node.ui_base().layout.z_index)))
        .or_else(|| with_node!(ctx, UiTextBlock, id, |node| Some(node.ui_base().layout.z_index)))
        .or_else(|| with_node!(ctx, UiLayout, id, |node| Some(node.ui_base().layout.z_index)))
        .or_else(|| with_node!(ctx, UiHLayout, id, |node| Some(node.ui_base().layout.z_index)))
        .or_else(|| with_node!(ctx, UiVLayout, id, |node| Some(node.ui_base().layout.z_index)))
        .or_else(|| with_node!(ctx, UiGrid, id, |node| Some(node.ui_base().layout.z_index)))
}

fn set_ui_z_index<RT: RuntimeAPI + ?Sized>(ctx: &mut RuntimeContext<'_, RT>, id: NodeID, z: i32) {
    let _ = with_node_mut!(ctx, UiPanel, id, |node| node.ui_base_mut().layout.z_index = z);
    let _ = with_node_mut!(ctx, UiButton, id, |node| node.ui_base_mut().layout.z_index = z);
    let _ = with_node_mut!(ctx, UiLabel, id, |node| node.ui_base_mut().layout.z_index = z);
    let _ = with_node_mut!(ctx, UiTextBox, id, |node| node.ui_base_mut().layout.z_index = z);
    let _ = with_node_mut!(ctx, UiTextBlock, id, |node| node.ui_base_mut().layout.z_index = z);
    let _ = with_node_mut!(ctx, UiLayout, id, |node| node.ui_base_mut().layout.z_index = z);
    let _ = with_node_mut!(ctx, UiHLayout, id, |node| node.ui_base_mut().layout.z_index = z);
    let _ = with_node_mut!(ctx, UiVLayout, id, |node| node.ui_base_mut().layout.z_index = z);
    let _ = with_node_mut!(ctx, UiGrid, id, |node| node.ui_base_mut().layout.z_index = z);
}

fn scale_ui_vector2(value: &mut UiVector2, scale: f32) {
    scale_ui_unit(&mut value.x, scale);
    scale_ui_unit(&mut value.y, scale);
}

fn scale_ui_unit(value: &mut UiUnit, scale: f32) {
    if let UiUnit::Pixels(px) = value {
        *px *= scale;
    }
}

fn apply_label_scale<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    id: NodeID,
    scale: f32,
) {
    let _ = with_node_mut!(ctx, UiLabel, id, |node| {
        node.font_size *= scale;
    });
}

fn apply_text_edit_scale<RT, T>(ctx: &mut RuntimeContext<'_, RT>, id: NodeID, scale: f32)
where
    RT: RuntimeAPI + ?Sized,
    T: std::ops::DerefMut<Target = UiTextEdit> + NodeTypeDispatch + 'static,
{
    let _ = with_node_mut!(ctx, T, id, |node| {
        node.font_size *= scale;
        node.padding.left *= scale;
        node.padding.top *= scale;
        node.padding.right *= scale;
        node.padding.bottom *= scale;
    });
}

fn apply_layout_spacing_scale<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    id: NodeID,
    scale: f32,
) {
    let _ = with_node_mut!(ctx, UiLayout, id, |node| {
        node.inner.spacing *= scale;
        node.inner.h_spacing *= scale;
        node.inner.v_spacing *= scale;
    });
    let _ = with_node_mut!(ctx, UiHLayout, id, |node| {
        node.inner.spacing *= scale;
        node.inner.h_spacing *= scale;
        node.inner.v_spacing *= scale;
    });
    let _ = with_node_mut!(ctx, UiVLayout, id, |node| {
        node.inner.spacing *= scale;
        node.inner.h_spacing *= scale;
        node.inner.v_spacing *= scale;
    });
    let _ = with_node_mut!(ctx, UiGrid, id, |node| {
        node.h_spacing *= scale;
        node.v_spacing *= scale;
    });
}

fn hide_viewport_status<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    viewport_status: NodeID,
) {
    let _ = with_node_mut!(ctx, UiTextBlock, viewport_status, |node| {
        node.inner.base.visible = false;
        node.inner.base.input_enabled = false;
        node.inner.base.mouse_filter = UiMouseFilter::Ignore;
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
