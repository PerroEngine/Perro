use crate::scripts_app_editor_app_rs as editor_app;
use crate::scripts_app_editor_manager_rs as editor_manager;
use crate::scripts_app_editor_project_rs as editor_project;
use crate::scripts_assets_editor_file_watch_rs as editor_file_watch;
use crate::scripts_assets_editor_files_rs as editor_files;
use crate::scripts_editor_main_rs::{
    EditorState, FILE_WATCH_INTERVAL_FRAMES, LIST_DOUBLE_CLICK_FRAMES, MAX_FILES,
    MAX_NODE_PICKER_ROWS, MAX_NODES, MAX_RECENT, MAX_TABS, RECENT_PROJECTS_PATH, cached_scene_doc,
    cached_scene_node, clear_scene_doc_cache, set_state_scene_doc, set_state_scene_doc_loaded,
};
use crate::scripts_scene_editor_animation_rs::*;
use crate::scripts_scene_editor_gizmos_rs as editor_gizmos;
use crate::scripts_scene_editor_nav_rs::*;
use crate::scripts_scene_editor_nodes_rs::*;
use crate::scripts_scene_editor_scene_deps_rs as editor_scene_deps;
use crate::scripts_scene_editor_scene_rs as editor_scene;
use crate::scripts_scene_editor_viewport_rs::*;
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
pub fn open_project<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root: String,
) -> Result<(), String> {
    clear_preview(ctx);
    clear_scene_doc_cache();
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

    crate::scripts_ui_editor_inspector_values_rs::clear_script_schema_cache();
    load_editor_shell(ctx)?;

    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.project_root = root_path.to_string_lossy().to_string();
        state.project_name = project_name;
        state.file_paths = file_paths;
        state.file_scope.clear();
        state.file_expanded_paths.clear();
        state.file_expanded_paths.push("res://".to_string());
        state.scene_paths = scene_paths;
        state.open_paths.clear();
        state.active_asset_path.clear();
        state.active_open = 0;
        state.doc_text.clear();
        state.scene_undo_stack.clear();
        state.scene_redo_stack.clear();
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
        state.last_file_row_click_frame = 0;
        state.last_file_row_click_slot = None;
        state.last_scene_row_click_frame = 0;
        state.last_scene_row_click_slot = None;
        reset_freecam_2d(state);
        state.dirty = false;
        state.activity_mode = "scene".to_string();
        state.sidebar_mode = "scene".to_string();
        state.anim_drawer_open = false;
        state.active_anim_path.clear();
        state.active_anim_player_key = None;
        state.active_glb_path.clear();
        state.active_glb_summary.clear();
        state.script_schema_reload_frames = 0;
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

pub fn load_editor_shell<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
) -> Result<(), String> {
    let (old, old_picker) = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let old = state.editor_shell_root;
        let old_picker = state.inspector_picker_root;
        state.editor_shell_root = 0;
        state.inspector_picker_root = 0;
        state.editor_name_cache_names.clear();
        state.editor_name_cache_ids.clear();
        (old, old_picker)
    })
    .unwrap_or((0, 0));
    clear_name_cache();
    if old != 0 {
        let _ = ctx.run.Nodes().remove_node(NodeID::from_u64(old));
    }
    if old_picker != 0 {
        let _ = ctx.run.Nodes().remove_node(NodeID::from_u64(old_picker));
    }

    let root = ctx
        .run
        .Scene()
        .load(editor_app::EDITOR_SHELL_SCENE.to_string())
        .map_err(|err| format!("editor shell load fail\n{err}"))?;
    let _ = ctx.run.Nodes().reparent(ctx.id, root);
    let picker_root = ctx
        .run
        .Scene()
        .load(editor_app::INSPECTOR_PICKER_SCENE.to_string())
        .map_err(|err| format!("inspector picker load fail\n{err}"))?;
    let _ = ctx.run.Nodes().reparent(ctx.id, picker_root);
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.editor_shell_root = root.as_u64();
        state.inspector_picker_root = picker_root.as_u64();
        state.editor_name_cache_names.clear();
        state.editor_name_cache_ids.clear();
    });
    clear_name_cache();
    Ok(())
}

