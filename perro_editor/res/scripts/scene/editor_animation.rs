use crate::scripts_app_editor_app_rs as editor_app;
use crate::scripts_app_editor_manager_rs as editor_manager;
use crate::scripts_app_editor_project_rs as editor_project;
use crate::scripts_assets_editor_assets_rs::*;
use crate::scripts_assets_editor_file_watch_rs as editor_file_watch;
use crate::scripts_assets_editor_files_rs as editor_files;
use crate::scripts_editor_main_rs::{
    EditorState, FILE_WATCH_INTERVAL_FRAMES, MAX_FILES, MAX_INSPECTOR_PICKER_ROWS,
    MAX_NODE_PICKER_ROWS, MAX_NODES, MAX_RECENT, MAX_TABS, RECENT_PROJECTS_PATH,
};
use crate::scripts_scene_editor_gizmos_rs as editor_gizmos;
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
        node.script_vars = Cow::Owned(vars);
        state.doc_text = doc.to_text();
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
        let Some((_name, field_value)) = node.script_vars.to_mut().get_mut(idx) else {
            return false;
        };
        *field_value = value;
        state.doc_text = doc.to_text();
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

pub fn pick_selected_resource_field<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
) {
    let pick = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let key = state.selected_key?;
        let doc = SceneDoc::parse(&state.doc_text);
        let node = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == key)?;
        let rows = resource_field_rows(&node.data);
        let row = rows.get(idx)?;
        state.inspector_picker_open = true;
        state.inspector_picker_field = row.name.clone();
        state.inspector_picker_kind = row.picker_kind.clone();
        state.inspector_picker_offset = 0;
        state.inspector_picker_filter.clear();
        Some(())
    });
    if pick.flatten().is_none() {
        return;
    }
    set_inspector_picker(ctx, true);
    refresh_all(ctx);
}

pub fn pick_selected_script_var_ref<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
) {
    let pick = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let key = state.selected_key?;
        let doc = SceneDoc::parse(&state.doc_text);
        let node = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == key)?;
        let rows = inspector_script_var_rows_for_node(state, node);
        let row = rows.get(idx)?;
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
        if row.kind != "Node" {
            return None;
        }
        state.inspector_picker_open = true;
        state.inspector_picker_field = idx.to_string();
        state.inspector_picker_kind = "script_node".to_string();
        state.inspector_picker_offset = 0;
        state.inspector_picker_filter.clear();
        Some(true)
    });
    let Some(open_picker) = pick.flatten() else {
        return;
    };
    if !open_picker {
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
    let changed = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        let Some(key) = state.selected_key else {
            return false;
        };
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
        if picker_kind == "script_node" {
            let Ok(row_idx) = field.parse::<usize>() else {
                return false;
            };
            let rows = inspector_script_var_rows_for_node(state, node);
            let Some(row) = rows.get(row_idx) else {
                return false;
            };
            let mut fields = inspector_script_var_fields_for_node(state, node);
            if !set_value_at_path(
                &mut fields,
                &row.path,
                SceneValue::Key(SceneValueKey::from(value.clone())),
            ) {
                return false;
            }
            if !write_script_var_override(node.script_vars.to_mut(), &fields, &row.path) {
                return false;
            }
        } else if picker_kind == "node" {
            set_scene_key(&mut node.data, &field, value.clone());
        } else {
            set_scene_string(&mut node.data, &field, value.clone());
            state.active_asset_path = base_res_asset_path(&value);
        }
        state.doc_text = doc.to_text();
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
    let Some(field) = perro_scene::scene_inspector_field(node_type, field) else {
        return vec![("Assets", &["*"])];
    };
    let perro_scene::SceneInspectorValueKind::Asset(kind) = field.kind else {
        return vec![("Assets", &["*"])];
    };
    perro_scene::scene_asset_filters(kind)
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
