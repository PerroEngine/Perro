use perro_api::prelude::*;
use perro_api::scene::{
    SceneDoc, SceneFieldName, SceneKey, SceneNodeData, SceneNodeEntry, SceneValue, SceneValueKey,
};
use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

mod editor_app;
mod editor_file_watch;
mod editor_files;
mod editor_gizmos;
mod editor_manager;
mod editor_project;
mod editor_scene;
mod editor_scene_deps;
mod editor_view;

type SelfNodeType = UiPanel;

const MAX_FILES: usize = 12;
const MAX_NODES: usize = 12;
const MAX_TABS: usize = 4;
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
    file_filter: String,
    file_scope: String,
    scene_paths: Vec<String>,
    open_paths: Vec<String>,
    active_asset_path: String,
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
    collapsed_scene_keys: Vec<u32>,
    copied_node_key: Option<u32>,
    ui_drag_key: Option<u32>,
    ui_drag_mode: String,
    ui_drag_last_x: f32,
    ui_drag_last_y: f32,
    viewport_mode: String,
    dirty: bool,
    add_node_popup_open: bool,
    add_node_as_sibling: bool,
    scene_filter: String,
    node_picker_offset: usize,
    node_picker_filter: String,
    recent_node_types: Vec<String>,
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
    activity_mode: String,
    sidebar_mode: String,
    anim_drawer_open: bool,
    active_anim_path: String,
    active_anim_player_key: Option<u32>,
    active_glb_path: String,
    active_glb_summary: String,
    active_glb_mesh_index: usize,
    active_glb_mat_index: usize,
    active_glb_anim_index: usize,
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
            state.activity_mode = "scene".to_string();
            state.sidebar_mode = "scene".to_string();
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
        update_editor_shortcuts(ctx);
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
            "add_node_button" => open_add_node_popup(ctx),
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
            "activity_scene_button" => set_activity_mode(ctx, "scene"),
            "activity_files_button" => set_sidebar_mode(ctx, "files"),
            "activity_anim_button" => set_activity_mode(ctx, "anim"),
            "scene_filter_box" => update_scene_filter(ctx),
            "file_filter_box" => update_file_filter(ctx),
            "file_new_scene_button" => create_quick_asset(ctx, "scene"),
            "file_new_script_button" => create_quick_asset(ctx, "script"),
            "file_new_anim_button" => create_quick_asset(ctx, "anim"),
            "file_new_mat_button" => create_quick_asset(ctx, "mat"),
            "file_new_folder_button" => create_quick_folder(ctx),
            "anim_create_button" => create_animation_for_selected_player(ctx),
            "anim_add_track_button" => add_track_for_selected_node(ctx),
            "anim_close_button" => set_anim_drawer(ctx, false),
            "inspector_duplicate_button" => duplicate_selected_node(ctx),
            "inspector_delete_button" => delete_selected_node(ctx),
            "inspector_open_ref_button" => open_selected_node_asset_ref(ctx),
            "inspector_visible_button" => toggle_selected_visible(ctx),
            "asset_use_button" => use_active_asset_on_selected_node(ctx),
            "asset_make_node_button" => make_node_from_active_asset(ctx),
            "asset_glb_anim_button" => export_selected_glb_animation(ctx),
            "asset_glb_mat_button" => export_selected_glb_material(ctx),
            "inspector_name_box" => rename_selected_node(ctx),
            "inspector_position_box" => {
                edit_selected_transform(ctx, "position", "inspector_position_box")
            }
            "inspector_rotation_box" => {
                edit_selected_transform(ctx, "rotation", "inspector_rotation_box")
            }
            "inspector_scale_box" => edit_selected_transform(ctx, "scale", "inspector_scale_box"),
            "add_node_search_box" => update_node_picker_filter(ctx),
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
                } else if let Some(idx) = suffix_index(&name, "scene_tab_close_") {
                    close_scene_tab(ctx, idx);
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
            signal!("editor_activity_scene"),
            signal!("editor_activity_files"),
            signal!("editor_activity_anim"),
            signal!("editor_scene_filter"),
            signal!("editor_file_filter"),
            signal!("editor_file_new_scene"),
            signal!("editor_file_new_script"),
            signal!("editor_file_new_anim"),
            signal!("editor_file_new_mat"),
            signal!("editor_file_new_folder"),
            signal!("editor_anim_create"),
            signal!("editor_anim_add_track"),
            signal!("editor_anim_close"),
            signal!("editor_inspector_duplicate"),
            signal!("editor_inspector_delete"),
            signal!("editor_inspector_open_ref"),
            signal!("editor_inspector_visible"),
            signal!("editor_asset_use"),
            signal!("editor_asset_make_node"),
            signal!("editor_asset_glb_anim"),
            signal!("editor_asset_glb_mat"),
            signal!("editor_inspector_rename"),
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
            signal!("editor_tab_2"),
            signal!("editor_tab_3"),
            signal!("editor_tab_close_0"),
            signal!("editor_tab_close_1"),
            signal!("editor_tab_close_2"),
            signal!("editor_tab_close_3"),
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
            signal!("editor_add_node_search"),
            signal!("editor_viewport_click"),
            signal!("editor_inspector_position"),
            signal!("editor_inspector_rotation"),
            signal!("editor_inspector_scale"),
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

    let stream_id = find_named(ctx, "viewport_stream_3d")
        .map(NodeID::as_u64)
        .unwrap_or(0);
    let mut label = String::new();
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let speed = if key_down!(ctx.ipt, KeyCode::ControlLeft) {
            18.0
        } else {
            7.0
        };
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
            "Viewport  mode={}  cam=({:.1}, {:.1}, {:.1}) yaw={:.2} pitch={:.2} stream={} cam_id={}\nkeys: WASD move  Space/Shift up/down  MMB look  F frame  Alt+Click copy-place",
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

fn update_editor_shortcuts<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let ctrl = key_down!(ctx.ipt, KeyCode::ControlLeft)
        || key_down!(ctx.ipt, KeyCode::ControlRight);
    let alt = key_down!(ctx.ipt, KeyCode::AltLeft) || key_down!(ctx.ipt, KeyCode::AltRight);
    let shift = key_down!(ctx.ipt, KeyCode::ShiftLeft) || key_down!(ctx.ipt, KeyCode::ShiftRight);
    if key_pressed!(ctx.ipt, KeyCode::Escape) {
        handle_editor_escape(ctx);
        return;
    }
    let picker_open = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.add_node_popup_open
    });
    if picker_open && key_pressed!(ctx.ipt, KeyCode::ArrowUp) {
        nudge_node_picker(ctx, -1);
        return;
    }
    if picker_open && key_pressed!(ctx.ipt, KeyCode::ArrowDown) {
        nudge_node_picker(ctx, 1);
        return;
    }
    if picker_open && key_pressed!(ctx.ipt, KeyCode::PageUp) {
        shift_node_picker(ctx, -1);
        return;
    }
    if picker_open && key_pressed!(ctx.ipt, KeyCode::PageDown) {
        shift_node_picker(ctx, 1);
        return;
    }
    if picker_open && key_pressed!(ctx.ipt, KeyCode::Home) {
        set_node_picker_edge(ctx, false);
        return;
    }
    if picker_open && key_pressed!(ctx.ipt, KeyCode::End) {
        set_node_picker_edge(ctx, true);
        return;
    }
    if picker_open && key_pressed!(ctx.ipt, KeyCode::Enter) {
        add_node_from_picker(ctx, 0);
        return;
    }
    if picker_open && !ctrl && key_pressed!(ctx.ipt, KeyCode::Tab) {
        toggle_add_node_insert_mode(ctx);
        return;
    }
    if picker_open && !ctrl && !alt && !shift && key_pressed!(ctx.ipt, KeyCode::Digit1) {
        set_node_picker_filter_text(ctx, "@2d");
        return;
    }
    if picker_open && !ctrl && !alt && !shift && key_pressed!(ctx.ipt, KeyCode::Digit2) {
        set_node_picker_filter_text(ctx, "@3d");
        return;
    }
    if picker_open && !ctrl && !alt && !shift && key_pressed!(ctx.ipt, KeyCode::Digit3) {
        set_node_picker_filter_text(ctx, "@ui");
        return;
    }
    if picker_open && !ctrl && !alt && !shift && key_pressed!(ctx.ipt, KeyCode::Digit4) {
        set_node_picker_filter_text(ctx, "@mesh");
        return;
    }
    if picker_open && !ctrl && !alt && !shift && key_pressed!(ctx.ipt, KeyCode::Digit5) {
        set_node_picker_filter_text(ctx, "@anim");
        return;
    }
    if picker_open && !ctrl && !alt && !shift && key_pressed!(ctx.ipt, KeyCode::Digit6) {
        set_node_picker_filter_text(ctx, "@recent");
        return;
    }
    if !picker_open && !ctrl && !alt && !shift && key_pressed!(ctx.ipt, KeyCode::Backspace) {
        nav_sidebar_parent(ctx);
        return;
    }
    if !picker_open && !ctrl && !alt && !shift && key_pressed!(ctx.ipt, KeyCode::Digit1) {
        set_mode(ctx, "2D");
        return;
    }
    if !picker_open && !ctrl && !alt && !shift && key_pressed!(ctx.ipt, KeyCode::Digit2) {
        set_mode(ctx, "3D");
        return;
    }
    if !picker_open && !ctrl && !alt && !shift && key_pressed!(ctx.ipt, KeyCode::Digit3) {
        set_mode(ctx, "UI");
        return;
    }
    if !picker_open
        && !ctrl
        && !alt
        && (key_pressed!(ctx.ipt, KeyCode::Equal)
            || key_pressed!(ctx.ipt, KeyCode::NumpadAdd))
    {
        zoom_active_viewport(ctx, 1);
        return;
    }
    if !picker_open
        && !ctrl
        && !alt
        && (key_pressed!(ctx.ipt, KeyCode::Minus)
            || key_pressed!(ctx.ipt, KeyCode::NumpadSubtract))
    {
        zoom_active_viewport(ctx, -1);
        return;
    }
    if !picker_open && !ctrl && !alt && key_pressed!(ctx.ipt, KeyCode::Digit0) {
        reset_active_viewport_zoom(ctx);
        return;
    }
    if !picker_open && !alt && !shift && !ctrl && key_pressed!(ctx.ipt, KeyCode::BracketLeft) {
        cycle_active_glb_ref(ctx, "mesh", -1);
        return;
    }
    if !picker_open && !alt && !shift && !ctrl && key_pressed!(ctx.ipt, KeyCode::BracketRight) {
        cycle_active_glb_ref(ctx, "mesh", 1);
        return;
    }
    if !picker_open && !alt && shift && !ctrl && key_pressed!(ctx.ipt, KeyCode::BracketLeft) {
        cycle_active_glb_ref(ctx, "mat", -1);
        return;
    }
    if !picker_open && !alt && shift && !ctrl && key_pressed!(ctx.ipt, KeyCode::BracketRight) {
        cycle_active_glb_ref(ctx, "mat", 1);
        return;
    }
    if !picker_open && !alt && shift && ctrl && key_pressed!(ctx.ipt, KeyCode::BracketLeft) {
        cycle_active_glb_ref(ctx, "animation", -1);
        return;
    }
    if !picker_open && !alt && shift && ctrl && key_pressed!(ctx.ipt, KeyCode::BracketRight) {
        cycle_active_glb_ref(ctx, "animation", 1);
        return;
    }
    if ctrl && alt && key_pressed!(ctx.ipt, KeyCode::Digit1) {
        add_node(ctx, "Node2D");
        return;
    }
    if ctrl && alt && key_pressed!(ctx.ipt, KeyCode::Digit2) {
        add_node(ctx, "Sprite2D");
        return;
    }
    if ctrl && alt && key_pressed!(ctx.ipt, KeyCode::Digit3) {
        add_node(ctx, "Node3D");
        return;
    }
    if ctrl && alt && key_pressed!(ctx.ipt, KeyCode::Digit4) {
        add_node(ctx, "MeshInstance3D");
        return;
    }
    if ctrl && alt && key_pressed!(ctx.ipt, KeyCode::Digit5) {
        add_node(ctx, "AnimationPlayer");
        return;
    }
    if ctrl && alt && key_pressed!(ctx.ipt, KeyCode::Digit6) {
        add_node(ctx, "UiPanel");
        return;
    }
    if ctrl && alt && key_pressed!(ctx.ipt, KeyCode::Digit7) {
        add_camera_for_active_view(ctx);
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::Digit1) {
        set_activity_mode(ctx, "scene");
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::Digit2) {
        set_sidebar_mode(ctx, "files");
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::Digit3) {
        set_activity_mode(ctx, "anim");
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::KeyB) {
        cycle_sidebar_panel(ctx);
        return;
    }
    if ctrl && shift && key_pressed!(ctx.ipt, KeyCode::KeyN) {
        open_add_node_sibling_popup(ctx);
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::KeyN) {
        open_add_node_popup(ctx);
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::Tab) {
        cycle_scene_tab(ctx, 1);
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::BracketLeft) {
        cycle_scene_tab(ctx, -1);
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::BracketRight) {
        cycle_scene_tab(ctx, 1);
        return;
    }
    if ctrl && shift && key_pressed!(ctx.ipt, KeyCode::KeyW) {
        close_all_scene_tabs(ctx);
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::KeyW) {
        close_active_scene_tab(ctx);
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::KeyC) {
        copy_selected_node(ctx);
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::KeyV) {
        paste_copied_node(ctx);
        return;
    }
    if alt && key_pressed!(ctx.ipt, KeyCode::ArrowUp) {
        move_selected_node_order(ctx, -1);
        return;
    }
    if alt && key_pressed!(ctx.ipt, KeyCode::ArrowDown) {
        move_selected_node_order(ctx, 1);
        return;
    }
    if alt && key_pressed!(ctx.ipt, KeyCode::ArrowLeft) {
        reparent_selected_node(ctx, -1);
        return;
    }
    if alt && key_pressed!(ctx.ipt, KeyCode::ArrowRight) {
        reparent_selected_node(ctx, 1);
        return;
    }
    if alt && key_pressed!(ctx.ipt, KeyCode::KeyR) {
        reset_selected_transform(ctx);
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::ArrowUp) {
        select_related_node(ctx, "parent");
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::ArrowDown) {
        select_related_node(ctx, "child");
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::ArrowLeft) {
        select_related_node(ctx, "prev");
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::ArrowRight) {
        select_related_node(ctx, "next");
        return;
    }
    if !ctrl && !alt && !shift && key_pressed!(ctx.ipt, KeyCode::ArrowLeft) {
        collapse_selected_scene_node(ctx);
        return;
    }
    if !ctrl && !alt && !shift && key_pressed!(ctx.ipt, KeyCode::ArrowRight) {
        expand_selected_scene_node(ctx);
        return;
    }
    if shift && key_pressed!(ctx.ipt, KeyCode::ArrowLeft) {
        nudge_selected_node(ctx, -1.0, 0.0, ctrl);
        return;
    }
    if shift && key_pressed!(ctx.ipt, KeyCode::ArrowRight) {
        nudge_selected_node(ctx, 1.0, 0.0, ctrl);
        return;
    }
    if shift && key_pressed!(ctx.ipt, KeyCode::ArrowUp) {
        nudge_selected_node(ctx, 0.0, 1.0, ctrl);
        return;
    }
    if shift && key_pressed!(ctx.ipt, KeyCode::ArrowDown) {
        nudge_selected_node(ctx, 0.0, -1.0, ctrl);
        return;
    }
    if !ctrl && key_pressed!(ctx.ipt, KeyCode::ArrowUp) {
        select_sidebar_delta(ctx, -1);
        return;
    }
    if !ctrl && key_pressed!(ctx.ipt, KeyCode::ArrowDown) {
        select_sidebar_delta(ctx, 1);
        return;
    }
    if !ctrl && !alt && !shift && key_pressed!(ctx.ipt, KeyCode::Home) {
        select_sidebar_edge(ctx, false);
        return;
    }
    if !ctrl && !alt && !shift && key_pressed!(ctx.ipt, KeyCode::End) {
        select_sidebar_edge(ctx, true);
        return;
    }
    if ctrl && shift && key_pressed!(ctx.ipt, KeyCode::Enter) {
        make_node_from_active_asset(ctx);
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::Enter) {
        use_active_asset_on_selected_node(ctx);
        return;
    }
    if key_pressed!(ctx.ipt, KeyCode::Enter) {
        open_sidebar_selection(ctx);
        return;
    }
    if !ctrl && !alt && !shift && key_pressed!(ctx.ipt, KeyCode::KeyF) {
        frame_selected_node(ctx);
        return;
    }
    if ctrl && shift && key_pressed!(ctx.ipt, KeyCode::KeyS) {
        save_all_scenes(ctx);
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::KeyS) {
        save_active_scene(ctx);
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::KeyR) {
        refresh_project_assets(ctx);
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::KeyE) {
        reveal_active_scene_in_files(ctx);
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::KeyF) {
        prepare_sidebar_find(ctx);
        return;
    }
    if ctrl && shift && key_pressed!(ctx.ipt, KeyCode::KeyG) {
        select_node_using_active_asset(ctx);
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::KeyG) {
        open_selected_node_asset_ref(ctx);
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::KeyH) {
        toggle_selected_visible(ctx);
        return;
    }
    if ctrl && key_pressed!(ctx.ipt, KeyCode::KeyD) {
        duplicate_selected_node(ctx);
        return;
    }
    if key_pressed!(ctx.ipt, KeyCode::Delete) {
        delete_selected_node(ctx);
    }
}

fn handle_editor_escape<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let action = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.add_node_popup_open {
            "picker"
        } else if state.anim_drawer_open {
            "anim"
        } else if state.sidebar_mode == "files"
            && (!state.file_filter.is_empty() || !state.file_scope.is_empty())
        {
            "files"
        } else if state.sidebar_mode == "scene" && !state.scene_filter.is_empty() {
            "scene"
        } else {
            "none"
        }
    });
    match action {
        "picker" => set_add_node_popup(ctx, false),
        "anim" => set_anim_drawer(ctx, false),
        "files" => {
            let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                state.file_filter.clear();
                state.file_scope.clear();
                state.log = "clear files filter".to_string();
            });
            refresh_all(ctx);
        }
        "scene" => {
            let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                state.scene_filter.clear();
                state.log = "clear scene filter".to_string();
            });
            refresh_all(ctx);
        }
        _ => {
            set_add_node_popup(ctx, false);
            set_anim_drawer(ctx, false);
        }
    }
}

fn cycle_sidebar_panel<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.sidebar_mode.clone()
    });
    if mode == "files" {
        set_activity_mode(ctx, "scene");
    } else {
        set_sidebar_mode(ctx, "files");
    }
}

fn prepare_sidebar_find<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.sidebar_mode == "files" {
            state.activity_mode = "files".to_string();
            state.log = "find assets\nuse file search".to_string();
        } else {
            state.sidebar_mode = "scene".to_string();
            state.activity_mode = "scene".to_string();
            state.log = "find nodes\nuse scene search".to_string();
        }
    });
    refresh_all(ctx);
}

fn open_add_node_popup<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.node_picker_offset = 0;
        state.node_picker_filter.clear();
        state.add_node_as_sibling = false;
    });
    refresh_all(ctx);
    set_add_node_popup(ctx, true);
}

fn add_camera_for_active_view<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let node_type = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.viewport_mode == "3D" {
            "Camera3D"
        } else {
            "Camera2D"
        }
    });
    add_node(ctx, node_type);
}

fn open_add_node_sibling_popup<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.node_picker_offset = 0;
        state.node_picker_filter.clear();
        state.add_node_as_sibling = true;
    });
    refresh_all(ctx);
    set_add_node_popup(ctx, true);
}

fn toggle_add_node_insert_mode<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.add_node_as_sibling = !state.add_node_as_sibling;
        state.log = if state.add_node_as_sibling {
            "add node mode\nsibling".to_string()
        } else {
            "add node mode\nchild".to_string()
        };
    });
    refresh_all(ctx);
}

fn select_sidebar_delta<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, delta: isize) {
    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.sidebar_mode.clone()
    });
    if mode == "files" {
        select_file_delta(ctx, delta);
    } else {
        select_scene_delta(ctx, delta);
    }
}

fn select_sidebar_edge<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, last: bool) {
    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.sidebar_mode.clone()
    });
    if mode == "files" {
        select_file_edge(ctx, last);
    } else {
        select_scene_edge(ctx, last);
    }
}

fn nav_sidebar_parent<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.sidebar_mode.clone()
    });
    if mode == "files" {
        nav_file_scope_parent(ctx);
    } else {
        select_related_node(ctx, "parent");
    }
}

fn select_scene_delta<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, delta: isize) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.doc_text.is_empty() {
            return false;
        }
        let doc = SceneDoc::parse(&state.doc_text);
        let tree = scene_tree_view(
            &doc,
            state.selected_key,
            &state.scene_filter,
            &state.collapsed_scene_keys,
        );
        if tree.keys.is_empty() {
            return false;
        }
        let current = tree
            .selected_row
            .unwrap_or_else(|| tree.keys.iter().position(|key| Some(*key) == state.selected_key).unwrap_or(0));
        let next = offset_index(current, tree.keys.len(), delta);
        let key = tree.keys[next];
        state.selected_key = Some(key);
        state.sidebar_mode = "scene".to_string();
        if let Some(mode) = selected_node_viewport_mode(&state.doc_text, key) {
            state.viewport_mode = mode.to_string();
        }
        if selected_node_type_name(&state.doc_text, key).as_deref() == Some("AnimationPlayer") {
            state.activity_mode = "anim".to_string();
            state.anim_drawer_open = true;
            state.active_anim_player_key = Some(key);
            state.active_glb_path.clear();
            state.active_glb_summary.clear();
            if let Some(path) = selected_node_field_text(&state.doc_text, key, "animation") {
                state.active_anim_path = path;
            }
        }
        state.log = format!("select node\n{}", doc.scene.key_name_or_id(SceneKey::new(key)));
        true
    })
    .unwrap_or(false);
    if changed {
        refresh_all(ctx);
    }
}

