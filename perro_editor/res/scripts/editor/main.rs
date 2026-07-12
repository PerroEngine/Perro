use perro_api::prelude::*;
use perro_api::scene::{
    SceneDoc, SceneFieldName, SceneKey, SceneNodeData, SceneNodeEntry, SceneValue, SceneValueKey,
};
use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};

use crate::scripts_assets_editor_assets_rs::*;
use crate::scripts_assets_editor_file_watch_rs as editor_file_watch;
use crate::scripts_scene_editor_animation_rs::*;
use crate::scripts_scene_editor_nav_rs::*;
use crate::scripts_scene_editor_nodes_rs::*;
use crate::scripts_scene_editor_viewport_rs::*;
use crate::scripts_ui_editor_inspector_values_rs::*;
use crate::scripts_ui_editor_ui_rs::*;

type SelfNodeType = UiPanel;

#[derive(Clone)]
struct CachedSceneDoc {
    text: String,
    doc: Arc<SceneDoc>,
    node_indices: Vec<(u32, usize)>,
}

static ACTIVE_SCENE_DOC_CACHE: OnceLock<Mutex<Vec<CachedSceneDoc>>> = OnceLock::new();
static UI_DRAG_DOC_CACHE: OnceLock<Mutex<Vec<(u64, SceneDoc)>>> = OnceLock::new();

const SCENE_DOC_CACHE_LIMIT: usize = MAX_SCENE_UNDO + 8;

/// Deep-cloned doc for callers that mutate it. Read-only paths use
/// [`cached_scene_doc_shared`] to skip the full clone.
pub fn cached_scene_doc(text: &str) -> SceneDoc {
    (*cached_scene_doc_shared(text)).clone()
}

pub fn cached_scene_doc_shared(text: &str) -> Arc<SceneDoc> {
    let cache = ACTIVE_SCENE_DOC_CACHE.get_or_init(|| Mutex::new(Vec::new()));
    let Ok(mut guard) = cache.lock() else {
        return Arc::new(SceneDoc::parse(text));
    };
    if let Some(idx) = guard.iter().position(|cached| cached.text == text) {
        let cached = guard.remove(idx);
        let doc = cached.doc.clone();
        guard.push(cached);
        return doc;
    }
    let doc = Arc::new(SceneDoc::parse(text));
    guard.push(CachedSceneDoc::new(text.to_string(), doc.clone()));
    if guard.len() > SCENE_DOC_CACHE_LIMIT {
        guard.remove(0);
    }
    doc
}

pub fn store_scene_doc_cache(text: &str, doc: &SceneDoc) {
    let cache = ACTIVE_SCENE_DOC_CACHE.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut guard) = cache.lock() {
        if let Some(idx) = guard.iter().position(|cached| cached.text == text) {
            guard.remove(idx);
        }
        guard.push(CachedSceneDoc::new(text.to_string(), Arc::new(doc.clone())));
        if guard.len() > SCENE_DOC_CACHE_LIMIT {
            guard.remove(0);
        }
    }
}

impl CachedSceneDoc {
    fn new(text: String, doc: Arc<SceneDoc>) -> Self {
        let mut node_indices = doc
            .scene
            .nodes
            .iter()
            .enumerate()
            .map(|(idx, node)| (node.key.as_u32(), idx))
            .collect::<Vec<_>>();
        node_indices.sort_by_key(|(key, _)| *key);
        Self {
            text,
            doc,
            node_indices,
        }
    }

    fn node(&self, key: u32) -> Option<SceneNodeEntry> {
        self.node_indices
            .binary_search_by_key(&key, |(item_key, _)| *item_key)
            .ok()
            .and_then(|pos| self.doc.scene.nodes.get(self.node_indices[pos].1))
            .cloned()
    }
}

pub fn cached_scene_node(text: &str, key: u32) -> Option<SceneNodeEntry> {
    let cache = ACTIVE_SCENE_DOC_CACHE.get_or_init(|| Mutex::new(Vec::new()));
    let Ok(mut guard) = cache.lock() else {
        return SceneDoc::parse(text)
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == key)
            .cloned();
    };
    if let Some(idx) = guard.iter().position(|cached| cached.text == text) {
        let cached = guard.remove(idx);
        let node = cached.node(key);
        guard.push(cached);
        return node;
    }
    let doc = Arc::new(SceneDoc::parse(text));
    let cached = CachedSceneDoc::new(text.to_string(), doc);
    let node = cached.node(key);
    guard.push(cached);
    if guard.len() > SCENE_DOC_CACHE_LIMIT {
        guard.remove(0);
    }
    node
}

pub fn set_state_scene_doc(state: &mut EditorState, doc: &SceneDoc) {
    let next = doc.to_text();
    if state.doc_text != next {
        push_scene_undo_snapshot(state);
        state.scene_redo_stack.clear();
        state.doc_text = next;
    }
    store_scene_doc_cache(&state.doc_text, doc);
}

pub fn set_state_scene_doc_loaded(state: &mut EditorState, doc: &SceneDoc) {
    state.doc_text = doc.to_text();
    state.scene_undo_stack.clear();
    state.scene_redo_stack.clear();
    store_scene_doc_cache(&state.doc_text, doc);
}

fn push_scene_undo_snapshot(state: &mut EditorState) {
    if state.doc_text.is_empty() {
        return;
    }
    if state
        .scene_undo_stack
        .last()
        .is_some_and(|item| item == &state.doc_text)
    {
        return;
    }
    state.scene_undo_stack.push(state.doc_text.clone());
    if state.scene_undo_stack.len() > MAX_SCENE_UNDO {
        state.scene_undo_stack.remove(0);
    }
}