pub fn open_project_dialog<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    if let Some(path) = FileMod::pick_folder("Open Perro Project")
        && let Err(err) = open_project(ctx, path.clone())
    {
        set_log(ctx, &format!("open project fail\n{path}\n{err}"));
        refresh_recent_projects(ctx);
        set_project_manager(ctx, true);
    }
}

pub fn choose_create_location<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    if let Some(path) = FileMod::pick_folder("Choose Project Location") {
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            state.create_parent_dir = path.clone();
            state.log = format!("create location\n{path}");
        });
        refresh_all(ctx);
    }
}

pub fn create_project_from_manager<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
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

pub fn open_recent_project<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, idx: usize) {
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

pub fn refresh_recent_projects<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let recent = load_recent_projects();
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.recent_projects = recent;
    });
    refresh_all(ctx);
}

pub fn add_recent_project<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, root: &str) {
    let mut recent = load_recent_projects();
    recent.retain(|item| item != root);
    recent.insert(0, root.to_string());
    recent.truncate(MAX_RECENT);
    save_recent_projects(&recent);
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.recent_projects = recent;
    });
}

pub fn validate_project_root(root: &Path) -> Result<(), String> {
    if !root.join(".perro").is_dir() {
        return Err("missing .perro dir".to_string());
    }
    if !root.join("project.toml").is_file() {
        return Err("missing project.toml".to_string());
    }
    Ok(())
}

pub fn scan_res_paths(root: &Path) -> Result<Vec<String>, String> {
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
    let mut folders = Vec::new();
    for path in out.iter() {
        let mut cursor = parent_res_folder(path);
        while !cursor.is_empty() {
            if !folders.iter().any(|item| item == &cursor)
                && !out.iter().any(|item| item == &cursor)
            {
                folders.push(cursor.clone());
            }
            cursor = parent_res_folder(&cursor);
        }
    }
    out.extend(folders);
    out.sort_by_key(|path| editor_files::res_browser_sort_key(path));
    Ok(out)
}

pub fn refresh_project_assets<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
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
                state.file_expanded_paths.retain(|path| {
                    path == "res://" || state.file_paths.iter().any(|file_path| file_path == path)
                });
                if !state
                    .file_expanded_paths
                    .iter()
                    .any(|path| path == "res://")
                {
                    state.file_expanded_paths.push("res://".to_string());
                }
                state.project_file_sigs = editor_file_watch::scan_project(root_path.as_path());
                state.log = format!("refresh project\nassets={count}");
            });
            rebuild_preview(ctx);
        }
        Err(err) => set_log(ctx, &format!("refresh fail\n{err}")),
    }
    refresh_all(ctx);
}

pub fn open_file_slot<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, idx: usize) {
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
            toggle_file_folder_expanded(state, &scene_path);
            state.file_scope = scene_path.clone();
            state.file_filter.clear();
            state.active_asset_path = scene_path.clone();
            state.activity_mode = "scene".to_string();
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

pub fn click_or_open_file_slot<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
) {
    let res_path = with_state!(ctx.run, EditorState, ctx.id, |state| {
        filtered_file_paths(state).get(idx).cloned()
    });
    let Some(scene_path) = res_path else {
        return;
    };
    let was_selected = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.active_asset_path == scene_path
    });
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.active_asset_path = scene_path.clone();
        state.sidebar_mode = "files".to_string();
    });
    if scene_path.ends_with(".scn") {
        open_scene_path(ctx, &scene_path);
        return;
    }
    let should_open = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let frame = state.file_watch_frame;
        let should_open = state
            .last_file_row_click_slot
            .is_some_and(|prev| prev == idx)
            && frame.wrapping_sub(state.last_file_row_click_frame) <= LIST_DOUBLE_CLICK_FRAMES;
        state.last_file_row_click_slot = Some(idx);
        state.last_file_row_click_frame = frame;
        should_open || (was_selected && scene_path.ends_with('/'))
    })
    .unwrap_or(false);
    if should_open {
        open_file_slot(ctx, idx);
        return;
    }
    if scene_path.ends_with('/') {
        set_log(
            ctx,
            &format!("folder\n{}", editor_files::rel_label(&scene_path)),
        );
    }
    refresh_all(ctx);
}

