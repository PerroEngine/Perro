use crate::scripts_assets_editor_assets_rs::*;
use crate::scripts_editor_main_rs::EditorState;
use crate::scripts_scene_editor_animation_rs::*;
use crate::scripts_scene_editor_viewport_rs::*;
use crate::scripts_ui_editor_ui_rs::*;
use perro_api::prelude::*;
use perro_api::scene::{Parser, SceneDoc, SceneFieldName, SceneValue};
use std::borrow::Cow;
use std::fs;

pub const MAX_INSPECTOR_VALUE_ROWS: usize = 8;
const MAX_INSPECTOR_DEPTH: usize = 4;

#[derive(Clone)]
pub struct InspectorValueRow {
    pub path: Vec<ValuePathStep>,
    pub path_key: String,
    pub name: String,
    pub kind: String,
    pub value: String,
    pub editable: bool,
    pub expandable: bool,
}

#[derive(Clone)]
pub enum ValuePathStep {
    Root(usize),
    Field(String),
    Index(usize),
}

pub fn inspector_script_var_rows(
    fields: &[(SceneFieldName, SceneValue)],
    expanded_paths: &[String],
) -> Vec<InspectorValueRow> {
    let mut rows = Vec::new();
    for (idx, (name, value)) in fields.iter().enumerate() {
        let mut path = vec![ValuePathStep::Root(idx)];
        push_value_rows(
            &mut rows,
            name.as_ref(),
            value,
            &mut path,
            0,
            MAX_INSPECTOR_VALUE_ROWS,
            expanded_paths,
        );
        if rows.len() >= MAX_INSPECTOR_VALUE_ROWS {
            break;
        }
    }
    rows
}

pub fn inspector_script_var_rows_for_node(
    state: &EditorState,
    node: &perro_api::scene::SceneNodeEntry,
) -> Vec<InspectorValueRow> {
    let fields = inspector_script_var_fields_for_node(state, node);
    inspector_script_var_rows(&fields, &state.inspector_expanded_paths)
}

pub fn inspector_script_var_fields_for_node(
    state: &EditorState,
    node: &perro_api::scene::SceneNodeEntry,
) -> Vec<(SceneFieldName, SceneValue)> {
    let mut fields = node
        .script
        .as_ref()
        .map(|path| script_state_default_fields(&state.project_root, path.as_ref()))
        .unwrap_or_default();
    merge_script_var_overrides(&mut fields, node.script_vars.as_ref());
    fields
}

fn merge_script_var_overrides(
    fields: &mut Vec<(SceneFieldName, SceneValue)>,
    overrides: &[(SceneFieldName, SceneValue)],
) {
    for (name, value) in overrides {
        if let Some((_, existing)) = fields.iter_mut().find(|(field, _)| field == name) {
            *existing = value.clone();
        } else {
            fields.push((name.clone(), value.clone()));
        }
    }
}

fn script_state_default_fields(
    project_root: &str,
    script_path: &str,
) -> Vec<(SceneFieldName, SceneValue)> {
    let abs = res_to_abs(project_root, script_path);
    let Ok(source) = fs::read_to_string(abs) else {
        return Vec::new();
    };
    let Some(struct_name) = parse_state_struct_name(&source) else {
        return Vec::new();
    };
    parse_script_struct_fields(&source, &struct_name)
        .into_iter()
        .map(|field| {
            (
                SceneFieldName::Custom(Cow::Owned(field.name)),
                field.default_value,
            )
        })
        .collect()
}

struct ScriptStateField {
    name: String,
    default_value: SceneValue,
}

fn parse_state_struct_name(source: &str) -> Option<String> {
    let mut saw_state_attr = false;
    for line in source.lines() {
        let line = strip_line_comment(line).trim();
        if line == "#[State]" || line == "#[state]" {
            saw_state_attr = true;
            continue;
        }
        if saw_state_attr {
            if line.starts_with("#[") || line.is_empty() {
                continue;
            }
            return parse_struct_name(line);
        }
    }
    None
}

fn parse_struct_name(line: &str) -> Option<String> {
    let mut parts = line.split_whitespace().peekable();
    while let Some(part) = parts.next() {
        if part == "struct" {
            return parts
                .next()
                .map(|value| value.trim_matches('{').trim().to_string())
                .filter(|value| !value.is_empty());
        }
    }
    None
}

fn parse_script_struct_fields(source: &str, struct_name: &str) -> Vec<ScriptStateField> {
    let lines = source.lines().collect::<Vec<_>>();
    let Some(mut idx) = lines
        .iter()
        .position(|line| parse_struct_name(strip_line_comment(line).trim()) == Some(struct_name.to_string()))
    else {
        return Vec::new();
    };
    let mut fields = Vec::new();
    let mut depth = 0_i32;
    let mut opened = false;
    let mut pending_default = None;
    while idx < lines.len() {
        let line = strip_line_comment(lines[idx]).trim();
        if !opened {
            if let Some(pos) = line.find('{') {
                opened = true;
                depth = 1 + brace_delta(&line[pos + 1..]);
            }
            idx += 1;
            continue;
        }
        if depth == 1 {
            if let Some(default_value) = parse_default_attr(line) {
                pending_default = Some(default_value);
            } else if let Some(field) = parse_script_field_line(line, pending_default.take()) {
                fields.push(field);
            }
        }
        depth += brace_delta(line);
        if depth <= 0 {
            break;
        }
        idx += 1;
    }
    fields
}