fn select_scene_edge<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, last: bool) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.doc_text.is_empty() {
            return false;
        }
        let doc = SceneDoc::parse(&state.doc_text);
        let tree = scene_tree_view(
            &doc,
            state.selected_key,
            &state.scene_filter,
            &state.collapsed_scene_keys,
        );
        if tree.keys.is_empty() {
            return false;
        }
        let key = if last {
            *tree.keys.last().unwrap_or(&tree.keys[0])
        } else {
            tree.keys[0]
        };
        state.selected_key = Some(key);
        state.sidebar_mode = "scene".to_string();
        state.activity_mode = "scene".to_string();
        if let Some(mode) = selected_node_viewport_mode(&state.doc_text, key) {
            state.viewport_mode = mode.to_string();
        }
        state.log = format!("select node\n{}", doc.scene.key_name_or_id(SceneKey::new(key)));
        true
    })
    .unwrap_or(false);
    if changed {
        refresh_all(ctx);
    }
}

fn select_related_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, relation: &str) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
        if state.doc_text.is_empty() {
            return false;
        }
        let doc = SceneDoc::parse(&state.doc_text);
        let Some(node) = doc.scene.nodes.iter().find(|node| node.key.as_u32() == key) else {
            return false;
        };
        let next = match relation {
            "parent" => node.parent.map(|parent| parent.as_u32()),
            "child" => doc
                .scene
                .nodes
                .iter()
                .find(|child| child.parent.map(|parent| parent.as_u32()) == Some(key))
                .map(|child| child.key.as_u32()),
            "prev" | "next" => {
                let parent = node.parent.map(|parent| parent.as_u32());
                let siblings = doc
                    .scene
                    .nodes
                    .iter()
                    .filter(|item| item.parent.map(|parent| parent.as_u32()) == parent)
                    .map(|item| item.key.as_u32())
                    .collect::<Vec<_>>();
                let Some(pos) = siblings.iter().position(|item| *item == key) else {
                    return false;
                };
                if relation == "prev" {
                    pos.checked_sub(1).and_then(|idx| siblings.get(idx).copied())
                } else {
                    siblings.get(pos + 1).copied()
                }
            }
            _ => None,
        };
        let Some(next) = next else {
            state.log = format!("select {relation}\nnone");
            return false;
        };
        state.selected_key = Some(next);
        state.sidebar_mode = "scene".to_string();
        state.activity_mode = "scene".to_string();
        if let Some(mode) = selected_node_viewport_mode(&state.doc_text, next) {
            state.viewport_mode = mode.to_string();
        }
        state.log = format!(
            "select {relation}\n{}",
            doc.scene.key_name_or_id(SceneKey::new(next))
        );
        true
    })
    .unwrap_or(false);
    if changed {
        refresh_all(ctx);
    }
}

fn collapse_selected_scene_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.sidebar_mode != "scene" || !state.scene_filter.is_empty() {
            return false;
        }
        let Some(key) = state.selected_key else {
            return false;
        };
        let doc = SceneDoc::parse(&state.doc_text);
        if scene_child_count(&doc, key) > 0 && !state.collapsed_scene_keys.contains(&key) {
            state.collapsed_scene_keys.push(key);
            state.log = "collapse node".to_string();
            true
        } else if let Some(parent) = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == key)
            .and_then(|node| node.parent)
        {
            state.selected_key = Some(parent.as_u32());
            state.log = "select parent".to_string();
            true
        } else {
            false
        }
    })
    .unwrap_or(false);
    if changed {
        refresh_all(ctx);
    }
}

fn expand_selected_scene_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.sidebar_mode != "scene" || !state.scene_filter.is_empty() {
            return false;
        }
        let Some(key) = state.selected_key else {
            return false;
        };
        if let Some(pos) = state.collapsed_scene_keys.iter().position(|item| *item == key) {
            state.collapsed_scene_keys.remove(pos);
            state.log = "expand node".to_string();
            return true;
        }
        let doc = SceneDoc::parse(&state.doc_text);
        let Some(child) = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.parent.map(|parent| parent.as_u32()) == Some(key))
        else {
            return false;
        };
        state.selected_key = Some(child.key.as_u32());
        state.log = "select child".to_string();
        true
    })
    .unwrap_or(false);
    if changed {
        refresh_all(ctx);
    }
}

fn select_file_delta<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, delta: isize) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let paths = filtered_file_paths(state);
        if paths.is_empty() {
            return false;
        }
        let current = paths
            .iter()
            .position(|path| path == &state.active_asset_path)
            .unwrap_or(0);
        let next = offset_index(current, paths.len(), delta);
        state.active_asset_path = paths[next].clone();
        state.sidebar_mode = "files".to_string();
        state.activity_mode = "files".to_string();
        state.log = format!("select asset\n{}", editor_files::rel_label(&state.active_asset_path));
        true
    })
    .unwrap_or(false);
    if changed {
        refresh_all(ctx);
    }
}

fn select_file_edge<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, last: bool) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let paths = filtered_file_paths(state);
        if paths.is_empty() {
            return false;
        }
        let idx = if last { paths.len() - 1 } else { 0 };
        state.active_asset_path = paths[idx].clone();
        state.sidebar_mode = "files".to_string();
        state.activity_mode = "files".to_string();
        state.log = format!("select asset\n{}", editor_files::rel_label(&state.active_asset_path));
        true
    })
    .unwrap_or(false);
    if changed {
        refresh_all(ctx);
    }
}

fn nav_file_scope_parent<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.sidebar_mode != "files" || state.file_scope.is_empty() {
            return false;
        }
        let parent = parent_res_folder(&state.file_scope);
        state.file_scope = parent.clone();
        state.active_asset_path = if parent.is_empty() {
            filtered_file_paths(state).first().cloned().unwrap_or_default()
        } else {
            parent.clone()
        };
        state.activity_mode = "files".to_string();
        state.log = if parent.is_empty() {
            "folder\nres://".to_string()
        } else {
            format!("folder\n{}", editor_files::rel_label(&parent))
        };
        true
    })
    .unwrap_or(false);
    if changed {
        refresh_all(ctx);
    }
}

fn open_active_file<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let idx = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.sidebar_mode != "files" {
            return None;
        }
        let paths = filtered_file_paths(state);
        if paths.is_empty() {
            return None;
        }
        paths
            .iter()
            .position(|path| path == &state.active_asset_path)
            .or(Some(0))
    });
    if let Some(idx) = idx {
        open_file_slot(ctx, idx);
    }
}

fn open_sidebar_selection<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.sidebar_mode.clone()
    });
    if mode == "files" {
        open_active_file(ctx);
        return;
    }
    let has_ref = with_state!(ctx.run, EditorState, ctx.id, |state| {
        let key = state.selected_key?;
        if state.doc_text.is_empty() {
            return None;
        }
        let doc = SceneDoc::parse(&state.doc_text);
        let node = doc.scene.nodes.iter().find(|node| node.key.as_u32() == key)?;
        selected_node_asset_ref_path(node)
    })
    .is_some();
    if has_ref {
        open_selected_node_asset_ref(ctx);
    } else {
        frame_selected_node(ctx);
    }
}

fn reveal_active_scene_in_files<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(path) = state.open_paths.get(state.active_open).cloned() else {
            state.log = "reveal file fail\nno active scene".to_string();
            return false;
        };
        state.sidebar_mode = "files".to_string();
        state.activity_mode = "files".to_string();
        state.active_asset_path = path.clone();
        state.file_scope = parent_res_folder(&path);
        state.file_filter.clear();
        state.log = format!("reveal file\n{path}");
        true
    })
    .unwrap_or(false);
    if changed {
        refresh_all(ctx);
    }
}

fn offset_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    if delta < 0 {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        (current + delta as usize).min(len - 1)
    }
}

fn wrap_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    if delta < 0 {
        (current + len - (delta.unsigned_abs() % len)) % len
    } else {
        (current + delta as usize) % len
    }
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
    let stream_id = find_named(ctx, "viewport_stream_2d")
        .map(NodeID::as_u64)
        .unwrap_or(0);
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
            "Viewport  mode={}  cam=({:.1}, {:.1}) zoom={:.2} stream={} cam_id={}\nkeys: WASD pan  +/- zoom  F frame  Shift+Click snap  Alt+Click copy-place",
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
            "Viewport  mode={}  canvas=({:.2}, {:.2}) zoom={:.2}\nkeys: MMB pan  wheel/+/- zoom  0 reset  F frame  click pick  Shift+drag snap",
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
    let _ = ctx
        .run
        .Nodes()
        .set_local_transform_3d(camera, Transform3D::new(pos, rot, Vector3::ONE));
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

fn frame_selected_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let framed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = "frame fail\nselect node".to_string();
            return false;
        };
        if state.doc_text.is_empty() {
            state.log = "frame fail\nno open scene".to_string();
            return false;
        }
        let doc = SceneDoc::parse(&state.doc_text);
        let Some(node) = doc.scene.nodes.iter().find(|node| node.key.as_u32() == key) else {
            state.log = "frame fail\nmissing node".to_string();
            return false;
        };
        let name = doc.scene.key_name_or_id(node.key).to_string();
        if state.viewport_mode == "3D" {
            let point = find_vec3_value(&node.data, "position").unwrap_or(Vector3::ZERO);
            state.cam_x = point.x;
            state.cam_y = point.y + 3.0;
            state.cam_z = point.z + 8.0;
            state.cam_yaw = 0.0;
            state.cam_pitch = -0.32;
            state.log = format!("frame 3d\n{name}");
            return true;
        }
        if state.viewport_mode == "2D" {
            let point = find_vec2_value(&node.data, "position").unwrap_or(Vector2::ZERO);
            state.cam2_x = point.x;
            state.cam2_y = point.y;
            state.cam2_zoom = state.cam2_zoom.max(1.0);
            state.log = format!("frame 2d\n{name}");
            return true;
        }
        state.ui_canvas_x = 0.0;
        state.ui_canvas_y = 0.0;
        state.ui_canvas_zoom = state.ui_canvas_zoom.max(1.0);
        state.log = format!("frame ui\n{name}");
        true
    })
    .unwrap_or(false);
    if framed {
        apply_freecam(ctx);
        apply_freecam_2d(ctx);
        apply_viewport_canvas(ctx);
    }
    refresh_all(ctx);
}

fn open_project<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root: String,
) -> Result<(), String> {
    clear_preview(ctx);
    let root_path = PathBuf::from(&root);
    validate_project_root(&root_path)?;
    let project_text =
        FileMod::load_string(root_path.join("project.toml").to_string_lossy().as_ref())
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
        state.file_scope.clear();
        state.scene_paths = scene_paths;
        state.open_paths.clear();
        state.active_asset_path.clear();
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
        state.collapsed_scene_keys.clear();
        state.ui_drag_key = None;
        state.ui_drag_mode.clear();
        state.ui_drag_last_x = 0.0;
        state.ui_drag_last_y = 0.0;
        reset_freecam_2d(state);
        state.dirty = false;
        state.activity_mode = "scene".to_string();
        state.sidebar_mode = "scene".to_string();
        state.anim_drawer_open = false;
        state.active_anim_path.clear();
        state.active_anim_player_key = None;
        state.active_glb_path.clear();
        state.active_glb_summary.clear();
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

fn load_editor_shell<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
) -> Result<(), String> {
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
            set_log(
                ctx,
                &format!("create project fail\n{parent}\n{name}\n{err}"),
            );
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

fn refresh_project_assets<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let root = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.project_root.clone()
    });
    if root.is_empty() {
        set_log(ctx, "refresh fail\nopen project first");
        refresh_all(ctx);
        return;
    }
    let root_path = PathBuf::from(&root);
    match scan_res_paths(root_path.as_path()) {
        Ok(paths) => {
            let scene_paths = paths
                .iter()
                .filter(|path| path.ends_with(".scn"))
                .cloned()
                .collect::<Vec<_>>();
            let count = paths.len();
            let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                state.file_paths = paths;
                state.scene_paths = scene_paths;
                state.project_file_sigs = editor_file_watch::scan_project(root_path.as_path());
                state.log = format!("refresh project\nassets={count}");
            });
            rebuild_preview(ctx);
        }
        Err(err) => set_log(ctx, &format!("refresh fail\n{err}")),
    }
    refresh_all(ctx);
}

fn open_file_slot<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, idx: usize) {
    let res_path = with_state!(ctx.run, EditorState, ctx.id, |state| {
        filtered_file_paths(state).get(idx).cloned()
    });
    let Some(scene_path) = res_path else {
        return;
    };
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.active_asset_path = scene_path.clone();
        state.sidebar_mode = "files".to_string();
    });
    if scene_path.ends_with('/') {
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            state.file_scope = scene_path.clone();
            state.file_filter.clear();
            state.active_asset_path = scene_path.clone();
            state.activity_mode = "files".to_string();
            state.sidebar_mode = "files".to_string();
            state.log = format!("folder\n{}", editor_files::rel_label(&scene_path));
        });
        set_log(ctx, &format!("folder\n{scene_path}"));
        refresh_all(ctx);
        return;
    }
    if scene_path.ends_with(".panim") {
        open_animation_path(ctx, &scene_path);
        return;
    }
    if is_gltf_path(&scene_path) {
        open_gltf_path(ctx, &scene_path);
        return;
    }
    if !scene_path.ends_with(".scn") {
        set_log(
            ctx,
            &format!(
                "{} file\n{}",
                editor_files::kind_label(&scene_path),
                editor_files::rel_label(&scene_path)
            ),
        );
        refresh_all(ctx);
        return;
    }
    open_scene_path(ctx, &scene_path);
}

fn open_scene_path<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, scene_path: &str) {
    if scene_path.ends_with('/') {
        set_log(
            ctx,
            &format!("folder\n{}", editor_files::rel_label(scene_path)),
        );
        return;
    }
    if !scene_path.ends_with(".scn") {
        set_log(
            ctx,
            &format!(
                "{} file\n{}",
                editor_files::kind_label(scene_path),
                editor_files::rel_label(scene_path)
            ),
        );
        refresh_all(ctx);
        return;
    }
    let blocked = with_state!(ctx.run, EditorState, ctx.id, |state| {
        let active = state.open_paths.get(state.active_open);
        active.is_some_and(|path| {
            path != scene_path && state.dirty_scene_paths.iter().any(|dirty| dirty == path)
        })
    });
    if blocked && !save_active_scene_to_disk(ctx, true) {
        refresh_all(ctx);
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
        state.active_asset_path = scene_path.to_string();
        state.active_open = state
            .open_paths
            .iter()
            .position(|path| path == scene_path)
            .unwrap_or(0);
        state.doc_text = doc.to_text();
        state.selected_key = first_key;
        state.collapsed_scene_keys.clear();
        state.viewport_mode = mode.to_string();
        if mode == "3D" {
            reset_freecam(state);
        } else if mode == "2D" {
            reset_freecam_2d(state);
        }
        state.dirty = false;
        state.dirty_scene_paths.retain(|path| path != scene_path);
        state.active_glb_path.clear();
        state.active_glb_summary.clear();
        state.log = format!("open scene\n{}", editor_files::rel_label(scene_path));
    });
    rebuild_preview(ctx);
    refresh_all(ctx);
}

fn open_animation_path<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, anim_path: &str) {
    let root = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.project_root.clone()
    });
    let abs = res_to_abs(&root, anim_path);
    match FileMod::load_string(&abs) {
        Ok(_) => {
            let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                state.activity_mode = "anim".to_string();
                state.anim_drawer_open = true;
                state.active_anim_player_key = None;
                state.active_anim_path = anim_path.to_string();
                state.active_asset_path = anim_path.to_string();
                state.active_glb_path.clear();
                state.active_glb_summary.clear();
                state.log = format!(
                    "open animation data\n{}",
                    editor_files::rel_label(anim_path)
                );
            });
            refresh_all(ctx);
        }
        Err(err) => set_log(ctx, &format!("open animation fail\n{anim_path}\n{err}")),
    }
}

fn open_gltf_path<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, gltf_path: &str) {
    let Some(info) = ctx.res.Glbs().inspect(gltf_path) else {
        set_log(ctx, &format!("open glb fail\n{gltf_path}"));
        return;
    };
    let summary = gltf_summary(
        gltf_path,
        info.mesh_count,
        info.material_count,
        info.animation_count,
        info.skeleton_count,
        info.texture_count,
        info.node_count,
        info.scene_count,
        0,
        0,
        0,
    );
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.activity_mode = "anim".to_string();
        state.anim_drawer_open = true;
        state.active_anim_player_key = None;
        state.active_anim_path.clear();
        state.active_asset_path = gltf_path.to_string();
        state.active_glb_path = gltf_path.to_string();
        state.active_glb_summary = summary;
        state.active_glb_mesh_index = 0;
        state.active_glb_mat_index = 0;
        state.active_glb_anim_index = 0;
        state.log = format!(
            "open glb\n{}\nmesh={} mat={} anim={}",
            editor_files::rel_label(gltf_path),
            info.mesh_count,
            info.material_count,
            info.animation_count
        );
    });
    refresh_all(ctx);
}

fn cycle_active_glb_ref<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    kind: &str,
    dir: isize,
) {
    let path = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.active_glb_path.is_empty() {
            None
        } else {
            Some(state.active_glb_path.clone())
        }
    });
    let Some(path) = path else {
        return;
    };
    let Some(info) = ctx.res.Glbs().inspect(&path) else {
        set_log(ctx, &format!("glb ref fail\n{path}"));
        return;
    };
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let count = match kind {
            "mesh" => info.mesh_count,
            "mat" => info.material_count,
            "animation" => info.animation_count,
            _ => 0,
        };
        if count == 0 {
            state.log = format!("glb {kind}\nnone");
            return;
        }
        let current = match kind {
            "mesh" => state.active_glb_mesh_index,
            "mat" => state.active_glb_mat_index,
            "animation" => state.active_glb_anim_index,
            _ => 0,
        };
        let next = offset_index(current.min(count - 1), count, dir);
        match kind {
            "mesh" => state.active_glb_mesh_index = next,
            "mat" => state.active_glb_mat_index = next,
            "animation" => state.active_glb_anim_index = next,
            _ => {}
        }
        state.active_glb_summary = gltf_summary(
            &path,
            info.mesh_count,
            info.material_count,
            info.animation_count,
            info.skeleton_count,
            info.texture_count,
            info.node_count,
            info.scene_count,
            state.active_glb_mesh_index,
            state.active_glb_mat_index,
            state.active_glb_anim_index,
        );
        state.log = format!("glb {kind}\n{path}:{kind}[{next}]");
    });
    refresh_all(ctx);
}

fn set_active_tab<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, idx: usize) {
    let needs_save = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state
            .open_paths
            .get(state.active_open)
            .map(|path| {
                idx != state.active_open && state.dirty_scene_paths.iter().any(|dirty| dirty == path)
            })
            .unwrap_or(false)
    });
    if needs_save && !save_active_scene_to_disk(ctx, true) {
        refresh_all(ctx);
        return;
    }
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

fn cycle_scene_tab<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, dir: isize) {
    let idx = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.open_paths.is_empty() {
            return None;
        }
        Some(wrap_index(state.active_open, state.open_paths.len(), dir))
    });
    if let Some(idx) = idx {
        set_active_tab(ctx, idx);
    }
}

fn close_active_scene_tab<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let idx = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (!state.open_paths.is_empty()).then_some(state.active_open)
    });
    if let Some(idx) = idx {
        close_scene_tab(ctx, idx);
    }
}

fn close_all_scene_tabs<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    save_all_scenes(ctx);
    let closed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let closed = state.open_paths.len();
        state.open_paths.clear();
        state.dirty_scene_paths.clear();
        state.active_open = 0;
        state.doc_text.clear();
        state.selected_key = None;
        state.preview_scene_paths.clear();
        state.preview_root = 0;
        state.preview_node_ids.clear();
        state.preview_node_keys.clear();
        state.dirty = false;
        state.log = format!("close all tabs\n{closed}");
        closed
    })
    .unwrap_or(0);
    if closed > 0 {
        clear_preview(ctx);
    }
    refresh_all(ctx);
}

