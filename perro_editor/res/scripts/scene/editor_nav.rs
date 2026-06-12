use crate::scripts_app_editor_app_rs as editor_app;
use crate::scripts_app_editor_manager_rs as editor_manager;
use crate::scripts_app_editor_project_rs as editor_project;
use crate::scripts_assets_editor_assets_rs::*;
use crate::scripts_assets_editor_file_watch_rs as editor_file_watch;
use crate::scripts_assets_editor_files_rs as editor_files;
use crate::scripts_editor_main_rs::{
    EditorState, FILE_WATCH_INTERVAL_FRAMES, MAX_FILES, MAX_NODE_PICKER_ROWS, MAX_NODES,
    MAX_RECENT, MAX_TABS, RECENT_PROJECTS_PATH,
};
use crate::scripts_scene_editor_animation_rs::*;
use crate::scripts_scene_editor_gizmos_rs as editor_gizmos;
use crate::scripts_scene_editor_nodes_rs::*;
use crate::scripts_scene_editor_scene_deps_rs as editor_scene_deps;
use crate::scripts_scene_editor_scene_rs as editor_scene;
use crate::scripts_scene_editor_viewport_rs::*;
use crate::scripts_ui_editor_inspector_values_rs::*;
use crate::scripts_ui_editor_ui_rs::*;
use crate::scripts_ui_editor_view_rs as editor_view;
use perro_api::prelude::*;
use perro_api::scene::{
    SceneDoc, SceneFieldName, SceneKey, SceneNodeData, SceneNodeEntry, SceneValue, SceneValueKey,
};
use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
pub fn update_freecam<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
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

pub fn update_editor_shortcuts<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let ctrl =
        key_down!(ctx.ipt, KeyCode::ControlLeft) || key_down!(ctx.ipt, KeyCode::ControlRight);
    let alt = key_down!(ctx.ipt, KeyCode::AltLeft) || key_down!(ctx.ipt, KeyCode::AltRight);
    let shift = key_down!(ctx.ipt, KeyCode::ShiftLeft) || key_down!(ctx.ipt, KeyCode::ShiftRight);
    if key_pressed!(ctx.ipt, KeyCode::Escape) {
        handle_editor_escape(ctx);
        return;
    }
    if key_pressed!(ctx.ipt, KeyCode::Enter) && commit_focused_inspector_box(ctx) {
        return;
    }
    if editor_text_box_has_focus(ctx) {
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
        && (key_pressed!(ctx.ipt, KeyCode::Equal) || key_pressed!(ctx.ipt, KeyCode::NumpadAdd))
    {
        zoom_active_viewport(ctx, 1);
        return;
    }
    if !picker_open
        && !ctrl
        && !alt
        && (key_pressed!(ctx.ipt, KeyCode::Minus) || key_pressed!(ctx.ipt, KeyCode::NumpadSubtract))
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
        set_anim_drawer(ctx, true);
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

pub fn editor_text_box_has_focus<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
) -> bool {
    with_state!(ctx.run, EditorState, ctx.id, |state| {
        !state.focused_inspector_box.is_empty()
    })
}

pub fn commit_focused_inspector_box<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
) -> bool {
    let name = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (!state.focused_inspector_box.is_empty()).then(|| state.focused_inspector_box.clone())
    });
    let Some(name) = name else {
        return false;
    };
    commit_inspector_box(ctx, &name);
    true
}

pub fn commit_inspector_box<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    name: &str,
) -> bool {
    match name {
        "inspector_name_box" => rename_selected_node(ctx),
        "inspector_position_box" => {
            edit_selected_transform(ctx, "position", "inspector_position_box")
        }
        "inspector_rotation_box" => {
            edit_selected_rotation(ctx)
        }
        "inspector_scale_box" => edit_selected_transform(ctx, "scale", "inspector_scale_box"),
        "inspector_vars_box" => edit_selected_script_vars(ctx),
        _ => {
            if let Some(idx) = middle_index(name, "inspector_var_", "_value") {
                edit_selected_script_var_path(ctx, idx);
            } else if name.starts_with("inspector_position_") && name.ends_with("_box") {
                edit_selected_transform(ctx, "position", "inspector_position_box");
            } else if name.starts_with("inspector_rotation_") && name.ends_with("_box") {
                edit_selected_rotation(ctx);
            } else if name == "inspector_rotation_quat_button" {
                set_inspector_rotation_mode(ctx, "quat");
            } else if name == "inspector_rotation_euler_button" {
                set_inspector_rotation_mode(ctx, "euler");
            } else if name.starts_with("inspector_scale_") && name.ends_with("_box") {
                edit_selected_transform(ctx, "scale", "inspector_scale_box");
            } else {
                return false;
            }
        }
    }
    true
}

