use crate::scripts_app_editor_app_rs as editor_app;
use crate::scripts_app_editor_manager_rs as editor_manager;
use crate::scripts_app_editor_project_rs as editor_project;
use crate::scripts_assets_editor_assets_rs::*;
use crate::scripts_assets_editor_file_watch_rs as editor_file_watch;
use crate::scripts_assets_editor_files_rs as editor_files;
use crate::scripts_editor_main_rs::{
    EditorState, FILE_WATCH_INTERVAL_FRAMES, LIST_DOUBLE_CLICK_FRAMES, MAX_FILES,
    MAX_NODE_PICKER_ROWS, MAX_NODES, MAX_RECENT, MAX_TABS, RECENT_PROJECTS_PATH, cached_scene_doc, cached_scene_doc_shared,
    cached_scene_node, capture_active_scene_session, set_state_scene_doc,
};
use crate::scripts_scene_editor_animation_rs::*;
use crate::scripts_scene_editor_gizmos_rs as editor_gizmos;
use crate::scripts_scene_editor_nav_rs::*;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorAssetFilter {
    pub label: &'static str,
    pub extensions: &'static [&'static str],
}

pub fn editor_asset_filters(kind: perro_scene::SceneAssetKind) -> &'static [EditorAssetFilter] {
    match kind {
        perro_scene::SceneAssetKind::Scene => &[EditorAssetFilter {
            label: "Scenes",
            extensions: &["scn"],
        }],
        perro_scene::SceneAssetKind::Script => &[EditorAssetFilter {
            label: "Rust Scripts",
            extensions: &["rs"],
        }],
        perro_scene::SceneAssetKind::Texture => &[EditorAssetFilter {
            label: "Images",
            extensions: &["png", "jpg", "jpeg", "webp", "bmp", "tga", "svg"],
        }],
        perro_scene::SceneAssetKind::Mesh | perro_scene::SceneAssetKind::Model => {
            &[EditorAssetFilter {
                label: "Meshes",
                extensions: &["glb", "gltf", "pmesh", "obj", "fbx"],
            }]
        }
        perro_scene::SceneAssetKind::Material => &[EditorAssetFilter {
            label: "Perro Materials",
            extensions: &["pmat"],
        }],
        perro_scene::SceneAssetKind::Animation => &[EditorAssetFilter {
            label: "Perro Animations",
            extensions: &["panim"],
        }],
        perro_scene::SceneAssetKind::AnimationTree => &[EditorAssetFilter {
            label: "Perro Animation Trees",
            extensions: &["panimtree"],
        }],
        perro_scene::SceneAssetKind::Skeleton => &[EditorAssetFilter {
            label: "Perro Skeletons",
            extensions: &["pskel", "pskel2d", "pskel3d"],
        }],
        perro_scene::SceneAssetKind::ParticleProfile => &[EditorAssetFilter {
            label: "Perro Particles",
            extensions: &["ppart"],
        }],
        perro_scene::SceneAssetKind::TileSet => &[EditorAssetFilter {
            label: "Perro Tile Sets",
            extensions: &["ptileset"],
        }],
        perro_scene::SceneAssetKind::UiStyle => &[EditorAssetFilter {
            label: "Perro UI Styles",
            extensions: &["uistyle"],
        }],
    }
}
pub fn select_node_slot<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, idx: usize) {
    let key = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.doc_text.is_empty() {
            None
        } else {
            let doc = cached_scene_doc_shared(&state.doc_text);
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
    }).unwrap_or_default();
    if let Some(key) = key {
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            state.selected_key = Some(key);
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
        });
        refresh_selection_panels(ctx);
    }
}

pub fn click_scene_node_slot<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
) {
    let Some(key) = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.doc_text.is_empty() {
            None
        } else {
            let doc = cached_scene_doc_shared(&state.doc_text);
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
    }).unwrap_or_default() else {
        return;
    };

    let was_selected = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.selected_key == Some(key)
    }).unwrap_or_default();
    let should_toggle = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let frame = state.file_watch_frame;
        let should_toggle = state
            .last_scene_row_click_slot
            .is_some_and(|prev| prev == idx)
            && frame.wrapping_sub(state.last_scene_row_click_frame) <= LIST_DOUBLE_CLICK_FRAMES;
        state.last_scene_row_click_slot = Some(idx);
        state.last_scene_row_click_frame = frame;
        should_toggle || was_selected
    })
    .unwrap_or(false);

    select_node_slot(ctx, idx);
    crate::scripts_scene_editor_animation_rs::follow_player_selection(ctx);

    if !should_toggle {
        return;
    }

    let has_children = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.doc_text.is_empty() {
            false
        } else {
            let doc = cached_scene_doc_shared(&state.doc_text);
            scene_child_count(&doc, key) > 0
        }
    }).unwrap_or_default();
    if !has_children {
        return;
    }

    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if let Some(pos) = state
            .collapsed_scene_keys
            .iter()
            .position(|collapsed| *collapsed == key)
        {
            state.collapsed_scene_keys.remove(pos);
            state.log = "expand node".to_string();
            return true;
        }
        state.collapsed_scene_keys.push(key);
        state.log = "collapse node".to_string();
        true
    })
    .unwrap_or(false);
    if changed {
        refresh_scene_panel(ctx);
    }
}