fn close_scene_tab<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, idx: usize) {
    let should_save = with_state!(ctx.run, EditorState, ctx.id, |state| {
        idx == state.active_open
            && state
                .open_paths
                .get(idx)
                .map(|path| state.dirty_scene_paths.iter().any(|dirty| dirty == path))
                .unwrap_or(false)
    });
    if should_save && !save_active_scene_to_disk(ctx, true) {
        refresh_all(ctx);
        return;
    }
    let next = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if idx >= state.open_paths.len() {
            return None;
        }
        let Some(target) = state.open_paths.get(idx).cloned() else {
            return None;
        };
        if state.dirty_scene_paths.iter().any(|path| path == &target) {
            state.log = format!("close blocked\nsave first\n{target}");
            return None;
        }
        let closed = state.open_paths.remove(idx);
        state.dirty_scene_paths.retain(|path| path != &closed);
        if state.open_paths.is_empty() {
            state.active_open = 0;
            state.doc_text.clear();
            state.selected_key = None;
            state.preview_scene_paths.clear();
            state.preview_root = 0;
            state.preview_node_ids.clear();
            state.preview_node_keys.clear();
            state.dirty = false;
            state.log = format!("close tab\n{closed}");
            return Some(None);
        }
        if state.active_open >= state.open_paths.len() {
            state.active_open = state.open_paths.len().saturating_sub(1);
        } else if idx <= state.active_open && state.active_open > 0 {
            state.active_open -= 1;
        }
        let next_path = state.open_paths.get(state.active_open).cloned();
        state.log = format!("close tab\n{closed}");
        Some(next_path)
    })
    .flatten();
    match next {
        Some(Some(path)) => open_scene_path(ctx, &path),
        Some(None) => {
            clear_preview(ctx);
            refresh_all(ctx);
        }
        None => refresh_all(ctx),
    }
}

fn open_first_scene<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let slot = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state
            .file_paths
            .iter()
            .position(|path| path.ends_with(".scn"))
    });
    if let Some(slot) = slot {
        open_file_slot(ctx, slot);
    }
}

fn create_quick_asset<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, kind: &str) {
    let request = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.project_root.is_empty() {
            return None;
        }
        let stem = quick_asset_stem(state, kind);
        let dir = quick_asset_dir(state, kind);
        let (path, text) = match kind {
            "scene" => {
                let path = unique_res_path(&state.project_root, &dir, &stem, "scn");
                (path, default_scene_text(&stem))
            }
            "script" => {
                let path = unique_res_path(&state.project_root, &dir, &stem, "rs");
                (path, default_script_text())
            }
            "anim" => {
                let path = unique_res_path(&state.project_root, &dir, &stem, "panim");
                (path, default_animation_panim(&stem))
            }
            "mat" => {
                let path = unique_res_path(&state.project_root, &dir, &stem, "pmat");
                (path, default_material_pmat())
            }
            _ => return None,
        };
        Some((state.project_root.clone(), path, text, kind.to_string()))
    });
    let Some((root, path, text, kind)) = request else {
        set_log(ctx, "new asset fail\nopen project first");
        return;
    };
    let abs = res_to_abs(&root, &path);
    if let Some(parent) = Path::new(&abs).parent() {
        let _ = fs::create_dir_all(parent);
    }
    match FileMod::save_string(&abs, &text) {
        Ok(()) => {
            let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                if let Ok(paths) = scan_res_paths(Path::new(&state.project_root)) {
                    state.file_paths = paths;
                }
                state.sidebar_mode = "files".to_string();
                state.activity_mode = "files".to_string();
                state.active_asset_path = path.clone();
                state.file_scope = parent_res_folder(&path);
                state.log = format!("new {kind}\n{path}");
            });
            if kind == "scene" {
                open_scene_path(ctx, &path);
            } else if kind == "anim" {
                attach_animation_to_selected_player(ctx, &path);
            } else if kind == "script" {
                attach_script_to_selected_node(ctx, &path);
            } else if kind == "mat" {
                attach_material_to_selected_node(ctx, &path);
            } else {
                refresh_all(ctx);
            }
        }
        Err(err) => set_log(ctx, &format!("new asset fail\n{path}\n{err}")),
    }
}

fn create_quick_folder<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let request = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.project_root.is_empty() {
            return None;
        }
        let dir = quick_asset_dir(state, "folder");
        let path = unique_res_folder_path(&state.project_root, &dir, "new_folder");
        Some((state.project_root.clone(), path))
    });
    let Some((root, path)) = request else {
        set_log(ctx, "new folder fail\nopen project first");
        return;
    };
    let abs = res_to_abs(&root, &path);
    match fs::create_dir_all(&abs) {
        Ok(()) => {
            let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                if let Ok(paths) = scan_res_paths(Path::new(&state.project_root)) {
                    state.file_paths = paths;
                }
                state.sidebar_mode = "files".to_string();
                state.activity_mode = "files".to_string();
                state.active_asset_path = path.clone();
                state.file_scope = parent_res_folder(&path);
                state.log = format!("new folder\n{path}");
            });
            refresh_all(ctx);
        }
        Err(err) => set_log(ctx, &format!("new folder fail\n{path}\n{err}")),
    }
}

fn attach_script_to_selected_node<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    script_path: &str,
) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = format!("new script\n{script_path}");
            return false;
        };
        if state.doc_text.is_empty() {
            state.log = format!("new script\n{script_path}");
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            state.log = format!("new script\n{script_path}");
            return false;
        };
        node.script = Some(Cow::Owned(script_path.to_string()));
        doc.normalize_links();
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.sidebar_mode = "scene".to_string();
        state.activity_mode = "scene".to_string();
        state.log = format!("new script\nattach {script_path}");
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
    }
    refresh_all(ctx);
}

fn attach_material_to_selected_node<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    material_path: &str,
) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = format!("new mat\n{material_path}");
            return false;
        };
        if state.doc_text.is_empty() {
            state.log = format!("new mat\n{material_path}");
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            state.log = format!("new mat\n{material_path}");
            return false;
        };
        if !node.data.type_name().contains("MeshInstance3D") {
            state.log = "new mat\nselect MeshInstance3D to auto-bind".to_string();
            return false;
        }
        set_scene_string(&mut node.data, "material", material_path.to_string());
        doc.normalize_links();
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.sidebar_mode = "scene".to_string();
        state.activity_mode = "scene".to_string();
        state.log = format!("new mat\nattach {material_path}");
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
    }
    refresh_all(ctx);
}

fn attach_animation_to_selected_player<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    anim_path: &str,
) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
        if state.doc_text.is_empty() {
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            return false;
        };
        if node.data.type_name() != "AnimationPlayer" {
            return false;
        }
        set_scene_string(&mut node.data, "animation", anim_path.to_string());
        doc.normalize_links();
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.activity_mode = "anim".to_string();
        state.anim_drawer_open = true;
        state.active_anim_player_key = Some(key);
        state.active_anim_path = anim_path.to_string();
        state.active_glb_path.clear();
        state.active_glb_summary.clear();
        state.log = format!("new anim\nbind {anim_path}");
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    } else {
        open_animation_path(ctx, anim_path);
    }
}

fn export_selected_glb_animation<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let request = with_state!(ctx.run, EditorState, ctx.id, |state| {
        let path = active_gltf_asset_path(state)?;
        Some((
            state.project_root.clone(),
            path,
            state.active_glb_anim_index,
            state.active_anim_player_key,
        ))
    });
    let Some((root, glb_path, anim_index, player_key)) = request else {
        set_log(ctx, "glb anim fail\nselect glb");
        return;
    };
    let clip_name = format!("anim_{anim_index}");
    let stem = format!(
        "{}_{}",
        glb_asset_stem(&glb_path),
        sanitize_file_stem(&clip_name)
    );
    let out_path = unique_res_path(&root, "animations", &stem, "panim");
    let out_abs = res_to_abs(&root, &out_path);
    match ctx
        .res
        .Glbs()
        .animation_to_panim(&glb_path, 60.0, anim_index, "Rig")
    {
        Ok(text) => {
            if let Some(parent) = Path::new(&out_abs).parent() {
                let _ = fs::create_dir_all(parent);
            }
            match FileMod::save_string(&out_abs, &text) {
                Ok(()) => {
                    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                        if let Ok(paths) = scan_res_paths(Path::new(&state.project_root)) {
                            state.file_paths = paths;
                        }
                        state.active_asset_path = out_path.clone();
                        state.active_anim_path = out_path.clone();
                        state.active_glb_path.clear();
                        state.active_glb_summary.clear();
                        state.activity_mode = "anim".to_string();
                        state.anim_drawer_open = true;
                        if player_key.is_none() {
                            state.active_anim_player_key = None;
                        }
                        state.log = format!("glb anim -> panim\n{out_path}");
                    });
                    if player_key.is_some() {
                        attach_animation_to_selected_player(ctx, &out_path);
                    } else {
                        refresh_all(ctx);
                    }
                }
                Err(err) => set_log(ctx, &format!("glb anim write fail\n{out_path}\n{err}")),
            }
        }
        Err(err) => set_log(ctx, &format!("glb anim fail\n{glb_path}\n{err}")),
    }
}

fn export_selected_glb_material<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let request = with_state!(ctx.run, EditorState, ctx.id, |state| {
        let path = active_gltf_asset_path(state)?;
        Some((
            state.project_root.clone(),
            path,
            state.active_glb_mat_index,
            state.selected_key,
        ))
    });
    let Some((root, glb_path, mat_index, selected_key)) = request else {
        set_log(ctx, "glb mat fail\nselect glb");
        return;
    };
    let mat_name = format!("mat_{mat_index}");
    let stem = format!(
        "{}_{}",
        glb_asset_stem(&glb_path),
        sanitize_file_stem(&mat_name)
    );
    let out_path = unique_res_path(&root, "materials", &stem, "pmat");
    let out_abs = res_to_abs(&root, &out_path);
    match ctx.res.Glbs().material_to_pmat(&glb_path, mat_index) {
        Ok(text) => {
            if let Some(parent) = Path::new(&out_abs).parent() {
                let _ = fs::create_dir_all(parent);
            }
            match FileMod::save_string(&out_abs, &text) {
                Ok(()) => {
                    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                        if let Ok(paths) = scan_res_paths(Path::new(&state.project_root)) {
                            state.file_paths = paths;
                        }
                        state.active_asset_path = out_path.clone();
                        state.log = format!("glb mat -> pmat\n{out_path}");
                    });
                    if selected_key.is_some() {
                        attach_material_to_selected_node(ctx, &out_path);
                    } else {
                        refresh_all(ctx);
                    }
                }
                Err(err) => set_log(ctx, &format!("glb mat write fail\n{out_path}\n{err}")),
            }
        }
        Err(err) => set_log(ctx, &format!("glb mat fail\n{glb_path}\n{err}")),
    }
}

fn active_gltf_asset_path(state: &EditorState) -> Option<String> {
    if is_gltf_path(&state.active_asset_path) {
        Some(state.active_asset_path.clone())
    } else if is_gltf_path(&state.active_glb_path) {
        Some(state.active_glb_path.clone())
    } else {
        None
    }
}

fn glb_asset_stem(path: &str) -> String {
    sanitize_file_stem(
        Path::new(&editor_files::rel_label(path))
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("glb"),
    )
}

fn unique_res_path(project_root: &str, dir: &str, stem: &str, ext: &str) -> String {
    let dir = dir.trim_matches('/');
    for idx in 0..1000 {
        let suffix = if idx == 0 {
            String::new()
        } else {
            format!("_{idx}")
        };
        let path = format!("res://{dir}/{stem}{suffix}.{ext}");
        if !Path::new(&res_to_abs(project_root, &path)).exists() {
            return path;
        }
    }
    format!("res://{dir}/{stem}_x.{ext}")
}

fn unique_res_folder_path(project_root: &str, dir: &str, stem: &str) -> String {
    let dir = dir.trim_matches('/');
    for idx in 0..1000 {
        let suffix = if idx == 0 {
            String::new()
        } else {
            format!("_{idx}")
        };
        let path = if dir.is_empty() {
            format!("res://{stem}{suffix}/")
        } else {
            format!("res://{dir}/{stem}{suffix}/")
        };
        if !Path::new(&res_to_abs(project_root, &path)).exists() {
            return path;
        }
    }
    if dir.is_empty() {
        format!("res://{stem}_x/")
    } else {
        format!("res://{dir}/{stem}_x/")
    }
}

fn quick_asset_dir(state: &EditorState, kind: &str) -> String {
    if state.sidebar_mode == "files" {
        if let Some(dir) = res_folder_dir(&state.file_scope)
            && !dir.is_empty()
        {
            return dir;
        }
        if state.active_asset_path.ends_with('/')
            && let Some(dir) = res_folder_dir(&state.active_asset_path)
            && !dir.is_empty()
        {
            return dir;
        }
        if !state.active_asset_path.is_empty()
            && let Some(parent) = parent_res_folder(&state.active_asset_path)
                .strip_prefix("res://")
                .map(|path| path.trim_matches('/').to_string())
            && !parent.is_empty()
        {
            return parent;
        }
    }
    match kind {
        "scene" => "scenes".to_string(),
        "script" => "scripts".to_string(),
        "anim" => "animations".to_string(),
        "mat" => "materials".to_string(),
        _ => "assets".to_string(),
    }
}

fn res_folder_dir(path: &str) -> Option<String> {
    path.trim_end_matches('/')
        .strip_prefix("res://")
        .map(|path| path.trim_matches('/').to_string())
}

fn quick_asset_stem(state: &EditorState, kind: &str) -> String {
    if !state.doc_text.is_empty()
        && let Some(key) = state.selected_key
    {
        let doc = SceneDoc::parse(&state.doc_text);
        if let Some(node) = doc.scene.nodes.iter().find(|node| node.key.as_u32() == key) {
            let name = sanitize_file_stem(&doc.scene.key_name_or_id(node.key));
            if !name.is_empty() {
                return match kind {
                    "scene" => name,
                    "script" => format!("{name}_script"),
                    "anim" => format!("{name}_clip"),
                    "mat" => format!("{name}_mat"),
                    _ => name,
                };
            }
        }
    }
    if !state.active_asset_path.is_empty() && !state.active_asset_path.ends_with('/') {
        let stem = Path::new(&editor_files::rel_label(&state.active_asset_path))
            .file_stem()
            .and_then(|value| value.to_str())
            .map(sanitize_file_stem)
            .unwrap_or_default();
        if !stem.is_empty() {
            return stem;
        }
    }
    match kind {
        "scene" => "NewScene".to_string(),
        "script" => "new_script".to_string(),
        "anim" => "new_clip".to_string(),
        "mat" => "new_mat".to_string(),
        _ => "new_asset".to_string(),
    }
}

fn default_scene_text(name: &str) -> String {
    format!(
        "$root = @{name}\n\n[{name}]\n    [Node2D]\n        position = (0.0, 0.0)\n    [/Node2D]\n[/{name}]\n"
    )
}

fn default_script_text() -> String {
    "use perro_api::prelude::*;\n\ntype SelfNodeType = Node;\n\nlifecycle!({\n    fn on_init(&self, _ctx: &mut ScriptContext<'_, API>) {\n    }\n});\n".to_string()
}

fn default_material_pmat() -> String {
    "type = \"standard\"\ncolor = (1.0, 1.0, 1.0, 1.0)\nroughness = 0.65\nmetallic = 0.0\n".to_string()
}

fn select_node_slot<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, idx: usize) {
    let key = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.doc_text.is_empty() {
            None
        } else {
            let doc = SceneDoc::parse(&state.doc_text);
            scene_tree_view(
                &doc,
                state.selected_key,
                &state.scene_filter,
                &state.collapsed_scene_keys,
            )
                .keys
                .get(idx)
                .copied()
        }
    });
    if let Some(key) = key {
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            state.selected_key = Some(key);
            if let Some(mode) = selected_node_viewport_mode(&state.doc_text, key) {
                state.viewport_mode = mode.to_string();
            }
            if selected_node_type_name(&state.doc_text, key).as_deref() == Some("AnimationPlayer") {
                state.activity_mode = "anim".to_string();
                state.anim_drawer_open = true;
                state.active_anim_player_key = Some(key);
                state.active_glb_path.clear();
                state.active_glb_summary.clear();
                if let Some(path) = selected_node_field_text(&state.doc_text, key, "animation") {
                    state.active_anim_path = path;
                }
            }
        });
        refresh_all(ctx);
    }
}

fn set_activity_mode<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, mode: &str) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.activity_mode = mode.to_string();
        if mode == "scene" {
            state.sidebar_mode = "scene".to_string();
        }
        if mode == "anim" {
            state.anim_drawer_open = true;
        }
    });
    refresh_all(ctx);
}

fn update_scene_filter<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(text) = read_text_box(ctx, "scene_filter_box") else {
        return;
    };
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.scene_filter = text;
    });
    refresh_all(ctx);
}

fn update_file_filter<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(text) = read_text_box(ctx, "file_filter_box") else {
        return;
    };
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.file_filter = text;
    });
    refresh_all(ctx);
}

fn set_sidebar_mode<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, mode: &str) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.sidebar_mode = mode.to_string();
        state.activity_mode = mode.to_string();
    });
    refresh_all(ctx);
}

fn set_anim_drawer<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, visible: bool) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.anim_drawer_open = visible;
        if visible {
            state.activity_mode = "anim".to_string();
        }
    });
    refresh_all(ctx);
}

fn open_selected_node_asset_ref<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let path = with_state!(ctx.run, EditorState, ctx.id, |state| {
        let key = state.selected_key?;
        if state.doc_text.is_empty() {
            return None;
        }
        let doc = SceneDoc::parse(&state.doc_text);
        let node = doc.scene.nodes.iter().find(|node| node.key.as_u32() == key)?;
        selected_node_asset_ref_path(node)
    });
    let Some(path) = path else {
        set_log(ctx, "open ref fail\nno asset ref");
        return;
    };
    open_asset_ref_path(ctx, &path);
}

fn select_node_using_active_asset<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.active_asset_path.is_empty() || state.active_asset_path.ends_with('/') {
            state.log = "find user fail\nselect asset file".to_string();
            return false;
        }
        if state.doc_text.is_empty() {
            state.log = "find user fail\nopen scene".to_string();
            return false;
        }
        let doc = SceneDoc::parse(&state.doc_text);
        let Some(node) = doc
            .scene
            .nodes
            .iter()
            .find(|node| node_uses_asset_path(node, &state.active_asset_path))
        else {
            state.log = format!("find user\nnone for {}", state.active_asset_path);
            return false;
        };
        let key = node.key.as_u32();
        state.selected_key = Some(key);
        state.sidebar_mode = "scene".to_string();
        state.activity_mode = "scene".to_string();
        state.scene_filter.clear();
        if let Some(mode) = viewport_mode_for_node_type(node.data.node_type) {
            state.viewport_mode = mode.to_string();
        }
        state.log = format!(
            "find user\n{}",
            doc.scene.key_name_or_id(SceneKey::new(key))
        );
        true
    })
    .unwrap_or(false);
    if changed {
        refresh_all(ctx);
    }
}

fn selected_node_asset_refs(node: &SceneNodeEntry) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(root_of) = node.root_of.as_ref()
        && root_of.starts_with("res://")
    {
        out.push(format!("root_of: {root_of}"));
    }
    if let Some(script) = node.script.as_ref()
        && script.starts_with("res://")
    {
        out.push(format!("script: {script}"));
    }
    for field in ["animation", "mesh", "material", "texture"] {
        if let Some(path) = scene_field_value_text(&node.data, field)
            && path.starts_with("res://")
        {
            out.push(format!("{field}: {path}"));
        }
    }
    out
}

fn node_uses_asset_path(node: &SceneNodeEntry, asset_path: &str) -> bool {
    let base = base_res_asset_path(asset_path);
    selected_node_asset_refs(node).into_iter().any(|line| {
        line.split_once(": ")
            .map(|(_, path)| path == asset_path || base_res_asset_path(path) == base)
            .unwrap_or(false)
    })
}

fn selected_node_asset_ref_path(node: &SceneNodeEntry) -> Option<String> {
    selected_node_asset_refs(node)
        .into_iter()
        .find_map(|line| line.split_once(": ").map(|(_, path)| path.to_string()))
}

fn open_asset_ref_path<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, path: &str) {
    let base = base_res_asset_path(path);
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.active_asset_path = base.clone();
        state.sidebar_mode = "files".to_string();
        state.activity_mode = "files".to_string();
        state.log = format!("open ref\n{path}");
    });
    if base.ends_with(".panim") {
        open_animation_path(ctx, &base);
    } else if is_gltf_path(&base) {
        open_gltf_path(ctx, &base);
    } else if base.ends_with(".scn") {
        open_scene_path(ctx, &base);
    } else {
        refresh_all(ctx);
    }
}

fn base_res_asset_path(path: &str) -> String {
    let Some(rest) = path.strip_prefix("res://") else {
        return path.to_string();
    };
    match rest.find(':') {
        Some(idx) => format!("res://{}", &rest[..idx]),
        None => path.to_string(),
    }
}

fn use_active_asset_on_selected_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let asset_path = state.active_asset_path.clone();
        if asset_path.is_empty() || asset_path.ends_with('/') {
            state.log = "use asset fail\nselect asset file".to_string();
            return false;
        }
        let Some(key) = state.selected_key else {
            state.log = "use asset fail\nselect node".to_string();
            return false;
        };
        if state.doc_text.is_empty() {
            state.log = "use asset fail\nno open scene".to_string();
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            state.log = "use asset fail\nmissing node".to_string();
            return false;
        };
        let node_type = node.data.type_name().to_string();
        let Some((kind, value)) = asset_binding_for_node(
            &asset_path,
            &node_type,
            state.active_glb_mesh_index,
            state.active_glb_mat_index,
        ) else {
            state.log = format!("use asset fail\n{asset_path}\n{node_type}");
            return false;
        };
        if kind == "script" {
            node.script = Some(Cow::Owned(value.clone()));
        } else if kind == "root_of" {
            node.root_of = Some(Cow::Owned(value.clone()));
        } else {
            set_scene_string(&mut node.data, kind, value.clone());
            if kind == "mesh" && is_gltf_path(&asset_path) {
                set_scene_string(
                    &mut node.data,
                    "material",
                    format!("{asset_path}:mat[{}]", state.active_glb_mat_index),
                );
            }
        }
        doc.normalize_links();
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = format!("use asset\n{kind} = {value}");
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
    }
    refresh_all(ctx);
}