pub fn undo_scene_doc(state: &mut EditorState) -> bool {
    let Some(prev) = state.scene_undo_stack.pop() else {
        state.log = "undo\nempty".to_string();
        return false;
    };
    if !state.doc_text.is_empty() {
        state.scene_redo_stack.push(state.doc_text.clone());
    }
    state.doc_text = prev;
    let _ = cached_scene_doc_shared(&state.doc_text);
    state.dirty = true;
    mark_active_scene_dirty(state);
    state.log = format!("undo\n{} left", state.scene_undo_stack.len());
    true
}

pub fn redo_scene_doc(state: &mut EditorState) -> bool {
    let Some(next) = state.scene_redo_stack.pop() else {
        state.log = "redo\nempty".to_string();
        return false;
    };
    push_scene_undo_snapshot(state);
    state.doc_text = next;
    let _ = cached_scene_doc_shared(&state.doc_text);
    state.dirty = true;
    mark_active_scene_dirty(state);
    state.log = format!("redo\n{} left", state.scene_redo_stack.len());
    true
}

fn mark_active_scene_dirty(state: &mut EditorState) {
    if let Some(path) = state.open_paths.get(state.active_open).cloned()
        && !state.dirty_scene_paths.iter().any(|item| item == &path)
    {
        state.dirty_scene_paths.push(path);
    }
}

pub fn clear_scene_doc_cache() {
    let cache = ACTIVE_SCENE_DOC_CACHE.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut guard) = cache.lock() {
        guard.clear();
    }
}

pub fn begin_ui_drag_doc(owner: u64, text: &str) {
    let cache = UI_DRAG_DOC_CACHE.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut guard) = cache.lock() {
        guard.retain(|(id, _)| *id != owner);
        if !text.is_empty() {
            guard.push((owner, cached_scene_doc(text)));
        }
    }
}

pub fn with_ui_drag_doc_mut<T>(owner: u64, f: impl FnOnce(&mut SceneDoc) -> T) -> Option<T> {
    let cache = UI_DRAG_DOC_CACHE.get_or_init(|| Mutex::new(Vec::new()));
    let mut guard = cache.lock().ok()?;
    let (_, doc) = guard.iter_mut().find(|(id, _)| *id == owner)?;
    Some(f(doc))
}

pub fn take_ui_drag_doc(owner: u64) -> Option<SceneDoc> {
    let cache = UI_DRAG_DOC_CACHE.get_or_init(|| Mutex::new(Vec::new()));
    let mut guard = cache.lock().ok()?;
    let idx = guard.iter().position(|(id, _)| *id == owner)?;
    Some(guard.swap_remove(idx).1)
}

pub const MAX_FILES: usize = 12;
pub const MAX_NODES: usize = 12;
pub const MAX_TABS: usize = 4;
pub const MAX_OUTPUT_MESSAGES: usize = 128;
pub const MAX_RECENT: usize = 5;
pub const MAX_NODE_PICKER_ROWS: usize = 12;
pub const MAX_INSPECTOR_PICKER_ROWS: usize = 12;
pub const RECENT_PROJECTS_PATH: &str = "user://recent_projects.json";
pub const FILE_WATCH_INTERVAL_FRAMES: u32 = 30;
pub const LIST_DOUBLE_CLICK_FRAMES: u32 = 18;
pub const MAX_SCENE_UNDO: usize = 64;
pub const MAX_ANIM_TRACKS: usize = 6;
pub const MAX_ANIM_MARKERS: usize = 24;
pub const DESTRUCTIVE_CONFIRM_TIMEOUT_FRAMES: u32 = 180;

pub fn destructive_confirmation_matches(
    armed_action: &str,
    armed_target: &str,
    armed_frame: u32,
    action: &str,
    target: &str,
    frame: u32,
) -> bool {
    !armed_action.is_empty()
        && armed_action == action
        && armed_target == target
        && frame.wrapping_sub(armed_frame) <= DESTRUCTIVE_CONFIRM_TIMEOUT_FRAMES
}

pub fn clear_destructive_confirmation(state: &mut EditorState) -> bool {
    if state.destructive_confirm_action.is_empty() {
        return false;
    }
    state.destructive_confirm_action.clear();
    state.destructive_confirm_target.clear();
    state.destructive_confirm_frame = 0;
    true
}

pub fn cancel_destructive_confirmation_for_action(state: &mut EditorState, action: &str) -> bool {
    if state.destructive_confirm_action.is_empty() || state.destructive_confirm_action == action {
        return false;
    }
    clear_destructive_confirmation(state)
}

pub fn arm_or_confirm_destructive_action(
    state: &mut EditorState,
    action: &str,
    target: &str,
) -> bool {
    if destructive_confirmation_matches(
        &state.destructive_confirm_action,
        &state.destructive_confirm_target,
        state.destructive_confirm_frame,
        action,
        target,
        state.file_watch_frame,
    ) {
        clear_destructive_confirmation(state);
        return true;
    }
    state.destructive_confirm_action = action.to_string();
    state.destructive_confirm_target = target.to_string();
    state.destructive_confirm_frame = state.file_watch_frame;
    false
}

