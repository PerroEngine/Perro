use perro_api::prelude::*;
use perro_api::scene::{
    SceneDoc, SceneFieldName, SceneKey, SceneNodeData, SceneNodeEntry, SceneValue,
};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::str::FromStr;

mod editor_project;

type SelfNodeType = UiPanel;

const MAX_FILES: usize = 12;
const MAX_NODES: usize = 4;
const MAX_TABS: usize = 2;
const MAX_RECENT: usize = 5;
const MAX_NODE_PICKER_ROWS: usize = 12;
const RECENT_PROJECTS_PATH: &str = "user://recent_projects.json";

#[State]
struct EditorState {
    project_root: String,
    project_name: String,
    create_parent_dir: String,
    recent_projects: Vec<String>,
    file_paths: Vec<String>,
    scene_paths: Vec<String>,
    open_paths: Vec<String>,
    active_open: usize,
    doc_text: String,
    selected_key: Option<u32>,
    viewport_mode: String,
    dirty: bool,
    node_picker_offset: usize,
    cam_x: f32,
    cam_y: f32,
    cam_z: f32,
    cam_yaw: f32,
    cam_pitch: f32,
    log: String,
}

lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        connect_editor_signals(ctx);

        let recent = load_recent_projects();
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            state.recent_projects = recent;
            state.log = "project manager".to_string();
        });
        refresh_all(ctx);
        set_project_manager(ctx, true);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        update_freecam(ctx);
    }
});

methods!({
    fn on_editor_signal(&self, ctx: &mut ScriptContext<'_, API>, sender: NodeID) {
        let Some(name) = get_node_name!(ctx.run, sender).map(|v| v.to_string()) else {
            return;
        };

        match name.as_str() {
            "open_project_button" => {
                refresh_recent_projects(ctx);
                set_project_manager(ctx, true);
            }
            "manager_browse_button" => {
                open_project_dialog(ctx);
            }
            "manager_choose_location_button" => {
                choose_create_location(ctx);
            }
            "manager_create_button" => {
                create_project_from_manager(ctx);
            }
            "manager_close_button" => {
                set_project_manager(ctx, false);
            }
            "save_scene_button" => {
                save_active_scene(ctx);
            }
            "add_node_button" => {
                let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                    state.node_picker_offset = 0;
                });
                refresh_all(ctx);
                set_add_node_popup(ctx, true);
            }
            "add_node_cancel_button" => set_add_node_popup(ctx, false),
            "add_node_prev_button" => {
                shift_node_picker(ctx, -1);
            }
            "add_node_next_button" => {
                shift_node_picker(ctx, 1);
            }
            "mode_ui_button" => set_mode(ctx, "UI"),
            "mode_2d_button" => set_mode(ctx, "2D"),
            "mode_3d_button" => set_mode(ctx, "3D"),
            _ => {
                if let Some(idx) = suffix_index(&name, "file_row_") {
                    open_file_slot(ctx, idx);
                } else if let Some(idx) = suffix_index(&name, "manager_recent_") {
                    open_recent_project(ctx, idx);
                } else if let Some(idx) = suffix_index(&name, "add_node_type_") {
                    add_node_from_picker(ctx, idx);
                } else if let Some(idx) = suffix_index(&name, "scene_row_") {
                    select_node_slot(ctx, idx);
                } else if let Some(idx) = suffix_index(&name, "scene_tab_") {
                    set_active_tab(ctx, idx);
                }
            }
        }
    }
});

fn connect_editor_signals<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_open_project"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_manager_browse"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_manager_choose_location"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_manager_create"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_manager_close"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_recent_0"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_recent_1"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_recent_2"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_recent_3"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_recent_4"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_save_scene"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_add_node"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_mode_ui"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_mode_2d"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_mode_3d"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_open_file_0"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_open_file_1"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_open_file_2"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_open_file_3"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_open_file_4"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_open_file_5"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_open_file_6"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_open_file_7"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_open_file_8"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_open_file_9"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_open_file_10"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_open_file_11"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_select_scene_0"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_select_scene_1"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_select_scene_2"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_select_scene_3"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_tab_0"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_tab_1"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_add_type_0"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_add_type_1"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_add_type_2"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_add_type_3"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_add_type_4"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_add_type_5"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_add_type_6"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_add_type_7"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_add_type_8"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_add_type_9"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_add_type_10"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_add_type_11"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_add_type_prev"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_add_type_next"), func!("on_editor_signal"));
    let _ = signal_connect!(ctx.run, ctx.id, signal!("editor_add_node_cancel"), func!("on_editor_signal"));
}