pub fn toggle_scene_node_slot<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
) {
    let Some(key) = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.doc_text.is_empty() {
            None
        } else {
            let doc = cached_scene_doc_shared(&state.doc_text);
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
    }).unwrap_or_default() else {
        return;
    };
    let has_children = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.doc_text.is_empty() {
            false
        } else {
            let doc = cached_scene_doc_shared(&state.doc_text);
            scene_child_count(&doc, key) > 0
        }
    }).unwrap_or_default();
    if !has_children {
        return;
    }
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if let Some(pos) = state
            .collapsed_scene_keys
            .iter()
            .position(|collapsed| *collapsed == key)
        {
            state.collapsed_scene_keys.remove(pos);
            state.log = "expand node".to_string();
            return true;
        }
        state.collapsed_scene_keys.push(key);
        state.log = "collapse node".to_string();
        true
    })
    .unwrap_or(false);
    if changed {
        refresh_scene_panel(ctx);
    }
}

pub fn set_scene_node_slot_open<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
    open: bool,
) {
    let Some(key) = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.doc_text.is_empty() {
            None
        } else {
            let doc = cached_scene_doc_shared(&state.doc_text);
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
    }).unwrap_or_default() else {
        return;
    };
    let has_children = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.doc_text.is_empty() {
            false
        } else {
            let doc = cached_scene_doc_shared(&state.doc_text);
            scene_child_count(&doc, key) > 0
        }
    }).unwrap_or_default();
    if !has_children {
        return;
    }
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let collapsed = !open;
        if collapsed {
            if state.collapsed_scene_keys.contains(&key) {
                return false;
            }
            state.collapsed_scene_keys.push(key);
            state.log = "collapse node".to_string();
            return true;
        }
        if let Some(pos) = state
            .collapsed_scene_keys
            .iter()
            .position(|collapsed_key| *collapsed_key == key)
        {
            state.collapsed_scene_keys.remove(pos);
            state.log = "expand node".to_string();
            return true;
        }
        false
    })
    .unwrap_or(false);
    if changed {
        refresh_scene_panel(ctx);
    }
}

pub fn set_activity_mode<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, mode: &str) {
    let was_glb = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.activity_mode == "glb"
    }).unwrap_or_default();
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if mode == "scene" {
            state.activity_mode = "scene".to_string();
            state.sidebar_mode = "scene".to_string();
            state.anim_drawer_open = false;
        }
        if mode == "glb" {
            state.activity_mode = "glb".to_string();
            state.sidebar_mode = "files".to_string();
            state.anim_drawer_open = false;
            state.log = "glb view\nopen .glb from list".to_string();
        } else if mode == "files" {
            state.activity_mode = "scene".to_string();
            state.sidebar_mode = "files".to_string();
        } else if mode == "anim" {
            state.activity_mode = "scene".to_string();
            state.anim_drawer_open = true;
        }
    });
    if was_glb && (mode == "scene" || mode == "files") {
        // Leaving the glb viewer: restore the open scene's preview stage.
        rebuild_preview(ctx);
    }
    refresh_all(ctx);
}

pub fn update_scene_filter<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(text) = read_text_box(ctx, "scene_filter_box") else {
        return;
    };
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.scene_filter = text;
        state.focused_inspector_box = "scene_filter_box".to_string();
    });
    refresh_all(ctx);
}

pub fn clear_scene_filter<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.scene_filter.clear();
        state.focused_inspector_box.clear();
        state.log = "clear scene filter".to_string();
    });
    refresh_all(ctx);
}

pub fn expand_scene_tree_all<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.collapsed_scene_keys.clear();
        state.log = "expand scene tree".to_string();
    });
    refresh_all(ctx);
}

pub fn collapse_scene_tree_all<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let doc = cached_scene_doc_shared(&state.doc_text);
        let mut keys = doc
            .scene
            .nodes
            .iter()
            .filter(|node| scene_node_has_children(&doc, node.key.as_u32()))
            .map(|node| node.key.as_u32())
            .collect::<Vec<_>>();
        if let Some(selected) = state.selected_key {
            reveal_scene_key(&doc, selected, &mut keys);
        }
        state.collapsed_scene_keys = keys;
        state.log = "fold scene tree".to_string();
    });
    refresh_all(ctx);
}

