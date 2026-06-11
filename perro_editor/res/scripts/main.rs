use perro_api::prelude::*;
use perro_api::scene::{
    SceneDoc, SceneFieldName, SceneKey, SceneNodeData, SceneNodeEntry, SceneValue,
};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::str::FromStr;

mod editor_project;
mod editor_file_watch;
mod editor_gizmos;
mod editor_scene_deps;
mod editor_app;
mod editor_files;
mod editor_manager;
mod editor_scene;
mod editor_view;

type SelfNodeType = UiPanel;

const MAX_FILES: usize = 12;
const MAX_NODES: usize = 12;
const MAX_TABS: usize = 2;
const MAX_RECENT: usize = 5;
const MAX_NODE_PICKER_ROWS: usize = 12;
const RECENT_PROJECTS_PATH: &str = "user://recent_projects.json";
const FILE_WATCH_INTERVAL_FRAMES: u32 = 30;

#[State]
struct EditorState {
    editor_shell_root: u64,
    project_root: String,
    project_name: String,
    create_parent_dir: String,
    recent_projects: Vec<String>,
    file_paths: Vec<String>,
    scene_paths: Vec<String>,
    open_paths: Vec<String>,
    active_open: usize,
    doc_text: String,
    preview_scene_paths: Vec<String>,
    preview_root: u64,
    preview_camera_2d: u64,
    preview_camera_3d: u64,
    preview_node_ids: Vec<u64>,
    preview_node_keys: Vec<u32>,
    project_file_sigs: Vec<editor_file_watch::FileSig>,
    dirty_scene_paths: Vec<String>,
    file_watch_frame: u32,
    preview_serial: u64,
    selected_key: Option<u32>,
    ui_drag_key: Option<u32>,
    ui_drag_mode: String,
    ui_drag_last_x: f32,
    ui_drag_last_y: f32,
    viewport_mode: String,
    dirty: bool,
    node_picker_offset: usize,
    cam_x: f32,
    cam_y: f32,
    cam_z: f32,
    cam_yaw: f32,
    cam_pitch: f32,
    cam2_x: f32,
    cam2_y: f32,
    cam2_zoom: f32,
    ui_canvas_x: f32,
    ui_canvas_y: f32,
    ui_canvas_zoom: f32,
    log: String,
}

lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        connect_editor_signals(ctx);

        let recent = load_recent_projects();
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            state.recent_projects = recent;
            state.log = "project manager".to_string();
            state.ui_canvas_zoom = 1.0;
            state.cam2_zoom = 1.0;
        });
        refresh_all(ctx);
        set_project_manager(ctx, true);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        update_freecam(ctx);
        update_ui_canvas(ctx);
        update_preview_pick(ctx);
        update_ui_drag(ctx);
        update_editor_cursor(ctx);
        poll_project_diffs(ctx);
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
                let has_editor = with_state!(ctx.run, EditorState, ctx.id, |state| {
                    state.editor_shell_root != 0
                });
                if has_editor {
                    set_project_manager(ctx, false);
                }
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
            "viewport_click_layer" => handle_viewport_click(ctx),
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
    let _ = signal_connect_many!(
        ctx.run,
        ctx.id,
        [
            signal!("editor_open_project"),
            signal!("editor_manager_browse"),
            signal!("editor_manager_choose_location"),
            signal!("editor_manager_create"),
            signal!("editor_manager_close"),
            signal!("editor_recent_0"),
            signal!("editor_recent_1"),
            signal!("editor_recent_2"),
            signal!("editor_recent_3"),
            signal!("editor_recent_4"),
            signal!("editor_save_scene"),
            signal!("editor_add_node"),
            signal!("editor_mode_ui"),
            signal!("editor_mode_2d"),
            signal!("editor_mode_3d"),
            signal!("editor_open_file_0"),
            signal!("editor_open_file_1"),
            signal!("editor_open_file_2"),
            signal!("editor_open_file_3"),
            signal!("editor_open_file_4"),
            signal!("editor_open_file_5"),
            signal!("editor_open_file_6"),
            signal!("editor_open_file_7"),
            signal!("editor_open_file_8"),
            signal!("editor_open_file_9"),
            signal!("editor_open_file_10"),
            signal!("editor_open_file_11"),
            signal!("editor_select_scene_0"),
            signal!("editor_select_scene_1"),
            signal!("editor_select_scene_2"),
            signal!("editor_select_scene_3"),
            signal!("editor_select_scene_4"),
            signal!("editor_select_scene_5"),
            signal!("editor_select_scene_6"),
            signal!("editor_select_scene_7"),
            signal!("editor_select_scene_8"),
            signal!("editor_select_scene_9"),
            signal!("editor_select_scene_10"),
            signal!("editor_select_scene_11"),
            signal!("editor_tab_0"),
            signal!("editor_tab_1"),
            signal!("editor_add_type_0"),
            signal!("editor_add_type_1"),
            signal!("editor_add_type_2"),
            signal!("editor_add_type_3"),
            signal!("editor_add_type_4"),
            signal!("editor_add_type_5"),
            signal!("editor_add_type_6"),
            signal!("editor_add_type_7"),
            signal!("editor_add_type_8"),
            signal!("editor_add_type_9"),
            signal!("editor_add_type_10"),
            signal!("editor_add_type_11"),
            signal!("editor_add_type_prev"),
            signal!("editor_add_type_next"),
            signal!("editor_add_node_cancel"),
            signal!("editor_viewport_click"),
        ],
        [func!("on_editor_signal")]
    );
}

fn update_freecam<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.viewport_mode.clone()
    });
    if mode == "2D" {
        update_freecam_2d(ctx);
        return;
    }
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

    let stream_id = find_named(ctx, "viewport_stream_3d").map(NodeID::as_u64).unwrap_or(0);
    let mut label = String::new();
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let speed = if key_down!(ctx.ipt, KeyCode::ControlLeft) { 18.0 } else { 7.0 };
        let rotation = Quaternion::from_euler_xyz(state.cam_pitch, state.cam_yaw, 0.0);
        let right = rotation.rotate_vector3(Vector3::new(1.0, 0.0, 0.0));
        let forward = rotation.rotate_vector3(Vector3::new(0.0, 0.0, -1.0));
        let up = Vector3::new(0.0, 1.0, 0.0);
        let movement = (right * dx) + (up * dy) + (forward * -dz);
        state.cam_x += movement.x * speed * dt;
        state.cam_y += movement.y * speed * dt;
        state.cam_z += movement.z * speed * dt;
        state.cam_yaw += mouse.x * 0.0025;
        state.cam_pitch = (state.cam_pitch - mouse.y * 0.0025).clamp(-1.4, 1.4);
        label = format!(
            "Viewport  mode={}  cam=({:.1}, {:.1}, {:.1}) yaw={:.2} pitch={:.2} stream={} cam_id={}",
            state.viewport_mode,
            state.cam_x,
            state.cam_y,
            state.cam_z,
            state.cam_yaw,
            state.cam_pitch,
            stream_id,
            state.preview_camera_3d
        );
    });
    apply_freecam(ctx);
    set_label(ctx, "viewport_label", &label);
}

