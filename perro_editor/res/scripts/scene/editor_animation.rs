use crate::scripts_app_editor_app_rs as editor_app;
use crate::scripts_app_editor_manager_rs as editor_manager;
use crate::scripts_app_editor_project_rs as editor_project;
use crate::scripts_assets_editor_assets_rs::*;
use crate::scripts_assets_editor_file_watch_rs as editor_file_watch;
use crate::scripts_assets_editor_files_rs as editor_files;
use crate::scripts_editor_main_rs::{
    EditorState, FILE_WATCH_INTERVAL_FRAMES, MAX_ANIM_MARKERS, MAX_ANIM_TRACKS, MAX_FILES,
    MAX_INSPECTOR_PICKER_ROWS, MAX_NODE_PICKER_ROWS, MAX_NODES, MAX_RECENT, MAX_TABS,
    RECENT_PROJECTS_PATH, cached_scene_doc, cached_scene_doc_shared, set_state_scene_doc,
};
use crate::scripts_scene_editor_gizmos_rs as editor_gizmos;
use crate::scripts_scene_editor_panim_rs as panim;
use crate::scripts_ui_theme_rs as theme;
use crate::scripts_scene_editor_nav_rs::*;
use crate::scripts_scene_editor_nodes_rs::*;
use crate::scripts_scene_editor_scene_deps_rs as editor_scene_deps;
use crate::scripts_scene_editor_scene_rs as editor_scene;
use crate::scripts_scene_editor_viewport_rs::*;
use crate::scripts_ui_editor_inspector_values_rs::*;
use crate::scripts_ui_editor_ui_rs::*;
use crate::scripts_ui_editor_view_rs as editor_view;
use perro_api::prelude::*;
use perro_api::scene::{
    Parser, SceneDoc, SceneFieldName, SceneKey, SceneNodeData, SceneNodeEntry, SceneValue,
    SceneValueKey,
};
use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
pub fn create_animation_for_selected_player<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
) {
    let request = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = "anim create fail\nselect AnimationPlayer".to_string();
            return None;
        };
        let mut doc = cached_scene_doc(&state.doc_text);
        let node_index = doc
            .scene
            .nodes
            .iter()
            .position(|node| node.key.as_u32() == key)?;
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
        set_state_scene_doc(state, &doc);
        state.dirty = true;
        state.activity_mode = "scene".to_string();
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
                state.bottom_dock_open = true;
                load_anim_text_into_state(state, &anim_path, text.clone());
                state.log = format!("create animation\n{}", editor_files::rel_label(&anim_path));
            });
            refresh_all(ctx);
        }
        Err(err) => set_log(ctx, &format!("anim write fail\n{anim_path}\n{err}")),
    }
}

pub fn add_track_for_selected_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let request = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(player_key) = state.active_anim_player_key else {
            state.log = "track add fail\nselect AnimationPlayer first".to_string();
            return None;
        };
        let Some(target_key) = state.selected_key else {
            state.log = "track add fail\nselect target node".to_string();
            return None;
        };
        let mut doc = cached_scene_doc(&state.doc_text);
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
        set_state_scene_doc(state, &doc);
        state.dirty = true;
        state.activity_mode = "scene".to_string();
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

pub fn default_animation_panim(animation_name: &str) -> String {
    format!(
        "[Animation]\nname = \"{animation_name}\"\nfps = 60\ndefault_interp = \"interpolate\"\ndefault_ease = \"linear\"\n[/Animation]\n\n[Objects]\nTarget = Node3D\n[/Objects]\n\n[Frame0]\n@Target {{\n    position = (0, 0, 0)\n}}\n[/Frame0]\n\n[Frame30]\n@Target {{\n    position = (2, 0, 0)\n}}\n[/Frame30]\n"
    )
}

pub fn unique_panim_object_name(node_name: &str, anim_path: &str, project_root: &str) -> String {
    let base = sanitize_panim_ident(node_name);
    let existing = if anim_path.is_empty() || anim_path == "-" {
        String::new()
    } else {
        FileMod::load_string(res_to_abs(project_root, anim_path)).unwrap_or_default()
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

pub fn sanitize_panim_ident(raw: &str) -> String {
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

pub fn panim_has_object(text: &str, object: &str) -> bool {
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

pub fn add_panim_track_text(text: &str, object: &str, node_type: &str) -> String {
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

pub fn insert_panim_object(text: &str, object: &str, node_type: &str) -> String {
    if let Some(pos) = text.find("[/Objects]") {
        let mut out = String::with_capacity(text.len() + object.len() + node_type.len() + 8);
        out.push_str(&text[..pos]);
        out.push_str(&format!("{object} = {node_type}\n"));
        out.push_str(&text[pos..]);
        return out;
    }
    format!("{text}\n[Objects]\n{object} = {node_type}\n[/Objects]\n")
}

pub fn panim_frame_has_object(text: &str, frame: u32, object: &str) -> bool {
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

pub fn insert_panim_frame0_object(text: &str, object: &str) -> String {
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

pub fn sanitize_file_stem(text: &str) -> String {
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

pub fn unique_res_animation_path(project_root: &str, stem: &str) -> String {
    for idx in 0..1000 {
        let suffix = if idx == 0 {
            String::new()
        } else {
            format!("_{idx}")
        };
        let path = format!("{}animations/{stem}{suffix}.panim", "res://");
        if !Path::new(&res_to_abs(project_root, &path)).exists() {
            return path;
        }
    }
    format!("{}animations/{stem}_x.panim", "res://")
}

pub fn edit_selected_transform<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    field: &str,
    text_box: &str,
) {
    let values = read_component_values(ctx, text_box)
        .or_else(|| read_text_box(ctx, text_box).and_then(|text| parse_number_list(&text)));
    let Some(values) = values else {
        set_log(ctx, "inspector edit fail\nbad number list");
        return;
    };
    let preview_value = match values.as_slice() {
        [x] => Some(SceneValue::F32(*x)),
        [x, y] => Some(SceneValue::Vec2 { x: *x, y: *y }),
        [x, y, z] => Some(SceneValue::Vec3 {
            x: *x,
            y: *y,
            z: *z,
        }),
        [x, y, z, w] => Some(SceneValue::Vec4 {
            x: *x,
            y: *y,
            z: *z,
            w: *w,
        }),
        _ => None,
    };
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
        if field == "rotation_deg" {
            node.data
                .fields
                .to_mut()
                .retain(|(name, _)| name.as_ref() != "rotation");
        } else if field == "rotation" {
            node.data
                .fields
                .to_mut()
                .retain(|(name, _)| name.as_ref() != "rotation_deg");
        }
        match values.as_slice() {
            [x] => set_scene_f32(&mut node.data, field, *x),
            [x, y] => set_scene_vec2(&mut node.data, field, Vector2::new(*x, *y)),
            [x, y, z] => set_scene_vec3(&mut node.data, field, Vector3::new(*x, *y, *z)),
            [x, y, z, w] => set_scene_vec4(&mut node.data, field, *x, *y, *z, *w),
            _ => return false,
        }
        set_state_scene_doc(state, &doc);
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
        if preview_value
            .as_ref()
            .is_none_or(|value| !sync_selected_preview_field(ctx, field, value))
        {
            rebuild_preview(ctx);
        }
        refresh_all(ctx);
    }
}

pub fn edit_selected_rotation<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let field = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.inspector_rotation_mode == "euler" {
            "rotation_deg"
        } else {
            "rotation"
        }
    });
    edit_selected_transform(ctx, field, "inspector_rotation_box");
}

pub fn set_inspector_rotation_mode<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    mode: &str,
) {
    let mode = if mode == "euler" { "euler" } else { "quat" };
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.inspector_rotation_mode = mode.to_string();
        state.focused_inspector_box.clear();
    });
    refresh_all(ctx);
}

pub fn set_inspector_quat_mode_from_dropdown<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
) {
    let Some(value) = read_dropdown_value(ctx, &format!("inspector_var_{idx}_quat_mode")) else {
        return;
    };
    let mode = if value.eq_ignore_ascii_case("euler") {
        "euler"
    } else {
        "quat"
    };
    set_inspector_rotation_mode(ctx, mode);
}

pub fn read_component_values<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    text_box: &str,
) -> Option<Vec<f32>> {
    let prefix = text_box.strip_suffix("_box")?;
    let mut values = Vec::new();
    for idx in 0..4 {
        let id = format!("{prefix}_{idx}_box");
        let Some(text) = read_text_box(ctx, &id) else {
            break;
        };
        let trimmed = text.trim();
        if trimmed.is_empty() {
            break;
        }
        values.push(trimmed.parse::<f32>().ok()?);
    }
    (!values.is_empty()).then_some(values)
}