pub fn copy_selected_node_path<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = "node path fail\nselect node".to_string();
            return;
        };
        if state.doc_text.is_empty() {
            state.log = "node path fail\nno open scene".to_string();
            return;
        }
        let doc = cached_scene_doc_shared(&state.doc_text);
        let Some(node) = cached_scene_node(&state.doc_text, key) else {
            state.log = "node path fail\nmissing node".to_string();
            return;
        };
        state.log = format!("node path\n{}", scene_node_path(&doc, node.key));
    });
    refresh_all(ctx);
}

pub fn scene_node_has_children(doc: &SceneDoc, key: u32) -> bool {
    doc.scene
        .nodes
        .iter()
        .any(|node| node.parent.map(|parent| parent.as_u32()) == Some(key))
}

pub fn reveal_scene_key(doc: &SceneDoc, key: u32, collapsed_keys: &mut Vec<u32>) {
    let mut cursor = Some(key);
    while let Some(current) = cursor {
        let parent = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == current)
            .and_then(|node| node.parent.map(|parent| parent.as_u32()));
        if let Some(parent_key) = parent
            && let Some(pos) = collapsed_keys.iter().position(|key| *key == parent_key)
        {
            collapsed_keys.remove(pos);
        }
        cursor = parent;
    }
}

pub fn update_file_filter<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(text) = read_text_box(ctx, "file_filter_box") else {
        return;
    };
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.file_filter = text;
        state.focused_inspector_box = "file_filter_box".to_string();
    });
    refresh_all(ctx);
}

pub fn set_sidebar_mode<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, mode: &str) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.sidebar_mode = mode.to_string();
        if state.activity_mode != "glb" {
            state.activity_mode = "scene".to_string();
        }
    });
    refresh_all(ctx);
}

pub fn set_anim_drawer<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, visible: bool) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.anim_drawer_open = visible;
        state.bottom_dock_open = visible;
        if visible {
            state.activity_mode = "scene".to_string();
        }
    });
    refresh_all(ctx);
}

pub fn toggle_bottom_dock<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    animation: bool,
) {
    let anim_opened = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        (state.bottom_dock_open, state.anim_drawer_open) = next_bottom_dock_state(
            state.bottom_dock_open,
            state.anim_drawer_open,
            animation,
        );
        if state.bottom_dock_open {
            state.activity_mode = "scene".to_string();
        }
        state.bottom_dock_open && state.anim_drawer_open
    })
    .unwrap_or(false);
    if anim_opened {
        // Land on the selected AnimationPlayer's clip when the dock is empty.
        crate::scripts_scene_editor_animation_rs::try_open_selected_player_clip(ctx);
    }
    refresh_all(ctx);
}

pub fn next_bottom_dock_state(open: bool, current_animation: bool, target_animation: bool) -> (bool, bool) {
    if open && current_animation == target_animation {
        (false, current_animation)
    } else {
        (true, target_animation)
    }
}

pub fn toggle_distraction_free<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.distraction_free = !state.distraction_free;
    });
    refresh_all(ctx);
    apply_viewport_canvas(ctx);
}

pub fn open_selected_node_asset_ref<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let path = with_state!(ctx.run, EditorState, ctx.id, |state| {
        let key = state.selected_key?;
        if state.doc_text.is_empty() {
            return None;
        }
        let doc = cached_scene_doc_shared(&state.doc_text);
        let node = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == key)?;
        let refs = selected_node_asset_refs(node)
            .into_iter()
            .filter_map(|line| line.split_once(": ").map(|(_, path)| path.to_string()))
            .collect::<Vec<_>>();
        if refs.is_empty() {
            return None;
        }
        let active = state.active_asset_path.as_str();
        let next_idx = refs
            .iter()
            .position(|path| {
                !active.is_empty() && base_res_asset_path(path) == base_res_asset_path(active)
            })
            .map(|idx| (idx + 1) % refs.len())
            .unwrap_or(0);
        refs.get(next_idx).cloned()
    }).unwrap_or_default();
    let Some(path) = path else {
        set_log(ctx, "open ref fail\nno asset ref");
        return;
    };
    open_asset_ref_path(ctx, &path);
}