fn update_freecam_2d<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let dt = delta_time!(ctx.run).clamp(0.0, 1.0 / 30.0);
    let mut dx = 0.0;
    let mut dy = 0.0;
    if key_down!(ctx.ipt, KeyCode::KeyA) {
        dx -= 1.0;
    }
    if key_down!(ctx.ipt, KeyCode::KeyD) {
        dx += 1.0;
    }
    if key_down!(ctx.ipt, KeyCode::KeyW) {
        dy += 1.0;
    }
    if key_down!(ctx.ipt, KeyCode::KeyS) {
        dy -= 1.0;
    }
    let wheel = viewport_pointer(ctx)
        .map(|_| mouse_wheel!(ctx.ipt).y)
        .unwrap_or(0.0);
    let mouse = if mouse_down!(ctx.ipt, MouseButton::Middle) {
        mouse_delta!(ctx.ipt)
    } else {
        Vector2::ZERO
    };
    let zoom_dir = if key_down!(ctx.ipt, KeyCode::KeyE) {
        1.0
    } else if key_down!(ctx.ipt, KeyCode::KeyQ) {
        -1.0
    } else {
        0.0
    };
    let stream_id = find_named(ctx, "viewport_stream_2d").map(NodeID::as_u64).unwrap_or(0);
    let mut label = String::new();
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.cam2_zoom <= 0.001 {
            state.cam2_zoom = 1.0;
        }
        let speed = 480.0 / state.cam2_zoom.max(0.001);
        state.cam2_x += dx * speed * dt;
        state.cam2_y += dy * speed * dt;
        state.cam2_x -= mouse.x / state.cam2_zoom.max(0.001);
        state.cam2_y += mouse.y / state.cam2_zoom.max(0.001);
        if zoom_dir != 0.0 || wheel.abs() > 0.001 {
            let key_zoom = zoom_dir * 1.8 * dt;
            let wheel_zoom = wheel * 0.12;
            state.cam2_zoom = (state.cam2_zoom * (1.0 + key_zoom + wheel_zoom)).clamp(0.05, 40.0);
        }
        label = format!(
            "Viewport  mode={}  cam=({:.1}, {:.1}) zoom={:.2} stream={} cam_id={}",
            state.viewport_mode,
            state.cam2_x,
            state.cam2_y,
            state.cam2_zoom,
            stream_id,
            state.preview_camera_2d
        );
    });
    apply_freecam_2d(ctx);
    apply_viewport_canvas(ctx);
    set_label(ctx, "viewport_label", &label);
}

fn update_ui_canvas<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.viewport_mode.clone()
    });
    if mode != "UI" {
        return;
    }
    let inside = viewport_pointer(ctx).is_some();
    let wheel = if inside { mouse_wheel!(ctx.ipt).y } else { 0.0 };
    let mouse = if inside && mouse_down!(ctx.ipt, MouseButton::Middle) {
        mouse_delta!(ctx.ipt)
    } else {
        Vector2::ZERO
    };
    let mut label = String::new();
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.ui_canvas_zoom <= 0.001 {
            state.ui_canvas_zoom = 1.0;
        }
        state.ui_canvas_x += mouse.x / 540.0;
        state.ui_canvas_y += mouse.y / 300.0;
        if wheel.abs() > 0.001 {
            state.ui_canvas_zoom = (state.ui_canvas_zoom * (1.0 + wheel * 0.12)).clamp(0.25, 12.0);
        }
        label = format!(
            "Viewport  mode={}  canvas=({:.2}, {:.2}) zoom={:.2}",
            state.viewport_mode, state.ui_canvas_x, state.ui_canvas_y, state.ui_canvas_zoom
        );
    });
    apply_viewport_canvas(ctx);
    set_label(ctx, "viewport_label", &label);
}

fn reset_freecam(state: &mut EditorState) {
    state.cam_x = 0.0;
    state.cam_y = 3.0;
    state.cam_z = 8.0;
    state.cam_yaw = 0.0;
    state.cam_pitch = -0.25;
}

fn reset_freecam_2d(state: &mut EditorState) {
    state.cam2_x = 0.0;
    state.cam2_y = 0.0;
    state.cam2_zoom = 1.0;
}

fn apply_freecam<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let camera = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (state.preview_camera_3d != 0).then(|| NodeID::from_u64(state.preview_camera_3d))
    })
    .or_else(|| find_named(ctx, "editor_camera_3d"));
    let Some(camera) = camera else {
        return;
    };
    let (pos, rot) = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (
            Vector3::new(state.cam_x, state.cam_y, state.cam_z),
            Quaternion::from_euler_xyz(state.cam_pitch, state.cam_yaw, 0.0),
        )
    });
    let _ = with_node_mut!(ctx.run, Camera3D, camera, |node| {
        node.active = true;
    });
    let _ = ctx.run.Nodes().set_local_transform_3d(
        camera,
        Transform3D::new(pos, rot, Vector3::ONE),
    );
}

fn apply_freecam_2d<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let camera = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (state.preview_camera_2d != 0).then(|| NodeID::from_u64(state.preview_camera_2d))
    })
    .or_else(|| find_named(ctx, "editor_camera_2d"));
    let Some(camera) = camera else {
        return;
    };
    let (pos, zoom) = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (Vector2::new(state.cam2_x, state.cam2_y), state.cam2_zoom)
    });
    let _ = with_node_mut!(ctx.run, Camera2D, camera, |node| {
        node.active = true;
        node.zoom = zoom;
    });
    let _ = ctx
        .run
        .Nodes()
        .set_local_transform_2d(camera, Transform2D::new(pos, 0.0, Vector2::ONE));
}

fn open_project<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root: String,
) -> Result<(), String> {
    clear_preview(ctx);
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
    let initial_scene = editor_manager::project_main_scene(&project_text)
        .filter(|path| scene_paths.iter().any(|scene| scene == path))
        .or_else(|| scene_paths.first().cloned());
    let log = format!(
        "open project\nroot: {}\nscenes: {}",
        root_path.display(),
        scene_paths.len()
    );

    load_editor_shell(ctx)?;

    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.project_root = root_path.to_string_lossy().to_string();
        state.project_name = project_name;
        state.file_paths = file_paths;
        state.scene_paths = scene_paths;
        state.open_paths.clear();
        state.active_open = 0;
        state.doc_text.clear();
        state.preview_scene_paths.clear();
        state.preview_root = 0;
        state.preview_camera_2d = 0;
        state.preview_camera_3d = 0;
        state.preview_node_ids.clear();
        state.preview_node_keys.clear();
        state.project_file_sigs = editor_file_watch::scan_project(root_path.as_path());
        state.dirty_scene_paths.clear();
        state.file_watch_frame = 0;
        state.preview_serial = 0;
        state.selected_key = None;
        state.ui_drag_key = None;
        state.ui_drag_mode.clear();
        state.ui_drag_last_x = 0.0;
        state.ui_drag_last_y = 0.0;
        reset_freecam_2d(state);
        state.dirty = false;
        state.log = log;
    });

    add_recent_project(ctx, root_path.to_string_lossy().as_ref());
    set_project_manager(ctx, false);
    refresh_all(ctx);
    if let Some(scene) = initial_scene {
        open_scene_path(ctx, &scene);
    }
    Ok(())
}

fn load_editor_shell<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) -> Result<(), String> {
    let old = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let old = state.editor_shell_root;
        state.editor_shell_root = 0;
        old
    })
    .unwrap_or(0);
    if old != 0 {
        let _ = ctx.run.Nodes().remove_node(NodeID::from_u64(old));
    }

    let root = ctx
        .run
        .Scene()
        .load(editor_app::EDITOR_SHELL_SCENE.to_string())
        .map_err(|err| format!("editor shell load fail\n{err}"))?;
    let _ = ctx.run.Nodes().reparent(ctx.id, root);
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.editor_shell_root = root.as_u64();
    });
    Ok(())
}

fn open_project_dialog<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    if let Some(path) = FileMod::pick_folder("Open Perro Project")
        && let Err(err) = open_project(ctx, path.clone())
    {
        set_log(ctx, &format!("open project fail\n{path}\n{err}"));
        refresh_recent_projects(ctx);
        set_project_manager(ctx, true);
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
    out.sort_by_key(|path| editor_files::res_browser_sort_key(path));
    Ok(out)
}

fn open_file_slot<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, idx: usize) {
    let res_path = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.file_paths.get(idx).cloned()
    });
    let Some(scene_path) = res_path else {
        return;
    };
    if scene_path.ends_with('/') {
        set_log(ctx, &format!("folder\n{scene_path}"));
        return;
    }
    if !scene_path.ends_with(".scn") {
        set_log(ctx, &format!("{} file\n{}", editor_files::kind_label(&scene_path), editor_files::rel_label(&scene_path)));
        return;
    }
    open_scene_path(ctx, &scene_path);
}