pub fn toggle_file_folder_expanded(state: &mut EditorState, path: &str) {
    if path == "res://" {
        if !state
            .file_expanded_paths
            .iter()
            .any(|expanded| expanded == "res://")
        {
            state.file_expanded_paths.push("res://".to_string());
        }
        return;
    }
    if let Some(pos) = state
        .file_expanded_paths
        .iter()
        .position(|expanded| expanded == path)
    {
        state.file_expanded_paths.remove(pos);
        let prefix = path.to_string();
        state
            .file_expanded_paths
            .retain(|expanded| !expanded.starts_with(&prefix));
    } else {
        state.file_expanded_paths.push(path.to_string());
    }
}

pub fn set_file_folder_expanded(state: &mut EditorState, path: &str, open: bool) -> bool {
    if path == "res://" {
        if open
            && !state
                .file_expanded_paths
                .iter()
                .any(|expanded| expanded == "res://")
        {
            state.file_expanded_paths.push("res://".to_string());
            return true;
        }
        return false;
    }
    let Some(pos) = state
        .file_expanded_paths
        .iter()
        .position(|expanded| expanded == path)
    else {
        if open {
            state.file_expanded_paths.push(path.to_string());
            return true;
        }
        return false;
    };
    if open {
        return false;
    }
    state.file_expanded_paths.remove(pos);
    let prefix = path.to_string();
    state
        .file_expanded_paths
        .retain(|expanded| !expanded.starts_with(&prefix));
    true
}

pub fn clear_file_filter_and_scope<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.file_filter.clear();
        state.file_scope.clear();
        state.focused_inspector_box.clear();
        if state.active_asset_path.ends_with('/') {
            state.active_asset_path = "res://".to_string();
        }
        if !state
            .file_expanded_paths
            .iter()
            .any(|path| path == "res://")
        {
            state.file_expanded_paths.push("res://".to_string());
        }
        state.log = "clear files filter".to_string();
    });
    refresh_all(ctx);
}

pub fn expand_file_tree_all<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.file_expanded_paths = state
            .file_paths
            .iter()
            .filter(|path| path.ends_with('/'))
            .cloned()
            .collect();
        if !state
            .file_expanded_paths
            .iter()
            .any(|path| path == "res://")
        {
            state.file_expanded_paths.push("res://".to_string());
        }
        state.log = format!("expand files\n{} dirs", state.file_expanded_paths.len());
    });
    refresh_all(ctx);
}

pub fn collapse_file_tree_all<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.file_expanded_paths.clear();
        state.file_expanded_paths.push("res://".to_string());
        state.file_scope.clear();
        if state.active_asset_path.ends_with('/') {
            state.active_asset_path = "res://".to_string();
        }
        state.log = "fold files\nroot".to_string();
    });
    refresh_all(ctx);
}

pub fn open_scene_path<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    scene_path: &str,
) {
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
        state.activity_mode = "scene".to_string();
        state.sidebar_mode = "scene".to_string();
        set_state_scene_doc_loaded(state, &doc);
        state.selected_key = first_key;
        state.collapsed_scene_keys.clear();
        state.inspector_expanded_paths.clear();
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

pub fn open_animation_path<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    anim_path: &str,
) {
    let root = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.project_root.clone()
    });
    let abs = res_to_abs(&root, anim_path);
    match FileMod::load_string(&abs) {
        Ok(_) => {
            let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                state.activity_mode = "scene".to_string();
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

pub fn open_gltf_path<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, gltf_path: &str) {
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
        state.activity_mode = "glb".to_string();
        state.sidebar_mode = "files".to_string();
        state.anim_drawer_open = false;
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

pub fn cycle_active_glb_ref<API: ScriptAPI + ?Sized>(
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

pub fn set_active_tab<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, idx: usize) {
    let needs_save = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state
            .open_paths
            .get(state.active_open)
            .map(|path| {
                idx != state.active_open
                    && state.dirty_scene_paths.iter().any(|dirty| dirty == path)
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
    open_scene_path(ctx, &path);
}

pub fn cycle_scene_tab<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, dir: isize) {
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

pub fn close_active_scene_tab<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let idx = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (!state.open_paths.is_empty()).then_some(state.active_open)
    });
    if let Some(idx) = idx {
        close_scene_tab(ctx, idx);
    }
}

pub fn close_all_scene_tabs<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
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

pub fn close_scene_tab<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, idx: usize) {
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
        let target = state.open_paths.get(idx).cloned()?;
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

pub fn open_first_scene<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
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

pub fn create_quick_asset<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, kind: &str) {
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
                state.activity_mode = "scene".to_string();
                state.active_asset_path = path.clone();
                state.file_scope = parent_res_folder(&path);
                reveal_file_path_in_tree(state, &path);
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

pub fn create_quick_folder<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
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
                state.activity_mode = "scene".to_string();
                state.active_asset_path = path.clone();
                state.file_scope = parent_res_folder(&path);
                reveal_file_path_in_tree(state, &path);
                state.log = format!("new folder\n{path}");
            });
            refresh_all(ctx);
        }
        Err(err) => set_log(ctx, &format!("new folder fail\n{path}\n{err}")),
    }
}