pub fn select_node_using_active_asset<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.active_asset_path.is_empty() || state.active_asset_path.ends_with('/') {
            state.log = "find user fail\nselect asset file".to_string();
            return false;
        }
        if state.doc_text.is_empty() {
            state.log = "find user fail\nopen scene".to_string();
            return false;
        }
        let doc = cached_scene_doc_shared(&state.doc_text);
        let users = doc
            .scene
            .nodes
            .iter()
            .filter(|node| node_uses_asset_path(node, &state.active_asset_path))
            .collect::<Vec<_>>();
        if users.is_empty() {
            state.log = format!("find user\nnone for {}", state.active_asset_path);
            return false;
        };
        let next_idx = state
            .selected_key
            .and_then(|selected| {
                users
                    .iter()
                    .position(|node| node.key.as_u32() == selected)
                    .map(|idx| (idx + 1) % users.len())
            })
            .unwrap_or(0);
        let node = users[next_idx];
        let key = node.key.as_u32();
        state.selected_key = Some(key);
        state.sidebar_mode = "scene".to_string();
        state.activity_mode = "scene".to_string();
        state.scene_filter.clear();
        if let Some(mode) = viewport_mode_for_node_type(node.data.node_type) {
            state.viewport_mode = mode.to_string();
        }
        state.log = format!(
            "find user {}/{}\n{}",
            next_idx + 1,
            users.len(),
            doc.scene.key_name_or_id(SceneKey::new(key))
        );
        true
    })
    .unwrap_or(false);
    if changed {
        refresh_all(ctx);
    }
}

pub fn selected_node_asset_refs(node: &SceneNodeEntry) -> Vec<String> {
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
    for field in perro_scene::scene_node_asset_fields(node.data.node_type) {
        if let Some(path) = scene_field_value_text(&node.data, field.name)
            && path.starts_with("res://")
        {
            out.push(format!("{}: {path}", field.name));
        }
    }
    out
}

pub fn node_uses_asset_path(node: &SceneNodeEntry, asset_path: &str) -> bool {
    let base = base_res_asset_path(asset_path);
    selected_node_asset_refs(node).into_iter().any(|line| {
        line.split_once(": ")
            .map(|(_, path)| path == asset_path || base_res_asset_path(path) == base)
            .unwrap_or(false)
    })
}

pub fn selected_node_asset_ref_path(node: &SceneNodeEntry) -> Option<String> {
    selected_node_asset_refs(node)
        .into_iter()
        .find_map(|line| line.split_once(": ").map(|(_, path)| path.to_string()))
}