fn open_scene_path<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, scene_path: &str) {
    if scene_path.ends_with('/') {
        set_log(ctx, &format!("folder\n{}", editor_files::rel_label(scene_path)));
        return;
    }
    if !scene_path.ends_with(".scn") {
        set_log(ctx, &format!("{} file\n{}", editor_files::kind_label(scene_path), editor_files::rel_label(scene_path)));
        return;
    }
    let root = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.project_root.clone()
    });
    let abs = res_to_abs(&root, scene_path);
    let text = match FileMod::load_string(&abs) {
        Ok(text) => text,
        Err(err) => {
            set_log(ctx, &format!("open scene fail\n{scene_path}\n{err}"));
            return;
        }
    };
    let doc = SceneDoc::parse(&text);
    let first_key = doc.scene.nodes.first().map(|node| node.key.as_u32());
    let mode = editor_scene::root_viewport_mode(&doc);
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if !state.open_paths.iter().any(|path| path == scene_path) {
            state.open_paths.push(scene_path.to_string());
        }
        state.active_open = state
            .open_paths
            .iter()
            .position(|path| path == scene_path)
            .unwrap_or(0);
        state.doc_text = doc.to_text();
        state.selected_key = first_key;
        state.viewport_mode = mode.to_string();
        if mode == "3D" {
            reset_freecam(state);
        } else if mode == "2D" {
            reset_freecam_2d(state);
        }
        state.dirty = false;
        state.dirty_scene_paths.retain(|path| path != scene_path);
        state.log = format!("open scene\n{}", editor_files::rel_label(scene_path));
    });
    rebuild_preview(ctx);
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
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = format!("add node\n{name}: {}", node_type.name());
        msg = state.log.clone();
    });
    set_log(ctx, &msg);
    rebuild_preview(ctx);
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
                state.dirty_scene_paths.retain(|item| item != &path);
                state.project_file_sigs = editor_file_watch::scan_project(Path::new(&root));
                state.log = format!("save scene\n{path}");
            });
            rebuild_preview(ctx);
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
        if mode == "3D" {
            reset_freecam(state);
        } else if mode == "2D" {
            reset_freecam_2d(state);
        }
        state.log = format!("mode {mode}");
    });
    apply_viewport_mode(ctx, mode);
    apply_freecam(ctx);
    apply_freecam_2d(ctx);
    refresh_all(ctx);
}

#[derive(Clone, Copy, Debug)]
struct ViewportPointer {
    uv: Vector2,
    ndc: Vector2,
}

#[derive(Clone, Copy, Debug)]
struct ViewportRay3D {
    origin: Vector3,
    direction: Vector3,
}

fn handle_viewport_click<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(pointer) = viewport_pointer(ctx) else {
        return;
    };
    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.viewport_mode.clone()
    });
    match mode.as_str() {
        "UI" => {
            let _ = pick_preview_ui(ctx);
            set_log(
                ctx,
                &format!(
                    "ui canvas click\nuv=({:.3}, {:.3}) ndc=({:.3}, {:.3})",
                    pointer.uv.x, pointer.uv.y, pointer.ndc.x, pointer.ndc.y
                ),
            );
        }
        "2D" => {
            if let Some(world) = stream_pointer_world_2d(ctx, pointer) {
                set_log(
                    ctx,
                    &format!(
                        "2d stream click\nuv=({:.3}, {:.3}) world=({:.2}, {:.2})",
                        pointer.uv.x, pointer.uv.y, world.x, world.y
                    ),
                );
            }
        }
        "3D" => {
            if let Some(ray) = stream_pointer_ray_3d(ctx, pointer) {
                set_log(
                    ctx,
                    &format!(
                        "3d stream click\norigin=({:.2}, {:.2}, {:.2}) dir=({:.3}, {:.3}, {:.3})",
                        ray.origin.x,
                        ray.origin.y,
                        ray.origin.z,
                        ray.direction.x,
                        ray.direction.y,
                        ray.direction.z
                    ),
                );
            }
        }
        _ => {}
    }
}

fn viewport_pointer<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) -> Option<ViewportPointer> {
    let mouse = mouse_position!(ctx.ipt);
    let viewport = ctx.res.viewport_size();
    if viewport.x <= 0.0 || viewport.y <= 0.0 {
        return None;
    }

    let x = mouse.x;
    let y = mouse.y;
    let center_x = 0.5;
    let center_y = 0.055 + 0.885 * 0.5;
    let size_x = 0.60 * 0.94;
    let size_y = 0.885 * 0.72 * 0.82;
    let min_x = center_x - size_x * 0.5;
    let max_x = center_x + size_x * 0.5;
    let min_y = center_y - size_y * 0.5;
    let max_y = center_y + size_y * 0.5;
    if x < min_x || x > max_x || y < min_y || y > max_y {
        return None;
    }
    let uv = Vector2::new((x - min_x) / size_x, (y - min_y) / size_y);
    Some(ViewportPointer {
        uv,
        ndc: Vector2::new(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0),
    })
}

fn stream_pointer_world_2d<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    pointer: ViewportPointer,
) -> Option<Vector2> {
    let camera = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (state.preview_camera_2d != 0).then(|| NodeID::from_u64(state.preview_camera_2d))
    })
    .or_else(|| find_named(ctx, "editor_camera_2d"))?;
    let global = ctx.run.Nodes().get_global_transform_2d(camera)?;
    let zoom = with_node!(ctx.run, Camera2D, camera, |node| node.zoom).max(0.0001);
    let local = Vector2::new(pointer.ndc.x * 480.0 / zoom, pointer.ndc.y * 270.0 / zoom);
    let sin = global.rotation.sin();
    let cos = global.rotation.cos();
    Some(Vector2::new(
        global.position.x + local.x * cos - local.y * sin,
        global.position.y + local.x * sin + local.y * cos,
    ))
}

fn stream_pointer_ray_3d<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    pointer: ViewportPointer,
) -> Option<ViewportRay3D> {
    let camera = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (state.preview_camera_3d != 0).then(|| NodeID::from_u64(state.preview_camera_3d))
    })
    .or_else(|| find_named(ctx, "editor_camera_3d"))?;
    let global = ctx.run.Nodes().get_global_transform_3d(camera)?;
    let projection = with_node!(ctx.run, Camera3D, camera, |node| node.projection.clone());
    let aspect = 16.0 / 9.0;
    let local_dir = match projection {
        CameraProjection::Perspective { fov_y_degrees, .. } => {
            let tan_y = (fov_y_degrees.to_radians() * 0.5).tan();
            Vector3::new(pointer.ndc.x * aspect * tan_y, pointer.ndc.y * tan_y, -1.0).normalized()
        }
        CameraProjection::Orthographic { .. } => Vector3::new(0.0, 0.0, -1.0),
        CameraProjection::Frustum {
            left,
            right,
            bottom,
            top,
            near,
            ..
        } => {
            let x = left + (pointer.uv.x * (right - left));
            let y = bottom + ((1.0 - pointer.uv.y) * (top - bottom));
            Vector3::new(x, y, -near.max(0.001)).normalized()
        }
    };
    let local_origin = match projection {
        CameraProjection::Orthographic { size, .. } => {
            Vector3::new(pointer.ndc.x * size * aspect * 0.5, pointer.ndc.y * size * 0.5, 0.0)
        }
        _ => Vector3::ZERO,
    };
    let origin_offset = global.rotation.rotate_vector3(local_origin);
    Some(ViewportRay3D {
        origin: global.position + origin_offset,
        direction: global.rotation.rotate_vector3(local_dir).normalized(),
    })
}