pub fn duplicate_active_asset<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let request = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.project_root.is_empty() || state.active_asset_path.is_empty() {
            return None;
        }
        let source = state.active_asset_path.clone();
        if source == "res://" {
            return None;
        }
        let target = duplicate_res_target(&state.project_root, &source)?;
        Some((state.project_root.clone(), source, target))
    });
    let Some((root, source, target)) = request else {
        set_log(ctx, "dup asset fail\nselect asset");
        return;
    };
    let source_abs = res_to_abs(&root, &source);
    let target_abs = res_to_abs(&root, &target);
    let result = if source.ends_with('/') {
        copy_dir_recursive(Path::new(&source_abs), Path::new(&target_abs))
    } else {
        if let Some(parent) = Path::new(&target_abs).parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::copy(&source_abs, &target_abs)
            .map(|_| ())
            .map_err(|err| err.to_string())
    };
    match result {
        Ok(()) => {
            let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                if let Ok(paths) = scan_res_paths(Path::new(&state.project_root)) {
                    state.file_paths = paths;
                }
                state.sidebar_mode = "files".to_string();
                state.activity_mode = "scene".to_string();
                state.active_asset_path = target.clone();
                state.file_scope = parent_res_folder(&target);
                reveal_file_path_in_tree(state, &target);
                state.log = format!("dup asset\n{target}");
            });
            refresh_all(ctx);
        }
        Err(err) => set_log(ctx, &format!("dup asset fail\n{source}\n{err}")),
    }
}

pub fn copy_active_asset_path<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.active_asset_path.is_empty() {
            state.log = "asset path fail\nselect asset".to_string();
        } else {
            state.log = format!("asset path\n{}", state.active_asset_path);
        }
        true
    })
    .unwrap_or(false);
    if changed {
        refresh_all(ctx);
    }
}

pub fn open_active_asset<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let path = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.active_asset_path.is_empty() {
            state.log = "open asset fail\nselect asset".to_string();
            return None;
        }
        let path = state.active_asset_path.clone();
        state.sidebar_mode = "files".to_string();
        state.activity_mode = "scene".to_string();
        state.file_scope = parent_res_folder(&path);
        reveal_file_path_in_tree(state, &path);
        Some(path)
    })
    .flatten();
    let Some(path) = path else {
        refresh_all(ctx);
        return;
    };
    if path.ends_with('/') {
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            toggle_file_folder_expanded(state, &path);
            state.file_scope = path.clone();
            state.log = format!("folder\n{}", editor_files::rel_label(&path));
        });
        refresh_all(ctx);
        return;
    }
    if path.ends_with(".panim") {
        open_animation_path(ctx, &path);
    } else if is_gltf_path(&path) {
        open_gltf_path(ctx, &path);
    } else if path.ends_with(".scn") {
        open_scene_path(ctx, &path);
    } else {
        set_log(
            ctx,
            &format!(
                "{} file\n{}",
                editor_files::kind_label(&path),
                editor_files::rel_label(&path)
            ),
        );
        refresh_all(ctx);
    }
}