pub fn tick_destructive_confirmation<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let expired = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.destructive_confirm_action.is_empty()
            || state
                .file_watch_frame
                .wrapping_sub(state.destructive_confirm_frame)
                <= DESTRUCTIVE_CONFIRM_TIMEOUT_FRAMES
        {
            return false;
        }
        clear_destructive_confirmation(state);
        state.log = "confirm canceled\ntimeout".to_string();
        true
    })
    .unwrap_or(false);
    if expired {
        refresh_all(ctx);
    }
}

#[derive(Variant, Clone, Default)]
pub struct SceneSession {
    pub path: String,
    pub doc_text: String,
    pub undo: Vec<String>,
    pub redo: Vec<String>,
    pub dirty: bool,
    pub selected_key: Option<u32>,
    pub collapsed_scene_keys: Vec<u32>,
    pub inspector_expanded_paths: Vec<String>,
    pub inspector_collapsed_sections: Vec<String>,
    pub viewport_mode: String,
    pub viewport_tool: String,
    pub viewport_local: bool,
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
}

pub fn capture_active_scene_session(state: &mut EditorState) -> bool {
    let Some(path) = state.open_paths.get(state.active_open).cloned() else {
        return false;
    };
    let Some(session) = state.scene_sessions.get_mut(state.active_open) else {
        return false;
    };
    session.path = path.clone();
    session.doc_text.clone_from(&state.doc_text);
    session.undo.clone_from(&state.scene_undo_stack);
    session.redo.clone_from(&state.scene_redo_stack);
    session.dirty = state.dirty_scene_paths.iter().any(|dirty| dirty == &path) || state.dirty;
    session.selected_key = state.selected_key;
    session
        .collapsed_scene_keys
        .clone_from(&state.collapsed_scene_keys);
    session
        .inspector_expanded_paths
        .clone_from(&state.inspector_expanded_paths);
    session
        .inspector_collapsed_sections
        .clone_from(&state.inspector_collapsed_sections);
    session.viewport_mode.clone_from(&state.viewport_mode);
    session.viewport_tool.clone_from(&state.viewport_tool);
    session.viewport_local = state.viewport_local;
    session.cam_x = state.cam_x;
    session.cam_y = state.cam_y;
    session.cam_z = state.cam_z;
    session.cam_yaw = state.cam_yaw;
    session.cam_pitch = state.cam_pitch;
    session.cam2_x = state.cam2_x;
    session.cam2_y = state.cam2_y;
    session.cam2_zoom = state.cam2_zoom;
    session.ui_canvas_x = state.ui_canvas_x;
    session.ui_canvas_y = state.ui_canvas_y;
    session.ui_canvas_zoom = state.ui_canvas_zoom;
    true
}

pub fn restore_scene_session(state: &mut EditorState, idx: usize) -> bool {
    let Some(session) = state.scene_sessions.get(idx).cloned() else {
        return false;
    };
    if state.open_paths.get(idx) != Some(&session.path) {
        return false;
    }
    state.active_open = idx;
    state.doc_text = session.doc_text;
    state.scene_undo_stack = session.undo;
    state.scene_redo_stack = session.redo;
    state.selected_key = session.selected_key;
    state.collapsed_scene_keys = session.collapsed_scene_keys;
    state.inspector_expanded_paths = session.inspector_expanded_paths;
    state.inspector_collapsed_sections = session.inspector_collapsed_sections;
    state.viewport_mode = session.viewport_mode;
    state.viewport_tool = session.viewport_tool;
    state.viewport_local = session.viewport_local;
    state.cam_x = session.cam_x;
    state.cam_y = session.cam_y;
    state.cam_z = session.cam_z;
    state.cam_yaw = session.cam_yaw;
    state.cam_pitch = session.cam_pitch;
    state.cam2_x = session.cam2_x;
    state.cam2_y = session.cam2_y;
    state.cam2_zoom = session.cam2_zoom;
    state.ui_canvas_x = session.ui_canvas_x;
    state.ui_canvas_y = session.ui_canvas_y;
    state.ui_canvas_zoom = session.ui_canvas_zoom;
    state.dirty = session.dirty;
    if session.dirty {
        if !state
            .dirty_scene_paths
            .iter()
            .any(|path| path == &session.path)
        {
            state.dirty_scene_paths.push(session.path);
        }
    } else {
        state
            .dirty_scene_paths
            .retain(|path| path != &session.path);
    }
    true
}