fn poll_project_diffs<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let action = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.project_root.is_empty() {
            return None;
        }
        state.file_watch_frame = state.file_watch_frame.wrapping_add(1);
        if state.file_watch_frame % FILE_WATCH_INTERVAL_FRAMES != 0 {
            return None;
        }

        let root = PathBuf::from(&state.project_root);
        let next = editor_file_watch::scan_project(root.as_path());
        let changed = editor_file_watch::changed_paths(&state.project_file_sigs, &next);
        if changed.is_empty() {
            state.project_file_sigs = next;
            return None;
        }
        state.project_file_sigs = next;

        let res_changed = changed.iter().any(|path| editor_file_watch::is_under_res(&root, path));
        let changed_scenes = changed
            .iter()
            .filter_map(|path| editor_file_watch::abs_scene_to_res(&root, path))
            .collect::<Vec<_>>();
        Some((root, res_changed, changed_scenes))
    })
    .flatten();

    let Some((root, res_changed, changed_scenes)) = action else {
        return;
    };

    if res_changed
        && let Ok(paths) = scan_res_paths(root.as_path())
    {
        let scene_paths = paths
            .iter()
            .filter(|path| path.ends_with(".scn"))
            .cloned()
            .collect::<Vec<_>>();
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            state.file_paths = paths;
            state.scene_paths = scene_paths;
        });
    }

    if changed_scenes.is_empty() {
        refresh_all(ctx);
        return;
    }

    let reload = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let active = state.open_paths.get(state.active_open).cloned();
        let affects_preview = changed_scenes
            .iter()
            .any(|path| state.preview_scene_paths.iter().any(|item| item == path));
        let affects_open = active
            .as_ref()
            .is_some_and(|path| changed_scenes.iter().any(|item| item == path));

        if (affects_preview || affects_open) && state.dirty {
            for path in changed_scenes.iter() {
                if !state.dirty_scene_paths.iter().any(|item| item == path) {
                    state.dirty_scene_paths.push(path.clone());
                }
            }
            state.log = "external change pending".to_string();
            return None;
        }

        if affects_open {
            return active;
        }
        if affects_preview {
            state.log = format!("reload preview deps\n{}", changed_scenes.join("\n"));
            return Some(String::new());
        }
        state.log = format!("project file change\n{}", changed_scenes.join("\n"));
        None
    })
    .flatten();

    match reload {
        Some(path) if path.is_empty() => {
            rebuild_preview(ctx);
            refresh_all(ctx);
        }
        Some(path) => {
            reload_scene_path(ctx, &path);
        }
        None => refresh_all(ctx),
    }
}

fn reload_scene_path<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, scene_path: &str) {
    let root = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.project_root.clone()
    });
    let abs = res_to_abs(&root, scene_path);
    let text = match FileMod::load_string(&abs) {
        Ok(text) => text,
        Err(err) => {
            set_log(ctx, &format!("reload scene fail\n{scene_path}\n{err}"));
            return;
        }
    };
    let doc = SceneDoc::parse(&text);
    let first_key = doc.scene.nodes.first().map(|node| node.key.as_u32());
    let mode = editor_scene::root_viewport_mode(&doc);
    let normalized = doc.to_text();
    let same = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.doc_text == normalized
    });
    if same {
        return;
    }
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.doc_text = normalized;
        state.selected_key = first_key;
        state.viewport_mode = mode.to_string();
        if mode == "3D" {
            reset_freecam(state);
        } else if mode == "2D" {
            reset_freecam_2d(state);
        }
        state.dirty = false;
        state.dirty_scene_paths.retain(|path| path != scene_path);
        state.log = format!("reload scene\n{scene_path}");
    });
    rebuild_preview(ctx);
    refresh_all(ctx);
}

fn rebuild_preview<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    clear_preview(ctx);
    let (root, active, doc_text, serial) = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.preview_serial = state.preview_serial.wrapping_add(1);
        (
            state.project_root.clone(),
            state.open_paths.get(state.active_open).cloned(),
            state.doc_text.clone(),
            state.preview_serial,
        )
    })
    .unwrap_or_else(|| (String::new(), None, String::new(), 0));
    let Some(active) = active else {
        return;
    };
    if root.is_empty() || doc_text.is_empty() {
        return;
    }

    let deps = editor_scene_deps::collect_scene_deps(Path::new(&root), &active, &doc_text);
    let mut log = None;
    if let Some(err) = deps.error.clone() {
        log = Some(format!("preview deps fail\n{err}"));
    }
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.preview_scene_paths = deps.paths;
        if let Some(log) = log {
            state.log = log;
        }
    });
    load_preview_scene(ctx, &active, &doc_text, serial);
}

fn clear_preview<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let root = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let root = state.preview_root;
        state.preview_root = 0;
        state.preview_camera_2d = 0;
        state.preview_camera_3d = 0;
        state.preview_node_ids.clear();
        state.preview_node_keys.clear();
        root
    })
    .unwrap_or(0);
    if root != 0 {
        let _ = ctx.run.Nodes().remove_node(NodeID::from_u64(root));
    }
}

fn load_preview_scene<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    path: &str,
    doc_text: &str,
    serial: u64,
) {
    let project_root = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.project_root.clone()
    });
    let preview_text = rewrite_project_res_paths(&SceneDoc::parse(doc_text), &project_root).to_text();
    let preview_path = PathBuf::from(&project_root)
        .join(".perro")
        .join(format!("editor_preview_{serial}.scn"));
    if let Err(err) = FileMod::save_string(preview_path.to_string_lossy().as_ref(), &preview_text) {
        set_log(ctx, &format!("preview write fail\n{path}\n{err}"));
        return;
    }

    let root = match ctx.run.Scene().load(preview_path.to_string_lossy().to_string()) {
        Ok(root) => root,
        Err(err) => {
            set_log(ctx, &format!("preview load fail\n{path}\n{err}"));
            return;
        }
    };
    attach_preview_to_viewport(ctx, root);
    disable_preview_runtime_input(ctx, root);

    let doc_text = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.doc_text.clone()
    });
    let (node_ids, keys, preview_camera_2d, preview_camera_3d) = if doc_text.is_empty() {
        (Vec::new(), Vec::new(), 0, 0)
    } else {
        let doc = SceneDoc::parse(&doc_text);
        add_preview_env(ctx, root, &doc);
        let preview_camera_2d = if editor_scene::has_2d(&doc) {
            let name = format!("__editor_preview_camera_2d_{serial}");
            let camera = create_node!(ctx.run, Camera2D, name, tags![], root);
            set_viewport_stream_camera(ctx, "viewport_stream_2d", camera);
            Some(camera)
        } else {
            None
        };
        let preview_camera_3d = if editor_scene::has_3d(&doc) {
            let name = format!("__editor_preview_camera_3d_{serial}");
            let camera = create_node!(ctx.run, Camera3D, name, tags![], root);
            set_viewport_stream_camera(ctx, "viewport_stream_3d", camera);
            Some(camera)
        } else {
            None
        };
        if let Some(camera) = preview_camera_2d {
            let _ = with_node_mut!(ctx.run, Camera2D, camera, |node| {
                node.active = true;
                node.zoom = 1.0;
            });
        }
        if let Some(camera) = preview_camera_3d {
            let _ = with_node_mut!(ctx.run, Camera3D, camera, |node| {
                node.active = true;
            });
        }
        let doc_keys = preview_doc_order(&doc);
        let node_ids = preview_runtime_order(ctx, root, doc_keys.len());
        (
            node_ids.into_iter().map(NodeID::as_u64).collect::<Vec<_>>(),
            doc_keys,
            preview_camera_2d.map(NodeID::as_u64).unwrap_or(0),
            preview_camera_3d.map(NodeID::as_u64).unwrap_or(0),
        )
    };

    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.preview_root = root.as_u64();
        state.preview_node_ids = node_ids;
        state.preview_node_keys = keys;
        state.preview_camera_2d = preview_camera_2d;
        state.preview_camera_3d = preview_camera_3d;
    });
    apply_freecam(ctx);
    apply_freecam_2d(ctx);
}

fn add_preview_env<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root: NodeID,
    doc: &SceneDoc,
) {
    if !editor_scene::has_3d(doc) {
        return;
    }
    if !editor_scene::has_type(doc, perro_scene::NodeType::AmbientLight3D) {
        let light = create_node!(ctx.run, AmbientLight3D, "__editor_preview_ambient", tags![], root);
        let _ = with_node_mut!(ctx.run, AmbientLight3D, light, |node| {
            node.intensity = 0.35;
        });
    }
    if !editor_scene::has_type(doc, perro_scene::NodeType::Sky3D) {
        let _ = create_node!(ctx.run, Sky3D, "__editor_preview_sky", tags![], root);
    }
}

fn attach_preview_to_viewport<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root: NodeID,
) {
    let Some(panel) = find_named(ctx, "viewport_panel") else {
        return;
    };
    if ctx.run.Nodes().with_base_node::<UiBox, _, _>(root, |_| ()).is_some() {
        let _ = ctx.run.Nodes().reparent(panel, root);
        let _ = with_base_node_mut!(ctx.run, UiBox, root, |node| {
            node.layout.anchor = UiAnchor::Center;
            node.layout.size = UiVector2::ratio(0.94, 0.82);
            node.input_enabled = false;
        });
    }
}