pub fn duplicate_res_target(project_root: &str, source: &str) -> Option<String> {
    let parent = parent_res_folder(source)
        .strip_prefix("res://")
        .map(|path| path.trim_matches('/').to_string())
        .unwrap_or_default();
    let rel = editor_files::rel_label(source);
    let trimmed = rel.trim_end_matches('/');
    let name = Path::new(trimmed).file_name()?.to_str()?;
    if source.ends_with('/') {
        let stem = sanitize_file_stem(&format!("{name}_copy"));
        return Some(unique_res_folder_path(project_root, &parent, &stem));
    }
    let ext = Path::new(trimmed)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("asset");
    let stem = Path::new(trimmed)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("asset");
    Some(unique_res_path(
        project_root,
        &parent,
        &sanitize_file_stem(&format!("{stem}_copy")),
        ext,
    ))
}

pub fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|err| err.to_string())?;
    for entry in fs::read_dir(source).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let from = entry.path();
        let to = target.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            fs::copy(&from, &to)
                .map(|_| ())
                .map_err(|err| err.to_string())?;
        }
    }
    Ok(())
}

pub fn delete_active_asset<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let request = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.project_root.is_empty() || state.active_asset_path.is_empty() {
            return None;
        }
        let path = state.active_asset_path.clone();
        if path == "res://" {
            return None;
        }
        if state.dirty_scene_paths.iter().any(|dirty| dirty == &path) {
            return Some((state.project_root.clone(), path, true));
        }
        Some((state.project_root.clone(), path, false))
    });
    let Some((root, path, dirty_blocked)) = request else {
        set_log(ctx, "delete asset fail\nselect asset");
        return;
    };
    if dirty_blocked {
        set_log(ctx, &format!("delete asset blocked\nsave first\n{path}"));
        return;
    }
    let abs = res_to_abs(&root, &path);
    let result = if path.ends_with('/') {
        fs::remove_dir_all(&abs).map_err(|err| err.to_string())
    } else {
        fs::remove_file(&abs).map_err(|err| err.to_string())
    };
    match result {
        Ok(()) => {
            let delete_state = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                let removed_active_open = state
                    .open_paths
                    .get(state.active_open)
                    .is_some_and(|open| open == &path);
                state.open_paths.retain(|open| open != &path);
                state.dirty_scene_paths.retain(|dirty| dirty != &path);
                if state.active_open >= state.open_paths.len() {
                    state.active_open = state.open_paths.len().saturating_sub(1);
                }
                if let Ok(paths) = scan_res_paths(Path::new(&state.project_root)) {
                    state.file_paths = paths;
                }
                state.file_expanded_paths.retain(|expanded| {
                    expanded == "res://"
                        || state
                            .file_paths
                            .iter()
                            .any(|file_path| file_path == expanded)
                });
                state.active_asset_path = parent_res_folder(&path);
                if state.active_asset_path.is_empty() {
                    state.active_asset_path = "res://".to_string();
                }
                state.file_scope = parent_res_folder(&path);
                if !state.active_asset_path.is_empty() {
                    let reveal_path = state.active_asset_path.clone();
                    reveal_file_path_in_tree(state, &reveal_path);
                }
                state.sidebar_mode = "files".to_string();
                state.activity_mode = "scene".to_string();
                state.log = format!("delete asset\n{path}");
                if removed_active_open {
                    if let Some(next) = state.open_paths.get(state.active_open).cloned() {
                        return (Some(next), true);
                    }
                    state.doc_text.clear();
                    state.selected_key = None;
                    state.preview_scene_paths.clear();
                    state.preview_root = 0;
                    state.preview_node_ids.clear();
                    state.preview_node_keys.clear();
                    state.dirty = false;
                }
                (None, removed_active_open)
            })
            .unwrap_or((None, false));
            if let Some(next) = delete_state.0 {
                open_scene_path(ctx, &next);
            } else if delete_state.1 {
                clear_preview(ctx);
                refresh_all(ctx);
            } else {
                refresh_all(ctx);
            }
        }
        Err(err) => set_log(ctx, &format!("delete asset fail\n{path}\n{err}")),
    }
}