pub fn handle_editor_escape<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let action = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.inspector_picker_open {
            "inspector_picker"
        } else if state.add_node_popup_open {
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
        "inspector_picker" => set_inspector_picker(ctx, false),
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

pub fn cycle_sidebar_panel<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.sidebar_mode.clone()
    });
    if mode == "files" {
        set_activity_mode(ctx, "scene");
    } else {
        set_sidebar_mode(ctx, "files");
    }
}

pub fn prepare_sidebar_find<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.sidebar_mode == "files" {
            state.activity_mode = "scene".to_string();
            state.log = "find assets\nuse file search".to_string();
        } else {
            state.sidebar_mode = "scene".to_string();
            state.activity_mode = "scene".to_string();
            state.log = "find nodes\nuse scene search".to_string();
        }
    });
    refresh_all(ctx);
}

pub fn open_add_node_popup<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.node_picker_offset = 0;
        state.node_picker_filter.clear();
        state.add_node_as_sibling = false;
    });
    refresh_all(ctx);
    set_add_node_popup(ctx, true);
}

pub fn add_camera_for_active_view<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let node_type = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.viewport_mode == "3D" {
            "Camera3D"
        } else {
            "Camera2D"
        }
    });
    add_node(ctx, node_type);
}

pub fn open_add_node_sibling_popup<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.node_picker_offset = 0;
        state.node_picker_filter.clear();
        state.add_node_as_sibling = true;
    });
    refresh_all(ctx);
    set_add_node_popup(ctx, true);
}

pub fn toggle_add_node_insert_mode<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
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

pub fn select_sidebar_delta<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    delta: isize,
) {
    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.sidebar_mode.clone()
    });
    if mode == "files" {
        select_file_delta(ctx, delta);
    } else {
        select_scene_delta(ctx, delta);
    }
}

pub fn select_sidebar_edge<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, last: bool) {
    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.sidebar_mode.clone()
    });
    if mode == "files" {
        select_file_edge(ctx, last);
    } else {
        select_scene_edge(ctx, last);
    }
}

pub fn nav_sidebar_parent<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.sidebar_mode.clone()
    });
    if mode == "files" {
        nav_file_scope_parent(ctx);
    } else {
        select_related_node(ctx, "parent");
    }
}

pub fn select_scene_delta<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, delta: isize) {
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
        let current = tree.selected_row.unwrap_or_else(|| {
            tree.keys
                .iter()
                .position(|key| Some(*key) == state.selected_key)
                .unwrap_or(0)
        });
        let next = offset_index(current, tree.keys.len(), delta);
        let key = tree.keys[next];
        state.selected_key = Some(key);
        state.sidebar_mode = "scene".to_string();
        if let Some(mode) = selected_node_viewport_mode(&state.doc_text, key) {
            state.viewport_mode = mode.to_string();
        }
        if selected_node_type_name(&state.doc_text, key).as_deref() == Some("AnimationPlayer") {
            state.activity_mode = "scene".to_string();
            state.anim_drawer_open = true;
            state.active_anim_player_key = Some(key);
            state.active_glb_path.clear();
            state.active_glb_summary.clear();
            if let Some(path) = selected_node_field_text(&state.doc_text, key, "animation") {
                state.active_anim_path = path;
            }
        }
        state.log = format!(
            "select node\n{}",
            doc.scene.key_name_or_id(SceneKey::new(key))
        );
        true
    })
    .unwrap_or(false);
    if changed {
        refresh_all(ctx);
    }
}

pub fn select_scene_edge<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, last: bool) {
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
        state.log = format!(
            "select node\n{}",
            doc.scene.key_name_or_id(SceneKey::new(key))
        );
        true
    })
    .unwrap_or(false);
    if changed {
        refresh_all(ctx);
    }
}