fn preview_doc_order(doc: &SceneDoc) -> Vec<u32> {
    let mut out = Vec::new();
    if let Some(root) = doc.scene.root {
        push_doc_order(doc, root.as_u32(), &mut out);
    }
    for node in doc.scene.nodes.iter() {
        let key = node.key.as_u32();
        if !out.contains(&key) {
            push_doc_order(doc, key, &mut out);
        }
    }
    out
}

fn push_doc_order(doc: &SceneDoc, key: u32, out: &mut Vec<u32>) {
    if out.contains(&key) {
        return;
    }
    out.push(key);
    for child in doc
        .scene
        .nodes
        .iter()
        .filter(|child| child.parent.map(|parent| parent.as_u32()) == Some(key))
    {
        push_doc_order(doc, child.key.as_u32(), out);
    }
}

fn preview_runtime_order<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root: NodeID,
    limit: usize,
) -> Vec<NodeID> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        out.push(id);
        if out.len() >= limit {
            break;
        }
        let mut children = ctx.run.Nodes().get_children(id);
        children.reverse();
        stack.extend(children);
    }
    out
}

fn disable_preview_runtime_input<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root: NodeID,
) {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let _ = with_node_mut!(ctx.run, UiButton, id, |node| {
            node.input_enabled = false;
            node.disabled = true;
        });
        let _ = with_node_mut!(ctx.run, UiImageButton, id, |node| {
            node.input_enabled = false;
            node.disabled = true;
        });
        let _ = with_node_mut!(ctx.run, UiTextBox, id, |node| {
            node.base.input_enabled = false;
        });
        let _ = with_node_mut!(ctx.run, UiTextBlock, id, |node| {
            node.base.input_enabled = false;
        });
        let _ = with_node_mut!(ctx.run, UiScrollContainer, id, |node| {
            node.input_enabled = false;
        });
        let _ = with_node_mut!(ctx.run, Button2D, id, |node| {
            node.input_enabled = false;
            node.disabled = true;
        });
        let _ = with_node_mut!(ctx.run, ImageButton2D, id, |node| {
            node.input_enabled = false;
            node.disabled = true;
        });
        let _ = with_node_mut!(ctx.run, Camera2D, id, |node| {
            node.active = false;
        });
        let _ = with_node_mut!(ctx.run, Camera3D, id, |node| {
            node.active = false;
        });

        stack.extend(ctx.run.Nodes().get_children(id));
    }
}

fn update_preview_pick<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    if !mouse_pressed!(ctx.ipt, MouseButton::Left) {
        return;
    }
    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.viewport_mode.clone()
    });
    if mode != "UI" {
        return;
    }
    let pointer = viewport_pointer(ctx);
    if let Some((handle, pointer)) = pointer.and_then(|pointer| pick_resize_handle(ctx, pointer).map(|handle| (handle, pointer))) {
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            if let Some(key) = state.selected_key {
                state.ui_drag_key = Some(key);
                state.ui_drag_mode = handle.to_string();
                state.ui_drag_last_x = pointer.uv.x;
                state.ui_drag_last_y = pointer.uv.y;
                state.log = format!("resize node\n{handle}");
            }
        });
        refresh_all(ctx);
        return;
    }
    if let Some(pointer) = pointer
        && pick_rotation_zone(ctx, pointer).is_some()
    {
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            if let Some(key) = state.selected_key {
                state.ui_drag_key = Some(key);
                state.ui_drag_mode = "rotate".to_string();
                state.ui_drag_last_x = pointer.uv.x;
                state.ui_drag_last_y = pointer.uv.y;
                state.log = "rotate node".to_string();
            }
        });
        refresh_all(ctx);
        return;
    }
    let Some(key) = pick_preview_ui(ctx) else {
        return;
    };
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.selected_key = Some(key);
        state.ui_drag_key = Some(key);
        state.ui_drag_mode = "move".to_string();
        if let Some(pointer) = pointer {
            state.ui_drag_last_x = pointer.uv.x;
            state.ui_drag_last_y = pointer.uv.y;
        }
        state.log = format!("select node\nkey={key}");
    });
    refresh_all(ctx);
}

fn update_ui_drag<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    if mouse_released!(ctx.ipt, MouseButton::Left) {
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            state.ui_drag_key = None;
            state.ui_drag_mode.clear();
        });
        return;
    }
    if !mouse_down!(ctx.ipt, MouseButton::Left) {
        return;
    }
    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.viewport_mode.clone()
    });
    if mode != "UI" {
        return;
    }
    let Some(pointer) = viewport_pointer(ctx) else {
        return;
    };
    let drag = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let key = state.ui_drag_key?;
        if state.ui_drag_mode.is_empty() {
            return None;
        }
        let delta = Vector2::new(pointer.uv.x - state.ui_drag_last_x, state.ui_drag_last_y - pointer.uv.y);
        let mode = state.ui_drag_mode.clone();
        state.ui_drag_last_x = pointer.uv.x;
        state.ui_drag_last_y = pointer.uv.y;
        if delta.x.abs() < 0.0001 && delta.y.abs() < 0.0001 {
            return None;
        }
        Some((key, mode, delta))
    })
    .flatten();
    let Some((key, mode, root_delta)) = drag else {
        return;
    };
    if mode == "move" {
        move_doc_ui_node(ctx, key, root_delta);
    } else if mode == "rotate" {
        rotate_doc_ui_node(ctx, key, root_delta);
    } else {
        resize_doc_ui_node(ctx, key, &mode, root_delta);
    }
}

fn update_editor_cursor<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let icon = editor_cursor_icon(ctx);
    ctx.run.Window().set_cursor_icon(icon);
}

fn editor_cursor_icon<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) -> CursorIcon {
    let drag = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.ui_drag_mode.clone()
    });
    if !drag.is_empty() {
        return if drag == "move" {
            CursorIcon::Grabbing
        } else if drag == "rotate" {
            CursorIcon::AllResize
        } else {
            resize_cursor_icon(&drag)
        };
    }

    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.viewport_mode.clone()
    });
    if mode != "UI" {
        return CursorIcon::Default;
    }
    let Some(pointer) = viewport_pointer(ctx) else {
        return CursorIcon::Default;
    };
    if let Some(handle) = pick_resize_handle(ctx, pointer) {
        return resize_cursor_icon(handle);
    }
    if pick_rotation_zone(ctx, pointer).is_some() {
        return CursorIcon::AllResize;
    }
    if pick_preview_ui(ctx).is_some() {
        return CursorIcon::Grab;
    }
    CursorIcon::Default
}

fn resize_cursor_icon(handle: &str) -> CursorIcon {
    match handle {
        "resize_n" => CursorIcon::NResize,
        "resize_s" => CursorIcon::SResize,
        "resize_e" => CursorIcon::EResize,
        "resize_w" => CursorIcon::WResize,
        "resize_ne" => CursorIcon::NeResize,
        "resize_nw" => CursorIcon::NwResize,
        "resize_se" => CursorIcon::SeResize,
        "resize_sw" => CursorIcon::SwResize,
        _ => CursorIcon::AllResize,
    }
}

fn move_doc_ui_node<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    key: u32,
    root_delta: Vector2,
) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.doc_text.is_empty() {
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(parent_rect) = doc_ui_parent_rect(&doc, key) else {
            return false;
        };
        if parent_rect.size.x <= 0.0 || parent_rect.size.y <= 0.0 {
            return false;
        }
        let delta = Vector2::new(root_delta.x / parent_rect.size.x, root_delta.y / parent_rect.size.y);
        let Some(node) = doc.scene.nodes.to_mut().iter_mut().find(|node| node.key.as_u32() == key) else {
            return false;
        };
        let current = scene_field_vec2(&node.data, "translation_ratio").unwrap_or(Vector2::ZERO);
        set_scene_vec2(&mut node.data, "translation_ratio", current + delta);
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
}