fn parse_script_field_line(line: &str, default_attr: Option<String>) -> Option<ScriptStateField> {
    let trimmed = line.trim().trim_end_matches(',').trim();
    if trimmed.is_empty() || trimmed.starts_with("#[") || trimmed.starts_with("///") {
        return None;
    }
    let without_vis = trimmed
        .strip_prefix("pub ")
        .unwrap_or(trimmed)
        .trim_start();
    let (name, ty) = without_vis.split_once(':')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let default_value = default_attr
        .as_deref()
        .and_then(|value| parse_default_value(value, ty.trim()))
        .unwrap_or_else(|| default_scene_value_for_type(ty.trim()));
    Some(ScriptStateField {
        name: name.to_string(),
        default_value,
    })
}

fn parse_default_attr(line: &str) -> Option<String> {
    let inner = line.strip_prefix("#[default")?.strip_suffix(']')?.trim();
    let value = inner.strip_prefix('=')?.trim();
    Some(value.to_string())
}

fn parse_default_value(value: &str, ty: &str) -> Option<SceneValue> {
    if value.contains("::") || value.contains('{') || value.contains('[') {
        return None;
    }
    if is_int_type(ty)
        && let Ok(value) = value.parse::<i32>()
    {
        return Some(SceneValue::I32(value));
    }
    Some(Parser::new(value).parse_value_literal())
}

fn is_int_type(ty: &str) -> bool {
    matches!(
        ty.chars().filter(|ch| !ch.is_whitespace()).collect::<String>().as_str(),
        "i8" | "i16" | "i32" | "u8" | "u16" | "u32"
    )
}

fn default_scene_value_for_type(ty: &str) -> SceneValue {
    let ty = ty.chars().filter(|ch| !ch.is_whitespace()).collect::<String>();
    match ty.as_str() {
        "bool" => SceneValue::Bool(false),
        "f32" | "f64" => SceneValue::F32(0.0),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64"
        | "u128" | "usize" => SceneValue::I32(0),
        "String" | "Arc<str>" | "std::sync::Arc<str>" | "Cow<'static,str>" => {
            SceneValue::Str(Cow::Borrowed(""))
        }
        "Vector2" | "perro_api::prelude::Vector2" => SceneValue::Vec2 { x: 0.0, y: 0.0 },
        "Vector3" | "perro_api::prelude::Vector3" => SceneValue::Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        "Vector4" | "perro_api::prelude::Vector4" | "Color" | "perro_api::prelude::Color" => {
            SceneValue::Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            }
        }
        "NodeID" | "perro_api::prelude::NodeID" => {
            SceneValue::Key(perro_api::scene::SceneValueKey::from("null"))
        }
        _ if ty.starts_with("Option<") => SceneValue::Key(perro_api::scene::SceneValueKey::from("null")),
        _ if ty.starts_with("Vec<") => SceneValue::Array(Cow::Owned(Vec::new())),
        _ => SceneValue::Object(Cow::Owned(Vec::new())),
    }
}

fn strip_line_comment(line: &str) -> &str {
    line.split("//").next().unwrap_or(line)
}

fn brace_delta(line: &str) -> i32 {
    let opens = line.chars().filter(|ch| *ch == '{').count() as i32;
    let closes = line.chars().filter(|ch| *ch == '}').count() as i32;
    opens - closes
}

fn push_value_rows(
    rows: &mut Vec<InspectorValueRow>,
    name: &str,
    value: &SceneValue,
    path: &mut Vec<ValuePathStep>,
    depth: usize,
    max: usize,
    expanded_paths: &[String],
) {
    if rows.len() >= max {
        return;
    }
    let composite = matches!(value, SceneValue::Array(_) | SceneValue::Object(_));
    let path_key = value_path_key(path);
    let expanded = composite && expanded_paths.iter().any(|item| item == &path_key);
    rows.push(InspectorValueRow {
        path: path.clone(),
        path_key,
        name: format!("{}{}", "  ".repeat(depth), name),
        kind: scene_value_kind(value).to_string(),
        value: if composite {
            scene_value_summary(value, expanded)
        } else {
            scene_value_edit_text(value)
        },
        editable: !composite,
        expandable: composite,
    });
    if !expanded || depth >= MAX_INSPECTOR_DEPTH {
        return;
    }
    match value {
        SceneValue::Array(values) => {
            for (idx, item) in values.iter().enumerate() {
                path.push(ValuePathStep::Index(idx));
                push_value_rows(
                    rows,
                    &format!("[{idx}]"),
                    item,
                    path,
                    depth + 1,
                    max,
                    expanded_paths,
                );
                path.pop();
                if rows.len() >= max {
                    break;
                }
            }
        }
        SceneValue::Object(fields) => {
            for (field, item) in fields.iter() {
                path.push(ValuePathStep::Field(field.as_ref().to_string()));
                push_value_rows(
                    rows,
                    field.as_ref(),
                    item,
                    path,
                    depth + 1,
                    max,
                    expanded_paths,
                );
                path.pop();
                if rows.len() >= max {
                    break;
                }
            }
        }
        _ => {}
    }
}