pub fn open_asset_ref_path<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, path: &str) {
    let base = base_res_asset_path(path);
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.active_asset_path = base.clone();
        state.sidebar_mode = "files".to_string();
        state.activity_mode = "scene".to_string();
        state.file_scope = parent_res_folder(&base);
        reveal_file_path_in_tree(state, &base);
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

pub fn base_res_asset_path(path: &str) -> String {
    let Some(rest) = path.strip_prefix("res://") else {
        return path.to_string();
    };
    match rest.find(':') {
        Some(idx) => format!("res://{}", &rest[..idx]),
        None => path.to_string(),
    }
}

pub fn use_active_asset_on_selected_node<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let asset_path = state.active_asset_path.clone();
        if asset_path.is_empty() || asset_path.ends_with('/') {
            state.log = "use asset fail\nselect asset file".to_string();
            return false;
        }
        if asset_path.ends_with(".scn") {
            state.log = "use asset fail\ninstance scene instead".to_string();
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
        let mut doc = cached_scene_doc(&state.doc_text);
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
        set_state_scene_doc(state, &doc);
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

pub fn make_node_from_active_asset<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
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
        let mut doc = cached_scene_doc(&state.doc_text);
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
        set_state_scene_doc(state, &doc);
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

pub fn asset_binding_for_node(
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
    let node_type = perro_scene::NodeType::from_str(node_type).ok()?;
    let path_kind = asset_kind_for_path(path)?;
    for field in perro_scene::scene_node_asset_fields(node_type) {
        let perro_scene::NodeFieldType::Asset(field_kind) = field.ty else {
            continue;
        };
        if field_kind == path_kind
            || (path_kind == perro_scene::SceneAssetKind::Model
                && field_kind == perro_scene::SceneAssetKind::Mesh)
        {
            return Some((
                field.name,
                asset_field_value(path, field_kind, glb_mesh_index),
            ));
        }
    }
    None
}

pub fn asset_node_template(
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
    if path.ends_with(".panimtree") {
        return Some(("AnimationTree", Some(("tree", path.to_string()))));
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
    if path.ends_with(".ppart") {
        return Some(("ParticleEmitter3D", Some(("profile", path.to_string()))));
    }
    if path.ends_with(".ptileset") {
        return Some(("TileMap2D", Some(("tileset", path.to_string()))));
    }
    if path.ends_with(".pskel") || path.ends_with(".pskel2d") {
        return Some(("Skeleton2D", Some(("skeleton", path.to_string()))));
    }
    if path.ends_with(".pskel3d") {
        return Some(("Skeleton3D", Some(("skeleton", path.to_string()))));
    }
    if path.ends_with(".uistyle") {
        return Some(("UiPanel", Some(("style", path.to_string()))));
    }
    if editor_files::kind_label(path) == "image" {
        return Some(("Sprite2D", Some(("texture", path.to_string()))));
    }
    None
}

pub fn asset_kind_for_path(path: &str) -> Option<perro_scene::SceneAssetKind> {
    if editor_files::kind_label(path) == "image" {
        return Some(perro_scene::SceneAssetKind::Texture);
    }
    if path.ends_with(".glb") || path.ends_with(".gltf") {
        return Some(perro_scene::SceneAssetKind::Model);
    }
    if path.ends_with(".pmesh") || path.ends_with(".obj") || path.ends_with(".fbx") {
        return Some(perro_scene::SceneAssetKind::Mesh);
    }
    if path.ends_with(".pmat") {
        return Some(perro_scene::SceneAssetKind::Material);
    }
    if path.ends_with(".panim") {
        return Some(perro_scene::SceneAssetKind::Animation);
    }
    if path.ends_with(".panimtree") {
        return Some(perro_scene::SceneAssetKind::AnimationTree);
    }
    if path.ends_with(".pskel") || path.ends_with(".pskel2d") || path.ends_with(".pskel3d") {
        return Some(perro_scene::SceneAssetKind::Skeleton);
    }
    if path.ends_with(".ppart") {
        return Some(perro_scene::SceneAssetKind::ParticleProfile);
    }
    if path.ends_with(".ptileset") {
        return Some(perro_scene::SceneAssetKind::TileSet);
    }
    if path.ends_with(".uistyle") {
        return Some(perro_scene::SceneAssetKind::UiStyle);
    }
    None
}

pub fn asset_field_value(
    path: &str,
    kind: perro_scene::SceneAssetKind,
    glb_mesh_index: usize,
) -> String {
    match kind {
        perro_scene::SceneAssetKind::Mesh if is_gltf_path(path) => {
            format!("{path}:mesh[{glb_mesh_index}]")
        }
        _ => path.to_string(),
    }
}

pub fn add_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, node_type_name: &str) {
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
        let mut doc = cached_scene_doc(&state.doc_text);
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
                        format!(
                            "{}:mat[{}]",
                            state.active_asset_path, state.active_glb_mat_index
                        ),
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
        set_state_scene_doc(state, &doc);
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

pub fn add_node_from_picker<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, row: usize) {
    let node_type = with_state!(ctx.run, EditorState, ctx.id, |state| {
        exact_picker_node_type(&state.node_picker_filter).or_else(|| {
            picker_node_types(state, &state.node_picker_filter)
                .get(state.node_picker_offset + row)
                .copied()
        })
    }).unwrap_or_default();
    if let Some(node_type) = node_type {
        add_node(ctx, node_type.name());
    }
}

pub fn exact_picker_node_type(filter: &str) -> Option<perro_scene::NodeType> {
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

pub fn add_node_parent(
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

pub fn apply_spawn_position(data: &mut SceneNodeData, state: &EditorState) {
    if data.node_type.is_a(perro_scene::NodeType::Node2D) {
        set_scene_vec2(data, "position", Vector2::new(state.cam2_x, state.cam2_y));
    } else if data.node_type.is_a(perro_scene::NodeType::Node3D) {
        set_scene_vec3(data, "position", viewport_spawn_3d(state));
    }
}

pub fn viewport_spawn_3d(state: &EditorState) -> Vector3 {
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

pub fn update_node_picker_filter<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(text) = read_text_box(ctx, "add_node_search_box") else {
        return;
    };
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.node_picker_filter = text;
        state.node_picker_offset = 0;
        state.focused_inspector_box = "add_node_search_box".to_string();
    });
    refresh_all(ctx);
}

pub fn set_node_picker_filter_text<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    text: &str,
) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.node_picker_filter = text.to_string();
        state.node_picker_offset = 0;
    });
    refresh_all(ctx);
}

pub fn shift_node_picker<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, dir: isize) {
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

pub fn set_node_picker_edge<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, last: bool) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let max_start = picker_node_types(state, &state.node_picker_filter)
            .len()
            .saturating_sub(MAX_NODE_PICKER_ROWS);
        state.node_picker_offset = if last { max_start } else { 0 };
    });
    refresh_all(ctx);
}