pub fn rename_inspector_selection<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let rename_asset = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.sidebar_mode == "files" && !state.active_asset_path.is_empty()
    });
    if rename_asset {
        rename_active_asset(ctx);
    } else {
        rename_selected_node(ctx);
    }
}

pub fn rename_active_asset<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(raw_name) = read_text_box(ctx, "inspector_name_box") else {
        return;
    };
    let request = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.project_root.is_empty() || state.active_asset_path.is_empty() {
            return None;
        }
        let source = state.active_asset_path.clone();
        if source == "res://" || state.dirty_scene_paths.iter().any(|dirty| dirty == &source) {
            return Some((state.project_root.clone(), source, String::new(), true));
        }
        let target = rename_res_target(&state.project_root, &source, &raw_name)?;
        Some((state.project_root.clone(), source, target, false))
    });
    let Some((root, source, target, blocked)) = request else {
        set_log(ctx, "rename asset fail\nbad name");
        return;
    };
    if blocked {
        set_log(ctx, &format!("rename asset blocked\nsave first\n{source}"));
        return;
    }
    if source == target {
        set_log(ctx, &format!("rename asset\nno change\n{source}"));
        refresh_all(ctx);
        return;
    }
    let source_abs = res_to_abs(&root, &source);
    let target_abs = res_to_abs(&root, &target);
    if Path::new(&target_abs).exists() {
        set_log(ctx, &format!("rename asset fail\nexists\n{target}"));
        return;
    }
    if let Some(parent) = Path::new(&target_abs).parent() {
        let _ = fs::create_dir_all(parent);
    }
    match fs::rename(&source_abs, &target_abs) {
        Ok(()) => {
            let (next_open, scene_paths, dirty_paths) =
                with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                    for item in state.open_paths.iter_mut() {
                        if item == &source {
                            *item = target.clone();
                        }
                    }
                    for item in state.dirty_scene_paths.iter_mut() {
                        if item == &source {
                            *item = target.clone();
                        }
                    }
                    let active_open_path = state.open_paths.get(state.active_open).cloned();
                    if !state.doc_text.is_empty() {
                        let mut doc = cached_scene_doc(&state.doc_text);
                        if rewrite_asset_refs_in_doc(&mut doc, &source, &target) {
                            set_state_scene_doc(state, &doc);
                            if let Some(path) = active_open_path.clone()
                                && !state.dirty_scene_paths.iter().any(|item| item == &path)
                            {
                                state.dirty_scene_paths.push(path);
                            }
                            state.dirty = true;
                        }
                    }
                    if let Ok(paths) = scan_res_paths(Path::new(&state.project_root)) {
                        state.file_paths = paths;
                        state.scene_paths = state
                            .file_paths
                            .iter()
                            .filter(|path| path.ends_with(".scn"))
                            .cloned()
                            .collect();
                    }
                    state.active_asset_path = target.clone();
                    state.file_scope = parent_res_folder(&target);
                    reveal_file_path_in_tree(state, &target);
                    state.log = format!("rename asset\n{source} -> {target}");
                    let next_open = state
                        .open_paths
                        .get(state.active_open)
                        .filter(|open| *open == &target)
                        .cloned();
                    (
                        next_open,
                        state.scene_paths.clone(),
                        state.dirty_scene_paths.clone(),
                    )
                })
                .unwrap_or((None, Vec::new(), Vec::new()));
            let rewrite_count =
                rewrite_clean_scene_asset_refs(&root, &scene_paths, &dirty_paths, &source, &target);
            if rewrite_count > 0 {
                let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                    state.project_file_sigs = editor_file_watch::scan_project(Path::new(&root));
                    state.log =
                        format!("rename asset\n{source} -> {target}\nupd refs={rewrite_count}");
                });
            }
            if let Some(next) = next_open {
                open_scene_path(ctx, &next);
            } else {
                refresh_all(ctx);
            }
        }
        Err(err) => set_log(ctx, &format!("rename asset fail\n{source}\n{err}")),
    }
}

