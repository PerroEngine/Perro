use perro_api::prelude::*;
use perro_api::scene::{
    SceneDoc, SceneFieldName, SceneKey, SceneNodeData, SceneNodeEntry, SceneValue, SceneValueKey,
};
use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::scripts_assets_editor_assets_rs::*;
use crate::scripts_assets_editor_file_watch_rs as editor_file_watch;
use crate::scripts_scene_editor_animation_rs::*;
use crate::scripts_scene_editor_nav_rs::*;
use crate::scripts_scene_editor_nodes_rs::*;
use crate::scripts_scene_editor_viewport_rs::*;
use crate::scripts_ui_editor_inspector_values_rs::*;
use crate::scripts_ui_editor_ui_rs::*;

type SelfNodeType = UiPanel;

pub const MAX_FILES: usize = 12;
pub const MAX_NODES: usize = 12;
pub const MAX_TABS: usize = 4;
pub const MAX_RECENT: usize = 5;
pub const MAX_NODE_PICKER_ROWS: usize = 12;
pub const RECENT_PROJECTS_PATH: &str = "user://recent_projects.json";
pub const FILE_WATCH_INTERVAL_FRAMES: u32 = 30;
pub const LIST_DOUBLE_CLICK_FRAMES: u32 = 18;

#[State]
pub struct EditorState {
    pub editor_shell_root: u64,
    pub project_root: String,
    pub project_name: String,
    pub create_parent_dir: String,
    pub recent_projects: Vec<String>,
    pub file_paths: Vec<String>,
    pub file_filter: String,
    pub file_scope: String,
    pub scene_paths: Vec<String>,
    pub open_paths: Vec<String>,
    pub active_asset_path: String,
    pub active_open: usize,
    pub doc_text: String,
    pub preview_scene_paths: Vec<String>,
    pub preview_root: u64,
    pub preview_camera_2d: u64,
    pub preview_camera_3d: u64,
    pub preview_node_ids: Vec<u64>,
    pub preview_node_keys: Vec<u32>,
    pub preview_pick_node_ids: Vec<u64>,
    pub preview_pick_node_keys: Vec<u32>,
    pub preview_selected_gizmo: u64,
    pub preview_selected_gizmo_key: Option<u32>,
    pub project_file_sigs: Vec<editor_file_watch::FileSig>,
    pub dirty_scene_paths: Vec<String>,
    pub file_watch_frame: u32,
    pub last_file_row_click_frame: u32,
    pub last_file_row_click_slot: Option<usize>,
    pub last_scene_row_click_frame: u32,
    pub last_scene_row_click_slot: Option<usize>,
    pub preview_serial: u64,
    pub selected_key: Option<u32>,
    pub collapsed_scene_keys: Vec<u32>,
    pub copied_node_key: Option<u32>,
    pub ui_drag_key: Option<u32>,
    pub ui_drag_mode: String,
    pub ui_drag_last_x: f32,
    pub ui_drag_last_y: f32,
    pub viewport_mode: String,
    pub dirty: bool,
    pub add_node_popup_open: bool,
    pub add_node_as_sibling: bool,
    pub scene_filter: String,
    pub node_picker_offset: usize,
    pub node_picker_filter: String,
    pub recent_node_types: Vec<String>,
    pub cam_x: f32,
    pub cam_y: f32,
    pub cam_z: f32,
    pub cam_yaw: f32,
    pub cam_pitch: f32,
    pub cam2_x: f32,
    pub cam2_y: f32,
    pub cam2_zoom: f32,
    pub ui_canvas_x: f32,
    pub ui_canvas_y: f32,
    pub ui_canvas_zoom: f32,
    pub activity_mode: String,
    pub sidebar_mode: String,
    pub anim_drawer_open: bool,
    pub active_anim_path: String,
    pub active_anim_player_key: Option<u32>,
    pub active_glb_path: String,
    pub active_glb_summary: String,
    pub active_glb_mesh_index: usize,
    pub active_glb_mat_index: usize,
    pub active_glb_anim_index: usize,
    pub focused_inspector_box: String,
    pub inspector_rotation_mode: String,
    pub inspector_layout_applied: bool,
    pub log: String,
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
            state.inspector_rotation_mode = "quat".to_string();
            state.last_file_row_click_frame = 0;
            state.last_file_row_click_slot = None;
            state.last_scene_row_click_frame = 0;
            state.last_scene_row_click_slot = None;
        });
        refresh_all(ctx);
        set_project_manager(ctx, true);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        update_freecam(ctx);
        update_ui_canvas(ctx);
        draw_preview_2d_gizmos(ctx);
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
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            if state.focused_inspector_box == name {
                state.focused_inspector_box.clear();
            }
        });

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
            "activity_glb_button" => set_activity_mode(ctx, "glb"),
            "bottom_log_button" => set_anim_drawer(ctx, false),
            "bottom_anim_button" => set_anim_drawer(ctx, true),
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
                edit_selected_rotation(ctx)
            }
            "inspector_scale_box" => edit_selected_transform(ctx, "scale", "inspector_scale_box"),
            "inspector_vars_box" => edit_selected_script_vars(ctx),
            "add_node_search_box" => update_node_picker_filter(ctx),
            _ => {
                if let Some(idx) = suffix_index(&name, "file_row_") {
                    click_or_open_file_slot(ctx, idx);
                } else if let Some(idx) = suffix_index(&name, "manager_recent_") {
                    open_recent_project(ctx, idx);
                } else if let Some(idx) = suffix_index(&name, "add_node_type_") {
                    add_node_from_picker(ctx, idx);
                } else if let Some(idx) = middle_index(&name, "inspector_var_", "_value") {
                    edit_selected_script_var_path(ctx, idx);
                } else if let Some(idx) = middle_index(&name, "inspector_resource_", "_button") {
                    pick_selected_resource_field(ctx, idx);
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
                } else if let Some(idx) = suffix_index(&name, "scene_row_") {
                    click_scene_node_slot(ctx, idx);
                } else if let Some(idx) = suffix_index(&name, "scene_tab_") {
                    set_active_tab(ctx, idx);
                } else if let Some(idx) = suffix_index(&name, "scene_tab_close_") {
                    close_scene_tab(ctx, idx);
                }
            }
        }
    }

    fn on_editor_inspector_focus(&self, ctx: &mut ScriptContext<'_, API>, sender: NodeID) {
        let Some(name) = get_node_name!(ctx.run, sender).map(|v| v.to_string()) else {
            return;
        };
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            state.focused_inspector_box = name;
        });
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
            signal!("editor_activity_glb"),
            signal!("editor_bottom_log"),
            signal!("editor_bottom_anim"),
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
            signal!("editor_inspector_rotation_quat"),
            signal!("editor_inspector_rotation_euler"),
            signal!("editor_inspector_scale"),
            signal!("editor_inspector_vars"),
            signal!("editor_inspector_position_0"),
            signal!("editor_inspector_position_1"),
            signal!("editor_inspector_position_2"),
            signal!("editor_inspector_position_3"),
            signal!("editor_inspector_rotation_0"),
            signal!("editor_inspector_rotation_1"),
            signal!("editor_inspector_rotation_2"),
            signal!("editor_inspector_rotation_3"),
            signal!("editor_inspector_scale_0"),
            signal!("editor_inspector_scale_1"),
            signal!("editor_inspector_scale_2"),
            signal!("editor_inspector_scale_3"),
            signal!("editor_inspector_var_0"),
            signal!("editor_inspector_var_1"),
            signal!("editor_inspector_var_2"),
            signal!("editor_inspector_var_3"),
            signal!("editor_inspector_var_4"),
            signal!("editor_inspector_var_5"),
            signal!("editor_inspector_var_6"),
            signal!("editor_inspector_var_7"),
            signal!("editor_inspector_resource_0"),
            signal!("editor_inspector_resource_1"),
            signal!("editor_inspector_resource_2"),
            signal!("editor_inspector_resource_3"),
            signal!("editor_inspector_commit"),
        ],
        [func!("on_editor_signal")]
    );
    let _ = signal_connect_many!(
        ctx.run,
        ctx.id,
        [signal!("editor_inspector_focus")],
        [func!("on_editor_inspector_focus")]
    );
}