fn update_freecam<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.viewport_mode.clone()
    });
    if mode != "3D" {
        return;
    }

    let dt = delta_time!(ctx.run).clamp(0.0, 1.0 / 30.0);
    let mut dx = 0.0;
    let mut dy = 0.0;
    let mut dz = 0.0;
    if key_down!(ctx.ipt, KeyCode::KeyW) {
        dz -= 1.0;
    }
    if key_down!(ctx.ipt, KeyCode::KeyS) {
        dz += 1.0;
    }
    if key_down!(ctx.ipt, KeyCode::KeyA) {
        dx -= 1.0;
    }
    if key_down!(ctx.ipt, KeyCode::KeyD) {
        dx += 1.0;
    }
    if key_down!(ctx.ipt, KeyCode::Space) {
        dy += 1.0;
    }
    if key_down!(ctx.ipt, KeyCode::ShiftLeft) || key_down!(ctx.ipt, KeyCode::ShiftRight) {
        dy -= 1.0;
    }

    let mouse = if mouse_down!(ctx.ipt, MouseButton::Middle) {
        mouse_delta!(ctx.ipt)
    } else {
        Vector2::ZERO
    };

    let mut label = String::new();
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let speed = if key_down!(ctx.ipt, KeyCode::ControlLeft) { 18.0 } else { 7.0 };
        state.cam_x += dx * speed * dt;
        state.cam_y += dy * speed * dt;
        state.cam_z += dz * speed * dt;
        state.cam_yaw += mouse.x * 0.0025;
        state.cam_pitch = (state.cam_pitch - mouse.y * 0.0025).clamp(-1.4, 1.4);
        label = format!(
            "Viewport  mode={}  cam=({:.1}, {:.1}, {:.1}) yaw={:.2} pitch={:.2}",
            state.viewport_mode, state.cam_x, state.cam_y, state.cam_z, state.cam_yaw, state.cam_pitch
        );
    });
    set_label(ctx, "viewport_label", &label);
}

fn open_project<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root: String,
) -> Result<(), String> {
    let root_path = PathBuf::from(&root);
    validate_project_root(&root_path)?;
    let project_text = FileMod::load_string(root_path.join("project.toml").to_string_lossy().as_ref())
        .map_err(|err| err.to_string())?;
    let project_name = parse_project_name(&project_text).unwrap_or_else(|| {
        root_path
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("Perro Project")
            .to_string()
    });
    let file_paths = scan_res_paths(&root_path)?;
    let scene_paths = file_paths
        .iter()
        .filter(|path| path.ends_with(".scn"))
        .cloned()
        .collect::<Vec<_>>();
    let log = format!(
        "open project\nroot: {}\nscenes: {}",
        root_path.display(),
        scene_paths.len()
    );

    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.project_root = root_path.to_string_lossy().to_string();
        state.project_name = project_name;
        state.file_paths = file_paths;
        state.scene_paths = scene_paths;
        state.open_paths.clear();
        state.active_open = 0;
        state.doc_text.clear();
        state.selected_key = None;
        state.dirty = false;
        state.log = log;
    });

    add_recent_project(ctx, root_path.to_string_lossy().as_ref());
    set_project_manager(ctx, false);
    refresh_all(ctx);
    open_first_scene(ctx);
    Ok(())
}

fn open_project_dialog<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    if let Some(path) = FileMod::pick_folder("Open Perro Project") {
        if let Err(err) = open_project(ctx, path.clone()) {
            set_log(ctx, &format!("open project fail\n{path}\n{err}"));
            refresh_recent_projects(ctx);
            set_project_manager(ctx, true);
        }
    }
}

fn choose_create_location<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    if let Some(path) = FileMod::pick_folder("Choose Project Location") {
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            state.create_parent_dir = path.clone();
            state.log = format!("create location\n{path}");
        });
        refresh_all(ctx);
    }
}