pub fn rewrite_clean_scene_asset_refs(
    root: &str,
    scene_paths: &[String],
    dirty_paths: &[String],
    source: &str,
    target: &str,
) -> usize {
    let mut changed = 0;
    for path in scene_paths {
        if dirty_paths.iter().any(|dirty| dirty == path) {
            continue;
        }
        let abs = res_to_abs(root, path);
        let Ok(text) = FileMod::load_string(&abs) else {
            continue;
        };
        let mut doc = SceneDoc::parse(&text);
        if !rewrite_asset_refs_in_doc(&mut doc, source, target) {
            continue;
        }
        if FileMod::save_string(&abs, &doc.to_text()).is_ok() {
            changed += 1;
        }
    }
    changed
}

pub fn rewrite_asset_refs_in_doc(doc: &mut SceneDoc, source: &str, target: &str) -> bool {
    let mut changed = false;
    for node in doc.scene.nodes.to_mut().iter_mut() {
        if let Some(root_of) = node.root_of.as_mut()
            && let Some(next) = renamed_asset_ref(root_of.as_ref(), source, target)
        {
            *root_of = Cow::Owned(next);
            changed = true;
        }
        if let Some(script) = node.script.as_mut()
            && let Some(next) = renamed_asset_ref(script.as_ref(), source, target)
        {
            *script = Cow::Owned(next);
            changed = true;
        }
        changed |= rewrite_asset_refs_in_data(&mut node.data, source, target);
        for (_field, value) in node.script_vars.to_mut().iter_mut() {
            changed |= rewrite_asset_refs_in_value(value, source, target);
        }
    }
    changed
}

pub fn rewrite_asset_refs_in_data(data: &mut SceneNodeData, source: &str, target: &str) -> bool {
    let mut changed = false;
    for (_field, value) in data.fields.to_mut().iter_mut() {
        changed |= rewrite_asset_refs_in_value(value, source, target);
    }
    if let Some(base) = data.base.as_mut() {
        match base {
            perro_scene::SceneNodeDataBase::Borrowed(_) => {}
            perro_scene::SceneNodeDataBase::Owned(base) => {
                changed |= rewrite_asset_refs_in_data(base, source, target);
            }
        }
    }
    changed
}

pub fn rewrite_asset_refs_in_value(value: &mut SceneValue, source: &str, target: &str) -> bool {
    match value {
        SceneValue::Str(path) => {
            if let Some(next) = renamed_asset_ref(path.as_ref(), source, target) {
                *path = Cow::Owned(next);
                true
            } else {
                false
            }
        }
        SceneValue::Object(fields) => fields
            .to_mut()
            .iter_mut()
            .any(|(_field, value)| rewrite_asset_refs_in_value(value, source, target)),
        SceneValue::Array(values) => values
            .to_mut()
            .iter_mut()
            .any(|value| rewrite_asset_refs_in_value(value, source, target)),
        _ => false,
    }
}

pub fn renamed_asset_ref(path: &str, source: &str, target: &str) -> Option<String> {
    if path == source {
        return Some(target.to_string());
    }
    let base = base_res_asset_path(path);
    if base == source && path.len() > source.len() {
        return Some(format!("{target}{}", &path[source.len()..]));
    }
    None
}