pub fn nudge_node_picker<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, dir: isize) {
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

pub fn picker_node_types(state: &EditorState, filter: &str) -> Vec<perro_scene::NodeType> {
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
            if Some(node_type.name()) == asset_node {
                0
            } else {
                1
            },
            parent_node_rank(parent_kind, *node_type),
            viewport_node_rank(state, *node_type),
            recent_node_rank(state, node_type.name()).unwrap_or(usize::MAX),
            node_type_rank(*node_type),
            node_type.name(),
        )
    });
    out
}

pub fn picker_node_row(state: &EditorState, node_type: perro_scene::NodeType) -> String {
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
    format!(
        "{} {}{}",
        node_type_icon(node_type),
        node_type.name(),
        badges
    )
}

pub fn exact_node_rank(filter: &NodePickerFilter, node_type: perro_scene::NodeType) -> u8 {
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

pub fn push_recent_node_type(state: &mut EditorState, node_type: &str) {
    state.recent_node_types.retain(|item| item != node_type);
    state.recent_node_types.insert(0, node_type.to_string());
    state.recent_node_types.truncate(8);
}

pub fn recent_node_rank(state: &EditorState, node_type: &str) -> Option<usize> {
    state
        .recent_node_types
        .iter()
        .position(|item| item == node_type)
}

pub fn active_asset_node_type(state: &EditorState) -> Option<&'static str> {
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

pub fn picker_parent_node_kind(state: &EditorState) -> Option<&'static str> {
    if state.doc_text.is_empty() {
        return None;
    }
    let doc = cached_scene_doc_shared(&state.doc_text);
    let key = add_node_parent(&doc, state.selected_key, state.add_node_as_sibling)?;
    doc.scene
        .nodes
        .iter()
        .find(|node| node.key == key)
        .map(|node| node_type_kind(node.data.node_type))
}

pub fn node_type_kind(node_type: perro_scene::NodeType) -> &'static str {
    if node_type.is_a(perro_scene::NodeType::Node2D) {
        "2D"
    } else if node_type.is_a(perro_scene::NodeType::Node3D) {
        "3D"
    } else if node_type.is_a(perro_scene::NodeType::UiNode) {
        "UI"
    } else {
        "Node"
    }
}

pub fn parent_node_rank(kind: Option<&str>, node_type: perro_scene::NodeType) -> u8 {
    match kind {
        Some("2D") if node_type.is_a(perro_scene::NodeType::Node2D) => 0,
        Some("3D") if node_type.is_a(perro_scene::NodeType::Node3D) => 0,
        Some("UI") if node_type.is_a(perro_scene::NodeType::UiNode) => 0,
        Some("Node") => 0,
        Some(_) => 1,
        None => 0,
    }
}

pub fn node_type_rank(node_type: perro_scene::NodeType) -> u8 {
    match node_type.name() {
        "Node2D" => 0,
        "Sprite2D" => 1,
        "Camera2D" => 2,
        "Node3D" => 3,
        "MeshInstance3D" => 4,
        "Camera3D" => 5,
        "AnimationPlayer" => 6,
        "UiPanel" | "UiButton" | "UiDropdown" | "UiCheckbox" | "UiColorPicker" | "UiLabel" => 7,
        _ if node_type.is_a(perro_scene::NodeType::Node2D) => 20,
        _ if node_type.is_a(perro_scene::NodeType::Node3D) => 30,
        _ if node_type.is_a(perro_scene::NodeType::UiNode) => 40,
        _ => 90,
    }
}

pub struct NodePickerFilter {
    pub text: Vec<String>,
    pub tags: Vec<String>,
}