fn create_project_from_manager<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let parent = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.create_parent_dir.clone()
    });
    if parent.trim().is_empty() {
        set_log(ctx, "create project fail\npick location first");
        return;
    }

    let Some(name) = read_text_box(ctx, "create_name_box") else {
        set_log(ctx, "create project fail\nmissing project name box");
        return;
    };
    let name = name.trim().to_string();
    if name.is_empty() {
        set_log(ctx, "create project fail\nname empty");
        return;
    }

    match editor_project::create_project(parent.as_str(), name.as_str()) {
        Ok(root) => {
            if let Err(err) = open_project(ctx, root.clone()) {
                set_log(ctx, &format!("create ok, open fail\n{root}\n{err}"));
                refresh_recent_projects(ctx);
                set_project_manager(ctx, true);
            }
        }
        Err(err) => {
            set_log(ctx, &format!("create project fail\n{parent}\n{name}\n{err}"));
            refresh_all(ctx);
        }
    }
}

fn open_recent_project<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, idx: usize) {
    let path = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.recent_projects.get(idx).cloned()
    });
    let Some(path) = path else {
        return;
    };
    if let Err(err) = open_project(ctx, path.clone()) {
        set_log(ctx, &format!("open recent fail\n{path}\n{err}"));
        refresh_recent_projects(ctx);
        set_project_manager(ctx, true);
    }
}

fn refresh_recent_projects<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let recent = load_recent_projects();
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.recent_projects = recent;
    });
    refresh_all(ctx);
}

fn add_recent_project<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, root: &str) {
    let mut recent = load_recent_projects();
    recent.retain(|item| item != root);
    recent.insert(0, root.to_string());
    recent.truncate(MAX_RECENT);
    save_recent_projects(&recent);
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.recent_projects = recent;
    });
}

fn validate_project_root(root: &Path) -> Result<(), String> {
    if !root.join(".perro").is_dir() {
        return Err("missing .perro dir".to_string());
    }
    if !root.join("project.toml").is_file() {
        return Err("missing project.toml".to_string());
    }
    Ok(())
}

fn scan_res_paths(root: &Path) -> Result<Vec<String>, String> {
    let res = root.join("res");
    if !res.is_dir() {
        return Ok(Vec::new());
    }
    let files = FileMod::walk_dir(res.to_string_lossy().as_ref()).map_err(|err| err.to_string())?;
    let mut out = files
        .into_iter()
        .filter_map(|path| {
            let abs = Path::new(&path);
            let mut res_path = abs_to_res(root, abs)?;
            if abs.is_dir() {
                res_path.push('/');
            }
            Some(res_path)
        })
        .collect::<Vec<_>>();
    out.sort();
    Ok(out)
}

fn open_file_slot<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, idx: usize) {
    let (root, res_path) = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (state.project_root.clone(), state.file_paths.get(idx).cloned())
    });
    let Some(scene_path) = res_path else {
        return;
    };
    if scene_path.ends_with('/') {
        set_log(ctx, &format!("folder\n{scene_path}"));
        return;
    }
    if !scene_path.ends_with(".scn") {
        set_log(ctx, &format!("file\n{scene_path}"));
        return;
    }
    let abs = res_to_abs(&root, &scene_path);
    let text = match FileMod::load_string(&abs) {
        Ok(text) => text,
        Err(err) => {
            set_log(ctx, &format!("open scene fail\n{scene_path}\n{err}"));
            return;
        }
    };
    let doc = SceneDoc::parse(&text);
    let first_key = doc.scene.nodes.first().map(|node| node.key.as_u32());
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if !state.open_paths.iter().any(|path| path == &scene_path) {
            state.open_paths.push(scene_path.clone());
        }
        state.active_open = state
            .open_paths
            .iter()
            .position(|path| path == &scene_path)
            .unwrap_or(0);
        state.doc_text = doc.to_text();
        state.selected_key = first_key;
        state.dirty = false;
        state.log = format!("open scene\n{scene_path}");
    });
    refresh_all(ctx);
}

fn set_active_tab<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, idx: usize) {
    let path = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.open_paths.get(idx).cloned()
    });
    let Some(path) = path else {
        return;
    };
    let slot = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.file_paths.iter().position(|item| item == &path)
    });
    if let Some(slot) = slot {
        open_file_slot(ctx, slot);
    }
}

fn open_first_scene<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let slot = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.file_paths.iter().position(|path| path.ends_with(".scn"))
    });
    if let Some(slot) = slot {
        open_file_slot(ctx, slot);
    }
}

fn select_node_slot<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, idx: usize) {
    let key = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.doc_text.is_empty() {
            None
        } else {
            let doc = SceneDoc::parse(&state.doc_text);
            scene_tree_view(&doc, state.selected_key)
                .keys
                .get(idx)
                .copied()
        }
    });
    if let Some(key) = key {
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            state.selected_key = Some(key);
        });
        refresh_all(ctx);
    }
}