pub fn reset_selected_transform<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
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
        if node.data.node_type.is_a(perro_scene::NodeType::Node3D) {
            set_scene_vec3(&mut node.data, "position", Vector3::ZERO);
            set_scene_vec3(&mut node.data, "rotation", Vector3::ZERO);
            set_scene_vec3(&mut node.data, "scale", Vector3::ONE);
        } else if node.data.node_type.is_a(perro_scene::NodeType::Node2D) {
            set_scene_vec2(&mut node.data, "position", Vector2::ZERO);
            set_scene_f32(&mut node.data, "rotation", 0.0);
            set_scene_vec2(&mut node.data, "scale", Vector2::ONE);
        } else if node.data.node_type.is_a(perro_scene::NodeType::UiNode) {
            set_scene_vec2(&mut node.data, "translation_ratio", Vector2::ZERO);
            set_scene_f32(&mut node.data, "rotation", 0.0);
        } else {
            state.log = "reset xform skip\nselect spatial/ui node".to_string();
            return false;
        }
        set_state_scene_doc(state, &doc);
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
        let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
            state.viewport_mode.clone()
        });
        let fields: &[&str] = if mode == "3D" || mode == "2D" {
            &["position", "rotation", "scale"]
        } else {
            &["translation_ratio", "rotation"]
        };
        if !sync_selected_preview_doc_fields(ctx, fields) {
            rebuild_preview(ctx);
        }
        refresh_all(ctx);
    }
}

pub fn nudge_selected_node<API: ScriptAPI + ?Sized>(
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
        } else if node.data.node_type.is_a(perro_scene::NodeType::UiNode) {
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
        set_state_scene_doc(state, &doc);
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
        let mode = with_state!(ctx.run, EditorState, ctx.id, |state| {
            state.viewport_mode.clone()
        });
        let field = if mode == "UI" {
            "translation_ratio"
        } else {
            "position"
        };
        if !sync_selected_preview_doc_fields(ctx, &[field]) {
            rebuild_preview(ctx);
        }
        refresh_all(ctx);
    }
}

pub fn rename_selected_node<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
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
        let mut doc = cached_scene_doc(&state.doc_text);
        let idx = key as usize;
        if idx >= doc.scene.key_names.len() {
            state.log = "rename fail\nbad key".to_string();
            return false;
        }
        let scene_key = SceneKey::new(key);
        let current = doc.scene.key_name_or_id(scene_key).to_string();
        let next = unique_renamed_node_name(&doc, scene_key, &requested);
        if current == next {
            return false;
        }
        doc.scene.key_names.to_mut()[idx] = Cow::Owned(next.clone());
        if let Some(node) = doc
            .scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key == scene_key)
        {
            node.name = None;
        }
        let refs = rewrite_node_refs_in_doc(&mut doc, &current, &next);
        set_state_scene_doc(state, &doc);
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = format!("rename node\n{current} -> {next}\nrefs={refs}");
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
}

pub fn unique_renamed_node_name(doc: &SceneDoc, key: SceneKey, requested: &str) -> String {
    let name_free = |name: &str| {
        doc.scene
            .key_names
            .iter()
            .enumerate()
            .all(|(idx, item)| idx == key.as_usize() || item.as_ref() != name)
    };
    if name_free(requested) {
        return requested.to_string();
    }
    for idx in 1..1000 {
        let name = format!("{requested}{idx}");
        if name_free(&name) {
            return name;
        }
    }
    format!("{requested}_x")
}

pub fn rewrite_node_refs_in_doc(doc: &mut SceneDoc, old_name: &str, new_name: &str) -> usize {
    let mut changed = 0;
    for node in doc.scene.nodes.to_mut().iter_mut() {
        changed += rewrite_node_refs_in_data(&mut node.data, old_name, new_name);
        for (_field, value) in node.script_vars.to_mut().iter_mut() {
            changed += rewrite_node_refs_in_value(value, old_name, new_name);
        }
    }
    changed
}

pub fn rewrite_node_refs_in_data(
    data: &mut SceneNodeData,
    old_name: &str,
    new_name: &str,
) -> usize {
    let mut changed = 0;
    for (_field, value) in data.fields.to_mut().iter_mut() {
        changed += rewrite_node_refs_in_value(value, old_name, new_name);
    }
    if let Some(base) = data.base.as_mut() {
        match base {
            perro_scene::SceneNodeDataBase::Borrowed(_) => {}
            perro_scene::SceneNodeDataBase::Owned(base) => {
                changed += rewrite_node_refs_in_data(base, old_name, new_name);
            }
        }
    }
    changed
}

pub fn rewrite_node_refs_in_value(value: &mut SceneValue, old_name: &str, new_name: &str) -> usize {
    match value {
        SceneValue::Key(key) if key.as_ref() == old_name => {
            *key = SceneValueKey::from(new_name.to_string());
            1
        }
        SceneValue::Key(key) if key.as_ref() == format!("@{old_name}") => {
            *key = SceneValueKey::from(format!("@{new_name}"));
            1
        }
        SceneValue::Object(fields) => fields
            .to_mut()
            .iter_mut()
            .map(|(_field, value)| rewrite_node_refs_in_value(value, old_name, new_name))
            .sum(),
        SceneValue::Array(values) => values
            .to_mut()
            .iter_mut()
            .map(|value| rewrite_node_refs_in_value(value, old_name, new_name))
            .sum(),
        _ => 0,
    }
}

#[cfg(test)]
mod node_rename_tests {
    use super::*;

    #[test]
    fn rename_key_drops_display_name_override_and_rewrites_refs() {
        let mut doc = rename_test_doc();
        let key = SceneKey::new(1);
        let next = unique_renamed_node_name(&doc, key, "MainCam");
        doc.scene.key_names.to_mut()[key.as_usize()] = Cow::Owned(next.clone());
        doc.scene
            .nodes
            .to_mut()
            .iter_mut()
            .find(|node| node.key == key)
            .expect("camera node")
            .name = None;

        let refs = rewrite_node_refs_in_doc(&mut doc, "Camera", &next);

        assert_eq!(refs, 5);
        assert_eq!(doc.scene.key_name(key), Some("MainCam"));
        assert!(
            doc.scene
                .nodes
                .iter()
                .find(|node| node.key == key)
                .expect("camera node")
                .name
                .is_none()
        );
        assert_eq!(count_doc_key_refs(&doc, "Camera"), 0);
        assert_eq!(count_doc_key_refs(&doc, "MainCam"), 5);
        assert_eq!(count_doc_key_refs(&doc, "Other"), 1);
    }

    #[test]
    fn rename_conflict_uses_plain_numeric_suffix() {
        let mut doc = rename_test_doc();
        doc.scene.key_names.to_mut()[2] = Cow::Borrowed("MainCam");
        doc.scene.key_names.to_mut()[3] = Cow::Borrowed("MainCam1");
        let key = SceneKey::new(1);

        assert_eq!(unique_renamed_node_name(&doc, key, "MainCam"), "MainCam2");
    }

    #[test]
    fn value_rewrite_handles_deep_arrays_objects_and_at_prefixed_keys() {
        let mut value = SceneValue::Object(Cow::Owned(vec![
            (
                SceneFieldName::from_name("plain".to_string()),
                SceneValue::Key(SceneValueKey::from("Camera")),
            ),
            (
                SceneFieldName::from_name("escaped".to_string()),
                SceneValue::Key(SceneValueKey::from("@Camera")),
            ),
            (
                SceneFieldName::from_name("nested".to_string()),
                SceneValue::Array(Cow::Owned(vec![
                    SceneValue::Key(SceneValueKey::from("Camera")),
                    SceneValue::Object(Cow::Owned(vec![
                        (
                            SceneFieldName::from_name("deep".to_string()),
                            SceneValue::Key(SceneValueKey::from("Camera")),
                        ),
                        (
                            SceneFieldName::from_name("keep".to_string()),
                            SceneValue::Key(SceneValueKey::from("Other")),
                        ),
                    ])),
                ])),
            ),
        ]));

        let refs = rewrite_node_refs_in_value(&mut value, "Camera", "MainCam");

        assert_eq!(refs, 4);
        assert_eq!(count_value_key_refs(&value, "Camera"), 0);
        assert_eq!(count_value_key_refs(&value, "MainCam"), 3);
        assert_eq!(count_value_key_refs(&value, "@MainCam"), 1);
        assert_eq!(count_value_key_refs(&value, "Other"), 1);
    }