fn make_node_from_active_asset<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let asset_path = state.active_asset_path.clone();
        if asset_path.is_empty() || asset_path.ends_with('/') {
            state.log = "make node fail\nselect asset file".to_string();
            return false;
        }
        if state.doc_text.is_empty() {
            state.log = "make node fail\nno open scene".to_string();
            return false;
        }
        let Some((node_type_name, binding)) = asset_node_template(
            &asset_path,
            state.active_glb_mesh_index,
            state.active_glb_mat_index,
        ) else {
            state.log = format!("make node fail\n{asset_path}");
            return false;
        };
        let Ok(node_type) = perro_scene::NodeType::from_str(node_type_name) else {
            state.log = format!("make node fail\nbad type {node_type_name}");
            return false;
        };
        let mut doc = SceneDoc::parse(&state.doc_text);
        let next_id = doc.scene.key_names.len() as u32;
        let key = SceneKey::new(next_id);
        let rel = editor_files::rel_label(&asset_path);
        let stem = Path::new(&rel)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(node_type.name());
        let name = unique_node_name(&doc, &sanitize_file_stem(stem));
        let mut data = SceneNodeData::new(node_type, Cow::Owned(default_fields(node_type)), None);
        let mut script = None;
        let mut root_of = None;
        if let Some((field, value)) = binding {
            if field == "script" {
                script = Some(Cow::Owned(value));
            } else if field == "root_of" {
                root_of = Some(Cow::Owned(value));
            } else {
                set_scene_string(&mut data, field, value);
                if field == "mesh" && is_gltf_path(&asset_path) {
                    set_scene_string(
                        &mut data,
                        "material",
                        format!("{asset_path}:mat[{}]", state.active_glb_mat_index),
                    );
                }
            }
        }
        apply_spawn_position(&mut data, state);
        let parent = state.selected_key.map(SceneKey::new).or(doc.scene.root);
        doc.scene.key_names.to_mut().push(Cow::Owned(name.clone()));
        doc.scene.nodes.to_mut().push(SceneNodeEntry {
            data,
            has_data_override: true,
            key,
            name: None,
            tags: Cow::Owned(Vec::new()),
            children: Cow::Owned(Vec::new()),
            parent,
            script,
            clear_script: false,
            root_of,
            script_vars: Cow::Owned(Vec::new()),
        });
        doc.normalize_links();
        state.doc_text = doc.to_text();
        state.selected_key = Some(next_id);
        if let Some(mode) = viewport_mode_for_node_type(node_type) {
            state.viewport_mode = mode.to_string();
        }
        state.dirty = true;
        state.sidebar_mode = "scene".to_string();
        state.activity_mode = "scene".to_string();
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = format!("make node\n{name}: {node_type_name}");
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
    }
    refresh_all(ctx);
}

fn asset_binding_for_node(
    path: &str,
    node_type: &str,
    glb_mesh_index: usize,
    _glb_mat_index: usize,
) -> Option<(&'static str, String)> {
    if path.ends_with(".rs") {
        return Some(("script", path.to_string()));
    }
    if path.ends_with(".scn") {
        return Some(("root_of", path.to_string()));
    }
    if path.ends_with(".panim") && node_type == "AnimationPlayer" {
        return Some(("animation", path.to_string()));
    }
    if (path.ends_with(".glb") || path.ends_with(".gltf")) && node_type.contains("MeshInstance3D") {
        return Some(("mesh", format!("{path}:mesh[{glb_mesh_index}]")));
    }
    if (path.ends_with(".pmesh") || path.ends_with(".obj") || path.ends_with(".fbx"))
        && node_type.contains("MeshInstance3D")
    {
        return Some(("mesh", path.to_string()));
    }
    if path.ends_with(".pmat") && node_type.contains("MeshInstance3D") {
        return Some(("material", path.to_string()));
    }
    if editor_files::kind_label(path) == "image" && node_type == "Sprite2D" {
        return Some(("texture", path.to_string()));
    }
    None
}

fn asset_node_template(
    path: &str,
    glb_mesh_index: usize,
    _glb_mat_index: usize,
) -> Option<(&'static str, Option<(&'static str, String)>)> {
    if path.ends_with(".rs") {
        return Some(("Node", Some(("script", path.to_string()))));
    }
    if path.ends_with(".scn") {
        return Some(("Node", Some(("root_of", path.to_string()))));
    }
    if path.ends_with(".panim") {
        return Some(("AnimationPlayer", Some(("animation", path.to_string()))));
    }
    if path.ends_with(".glb") || path.ends_with(".gltf") {
        return Some((
            "MeshInstance3D",
            Some(("mesh", format!("{path}:mesh[{glb_mesh_index}]"))),
        ));
    }
    if path.ends_with(".pmesh") || path.ends_with(".obj") || path.ends_with(".fbx") {
        return Some(("MeshInstance3D", Some(("mesh", path.to_string()))));
    }
    if path.ends_with(".pmat") {
        return Some(("MeshInstance3D", Some(("material", path.to_string()))));
    }
    if editor_files::kind_label(path) == "image" {
        return Some(("Sprite2D", Some(("texture", path.to_string()))));
    }
    None
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
        let parent = add_node_parent(&doc, state.selected_key, state.add_node_as_sibling);
        let mut data = SceneNodeData::new(node_type, Cow::Owned(default_fields(node_type)), None);
        let mut script = None;
        let mut root_of = None;
        let mut auto_bind = None;
        if !state.active_asset_path.is_empty()
            && !state.active_asset_path.ends_with('/')
            && let Some((field, value)) = asset_binding_for_node(
                &state.active_asset_path,
                node_type.name(),
                state.active_glb_mesh_index,
                state.active_glb_mat_index,
            )
        {
            auto_bind = Some(format!("{field} = {value}"));
            if field == "script" {
                script = Some(Cow::Owned(value));
            } else if field == "root_of" {
                root_of = Some(Cow::Owned(value));
            } else {
                set_scene_string(&mut data, field, value);
                if field == "mesh" && is_gltf_path(&state.active_asset_path) {
                    set_scene_string(
                        &mut data,
                        "material",
                        format!("{}:mat[{}]", state.active_asset_path, state.active_glb_mat_index),
                    );
                }
            }
        }
        apply_spawn_position(&mut data, state);
        let node = SceneNodeEntry {
            data,
            has_data_override: true,
            key,
            name: None,
            tags: Cow::Owned(Vec::new()),
            children: Cow::Owned(Vec::new()),
            parent,
            script,
            clear_script: false,
            root_of,
            script_vars: Cow::Owned(Vec::new()),
        };
        doc.scene.key_names.to_mut().push(Cow::Owned(name.clone()));
        doc.scene.nodes.to_mut().push(node);
        doc.normalize_links();
        state.doc_text = doc.to_text();
        state.selected_key = Some(next_id);
        state.add_node_as_sibling = false;
        push_recent_node_type(state, node_type.name());
        if let Some(mode) = viewport_mode_for_node_type(node_type) {
            state.viewport_mode = mode.to_string();
        }
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = match auto_bind {
            Some(binding) => format!("add node\n{name}: {}\n{binding}", node_type.name()),
            None => format!("add node\n{name}: {}", node_type.name()),
        };
        msg = state.log.clone();
    });
    set_log(ctx, &msg);
    rebuild_preview(ctx);
    refresh_all(ctx);
}

fn add_node_from_picker<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, row: usize) {
    let node_type = with_state!(ctx.run, EditorState, ctx.id, |state| {
        exact_picker_node_type(&state.node_picker_filter).or_else(|| {
            picker_node_types(state, &state.node_picker_filter)
                .get(state.node_picker_offset + row)
                .copied()
        })
    });
    if let Some(node_type) = node_type {
        add_node(ctx, node_type.name());
    }
}

fn exact_picker_node_type(filter: &str) -> Option<perro_scene::NodeType> {
    let filter = NodePickerFilter::parse(filter);
    if !filter.tags.is_empty() || filter.text.len() != 1 {
        return None;
    }
    let needle = filter.text.first()?;
    perro_scene::NodeType::ALL
        .iter()
        .copied()
        .find(|node_type| node_type.name().eq_ignore_ascii_case(needle))
}

fn add_node_parent(
    doc: &SceneDoc,
    selected_key: Option<u32>,
    as_sibling: bool,
) -> Option<SceneKey> {
    let Some(raw_key) = selected_key else {
        return doc.scene.root.map(|key| SceneKey::new(key.as_u32()));
    };
    if as_sibling {
        return doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == raw_key)
            .and_then(|node| node.parent.map(|parent| SceneKey::new(parent.as_u32())))
            .or(doc.scene.root.map(|key| SceneKey::new(key.as_u32())));
    }
    Some(SceneKey::new(raw_key))
}

fn apply_spawn_position(data: &mut SceneNodeData, state: &EditorState) {
    if data.node_type.is_a(perro_scene::NodeType::Node2D) {
        set_scene_vec2(data, "position", Vector2::new(state.cam2_x, state.cam2_y));
    } else if data.node_type.is_a(perro_scene::NodeType::Node3D) {
        set_scene_vec3(data, "position", viewport_spawn_3d(state));
    }
}

fn viewport_spawn_3d(state: &EditorState) -> Vector3 {
    let origin = Vector3::new(state.cam_x, state.cam_y, state.cam_z);
    let rotation = Quaternion::from_euler_xyz(state.cam_pitch, state.cam_yaw, 0.0);
    let forward = rotation
        .rotate_vector3(Vector3::new(0.0, 0.0, -1.0))
        .normalized();
    let mut point = origin + forward * 6.0;
    if forward.y.abs() > 0.0001 {
        let t = -origin.y / forward.y;
        if t.is_finite() && t > 0.0 && t < 1000.0 {
            point = origin + forward * t;
        }
    }
    point.y = 0.0;
    point
}

fn update_node_picker_filter<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(text) = read_text_box(ctx, "add_node_search_box") else {
        return;
    };
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.node_picker_filter = text;
        state.node_picker_offset = 0;
    });
    refresh_all(ctx);
}

fn set_node_picker_filter_text<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    text: &str,
) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.node_picker_filter = text.to_string();
        state.node_picker_offset = 0;
    });
    refresh_all(ctx);
}

fn shift_node_picker<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, dir: isize) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let max_start = picker_node_types(state, &state.node_picker_filter)
            .len()
            .saturating_sub(MAX_NODE_PICKER_ROWS);
        if dir < 0 {
            state.node_picker_offset = state
                .node_picker_offset
                .saturating_sub(MAX_NODE_PICKER_ROWS);
        } else {
            state.node_picker_offset =
                (state.node_picker_offset + MAX_NODE_PICKER_ROWS).min(max_start);
        }
    });
    refresh_all(ctx);
}

fn set_node_picker_edge<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, last: bool) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let max_start = picker_node_types(state, &state.node_picker_filter)
            .len()
            .saturating_sub(MAX_NODE_PICKER_ROWS);
        state.node_picker_offset = if last { max_start } else { 0 };
    });
    refresh_all(ctx);
}

fn nudge_node_picker<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, dir: isize) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let max_start = picker_node_types(state, &state.node_picker_filter)
            .len()
            .saturating_sub(1);
        if dir < 0 {
            state.node_picker_offset = state.node_picker_offset.saturating_sub(1);
        } else {
            state.node_picker_offset = (state.node_picker_offset + 1).min(max_start);
        }
    });
    refresh_all(ctx);
}

fn picker_node_types(state: &EditorState, filter: &str) -> Vec<perro_scene::NodeType> {
    let filter = NodePickerFilter::parse(filter);
    let asset_node = active_asset_node_type(state);
    let parent_kind = picker_parent_node_kind(state);
    let mut out = perro_scene::NodeType::ALL
        .iter()
        .copied()
        .filter(|node_type| {
            if filter.is_empty() {
                return true;
            }
            if filter.tags.iter().any(|tag| tag == "recent")
                && recent_node_rank(state, node_type.name()).is_none()
            {
                return false;
            }
            node_type_matches_picker_filter(*node_type, &filter)
        })
        .collect::<Vec<_>>();
    out.sort_by_key(|node_type| {
        (
            exact_node_rank(&filter, *node_type),
            if Some(node_type.name()) == asset_node { 0 } else { 1 },
            parent_node_rank(parent_kind, *node_type),
            viewport_node_rank(state, *node_type),
            recent_node_rank(state, node_type.name()).unwrap_or(usize::MAX),
            node_type_rank(*node_type),
            node_type.name(),
        )
    });
    out
}

fn picker_node_row(state: &EditorState, node_type: perro_scene::NodeType) -> String {
    let mut badges = Vec::new();
    if Some(node_type.name()) == active_asset_node_type(state) {
        badges.push("asset");
    }
    if recent_node_rank(state, node_type.name()).is_some() {
        badges.push("recent");
    }
    if parent_node_rank(picker_parent_node_kind(state), node_type) == 0 {
        badges.push("parent");
    }
    if viewport_node_rank(state, node_type) == 0 {
        badges.push(state.viewport_mode.as_str());
    }
    let badges = if badges.is_empty() {
        String::new()
    } else {
        format!("  [{}]", badges.join(" "))
    };
    format!("{} {}{}", node_type_icon(node_type), node_type.name(), badges)
}

fn exact_node_rank(filter: &NodePickerFilter, node_type: perro_scene::NodeType) -> u8 {
    if filter.tags.is_empty()
        && filter.text.len() == 1
        && node_type
            .name()
            .eq_ignore_ascii_case(filter.text.first().map(String::as_str).unwrap_or(""))
    {
        0
    } else {
        1
    }
}

fn push_recent_node_type(state: &mut EditorState, node_type: &str) {
    state.recent_node_types.retain(|item| item != node_type);
    state.recent_node_types.insert(0, node_type.to_string());
    state.recent_node_types.truncate(8);
}

fn recent_node_rank(state: &EditorState, node_type: &str) -> Option<usize> {
    state.recent_node_types.iter().position(|item| item == node_type)
}

fn active_asset_node_type(state: &EditorState) -> Option<&'static str> {
    if state.active_asset_path.is_empty() || state.active_asset_path.ends_with('/') {
        return None;
    }
    asset_node_template(
        &state.active_asset_path,
        state.active_glb_mesh_index,
        state.active_glb_mat_index,
    )
    .map(|(node_type, _)| node_type)
}

fn picker_parent_node_kind(state: &EditorState) -> Option<&'static str> {
    if state.doc_text.is_empty() {
        return None;
    }
    let doc = SceneDoc::parse(&state.doc_text);
    let key = add_node_parent(&doc, state.selected_key, state.add_node_as_sibling)?;
    doc.scene
        .nodes
        .iter()
        .find(|node| node.key == key)
        .map(|node| node_type_kind(node.data.node_type))
}

fn node_type_kind(node_type: perro_scene::NodeType) -> &'static str {
    if node_type.is_a(perro_scene::NodeType::Node2D) {
        "2D"
    } else if node_type.is_a(perro_scene::NodeType::Node3D) {
        "3D"
    } else if node_type.is_a(perro_scene::NodeType::UiBox) {
        "UI"
    } else {
        "Node"
    }
}

fn parent_node_rank(kind: Option<&str>, node_type: perro_scene::NodeType) -> u8 {
    match kind {
        Some("2D") if node_type.is_a(perro_scene::NodeType::Node2D) => 0,
        Some("3D") if node_type.is_a(perro_scene::NodeType::Node3D) => 0,
        Some("UI") if node_type.is_a(perro_scene::NodeType::UiBox) => 0,
        Some("Node") => 0,
        Some(_) => 1,
        None => 0,
    }
}

fn node_type_rank(node_type: perro_scene::NodeType) -> u8 {
    match node_type.name() {
        "Node2D" => 0,
        "Sprite2D" => 1,
        "Camera2D" => 2,
        "Node3D" => 3,
        "MeshInstance3D" => 4,
        "Camera3D" => 5,
        "AnimationPlayer" => 6,
        "UiPanel" | "UiButton" | "UiLabel" => 7,
        _ if node_type.is_a(perro_scene::NodeType::Node2D) => 20,
        _ if node_type.is_a(perro_scene::NodeType::Node3D) => 30,
        _ if node_type.is_a(perro_scene::NodeType::UiBox) => 40,
        _ => 90,
    }
}

struct NodePickerFilter {
    text: Vec<String>,
    tags: Vec<String>,
}

impl NodePickerFilter {
    fn parse(raw: &str) -> Self {
        let mut text = Vec::new();
        let mut tags = Vec::new();
        for token in raw.trim().to_ascii_lowercase().split_whitespace() {
            if let Some(tag) = token.strip_prefix('@') {
                if !tag.is_empty() {
                    tags.push(tag.to_string());
                }
            } else {
                text.push(token.to_string());
            }
        }
        Self { text, tags }
    }

    fn is_empty(&self) -> bool {
        self.text.is_empty() && self.tags.is_empty()
    }
}

fn node_type_matches_picker_filter(
    node_type: perro_scene::NodeType,
    filter: &NodePickerFilter,
) -> bool {
    filter
        .tags
        .iter()
        .filter(|tag| tag.as_str() != "recent")
        .all(|tag| node_type_has_picker_tag(node_type, tag))
        && filter
            .text
            .iter()
            .all(|needle| node_type_search_text(node_type).contains(needle))
}

fn node_type_has_picker_tag(node_type: perro_scene::NodeType, tag: &str) -> bool {
    match tag {
        "2d" => node_type.is_a(perro_scene::NodeType::Node2D),
        "3d" => node_type.is_a(perro_scene::NodeType::Node3D),
        "ui" => node_type.is_a(perro_scene::NodeType::UiBox),
        "mesh" => node_type.name().contains("Mesh") || node_type_search_text(node_type).contains("mesh"),
        "anim" => node_type.name().contains("Animation") || node_type_search_text(node_type).contains("anim"),
        "phys" | "physics" => node_type_search_text(node_type).contains("physics"),
        "light" => node_type_search_text(node_type).contains("light"),
        "audio" => node_type_search_text(node_type).contains("audio"),
        "cam" | "camera" => node_type_search_text(node_type).contains("camera"),
        "recent" => false,
        _ => node_type_search_text(node_type).contains(tag),
    }
}

fn viewport_node_rank(state: &EditorState, node_type: perro_scene::NodeType) -> u8 {
    match state.viewport_mode.as_str() {
        "2D" if node_type.is_a(perro_scene::NodeType::Node2D) => 0,
        "3D" if node_type.is_a(perro_scene::NodeType::Node3D) => 0,
        "UI" if node_type.is_a(perro_scene::NodeType::UiBox) => 0,
        _ => 1,
    }
}

fn node_type_search_text(node_type: perro_scene::NodeType) -> String {
    let aliases = match node_type.name() {
        "Sprite2D" => " image texture png 2d visual",
        "MeshInstance3D" => " mesh model glb gltf pmesh 3d visual",
        "AnimationPlayer" => " anim animation clip panim timeline",
        "Camera2D" | "Camera3D" => " camera view viewport",
        "UiPanel" | "UiButton" | "UiLabel" => " ui control hud menu",
        "PointLight2D" | "SpotLight2D" | "RayLight2D" | "AmbientLight2D" | "PointLight3D"
        | "SpotLight3D" | "RayLight3D" | "AmbientLight3D" => " light lamp glow shadow",
        "AudioPlayer2D"
        | "AudioStreamPlayer2D"
        | "AudioArea2D"
        | "AudioPlayer3D"
        | "AudioStreamPlayer3D"
        | "AudioArea3D" => " audio sound music",
        "PhysicsBody2D" | "StaticBody2D" | "RigidBody2D" | "CharacterBody2D" | "Area2D"
        | "CollisionShape2D" | "PhysicsBody3D" | "StaticBody3D" | "RigidBody3D"
        | "CharacterBody3D" | "Area3D" | "CollisionShape3D" => " physics collision body area",
        _ => "",
    };
    format!(
        "{} {} {}",
        node_type.name().to_ascii_lowercase(),
        node_type_icon(node_type).to_ascii_lowercase(),
        aliases
    )
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
        fields.push((
            SceneFieldName::Position,
            SceneValue::Vec2 { x: 0.0, y: 0.0 },
        ));
    } else if node_type.is_a(perro_scene::NodeType::UiBox) {
        fields.push((
            SceneFieldName::Anchor,
            SceneValue::Str(Cow::Borrowed("center")),
        ));
        fields.push((
            SceneFieldName::SizeRatio,
            SceneValue::Vec2 { x: 0.20, y: 0.12 },
        ));
    }

    if node_type == perro_scene::NodeType::UiLabel || node_type == perro_scene::NodeType::UiButton {
        fields.push((
            SceneFieldName::Text,
            SceneValue::Str(Cow::Borrowed("New Node")),
        ));
    }
    fields
}

fn save_active_scene<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let saved = save_active_scene_to_disk(ctx, false);
    if saved {
        rebuild_preview(ctx);
    }
    refresh_all(ctx);
}

fn save_active_scene_to_disk<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    quiet: bool,
) -> bool {
    let save = with_state!(ctx.run, EditorState, ctx.id, |state| {
        let path = state.open_paths.get(state.active_open).cloned();
        let root = state.project_root.clone();
        let doc_text = state.doc_text.clone();
        (root, path, doc_text)
    });
    let (root, Some(path), doc_text) = save else {
        if !quiet {
            set_log(ctx, "save fail\nno open scene");
        }
        return false;
    };
    if doc_text.is_empty() {
        if !quiet {
            set_log(ctx, "save fail\nno open scene");
        }
        return false;
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
                if !quiet {
                    state.log = format!("save scene\n{path}");
                }
            });
            true
        }
        Err(err) => {
            let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                state.log = format!("save fail\n{path}\n{err}");
            });
            false
        }
    }
}