#[State]
pub struct EditorState {
    pub editor_shell_root: u64,
    pub inspector_picker_root: u64,
    pub editor_name_cache_names: Vec<String>,
    pub editor_name_cache_ids: Vec<u64>,
    pub project_root: String,
    pub project_name: String,
    pub create_parent_dir: String,
    pub recent_projects: Vec<String>,
    pub file_paths: Vec<String>,
    pub file_filter: String,
    pub file_scope: String,
    pub file_expanded_paths: Vec<String>,
    pub scene_paths: Vec<String>,
    pub open_paths: Vec<String>,
    pub scene_sessions: Vec<SceneSession>,
    pub active_asset_path: String,
    pub active_open: usize,
    pub doc_text: String,
    pub scene_undo_stack: Vec<String>,
    pub scene_redo_stack: Vec<String>,
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
    pub ui_drag_changed: bool,
    pub ui_drag_needs_rebuild: bool,
    pub viewport_mode: String,
    pub viewport_tool: String,
    pub viewport_local: bool,
    pub viewport_snap: bool,
    pub dirty: bool,
    pub add_node_popup_open: bool,
    pub add_node_as_sibling: bool,
    pub inspector_picker_open: bool,
    pub inspector_picker_field: String,
    pub inspector_picker_kind: String,
    pub inspector_picker_offset: usize,
    pub inspector_picker_filter: String,
    pub inspector_expanded_paths: Vec<String>,
    pub inspector_collapsed_sections: Vec<String>,
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
    pub bottom_dock_open: bool,
    pub distraction_free: bool,
    pub command_palette_open: bool,
    pub command_palette_filter: String,
    pub active_anim_path: String,
    pub active_anim_player_key: Option<u32>,
    pub anim_doc_text: String,
    pub anim_dirty: bool,
    pub anim_selected_track: usize,
    pub anim_track_scroll: usize,
    pub anim_playhead: f32,
    pub anim_playing: bool,
    pub anim_loop: bool,
    pub anim_preview_player: u64,
    pub anim_preview_clip: u64,
    pub anim_clip_dirty: bool,
    pub anim_ruler_drag: bool,
    pub anim_marker_ids: Vec<u64>,
    pub anim_playhead_id: u64,
    pub glb_viewer_mesh_ids: Vec<u64>,
    pub glb_viewer_rig_id: u64,
    pub glb_viewer_player: u64,
    pub glb_viewer_clip: u64,
    pub glb_viewer_playing: bool,
    pub active_glb_path: String,
    pub active_glb_summary: String,
    pub active_glb_mesh_index: usize,
    pub active_glb_mat_index: usize,
    pub active_glb_anim_index: usize,
    pub focused_inspector_box: String,
    pub inspector_rotation_mode: String,
    pub inspector_filter: String,
    pub inspector_modified_only: bool,
    pub inspector_layout_applied: bool,
    pub inspector_selected_key: Option<u32>,
    pub script_schema_reload_frames: u32,
    pub destructive_confirm_action: String,
    pub destructive_confirm_target: String,
    pub destructive_confirm_frame: u32,
    pub log: String,
    pub output_messages: Vec<String>,
    pub output_levels: Vec<String>,
    pub output_repeats: Vec<u32>,
    pub output_seen_log: String,
    pub output_filter: String,
    pub output_hide_info: bool,
    pub output_hide_warn: bool,
    pub output_hide_error: bool,
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
        tick_script_schema_reload(ctx);
        tick_destructive_confirmation(ctx);
        update_anim_editor(ctx);
    }
});