fn add_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, node_type_name: &str) {
    set_add_node_popup(ctx, false);
    let Ok(node_type) = perro_scene::NodeType::from_str(node_type_name) else {
        set_log(ctx, &format!("add node fail\nbad type: {node_type_name}"));
        return;
    };
    let mut msg = "no open scene".to_string();
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.doc_text.is_empty() {
            return;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let next_id = doc.scene.key_names.len() as u32;
        let key = SceneKey::new(next_id);
        let name = unique_node_name(&doc, node_type.name());
        let parent = state.selected_key.map(SceneKey::new).or(doc.scene.root);
        let node = SceneNodeEntry {
            data: SceneNodeData::new(node_type, Cow::Owned(default_fields(node_type)), None),
            has_data_override: true,
            key,
            name: None,
            tags: Cow::Owned(Vec::new()),
            children: Cow::Owned(Vec::new()),
            parent,
            script: None,
            clear_script: false,
            root_of: None,
            script_vars: Cow::Owned(Vec::new()),
        };
        doc.scene.key_names.to_mut().push(Cow::Owned(name.clone()));
        doc.scene.nodes.to_mut().push(node);
        doc.normalize_links();
        state.doc_text = doc.to_text();
        state.selected_key = Some(next_id);
        state.dirty = true;
        state.log = format!("add node\n{name}: {}", node_type.name());
        msg = state.log.clone();
    });
    set_log(ctx, &msg);
    refresh_all(ctx);
}

fn add_node_from_picker<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, row: usize) {
    let node_type = with_state!(ctx.run, EditorState, ctx.id, |state| {
        picker_node_type(state.node_picker_offset + row)
    });
    if let Some(node_type) = node_type {
        add_node(ctx, node_type.name());
    }
}

fn shift_node_picker<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, dir: isize) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let max_start = perro_scene::NodeType::ALL
            .len()
            .saturating_sub(MAX_NODE_PICKER_ROWS);
        if dir < 0 {
            state.node_picker_offset = state.node_picker_offset.saturating_sub(MAX_NODE_PICKER_ROWS);
        } else {
            state.node_picker_offset =
                (state.node_picker_offset + MAX_NODE_PICKER_ROWS).min(max_start);
        }
    });
    refresh_all(ctx);
}

fn picker_node_type(idx: usize) -> Option<perro_scene::NodeType> {
    perro_scene::NodeType::ALL.get(idx).copied()
}

fn default_fields(node_type: perro_scene::NodeType) -> Vec<(SceneFieldName, SceneValue)> {
    let mut fields = Vec::new();
    if node_type.is_a(perro_scene::NodeType::Node3D) {
        fields.push((
            SceneFieldName::Position,
            SceneValue::Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
        ));
    } else if node_type.is_a(perro_scene::NodeType::Node2D) {
        fields.push((SceneFieldName::Position, SceneValue::Vec2 { x: 0.0, y: 0.0 }));
    } else if node_type.is_a(perro_scene::NodeType::UiBox) {
        fields.push((SceneFieldName::Anchor, SceneValue::Str(Cow::Borrowed("center"))));
        fields.push((SceneFieldName::SizeRatio, SceneValue::Vec2 { x: 0.20, y: 0.12 }));
    }

    if node_type == perro_scene::NodeType::UiLabel || node_type == perro_scene::NodeType::UiButton {
        fields.push((SceneFieldName::Text, SceneValue::Str(Cow::Borrowed("New Node"))));
    }
    fields
}

fn save_active_scene<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let save = with_state!(ctx.run, EditorState, ctx.id, |state| {
        let path = state.open_paths.get(state.active_open).cloned();
        let root = state.project_root.clone();
        let doc_text = state.doc_text.clone();
        (root, path, doc_text)
    });
    let (root, Some(path), doc_text) = save else {
        set_log(ctx, "save fail\nno open scene");
        return;
    };
    if doc_text.is_empty() {
        set_log(ctx, "save fail\nno open scene");
        return;
    }
    let mut doc = SceneDoc::parse(&doc_text);
    doc.normalize_links();
    let text = doc.to_text();
    let abs = res_to_abs(&root, &path);
    match FileMod::save_string(&abs, &text) {
        Ok(_) => {
            let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                state.dirty = false;
                state.log = format!("save scene\n{path}");
            });
        }
        Err(err) => {
            let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                state.log = format!("save fail\n{path}\n{err}");
            });
        }
    }
    refresh_all(ctx);
}