    fn rename_test_doc() -> SceneDoc {
        SceneDoc::from_scene(perro_scene::Scene {
            root: Some(SceneKey::new(0)),
            key_names: Cow::Owned(vec![
                Cow::Borrowed("Root"),
                Cow::Borrowed("Camera"),
                Cow::Borrowed("Stream"),
                Cow::Borrowed("Other"),
            ]),
            nodes: Cow::Owned(vec![
                test_node(SceneKey::new(0), None, vec![], vec![]),
                test_node(SceneKey::new(1), Some("Old Display Name"), vec![], vec![]),
                test_node(
                    SceneKey::new(2),
                    None,
                    vec![(
                        SceneFieldName::Camera,
                        SceneValue::Key(SceneValueKey::from("Camera")),
                    )],
                    vec![
                        (
                            SceneFieldName::from_name("direct".to_string()),
                            SceneValue::Key(SceneValueKey::from("Camera")),
                        ),
                        (
                            SceneFieldName::from_name("nested".to_string()),
                            SceneValue::Object(Cow::Owned(vec![
                                (
                                    SceneFieldName::from_name("target".to_string()),
                                    SceneValue::Key(SceneValueKey::from("Camera")),
                                ),
                                (
                                    SceneFieldName::from_name("keep".to_string()),
                                    SceneValue::Key(SceneValueKey::from("Other")),
                                ),
                            ])),
                        ),
                        (
                            SceneFieldName::from_name("arr".to_string()),
                            SceneValue::Array(Cow::Owned(vec![
                                SceneValue::Key(SceneValueKey::from("Camera")),
                                SceneValue::Object(Cow::Owned(vec![(
                                    SceneFieldName::from_name("deep".to_string()),
                                    SceneValue::Key(SceneValueKey::from("Camera")),
                                )])),
                            ])),
                        ),
                    ],
                ),
                test_node(SceneKey::new(3), None, vec![], vec![]),
            ]),
        })
    }

    fn test_node(
        key: SceneKey,
        name: Option<&'static str>,
        data_fields: Vec<(SceneFieldName, SceneValue)>,
        script_vars: Vec<(SceneFieldName, SceneValue)>,
    ) -> SceneNodeEntry {
        SceneNodeEntry {
            data: SceneNodeData::new(perro_scene::NodeType::Node, Cow::Owned(data_fields), None),
            has_data_override: true,
            key,
            name: name.map(Cow::Borrowed),
            tags: Cow::Borrowed(&[]),
            children: Cow::Borrowed(&[]),
            parent: None,
            script: None,
            clear_script: false,
            root_of: None,
            script_vars: Cow::Owned(script_vars),
        }
    }

    fn count_doc_key_refs(doc: &SceneDoc, name: &str) -> usize {
        doc.scene
            .nodes
            .iter()
            .map(|node| {
                node.data
                    .fields
                    .iter()
                    .map(|(_, value)| count_value_key_refs(value, name))
                    .sum::<usize>()
                    + node
                        .script_vars
                        .iter()
                        .map(|(_, value)| count_value_key_refs(value, name))
                        .sum::<usize>()
            })
            .sum()
    }

    fn count_value_key_refs(value: &SceneValue, name: &str) -> usize {
        match value {
            SceneValue::Key(key) => usize::from(key.as_ref() == name),
            SceneValue::Object(fields) => fields
                .iter()
                .map(|(_, value)| count_value_key_refs(value, name))
                .sum(),
            SceneValue::Array(values) => values
                .iter()
                .map(|value| count_value_key_refs(value, name))
                .sum(),
            _ => 0,
        }
    }
}

pub fn edit_selected_script_vars<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(text) = read_text_box(ctx, "inspector_vars_box") else {
        return;
    };
    let vars = match parse_script_vars_text(&text) {
        Ok(vars) => vars,
        Err(err) => {
            set_log(ctx, &format!("script vars parse fail\n{err}"));
            return;
        }
    };
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
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
        node.script_vars = Cow::Owned(vars);
        set_state_scene_doc(state, &doc);
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = "edit script vars".to_string();
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
}

pub fn edit_selected_script_var_row<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
) {
    let Some(text) = read_text_box(ctx, &format!("inspector_var_{idx}_value")) else {
        return;
    };
    let value = match parse_script_var_value(text.trim()) {
        Ok(value) => value,
        Err(err) => {
            set_log(ctx, &format!("script var parse fail\n{err}"));
            return;
        }
    };
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
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
        let Some((_name, field_value)) = node.script_vars.to_mut().get_mut(idx) else {
            return false;
        };
        *field_value = value;
        set_state_scene_doc(state, &doc);
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = "edit script var".to_string();
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
}

pub fn pick_selected_script_var_ref<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
) {
    let pick = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let key = state.selected_key?;
        let doc = cached_scene_doc_shared(&state.doc_text);
        let node = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == key)?;
        let rows = inspector_display_rows_for_node(state, node);
        let row = rows.get(idx)?;
        if row.source == "section" {
            if let Some(pos) = state
                .inspector_collapsed_sections
                .iter()
                .position(|item| item == &row.path_key)
            {
                state.inspector_collapsed_sections.remove(pos);
            } else {
                state
                    .inspector_collapsed_sections
                    .push(row.path_key.clone());
            }
            return Some(false);
        }
        if row.expandable {
            if let Some(pos) = state
                .inspector_expanded_paths
                .iter()
                .position(|item| item == &row.path_key)
            {
                state.inspector_expanded_paths.remove(pos);
            } else {
                state.inspector_expanded_paths.push(row.path_key.clone());
            }
            return Some(false);
        }
        if row.source == "script_path" {
            state.inspector_picker_open = true;
            state.inspector_picker_field = "script".to_string();
            state.inspector_picker_kind = "asset".to_string();
            state.inspector_picker_offset = 0;
            state.inspector_picker_filter.clear();
            return Some(true);
        }
        if row.kind == "Bool" {
            let mut doc = cached_scene_doc(&state.doc_text);
            let node = doc
                .scene
                .nodes
                .to_mut()
                .iter_mut()
                .find(|node| node.key.as_u32() == key)?;
            let current = row.value == "true";
            if row.source == "script" {
                let defaults = inspector_script_var_default_fields_for_node(state, node);
                let mut fields = inspector_script_var_fields_for_node(state, node);
                if !set_value_at_path(&mut fields, &row.path, SceneValue::Bool(!current)) {
                    return None;
                }
                if !write_script_var_override(
                    node.script_vars.to_mut(),
                    &defaults,
                    &fields,
                    &row.path,
                ) {
                    return None;
                }
            } else {
                let mut fields = inspector_scene_value_fields_for_node(node);
                if !set_value_at_path(&mut fields, &row.path, SceneValue::Bool(!current)) {
                    return None;
                }
                if !write_scene_field_override(node.data.fields.to_mut(), &fields, &row.path) {
                    return None;
                }
            }
            set_state_scene_doc(state, &doc);
            state.dirty = true;
            if let Some(path) = state.open_paths.get(state.active_open).cloned()
                && !state.dirty_scene_paths.iter().any(|item| item == &path)
            {
                state.dirty_scene_paths.push(path);
            }
            state.log = format!("toggle script var\n{}", row.name.trim());
            return Some(false);
        }
        if row.kind.starts_with("Asset(") {
            state.inspector_picker_open = true;
            state.inspector_picker_field = idx.to_string();
            state.inspector_picker_kind = "value_asset".to_string();
            state.inspector_picker_offset = 0;
            state.inspector_picker_filter.clear();
            return Some(true);
        }
        if !row.kind.starts_with("Node") {
            if row.enum_options.is_empty() {
                return None;
            }
            state.inspector_picker_open = true;
            state.inspector_picker_field = idx.to_string();
            state.inspector_picker_kind = "value_enum".to_string();
            state.inspector_picker_offset = 0;
            state.inspector_picker_filter.clear();
            return Some(true);
        }
        state.inspector_picker_open = true;
        state.inspector_picker_field = idx.to_string();
        state.inspector_picker_kind = "value_node".to_string();
        state.inspector_picker_offset = 0;
        state.inspector_picker_filter.clear();
        Some(true)
    });
    let Some(open_picker) = pick.flatten() else {
        return;
    };
    if !open_picker {
        rebuild_preview(ctx);
        refresh_all(ctx);
        return;
    }
    set_inspector_picker(ctx, true);
    refresh_all(ctx);
}

pub fn update_inspector_picker_filter<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    update_inspector_picker_filter_from(ctx, "inspector_pick_filter_box");
}

pub fn update_inspector_picker_filter_from<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    box_name: &str,
) {
    let Some(text) = read_text_box(ctx, box_name) else {
        return;
    };
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.inspector_picker_filter = text;
        state.inspector_picker_offset = 0;
        state.focused_inspector_box = box_name.to_string();
    });
    refresh_all(ctx);
}