impl NodePickerFilter {
    pub fn parse(raw: &str) -> Self {
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

    pub fn is_empty(&self) -> bool {
        self.text.is_empty() && self.tags.is_empty()
    }
}

pub fn node_type_matches_picker_filter(
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

pub fn node_type_has_picker_tag(node_type: perro_scene::NodeType, tag: &str) -> bool {
    match tag {
        "2d" => node_type.is_a(perro_scene::NodeType::Node2D),
        "3d" => node_type.is_a(perro_scene::NodeType::Node3D),
        "ui" => node_type.is_a(perro_scene::NodeType::UiNode),
        "mesh" => {
            node_type.name().contains("Mesh") || node_type_search_text(node_type).contains("mesh")
        }
        "anim" => {
            node_type.name().contains("Animation")
                || node_type_search_text(node_type).contains("anim")
        }
        "phys" | "physics" => node_type_search_text(node_type).contains("physics"),
        "light" => node_type_search_text(node_type).contains("light"),
        "audio" => node_type_search_text(node_type).contains("audio"),
        "cam" | "camera" => node_type_search_text(node_type).contains("camera"),
        "recent" => false,
        _ => node_type_search_text(node_type).contains(tag),
    }
}

pub fn viewport_node_rank(state: &EditorState, node_type: perro_scene::NodeType) -> u8 {
    match state.viewport_mode.as_str() {
        "2D" if node_type.is_a(perro_scene::NodeType::Node2D) => 0,
        "3D" if node_type.is_a(perro_scene::NodeType::Node3D) => 0,
        "UI" if node_type.is_a(perro_scene::NodeType::UiNode) => 0,
        _ => 1,
    }
}

pub fn node_type_search_text(node_type: perro_scene::NodeType) -> String {
    let aliases = match node_type.name() {
        "Sprite2D" => " image texture png 2d visual",
        "MeshInstance3D" => " mesh model glb gltf pmesh 3d visual",
        "AnimationPlayer" => " anim animation clip panim timeline",
        "Camera2D" | "Camera3D" => " camera view viewport",
        "UiPanel" | "UiButton" | "UiDropdown" | "UiCheckbox" | "UiColorPicker" | "UiLabel" => {
            " ui control hud menu color picker"
        }
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

pub fn default_fields(node_type: perro_scene::NodeType) -> Vec<(SceneFieldName, SceneValue)> {
    perro_scene::scene_default_fields(node_type)
}

pub fn save_active_scene<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let saved = save_active_scene_to_disk(ctx, false);
    if saved {
        rebuild_preview(ctx);
    }
    refresh_all(ctx);
}

pub fn save_active_scene_to_disk<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    quiet: bool,
) -> bool {
    let save = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        capture_active_scene_session(state);
        let path = state.open_paths.get(state.active_open).cloned();
        let root = state.project_root.clone();
        let doc_text = state
            .scene_sessions
            .get(state.active_open)
            .map(|session| session.doc_text.clone())
            .unwrap_or_else(|| state.doc_text.clone());
        (root, path, doc_text)
    })
    .unwrap_or_default();
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
                if let Some(session) = state.scene_sessions.get_mut(state.active_open) {
                    session.dirty = false;
                    session.doc_text.clone_from(&text);
                }
                state.doc_text.clone_from(&text);
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

pub fn save_all_scenes<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) -> bool {
    let (root, sessions, dirty_paths) =
        with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            capture_active_scene_session(state);
            (
                state.project_root.clone(),
                state.scene_sessions.clone(),
                state.dirty_scene_paths.clone(),
            )
        })
        .unwrap_or_default();
    if sessions.is_empty() || dirty_paths.is_empty() {
        set_log(ctx, "save all\nnothing dirty");
        refresh_all(ctx);
        return true;
    }

    let mut saved = Vec::new();
    let mut failed = Vec::new();
    for path in dirty_paths.iter() {
        let Some(session) = sessions.iter().find(|session| &session.path == path) else {
            failed.push(format!("{path}: missing editor session"));
            continue;
        };
        let mut doc = SceneDoc::parse(&session.doc_text);
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
            if let Some(session) = state
                .scene_sessions
                .iter_mut()
                .find(|session| &session.path == path)
            {
                session.dirty = false;
            }
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
            format!(
                "save all\nsaved={}\nfail={}",
                saved.len(),
                failed.join("\n")
            )
        };
    });
    rebuild_preview(ctx);
    refresh_all(ctx);
    failed.is_empty()
}

pub fn delete_selected_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = "delete node fail\nselect node".to_string();
            return false;
        };
        if state.doc_text.is_empty() {
            state.log = "delete node fail\nno open scene".to_string();
            return false;
        }
        let mut doc = cached_scene_doc(&state.doc_text);
        if doc.scene.root.map(|root| root.as_u32()) == Some(key) {
            state.log = "delete node fail\nroot node".to_string();
            return false;
        }
        let Some(target) = cached_scene_node(&state.doc_text, key) else {
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
        set_state_scene_doc(state, &doc);
        state.selected_key = parent_key
            .filter(|parent| {
                doc.scene
                    .nodes
                    .iter()
                    .any(|node| node.key.as_u32() == *parent)
            })
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

pub fn toggle_selected_visible<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let mut next_visible: Option<bool> = None;
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = "visible fail\nselect node".to_string();
            return false;
        };
        if state.doc_text.is_empty() {
            state.log = "visible fail\nno open scene".to_string();
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
            state.log = "visible fail\nmissing node".to_string();
            return false;
        };
        let visible = scene_field_bool(&node.data, "visible").unwrap_or(true);
        let next = !visible;
        next_visible = Some(next);
        set_scene_bool(&mut node.data, "visible", next);
        set_state_scene_doc(state, &doc);
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
    if changed
        && next_visible.is_none_or(|value| {
            !sync_selected_preview_field(ctx, "visible", &SceneValue::Bool(value))
        })
    {
        rebuild_preview(ctx);
    }
    refresh_all(ctx);
}