fn set_mode<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, mode: &str) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.viewport_mode = mode.to_string();
        state.log = format!("mode {mode}");
    });
    refresh_all(ctx);
}

fn refresh_all<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let view = with_state!(ctx.run, EditorState, ctx.id, |state| EditorView::from_state(state));

    set_label(
        ctx,
        "project_status",
        &format!("{}  {}", view.project_name, view.project_root),
    );
    set_label(ctx, "status_bar", &view.status);
    set_label(ctx, "log_text", &view.log);
    set_label(ctx, "viewport_label", &view.viewport);

    for idx in 0..MAX_RECENT {
        let text = view
            .recent_projects
            .get(idx)
            .cloned()
            .unwrap_or_else(|| "-".to_string());
        set_label(ctx, &format!("manager_recent_{idx}_label"), &short_path(&text, 44));
    }
    set_label(
        ctx,
        "create_location_label",
        &format!("location: {}", short_path(&view.create_parent_dir, 34)),
    );

    set_label(ctx, "add_node_page_label", &view.node_picker_page);
    for idx in 0..MAX_NODE_PICKER_ROWS {
        let text = view
            .node_picker_rows
            .get(idx)
            .cloned()
            .unwrap_or_else(|| "-".to_string());
        set_label(ctx, &format!("add_node_type_{idx}_label"), &text);
    }

    for idx in 0..MAX_FILES {
        let text = view
            .file_paths
            .get(idx)
            .cloned()
            .unwrap_or_else(|| "-".to_string());
        set_label(ctx, &format!("file_row_{idx}_label"), &short_path(&text, 28));
    }

    for idx in 0..MAX_TABS {
        let text = view
            .open_paths
            .get(idx)
            .cloned()
            .unwrap_or_else(|| "-".to_string());
        set_label(ctx, &format!("scene_tab_{idx}_label"), &short_path(&text, 24));
        set_button_fill(
            ctx,
            &format!("scene_tab_{idx}"),
            if idx == view.active_open { "#54657A" } else { "#3D4654" },
        );
    }

    for idx in 0..MAX_NODES {
        let text = view
            .nodes
            .get(idx)
            .cloned()
            .unwrap_or_else(|| "-".to_string());
        set_label(ctx, &format!("scene_row_{idx}_label"), &text);
        set_button_fill(
            ctx,
            &format!("scene_row_{idx}"),
            if view.selected_row == Some(idx) { "#54657A" } else { "#39414E" },
        );
    }
    apply_scene_tree_layout(ctx, &view.scene_roots, &view.scene_branches);

    set_label(ctx, "inspector_name", &format!("name: {}", view.inspector_name));
    set_label(ctx, "inspector_type", &format!("type: {}", view.inspector_type));
    set_label(ctx, "inspector_parent", &format!("parent: {}", view.inspector_parent));
    set_label(ctx, "inspector_pos", &format!("pos: {}", view.inspector_pos));
    set_label(ctx, "inspector_script", &format!("script: {}", view.inspector_script));
    set_label(ctx, "inspector_vars", &format!("script vars: {}", view.inspector_vars));
}

#[derive(Default)]
struct EditorView {
    project_root: String,
    project_name: String,
    create_parent_dir: String,
    recent_projects: Vec<String>,
    file_paths: Vec<String>,
    scene_paths: Vec<String>,
    open_paths: Vec<String>,
    active_open: usize,
    nodes: Vec<String>,
    scene_roots: Vec<usize>,
    scene_branches: Vec<(usize, Vec<usize>)>,
    selected_row: Option<usize>,
    inspector_name: String,
    inspector_type: String,
    inspector_parent: String,
    inspector_pos: String,
    inspector_script: String,
    inspector_vars: String,
    viewport: String,
    status: String,
    log: String,
    node_picker_rows: Vec<String>,
    node_picker_page: String,
}