pub fn shift_inspector_picker<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    delta: isize,
) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let count = inspector_picker_entries(state).len();
        if count == 0 {
            state.inspector_picker_offset = 0;
            return;
        }
        let max = count.saturating_sub(1);
        let next = if delta < 0 {
            state
                .inspector_picker_offset
                .saturating_sub(MAX_INSPECTOR_PICKER_ROWS)
        } else {
            state
                .inspector_picker_offset
                .saturating_add(MAX_INSPECTOR_PICKER_ROWS)
                .min(max)
        };
        state.inspector_picker_offset = next - (next % MAX_INSPECTOR_PICKER_ROWS);
    });
    refresh_all(ctx);
}

pub fn choose_inspector_picker_row<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
) {
    let pick = with_state!(ctx.run, EditorState, ctx.id, |state| {
        let entry_idx = state.inspector_picker_offset + idx;
        let entry = inspector_picker_entries(state).get(entry_idx).cloned()?;
        Some((
            state.inspector_picker_field.clone(),
            state.inspector_picker_kind.clone(),
            entry.value,
        ))
    });
    let Some((field, picker_kind, value)) = pick else {
        return;
    };
    if picker_kind == "anim_field" {
        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            state.inspector_picker_open = false;
            state.inspector_picker_field.clear();
            state.inspector_picker_kind.clear();
            state.inspector_picker_offset = 0;
            state.inspector_picker_filter.clear();
        });
        set_inspector_picker(ctx, false);
        add_anim_track_field(ctx, &value);
        return;
    }
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
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
        if matches!(
            picker_kind.as_str(),
            "script_node" | "script_enum" | "value_node" | "value_enum" | "value_asset"
        ) {
            let Ok(row_idx) = field.parse::<usize>() else {
                return false;
            };
            let rows = if picker_kind.starts_with("value_") {
                inspector_display_rows_for_node(state, node)
            } else {
                inspector_script_var_rows_for_node(state, node)
            };
            let Some(row) = rows.get(row_idx) else {
                return false;
            };
            let scene_value = if picker_kind == "value_asset" {
                SceneValue::Str(Cow::Owned(value.clone()))
            } else {
                SceneValue::Key(SceneValueKey::from(value.clone()))
            };
            if row.source == "custom_icon" {
                crate::scripts_ui_editor_inspector_values_rs::write_node_custom_icon(
                    node,
                    Some(&value),
                );
            } else if row.source == "script" {
                let defaults = inspector_script_var_default_fields_for_node(state, node);
                let mut fields = inspector_script_var_fields_for_node(state, node);
                if !set_value_at_path(&mut fields, &row.path, scene_value) {
                    return false;
                }
                if !write_script_var_override(
                    node.script_vars.to_mut(),
                    &defaults,
                    &fields,
                    &row.path,
                ) {
                    return false;
                }
            } else {
                let mut fields = inspector_scene_value_fields_for_node(node);
                if !set_value_at_path(&mut fields, &row.path, scene_value) {
                    return false;
                }
                if !write_scene_field_override(node.data.fields.to_mut(), &fields, &row.path) {
                    return false;
                }
            }
        } else if picker_kind == "node" {
            set_scene_key(&mut node.data, &field, value.clone());
        } else if field == "script" {
            node.script = Some(Cow::Owned(value.clone()));
            node.clear_script = false;
            state.active_asset_path = base_res_asset_path(&value);
        } else if field == "root_of" {
            node.root_of = Some(Cow::Owned(value.clone()));
            state.active_asset_path = base_res_asset_path(&value);
        } else {
            set_scene_string(&mut node.data, &field, value.clone());
            state.active_asset_path = base_res_asset_path(&value);
        }
        set_state_scene_doc(state, &doc);
        state.dirty = true;
        state.inspector_picker_open = false;
        state.inspector_picker_field.clear();
        state.inspector_picker_kind.clear();
        state.inspector_picker_offset = 0;
        state.inspector_picker_filter.clear();
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = format!("set {field}\n{value}");
        true
    })
    .unwrap_or(false);
    if changed {
        set_inspector_picker(ctx, false);
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
}

pub fn resource_dialog_filters(
    node_type: perro_scene::NodeType,
    field: &str,
) -> Vec<(&'static str, &'static [&'static str])> {
    let Some(field) = perro_scene::scene_node_field(node_type, field) else {
        return vec![("Assets", &["*"])];
    };
    let perro_scene::NodeFieldType::Asset(kind) = field.ty else {
        return vec![("Assets", &["*"])];
    };
    editor_asset_filters(kind)
        .iter()
        .map(|filter| (filter.label, filter.extensions))
        .collect()
}

pub fn parse_script_vars_text(text: &str) -> Result<Vec<(SceneFieldName, SceneValue)>, String> {
    let mut out = Vec::new();
    for (line_no, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            return Err(format!("line {} missing =", line_no + 1));
        };
        let name = name.trim();
        if name.is_empty() {
            return Err(format!("line {} missing name", line_no + 1));
        }
        out.push((
            SceneFieldName::from_name(name.to_string()),
            parse_script_var_value(value.trim())
                .map_err(|err| format!("line {} {err}", line_no + 1))?,
        ));
    }
    Ok(out)
}

pub fn parse_script_var_value(text: &str) -> Result<SceneValue, String> {
    if text.is_empty() {
        return Err("missing value".to_string());
    }
    std::panic::catch_unwind(|| Parser::new(text).parse_value_literal())
        .map_err(|_| "bad scene value".to_string())
}

// ---------------------------------------------------------------------------
// Animation editor dock: track model, live preview player, timeline widgets.
// ---------------------------------------------------------------------------

// Layout constants shared with shell.scn's anim drawer. Keep in sync.
pub const ANIM_TRACK_COL_W: f32 = 0.20;
pub const ANIM_TIMELINE_COL_W: f32 = 0.79;
pub const ANIM_BODY_SPACING: f32 = 0.004;

pub fn cached_anim_doc(state: &EditorState) -> panim::PanimDoc {
    panim::parse_panim(&state.anim_doc_text)
}

fn touch_anim_doc(state: &mut EditorState, doc: &panim::PanimDoc) {
    state.anim_doc_text = panim::serialize_panim(doc);
    state.anim_dirty = true;
    state.anim_clip_dirty = true;
}

// Loads .panim text into editor state and auto-binds the scene's
// AnimationPlayer when one references this clip.
pub fn load_anim_text_into_state(state: &mut EditorState, anim_path: &str, text: String) {
    state.anim_doc_text = text;
    state.anim_dirty = false;
    state.anim_clip_dirty = true;
    state.anim_selected_track = 0;
    state.anim_track_scroll = 0;
    state.anim_playhead = 0.0;
    state.anim_playing = false;
    state.anim_ruler_drag = false;
    if state.active_anim_player_key.is_none() && !state.doc_text.is_empty() {
        let doc = cached_scene_doc_shared(&state.doc_text);
        let references_clip = |node: &SceneNodeEntry| {
            node.data.type_name() == "AnimationPlayer"
                && doc_field_value(&node.data, "animation")
                    .is_some_and(|value| scene_value_is_str(&value, anim_path))
        };
        // Prefer the selected AnimationPlayer when it references this clip,
        // so editing previews on the instance the user is working with.
        state.active_anim_player_key = state
            .selected_key
            .and_then(|key| {
                doc.scene
                    .nodes
                    .iter()
                    .find(|node| node.key.as_u32() == key && references_clip(node))
            })
            .or_else(|| doc.scene.nodes.iter().find(|node| references_clip(node)))
            .map(|node| node.key.as_u32());
    }
}

// Binding context for the dock UI: which player the preview runs through
// and, per object, the scene-node name it resolves to (None = unbound).
pub fn anim_binding_context(
    state: &EditorState,
    clip: &panim::PanimDoc,
) -> (String, Vec<(String, Option<String>)>) {
    if state.doc_text.is_empty() {
        return (
            "no scene open".to_string(),
            clip.objects
                .iter()
                .map(|(object, _)| (object.clone(), None))
                .collect(),
        );
    }
    let doc = cached_scene_doc_shared(&state.doc_text);
    let scene_names: Vec<String> = doc
        .scene
        .nodes
        .iter()
        .map(|node| doc.scene.key_name_or_id(node.key).to_string())
        .collect();
    let mut bindings: Vec<(String, String)> = Vec::new();
    let mut player_name = None;
    if let Some(player_key) = state.active_anim_player_key
        && let Some(player) = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == player_key)
    {
        player_name = Some(doc.scene.key_name_or_id(player.key).to_string());
        bindings = player_bindings(&player.data);
    }
    let context = match &player_name {
        Some(name) => format!("editing on {name}"),
        None => "no AnimationPlayer — select one, press Bind".to_string(),
    };
    let per_object = clip
        .objects
        .iter()
        .map(|(object, _)| {
            // Only explicit bindings count; unbound objects do nothing at
            // runtime, so show them unbound here too.
            let target = bindings
                .iter()
                .find(|(bound, _)| bound == object)
                .map(|(_, name)| name.clone());
            let resolved = target
                .as_ref()
                .is_some_and(|name| scene_names.iter().any(|scene| scene == name));
            (object.clone(), resolved.then(|| target.unwrap_or_default()))
        })
        .collect();
    (context, per_object)
}