fn resize_doc_ui_node<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    key: u32,
    handle: &str,
    root_delta: Vector2,
) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.doc_text.is_empty() {
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(parent_rect) = doc_ui_parent_rect(&doc, key) else {
            return false;
        };
        let Some(rect) = doc_ui_rect(&doc, key) else {
            return false;
        };
        if parent_rect.size.x <= 0.0 || parent_rect.size.y <= 0.0 {
            return false;
        }

        let mut min = rect.center - rect.size * 0.5;
        let mut max = rect.center + rect.size * 0.5;
        let (sx, sy) = resize_handle_sign(handle);
        if sx < 0.0 {
            min.x += root_delta.x;
        } else if sx > 0.0 {
            max.x += root_delta.x;
        }
        if sy < 0.0 {
            min.y += root_delta.y;
        } else if sy > 0.0 {
            max.y += root_delta.y;
        }
        let min_size = Vector2::new(0.02, 0.02);
        if max.x - min.x < min_size.x {
            if sx < 0.0 {
                min.x = max.x - min_size.x;
            } else {
                max.x = min.x + min_size.x;
            }
        }
        if max.y - min.y < min_size.y {
            if sy < 0.0 {
                min.y = max.y - min_size.y;
            } else {
                max.y = min.y + min_size.y;
            }
        }
        let new_size = max - min;
        let new_center = min + new_size * 0.5;
        let Some(node) = doc.scene.nodes.to_mut().iter_mut().find(|node| node.key.as_u32() == key) else {
            return false;
        };
        let anchor_text = scene_field_str(&node.data, "anchor").unwrap_or_else(|| "center".to_string());
        let anchor = scene_anchor_dir(&anchor_text);
        let anchor_point = parent_rect.center
            + Vector2::new(parent_rect.size.x * 0.5 * anchor.x, parent_rect.size.y * 0.5 * anchor.y);
        let inward = Vector2::new(new_size.x * 0.5 * anchor.x, new_size.y * 0.5 * anchor.y);
        let translation = Vector2::new(
            (new_center.x - anchor_point.x + inward.x) / parent_rect.size.x,
            (new_center.y - anchor_point.y + inward.y) / parent_rect.size.y,
        );
        let size_ratio = Vector2::new(new_size.x / parent_rect.size.x, new_size.y / parent_rect.size.y);
        set_scene_vec2(&mut node.data, "size_ratio", size_ratio);
        set_scene_vec2(&mut node.data, "translation_ratio", translation);
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
}

fn rotate_doc_ui_node<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    key: u32,
    root_delta: Vector2,
) {
    let (prev, curr) = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (
            Vector2::new(state.ui_drag_last_x - root_delta.x, 1.0 - state.ui_drag_last_y - root_delta.y),
            Vector2::new(state.ui_drag_last_x, 1.0 - state.ui_drag_last_y),
        )
    });
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.doc_text.is_empty() {
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(rect) = doc_ui_rect(&doc, key) else {
            return false;
        };
        let prev_angle = (prev.y - rect.center.y).atan2(prev.x - rect.center.x);
        let curr_angle = (curr.y - rect.center.y).atan2(curr.x - rect.center.x);
        let delta = curr_angle - prev_angle;
        if !delta.is_finite() || delta.abs() < 0.0001 {
            return false;
        }
        let Some(node) = doc.scene.nodes.to_mut().iter_mut().find(|node| node.key.as_u32() == key) else {
            return false;
        };
        let current = scene_field_f32(&node.data, "rotation").unwrap_or(0.0);
        set_scene_f32(&mut node.data, "rotation", current + delta);
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
}

fn pick_preview_ui<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) -> Option<u32> {
    let pointer = viewport_pointer(ctx)?;
    let doc_text = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.doc_text.clone()
    });
    if doc_text.is_empty() {
        return None;
    }
    let doc = SceneDoc::parse(&doc_text);
    let point = Vector2::new(pointer.uv.x, 1.0 - pointer.uv.y);
    pick_doc_ui_node(&doc, point)
}

fn pick_resize_handle<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    pointer: ViewportPointer,
) -> Option<&'static str> {
    let (doc_text, selected) = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (state.doc_text.clone(), state.selected_key)
    });
    let key = selected?;
    let doc = SceneDoc::parse(&doc_text);
    let rect = doc_ui_rect(&doc, key)?;
    let point = Vector2::new(pointer.uv.x, 1.0 - pointer.uv.y);
    resize_handles(rect)
        .into_iter()
        .find(|(_, center)| (point.x - center.x).abs() <= 0.018 && (point.y - center.y).abs() <= 0.018)
        .map(|(name, _)| name)
}

fn pick_rotation_zone<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    pointer: ViewportPointer,
) -> Option<&'static str> {
    let (doc_text, selected) = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (state.doc_text.clone(), state.selected_key)
    });
    let key = selected?;
    let doc = SceneDoc::parse(&doc_text);
    let rect = doc_ui_rect(&doc, key)?;
    let point = Vector2::new(pointer.uv.x, 1.0 - pointer.uv.y);
    let min = rect.center - rect.size * 0.5;
    let max = rect.center + rect.size * 0.5;
    let zones = [
        ("rotate_nw", Vector2::new(min.x - 0.035, max.y + 0.035)),
        ("rotate_ne", Vector2::new(max.x + 0.035, max.y + 0.035)),
        ("rotate_sw", Vector2::new(min.x - 0.035, min.y - 0.035)),
        ("rotate_se", Vector2::new(max.x + 0.035, min.y - 0.035)),
    ];
    zones
        .into_iter()
        .find(|(_, center)| (point.x - center.x).abs() <= 0.045 && (point.y - center.y).abs() <= 0.045)
        .map(|(name, _)| name)
}