impl EditorView {
    fn from_state(state: &EditorState) -> Self {
        let mut nodes = Vec::new();
        let mut scene_roots = Vec::new();
        let mut scene_branches = Vec::new();
        let mut selected_row = None;
        let mut inspector_name = "-".to_string();
        let mut inspector_type = "-".to_string();
        let mut inspector_parent = "-".to_string();
        let mut inspector_pos = "-".to_string();
        let mut inspector_script = "-".to_string();
        let mut inspector_vars = "-".to_string();

        if !state.doc_text.is_empty() {
            let doc = SceneDoc::parse(&state.doc_text);
            let tree = scene_tree_view(&doc, state.selected_key);
            nodes = tree.labels;
            scene_roots = tree.roots;
            scene_branches = tree.branches;
            selected_row = tree.selected_row;

            if let Some(key) = state.selected_key.and_then(|raw| {
                doc.scene
                    .nodes
                    .iter()
                    .find(|node| node.key.as_u32() == raw)
                    .map(|node| node.key)
            }) {
                if let Some(node) = doc.scene.nodes.iter().find(|node| node.key == key) {
                    inspector_name = doc.scene.key_name_or_id(node.key).to_string();
                    inspector_type = node.data.type_name().to_string();
                    inspector_parent = node
                        .parent
                        .map(|key| doc.scene.key_name_or_id(key).to_string())
                        .unwrap_or_else(|| "-".to_string());
                    inspector_pos = find_position_text(&node.data).unwrap_or_else(|| "-".to_string());
                    inspector_script = node
                        .script
                        .as_ref()
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".to_string());
                    inspector_vars = if node.script_vars.is_empty() {
                        "-".to_string()
                    } else {
                        format!("{} fields", node.script_vars.len())
                    };
                }
            }
        }

        let status = if state.project_root.is_empty() {
            "ready | open project".to_string()
        } else {
            format!(
                "ready | {} | open={} | dirty={}",
                state.project_name,
                state.open_paths.len(),
                state.dirty
            )
        };
        let viewport = format!(
            "Viewport  mode={}  cam=({:.1}, {:.1}, {:.1})",
            state.viewport_mode, state.cam_x, state.cam_y, state.cam_z
        );
        let node_picker_rows = picker_rows(state.node_picker_offset);
        let page = (state.node_picker_offset / MAX_NODE_PICKER_ROWS) + 1;
        let page_count = perro_scene::NodeType::ALL
            .len()
            .div_ceil(MAX_NODE_PICKER_ROWS);
        Self {
            project_root: state.project_root.clone(),
            project_name: if state.project_name.is_empty() {
                "No project".to_string()
            } else {
                state.project_name.clone()
            },
            create_parent_dir: if state.create_parent_dir.is_empty() {
                "-".to_string()
            } else {
                state.create_parent_dir.clone()
            },
            recent_projects: state.recent_projects.clone(),
            file_paths: state.file_paths.clone(),
            scene_paths: state.scene_paths.clone(),
            open_paths: state.open_paths.clone(),
            active_open: state.active_open,
            nodes,
            scene_roots,
            scene_branches,
            selected_row,
            inspector_name,
            inspector_type,
            inspector_parent,
            inspector_pos,
            inspector_script,
            inspector_vars,
            viewport,
            status,
            log: state.log.clone(),
            node_picker_rows,
            node_picker_page: format!("page {page}/{page_count}"),
        }
    }
}

#[derive(Default)]
struct SceneTreeRows {
    labels: Vec<String>,
    keys: Vec<u32>,
    roots: Vec<usize>,
    branches: Vec<(usize, Vec<usize>)>,
    selected_row: Option<usize>,
}

fn scene_tree_view(doc: &SceneDoc, selected_key: Option<u32>) -> SceneTreeRows {
    let mut out = SceneTreeRows::default();
    let mut visited = Vec::new();
    let mut roots = Vec::new();

    if let Some(root) = doc.scene.root {
        roots.push(root.as_u32());
    }
    for node in doc.scene.nodes.iter() {
        let key = node.key.as_u32();
        if node.parent.is_none() && !roots.contains(&key) {
            roots.push(key);
        }
    }
    for key in roots {
        push_scene_tree_row(doc, key, selected_key, &mut visited, &mut out);
    }
    for node in doc.scene.nodes.iter() {
        let key = node.key.as_u32();
        if !visited.contains(&key) {
            push_scene_tree_row(doc, key, selected_key, &mut visited, &mut out);
        }
    }
    out
}