pub fn clear_selected_node_asset_refs<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = "clear refs fail\nselect node".to_string();
            return false;
        };
        if state.doc_text.is_empty() {
            state.log = "clear refs fail\nno open scene".to_string();
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
            state.log = "clear refs fail\nmissing node".to_string();
            return false;
        };
        let mut count = 0;
        if node.script.is_some() {
            node.script = None;
            node.clear_script = true;
            count += 1;
        }
        if node.root_of.is_some() {
            node.root_of = None;
            count += 1;
        }
        let asset_fields = perro_scene::scene_node_asset_fields(node.data.node_type)
            .into_iter()
            .map(|field| field.name.to_string())
            .collect::<Vec<_>>();
        let before = node.data.fields.len();
        node.data
            .fields
            .to_mut()
            .retain(|(field, _)| !asset_fields.iter().any(|name| field.as_ref() == name));
        count += before.saturating_sub(node.data.fields.len());
        if count == 0 {
            state.log = "clear refs\nnone".to_string();
            return false;
        }
        doc.normalize_links();
        set_state_scene_doc(state, &doc);
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = format!("clear refs\nrm {count}");
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
    }
    refresh_all(ctx);
}

pub fn duplicate_selected_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = "duplicate node fail\nselect node".to_string();
            return false;
        };
        if state.doc_text.is_empty() {
            state.log = "duplicate node fail\nno open scene".to_string();
            return false;
        }
        let mut doc = cached_scene_doc(&state.doc_text);
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
            if let Some(parent) = node.parent
                && let Some(new_parent) = mapped_scene_key(&map, parent.as_u32())
            {
                node.parent = Some(SceneKey::new(new_parent));
            }
            if old_key == key {
                offset_duplicated_node(&mut node.data);
            }
            node.children = Cow::Owned(Vec::new());
            doc.scene.nodes.to_mut().push(node);
        }
        doc.normalize_links();
        set_state_scene_doc(state, &doc);
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

pub fn copy_selected_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let copied = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = "copy node fail\nselect node".to_string();
            return false;
        };
        if state.doc_text.is_empty() {
            state.log = "copy node fail\nno open scene".to_string();
            return false;
        }
        let doc = cached_scene_doc_shared(&state.doc_text);
        let Some(node) = cached_scene_node(&state.doc_text, key) else {
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

pub fn paste_copied_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(source_key) = state.copied_node_key else {
            state.log = "paste node fail\ncopy node first".to_string();
            return false;
        };
        if state.doc_text.is_empty() {
            state.log = "paste node fail\nno open scene".to_string();
            return false;
        }
        let mut doc = cached_scene_doc(&state.doc_text);
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
            } else if let Some(parent) = node.parent
                && let Some(new_parent) = mapped_scene_key(&map, parent.as_u32())
            {
                node.parent = Some(SceneKey::new(new_parent));
            }
            node.children = Cow::Owned(Vec::new());
            doc.scene.nodes.to_mut().push(node);
        }
        doc.normalize_links();
        set_state_scene_doc(state, &doc);
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

pub fn move_selected_node_order<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    dir: isize,
) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = "move node fail\nselect node".to_string();
            return false;
        };
        if state.doc_text.is_empty() {
            state.log = "move node fail\nno open scene".to_string();
            return false;
        }
        let mut doc = cached_scene_doc(&state.doc_text);
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
        set_state_scene_doc(state, &doc);
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

pub fn reparent_selected_node<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    dir: isize,
) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = "reparent fail\nselect node".to_string();
            return false;
        };
        if state.doc_text.is_empty() {
            state.log = "reparent fail\nno open scene".to_string();
            return false;
        }
        let mut doc = cached_scene_doc(&state.doc_text);
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
        set_state_scene_doc(state, &doc);
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

pub fn collect_scene_subtree_keys(doc: &SceneDoc, root_key: u32) -> Vec<u32> {
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

pub fn mapped_scene_key(map: &[(u32, u32)], key: u32) -> Option<u32> {
    map.iter()
        .find(|(old_key, _)| *old_key == key)
        .map(|(_, new_key)| *new_key)
}

pub fn offset_duplicated_node(data: &mut SceneNodeData) {
    if data.node_type.is_a(perro_scene::NodeType::Node3D) {
        let pos = find_vec3_value(data, "position").unwrap_or(Vector3::ZERO);
        set_scene_vec3(data, "position", pos + Vector3::new(1.0, 0.0, 1.0));
    } else if data.node_type.is_a(perro_scene::NodeType::Node2D) {
        let pos = find_vec2_value(data, "position").unwrap_or(Vector2::ZERO);
        set_scene_vec2(data, "position", pos + Vector2::new(16.0, -16.0));
    }
}