fn save_all_scenes<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let (root, open_paths, active_open, active_doc_text, dirty_paths) =
        with_state!(ctx.run, EditorState, ctx.id, |state| {
            (
                state.project_root.clone(),
                state.open_paths.clone(),
                state.active_open,
                state.doc_text.clone(),
                state.dirty_scene_paths.clone(),
            )
        });
    if open_paths.is_empty() || dirty_paths.is_empty() {
        set_log(ctx, "save all\nnothing dirty");
        refresh_all(ctx);
        return;
    }

    let mut saved = Vec::new();
    let mut failed = Vec::new();
    for path in dirty_paths.iter() {
        let Some(idx) = open_paths.iter().position(|open| open == path) else {
            continue;
        };
        let text = if idx == active_open {
            active_doc_text.clone()
        } else {
            let abs = res_to_abs(&root, path);
            match FileMod::load_string(&abs) {
                Ok(text) => text,
                Err(err) => {
                    failed.push(format!("{path}: {err}"));
                    continue;
                }
            }
        };
        let mut doc = SceneDoc::parse(&text);
        doc.normalize_links();
        let abs = res_to_abs(&root, path);
        match FileMod::save_string(&abs, &doc.to_text()) {
            Ok(_) => saved.push(path.clone()),
            Err(err) => failed.push(format!("{path}: {err}")),
        }
    }

    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        for path in saved.iter() {
            state.dirty_scene_paths.retain(|item| item != path);
        }
        state.dirty = state
            .open_paths
            .get(state.active_open)
            .map(|path| state.dirty_scene_paths.iter().any(|dirty| dirty == path))
            .unwrap_or(false);
        state.project_file_sigs = editor_file_watch::scan_project(Path::new(&root));
        state.log = if failed.is_empty() {
            format!("save all\n{} scene(s)", saved.len())
        } else {
            format!("save all\nsaved={}\nfail={}", saved.len(), failed.join("\n"))
        };
    });
    rebuild_preview(ctx);
    refresh_all(ctx);
}

fn delete_selected_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = "delete node fail\nselect node".to_string();
            return false;
        };
        if state.doc_text.is_empty() {
            state.log = "delete node fail\nno open scene".to_string();
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        if doc.scene.root.map(|root| root.as_u32()) == Some(key) {
            state.log = "delete node fail\nroot node".to_string();
            return false;
        }
        let Some(target) = doc.scene.nodes.iter().find(|node| node.key.as_u32() == key) else {
            state.log = "delete node fail\nmissing node".to_string();
            return false;
        };
        let parent_key = target.parent.map(|parent| parent.as_u32());
        let removed_keys = collect_scene_subtree_keys(&doc, key);
        doc.scene
            .nodes
            .to_mut()
            .retain(|node| !removed_keys.contains(&node.key.as_u32()));
        doc.normalize_links();
        state.doc_text = doc.to_text();
        state.selected_key = parent_key
            .filter(|parent| doc.scene.nodes.iter().any(|node| node.key.as_u32() == *parent))
            .or_else(|| doc.scene.nodes.first().map(|node| node.key.as_u32()));
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = format!("delete node\nrm {} node", removed_keys.len());
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    } else {
        refresh_all(ctx);
    }
}

fn toggle_selected_visible<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = "visible fail\nselect node".to_string();
            return false;
        };
        if state.doc_text.is_empty() {
            state.log = "visible fail\nno open scene".to_string();
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            state.log = "visible fail\nmissing node".to_string();
            return false;
        };
        let visible = scene_field_bool(&node.data, "visible").unwrap_or(true);
        set_scene_bool(&mut node.data, "visible", !visible);
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = format!(
            "visible\n{} = {}",
            doc.scene.key_name_or_id(SceneKey::new(key)),
            !visible
        );
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
    }
    refresh_all(ctx);
}

fn duplicate_selected_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = "duplicate node fail\nselect node".to_string();
            return false;
        };
        if state.doc_text.is_empty() {
            state.log = "duplicate node fail\nno open scene".to_string();
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let subtree_keys = collect_scene_subtree_keys(&doc, key);
        if subtree_keys.is_empty() {
            state.log = "duplicate node fail\nmissing node".to_string();
            return false;
        }
        let mut map = Vec::new();
        let mut clones = Vec::new();
        for old_key in subtree_keys.iter().copied() {
            let Some(source) = doc
                .scene
                .nodes
                .iter()
                .find(|node| node.key.as_u32() == old_key)
                .cloned()
            else {
                continue;
            };
            let new_key = doc.scene.key_names.len() as u32;
            let source_name = doc.scene.key_name_or_id(source.key).to_string();
            let new_name = unique_node_name(&doc, &format!("{source_name}_copy"));
            doc.scene.key_names.to_mut().push(Cow::Owned(new_name));
            map.push((old_key, new_key));
            clones.push(source);
        }
        if clones.is_empty() {
            state.log = "duplicate node fail\nmissing node".to_string();
            return false;
        }
        for mut node in clones {
            let old_key = node.key.as_u32();
            let Some(new_key) = mapped_scene_key(&map, old_key) else {
                continue;
            };
            node.key = SceneKey::new(new_key);
            if let Some(parent) = node.parent {
                if let Some(new_parent) = mapped_scene_key(&map, parent.as_u32()) {
                    node.parent = Some(SceneKey::new(new_parent));
                }
            }
            if old_key == key {
                offset_duplicated_node(&mut node.data);
            }
            node.children = Cow::Owned(Vec::new());
            doc.scene.nodes.to_mut().push(node);
        }
        doc.normalize_links();
        state.doc_text = doc.to_text();
        state.selected_key = mapped_scene_key(&map, key);
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = format!("duplicate node\nadd {} node", map.len());
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
    }
    refresh_all(ctx);
}

fn copy_selected_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let copied = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = "copy node fail\nselect node".to_string();
            return false;
        };
        if state.doc_text.is_empty() {
            state.log = "copy node fail\nno open scene".to_string();
            return false;
        }
        let doc = SceneDoc::parse(&state.doc_text);
        let Some(node) = doc.scene.nodes.iter().find(|node| node.key.as_u32() == key) else {
            state.log = "copy node fail\nmissing node".to_string();
            return false;
        };
        state.copied_node_key = Some(key);
        state.log = format!("copy node\n{}", doc.scene.key_name_or_id(node.key));
        true
    })
    .unwrap_or(false);
    if copied {
        refresh_all(ctx);
    }
}

fn paste_copied_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(source_key) = state.copied_node_key else {
            state.log = "paste node fail\ncopy node first".to_string();
            return false;
        };
        if state.doc_text.is_empty() {
            state.log = "paste node fail\nno open scene".to_string();
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        if !doc
            .scene
            .nodes
            .iter()
            .any(|node| node.key.as_u32() == source_key)
        {
            state.log = "paste node fail\ncopied node missing".to_string();
            return false;
        }
        let root_parent = state
            .selected_key
            .filter(|key| *key != source_key)
            .map(SceneKey::new)
            .or(doc.scene.root);
        let subtree_keys = collect_scene_subtree_keys(&doc, source_key);
        let mut map = Vec::new();
        let mut clones = Vec::new();
        for old_key in subtree_keys.iter().copied() {
            let Some(source) = doc
                .scene
                .nodes
                .iter()
                .find(|node| node.key.as_u32() == old_key)
                .cloned()
            else {
                continue;
            };
            let new_key = doc.scene.key_names.len() as u32;
            let source_name = doc.scene.key_name_or_id(source.key).to_string();
            let new_name = unique_node_name(&doc, &format!("{source_name}_paste"));
            doc.scene.key_names.to_mut().push(Cow::Owned(new_name));
            map.push((old_key, new_key));
            clones.push(source);
        }
        if clones.is_empty() {
            state.log = "paste node fail\nempty copy".to_string();
            return false;
        }
        for mut node in clones {
            let old_key = node.key.as_u32();
            let Some(new_key) = mapped_scene_key(&map, old_key) else {
                continue;
            };
            node.key = SceneKey::new(new_key);
            if old_key == source_key {
                node.parent = root_parent;
                offset_duplicated_node(&mut node.data);
            } else if let Some(parent) = node.parent {
                if let Some(new_parent) = mapped_scene_key(&map, parent.as_u32()) {
                    node.parent = Some(SceneKey::new(new_parent));
                }
            }
            node.children = Cow::Owned(Vec::new());
            doc.scene.nodes.to_mut().push(node);
        }
        doc.normalize_links();
        state.doc_text = doc.to_text();
        state.selected_key = mapped_scene_key(&map, source_key);
        if let Some(key) = state.selected_key
            && let Some(mode) = selected_node_viewport_mode(&state.doc_text, key)
        {
            state.viewport_mode = mode.to_string();
        }
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = format!("paste node\nadd {} node", map.len());
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
    }
    refresh_all(ctx);
}

fn move_selected_node_order<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, dir: isize) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = "move node fail\nselect node".to_string();
            return false;
        };
        if state.doc_text.is_empty() {
            state.log = "move node fail\nno open scene".to_string();
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(index) = doc
            .scene
            .nodes
            .iter()
            .position(|node| node.key.as_u32() == key)
        else {
            state.log = "move node fail\nmissing node".to_string();
            return false;
        };
        let parent = doc.scene.nodes[index].parent.map(|parent| parent.as_u32());
        let siblings = doc
            .scene
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(idx, node)| {
                (node.parent.map(|parent| parent.as_u32()) == parent).then_some(idx)
            })
            .collect::<Vec<_>>();
        let Some(pos) = siblings.iter().position(|idx| *idx == index) else {
            return false;
        };
        let next_pos = offset_index(pos, siblings.len(), dir);
        if next_pos == pos {
            state.log = "move node\nat edge".to_string();
            return false;
        }
        let other_index = siblings[next_pos];
        doc.scene.nodes.to_mut().swap(index, other_index);
        doc.normalize_links();
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = if dir < 0 {
            "move node\nup".to_string()
        } else {
            "move node\ndown".to_string()
        };
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
    }
    refresh_all(ctx);
}

fn reparent_selected_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, dir: isize) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = "reparent fail\nselect node".to_string();
            return false;
        };
        if state.doc_text.is_empty() {
            state.log = "reparent fail\nno open scene".to_string();
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        if doc.scene.root.map(|root| root.as_u32()) == Some(key) {
            state.log = "reparent fail\nroot node".to_string();
            return false;
        }
        let Some(index) = doc
            .scene
            .nodes
            .iter()
            .position(|node| node.key.as_u32() == key)
        else {
            state.log = "reparent fail\nmissing node".to_string();
            return false;
        };
        let current_parent = doc.scene.nodes[index].parent.map(|parent| parent.as_u32());
        let next_parent = if dir < 0 {
            let Some(parent_key) = current_parent else {
                state.log = "reparent\nat root".to_string();
                return false;
            };
            doc.scene
                .nodes
                .iter()
                .find(|node| node.key.as_u32() == parent_key)
                .and_then(|node| node.parent.map(|parent| parent.as_u32()))
        } else {
            let siblings = doc
                .scene
                .nodes
                .iter()
                .filter(|node| node.parent.map(|parent| parent.as_u32()) == current_parent)
                .map(|node| node.key.as_u32())
                .collect::<Vec<_>>();
            let Some(pos) = siblings.iter().position(|sibling| *sibling == key) else {
                return false;
            };
            if pos == 0 {
                state.log = "reparent\nno previous sibling".to_string();
                return false;
            }
            Some(siblings[pos - 1])
        };
        if next_parent == Some(key) || current_parent == next_parent {
            state.log = "reparent\nno change".to_string();
            return false;
        }
        doc.scene.nodes.to_mut()[index].parent = next_parent.map(SceneKey::new);
        doc.normalize_links();
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = if dir < 0 {
            "reparent\nout".to_string()
        } else {
            "reparent\nin".to_string()
        };
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
    }
    refresh_all(ctx);
}

fn collect_scene_subtree_keys(doc: &SceneDoc, root_key: u32) -> Vec<u32> {
    let mut out = vec![root_key];
    let mut cursor = 0;
    while cursor < out.len() {
        let parent_key = out[cursor];
        for node in doc.scene.nodes.iter() {
            let child_key = node.key.as_u32();
            if node.parent.map(|parent| parent.as_u32()) == Some(parent_key)
                && !out.contains(&child_key)
            {
                out.push(child_key);
            }
        }
        cursor += 1;
    }
    out
}

fn mapped_scene_key(map: &[(u32, u32)], key: u32) -> Option<u32> {
    map.iter()
        .find(|(old_key, _)| *old_key == key)
        .map(|(_, new_key)| *new_key)
}

fn offset_duplicated_node(data: &mut SceneNodeData) {
    if data.node_type.is_a(perro_scene::NodeType::Node3D) {
        let pos = find_vec3_value(data, "position").unwrap_or(Vector3::ZERO);
        set_scene_vec3(data, "position", pos + Vector3::new(1.0, 0.0, 1.0));
    } else if data.node_type.is_a(perro_scene::NodeType::Node2D) {
        let pos = find_vec2_value(data, "position").unwrap_or(Vector2::ZERO);
        set_scene_vec2(data, "position", pos + Vector2::new(16.0, -16.0));
    }
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

fn zoom_active_viewport<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, dir: i32) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let factor = if dir > 0 { 1.25 } else { 0.8 };
        if state.viewport_mode == "2D" {
            state.cam2_zoom = (state.cam2_zoom * factor).clamp(0.05, 40.0);
            state.log = format!("zoom 2d\n{:.2}", state.cam2_zoom);
        } else if state.viewport_mode == "UI" {
            state.ui_canvas_zoom = (state.ui_canvas_zoom * factor).clamp(0.25, 12.0);
            state.log = format!("zoom ui\n{:.2}", state.ui_canvas_zoom);
        } else {
            state.log = "zoom\nuse 2d/ui viewport".to_string();
        }
    });
    apply_freecam_2d(ctx);
    refresh_all(ctx);
}

fn reset_active_viewport_zoom<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.viewport_mode == "2D" {
            state.cam2_zoom = 1.0;
            state.log = "zoom 2d\nreset".to_string();
        } else if state.viewport_mode == "UI" {
            state.ui_canvas_zoom = 1.0;
            state.log = "zoom ui\nreset".to_string();
        } else {
            state.log = "zoom\nuse 2d/ui viewport".to_string();
        }
    });
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
                let place = if viewport_shift_down(ctx) {
                    snap_vec2(world, 16.0)
                } else {
                    world
                };
                if viewport_alt_down(ctx) && duplicate_selected_node_at(ctx, Some(place), None) {
                    return;
                }
                if place_selected_2d(ctx, world) {
                    return;
                }
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
                if let Some(point) = ray_ground_point(ray) {
                    let place = if viewport_shift_down(ctx) {
                        snap_vec3(point, 1.0)
                    } else {
                        point
                    };
                    if viewport_alt_down(ctx) && duplicate_selected_node_at(ctx, None, Some(place)) {
                        return;
                    }
                    if place_selected_3d(ctx, point) {
                        return;
                    }
                }
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

fn viewport_alt_down<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) -> bool {
    key_down!(ctx.ipt, KeyCode::AltLeft) || key_down!(ctx.ipt, KeyCode::AltRight)
}

fn viewport_shift_down<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) -> bool {
    key_down!(ctx.ipt, KeyCode::ShiftLeft) || key_down!(ctx.ipt, KeyCode::ShiftRight)
}

fn duplicate_selected_node_at<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    pos2: Option<Vector2>,
    pos3: Option<Vector3>,
) -> bool {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
        if state.doc_text.is_empty() {
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let subtree_keys = collect_scene_subtree_keys(&doc, key);
        if subtree_keys.is_empty() {
            return false;
        }
        let mut map = Vec::new();
        let mut clones = Vec::new();
        for old_key in subtree_keys.iter().copied() {
            let Some(source) = doc
                .scene
                .nodes
                .iter()
                .find(|node| node.key.as_u32() == old_key)
                .cloned()
            else {
                continue;
            };
            let new_key = doc.scene.key_names.len() as u32;
            let source_name = doc.scene.key_name_or_id(source.key).to_string();
            let new_name = unique_node_name(&doc, &format!("{source_name}_copy"));
            doc.scene.key_names.to_mut().push(Cow::Owned(new_name));
            map.push((old_key, new_key));
            clones.push(source);
        }
        if clones.is_empty() {
            return false;
        }
        for mut node in clones {
            let old_key = node.key.as_u32();
            let Some(new_key) = mapped_scene_key(&map, old_key) else {
                continue;
            };
            node.key = SceneKey::new(new_key);
            if let Some(parent) = node.parent
                && let Some(new_parent) = mapped_scene_key(&map, parent.as_u32())
            {
                node.parent = Some(SceneKey::new(new_parent));
            }
            if old_key == key {
                if let Some(point) = pos2
                    && node.data.node_type.is_a(perro_scene::NodeType::Node2D)
                {
                    set_scene_vec2(&mut node.data, "position", point);
                }
                if let Some(point) = pos3
                    && node.data.node_type.is_a(perro_scene::NodeType::Node3D)
                {
                    set_scene_vec3(&mut node.data, "position", point);
                }
            }
            node.children = Cow::Owned(Vec::new());
            doc.scene.nodes.to_mut().push(node);
        }
        doc.normalize_links();
        state.doc_text = doc.to_text();
        state.selected_key = mapped_scene_key(&map, key);
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = format!("alt-place copy\nadd {} node", map.len());
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
    changed
}

fn place_selected_2d<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    world: Vector2,
) -> bool {
    let snap = key_down!(ctx.ipt, KeyCode::ShiftLeft) || key_down!(ctx.ipt, KeyCode::ShiftRight);
    let world = if snap { snap_vec2(world, 16.0) } else { world };
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
        if state.doc_text.is_empty() {
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            return false;
        };
        if !node.data.node_type.is_a(perro_scene::NodeType::Node2D) {
            return false;
        }
        set_scene_vec2(&mut node.data, "position", world);
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = if snap {
            format!("place 2d\npos=({:.2}, {:.2})\nsnap=16", world.x, world.y)
        } else {
            format!("place 2d\npos=({:.2}, {:.2})", world.x, world.y)
        };
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
    changed
}

fn place_selected_3d<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    point: Vector3,
) -> bool {
    let snap = key_down!(ctx.ipt, KeyCode::ShiftLeft) || key_down!(ctx.ipt, KeyCode::ShiftRight);
    let point = if snap { snap_vec3(point, 1.0) } else { point };
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
        if state.doc_text.is_empty() {
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            return false;
        };
        if !node.data.node_type.is_a(perro_scene::NodeType::Node3D) {
            return false;
        }
        set_scene_vec3(&mut node.data, "position", point);
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = if snap {
            format!(
                "place 3d\npos=({:.2}, {:.2}, {:.2})\nsnap=1",
                point.x, point.y, point.z
            )
        } else {
            format!(
                "place 3d\npos=({:.2}, {:.2}, {:.2})",
                point.x, point.y, point.z
            )
        };
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
    changed
}

fn snap_vec2(value: Vector2, grid: f32) -> Vector2 {
    Vector2::new((value.x / grid).round() * grid, (value.y / grid).round() * grid)
}

fn snap_vec3(value: Vector3, grid: f32) -> Vector3 {
    Vector3::new(
        (value.x / grid).round() * grid,
        (value.y / grid).round() * grid,
        (value.z / grid).round() * grid,
    )
}

fn snap_f32(value: f32, grid: f32) -> f32 {
    if grid <= 0.0 {
        value
    } else {
        (value / grid).round() * grid
    }
}

fn ray_ground_point(ray: ViewportRay3D) -> Option<Vector3> {
    if ray.direction.y.abs() < 0.0001 {
        return None;
    }
    let t = -ray.origin.y / ray.direction.y;
    if !t.is_finite() || t < 0.0 {
        return None;
    }
    Some(ray.origin + ray.direction * t)
}