pub fn scene_value_summary(value: &SceneValue, expanded: bool) -> String {
    let marker = if expanded { "v" } else { ">" };
    match value {
        SceneValue::Array(values) if values.is_empty() => format!("{marker} Array []"),
        SceneValue::Array(values) => format!("{marker} Array [{}]", values.len()),
        SceneValue::Object(fields) if fields.is_empty() => format!("{marker} Object {{}}"),
        SceneValue::Object(fields) => format!("{marker} Object [{}]", fields.len()),
        _ => scene_value_edit_text(value),
    }
}

pub fn value_path_key(path: &[ValuePathStep]) -> String {
    let mut out = String::new();
    for step in path {
        match step {
            ValuePathStep::Root(idx) => out.push_str(&format!("r{idx}")),
            ValuePathStep::Field(name) => {
                out.push('.');
                out.push_str(name);
            }
            ValuePathStep::Index(idx) => out.push_str(&format!("[{idx}]")),
        }
    }
    out
}

pub fn edit_selected_script_var_path<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
) {
    let rows = with_state!(ctx.run, EditorState, ctx.id, |state| {
        let key = state.selected_key?;
        let doc = SceneDoc::parse(&state.doc_text);
        let node = doc
            .scene
            .nodes
            .iter()
            .find(|node| node.key.as_u32() == key)?;
        Some(inspector_script_var_rows_for_node(state, node))
    });
    let Some(rows) = rows else {
        return;
    };
    let Some(row) = rows.get(idx).cloned() else {
        return;
    };
    if !row.editable {
        set_log(ctx, "script var edit fail\ncontainer row");
        return;
    }
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
        let mut fields = inspector_script_var_fields_for_node(state, node);
        if !set_value_at_path(&mut fields, &row.path, value) {
            return false;
        }
        if !write_script_var_override(node.script_vars.to_mut(), &fields, &row.path) {
            return false;
        }
        state.doc_text = doc.to_text();
        state.dirty = true;
        if let Some(path) = state.open_paths.get(state.active_open).cloned()
            && !state.dirty_scene_paths.iter().any(|item| item == &path)
        {
            state.dirty_scene_paths.push(path);
        }
        state.log = format!("edit script var\n{}", row.name.trim());
        true
    })
    .unwrap_or(false);
    if changed {
        rebuild_preview(ctx);
        refresh_all(ctx);
    }
}

pub fn set_value_at_path(
    fields: &mut Vec<(SceneFieldName, SceneValue)>,
    path: &[ValuePathStep],
    value: SceneValue,
) -> bool {
    let Some(ValuePathStep::Root(idx)) = path.first() else {
        return false;
    };
    let Some((_name, root_value)) = fields.get_mut(*idx) else {
        return false;
    };
    set_nested_value(root_value, &path[1..], value)
}

pub fn write_script_var_override(
    overrides: &mut Vec<(SceneFieldName, SceneValue)>,
    fields: &[(SceneFieldName, SceneValue)],
    path: &[ValuePathStep],
) -> bool {
    let Some(ValuePathStep::Root(idx)) = path.first() else {
        return false;
    };
    let Some((name, value)) = fields.get(*idx).cloned() else {
        return false;
    };
    if let Some((_, existing)) = overrides.iter_mut().find(|(field, _)| field == &name) {
        *existing = value;
    } else {
        overrides.push((name, value));
    }
    true
}

fn set_nested_value(target: &mut SceneValue, path: &[ValuePathStep], value: SceneValue) -> bool {
    if path.is_empty() {
        *target = value;
        return true;
    }
    match (&mut *target, &path[0]) {
        (SceneValue::Array(values), ValuePathStep::Index(idx)) => {
            let Some(item) = values.to_mut().get_mut(*idx) else {
                return false;
            };
            set_nested_value(item, &path[1..], value)
        }
        (SceneValue::Object(fields), ValuePathStep::Field(name)) => {
            let Some((_field, item)) = fields
                .to_mut()
                .iter_mut()
                .find(|(field, _)| field.as_ref() == name)
            else {
                return false;
            };
            set_nested_value(item, &path[1..], value)
        }
        _ => false,
    }
}