// Dock follows AnimationPlayer selection in the scene tree: clips are
// always edited THROUGH a player instance, so selecting one routes (or
// loads) the dock onto it.
pub fn follow_player_selection<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let (to_open, attached) = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if !(state.anim_drawer_open && state.bottom_dock_open) || state.doc_text.is_empty() {
            return (None, false);
        }
        let Some(key) = state.selected_key else {
            return (None, false);
        };
        if state.active_anim_player_key == Some(key) {
            return (None, false);
        }
        let doc = cached_scene_doc_shared(&state.doc_text);
        let Some(node) = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == key)
        else {
            return (None, false);
        };
        if node.data.type_name() != "AnimationPlayer" {
            return (None, false);
        }
        let path = match doc_field_value(&node.data, "animation") {
            Some(SceneValue::Str(path)) if !path.is_empty() && path.as_ref() != "-" => {
                path.to_string()
            }
            _ => String::new(),
        };
        if !path.is_empty() && path != state.active_anim_path {
            // Different clip on this player: load it (loader prefers the
            // selected player as the preview route).
            (Some(path), false)
        } else {
            // Same clip, or a player without one yet: reroute the preview.
            state.active_anim_player_key = Some(key);
            state.anim_clip_dirty = true;
            state.log = format!("anim player\n{}", doc.scene.key_name_or_id(node.key));
            (None, true)
        }
    })
    .unwrap_or((None, false));
    if let Some(path) = to_open {
        open_animation_path(ctx, &path);
    } else if attached {
        refresh_all(ctx);
    }
}

// Opens the selected AnimationPlayer's clip in the dock when nothing is
// loaded yet, so the Animation tab lands on the instance you selected.
pub fn try_open_selected_player_clip<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let path = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if !state.anim_doc_text.is_empty() || state.doc_text.is_empty() {
            return None;
        }
        let key = state.selected_key?;
        let doc = cached_scene_doc_shared(&state.doc_text);
        let node = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == key)?;
        if node.data.type_name() != "AnimationPlayer" {
            return None;
        }
        let value = doc_field_value(&node.data, "animation")?;
        let SceneValue::Str(path) = value else {
            return None;
        };
        let path = path.to_string();
        (!path.is_empty() && path != "-").then_some(path)
    });
    if let Some(path) = path {
        open_animation_path(ctx, &path);
    }
}

// Bind button: selected AnimationPlayer attaches the dock to it (and
// points it at the open clip); any other node binds the selected track's
// object to it on the active player.
pub fn bind_anim_selection<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            state.log = "bind fail\nselect scene node".to_string();
            return false;
        };
        if state.doc_text.is_empty() || state.anim_doc_text.is_empty() {
            state.log = "bind fail\nopen scene + animation".to_string();
            return false;
        }
        let mut doc = cached_scene_doc(&state.doc_text);
        let Some(node_index) = doc
            .scene
            .nodes
            .iter()
            .position(|node| node.key.as_u32() == key)
        else {
            return false;
        };
        if doc.scene.nodes[node_index].data.type_name() == "AnimationPlayer" {
            state.active_anim_player_key = Some(key);
            state.anim_clip_dirty = true;
            let player_name = doc
                .scene
                .key_name_or_id(doc.scene.nodes[node_index].key)
                .to_string();
            let already_bound = doc_field_value(&doc.scene.nodes[node_index].data, "animation")
                .is_some_and(|value| scene_value_is_str(&value, &state.active_anim_path));
            if !already_bound && !state.active_anim_path.is_empty() {
                set_scene_string(
                    &mut doc.scene.nodes.to_mut()[node_index].data,
                    "animation",
                    state.active_anim_path.clone(),
                );
                set_state_scene_doc(state, &doc);
                state.dirty = true;
                if let Some(path) = state.open_paths.get(state.active_open).cloned()
                    && !state.dirty_scene_paths.iter().any(|item| item == &path)
                {
                    state.dirty_scene_paths.push(path);
                }
            }
            state.log = format!("attach player\n{player_name}");
            return true;
        }
        let Some(player_key) = state.active_anim_player_key else {
            state.log = "bind fail\nattach AnimationPlayer first\n(select player, press Bind)".to_string();
            return false;
        };
        let clip = cached_anim_doc(state);
        let Some(track) = clip.tracks.get(state.anim_selected_track) else {
            state.log = "bind fail\nselect track".to_string();
            return false;
        };
        let object = track.object.clone();
        let node_name = doc
            .scene
            .key_name_or_id(doc.scene.nodes[node_index].key)
            .to_string();
        let Some(player_index) = doc
            .scene
            .nodes
            .iter()
            .position(|node| node.key.as_u32() == player_key)
        else {
            state.log = "bind fail\nmissing AnimationPlayer".to_string();
            return false;
        };
        set_scene_binding(
            &mut doc.scene.nodes.to_mut()[player_index].data,
            &object,
            &node_name,
        );
        set_state_scene_doc(state, &doc);
        state.dirty = true;
        state.anim_clip_dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = format!("bind\n{object} -> {node_name}");
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
    }
    refresh_all(ctx);
}

fn scene_value_is_str(value: &SceneValue, expected: &str) -> bool {
    match value {
        SceneValue::Str(text) => text.as_ref() == expected,
        _ => false,
    }
}

// Field lookup that climbs the base-data chain like the runtime loader does.
pub fn doc_field_value(data: &SceneNodeData, field: &str) -> Option<SceneValue> {
    for (name, value) in data.fields.iter() {
        if name.as_ref() == field {
            return Some(value.clone());
        }
    }
    match data.base.as_ref()? {
        perro_scene::SceneNodeDataBase::Owned(base) => doc_field_value(base, field),
        perro_scene::SceneNodeDataBase::Borrowed(_) => None,
    }
}

pub fn scene_value_to_panim_text(value: &SceneValue) -> Option<String> {
    match value {
        SceneValue::Bool(v) => Some(v.to_string()),
        SceneValue::F32(v) => Some(format!("{v}")),
        SceneValue::I32(v) => Some(format!("{v}")),
        SceneValue::Vec2 { x, y } => Some(format!("({x}, {y})")),
        SceneValue::Vec3 { x, y, z } => Some(format!("({x}, {y}, {z})")),
        SceneValue::Vec4 { x, y, z, w } => Some(format!("({x}, {y}, {z}, {w})")),
        SceneValue::Str(text) => Some(format!("\"{}\"", text)),
        _ => None,
    }
}

// Reads the `bindings = { Object = NodeName }` map off a player node.
fn player_bindings(data: &SceneNodeData) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(SceneValue::Object(fields)) = doc_field_value(data, "bindings") {
        for (object, value) in fields.iter() {
            if let SceneValue::Key(name) = value {
                out.push((object.as_ref().to_string(), name.as_ref().to_string()));
            }
        }
    }
    out
}

// object -> scene doc key, strictly through the attached AnimationPlayer's
// bindings. The runtime resolves ONLY explicit bindings (scene_loader
// merge), so the editor previews exactly the same way: no player attached
// means nothing binds.
pub fn resolve_anim_object_keys(state: &EditorState, clip: &panim::PanimDoc) -> Vec<(String, u32)> {
    if state.doc_text.is_empty() {
        return Vec::new();
    }
    let Some(player_key) = state.active_anim_player_key else {
        return Vec::new();
    };
    let doc = cached_scene_doc_shared(&state.doc_text);
    let Some(player) = doc
        .scene
        .nodes
        .iter()
        .find(|node| node.key.as_u32() == player_key)
    else {
        return Vec::new();
    };
    let bindings = player_bindings(&player.data);
    let name_to_key: Vec<(String, u32)> = doc
        .scene
        .nodes
        .iter()
        .map(|node| {
            (
                doc.scene.key_name_or_id(node.key).to_string(),
                node.key.as_u32(),
            )
        })
        .collect();
    let mut out = Vec::new();
    for (object, _) in &clip.objects {
        let Some(target_name) = bindings
            .iter()
            .find(|(bound_object, _)| bound_object == object)
            .map(|(_, name)| name.clone())
        else {
            continue;
        };
        if let Some(pos) = name_to_key.iter().position(|(name, _)| *name == target_name) {
            out.push((object.clone(), name_to_key[pos].1));
        }
    }
    out
}

fn node_exists<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, id: u64) -> bool {
    id != 0 && get_node_name!(ctx.run, NodeID::from_u64(id)).is_some()
}