fn viewport_pointer<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
) -> Option<ViewportPointer> {
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
        CameraProjection::Orthographic { size, .. } => Vector3::new(
            pointer.ndc.x * size * aspect * 0.5,
            pointer.ndc.y * size * 0.5,
            0.0,
        ),
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

        let res_changed = changed
            .iter()
            .any(|path| editor_file_watch::is_under_res(&root, path));
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

    if res_changed && let Ok(paths) = scan_res_paths(root.as_path()) {
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
    let preview_text =
        rewrite_project_res_paths(&SceneDoc::parse(doc_text), &project_root).to_text();
    let preview_path = PathBuf::from(&project_root)
        .join(".perro")
        .join(format!("editor_preview_{serial}.scn"));
    if let Err(err) = FileMod::save_string(preview_path.to_string_lossy().as_ref(), &preview_text) {
        set_log(ctx, &format!("preview write fail\n{path}\n{err}"));
        return;
    }

    let root = match ctx
        .run
        .Scene()
        .load(preview_path.to_string_lossy().to_string())
    {
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
        let light = create_node!(
            ctx.run,
            AmbientLight3D,
            "__editor_preview_ambient",
            tags![],
            root
        );
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
    if ctx
        .run
        .Nodes()
        .with_base_node::<UiBox, _, _>(root, |_| ())
        .is_some()
    {
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
    if let Some((handle, pointer)) =
        pointer.and_then(|pointer| pick_resize_handle(ctx, pointer).map(|handle| (handle, pointer)))
    {
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
        let delta = Vector2::new(
            pointer.uv.x - state.ui_drag_last_x,
            state.ui_drag_last_y - pointer.uv.y,
        );
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
    let snap = viewport_shift_down(ctx);
    if mode == "move" {
        move_doc_ui_node(ctx, key, root_delta, snap);
    } else if mode == "rotate" {
        rotate_doc_ui_node(ctx, key, root_delta, snap);
    } else {
        resize_doc_ui_node(ctx, key, &mode, root_delta, snap);
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
    snap: bool,
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
        let delta = Vector2::new(
            root_delta.x / parent_rect.size.x,
            root_delta.y / parent_rect.size.y,
        );
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            return false;
        };
        let current = scene_field_vec2(&node.data, "translation_ratio").unwrap_or(Vector2::ZERO);
        let next = if snap {
            snap_vec2(current + delta, 0.01)
        } else {
            current + delta
        };
        set_scene_vec2(&mut node.data, "translation_ratio", next);
        state.log = if snap {
            "move ui\nsnap=0.01".to_string()
        } else {
            "move ui".to_string()
        };
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
    snap: bool,
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
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            return false;
        };
        let anchor_text =
            scene_field_str(&node.data, "anchor").unwrap_or_else(|| "center".to_string());
        let anchor = scene_anchor_dir(&anchor_text);
        let anchor_point = parent_rect.center
            + Vector2::new(
                parent_rect.size.x * 0.5 * anchor.x,
                parent_rect.size.y * 0.5 * anchor.y,
            );
        let inward = Vector2::new(new_size.x * 0.5 * anchor.x, new_size.y * 0.5 * anchor.y);
        let mut translation = Vector2::new(
            (new_center.x - anchor_point.x + inward.x) / parent_rect.size.x,
            (new_center.y - anchor_point.y + inward.y) / parent_rect.size.y,
        );
        let mut size_ratio = Vector2::new(
            new_size.x / parent_rect.size.x,
            new_size.y / parent_rect.size.y,
        );
        if snap {
            translation = snap_vec2(translation, 0.01);
            size_ratio = snap_vec2(size_ratio, 0.01);
        }
        set_scene_vec2(&mut node.data, "size_ratio", size_ratio);
        set_scene_vec2(&mut node.data, "translation_ratio", translation);
        state.log = if snap {
            "resize ui\nsnap=0.01".to_string()
        } else {
            "resize ui".to_string()
        };
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
    snap: bool,
) {
    let (prev, curr) = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (
            Vector2::new(
                state.ui_drag_last_x - root_delta.x,
                1.0 - state.ui_drag_last_y - root_delta.y,
            ),
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
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            return false;
        };
        let current = scene_field_f32(&node.data, "rotation").unwrap_or(0.0);
        let next = if snap {
            snap_f32(current + delta, std::f32::consts::TAU / 24.0)
        } else {
            current + delta
        };
        set_scene_f32(&mut node.data, "rotation", next);
        state.log = if snap {
            "rotate ui\nsnap=15deg".to_string()
        } else {
            "rotate ui".to_string()
        };
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
        .find(|(_, center)| {
            (point.x - center.x).abs() <= 0.018 && (point.y - center.y).abs() <= 0.018
        })
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
        .find(|(_, center)| {
            (point.x - center.x).abs() <= 0.045 && (point.y - center.y).abs() <= 0.045
        })
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
        if node.parent.is_none()
            && doc.scene.root.map(|root| root.as_u32()) != Some(node.key.as_u32())
        {
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
    let anchor_point = parent.center
        + Vector2::new(
            parent.size.x * 0.5 * anchor.x,
            parent.size.y * 0.5 * anchor.y,
        );
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
    let node = doc
        .scene
        .nodes
        .iter()
        .find(|node| node.key.as_u32() == key)?;
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
    let node = doc
        .scene
        .nodes
        .iter()
        .find(|node| node.key.as_u32() == key)?;
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

fn selected_node_type_name(doc_text: &str, key: u32) -> Option<String> {
    let doc = SceneDoc::parse(doc_text);
    doc.scene
        .nodes
        .iter()
        .find(|node| node.key.as_u32() == key)
        .map(|node| node.data.type_name().to_string())
}

fn selected_node_viewport_mode(doc_text: &str, key: u32) -> Option<&'static str> {
    let doc = SceneDoc::parse(doc_text);
    let node_type = doc
        .scene
        .nodes
        .iter()
        .find(|node| node.key.as_u32() == key)?
        .data
        .node_type;
    viewport_mode_for_node_type(node_type)
}

fn viewport_mode_for_node_type(node_type: perro_scene::NodeType) -> Option<&'static str> {
    if node_type.is_a(perro_scene::NodeType::UiBox) {
        Some("UI")
    } else if node_type.is_a(perro_scene::NodeType::Node3D) {
        Some("3D")
    } else if node_type.is_a(perro_scene::NodeType::Node2D) {
        Some("2D")
    } else {
        None
    }
}

fn selected_node_field_text(doc_text: &str, key: u32, field: &str) -> Option<String> {
    let doc = SceneDoc::parse(doc_text);
    let node = doc
        .scene
        .nodes
        .iter()
        .find(|node| node.key.as_u32() == key)?;
    scene_field_value_text(&node.data, field)
}

fn scene_field_value_text(data: &SceneNodeData, field: &str) -> Option<String> {
    match scene_field(data, field)? {
        SceneValue::Str(value) => Some(value.to_string()),
        SceneValue::Key(key) => Some(key.to_string()),
        SceneValue::F32(value) => Some(value.to_string()),
        SceneValue::I32(value) => Some(value.to_string()),
        _ => None,
    }
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

fn set_scene_bool(data: &mut SceneNodeData, field: &str, value: bool) {
    let name = SceneFieldName::from_name(field.to_string());
    for (field_name, field_value) in data.fields.to_mut().iter_mut() {
        if field_name.as_ref() == field {
            *field_value = SceneValue::Bool(value);
            return;
        }
    }
    data.fields.to_mut().push((name, SceneValue::Bool(value)));
}

fn set_scene_vec3(data: &mut SceneNodeData, field: &str, value: Vector3) {
    let name = SceneFieldName::from_name(field.to_string());
    for (field_name, field_value) in data.fields.to_mut().iter_mut() {
        if field_name.as_ref() == field {
            *field_value = SceneValue::Vec3 {
                x: value.x,
                y: value.y,
                z: value.z,
            };
            return;
        }
    }
    data.fields.to_mut().push((
        name,
        SceneValue::Vec3 {
            x: value.x,
            y: value.y,
            z: value.z,
        },
    ));
}

fn set_scene_string(data: &mut SceneNodeData, field: &str, value: String) {
    let name = SceneFieldName::from_name(field.to_string());
    for (field_name, field_value) in data.fields.to_mut().iter_mut() {
        if field_name.as_ref() == field {
            *field_value = SceneValue::Str(Cow::Owned(value));
            return;
        }
    }
    data.fields
        .to_mut()
        .push((name, SceneValue::Str(Cow::Owned(value))));
}

fn set_scene_binding(data: &mut SceneNodeData, object: &str, node_name: &str) {
    let name = SceneFieldName::Bindings;
    for (field_name, field_value) in data.fields.to_mut().iter_mut() {
        if field_name.as_ref() == "bindings" {
            match field_value {
                SceneValue::Object(fields) => {
                    for (binding_name, binding_value) in fields.to_mut().iter_mut() {
                        if binding_name.as_ref() == object {
                            *binding_value =
                                SceneValue::Key(SceneValueKey::from(node_name.to_string()));
                            return;
                        }
                    }
                    fields.to_mut().push((
                        SceneFieldName::from_name(object.to_string()),
                        SceneValue::Key(SceneValueKey::from(node_name.to_string())),
                    ));
                }
                _ => {
                    *field_value = SceneValue::Object(Cow::Owned(vec![(
                        SceneFieldName::from_name(object.to_string()),
                        SceneValue::Key(SceneValueKey::from(node_name.to_string())),
                    )]));
                }
            }
            return;
        }
    }
    data.fields.to_mut().push((
        name,
        SceneValue::Object(Cow::Owned(vec![(
            SceneFieldName::from_name(object.to_string()),
            SceneValue::Key(SceneValueKey::from(node_name.to_string())),
        )])),
    ));
}

fn create_animation_for_selected_player<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let request = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = "anim create fail\nselect AnimationPlayer".to_string();
            return None;
        };
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(node_index) = doc
            .scene
            .nodes
            .iter()
            .position(|node| node.key.as_u32() == key)
        else {
            return None;
        };
        if doc.scene.nodes[node_index].data.type_name() != "AnimationPlayer" {
            state.log = "anim create fail\nselected node not AnimationPlayer".to_string();
            return None;
        }
        let player_name = doc
            .scene
            .key_name_or_id(doc.scene.nodes[node_index].key)
            .to_string();
        let target_key = doc.scene.nodes[node_index]
            .parent
            .unwrap_or(doc.scene.nodes[node_index].key);
        let target_name = doc.scene.key_name_or_id(target_key).to_string();
        let anim_name = format!("{}_clip", sanitize_file_stem(&player_name));
        let anim_path = unique_res_animation_path(&state.project_root, &anim_name);
        let abs = res_to_abs(&state.project_root, &anim_path);
        let text = default_animation_panim(&anim_name);
        set_scene_string(
            &mut doc.scene.nodes.to_mut()[node_index].data,
            "animation",
            anim_path.clone(),
        );
        set_scene_binding(
            &mut doc.scene.nodes.to_mut()[node_index].data,
            "Target",
            &target_name,
        );
        state.doc_text = doc.to_text();
        state.dirty = true;
        state.activity_mode = "anim".to_string();
        state.anim_drawer_open = true;
        state.active_anim_path = anim_path.clone();
        state.active_anim_player_key = Some(key);
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        Some((abs, text, anim_path))
    })
    .flatten();
    let Some((abs, text, anim_path)) = request else {
        refresh_all(ctx);
        return;
    };
    if let Some(parent) = Path::new(&abs).parent() {
        let _ = fs::create_dir_all(parent);
    }
    match FileMod::save_string(&abs, &text) {
        Ok(()) => {
            rebuild_preview(ctx);
            let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                if let Ok(paths) = scan_res_paths(Path::new(&state.project_root)) {
                    state.file_paths = paths;
                }
                state.log = format!("create animation\n{}", editor_files::rel_label(&anim_path));
            });
            refresh_all(ctx);
        }
        Err(err) => set_log(ctx, &format!("anim write fail\n{anim_path}\n{err}")),
    }
}

fn add_track_for_selected_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let request = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(player_key) = state.active_anim_player_key else {
            state.log = "track add fail\nselect AnimationPlayer first".to_string();
            return None;
        };
        let Some(target_key) = state.selected_key else {
            state.log = "track add fail\nselect target node".to_string();
            return None;
        };
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(player_index) = doc
            .scene
            .nodes
            .iter()
            .position(|node| node.key.as_u32() == player_key)
        else {
            state.log = "track add fail\nmissing AnimationPlayer".to_string();
            return None;
        };
        let Some(target) = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == target_key)
        else {
            state.log = "track add fail\nmissing target".to_string();
            return None;
        };
        if doc.scene.nodes[player_index].data.type_name() != "AnimationPlayer" {
            state.log = "track add fail\nactive node not AnimationPlayer".to_string();
            return None;
        }
        let target_name = doc.scene.key_name_or_id(target.key).to_string();
        let object_name =
            unique_panim_object_name(&target_name, &state.active_anim_path, &state.project_root);
        let target_type = target.data.type_name().to_string();
        let mut anim_path = state.active_anim_path.clone();
        if anim_path.is_empty() || anim_path == "-" {
            anim_path = selected_node_field_text(&state.doc_text, player_key, "animation")
                .unwrap_or_else(|| "-".to_string());
        }
        if anim_path == "-" || anim_path.is_empty() {
            let player_name = doc
                .scene
                .key_name_or_id(doc.scene.nodes[player_index].key)
                .to_string();
            let anim_name = format!("{}_clip", sanitize_file_stem(&player_name));
            anim_path = unique_res_animation_path(&state.project_root, &anim_name);
            set_scene_string(
                &mut doc.scene.nodes.to_mut()[player_index].data,
                "animation",
                anim_path.clone(),
            );
        }
        set_scene_binding(
            &mut doc.scene.nodes.to_mut()[player_index].data,
            &object_name,
            &target_name,
        );
        state.doc_text = doc.to_text();
        state.dirty = true;
        state.activity_mode = "anim".to_string();
        state.anim_drawer_open = true;
        state.active_anim_path = anim_path.clone();
        state.active_glb_path.clear();
        state.active_glb_summary.clear();
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        Some((
            res_to_abs(&state.project_root, &anim_path),
            anim_path,
            object_name,
            target_type,
            target_name,
        ))
    })
    .flatten();
    let Some((abs, anim_path, object_name, target_type, target_name)) = request else {
        refresh_all(ctx);
        return;
    };
    if let Some(parent) = Path::new(&abs).parent() {
        let _ = fs::create_dir_all(parent);
    }
    let current = FileMod::load_string(&abs).unwrap_or_else(|_| {
        let stem = Path::new(&anim_path)
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or("clip");
        default_animation_panim(stem)
    });
    let next = add_panim_track_text(&current, &object_name, &target_type);
    match FileMod::save_string(&abs, &next) {
        Ok(()) => {
            rebuild_preview(ctx);
            let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                if let Ok(paths) = scan_res_paths(Path::new(&state.project_root)) {
                    state.file_paths = paths;
                }
                state.log = format!("add track\n{object_name} -> {target_name}");
            });
            refresh_all(ctx);
        }
        Err(err) => set_log(ctx, &format!("track write fail\n{anim_path}\n{err}")),
    }
}

fn default_animation_panim(animation_name: &str) -> String {
    format!(
        "[Animation]\nname = \"{animation_name}\"\nfps = 60\ndefault_interp = \"interpolate\"\ndefault_ease = \"linear\"\n[/Animation]\n\n[Objects]\nTarget = Node3D\n[/Objects]\n\n[Frame0]\n@Target {{\n    position = (0, 0, 0)\n}}\n[/Frame0]\n\n[Frame30]\n@Target {{\n    position = (2, 0, 0)\n}}\n[/Frame30]\n"
    )
}

fn unique_panim_object_name(node_name: &str, anim_path: &str, project_root: &str) -> String {
    let base = sanitize_panim_ident(node_name);
    let existing = if anim_path.is_empty() || anim_path == "-" {
        String::new()
    } else {
        FileMod::load_string(&res_to_abs(project_root, anim_path)).unwrap_or_default()
    };
    if !panim_has_object(&existing, &base) {
        return base;
    }
    for idx in 1..1000 {
        let name = format!("{base}_{idx}");
        if !panim_has_object(&existing, &name) {
            return name;
        }
    }
    format!("{base}_x")
}

fn sanitize_panim_ident(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else if ch.is_whitespace() || ch == '-' || ch == '.' {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push_str("Track");
    }
    if out.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

fn panim_has_object(text: &str, object: &str) -> bool {
    let mut in_objects = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "[Objects]" {
            in_objects = true;
            continue;
        }
        if trimmed == "[/Objects]" {
            break;
        }
        if in_objects
            && trimmed
                .split_once('=')
                .map(|(name, _)| name.trim() == object)
                .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

fn add_panim_track_text(text: &str, object: &str, node_type: &str) -> String {
    let mut out = if panim_has_object(text, object) {
        text.to_string()
    } else {
        insert_panim_object(text, object, node_type)
    };
    if !panim_frame_has_object(&out, 0, object) {
        out = insert_panim_frame0_object(&out, object);
    }
    out
}

fn insert_panim_object(text: &str, object: &str, node_type: &str) -> String {
    if let Some(pos) = text.find("[/Objects]") {
        let mut out = String::with_capacity(text.len() + object.len() + node_type.len() + 8);
        out.push_str(&text[..pos]);
        out.push_str(&format!("{object} = {node_type}\n"));
        out.push_str(&text[pos..]);
        return out;
    }
    format!("{text}\n[Objects]\n{object} = {node_type}\n[/Objects]\n")
}

fn panim_frame_has_object(text: &str, frame: u32, object: &str) -> bool {
    let start_tag = format!("[Frame{frame}]");
    let end_tag = format!("[/Frame{frame}]");
    let Some(start) = text.find(&start_tag) else {
        return false;
    };
    let end = text[start..]
        .find(&end_tag)
        .map(|offset| start + offset)
        .unwrap_or(text.len());
    text[start..end]
        .lines()
        .any(|line| line.trim() == format!("@{object} {{"))
}

fn insert_panim_frame0_object(text: &str, object: &str) -> String {
    let block = format!("@{object} {{\n    position = (0, 0, 0)\n}}\n");
    if let Some(pos) = text.find("[/Frame0]") {
        let mut out = String::with_capacity(text.len() + block.len());
        out.push_str(&text[..pos]);
        out.push_str(&block);
        out.push_str(&text[pos..]);
        return out;
    }
    format!("{text}\n[Frame0]\n{block}[/Frame0]\n")
}

fn sanitize_file_stem(text: &str) -> String {
    let out = text
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    out.trim_matches('_').to_ascii_lowercase()
}

fn unique_res_animation_path(project_root: &str, stem: &str) -> String {
    for idx in 0..1000 {
        let suffix = if idx == 0 {
            String::new()
        } else {
            format!("_{idx}")
        };
        let path = format!("res://animations/{stem}{suffix}.panim");
        if !Path::new(&res_to_abs(project_root, &path)).exists() {
            return path;
        }
    }
    format!("res://animations/{stem}_x.panim")
}

fn edit_selected_transform<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    field: &str,
    text_box: &str,
) {
    let Some(text) = read_text_box(ctx, text_box) else {
        return;
    };
    let Some(values) = parse_number_list(&text) else {
        set_log(ctx, "inspector edit fail\nbad number list");
        return;
    };
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
        if state.doc_text.is_empty() {
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            return false;
        };
        match values.as_slice() {
            [x] => set_scene_f32(&mut node.data, field, *x),
            [x, y] => set_scene_vec2(&mut node.data, field, Vector2::new(*x, *y)),
            [x, y, z] => set_scene_vec3(&mut node.data, field, Vector3::new(*x, *y, *z)),
            _ => return false,
        }
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

fn reset_selected_transform<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
        if state.doc_text.is_empty() {
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            return false;
        };
        if node.data.node_type.is_a(perro_scene::NodeType::Node3D) {
            set_scene_vec3(&mut node.data, "position", Vector3::ZERO);
            set_scene_vec3(&mut node.data, "rotation", Vector3::ZERO);
            set_scene_vec3(&mut node.data, "scale", Vector3::ONE);
        } else if node.data.node_type.is_a(perro_scene::NodeType::Node2D) {
            set_scene_vec2(&mut node.data, "position", Vector2::ZERO);
            set_scene_f32(&mut node.data, "rotation", 0.0);
            set_scene_vec2(&mut node.data, "scale", Vector2::ONE);
        } else if node.data.node_type.is_a(perro_scene::NodeType::UiBox) {
            set_scene_vec2(&mut node.data, "translation_ratio", Vector2::ZERO);
            set_scene_f32(&mut node.data, "rotation", 0.0);
        } else {
            state.log = "reset xform skip\nselect spatial/ui node".to_string();
            return false;
        }
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = "reset xform\nAlt+R".to_string();
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
}

fn nudge_selected_node<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    dx: f32,
    dy: f32,
    fine: bool,
) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
        if state.doc_text.is_empty() {
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key.as_u32() == key)
        else {
            return false;
        };
        if node.data.node_type.is_a(perro_scene::NodeType::Node2D) {
            let step = if fine { 1.0 } else { 16.0 };
            let current = find_vec2_value(&node.data, "position").unwrap_or(Vector2::ZERO);
            let next = current + Vector2::new(dx * step, dy * step);
            set_scene_vec2(&mut node.data, "position", next);
            state.log = format!("nudge 2d\npos=({:.2}, {:.2})", next.x, next.y);
        } else if node.data.node_type.is_a(perro_scene::NodeType::Node3D) {
            let step = if fine { 0.1 } else { 1.0 };
            let current = find_vec3_value(&node.data, "position").unwrap_or(Vector3::ZERO);
            let next = current + Vector3::new(dx * step, 0.0, -dy * step);
            set_scene_vec3(&mut node.data, "position", next);
            state.log = format!(
                "nudge 3d\npos=({:.2}, {:.2}, {:.2})",
                next.x, next.y, next.z
            );
        } else if node.data.node_type.is_a(perro_scene::NodeType::UiBox) {
            let step = if fine { 0.002 } else { 0.01 };
            let current =
                scene_field_vec2(&node.data, "translation_ratio").unwrap_or(Vector2::ZERO);
            let next = current + Vector2::new(dx * step, dy * step);
            set_scene_vec2(&mut node.data, "translation_ratio", next);
            state.log = format!("nudge ui\npos=({:.3}, {:.3})", next.x, next.y);
        } else {
            state.log = "nudge skip\nselect spatial/ui node".to_string();
            return false;
        }
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