pub fn select_related_node<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    relation: &str,
) {
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
                    pos.checked_sub(1)
                        .and_then(|idx| siblings.get(idx).copied())
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

pub fn collapse_selected_scene_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
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

pub fn expand_selected_scene_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.sidebar_mode != "scene" || !state.scene_filter.is_empty() {
            return false;
        }
        let Some(key) = state.selected_key else {
            return false;
        };
        if let Some(pos) = state
            .collapsed_scene_keys
            .iter()
            .position(|item| *item == key)
        {
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

pub fn select_file_delta<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, delta: isize) {
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
        state.activity_mode = "scene".to_string();
        state.log = format!(
            "select asset\n{}",
            editor_files::rel_label(&state.active_asset_path)
        );
        true
    })
    .unwrap_or(false);
    if changed {
        refresh_all(ctx);
    }
}

pub fn select_file_edge<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, last: bool) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let paths = filtered_file_paths(state);
        if paths.is_empty() {
            return false;
        }
        let idx = if last { paths.len() - 1 } else { 0 };
        state.active_asset_path = paths[idx].clone();
        state.sidebar_mode = "files".to_string();
        state.activity_mode = "scene".to_string();
        state.log = format!(
            "select asset\n{}",
            editor_files::rel_label(&state.active_asset_path)
        );
        true
    })
    .unwrap_or(false);
    if changed {
        refresh_all(ctx);
    }
}

pub fn nav_file_scope_parent<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.sidebar_mode != "files" || state.file_scope.is_empty() {
            return false;
        }
        let parent = parent_res_folder(&state.file_scope);
        state.file_scope = parent.clone();
        state.active_asset_path = if parent.is_empty() {
            filtered_file_paths(state)
                .first()
                .cloned()
                .unwrap_or_default()
        } else {
            parent.clone()
        };
        state.activity_mode = "scene".to_string();
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

pub fn open_active_file<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
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

pub fn open_sidebar_selection<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
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
        let node = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == key)?;
        selected_node_asset_ref_path(node)
    })
    .is_some();
    if has_ref {
        open_selected_node_asset_ref(ctx);
    } else {
        frame_selected_node(ctx);
    }
}

pub fn reveal_active_scene_in_files<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(path) = state.open_paths.get(state.active_open).cloned() else {
            state.log = "reveal file fail\nno active scene".to_string();
            return false;
        };
        state.sidebar_mode = "files".to_string();
        state.activity_mode = "scene".to_string();
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

pub fn offset_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    if delta < 0 {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        (current + delta as usize).min(len - 1)
    }
}

pub fn wrap_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    if delta < 0 {
        (current + len - (delta.unsigned_abs() % len)) % len
    } else {
        (current + delta as usize) % len
    }
}

pub fn update_freecam_2d<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
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

pub fn update_ui_canvas<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.viewport_mode.clone()
    });
    if mode != "UI" {
        return;
    }
    let inside = viewport_pointer(ctx).is_some();
    let wheel = if inside { mouse_wheel!(ctx.ipt).y } else { 0.0 };
    let mut label = String::new();
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.ui_canvas_zoom <= 0.001 {
            state.ui_canvas_zoom = 1.0;
        }
        state.ui_canvas_x = 0.0;
        state.ui_canvas_y = 0.0;
        if wheel.abs() > 0.001 {
            state.ui_canvas_zoom = (state.ui_canvas_zoom * (1.0 + wheel * 0.12)).clamp(0.25, 12.0);
        }
        label = format!(
            "Viewport  mode={}  screen canvas zoom={:.2}\nkeys: wheel/+/- zoom  0 reset  F frame  click pick  Shift+drag snap",
            state.viewport_mode, state.ui_canvas_zoom
        );
    });
    apply_viewport_canvas(ctx);
    set_label(ctx, "viewport_label", &label);
}

pub fn reset_freecam(state: &mut EditorState) {
    state.cam_x = 0.0;
    state.cam_y = 3.0;
    state.cam_z = 8.0;
    state.cam_yaw = 0.0;
    state.cam_pitch = -0.25;
}

pub fn reset_freecam_2d(state: &mut EditorState) {
    state.cam2_x = 0.0;
    state.cam2_y = 0.0;
    state.cam2_zoom = 1.0;
}

pub fn apply_freecam<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
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
        node.active = false;
    });
    let _ = ctx
        .run
        .Nodes()
        .set_local_transform_3d(camera, Transform3D::new(pos, rot, Vector3::ONE));
}

pub fn apply_freecam_2d<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
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
        node.active = false;
        node.zoom = zoom;
    });
    let _ = ctx
        .run
        .Nodes()
        .set_local_transform_2d(camera, Transform2D::new(pos, 0.0, Vector2::ONE));
}

pub fn frame_selected_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
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