fn resize_handles(rect: EditorUiRect) -> [(&'static str, Vector2); 8] {
    let min = rect.center - rect.size * 0.5;
    let max = rect.center + rect.size * 0.5;
    let mid_x = rect.center.x;
    let mid_y = rect.center.y;
    [
        ("resize_nw", Vector2::new(min.x, max.y)),
        ("resize_n", Vector2::new(mid_x, max.y)),
        ("resize_ne", Vector2::new(max.x, max.y)),
        ("resize_w", Vector2::new(min.x, mid_y)),
        ("resize_e", Vector2::new(max.x, mid_y)),
        ("resize_sw", Vector2::new(min.x, min.y)),
        ("resize_s", Vector2::new(mid_x, min.y)),
        ("resize_se", Vector2::new(max.x, min.y)),
    ]
}

fn resize_handle_sign(handle: &str) -> (f32, f32) {
    match handle {
        "resize_nw" => (-1.0, 1.0),
        "resize_n" => (0.0, 1.0),
        "resize_ne" => (1.0, 1.0),
        "resize_w" => (-1.0, 0.0),
        "resize_e" => (1.0, 0.0),
        "resize_sw" => (-1.0, -1.0),
        "resize_s" => (0.0, -1.0),
        "resize_se" => (1.0, -1.0),
        _ => (0.0, 0.0),
    }
}

#[derive(Clone, Copy)]
struct EditorUiRect {
    center: Vector2,
    size: Vector2,
    rotation: f32,
}

impl EditorUiRect {
    fn contains(self, point: Vector2) -> bool {
        let half = self.size * 0.5;
        point.x >= self.center.x - half.x
            && point.x <= self.center.x + half.x
            && point.y >= self.center.y - half.y
            && point.y <= self.center.y + half.y
    }
}

fn pick_doc_ui_node(doc: &SceneDoc, point: Vector2) -> Option<u32> {
    let root_rect = EditorUiRect {
        center: Vector2::new(0.5, 0.5),
        size: Vector2::ONE,
        rotation: 0.0,
    };
    let mut hit = None;
    if let Some(root) = doc.scene.root {
        pick_doc_ui_node_inner(doc, root.as_u32(), root_rect, point, &mut hit);
    }
    for node in doc.scene.nodes.iter() {
        if node.parent.is_none() && doc.scene.root.map(|root| root.as_u32()) != Some(node.key.as_u32()) {
            pick_doc_ui_node_inner(doc, node.key.as_u32(), root_rect, point, &mut hit);
        }
    }
    hit
}

fn pick_doc_ui_node_inner(
    doc: &SceneDoc,
    key: u32,
    parent_rect: EditorUiRect,
    point: Vector2,
    hit: &mut Option<u32>,
) {
    let Some(node) = doc.scene.nodes.iter().find(|node| node.key.as_u32() == key) else {
        return;
    };
    let Some(rect) = editor_ui_rect(&node.data, parent_rect) else {
        return;
    };
    if rect.contains(point) {
        *hit = Some(key);
    }
    for child in doc
        .scene
        .nodes
        .iter()
        .filter(|child| child.parent.map(|parent| parent.as_u32()) == Some(key))
    {
        pick_doc_ui_node_inner(doc, child.key.as_u32(), rect, point, hit);
    }
}

fn editor_ui_rect(data: &SceneNodeData, parent: EditorUiRect) -> Option<EditorUiRect> {
    if !data.type_name().starts_with("Ui") {
        return None;
    }
    if scene_field_bool(data, "visible") == Some(false) {
        return None;
    }
    let anchor_text = scene_field_str(data, "anchor").unwrap_or_else(|| "center".to_string());
    let anchor = scene_anchor_dir(&anchor_text);
    let size_ratio = scene_field_vec2(data, "size_ratio").unwrap_or(Vector2::ZERO);
    let translation = scene_field_vec2(data, "translation_ratio").unwrap_or(Vector2::ZERO);
    let rotation = scene_field_f32(data, "rotation").unwrap_or(0.0);
    let size = Vector2::new(parent.size.x * size_ratio.x, parent.size.y * size_ratio.y);
    if size.x <= 0.0 || size.y <= 0.0 {
        return None;
    }
    let anchor_point = parent.center + Vector2::new(parent.size.x * 0.5 * anchor.x, parent.size.y * 0.5 * anchor.y);
    let inward = Vector2::new(size.x * 0.5 * anchor.x, size.y * 0.5 * anchor.y);
    let offset = Vector2::new(parent.size.x * translation.x, parent.size.y * translation.y);
    Some(EditorUiRect {
        center: anchor_point - inward + offset,
        size,
        rotation,
    })
}

fn doc_ui_parent_rect(doc: &SceneDoc, key: u32) -> Option<EditorUiRect> {
    let root_rect = EditorUiRect {
        center: Vector2::new(0.5, 0.5),
        size: Vector2::ONE,
        rotation: 0.0,
    };
    let node = doc.scene.nodes.iter().find(|node| node.key.as_u32() == key)?;
    let Some(parent) = node.parent else {
        return Some(root_rect);
    };
    doc_ui_rect(doc, parent.as_u32()).or(Some(root_rect))
}

fn doc_ui_rect(doc: &SceneDoc, key: u32) -> Option<EditorUiRect> {
    let root_rect = EditorUiRect {
        center: Vector2::new(0.5, 0.5),
        size: Vector2::ONE,
        rotation: 0.0,
    };
    let node = doc.scene.nodes.iter().find(|node| node.key.as_u32() == key)?;
    let parent = node
        .parent
        .and_then(|parent| doc_ui_rect(doc, parent.as_u32()))
        .unwrap_or(root_rect);
    editor_ui_rect(&node.data, parent)
}

fn scene_anchor_dir(anchor: &str) -> Vector2 {
    match anchor {
        "left" => Vector2::new(-1.0, 0.0),
        "right" => Vector2::new(1.0, 0.0),
        "top" => Vector2::new(0.0, 1.0),
        "bottom" => Vector2::new(0.0, -1.0),
        "top_left" | "top-left" => Vector2::new(-1.0, 1.0),
        "top_right" | "top-right" => Vector2::new(1.0, 1.0),
        "bottom_left" | "bottom-left" => Vector2::new(-1.0, -1.0),
        "bottom_right" | "bottom-right" => Vector2::new(1.0, -1.0),
        _ => Vector2::ZERO,
    }
}

fn scene_field(data: &SceneNodeData, field: &str) -> Option<SceneValue> {
    for (name, value) in data.fields.iter() {
        if name.as_ref() == field {
            return Some(value.clone());
        }
    }
    data.base_ref().and_then(|base| scene_field(base, field))
}

fn scene_field_bool(data: &SceneNodeData, field: &str) -> Option<bool> {
    scene_field(data, field)?.as_bool()
}

fn scene_field_str(data: &SceneNodeData, field: &str) -> Option<String> {
    scene_field(data, field)?.as_str().map(str::to_string)
}

fn scene_field_vec2(data: &SceneNodeData, field: &str) -> Option<Vector2> {
    scene_field(data, field)?
        .as_vec2()
        .map(|(x, y)| Vector2::new(x, y))
}

fn scene_field_f32(data: &SceneNodeData, field: &str) -> Option<f32> {
    scene_field(data, field)?.as_f32()
}

fn set_scene_vec2(data: &mut SceneNodeData, field: &str, value: Vector2) {
    let name = SceneFieldName::from_name(field.to_string());
    for (field_name, field_value) in data.fields.to_mut().iter_mut() {
        if field_name.as_ref() == field {
            *field_value = SceneValue::Vec2 {
                x: value.x,
                y: value.y,
            };
            return;
        }
    }
    data.fields.to_mut().push((
        name,
        SceneValue::Vec2 {
            x: value.x,
            y: value.y,
        },
    ));
}

fn set_scene_f32(data: &mut SceneNodeData, field: &str, value: f32) {
    let name = SceneFieldName::from_name(field.to_string());
    for (field_name, field_value) in data.fields.to_mut().iter_mut() {
        if field_name.as_ref() == field {
            *field_value = SceneValue::F32(value);
            return;
        }
    }
    data.fields.to_mut().push((name, SceneValue::F32(value)));
}

fn refresh_all<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let view = with_state!(ctx.run, EditorState, ctx.id, EditorView::from_state);

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
        set_label(ctx, &format!("manager_recent_{idx}_label"), &editor_view::short_path(&text, 44));
    }
    set_label(
        ctx,
        "create_location_label",
        &format!("location: {}", editor_view::short_path(&view.create_parent_dir, 34)),
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
            .map(|path| format!("{}  {}", editor_files::kind_label(path), editor_files::rel_label(path)))
            .unwrap_or_else(|| "-".to_string());
        set_label(ctx, &format!("file_row_{idx}_label"), &editor_view::short_path(&text, 28));
    }
    apply_file_tree_layout(ctx);

    for idx in 0..MAX_TABS {
        let text = view
            .open_paths
            .get(idx)
            .map(|path| editor_files::rel_label(path))
            .unwrap_or_else(|| "-".to_string());
        set_label(ctx, &format!("scene_tab_{idx}_label"), &editor_view::short_path(&text, 24));
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
    apply_scene_list_layout(ctx);
    apply_viewport_mode(ctx, &view.viewport_mode);
    apply_editor_gizmos(ctx, &view.gizmo, &view.viewport_mode);
    apply_selected_ui_overlay(ctx, view.selected_ui_rect);

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
    viewport_mode: String,
    gizmo: editor_gizmos::GizmoView,
    selected_ui_rect: Option<EditorUiRect>,
    node_picker_rows: Vec<String>,
    node_picker_page: String,
}