methods!({
    fn on_editor_signal(&self, ctx: &mut ScriptContext<'_, API>, sender: NodeID) {
        let Some(name) = get_node_name!(ctx.run, sender).map(|v| v.to_string()) else {
            return;
        };
        let confirmation_action = suffix_index(&name, "scene_tab_close_")
            .and_then(|slot| {
                with_state!(ctx.run, EditorState, ctx.id, |state| {
                    visible_tab_index(state.open_paths.len(), state.active_open, slot)
                })
            })
            .map(|idx| format!("scene_tab_close_{idx}"))
            .unwrap_or_else(|| name.clone());
        let canceled = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            cancel_destructive_confirmation_for_action(state, &confirmation_action)
        })
        .unwrap_or(false);
        if canceled {
            refresh_all(ctx);
        }
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
            "add_node_button" | "scene_add_child_button" => open_add_node_popup(ctx),
            "add_node_sibling_button" => open_add_node_sibling_popup(ctx),
            "add_node_cancel_button" => {
                if with_state!(ctx.run, EditorState, ctx.id, |state| state
                    .inspector_picker_open)
                {
                    set_inspector_picker(ctx, false);
                } else {
                    set_add_node_popup(ctx, false);
                }
            }
            "inspector_pick_cancel_button" => set_inspector_picker(ctx, false),
            "inspector_pick_prev_button" => shift_inspector_picker(ctx, -1),
            "inspector_pick_next_button" => shift_inspector_picker(ctx, 1),
            "inspector_pick_filter_box" => update_inspector_picker_filter(ctx),
            "add_node_prev_button" => {
                if with_state!(ctx.run, EditorState, ctx.id, |state| state
                    .inspector_picker_open)
                {
                    shift_inspector_picker(ctx, -1);
                } else {
                    shift_node_picker(ctx, -1);
                }
            }
            "add_node_next_button" => {
                if with_state!(ctx.run, EditorState, ctx.id, |state| state
                    .inspector_picker_open)
                {
                    shift_inspector_picker(ctx, 1);
                } else {
                    shift_node_picker(ctx, 1);
                }
            }
            "viewport_click_layer" => handle_viewport_click(ctx),
            "mode_ui_button" => set_mode(ctx, "UI"),
            "mode_2d_button" => set_mode(ctx, "2D"),
            "mode_3d_button" => set_mode(ctx, "3D"),
            "viewport_tool_select_button" => set_viewport_tool(ctx, "select"),
            "viewport_tool_move_button" => set_viewport_tool(ctx, "move"),
            "viewport_tool_rotate_button" => set_viewport_tool(ctx, "rotate"),
            "viewport_tool_scale_button" => set_viewport_tool(ctx, "scale"),
            "viewport_space_button" => toggle_viewport_space(ctx),
            "viewport_snap_button" => toggle_viewport_snap(ctx),
            "viewport_frame_button" => frame_selected_node(ctx),
            "activity_scene_button" => set_activity_mode(ctx, "scene"),
            "activity_glb_button" => set_activity_mode(ctx, "glb"),
            "scene_tab_prev_button" => shift_visible_tab_page(ctx, -1),
            "scene_tab_next_button" => shift_visible_tab_page(ctx, 1),
            "bottom_log_button" => toggle_bottom_dock(ctx, false),
            "bottom_anim_button" => toggle_bottom_dock(ctx, true),
            "output_clear_button" => clear_editor_output(ctx),
            "output_filter_box" => update_editor_output_filter(ctx),
            "output_info_button" => toggle_editor_output_level(ctx, "info"),
            "output_warn_button" => toggle_editor_output_level(ctx, "warn"),
            "output_error_button" => toggle_editor_output_level(ctx, "error"),
            "distraction_free_button" => toggle_distraction_free(ctx),
            "command_palette_button" => set_command_palette(ctx, true),
            "command_palette_filter_box" => update_command_palette_filter(ctx),
            "command_palette_close_button" | "command_palette_scrim" => set_command_palette(ctx, false),
            "scene_filter_box" => update_scene_filter(ctx),
            "file_filter_box" => update_file_filter(ctx),
            "file_new_scene_button" => create_quick_asset(ctx, "scene"),
            "file_new_script_button" => create_quick_asset(ctx, "script"),
            "file_new_anim_button" => create_quick_asset(ctx, "anim"),
            "file_new_mat_button" => create_quick_asset(ctx, "mat"),
            "file_new_folder_button" => create_quick_folder(ctx),
            "file_refresh_button" => refresh_project_assets(ctx),
            "file_clear_button" => clear_file_filter_and_scope(ctx),
            "file_up_button" => nav_file_scope_parent(ctx),
            "file_expand_all_button" => expand_file_tree_all(ctx),
            "file_collapse_all_button" => collapse_file_tree_all(ctx),
            "file_duplicate_button" => duplicate_active_asset(ctx),
            "file_delete_button" => delete_active_asset(ctx),
            "file_copy_path_button" => copy_active_asset_path(ctx),
            "anim_create_button" => create_animation_for_selected_player(ctx),
            "anim_add_track_button" => open_anim_track_picker(ctx),
            "anim_close_button" => close_anim_editor(ctx),
            "anim_play_button" => toggle_anim_play(ctx),
            "anim_stop_button" => stop_anim_playback(ctx),
            "anim_loop_button" => toggle_anim_loop(ctx),
            "anim_key_button" => insert_anim_key(ctx),
            "anim_del_key_button" => delete_anim_key(ctx),
            "anim_bind_button" => bind_anim_selection(ctx),
            "anim_rest_button" => reset_anim_to_rest(ctx),
            "anim_save_button" => save_anim_doc(ctx),
            "anim_frame_box" => seek_anim_frame_box(ctx),
            "anim_fps_box" => edit_anim_fps_box(ctx),
            "anim_timeline_ruler" => begin_anim_ruler_seek(ctx),
            "asset_glb_play_button" => toggle_glb_viewer_animation(ctx),
            "scene_duplicate_button" => duplicate_selected_node(ctx),
            "scene_copy_button" => copy_selected_node(ctx),
            "scene_paste_button" => paste_copied_node(ctx),
            "scene_delete_button" => delete_selected_node(ctx),
            "scene_move_up_button" => move_selected_node_order(ctx, -1),
            "scene_move_down_button" => move_selected_node_order(ctx, 1),
            "scene_reparent_out_button" => reparent_selected_node(ctx, -1),
            "scene_reparent_in_button" => reparent_selected_node(ctx, 1),
            "scene_clear_button" => clear_scene_filter(ctx),
            "scene_expand_all_button" => expand_scene_tree_all(ctx),
            "scene_collapse_all_button" => collapse_scene_tree_all(ctx),
            "scene_copy_path_button" => copy_selected_node_path(ctx),
            "scene_select_parent_button" => select_related_node(ctx, "parent"),
            "scene_select_child_button" => select_related_node(ctx, "child"),
            "scene_select_prev_button" => select_related_node(ctx, "prev"),
            "scene_select_next_button" => select_related_node(ctx, "next"),
            "asset_use_button" => use_active_asset_on_selected_node(ctx),
            "asset_glb_anim_button" => export_selected_glb_animation(ctx),
            "asset_glb_mat_button" => export_selected_glb_material(ctx),
            "inspector_name_box" => rename_inspector_selection(ctx),
            "inspector_filter_box" => update_inspector_filter(ctx),
            "inspector_modified_button" => toggle_inspector_modified_only(ctx),
            "inspector_expand_all_button" => set_all_inspector_sections(ctx, false),
            "inspector_collapse_all_button" => set_all_inspector_sections(ctx, true),
            "inspector_pos" => toggle_inspector_section(ctx, "transform"),
            "inspector_vars" => toggle_inspector_section(ctx, "vars"),
            "inspector_position_box" => {
                edit_selected_transform(ctx, "position", "inspector_position_box")
            }
            "inspector_rotation_box" => edit_selected_rotation(ctx),
            "inspector_scale_box" => edit_selected_transform(ctx, "scale", "inspector_scale_box"),
            "inspector_vars_box" => edit_selected_script_vars(ctx),
            "add_node_search_box" => {
                if with_state!(ctx.run, EditorState, ctx.id, |state| state
                    .inspector_picker_open)
                {
                    update_inspector_picker_filter_from(ctx, "add_node_search_box");
                } else {
                    update_node_picker_filter(ctx);
                }
            }
            _ => {
                if let Some(idx) = suffix_index(&name, "anim_track_row_") {
                    select_anim_track(ctx, idx);
                } else if let Some(idx) = suffix_index(&name, "anim_lane_") {
                    click_anim_lane(ctx, idx);
                } else if let Some(idx) = suffix_index(&name, "command_palette_row_") {
                    execute_command_palette_row(ctx, idx);
                } else if let Some(idx) = suffix_index(&name, "manager_recent_") {
                    open_recent_project(ctx, idx);
                } else if let Some(idx) = suffix_index(&name, "add_node_type_") {
                    if with_state!(ctx.run, EditorState, ctx.id, |state| state
                        .inspector_picker_open)
                    {
                        choose_inspector_picker_row(ctx, idx);
                    } else {
                        add_node_from_picker(ctx, idx);
                    }
                } else if let Some(idx) = middle_index(&name, "inspector_var_", "_value") {
                    edit_selected_script_var_path(ctx, idx);
                } else if let Some(idx) = middle_index(&name, "inspector_var_", "_check") {
                    edit_selected_script_var_path(ctx, idx);
                } else if let Some(idx) = middle_index(&name, "inspector_var_", "_color_swatch") {
                    edit_selected_script_var_path(ctx, idx);
                } else if let Some(idx) = inspector_var_component_row(&name) {
                    edit_selected_script_var_path(ctx, idx);
                } else if let Some(idx) = middle_index(&name, "inspector_var_", "_pick_button") {
                    pick_selected_script_var_ref(ctx, idx);
                } else if let Some(idx) = middle_index(&name, "inspector_var_", "_add_button") {
                    mutate_selected_inspector_array(ctx, idx, true);
                } else if let Some(idx) = middle_index(&name, "inspector_var_", "_remove_button") {
                    mutate_selected_inspector_array(ctx, idx, false);
                } else if let Some(idx) = middle_index(&name, "inspector_var_", "_default_button") {
                    reset_selected_inspector_value(ctx, idx);
                } else if middle_index(&name, "inspector_var_", "_quat_button").is_some() {
                    set_inspector_rotation_mode(ctx, "quat");
                } else if middle_index(&name, "inspector_var_", "_euler_button").is_some() {
                    set_inspector_rotation_mode(ctx, "euler");
                } else if let Some(idx) = middle_index(&name, "inspector_var_", "_quat_mode") {
                    set_inspector_quat_mode_from_dropdown(ctx, idx);
                } else if let Some(idx) = middle_index(&name, "inspector_var_", "_bit_all") {
                    set_selected_inspector_bitmask_all(ctx, idx, true);
                } else if let Some(idx) = middle_index(&name, "inspector_var_", "_bit_none") {
                    set_selected_inspector_bitmask_all(ctx, idx, false);
                } else if let Some((idx, bit)) = inspector_var_bit_button(&name) {
                    toggle_selected_inspector_bitmask_bit(ctx, idx, bit);
                } else if let Some(idx) = middle_index(&name, "inspector_var_", "_dropdown") {
                    edit_selected_script_var_path(ctx, idx);
                } else if let Some(idx) = suffix_index(&name, "inspector_pick_row_") {
                    choose_inspector_picker_row(ctx, idx);
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
                } else if let Some(slot) = suffix_index(&name, "scene_tab_close_") {
                    let idx = with_state!(ctx.run, EditorState, ctx.id, |state| {
                        visible_tab_index(state.open_paths.len(), state.active_open, slot)
                    });
                    if let Some(idx) = idx {
                        close_scene_tab(ctx, idx);
                    }
                } else if let Some(slot) = suffix_index(&name, "scene_tab_") {
                    let idx = with_state!(ctx.run, EditorState, ctx.id, |state| {
                        visible_tab_index(state.open_paths.len(), state.active_open, slot)
                    });
                    if let Some(idx) = idx {
                        set_active_tab(ctx, idx);
                    }
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

    fn on_editor_scene_tree_selected(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _tree: NodeID,
        idx: i32,
        _value: Variant,
    ) {
        if idx >= 0 {
            let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                clear_destructive_confirmation(state)
            });
            click_scene_node_slot(ctx, idx as usize);
        }
    }

    fn on_editor_scene_tree_toggled(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _tree: NodeID,
        idx: i32,
        open: bool,
        _value: Variant,
    ) {
        if idx >= 0 {
            set_scene_node_slot_open(ctx, idx as usize, open);
        }
    }

    fn on_editor_file_tree_selected(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _tree: NodeID,
        idx: i32,
        _value: Variant,
    ) {
        if idx >= 0 {
            let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                clear_destructive_confirmation(state)
            });
            click_or_open_file_slot(ctx, idx as usize);
        }
    }

    fn on_editor_file_tree_toggled(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _tree: NodeID,
        idx: i32,
        open: bool,
        _value: Variant,
    ) {
        if idx < 0 {
            return;
        }
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            clear_destructive_confirmation(state)
        });
        let Some(path) = with_state!(ctx.run, EditorState, ctx.id, |state| {
            filtered_file_paths(state).get(idx as usize).cloned()
        }) else {
            return;
        };
        let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            set_file_folder_expanded(state, &path, open)
        })
        .unwrap_or(false);
        if changed {
            refresh_file_panel(ctx);
        }
    }
});