// Keeps the hidden preview AnimationPlayer in sync with the edited clip
// text. Recreates the player after preview rebuilds and rebuilds the clip
// whenever the text changed.
pub fn ensure_anim_preview<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let request = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if !state.anim_drawer_open || state.anim_doc_text.is_empty() || state.preview_root == 0 {
            return None;
        }
        // No attached player means no bindings resolve (runtime parity), so
        // there is nothing to preview.
        state.active_anim_player_key?;
        Some((
            state.preview_root,
            state.anim_preview_player,
            state.anim_clip_dirty,
            state.anim_preview_clip,
            state.anim_doc_text.clone(),
            state.anim_playhead,
        ))
    });
    let Some((root, player, clip_dirty, old_clip, text, playhead)) = request else {
        return;
    };
    let player_alive = node_exists(ctx, player);
    if player_alive && !clip_dirty {
        return;
    }
    let player_id = if player_alive {
        NodeID::from_u64(player)
    } else {
        create_node!(
            ctx.run,
            AnimationPlayer,
            "__editor_anim_preview_player",
            tags![],
            NodeID::from_u64(root)
        )
    };
    if old_clip != 0 {
        let _ = ctx.res.Animations().drop(AnimationID::from_u64(old_clip));
    }
    let clip = ctx.res.Animations().create_from_bytes(text.as_bytes());
    let _ = ctx.run.AnimPlayer().set_clip(player_id, clip);
    let _ = ctx.run.AnimPlayer().clear_bindings(player_id);
    let clip_doc = panim::parse_panim(&text);
    let objects = with_state!(ctx.run, EditorState, ctx.id, |state| {
        resolve_anim_object_keys(state, &clip_doc)
    });
    for (object, key) in objects {
        if let Some(node) = preview_node_for_key(ctx, key) {
            let _ = ctx.run.AnimPlayer().bind(player_id, &object, node);
        }
    }
    let _ = ctx.run.AnimPlayer().pause(player_id, true);
    let _ = ctx.run.AnimPlayer().seek_frame(player_id, playhead.max(0.0) as u32);
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.anim_preview_player = player_id.as_u64();
        state.anim_preview_clip = clip.as_u64();
        state.anim_clip_dirty = false;
    });
}

fn seek_anim_preview<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, frame: u32) {
    let player = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.anim_preview_player
    });
    if player != 0 {
        let player = NodeID::from_u64(player);
        let _ = ctx.run.AnimPlayer().pause(player, true);
        let _ = ctx.run.AnimPlayer().seek_frame(player, frame);
    }
}

// Window-space x span of the timeline column; mirrors the shell layout
// constants (see viewport_stream_rect_ratio for the same derivation).
pub fn anim_timeline_x_span<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
) -> Option<(f32, f32)> {
    const MAIN_PADDING: f32 = 0.0025;
    const MAIN_SPACING: f32 = 0.0025;
    let layout = with_state!(ctx.run, EditorState, ctx.id, editor_layout);
    let split_content_w = 1.0 - (MAIN_PADDING * 2.0) - (MAIN_SPACING * 3.0);
    let activity_w = split_content_w * layout.activity_w;
    let left_w = split_content_w * layout.left_w;
    let center_w = split_content_w * layout.center_w;
    let center_x0 = MAIN_PADDING + activity_w + MAIN_SPACING + left_w + MAIN_SPACING;
    let inner_x0 = center_x0 + center_w * 0.006;
    let inner_w = center_w * (1.0 - 0.012);
    let timeline_x0 = inner_x0 + inner_w * (ANIM_TRACK_COL_W + ANIM_BODY_SPACING);
    let timeline_w = inner_w * ANIM_TIMELINE_COL_W;
    Some((timeline_x0, timeline_w))
}

fn anim_pointer_ratio<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) -> Option<f32> {
    let mouse = mouse_position!(ctx.ipt);
    let viewport = ctx.res.viewport_size();
    if viewport.x <= 0.0 {
        return None;
    }
    let (x0, w) = anim_timeline_x_span(ctx)?;
    if w <= 0.0 {
        return None;
    }
    Some(((mouse.x / viewport.x - x0) / w).clamp(0.0, 1.0))
}

pub fn anim_total_frames(state: &EditorState) -> u32 {
    cached_anim_doc(state).total_frames().max(2)
}

fn seek_anim_to_ratio<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, ratio: f32) {
    let frame = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let total = anim_total_frames(state);
        let frame = (ratio * (total - 1) as f32).round().clamp(0.0, (total - 1) as f32);
        state.anim_playhead = frame;
        frame as u32
    })
    .unwrap_or(0);
    seek_anim_preview(ctx, frame);
    sync_anim_transport_widgets(ctx);
}

pub fn begin_anim_ruler_seek<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.anim_ruler_drag = true;
        state.anim_playing = false;
    });
    if let Some(ratio) = anim_pointer_ratio(ctx) {
        seek_anim_to_ratio(ctx, ratio);
    }
}

pub fn select_anim_track<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, idx: usize) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let track = state.anim_track_scroll + idx;
        let count = cached_anim_doc(state).tracks.len();
        if track < count {
            state.anim_selected_track = track;
        }
    });
    refresh_all(ctx);
}

pub fn click_anim_lane<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, idx: usize) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let track = state.anim_track_scroll + idx;
        let count = cached_anim_doc(state).tracks.len();
        if track < count {
            state.anim_selected_track = track;
        }
        state.anim_ruler_drag = true;
        state.anim_playing = false;
    });
    if let Some(ratio) = anim_pointer_ratio(ctx) {
        seek_anim_to_ratio(ctx, ratio);
    }
    refresh_all(ctx);
}

pub fn toggle_anim_play<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.anim_playing = !state.anim_playing;
        if state.anim_playing {
            let total = anim_total_frames(state);
            if state.anim_playhead >= (total - 1) as f32 {
                state.anim_playhead = 0.0;
            }
        }
    });
    sync_anim_transport_widgets(ctx);
}

pub fn stop_anim_playback<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.anim_playing = false;
        state.anim_playhead = 0.0;
    });
    seek_anim_preview(ctx, 0);
    sync_anim_transport_widgets(ctx);
}

pub fn toggle_anim_loop<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.anim_loop = !state.anim_loop;
    });
    sync_anim_transport_widgets(ctx);
}

// Reset to rest: drop the preview player and restore the authored scene
// pose so the live preview never overwrites the document state.
pub fn reset_anim_to_rest<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let (player, clip) = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let player = state.anim_preview_player;
        let clip = state.anim_preview_clip;
        state.anim_preview_player = 0;
        state.anim_preview_clip = 0;
        state.anim_clip_dirty = true;
        state.anim_playing = false;
        state.log = "reset to rest".to_string();
        (player, clip)
    })
    .unwrap_or((0, 0));
    if player != 0 && node_exists(ctx, player) {
        let _ = ctx.run.Nodes().remove_node(NodeID::from_u64(player));
    }
    if clip != 0 {
        let _ = ctx.res.Animations().drop(AnimationID::from_u64(clip));
    }
    rebuild_preview(ctx);
    refresh_all(ctx);
}

pub fn save_anim_doc<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let request = with_state!(ctx.run, EditorState, ctx.id, |state| {
        if state.active_anim_path.is_empty() || state.anim_doc_text.is_empty() {
            return None;
        }
        Some((
            res_to_abs(&state.project_root, &state.active_anim_path),
            state.active_anim_path.clone(),
            state.anim_doc_text.clone(),
        ))
    });
    let Some((abs, path, text)) = request else {
        set_log(ctx, "anim save fail\nno open animation");
        return;
    };
    if let Some(parent) = Path::new(&abs).parent() {
        let _ = fs::create_dir_all(parent);
    }
    match FileMod::save_string(&abs, &text) {
        Ok(()) => {
            let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                state.anim_dirty = false;
                state.project_file_sigs =
                    editor_file_watch::scan_project(Path::new(&state.project_root));
                state.log = format!("save animation\n{}", editor_files::rel_label(&path));
            });
            refresh_all(ctx);
        }
        Err(err) => set_log(ctx, &format!("anim save fail\n{path}\n{err}")),
    }
}

pub fn close_anim_editor<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let (player, clip) = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let out = (state.anim_preview_player, state.anim_preview_clip);
        state.anim_preview_player = 0;
        state.anim_preview_clip = 0;
        state.anim_playing = false;
        state.anim_ruler_drag = false;
        out
    })
    .unwrap_or((0, 0));
    if player != 0 && node_exists(ctx, player) {
        let _ = ctx.run.Nodes().remove_node(NodeID::from_u64(player));
    }
    if clip != 0 {
        let _ = ctx.res.Animations().drop(AnimationID::from_u64(clip));
    }
    rebuild_preview(ctx);
    set_anim_drawer(ctx, false);
}