pub fn rename_res_target(project_root: &str, source: &str, raw_name: &str) -> Option<String> {
    let parent = parent_res_folder(source);
    let parent_dir = parent
        .strip_prefix("res://")
        .map(|path| path.trim_matches('/').to_string())
        .unwrap_or_default();
    let rel = editor_files::rel_label(source);
    let trimmed = rel.trim_end_matches('/');
    let old_name = Path::new(trimmed).file_name()?.to_str()?;
    let clean = raw_name.trim().trim_matches('/').trim_matches('\\');
    if clean.is_empty() || clean.contains('/') || clean.contains('\\') {
        return None;
    }
    if source.ends_with('/') {
        let stem = sanitize_file_stem(clean);
        if stem.is_empty() {
            return None;
        }
        return Some(if parent_dir.is_empty() {
            format!("res://{stem}/")
        } else {
            format!("res://{parent_dir}/{stem}/")
        });
    }
    let old_ext = Path::new(old_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let clean_path = Path::new(clean);
    let clean_ext = clean_path.extension().and_then(|value| value.to_str());
    let stem = clean_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(sanitize_file_stem)
        .filter(|value| !value.is_empty())?;
    let ext = clean_ext.unwrap_or(old_ext);
    let file = if ext.is_empty() {
        stem
    } else {
        format!("{stem}.{ext}")
    };
    let target = if parent_dir.is_empty() {
        format!("res://{file}")
    } else {
        format!("res://{parent_dir}/{file}")
    };
    if Path::new(&res_to_abs(project_root, &target)).exists() && target != source {
        return None;
    }
    Some(target)
}

pub fn attach_script_to_selected_node<API: ScriptAPI + ?Sized>(
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
        let mut doc = cached_scene_doc(&state.doc_text);
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
        set_state_scene_doc(state, &doc);
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

pub fn attach_material_to_selected_node<API: ScriptAPI + ?Sized>(
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
        let mut doc = cached_scene_doc(&state.doc_text);
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
        set_state_scene_doc(state, &doc);
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

pub fn attach_animation_to_selected_player<API: ScriptAPI + ?Sized>(
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
        let mut doc = cached_scene_doc(&state.doc_text);
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
        set_state_scene_doc(state, &doc);
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.activity_mode = "glb".to_string();
        state.anim_drawer_open = false;
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

pub fn export_selected_glb_animation<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
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
                        state.activity_mode = "scene".to_string();
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

pub fn export_selected_glb_material<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
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

pub fn active_gltf_asset_path(state: &EditorState) -> Option<String> {
    if is_gltf_path(&state.active_asset_path) {
        Some(state.active_asset_path.clone())
    } else if is_gltf_path(&state.active_glb_path) {
        Some(state.active_glb_path.clone())
    } else {
        None
    }
}

pub fn glb_asset_stem(path: &str) -> String {
    sanitize_file_stem(
        Path::new(&editor_files::rel_label(path))
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("glb"),
    )
}

pub fn unique_res_path(project_root: &str, dir: &str, stem: &str, ext: &str) -> String {
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

pub fn unique_res_folder_path(project_root: &str, dir: &str, stem: &str) -> String {
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

pub fn quick_asset_dir(state: &EditorState, kind: &str) -> String {
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

pub fn reveal_file_path_in_tree(state: &mut EditorState, path: &str) {
    if !state
        .file_expanded_paths
        .iter()
        .any(|expanded| expanded == "res://")
    {
        state.file_expanded_paths.push("res://".to_string());
    }
    for folder in file_ancestor_folders(path) {
        if !state
            .file_expanded_paths
            .iter()
            .any(|expanded| expanded == &folder)
        {
            state.file_expanded_paths.push(folder);
        }
    }
    if path.ends_with('/')
        && !state
            .file_expanded_paths
            .iter()
            .any(|expanded| expanded == path)
    {
        state.file_expanded_paths.push(path.to_string());
    }
}

pub fn res_folder_dir(path: &str) -> Option<String> {
    path.trim_end_matches('/')
        .strip_prefix("res://")
        .map(|path| path.trim_matches('/').to_string())
}

pub fn quick_asset_stem(state: &EditorState, kind: &str) -> String {
    if !state.doc_text.is_empty()
        && let Some(key) = state.selected_key
    {
        let doc = cached_scene_doc(&state.doc_text);
        if let Some(node) = cached_scene_node(&state.doc_text, key) {
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

pub fn default_scene_text(name: &str) -> String {
    format!(
        "$root = @{name}\n\n[{name}]\n    [Node2D]\n        position = (0.0, 0.0)\n    [/Node2D]\n[/{name}]\n"
    )
}

pub fn default_script_text() -> String {
    format!(
        "use perro_api::prelude::*;\n\n{} = Node;\n\n{}!({{\n    {}(&self, _ctx: &mut ScriptContext<'_, API>) {{\n    }}\n}});\n",
        "type SelfNodeType", "lifecycle", "fn on_init"
    )
}

pub fn default_material_pmat() -> String {
    "type = \"standard\"\ncolor = (1.0, 1.0, 1.0, 1.0)\nroughness = 0.65\nmetallic = 0.0\n"
        .to_string()
}