impl EditorView {
    fn from_state(state: &EditorState) -> Self {
        let mut nodes = Vec::new();
        let mut selected_row = None;
        let mut inspector_name = "-".to_string();
        let mut inspector_type = "-".to_string();
        let mut inspector_parent = "-".to_string();
        let mut inspector_pos = "-".to_string();
        let mut inspector_script = "-".to_string();
        let mut inspector_vars = "-".to_string();
        let mut gizmo = editor_gizmos::GizmoView::default();
        let mut selected_ui_rect = None;

        if !state.doc_text.is_empty() {
            let doc = SceneDoc::parse(&state.doc_text);
            let tree = scene_tree_view(&doc, state.selected_key);
            gizmo = editor_gizmos::gizmo_view(&doc, state.selected_key);
            selected_ui_rect = state.selected_key.and_then(|key| doc_ui_rect(&doc, key));
            nodes = tree.labels;
            selected_row = tree.selected_row;

            if let Some(key) = state.selected_key.and_then(|raw| {
                doc.scene
                    .nodes
                    .iter()
                    .find(|node| node.key.as_u32() == raw)
                    .map(|node| node.key)
            }) && let Some(node) = doc.scene.nodes.iter().find(|node| node.key == key)
            {
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
            viewport_mode: state.viewport_mode.clone(),
            gizmo,
            selected_ui_rect,
            node_picker_rows,
            node_picker_page: format!("page {page}/{page_count}"),
        }
    }
}

#[derive(Default)]
struct SceneTreeRows {
    labels: Vec<String>,
    keys: Vec<u32>,
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
    for child in doc
        .scene
        .nodes
        .iter()
        .filter(|child| child.parent.map(|parent| parent.as_u32()) == Some(key))
    {
        let _ = push_scene_tree_row(doc, child.key.as_u32(), selected_key, visited, out);
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

fn rewrite_project_res_paths(doc: &SceneDoc, project_root: &str) -> SceneDoc {
    let mut doc = doc.clone();
    for node in doc.scene.nodes.to_mut().iter_mut() {
        if let Some(script) = node.script.as_mut()
            && script.starts_with("res://")
        {
            *script = Cow::Owned(res_to_abs(project_root, script));
        }
        if let Some(root_of) = node.root_of.as_mut()
            && root_of.starts_with("res://")
        {
            *root_of = Cow::Owned(res_to_abs(project_root, root_of));
        }
        rewrite_project_res_data(&mut node.data, project_root);
        for (_, value) in node.script_vars.to_mut().iter_mut() {
            rewrite_project_res_value(value, project_root);
        }
    }
    doc
}

fn rewrite_project_res_data(data: &mut SceneNodeData, project_root: &str) {
    for (_, value) in data.fields.to_mut().iter_mut() {
        rewrite_project_res_value(value, project_root);
    }
    if let Some(base) = data.base.as_mut() {
        match base {
            perro_scene::SceneNodeDataBase::Borrowed(_) => {}
            perro_scene::SceneNodeDataBase::Owned(base) => rewrite_project_res_data(base, project_root),
        }
    }
}

fn rewrite_project_res_value(value: &mut SceneValue, project_root: &str) {
    match value {
        SceneValue::Str(path) if path.starts_with("res://") => {
            *path = Cow::Owned(res_to_abs(project_root, path));
        }
        SceneValue::Object(fields) => {
            for (_, value) in fields.to_mut().iter_mut() {
                rewrite_project_res_value(value, project_root);
            }
        }
        SceneValue::Array(values) => {
            for value in values.to_mut().iter_mut() {
                rewrite_project_res_value(value, project_root);
            }
        }
        _ => {}
    }
}

fn suffix_index(name: &str, prefix: &str) -> Option<usize> {
    name.strip_prefix(prefix)?.parse::<usize>().ok()
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

fn apply_viewport_mode<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, mode: &str) {
    set_grid_visible(ctx, "viewport_grid", mode == "UI");
    set_camera_stream_visible(ctx, "viewport_stream_2d", mode == "2D");
    set_camera_stream_visible(ctx, "viewport_stream_3d", mode == "3D");
    set_panel_display(ctx, "viewport_canvas_overlay", mode == "UI" || mode == "2D");
    apply_viewport_canvas(ctx);
}

fn apply_editor_gizmos<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    gizmo: &editor_gizmos::GizmoView,
    mode: &str,
) {
    let show_2d = mode == "2D";
    let show_3d = mode == "3D";
    set_panel_visible(ctx, "selected_outline", gizmo.selected && !show_3d);
    set_panel_visible(ctx, "camera2d_gizmo", gizmo.camera_2d && show_2d);
    set_panel_visible(ctx, "camera3d_gizmo", gizmo.camera_3d && show_3d);
    if gizmo.selected && !show_3d {
        set_panel_size(ctx, "selected_outline", gizmo.outline_size);
    }
}

fn apply_selected_ui_overlay<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    rect: Option<EditorUiRect>,
) {
    let Some(rect) = rect else {
        set_resize_handles_visible(ctx, false);
        return;
    };
    set_resize_handles_visible(ctx, true);
    if let Some(id) = find_named(ctx, "selected_outline") {
        let _ = with_node_mut!(ctx.run, UiPanel, id, |node| {
            node.layout.size = UiVector2::ratio(rect.size.x * 0.94, rect.size.y * 0.82);
            node.transform.translation = Vector2::new((rect.center.x - 0.5) * 0.94, (rect.center.y - 0.5) * 0.82);
            node.transform.rotation = rect.rotation;
        });
    }
}

fn set_resize_handles_visible<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, visible: bool) {
    for name in [
        "resize_nw",
        "resize_n",
        "resize_ne",
        "resize_w",
        "resize_e",
        "resize_sw",
        "resize_s",
        "resize_se",
    ] {
        set_panel_visible(ctx, name, visible);
    }
}

fn set_panel_visible<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, name: &str, visible: bool) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiPanel, id, |node| {
            node.visible = visible;
            node.input_enabled = visible;
        });
    }
}

fn set_panel_display<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, name: &str, visible: bool) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiPanel, id, |node| {
            node.visible = visible;
            node.input_enabled = false;
        });
    }
}

fn set_grid_visible<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, name: &str, visible: bool) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiGrid, id, |node| {
            node.visible = visible;
            node.input_enabled = visible;
        });
    }
}

fn set_camera_stream_visible<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, name: &str, visible: bool) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiCameraStream, id, |node| {
            node.visible = visible;
            node.input_enabled = visible;
        });
    }
}

fn set_viewport_stream_camera<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, name: &str, camera: NodeID) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiCameraStream, id, |node| {
            node.stream.camera = camera;
        });
    }
}

fn set_panel_size<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, name: &str, size: (f32, f32)) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiPanel, id, |node| {
            node.layout.size = UiVector2::ratio(size.0, size.1);
        });
    }
}

fn apply_viewport_canvas<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let (mode, pan_x, pan_y, zoom) = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.viewport_mode == "2D" {
            (
                state.viewport_mode.clone(),
                -state.cam2_x / 960.0,
                state.cam2_y / 540.0,
                state.cam2_zoom.max(0.05),
            )
        } else {
            (
                state.viewport_mode.clone(),
                state.ui_canvas_x,
                state.ui_canvas_y,
                state.ui_canvas_zoom.max(0.25),
            )
        }
    });
    let show = mode == "UI" || mode == "2D";
    set_panel_display(ctx, "viewport_canvas_overlay", show);
    if !show {
        return;
    }

    let spacing = if mode == "UI" {
        (0.25 * zoom).clamp(0.04, 0.5)
    } else {
        (0.125 * zoom).clamp(0.03, 0.4)
    };
    for i in 0..9 {
        let offset = (i as f32 - 4.0) * spacing;
        set_canvas_line(ctx, &format!("canvas_v_{i}"), true, offset + pan_x, false);
        set_canvas_line(ctx, &format!("canvas_h_{i}"), false, offset + pan_y, false);
    }
    set_canvas_line(ctx, "canvas_origin_x", false, pan_y, true);
    set_canvas_line(ctx, "canvas_origin_y", true, pan_x, true);
}

fn set_canvas_line<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    vertical: bool,
    offset: f32,
    origin: bool,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiPanel, id, |node| {
            node.visible = offset.abs() <= 0.55 || origin;
            node.input_enabled = false;
            node.layout.size = if vertical {
                UiVector2::ratio(if origin { 0.003 } else { 0.0015 }, 1.0)
            } else {
                UiVector2::ratio(1.0, if origin { 0.003 } else { 0.0015 })
            };
            node.transform.translation = if vertical {
                Vector2::new(offset, 0.0)
            } else {
                Vector2::new(0.0, offset)
            };
        });
    }
}

fn apply_scene_list_layout<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(list_id) = find_named(ctx, "scene_rows") else {
        return;
    };
    let _ = with_node_mut!(ctx.run, UiList, list_id, |list| {
        list.indent = 18.0;
        list.v_spacing = 0.006;
    });
}

fn apply_file_tree_layout<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(list_id) = find_named(ctx, "file_rows") else {
        return;
    };
    let _ = with_node_mut!(ctx.run, UiList, list_id, |list| {
        list.indent = 16.0;
        list.v_spacing = 0.006;
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