pub fn seek_anim_frame_box<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(text) = read_text_box(ctx, "anim_frame_box") else {
        return;
    };
    let Ok(frame) = text.trim().parse::<u32>() else {
        return;
    };
    let frame = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let max = anim_total_frames(state) - 1;
        let frame = frame.min(max);
        state.anim_playhead = frame as f32;
        state.anim_playing = false;
        frame
    })
    .unwrap_or(0);
    seek_anim_preview(ctx, frame);
    refresh_all(ctx);
}

pub fn edit_anim_fps_box<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let Some(text) = read_text_box(ctx, "anim_fps_box") else {
        return;
    };
    let Ok(fps) = text.trim().parse::<f32>() else {
        return;
    };
    if fps <= 0.0 || fps > 1000.0 {
        return;
    }
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let mut doc = cached_anim_doc(state);
        doc.fps = fps;
        touch_anim_doc(state, &doc);
        state.log = format!("anim fps\n{fps}");
    });
    refresh_all(ctx);
}

// Inserts a key on the selected track at the playhead. The value comes
// from the bound node's scene-doc field (the pose you authored in the
// viewport), falling back to a sensible per-field default.
pub fn insert_anim_key<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let mut doc = cached_anim_doc(state);
        let Some(track) = doc.tracks.get(state.anim_selected_track).cloned() else {
            state.log = "key fail\nselect track".to_string();
            return false;
        };
        let frame = state.anim_playhead.round().max(0.0) as u32;
        let object_type = doc
            .object_type(&track.object)
            .unwrap_or("Node3D")
            .to_string();
        let value = anim_key_value_from_scene(state, &doc, &track.object, &track.field)
            .unwrap_or_else(|| {
                panim::default_field_value_text(&object_type, &track.field).to_string()
            });
        doc.set_key(&track.object, &track.field, frame, value);
        touch_anim_doc(state, &doc);
        state.log = format!("key {}\n{}.{} @ {frame}", doc.name, track.object, track.field);
        true
    })
    .unwrap_or(false);
    if changed {
        refresh_all(ctx);
    }
}

fn anim_key_value_from_scene(
    state: &EditorState,
    clip: &panim::PanimDoc,
    object: &str,
    field: &str,
) -> Option<String> {
    let keys = resolve_anim_object_keys(state, clip);
    let key = keys
        .iter()
        .find(|(name, _)| name == object)
        .map(|(_, key)| *key)?;
    let doc = cached_scene_doc_shared(&state.doc_text);
    let node = doc.scene.nodes.iter().find(|node| node.key.as_u32() == key)?;
    let value = doc_field_value(&node.data, field)?;
    scene_value_to_panim_text(&value)
}

pub fn delete_anim_key<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let mut doc = cached_anim_doc(state);
        let track_idx = state.anim_selected_track;
        let Some(track) = doc.tracks.get(track_idx).cloned() else {
            state.log = "del key fail\nselect track".to_string();
            return false;
        };
        let playhead = state.anim_playhead.round().max(0.0) as u32;
        let Some(frame) = doc.key_near(track_idx, playhead).filter(|frame| {
            frame.abs_diff(playhead) <= 2
        }) else {
            state.log = "del key fail\nno key near playhead".to_string();
            return false;
        };
        if track.keys.len() == 1 {
            doc.remove_track(&track.object, &track.field);
            if state.anim_selected_track >= doc.tracks.len() && state.anim_selected_track > 0 {
                state.anim_selected_track -= 1;
            }
            state.log = format!("remove track\n{}.{}", track.object, track.field);
        } else {
            doc.remove_key(&track.object, &track.field, frame);
            state.log = format!("del key\n{}.{} @ {frame}", track.object, track.field);
        }
        touch_anim_doc(state, &doc);
        true
    })
    .unwrap_or(false);
    if changed {
        refresh_all(ctx);
    }
}

// "+ Track" opens the field picker for the selected scene node.
pub fn open_anim_track_picker<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let ok = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.selected_key.is_none() {
            state.log = "track fail\nselect scene node".to_string();
            return false;
        }
        state.inspector_picker_open = true;
        state.inspector_picker_field = "anim".to_string();
        state.inspector_picker_kind = "anim_field".to_string();
        state.inspector_picker_offset = 0;
        state.inspector_picker_filter.clear();
        true
    })
    .unwrap_or(false);
    if ok {
        set_inspector_picker(ctx, true);
    }
    refresh_all(ctx);
}

// Adds (or reuses) the object for the selected node, keys `field` at
// frame 0 with the node's current value, and writes the object binding
// onto the attached AnimationPlayer — clips are always wired through a
// player, matching the runtime's binding-only resolution.
pub fn add_anim_track_field<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    field: &str,
) {
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        if state.anim_doc_text.is_empty() {
            state.log = "track fail\nno clip open\nselect AnimationPlayer, press New .panim".to_string();
            return false;
        }
        let Some(player_key) = state.active_anim_player_key else {
            state.log = "track fail\nno AnimationPlayer attached\nselect player, press Bind".to_string();
            return false;
        };
        let Some(key) = state.selected_key else {
            state.log = "track fail\nselect scene node".to_string();
            return false;
        };
        let mut doc = cached_scene_doc(&state.doc_text);
        let Some(node) = doc.scene.nodes.iter().find(|node| node.key.as_u32() == key) else {
            return false;
        };
        let Some(player_index) = doc
            .scene
            .nodes
            .iter()
            .position(|node| node.key.as_u32() == player_key)
        else {
            state.log = "track fail\nmissing AnimationPlayer".to_string();
            return false;
        };
        let node_name = doc.scene.key_name_or_id(node.key).to_string();
        let node_type = node.data.type_name().to_string();
        let node_value = doc_field_value(&node.data, field);
        let mut clip = cached_anim_doc(state);
        // Reuse the object this player already binds to the node, else add
        // one named after the node and bind it.
        let existing = player_bindings(&doc.scene.nodes[player_index].data)
            .into_iter()
            .find(|(_, target)| *target == node_name)
            .map(|(object, _)| object);
        let object = existing.unwrap_or_else(|| {
            let mut base = sanitize_panim_ident(&node_name);
            let mut suffix = 1;
            while clip.object_type(&base).is_some() {
                base = format!("{}_{suffix}", sanitize_panim_ident(&node_name));
                suffix += 1;
            }
            base
        });
        clip.ensure_object(&object, &node_type);
        if clip.track_index(&object, field).is_some() {
            state.log = format!("track exists\n{object}.{field}");
            return false;
        }
        let value = node_value
            .as_ref()
            .and_then(scene_value_to_panim_text)
            .unwrap_or_else(|| panim::default_field_value_text(&node_type, field).to_string());
        clip.set_key(&object, field, 0, value);
        touch_anim_doc(state, &clip);
        state.anim_selected_track = clip
            .track_index(&object, field)
            .unwrap_or(clip.tracks.len().saturating_sub(1));
        // Bind the object on the player when not already pointing there.
        let already_bound = player_bindings(&doc.scene.nodes[player_index].data)
            .iter()
            .any(|(bound_object, target)| *bound_object == object && *target == node_name);
        if !already_bound {
            set_scene_binding(
                &mut doc.scene.nodes.to_mut()[player_index].data,
                &object,
                &node_name,
            );
            set_state_scene_doc(state, &doc);
            state.dirty = true;
            if let Some(path) = state.open_paths.get(state.active_open).cloned()
                && !state.dirty_scene_paths.iter().any(|item| item == &path)
            {
                state.dirty_scene_paths.push(path);
            }
        }
        state.log = format!("add track\n{object}.{field} -> {node_name}");
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
}

// Per-frame: ruler scrubbing and playback advance. Cheap when idle.
pub fn update_anim_editor<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let (open, dragging, playing) = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (
            state.anim_drawer_open && state.bottom_dock_open,
            state.anim_ruler_drag,
            state.anim_playing,
        )
    });
    if !open {
        return;
    }
    ensure_anim_preview(ctx);
    if dragging {
        if !mouse_down!(ctx.ipt, MouseButton::Left) {
            let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
                state.anim_ruler_drag = false;
            });
        } else if let Some(ratio) = anim_pointer_ratio(ctx) {
            seek_anim_to_ratio(ctx, ratio);
        }
        return;
    }
    if !playing {
        return;
    }
    let dt = delta_time!(ctx.run);
    let frame = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let doc = cached_anim_doc(state);
        let total = doc.total_frames().max(2);
        let last = (total - 1) as f32;
        let mut next = state.anim_playhead + dt * doc.fps.max(0.001);
        if next > last {
            if state.anim_loop {
                next %= last.max(0.001);
            } else {
                next = last;
                state.anim_playing = false;
            }
        }
        state.anim_playhead = next;
        next.round() as u32
    })
    .unwrap_or(0);
    seek_anim_preview(ctx, frame);
    sync_anim_transport_widgets(ctx);
}