fn inspector_var_bit_button(name: &str) -> Option<(usize, usize)> {
    let rest = name.strip_prefix("inspector_var_")?;
    let (idx, bit) = rest.split_once("_bit_")?;
    let idx = idx.parse().ok()?;
    let bit = bit.parse().ok()?;
    Some((idx, bit))
}

#[cfg(test)]
mod destructive_confirmation_tests {
    use super::*;

    #[test]
    fn repeat_same_action_and_target_confirms() {
        assert!(destructive_confirmation_matches(
            "file_delete_button",
            "art-dir",
            20,
            "file_delete_button",
            "art-dir",
            40,
        ));
    }

    #[test]
    fn action_or_target_change_cancels() {
        assert!(!destructive_confirmation_matches(
            "file_delete_button",
            "old-asset",
            20,
            "scene_tab_close_0",
            "old-asset",
            21,
        ));
        assert!(!destructive_confirmation_matches(
            "file_delete_button",
            "old-asset",
            20,
            "file_delete_button",
            "new-asset",
            21,
        ));
    }

    #[test]
    fn timeout_and_frame_wrap_stay_safe() {
        assert!(!destructive_confirmation_matches(
            "file_delete_button",
            "old-asset",
            20,
            "file_delete_button",
            "old-asset",
            20 + DESTRUCTIVE_CONFIRM_TIMEOUT_FRAMES + 1,
        ));
        assert!(destructive_confirmation_matches(
            "file_delete_button",
            "old-asset",
            u32::MAX - 2,
            "file_delete_button",
            "old-asset",
            1,
        ));
    }