fn push_scene_tree_row(
    doc: &SceneDoc,
    key: u32,
    selected_key: Option<u32>,
    visited: &mut Vec<u32>,
    out: &mut SceneTreeRows,
) -> Option<usize> {
    if out.labels.len() >= MAX_NODES || visited.contains(&key) {
        return None;
    }
    let node = doc
        .scene
        .nodes
        .iter()
        .find(|node| node.key.as_u32() == key)?;
    visited.push(key);
    let row = out.labels.len();
    let prefix = if Some(key) == selected_key {
        out.selected_row = Some(row);
        ">"
    } else {
        " "
    };
    out.labels.push(format!(
        "{prefix} {} : {}",
        doc.scene.key_name_or_id(node.key),
        node.data.type_name()
    ));
    out.keys.push(key);
    if node.parent.is_none() || doc.scene.root.map(|root| root.as_u32()) == Some(key) {
        out.roots.push(row);
    }

    let mut child_rows = Vec::new();
    for child in doc
        .scene
        .nodes
        .iter()
        .filter(|child| child.parent.map(|parent| parent.as_u32()) == Some(key))
    {
        if let Some(child_row) =
            push_scene_tree_row(doc, child.key.as_u32(), selected_key, visited, out)
        {
            child_rows.push(child_row);
        }
    }
    if !child_rows.is_empty() {
        out.branches.push((row, child_rows));
    }
    Some(row)
}

fn picker_rows(offset: usize) -> Vec<String> {
    perro_scene::NodeType::ALL
        .iter()
        .skip(offset)
        .take(MAX_NODE_PICKER_ROWS)
        .map(|node_type| format!("{} {}", node_type_icon(*node_type), node_type.name()))
        .collect()
}

fn node_type_icon(node_type: perro_scene::NodeType) -> &'static str {
    match node_type.name() {
        "Sprite2D" => "[SPR]",
        "Camera2D" | "Camera3D" => "[CAM]",
        "MeshInstance3D" | "MultiMeshInstance3D" => "[MSH]",
        "PointLight2D" | "SpotLight2D" | "RayLight2D" | "AmbientLight2D" | "PointLight3D"
        | "SpotLight3D" | "RayLight3D" | "AmbientLight3D" => "[LGT]",
        "AudioPlayer2D" | "AudioStreamPlayer2D" | "AudioArea2D" | "AudioPlayer3D"
        | "AudioStreamPlayer3D" | "AudioArea3D" => "[AUD]",
        "PhysicsBody2D" | "StaticBody2D" | "RigidBody2D" | "CharacterBody2D" | "Area2D"
        | "CollisionShape2D" | "PhysicsBody3D" | "StaticBody3D" | "RigidBody3D"
        | "CharacterBody3D" | "Area3D" | "CollisionShape3D" => "[PHY]",
        _ if node_type.is_a(perro_scene::NodeType::UiBox) => "[UI]",
        _ if node_type.is_a(perro_scene::NodeType::Node2D) => "[2D]",
        _ if node_type.is_a(perro_scene::NodeType::Node3D) => "[3D]",
        _ if node_type.name().ends_with("Resource") => "[RES]",
        _ => "[NOD]",
    }
}

fn find_position_text(data: &SceneNodeData) -> Option<String> {
    for (name, value) in data.fields.iter() {
        if name.as_ref() == "position" {
            return match value {
                SceneValue::Vec2 { x, y } => Some(format!("({x:.2}, {y:.2})")),
                SceneValue::Vec3 { x, y, z } => Some(format!("({x:.2}, {y:.2}, {z:.2})")),
                _ => None,
            };
        }
    }
    data.base_ref().and_then(find_position_text)
}

fn unique_node_name(doc: &SceneDoc, prefix: &str) -> String {
    for idx in 1..1000 {
        let name = format!("{prefix}_{idx}");
        if !doc.scene.key_names.iter().any(|item| item.as_ref() == name) {
            return name;
        }
    }
    format!("{prefix}_x")
}

fn parse_project_name(text: &str) -> Option<String> {
    let mut in_project = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "[project]" {
            in_project = true;
            continue;
        }
        if in_project && trimmed.starts_with('[') {
            return None;
        }
        if in_project && trimmed.starts_with("name") {
            let (_, value) = trimmed.split_once('=')?;
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn abs_to_res(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root.join("res")).ok()?;
    let rel = rel.to_string_lossy().replace('\\', "/");
    Some(format!("res://{}", rel.trim_start_matches('/')))
}

fn res_to_abs(root: &str, res_path: &str) -> String {
    let rel = res_path.trim_start_matches("res://");
    Path::new(root)
        .join("res")
        .join(rel)
        .to_string_lossy()
        .to_string()
}

fn suffix_index(name: &str, prefix: &str) -> Option<usize> {
    name.strip_prefix(prefix)?.parse::<usize>().ok()
}

fn short_path(path: &str, max: usize) -> String {
    if path.len() <= max {
        path.to_string()
    } else {
        format!("...{}", &path[path.len().saturating_sub(max - 3)..])
    }
}

fn set_log<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, text: &str) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.log = text.to_string();
    });
    set_label(ctx, "log_text", text);
}