// ---------------------------------------------------------------------------
// Drawer widgets: key markers, playhead, transport labels.
// ---------------------------------------------------------------------------

const ANIM_MARKER_W: f32 = 0.011;
const ANIM_PLAYHEAD_W: f32 = 0.0035;

fn style_anim_marker<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, id: NodeID, height: f32, z: i32) {
    let _ = with_node_mut!(ctx.run, UiPanel, id, |node| {
        node.base.layout.anchor = UiAnchor::Left;
        node.base.layout.size = UiVector2::ratio(ANIM_MARKER_W, height);
        node.base.layout.z_index = z;
        node.base.input_enabled = false;
        node.base.mouse_filter = UiMouseFilter::Pass;
        node.base.visible = false;
        node.style.stroke_width = 0.0;
        node.style.corner_radii = UiCornerRadii::all(0.35);
    });
}

// Creates the runtime-only marker/playhead panels once per shell load.
fn ensure_anim_marker_nodes<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) -> bool {
    let playhead = with_state!(ctx.run, EditorState, ctx.id, |state| state.anim_playhead_id);
    if node_exists(ctx, playhead) {
        return true;
    }
    let Some(column) = find_named(ctx, "anim_timeline_col") else {
        return false;
    };
    let playhead_id = create_node!(ctx.run, UiPanel, "__anim_playhead", tags![], column);
    let _ = with_node_mut!(ctx.run, UiPanel, playhead_id, |node| {
        node.base.layout.anchor = UiAnchor::Left;
        node.base.layout.size = UiVector2::ratio(ANIM_PLAYHEAD_W, 1.0);
        node.base.layout.z_index = 40;
        node.base.input_enabled = false;
        node.base.mouse_filter = UiMouseFilter::Pass;
        node.style.stroke_width = 0.0;
        node.style.corner_radii = UiCornerRadii::all(0.0);
        if let Some(color) = Color::from_hex(theme::ACCENT_SOFT) {
            node.style.fill = color;
        }
    });
    let mut marker_ids = Vec::with_capacity(MAX_ANIM_TRACKS * MAX_ANIM_MARKERS);
    for lane in 0..MAX_ANIM_TRACKS {
        let lane_id = find_named(ctx, &format!("anim_lane_{lane}"));
        for slot in 0..MAX_ANIM_MARKERS {
            let Some(lane_id) = lane_id else {
                marker_ids.push(0);
                continue;
            };
            let marker = create_node!(
                ctx.run,
                UiPanel,
                format!("__anim_key_{lane}_{slot}"),
                tags![],
                lane_id
            );
            style_anim_marker(ctx, marker, 0.46, 20);
            marker_ids.push(marker.as_u64());
        }
    }
    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.anim_playhead_id = playhead_id.as_u64();
        state.anim_marker_ids = marker_ids;
    });
    true
}

// Lightweight transport sync used during scrub/playback (no full refresh).
pub fn sync_anim_transport_widgets<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let (playhead, total, playing, looping, playhead_id) =
        with_state!(ctx.run, EditorState, ctx.id, |state| {
            let total = anim_total_frames(state);
            (
                state.anim_playhead,
                total,
                state.anim_playing,
                state.anim_loop,
                state.anim_playhead_id,
            )
        });
    set_label(ctx, "anim_play_label", if playing { "Pause" } else { "Play" });
    set_button_fill(
        ctx,
        "anim_loop_button",
        if looping { theme::ACCENT } else { theme::BG_WIDGET },
    );
    set_text_box(ctx, "anim_frame_box", &format!("{}", playhead.round() as u32));
    set_label(ctx, "anim_len_label", &format!("/ {}", total - 1));
    if playhead_id != 0 {
        let ratio = ((playhead + 0.5) / total as f32).clamp(0.0, 1.0) - ANIM_PLAYHEAD_W * 0.5;
        let _ = with_node_mut!(ctx.run, UiPanel, NodeID::from_u64(playhead_id), |node| {
            node.base.visible = true;
            node.base.transform.translation = Vector2::new(ratio.max(0.0), 0.0);
        });
    }
}

// Full drawer refresh: track rows, key markers, toolbar state.
pub fn refresh_anim_drawer_widgets<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    let open = with_state!(ctx.run, EditorState, ctx.id, |state| {
        state.anim_drawer_open && state.bottom_dock_open
    });
    if !open {
        return;
    }
    if !ensure_anim_marker_nodes(ctx) {
        return;
    }
    let (tracks, total, name, fps, selected, scroll, dirty, path, marker_ids, bind_context) =
        with_state!(ctx.run, EditorState, ctx.id, |state| {
            let doc = cached_anim_doc(state);
            let total = doc.total_frames().max(2);
            let (bind_context, bound_objects) = anim_binding_context(state, &doc);
            let tracks: Vec<(String, String, usize, Vec<(u32, bool)>, bool)> = doc
                .tracks
                .iter()
                .map(|track| {
                    let bound = bound_objects
                        .iter()
                        .find(|(object, _)| *object == track.object)
                        .is_some_and(|(_, target)| target.is_some());
                    (
                        track.object.clone(),
                        track.field.clone(),
                        track.keys.len(),
                        track
                            .keys
                            .iter()
                            .map(|key| (key.frame, key.open))
                            .collect(),
                        bound,
                    )
                })
                .collect();
            (
                tracks,
                total,
                doc.name.clone(),
                doc.fps,
                state.anim_selected_track,
                state.anim_track_scroll,
                state.anim_dirty,
                state.active_anim_path.clone(),
                state.anim_marker_ids.clone(),
                bind_context,
            )
        });
    let title = if path.is_empty() {
        "Animation".to_string()
    } else {
        format!(
            "{}{}  {}",
            name,
            if dirty { " *" } else { "" },
            editor_files::rel_label(&path)
        )
    };
    set_label(ctx, "anim_drawer_title", &title);
    set_label(
        ctx,
        "anim_ruler_label",
        &format!("{bind_context}  ·  drag to scrub"),
    );
    set_text_box(ctx, "anim_fps_box", &format!("{fps}"));
    set_button_fill(
        ctx,
        "anim_save_button",
        if dirty { theme::REVERT } else { theme::BG_WIDGET },
    );
    let accent = Color::from_hex(theme::ACCENT);
    let key_color = Color::from_hex(theme::TEXT_DIM);
    let open_color = Color::from_hex(theme::TEXT_FAINT);
    for row in 0..MAX_ANIM_TRACKS {
        let track = tracks.get(scroll + row);
        let label = match track {
            Some((object, field, count, _, bound)) => {
                if *bound {
                    format!("{object} . {field}  [{count}]")
                } else {
                    format!("{object} . {field}  · unbound")
                }
            }
            None => String::new(),
        };
        set_label(ctx, &format!("anim_track_label_{row}"), &label);
        set_label_color(
            ctx,
            &format!("anim_track_label_{row}"),
            match track {
                Some((_, _, _, _, bound)) if !bound && scroll + row != selected => {
                    theme::TEXT_FAINT
                }
                _ => theme::TEXT,
            },
        );
        set_button_fill(
            ctx,
            &format!("anim_track_row_{row}"),
            if track.is_some() && scroll + row == selected {
                theme::ACCENT
            } else if track.is_some() {
                theme::BG_WIDGET
            } else {
                "#00000000"
            },
        );
        for slot in 0..MAX_ANIM_MARKERS {
            let id = marker_ids
                .get(row * MAX_ANIM_MARKERS + slot)
                .copied()
                .unwrap_or(0);
            if id == 0 {
                continue;
            }
            let key = track.and_then(|(_, _, _, keys, _)| keys.get(slot).copied());
            let _ = with_node_mut!(ctx.run, UiPanel, NodeID::from_u64(id), |node| {
                match key {
                    Some((frame, open_key)) => {
                        node.base.visible = true;
                        let ratio = ((frame as f32 + 0.5) / total as f32).clamp(0.0, 1.0)
                            - ANIM_MARKER_W * 0.5;
                        node.base.transform.translation = Vector2::new(ratio.max(0.0), 0.0);
                        let fill = if open_key {
                            open_color
                        } else if scroll + row == selected {
                            accent
                        } else {
                            key_color
                        };
                        if let Some(fill) = fill {
                            node.style.fill = fill;
                        }
                    }
                    None => node.base.visible = false,
                }
            });
        }
    }
    sync_anim_transport_widgets(ctx);
}

pub fn parse_number_list(text: &str) -> Option<Vec<f32>> {
    let values = text
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split([',', ' '])
        .filter(|part| !part.trim().is_empty())
        .map(|part| part.trim().parse::<f32>())
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (!values.is_empty() && values.len() <= 4).then_some(values)
}