fn rename_selected_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(text) = read_text_box(ctx, "inspector_name_box") else {
        return;
    };
    let requested = sanitize_node_name(&text);
    if requested.is_empty() {
        set_log(ctx, "rename fail\nempty name");
        return;
    }
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
        if state.doc_text.is_empty() {
            return false;
        }
        let mut doc = SceneDoc::parse(&state.doc_text);
        let idx = key as usize;
        if idx >= doc.scene.key_names.len() {
            state.log = "rename fail\nbad key".to_string();
            return false;
        }
        let current = doc.scene.key_name_or_id(SceneKey::new(key)).to_string();
        let next = if current == requested {
            requested.clone()
        } else {
            unique_node_name(&doc, &requested)
        };
        if current == next {
            return false;
        }
        doc.scene.key_names.to_mut()[idx] = Cow::Owned(next.clone());
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = format!("rename node\n{current} -> {next}");
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
}

fn parse_number_list(text: &str) -> Option<Vec<f32>> {
    let values = text
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split([',', ' '])
        .filter(|part| !part.trim().is_empty())
        .map(|part| part.trim().parse::<f32>())
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (!values.is_empty() && values.len() <= 3).then_some(values)
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
    set_button_fill(
        ctx,
        "activity_scene_button",
        if view.sidebar_mode == "scene" {
            "#54657A"
        } else {
            "#3D4654"
        },
    );
    set_button_fill(
        ctx,
        "activity_files_button",
        if view.sidebar_mode == "files" {
            "#54657A"
        } else {
            "#3D4654"
        },
    );
    set_button_fill(
        ctx,
        "activity_anim_button",
        if view.activity_mode == "anim" {
            "#54657A"
        } else {
            "#3D4654"
        },
    );
    set_ui_display(ctx, "log_title", !view.anim_drawer_open);
    set_ui_display(ctx, "log_text", !view.anim_drawer_open);
    set_ui_display(ctx, "anim_drawer", view.anim_drawer_open);
    set_ui_display(ctx, "anim_create_button", view.anim_can_create);
    set_ui_display(ctx, "anim_add_track_button", view.anim_can_add_track);
    set_label(ctx, "anim_drawer_title", &view.anim_title);
    set_label(ctx, "anim_status_text", &view.anim_status);
    set_label(ctx, "anim_tracks_text", &view.anim_tracks);
    set_ui_display(ctx, "scene_tree_title", view.sidebar_mode == "scene");
    set_ui_display(ctx, "scene_filter_box", view.sidebar_mode == "scene");
    set_text_box(ctx, "scene_filter_box", &view.scene_filter);
    set_ui_display(ctx, "scene_rows", view.sidebar_mode == "scene");
    set_ui_display(ctx, "file_title", view.sidebar_mode == "files");
    set_label(ctx, "file_title", &view.file_title);
    set_ui_display(ctx, "file_action_row", view.sidebar_mode == "files");
    set_ui_display(ctx, "file_filter_box", view.sidebar_mode == "files");
    set_text_box(ctx, "file_filter_box", &view.file_filter);
    set_ui_display(ctx, "file_rows", view.sidebar_mode == "files");
    set_ui_box_size(ctx, "scene_rows", (1.0, 0.86));
    set_ui_box_size(ctx, "file_rows", (1.0, 0.75));

    for idx in 0..MAX_RECENT {
        let text = view
            .recent_projects
            .get(idx)
            .cloned()
            .unwrap_or_else(|| "-".to_string());
        set_label(
            ctx,
            &format!("manager_recent_{idx}_label"),
            &editor_view::short_path(&text, 44),
        );
    }
    set_label(
        ctx,
        "create_location_label",
        &format!(
            "location: {}",
            editor_view::short_path(&view.create_parent_dir, 34)
        ),
    );

    set_label(ctx, "add_node_page_label", &view.node_picker_page);
    set_label(ctx, "add_node_parent_label", &view.node_picker_parent);
    set_text_box(ctx, "add_node_search_box", &view.node_picker_filter);
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
            .map(|path| {
                format!(
                    "{}  {}",
                    editor_files::display_kind_label(path),
                    file_row_label(path, &view.file_scope)
                )
            })
            .unwrap_or_else(|| "-".to_string());
        set_label(
            ctx,
            &format!("file_row_{idx}_label"),
            &editor_view::short_path(&text, 28),
        );
        set_button_fill(
            ctx,
            &format!("file_row_{idx}"),
            if view.file_paths.get(idx) == Some(&view.active_asset_path) {
                "#54657A"
            } else {
                "#39414E"
            },
        );
    }
    apply_file_tree_layout(ctx);

    for idx in 0..MAX_TABS {
        let text = view
            .open_paths
            .get(idx)
            .map(|path| {
                let mark = if view.dirty_scene_paths.iter().any(|dirty| dirty == path) {
                    "* "
                } else {
                    ""
                };
                format!("{mark}{}", editor_files::rel_label(path))
            })
            .unwrap_or_else(|| "-".to_string());
        set_label(
            ctx,
            &format!("scene_tab_{idx}_label"),
            &editor_view::short_path(&text, 24),
        );
        set_ui_display(
            ctx,
            &format!("scene_tab_close_{idx}"),
            view.open_paths.get(idx).is_some(),
        );
        set_button_fill(
            ctx,
            &format!("scene_tab_{idx}"),
            if idx == view.active_open {
                "#54657A"
            } else {
                "#3D4654"
            },
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
            if view.selected_row == Some(idx) {
                "#54657A"
            } else {
                "#39414E"
            },
        );
    }
    apply_scene_list_layout(ctx);
    apply_viewport_mode(ctx, &view.viewport_mode);
    apply_editor_gizmos(ctx, &view.gizmo, &view.viewport_mode);
    apply_selected_ui_overlay(ctx, view.selected_ui_rect);

    set_label(ctx, "inspector_title", &view.inspector_title);
    set_label(ctx, "inspector_name", &view.inspector_name);
    set_ui_display(ctx, "inspector_name_box", view.inspector_node_actions);
    set_text_box(ctx, "inspector_name_box", &view.inspector_name_edit);
    set_label(ctx, "inspector_type", &view.inspector_type);
    set_label(ctx, "inspector_parent", &view.inspector_parent);
    set_ui_display(ctx, "inspector_action_row", view.inspector_node_actions);
    set_ui_display(ctx, "asset_action_row", view.inspector_asset_actions);
    set_ui_display(ctx, "asset_glb_anim_button", view.inspector_glb_asset_actions);
    set_ui_display(ctx, "asset_glb_mat_button", view.inspector_glb_asset_actions);
    set_label(ctx, "inspector_pos", &view.inspector_pos_label);
    set_text_box(ctx, "inspector_position_box", &view.inspector_pos);
    set_label(ctx, "inspector_rotation_label", &view.inspector_rotation_label);
    set_text_box(ctx, "inspector_rotation_box", &view.inspector_rotation);
    set_label(ctx, "inspector_scale_label", &view.inspector_scale_label);
    set_text_box(ctx, "inspector_scale_box", &view.inspector_scale);
    set_label(
        ctx,
        "inspector_script",
        &view.inspector_script,
    );
    set_label(
        ctx,
        "inspector_vars",
        &view.inspector_vars,
    );
}

#[derive(Default)]
struct EditorView {
    project_root: String,
    project_name: String,
    create_parent_dir: String,
    recent_projects: Vec<String>,
    file_paths: Vec<String>,
    file_filter: String,
    file_scope: String,
    file_title: String,
    active_asset_path: String,
    scene_paths: Vec<String>,
    open_paths: Vec<String>,
    dirty_scene_paths: Vec<String>,
    active_open: usize,
    nodes: Vec<String>,
    selected_row: Option<usize>,
    inspector_title: String,
    inspector_name: String,
    inspector_name_edit: String,
    inspector_type: String,
    inspector_parent: String,
    inspector_node_actions: bool,
    inspector_asset_actions: bool,
    inspector_glb_asset_actions: bool,
    inspector_pos_label: String,
    inspector_pos: String,
    inspector_rotation_label: String,
    inspector_rotation: String,
    inspector_scale_label: String,
    inspector_scale: String,
    inspector_script: String,
    inspector_vars: String,
    viewport: String,
    status: String,
    log: String,
    viewport_mode: String,
    activity_mode: String,
    sidebar_mode: String,
    scene_filter: String,
    anim_drawer_open: bool,
    anim_title: String,
    anim_status: String,
    anim_tracks: String,
    anim_can_create: bool,
    anim_can_add_track: bool,
    gizmo: editor_gizmos::GizmoView,
    selected_ui_rect: Option<EditorUiRect>,
    node_picker_rows: Vec<String>,
    node_picker_page: String,
    node_picker_filter: String,
    node_picker_parent: String,
}

impl EditorView {
    fn from_state(state: &EditorState) -> Self {
        let mut nodes = Vec::new();
        let mut selected_row = None;
        let mut inspector_title = "Inspector".to_string();
        let mut inspector_name = "name: -".to_string();
        let mut inspector_name_edit = "-".to_string();
        let mut inspector_type = "type: -".to_string();
        let mut inspector_parent = "parent: -".to_string();
        let mut inspector_node_actions = false;
        let mut inspector_asset_actions = false;
        let mut inspector_glb_asset_actions = false;
        let mut inspector_pos_label = "position".to_string();
        let mut inspector_pos = "-".to_string();
        let mut inspector_rotation_label = "rotation".to_string();
        let mut inspector_rotation = "-".to_string();
        let mut inspector_scale_label = "scale".to_string();
        let mut inspector_scale = "-".to_string();
        let mut inspector_script = "script: -".to_string();
        let mut inspector_vars = "script vars: -".to_string();
        let mut gizmo = editor_gizmos::GizmoView::default();
        let mut selected_ui_rect = None;

        if !state.doc_text.is_empty() {
            let doc = SceneDoc::parse(&state.doc_text);
            let tree = scene_tree_view(
                &doc,
                state.selected_key,
                &state.scene_filter,
                &state.collapsed_scene_keys,
            );
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
                inspector_name = format!("name: {}", doc.scene.key_name_or_id(node.key));
                inspector_name_edit = doc.scene.key_name_or_id(node.key).to_string();
                inspector_type = format!("type: {}", node.data.type_name());
                inspector_parent = format!(
                    "path: {}\nchildren: {}",
                    scene_node_path(&doc, node.key),
                    scene_child_count(&doc, node.key.as_u32())
                );
                inspector_pos = find_position_text(&node.data).unwrap_or_else(|| "-".to_string());
                inspector_rotation = find_scene_value_text(&node.data, "rotation")
                    .unwrap_or_else(|| "-".to_string());
                inspector_scale =
                    find_scene_value_text(&node.data, "scale").unwrap_or_else(|| "-".to_string());
                let script = node
                    .script
                    .as_ref()
                    .map(|v| v.as_ref())
                    .unwrap_or("-");
                let root_of = node
                    .root_of
                    .as_ref()
                    .map(|v| v.as_ref())
                    .unwrap_or("-");
                inspector_script = format!("script: {script}\nroot_of: {root_of}");
                let vars = if node.script_vars.is_empty() {
                    "script vars: -".to_string()
                } else {
                    format!("script vars: {} fields", node.script_vars.len())
                };
                let refs = selected_node_asset_refs(node);
                let refs = if refs.is_empty() {
                    "refs: -".to_string()
                } else {
                    format!("refs:\n{}", refs.join("\n"))
                };
                let visible = scene_field_bool(&node.data, "visible").unwrap_or(true);
                inspector_vars = format!(
                    "{vars}\nvisible: {visible}\n{refs}\nkeys: Left/Right fold  Backspace parent  Ctrl+F find  Enter ref/frame  Ctrl+G ref  Ctrl+Shift+G users  Ctrl+H visible  Ctrl+B side  Alt+R reset  Shift+Arrows nudge"
                );
                inspector_node_actions = true;
            }
        }

        if state.sidebar_mode == "files" && !state.active_asset_path.is_empty() {
            let asset = asset_inspector_text(state);
            inspector_title = "Asset".to_string();
            inspector_name = asset.name;
            inspector_name_edit = "-".to_string();
            inspector_type = asset.kind;
            inspector_parent = asset.path;
            inspector_pos_label = "size".to_string();
            inspector_pos = asset.size;
            inspector_rotation_label = "refs".to_string();
            inspector_rotation = asset.refs;
            inspector_scale_label = "state".to_string();
            inspector_scale = asset.state;
            inspector_script = asset.detail;
            inspector_vars = asset.actions;
            inspector_node_actions = false;
            inspector_asset_actions = true;
            inspector_glb_asset_actions = is_gltf_path(&state.active_asset_path);
        }

        let status = editor_status_text(state);
        let viewport = format!(
            "Viewport  mode={}  cam=({:.1}, {:.1}, {:.1})",
            state.viewport_mode, state.cam_x, state.cam_y, state.cam_z
        );
        let (anim_title, anim_status, anim_tracks, anim_can_create, anim_can_add_track) =
            animation_drawer_text(state);
        let node_picker_rows = picker_rows(state, &state.node_picker_filter, state.node_picker_offset);
        let page = (state.node_picker_offset / MAX_NODE_PICKER_ROWS) + 1;
        let picker_count = picker_node_types(state, &state.node_picker_filter).len().max(1);
        let page_count = picker_count
            .div_ceil(MAX_NODE_PICKER_ROWS);
        let node_picker_parent = picker_parent_text(state);
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
            file_paths: filtered_file_paths(state),
            file_filter: state.file_filter.clone(),
            file_scope: state.file_scope.clone(),
            file_title: file_panel_title(state),
            active_asset_path: state.active_asset_path.clone(),
            scene_paths: state.scene_paths.clone(),
            open_paths: state.open_paths.clone(),
            dirty_scene_paths: state.dirty_scene_paths.clone(),
            active_open: state.active_open,
            nodes,
            selected_row,
            inspector_title,
            inspector_name,
            inspector_name_edit,
            inspector_type,
            inspector_parent,
            inspector_node_actions,
            inspector_asset_actions,
            inspector_glb_asset_actions,
            inspector_pos_label,
            inspector_pos,
            inspector_rotation_label,
            inspector_rotation,
            inspector_scale_label,
            inspector_scale,
            inspector_script,
            inspector_vars,
            viewport,
            status,
            log: state.log.clone(),
            viewport_mode: state.viewport_mode.clone(),
            activity_mode: state.activity_mode.clone(),
            sidebar_mode: state.sidebar_mode.clone(),
            scene_filter: state.scene_filter.clone(),
            anim_drawer_open: state.anim_drawer_open,
            anim_title,
            anim_status,
            anim_tracks,
            anim_can_create,
            anim_can_add_track,
            gizmo,
            selected_ui_rect,
            node_picker_rows,
            node_picker_page: format!("page {page}/{page_count}"),
            node_picker_filter: state.node_picker_filter.clone(),
            node_picker_parent,
        }
    }
}

struct AssetInspectorText {
    name: String,
    kind: String,
    path: String,
    size: String,
    refs: String,
    state: String,
    detail: String,
    actions: String,
}

fn editor_status_text(state: &EditorState) -> String {
    if state.project_root.is_empty() {
        return "ready | open project".to_string();
    }
    let node_count = if state.doc_text.is_empty() {
        0
    } else {
        SceneDoc::parse(&state.doc_text).scene.nodes.len()
    };
    let file_count = filtered_file_paths(state).len();
    let scope = if state.file_scope.is_empty() {
        "res://".to_string()
    } else {
        editor_view::short_path(&state.file_scope, 22)
    };
    let filters = [
        (!state.scene_filter.is_empty()).then_some("scene-filter"),
        (!state.file_filter.is_empty()).then_some("file-filter"),
        (!state.node_picker_filter.is_empty()).then_some("node-filter"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let filters = if filters.is_empty() {
        "-".to_string()
    } else {
        filters.join(",")
    };
    format!(
        "ready | {} | tabs={} dirty={} nodes={} files={} scope={} filters={} quick=CtrlAlt1-7",
        state.project_name,
        state.open_paths.len(),
        state.dirty,
        node_count,
        file_count,
        scope,
        filters
    )
}

fn asset_inspector_text(state: &EditorState) -> AssetInspectorText {
    let path = state.active_asset_path.as_str();
    let kind = editor_files::kind_label(path);
    let rel = editor_files::rel_label(path);
    let abs = res_to_abs(&state.project_root, path);
    let size = if path.ends_with('/') {
        "folder".to_string()
    } else {
        fs::metadata(&abs)
            .map(|meta| format!("{} bytes", meta.len()))
            .unwrap_or_else(|_| "missing".to_string())
    };
    let refs = asset_ref_text(state, path, kind);
    let detail = asset_detail_text(state, path, kind);
    let actions = match kind {
        "scene" => {
            "Enter -> open scene\nCtrl+Shift+Enter -> instance scene\nCtrl+Shift+G -> find user\nCtrl+E -> reveal tab".to_string()
        }
        "mesh" if is_gltf_path(path) => {
            "Use -> bind ref\nNode -> mesh node\nCtrl+Shift+G -> find user\n[] -> mesh  Shift+[] -> mat\nAnim -> .panim\nMat -> .pmat".to_string()
        }
        "mesh" => "Ctrl+Enter -> use\nCtrl+Shift+Enter -> node\nCtrl+Shift+G -> find user".to_string(),
        "resource" if path.ends_with(".panim") => {
            "Ctrl+Enter -> bind clip\nCtrl+Shift+G -> find user\nNew Track -> bind node".to_string()
        }
        "folder" => "Enter -> scope folder\nBackspace -> parent\nEsc -> root/filter clear".to_string(),
        _ => "Enter -> inspect\nCtrl+Enter -> use\nCtrl+Shift+Enter -> node\nCtrl+Shift+G -> find user".to_string(),
    };
    AssetInspectorText {
        name: format!("name: {rel}"),
        kind: format!("type: {kind}"),
        path: format!("path: {path}"),
        size,
        refs,
        state: if Path::new(&abs).exists() || path.ends_with('/') {
            "ok".to_string()
        } else {
            "missing".to_string()
        },
        detail,
        actions,
    }
}

fn asset_ref_text(state: &EditorState, path: &str, kind: &str) -> String {
    let users = asset_user_text(state, path);
    if path.ends_with(".glb") || path.ends_with(".gltf") {
        return format!(
            "{}\n{}\n{}\n{}",
            indexed_ref(path, "mesh", usize::MAX, state.active_glb_mesh_index),
            indexed_ref(path, "mat", usize::MAX, state.active_glb_mat_index),
            indexed_ref(path, "animation", usize::MAX, state.active_glb_anim_index),
            users
        );
    }
    match kind {
        "scene" | "script" | "resource" | "mesh" | "image" | "audio" => {
            format!("{path}\n{users}")
        }
        _ => users,
    }
}

fn asset_user_text(state: &EditorState, path: &str) -> String {
    if state.doc_text.is_empty() || path.ends_with('/') {
        return "users: -".to_string();
    }
    let doc = SceneDoc::parse(&state.doc_text);
    let mut users = Vec::new();
    for node in doc.scene.nodes.iter() {
        if !node_uses_asset_path(node, path) {
            continue;
        }
        users.push(format!(
            "{} : {}",
            doc.scene.key_name_or_id(node.key),
            node.data.type_name()
        ));
        if users.len() >= 4 {
            break;
        }
    }
    let total = doc
        .scene
        .nodes
        .iter()
        .filter(|node| node_uses_asset_path(node, path))
        .count();
    if users.is_empty() {
        "users: -".to_string()
    } else if total > users.len() {
        format!("users: {total}\n{}\n+{} more", users.join("\n"), total - users.len())
    } else {
        format!("users: {total}\n{}", users.join("\n"))
    }
}

fn asset_detail_text(state: &EditorState, path: &str, kind: &str) -> String {
    if path.ends_with(".glb") || path.ends_with(".gltf") {
        return if state.active_glb_path == path && !state.active_glb_summary.is_empty() {
            state.active_glb_summary.clone()
        } else {
            "GLB asset\nopen row -> inspect meshes/materials/animations".to_string()
        };
    }
    if path.ends_with(".panim") {
        return panim_summary(&state.project_root, path);
    }
    if kind == "script" || (kind == "resource" && !path.ends_with(".panim")) {
        return text_asset_preview(&state.project_root, path);
    }
    if kind == "scene" && !state.doc_text.is_empty() && state.open_paths.get(state.active_open).map(String::as_str) == Some(path) {
        let doc = SceneDoc::parse(&state.doc_text);
        return format!("nodes={}\nmode={}", doc.scene.nodes.len(), editor_scene::root_viewport_mode(&doc));
    }
    format!("{kind} asset")
}

fn text_asset_preview(project_root: &str, path: &str) -> String {
    let abs = res_to_abs(project_root, path);
    let Ok(text) = FileMod::load_string(&abs) else {
        return "text preview\nnot readable".to_string();
    };
    let lines = text
        .lines()
        .take(8)
        .map(|line| {
            let line = line.trim_end();
            let mut short = line.chars().take(72).collect::<String>();
            if line.chars().count() > 72 {
                short.push_str("...");
                short
            } else {
                short
            }
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        "text preview\n(empty)".to_string()
    } else {
        format!("text preview\n{}", lines.join("\n"))
    }
}

fn animation_drawer_text(state: &EditorState) -> (String, String, String, bool, bool) {
    if !state.active_glb_path.is_empty() {
        return (
            format!(
                "GLB Viewer  {}",
                editor_files::rel_label(&state.active_glb_path)
            ),
            "container refs stay usable in scene fields".to_string(),
            state.active_glb_summary.clone(),
            false,
            false,
        );
    }
    let Some(key) = state.active_anim_player_key else {
        if state.active_anim_path.is_empty() {
            return (
                "Animation".to_string(),
                "select AnimationPlayer or open .panim".to_string(),
                "no live binding".to_string(),
                false,
                false,
            );
        }
        return (
            format!(
                "Animation Data  {}",
                editor_files::rel_label(&state.active_anim_path)
            ),
            ".panim data view\nno scene binding until selected AnimationPlayer references it"
                .to_string(),
            panim_summary(&state.project_root, &state.active_anim_path),
            false,
            false,
        );
    };
    let doc = SceneDoc::parse(&state.doc_text);
    let name = doc
        .scene
        .nodes
        .iter()
        .find(|node| node.key.as_u32() == key)
        .map(|node| doc.scene.key_name_or_id(node.key).to_string())
        .unwrap_or_else(|| "AnimationPlayer".to_string());
    let path = if state.active_anim_path.is_empty() {
        selected_node_field_text(&state.doc_text, key, "animation")
            .unwrap_or_else(|| "-".to_string())
    } else {
        state.active_anim_path.clone()
    };
    (
        format!("Animation Player  {name}"),
        format!(
            "player={name}\ncurrent animations:\n{path}\ncreate writes .panim + binds Target to parent node"
        ),
        if path == "-" {
            "no clip bound".to_string()
        } else {
            panim_summary(&state.project_root, &path)
        },
        true,
        true,
    )
}

fn gltf_summary(
    path: &str,
    mesh_count: usize,
    material_count: usize,
    animation_count: usize,
    skeleton_count: usize,
    texture_count: usize,
    node_count: usize,
    scene_count: usize,
    mesh_index: usize,
    mat_index: usize,
    anim_index: usize,
) -> String {
    format!(
        "GLB  {}\nselected:\nmesh = {}\nmat = {}\nanim = {}\nmeshes: {}\n{}\nmaterials: {}\n{}\nanimations: {}\n{}\nskins: {}\ntextures: {}\nnodes: {} scenes: {}\nkeys: [] mesh  Shift+[] mat  Ctrl+Shift+[] anim\nconvert:\n- anim -> perro_cli import_anim {} --output res/animations/<clip>.panim --clip {}\n- mesh -> static pipeline emits {}:mesh[index] pmesh entries\n- mat -> static pipeline emits {}:mat[index] pmat refs",
        editor_files::rel_label(path),
        indexed_ref(path, "mesh", mesh_count, mesh_index),
        indexed_ref(path, "mat", material_count, mat_index),
        indexed_ref(path, "animation", animation_count, anim_index),
        mesh_count,
        indexed_refs(path, "mesh", mesh_count),
        material_count,
        indexed_refs(path, "mat", material_count),
        animation_count,
        indexed_refs(path, "animation", animation_count),
        skeleton_count,
        texture_count,
        node_count,
        scene_count,
        editor_files::rel_label(path),
        anim_index,
        path,
        path
    )
}

fn indexed_ref(path: &str, kind: &str, count: usize, index: usize) -> String {
    if count == 0 {
        "-".to_string()
    } else {
        format!("{path}:{kind}[{}]", index.min(count - 1))
    }
}

fn indexed_refs(path: &str, kind: &str, count: usize) -> String {
    if count == 0 {
        return "-".to_string();
    }
    let shown = count.min(6);
    let mut out = (0..shown)
        .map(|idx| format!("{path}:{kind}[{idx}]"))
        .collect::<Vec<_>>()
        .join("\n");
    if count > shown {
        out.push_str(&format!("\n+{} more", count - shown));
    }
    out
}

fn panim_summary(project_root: &str, anim_path: &str) -> String {
    if anim_path.is_empty() || anim_path == "-" {
        return "no .panim".to_string();
    }
    let abs = res_to_abs(project_root, anim_path);
    let Ok(text) = FileMod::load_string(&abs) else {
        return "clip not readable".to_string();
    };
    let mut in_objects = false;
    let mut objects = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "[Objects]" {
            in_objects = true;
            continue;
        }
        if trimmed == "[/Objects]" {
            break;
        }
        if in_objects && trimmed.contains('=') {
            objects.push(trimmed.to_string());
        }
        if objects.len() >= 6 {
            break;
        }
    }
    let objects = objects.join("\n");
    let frame_count = text
        .lines()
        .filter(|line| line.trim().starts_with("[Frame"))
        .count();
    format!(
        "frames={frame_count}\nobjects:\n{}",
        if objects.is_empty() { "-" } else { &objects }
    )
}

#[derive(Default)]
struct SceneTreeRows {
    labels: Vec<String>,
    keys: Vec<u32>,
    selected_row: Option<usize>,
}

fn scene_tree_view(
    doc: &SceneDoc,
    selected_key: Option<u32>,
    filter: &str,
    collapsed_keys: &[u32],
) -> SceneTreeRows {
    let filter = NodePickerFilter::parse(filter);
    if !filter.is_empty() {
        return filtered_scene_tree_view(doc, selected_key, &filter);
    }
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
        push_scene_tree_row(doc, key, selected_key, collapsed_keys, &mut visited, &mut out);
    }
    for node in doc.scene.nodes.iter() {
        let key = node.key.as_u32();
        if !visited.contains(&key) {
            push_scene_tree_row(doc, key, selected_key, collapsed_keys, &mut visited, &mut out);
        }
    }
    out
}

fn scene_node_path(doc: &SceneDoc, key: SceneKey) -> String {
    let mut parts = Vec::new();
    let mut cursor = Some(key);
    let mut guard = 0;
    while let Some(key) = cursor {
        parts.push(doc.scene.key_name_or_id(key).to_string());
        cursor = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key == key)
            .and_then(|node| node.parent);
        guard += 1;
        if guard > doc.scene.nodes.len() {
            break;
        }
    }
    parts.reverse();
    parts.join("/")
}