fn set_label<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, name: &str, text: &str) {
    if let Some(id) = find_named(ctx, name) {
        let text = text.to_string();
        let _ = with_node_mut!(ctx.run, UiLabel, id, |node| {
            node.set_text(text);
        });
    }
}

fn read_text_box<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, name: &str) -> Option<String> {
    let id = find_named(ctx, name)?;
    Some(with_node!(ctx.run, UiTextBox, id, |node| node.text.to_string()))
}

fn set_button_fill<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, name: &str, fill: &str) {
    let Some(color) = Color::from_hex(fill) else {
        return;
    };
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiButton, id, |node| {
            node.style.fill = color;
        });
    }
}

fn apply_scene_tree_layout<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    roots: &[usize],
    branches: &[(usize, Vec<usize>)],
) {
    let Some(tree_id) = find_named(ctx, "scene_rows") else {
        return;
    };
    let row_ids = (0..MAX_NODES)
        .map(|idx| find_named(ctx, &format!("scene_row_{idx}")))
        .collect::<Vec<_>>();
    let root_ids = roots
        .iter()
        .filter_map(|idx| row_ids.get(*idx).and_then(|id| *id))
        .collect::<Vec<_>>();

    let _ = with_node_mut!(ctx.run, UiList, tree_id, |tree| {
        tree.roots = root_ids;
        tree.branches.clear();
        tree.collapsed.clear();
        tree.indent = 18.0;
        tree.v_spacing = 0.006;
        for (parent_idx, child_indices) in branches {
            let Some(Some(parent_id)) = row_ids.get(*parent_idx) else {
                continue;
            };
            let children = child_indices
                .iter()
                .filter_map(|idx| row_ids.get(*idx).and_then(|id| *id))
                .collect::<Vec<_>>();
            tree.set_branch(*parent_id, children);
        }
    });
}

fn set_add_node_popup<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, visible: bool) {
    if let Some(id) = find_named(ctx, "add_node_popup") {
        let _ = with_node_mut!(ctx.run, UiVLayout, id, |node| {
            node.visible = visible;
            node.input_enabled = visible;
        });
    }
}

fn set_project_manager<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, visible: bool) {
    if let Some(id) = find_named(ctx, "project_manager") {
        let _ = with_node_mut!(ctx.run, UiVLayout, id, |node| {
            node.visible = visible;
            node.input_enabled = visible;
        });
    }
}

fn find_named<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, name: &str) -> Option<NodeID> {
    let mut stack = vec![ctx.id];
    while let Some(id) = stack.pop() {
        if get_node_name!(ctx.run, id).as_deref() == Some(name) {
            return Some(id);
        }
        stack.extend(get_children!(ctx.run, id));
    }
    None
}

fn save_recent_projects(recent: &[String]) {
    let list = recent
        .iter()
        .map(|item| format!("\"{}\"", json_escape(item)))
        .collect::<Vec<_>>()
        .join(",");
    let text = format!("{{\"recent\":[{list}]}}");
    let _ = FileMod::save_string(RECENT_PROJECTS_PATH, &text);
}

fn load_recent_projects() -> Vec<String> {
    let text = FileMod::load_string(RECENT_PROJECTS_PATH).unwrap_or_default();
    let mut out = Vec::new();
    for path in parse_recent_projects(&text) {
        if !out.iter().any(|item| item == &path) && validate_project_root(Path::new(&path)).is_ok() {
            out.push(path);
        }
    }
    out.truncate(MAX_RECENT);
    save_recent_projects(&out);
    out
}

fn parse_recent_projects(text: &str) -> Vec<String> {
    let Some(start) = text.find('[') else {
        return Vec::new();
    };
    let Some(end) = text[start..].find(']') else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut item = String::new();
    let mut in_string = false;
    let mut escape = false;
    for ch in text[start + 1..start + end].chars() {
        if escape {
            item.push(ch);
            escape = false;
            continue;
        }
        if ch == '\\' && in_string {
            escape = true;
            continue;
        }
        if ch == '"' {
            if in_string {
                if !item.is_empty() {
                    out.push(item.clone());
                }
                item.clear();
            }
            in_string = !in_string;
            continue;
        }
        if in_string {
            item.push(ch);
        }
    }
    out
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
