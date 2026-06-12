use crate::scripts_editor_app_rs as editor_app;
use crate::scripts_editor_assets_rs::*;
use crate::scripts_editor_file_watch_rs as editor_file_watch;
use crate::scripts_editor_files_rs as editor_files;
use crate::scripts_editor_gizmos_rs as editor_gizmos;
use crate::scripts_editor_manager_rs as editor_manager;
use crate::scripts_editor_nav_rs::*;
use crate::scripts_editor_nodes_rs::*;
use crate::scripts_editor_project_rs as editor_project;
use crate::scripts_editor_scene_deps_rs as editor_scene_deps;
use crate::scripts_editor_scene_rs as editor_scene;
use crate::scripts_editor_ui_rs::*;
use crate::scripts_editor_view_rs as editor_view;
use crate::scripts_editor_viewport_rs::*;
use crate::scripts_main_rs::{
    EditorState, FILE_WATCH_INTERVAL_FRAMES, MAX_FILES, MAX_NODE_PICKER_ROWS, MAX_NODES,
    MAX_RECENT, MAX_TABS, RECENT_PROJECTS_PATH,
};
use perro_api::prelude::*;
use perro_api::scene::{
    SceneDoc, SceneFieldName, SceneKey, SceneNodeData, SceneNodeEntry, SceneValue, SceneValueKey,
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
    let pick = with_state!(ctx.run, EditorState, ctx.id, |state| {
        let key = state.selected_key?;
        let doc = SceneDoc::parse(&state.doc_text);
        let node = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == key)?;
        let rows = resource_field_rows(&node.data);
        let field = rows.get(idx)?.name.clone();
        Some((state.project_root.clone(), field))
    });
    let Some((root, field)) = pick else {
        return;
    };
    let filters = resource_dialog_filters(&field);
    let Some(abs) = FileMod::pick_file(&format!("Select {field}"), filters.as_slice()) else {
        return;
    };
    let Some(mut res_path) = abs_to_res(Path::new(&root), Path::new(&abs)) else {
        set_log(ctx, "select resource fail\nfile must be under project res/");
        return;
    };
    if field == "mesh" && is_gltf_path(&res_path) {
        res_path = format!("{res_path}:mesh[0]");
    }
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
        set_scene_string(&mut node.data, &field, res_path.clone());
        state.doc_text = doc.to_text();
        state.active_asset_path = res_path.clone();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = format!("set {field}\n{res_path}");
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
}

pub fn resource_dialog_filters(field: &str) -> Vec<(&'static str, &'static [&'static str])> {
    match field {
        "texture" => vec![("Images", &["png", "jpg", "jpeg", "webp", "bmp", "tga", "svg"])],
        "mesh" => vec![("GLB", &["glb", "gltf"])],
        "material" => vec![("Perro Material", &["pmat"])],
        "animation" => vec![("Perro Animation", &["panim"])],
        _ => vec![("Assets", &["*"])],
    }
}

pub fn parse_script_vars_text(
    text: &str,
) -> Result<Vec<(SceneFieldName, SceneValue)>, String> {
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
    if let Some(key) = text.strip_prefix('@') {
        return Ok(SceneValue::Key(SceneValueKey::from(key.trim().to_string())));
    }
    if text.eq_ignore_ascii_case("true") {
        return Ok(SceneValue::Bool(true));
    }
    if text.eq_ignore_ascii_case("false") {
        return Ok(SceneValue::Bool(false));
    }
    if text.starts_with('"') && text.ends_with('"') && text.len() >= 2 {
        return Ok(SceneValue::Str(Cow::Owned(
            text[1..text.len() - 1].replace("\\\"", "\""),
        )));
    }
    if text.starts_with('(') && text.ends_with(')') {
        let values = parse_number_list(text).ok_or_else(|| "bad tuple".to_string())?;
        return match values.as_slice() {
            [x, y] => Ok(SceneValue::Vec2 { x: *x, y: *y }),
            [x, y, z] => Ok(SceneValue::Vec3 {
                x: *x,
                y: *y,
                z: *z,
            }),
            [x, y, z, w] => Ok(SceneValue::Vec4 {
                x: *x,
                y: *y,
                z: *z,
                w: *w,
            }),
            _ => Err("tuple needs 2-4 nums".to_string()),
        };
    }
    if let Ok(value) = text.parse::<i32>() {
        return Ok(SceneValue::I32(value));
    }
    if let Ok(value) = text.parse::<f32>() {
        return Ok(SceneValue::F32(value));
    }
    Ok(SceneValue::Str(Cow::Owned(text.to_string())))
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