    #[test]
    fn scene_sessions_keep_unsaved_docs_and_undo_per_tab() {
        let mut state = EditorState {
            open_paths: vec!["scene-a".to_string(), "scene-b".to_string()],
            scene_sessions: vec![
                SceneSession {
                    path: "scene-a".to_string(),
                    ..SceneSession::default()
                },
                SceneSession {
                    path: "scene-b".to_string(),
                    doc_text: "doc-b".to_string(),
                    undo: vec!["undo-b".to_string()],
                    viewport_mode: "3D".to_string(),
                    ..SceneSession::default()
                },
            ],
            doc_text: "doc-a-edit".to_string(),
            scene_undo_stack: vec!["undo-a".to_string()],
            dirty: true,
            dirty_scene_paths: vec!["scene-a".to_string()],
            ..EditorState::default()
        };

        assert!(capture_active_scene_session(&mut state));
        assert!(restore_scene_session(&mut state, 1));
        assert_eq!(state.doc_text, "doc-b");
        assert_eq!(state.scene_undo_stack, ["undo-b"]);

        state.doc_text = "doc-b-edit".to_string();
        state.scene_redo_stack = vec!["redo-b".to_string()];
        assert!(capture_active_scene_session(&mut state));
        assert!(restore_scene_session(&mut state, 0));
        assert_eq!(state.doc_text, "doc-a-edit");
        assert_eq!(state.scene_undo_stack, ["undo-a"]);
        assert!(state.dirty);
    }

    #[test]
    fn bottom_dock_tabs_open_switch_and_collapse() {
        assert_eq!(next_bottom_dock_state(false, false, false), (true, false));
        assert_eq!(next_bottom_dock_state(true, false, false), (false, false));
        assert_eq!(next_bottom_dock_state(true, false, true), (true, true));
        assert_eq!(next_bottom_dock_state(true, true, true), (false, true));
    }

    #[test]
    fn distraction_layout_expands_center_and_keeps_width_budget() {
        let normal = editor_layout_metrics(false, false);
        let focus = editor_layout_metrics(true, true);
        let normal_sum = normal.activity_w + normal.left_w + normal.center_w + normal.inspector_w;
        let focus_sum = focus.activity_w + focus.left_w + focus.center_w + focus.inspector_w;
        assert!(normal_sum <= 1.0);
        assert!(focus_sum <= 1.0);
        assert!(focus.center_w > normal.center_w);
        assert!(focus.viewport_h > normal.viewport_h);
    }

    #[test]
    fn output_history_classifies_coalesces_and_filters() {
        let mut state = EditorState {
            log: "dirty scene".to_string(),
            ..EditorState::default()
        };
        capture_editor_output_state(&mut state);
        state.output_seen_log.clear();
        capture_editor_output_state(&mut state);
        assert_eq!(state.output_repeats, [2]);
        assert_eq!(state.output_levels, ["warn"]);
        state.output_filter = "dirty".to_string();
        assert!(filtered_editor_output(&state).contains("x2"));
        state.output_hide_warn = true;
        assert_eq!(filtered_editor_output(&state), "No output");
    }

    #[test]
    fn snap_toggle_shift_inverts() {
        let mut state = EditorState::default();
        assert!(!viewport_snap_active(&state, false));
        assert!(viewport_snap_active(&state, true));
        state.viewport_snap = true;
        assert!(viewport_snap_active(&state, false));
        assert!(!viewport_snap_active(&state, true));
    }