fn filtered_scene_tree_view(
    doc: &SceneDoc,
    selected_key: Option<u32>,
    filter: &NodePickerFilter,
) -> SceneTreeRows {
    let mut out = SceneTreeRows::default();
    for node in doc.scene.nodes.iter() {
        if out.labels.len() >= MAX_NODES {
            break;
        }
        let name = doc.scene.key_name_or_id(node.key).to_string();
        let type_name = node.data.type_name();
        if !scene_node_matches_filter(doc, node, &name, type_name, filter) {
            continue;
        }
        let row = out.labels.len();
        let key = node.key.as_u32();
        let prefix = if Some(key) == selected_key {
            out.selected_row = Some(row);
            ">"
        } else {
            " "
        };
        let parent = node
            .parent
            .map(|key| doc.scene.key_name_or_id(key).to_string())
            .unwrap_or_else(|| "-".to_string());
        out.labels.push(scene_row_label(
            prefix,
            &name,
            type_name,
            &scene_node_badges(node),
            scene_child_count(doc, key),
            false,
            Some(&parent),
        ));
        out.keys.push(key);
    }
    out
}

fn scene_node_matches_filter(
    doc: &SceneDoc,
    node: &SceneNodeEntry,
    name: &str,
    type_name: &str,
    filter: &NodePickerFilter,
) -> bool {
    let path = scene_node_path(doc, node.key);
    let badges = scene_node_badges(node);
    let hay = format!(
        "{} {} {} {} {}",
        name.to_ascii_lowercase(),
        type_name.to_ascii_lowercase(),
        path.to_ascii_lowercase(),
        node_type_search_text(node.data.node_type),
        badges.to_ascii_lowercase()
    );
    filter.text.iter().all(|needle| hay.contains(needle))
        && filter
            .tags
            .iter()
            .all(|tag| node_type_has_picker_tag(node.data.node_type, tag))
}

fn scene_child_count(doc: &SceneDoc, key: u32) -> usize {
    doc.scene
        .nodes
        .iter()
        .filter(|node| node.parent.map(|parent| parent.as_u32()) == Some(key))
        .count()
}

fn push_scene_tree_row(
    doc: &SceneDoc,
    key: u32,
    selected_key: Option<u32>,
    collapsed_keys: &[u32],
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
    let children = scene_child_count(doc, key);
    out.labels.push(scene_row_label(
        prefix,
        &doc.scene.key_name_or_id(node.key).to_string(),
        node.data.type_name(),
        &scene_node_badges(node),
        children,
        collapsed_keys.contains(&key),
        None,
    ));
    out.keys.push(key);
    if children > 0 && collapsed_keys.contains(&key) {
        return Some(row);
    }
    for child in doc
        .scene
        .nodes
        .iter()
        .filter(|child| child.parent.map(|parent| parent.as_u32()) == Some(key))
    {
        let _ = push_scene_tree_row(
            doc,
            child.key.as_u32(),
            selected_key,
            collapsed_keys,
            visited,
            out,
        );
    }
    Some(row)
}

fn scene_row_label(
    prefix: &str,
    name: &str,
    type_name: &str,
    badges: &str,
    children: usize,
    collapsed: bool,
    parent: Option<&str>,
) -> String {
    let name = editor_view::short_path(name, 18);
    let type_name = editor_view::short_path(type_name, 20);
    let fold = if children == 0 {
        " "
    } else if collapsed {
        "+"
    } else {
        "-"
    };
    if let Some(parent) = parent {
        format!(
            "{prefix}{fold} {name} : {type_name}{badges}  ch={children}  < {}",
            editor_view::short_path(parent, 14)
        )
    } else {
        format!("{prefix}{fold} {name} : {type_name}{badges}  ch={children}")
    }
}

fn scene_node_badges(node: &SceneNodeEntry) -> String {
    let mut out = Vec::new();
    if scene_field_bool(&node.data, "visible") == Some(false) {
        out.push("hid");
    }
    if node.root_of.is_some() {
        out.push("inst");
    }
    if node.script.is_some() {
        out.push("scr");
    }
    if selected_node_asset_refs(node)
        .iter()
        .any(|item| !item.starts_with("script:") && !item.starts_with("root_of:"))
    {
        out.push("res");
    }
    if out.is_empty() {
        String::new()
    } else {
        format!(" [{}]", out.join(" "))
    }
}

fn picker_rows(state: &EditorState, filter: &str, offset: usize) -> Vec<String> {
    picker_node_types(state, filter)
        .into_iter()
        .skip(offset)
        .take(MAX_NODE_PICKER_ROWS)
        .map(|node_type| picker_node_row(state, node_type))
        .collect()
}

fn filtered_file_paths(state: &EditorState) -> Vec<String> {
    let filter = NodePickerFilter::parse(&state.file_filter);
    state
        .file_paths
        .iter()
        .filter(|path| {
            file_path_in_scope(path, &state.file_scope)
                && (filter.is_empty() || file_path_matches_filter(path, &filter))
        })
        .cloned()
        .collect()
}

fn file_row_label(path: &str, scope: &str) -> String {
    if scope.is_empty() {
        return editor_files::rel_label(path);
    }
    path.strip_prefix(scope)
        .map(|rest| rest.trim_end_matches('/').to_string())
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| editor_files::rel_label(path))
}

fn file_path_matches_filter(path: &str, filter: &NodePickerFilter) -> bool {
    let hay = format!(
        "{} {} {} {}",
        path.to_ascii_lowercase(),
        editor_files::rel_label(path).to_ascii_lowercase(),
        editor_files::kind_label(path).to_ascii_lowercase(),
        editor_files::display_kind_label(path).to_ascii_lowercase()
    );
    filter.text.iter().all(|needle| hay.contains(needle))
        && filter.tags.iter().all(|tag| file_path_has_tag(path, tag))
}

fn file_path_has_tag(path: &str, tag: &str) -> bool {
    let kind = editor_files::kind_label(path);
    let badge = editor_files::display_kind_label(path).to_ascii_lowercase();
    match tag {
        "dir" | "folder" => path.ends_with('/'),
        "scene" | "scn" => kind == "scene",
        "script" | "rs" => kind == "script",
        "img" | "image" => kind == "image",
        "audio" | "aud" => kind == "audio",
        "mesh" => kind == "mesh",
        "glb" | "gltf" => badge == "glb",
        "anim" | "panim" => path.ends_with(".panim"),
        "mat" | "pmat" => path.ends_with(".pmat"),
        "res" | "resource" => kind == "resource",
        _ => badge.contains(tag) || kind.contains(tag) || path.to_ascii_lowercase().contains(tag),
    }
}

fn file_panel_title(state: &EditorState) -> String {
    if state.file_scope.is_empty() {
        "File System (res://)".to_string()
    } else {
        format!(
            "File System ({})  Backspace up",
            editor_view::short_path(&state.file_scope, 28)
        )
    }
}

fn file_path_in_scope(path: &str, scope: &str) -> bool {
    if scope.is_empty() {
        return true;
    }
    let Some(rest) = path.strip_prefix(scope) else {
        return false;
    };
    !rest.is_empty() && !rest.trim_end_matches('/').contains('/')
}

fn parent_res_folder(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    let Some((head, _tail)) = trimmed.rsplit_once('/') else {
        return String::new();
    };
    if head == "res:" || head == "res://" {
        String::new()
    } else {
        format!("{head}/")
    }
}

fn picker_parent_text(state: &EditorState) -> String {
    if state.doc_text.is_empty() {
        return "target: -".to_string();
    }
    let doc = SceneDoc::parse(&state.doc_text);
    let Some(key) = state.selected_key.or_else(|| doc.scene.root.map(|key| key.as_u32())) else {
        return "target: scene root".to_string();
    };
    let Some(node) = doc.scene.nodes.iter().find(|node| node.key.as_u32() == key) else {
        return "target: scene root".to_string();
    };
    if state.add_node_as_sibling {
        let parent = node
            .parent
            .map(|key| doc.scene.key_name_or_id(key).to_string())
            .unwrap_or_else(|| "scene root".to_string());
        let kind = picker_parent_node_kind(state).unwrap_or("Root");
        return format!(
            "as sibling of {}  parent: {parent}  kind={kind}  Tab toggles",
            doc.scene.key_name_or_id(node.key)
        );
    }
    format!(
        "as child of {} ({})  kind={}  Tab toggles",
        doc.scene.key_name_or_id(node.key),
        node.data.type_name(),
        node_type_kind(node.data.node_type)
    )
}

fn node_type_icon(node_type: perro_scene::NodeType) -> &'static str {
    match node_type.name() {
        "Sprite2D" => "[SPR]",
        "Camera2D" | "Camera3D" => "[CAM]",
        "MeshInstance3D" | "MultiMeshInstance3D" => "[MSH]",
        "PointLight2D" | "SpotLight2D" | "RayLight2D" | "AmbientLight2D" | "PointLight3D"
        | "SpotLight3D" | "RayLight3D" | "AmbientLight3D" => "[LGT]",
        "AudioPlayer2D"
        | "AudioStreamPlayer2D"
        | "AudioArea2D"
        | "AudioPlayer3D"
        | "AudioStreamPlayer3D"
        | "AudioArea3D" => "[AUD]",
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
    find_scene_value_text(data, "position")
}

fn find_vec2_value(data: &SceneNodeData, field: &str) -> Option<Vector2> {
    for (name, value) in data.fields.iter() {
        if name.as_ref() == field {
            return match value {
                SceneValue::Vec2 { x, y } => Some(Vector2::new(*x, *y)),
                SceneValue::Vec3 { x, y, .. } => Some(Vector2::new(*x, *y)),
                _ => None,
            };
        }
    }
    data.base_ref()
        .and_then(|base| find_vec2_value(base, field))
}

fn find_vec3_value(data: &SceneNodeData, field: &str) -> Option<Vector3> {
    for (name, value) in data.fields.iter() {
        if name.as_ref() == field {
            return match value {
                SceneValue::Vec3 { x, y, z } => Some(Vector3::new(*x, *y, *z)),
                SceneValue::Vec2 { x, y } => Some(Vector3::new(*x, *y, 0.0)),
                _ => None,
            };
        }
    }
    data.base_ref()
        .and_then(|base| find_vec3_value(base, field))
}

fn find_scene_value_text(data: &SceneNodeData, field: &str) -> Option<String> {
    for (name, value) in data.fields.iter() {
        if name.as_ref() == field {
            return match value {
                SceneValue::F32(value) => Some(format!("{value:.2}")),
                SceneValue::Vec2 { x, y } => Some(format!("({x:.2}, {y:.2})")),
                SceneValue::Vec3 { x, y, z } => Some(format!("({x:.2}, {y:.2}, {z:.2})")),
                _ => None,
            };
        }
    }
    data.base_ref()
        .and_then(|base| find_scene_value_text(base, field))
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

fn sanitize_node_name(text: &str) -> String {
    let mut out = String::new();
    for ch in text.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else if ch.is_whitespace() || ch == '-' || ch == '.' {
            out.push('_');
        }
    }
    if out.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
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

fn is_gltf_path(path: &str) -> bool {
    path.ends_with(".glb") || path.ends_with(".gltf")
}

fn rewrite_project_res_paths(doc: &SceneDoc, project_root: &str) -> SceneDoc {
    let mut doc = doc.clone();
    for node in doc.scene.nodes.to_mut().iter_mut() {
        node.script = None;
        node.clear_script = true;
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
            perro_scene::SceneNodeDataBase::Owned(base) => {
                rewrite_project_res_data(base, project_root)
            }
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

fn set_text_box<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, name: &str, text: &str) {
    if let Some(id) = find_named(ctx, name) {
        let text = text.to_string();
        let _ = with_node_mut!(ctx.run, UiTextBox, id, |node| {
            if node.text.as_ref() != text {
                node.set_text(text);
            }
        });
    }
}

fn read_text_box<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
) -> Option<String> {
    let id = find_named(ctx, name)?;
    Some(with_node!(ctx.run, UiTextBox, id, |node| node
        .text
        .to_string()))
}

fn set_button_fill<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    fill: &str,
) {
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
    set_grid_visible(ctx, "viewport_grid", false);
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
            node.transform.translation =
                Vector2::new((rect.center.x - 0.5) * 0.94, (rect.center.y - 0.5) * 0.82);
            node.transform.rotation = rect.rotation;
        });
    }
}

fn set_resize_handles_visible<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    visible: bool,
) {
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

fn set_panel_visible<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    visible: bool,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiPanel, id, |node| {
            node.visible = visible;
            node.input_enabled = visible;
        });
    }
}

fn set_panel_display<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    visible: bool,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiPanel, id, |node| {
            node.visible = visible;
            node.input_enabled = false;
        });
    }
}

fn set_ui_display<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    visible: bool,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_base_node_mut!(ctx.run, UiBox, id, |node| {
            node.visible = visible;
            node.input_enabled = visible;
        });
    }
}

fn set_ui_box_size<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    size: (f32, f32),
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_base_node_mut!(ctx.run, UiBox, id, |node| {
            node.layout.size = UiVector2::ratio(size.0, size.1);
        });
    }
}

fn set_grid_visible<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    visible: bool,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiGrid, id, |node| {
            node.visible = visible;
            node.input_enabled = visible;
        });
    }
}

fn set_camera_stream_visible<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    visible: bool,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiCameraStream, id, |node| {
            node.visible = visible;
            node.input_enabled = visible;
        });
    }
}

fn set_viewport_stream_camera<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    camera: NodeID,
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiCameraStream, id, |node| {
            node.stream.camera = camera;
        });
    }
}

fn set_panel_size<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    size: (f32, f32),
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiPanel, id, |node| {
            node.layout.size = UiVector2::ratio(size.0, size.1);
        });
    }
}

fn apply_viewport_canvas<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let (mode, pan_x, pan_y, zoom) = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.viewport_mode == "2D" {
            let zoom = state.cam2_zoom.max(0.05);
            (
                state.viewport_mode.clone(),
                -state.cam2_x * zoom / 960.0,
                state.cam2_y * zoom / 540.0,
                zoom,
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
        set_canvas_line(
            ctx,
            &format!("canvas_v_{i}"),
            true,
            wrap_grid_offset(offset + pan_x, spacing),
            false,
        );
        set_canvas_line(
            ctx,
            &format!("canvas_h_{i}"),
            false,
            wrap_grid_offset(offset + pan_y, spacing),
            false,
        );
    }
    set_canvas_line(ctx, "canvas_origin_x", false, pan_y, true);
    set_canvas_line(ctx, "canvas_origin_y", true, pan_x, true);
}

fn wrap_grid_offset(offset: f32, spacing: f32) -> f32 {
    if spacing <= 0.0 {
        return offset;
    }
    let half = spacing * 4.0;
    let width = spacing * 9.0;
    (offset + half).rem_euclid(width) - half
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
        list.v_spacing = 0.004;
    });
    for idx in 0..MAX_NODES {
        set_button_size(ctx, &format!("scene_row_{idx}"), (1.0, 0.062));
    }
}

fn apply_file_tree_layout<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(list_id) = find_named(ctx, "file_rows") else {
        return;
    };
    let _ = with_node_mut!(ctx.run, UiList, list_id, |list| {
        list.indent = 8.0;
        list.v_spacing = 0.004;
    });
    for idx in 0..MAX_FILES {
        set_button_size(ctx, &format!("file_row_{idx}"), (1.0, 0.062));
    }
}

fn set_button_size<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
    size: (f32, f32),
) {
    if let Some(id) = find_named(ctx, name) {
        let _ = with_node_mut!(ctx.run, UiButton, id, |node| {
            node.layout.size = UiVector2::ratio(size.0, size.1);
        });
    }
}

fn set_add_node_popup<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, visible: bool) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.add_node_popup_open = visible;
    });
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

fn find_named<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
) -> Option<NodeID> {
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
        if !out.iter().any(|item| item == &path) && validate_project_root(Path::new(&path)).is_ok()
        {
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