    #[test]
    fn command_palette_filters_all_tokens() {
        let rows = editor_commands("viewport 3d");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "mode_3d");
        assert!(editor_commands("save").len() >= 2);
    }
}

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
            signal!("editor_add_node_sibling"),
            signal!("editor_mode_ui"),
            signal!("editor_mode_2d"),
            signal!("editor_mode_3d"),
            signal!("editor_viewport_tool_select"),
            signal!("editor_viewport_tool_move"),
            signal!("editor_viewport_tool_rotate"),
            signal!("editor_viewport_tool_scale"),
            signal!("editor_viewport_space"),
            signal!("editor_viewport_snap"),
            signal!("editor_viewport_frame"),
            signal!("editor_activity_scene"),
            signal!("editor_activity_glb"),
            signal!("editor_bottom_log"),
            signal!("editor_bottom_anim"),
            signal!("editor_output_clear"),
            signal!("editor_output_filter"),
            signal!("editor_output_info"),
            signal!("editor_output_warn"),
            signal!("editor_output_error"),
            signal!("editor_distraction_free"),
            signal!("editor_command_palette"),
            signal!("editor_command_palette_filter"),
            signal!("editor_command_palette_close"),
            signal!("editor_command_0"),
            signal!("editor_command_1"),
            signal!("editor_command_2"),
            signal!("editor_command_3"),
            signal!("editor_command_4"),
            signal!("editor_command_5"),
            signal!("editor_command_6"),
            signal!("editor_command_7"),
            signal!("editor_scene_filter"),
            signal!("editor_file_filter"),
            signal!("editor_file_new_scene"),
            signal!("editor_file_new_script"),
            signal!("editor_file_new_anim"),
            signal!("editor_file_new_mat"),
            signal!("editor_file_new_folder"),
            signal!("editor_file_refresh"),
            signal!("editor_file_clear"),
            signal!("editor_file_up"),
            signal!("editor_file_expand_all"),
            signal!("editor_file_collapse_all"),
            signal!("editor_file_duplicate"),
            signal!("editor_file_delete"),
            signal!("editor_file_copy_path"),
            signal!("editor_anim_create"),
            signal!("editor_anim_add_track"),
            signal!("editor_anim_close"),
            signal!("editor_anim_play"),
            signal!("editor_anim_stop"),
            signal!("editor_anim_loop"),
            signal!("editor_anim_key"),
            signal!("editor_anim_del_key"),
            signal!("editor_anim_bind"),
            signal!("editor_anim_rest"),
            signal!("editor_anim_save"),
            signal!("editor_anim_frame"),
            signal!("editor_anim_fps"),
            signal!("editor_anim_ruler"),
            signal!("editor_anim_track_0"),
            signal!("editor_anim_track_1"),
            signal!("editor_anim_track_2"),
            signal!("editor_anim_track_3"),
            signal!("editor_anim_track_4"),
            signal!("editor_anim_track_5"),
            signal!("editor_anim_lane_0"),
            signal!("editor_anim_lane_1"),
            signal!("editor_anim_lane_2"),
            signal!("editor_anim_lane_3"),
            signal!("editor_anim_lane_4"),
            signal!("editor_anim_lane_5"),
            signal!("editor_asset_glb_play"),
            signal!("editor_inspector_duplicate"),
            signal!("editor_scene_copy"),
            signal!("editor_scene_paste"),
            signal!("editor_scene_delete"),
            signal!("editor_scene_move_up"),
            signal!("editor_scene_move_down"),
            signal!("editor_scene_reparent_out"),
            signal!("editor_scene_reparent_in"),
            signal!("editor_scene_clear"),
            signal!("editor_scene_expand_all"),
            signal!("editor_scene_collapse_all"),
            signal!("editor_scene_copy_path"),
            signal!("editor_scene_select_parent"),
            signal!("editor_scene_select_child"),
            signal!("editor_scene_select_prev"),
            signal!("editor_scene_select_next"),
            signal!("editor_asset_use"),
            signal!("editor_asset_glb_anim"),
            signal!("editor_asset_glb_mat"),
            signal!("editor_inspector_rename"),
            signal!("editor_inspector_filter"),
            signal!("editor_inspector_modified"),
            signal!("editor_inspector_expand_all"),
            signal!("editor_inspector_collapse_all"),
            signal!("editor_tab_0"),
            signal!("editor_tab_1"),
            signal!("editor_tab_2"),
            signal!("editor_tab_3"),
            signal!("editor_tab_prev"),
            signal!("editor_tab_next"),
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
            signal!("editor_inspector_var_pick_0"),
            signal!("editor_inspector_var_pick_1"),
            signal!("editor_inspector_var_pick_2"),
            signal!("editor_inspector_var_pick_3"),
            signal!("editor_inspector_var_pick_4"),
            signal!("editor_inspector_var_pick_5"),
            signal!("editor_inspector_var_pick_6"),
            signal!("editor_inspector_var_pick_7"),
            signal!("editor_inspector_pick_0"),
            signal!("editor_inspector_pick_1"),
            signal!("editor_inspector_pick_2"),
            signal!("editor_inspector_pick_3"),
            signal!("editor_inspector_pick_4"),
            signal!("editor_inspector_pick_5"),
            signal!("editor_inspector_pick_6"),
            signal!("editor_inspector_pick_7"),
            signal!("editor_inspector_pick_8"),
            signal!("editor_inspector_pick_9"),
            signal!("editor_inspector_pick_10"),
            signal!("editor_inspector_pick_11"),
            signal!("editor_inspector_pick_prev"),
            signal!("editor_inspector_pick_next"),
            signal!("editor_inspector_pick_cancel"),
            signal!("editor_inspector_pick_filter"),
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
    let _ = signal_connect!(
        ctx.run,
        ctx.id,
        signal!("editor_scene_tree_selected"),
        func!("on_editor_scene_tree_selected")
    );
    let _ = signal_connect!(
        ctx.run,
        ctx.id,
        signal!("editor_scene_tree_toggled"),
        func!("on_editor_scene_tree_toggled")
    );
    let _ = signal_connect!(
        ctx.run,
        ctx.id,
        signal!("editor_file_tree_selected"),
        func!("on_editor_file_tree_selected")
    );
    let _ = signal_connect!(
        ctx.run,
        ctx.id,
        signal!("editor_file_tree_toggled"),
        func!("on_editor_file_tree_toggled")
    );
}
