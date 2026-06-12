use crate::scripts_editor_animation_rs::*;
use crate::scripts_editor_assets_rs::*;
use crate::scripts_editor_ui_rs::*;
use crate::scripts_editor_viewport_rs::*;
use crate::scripts_main_rs::EditorState;
use perro_api::prelude::*;
use perro_api::scene::{SceneDoc, SceneFieldName, SceneValue};
use std::borrow::Cow;

pub const MAX_INSPECTOR_VALUE_ROWS: usize = 8;
const MAX_INSPECTOR_DEPTH: usize = 4;

#[derive(Clone)]
pub struct InspectorValueRow {
    pub path: Vec<ValuePathStep>,
    pub name: String,
    pub kind: String,
    pub value: String,
    pub editable: bool,
}

#[derive(Clone)]
pub enum ValuePathStep {
    Root(usize),
    Field(String),
    Index(usize),
}

pub fn inspector_script_var_rows(fields: &[(SceneFieldName, SceneValue)]) -> Vec<InspectorValueRow> {
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
        );
        if rows.len() >= MAX_INSPECTOR_VALUE_ROWS {
            break;
        }
    }
    rows
}

fn push_value_rows(
    rows: &mut Vec<InspectorValueRow>,
    name: &str,
    value: &SceneValue,
    path: &mut Vec<ValuePathStep>,
    depth: usize,
    max: usize,
) {
    if rows.len() >= max {
        return;
    }
    let composite = matches!(value, SceneValue::Array(_) | SceneValue::Object(_));
    rows.push(InspectorValueRow {
        path: path.clone(),
        name: format!("{}{}", "  ".repeat(depth), name),
        kind: scene_value_kind(value).to_string(),
        value: if composite {
            scene_value_summary(value)
        } else {
            scene_value_edit_text(value)
        },
        editable: !composite,
    });
    if depth >= MAX_INSPECTOR_DEPTH {
        return;
    }
    match value {
        SceneValue::Array(values) => {
            for (idx, item) in values.iter().enumerate() {
                path.push(ValuePathStep::Index(idx));
                push_value_rows(rows, &format!("[{idx}]"), item, path, depth + 1, max);
                path.pop();
                if rows.len() >= max {
                    break;
                }
            }
        }
        SceneValue::Object(fields) => {
            for (field, item) in fields.iter() {
                path.push(ValuePathStep::Field(field.as_ref().to_string()));
                push_value_rows(rows, field.as_ref(), item, path, depth + 1, max);
                path.pop();
                if rows.len() >= max {
                    break;
                }
            }
        }
        _ => {}
    }
}

pub fn scene_value_summary(value: &SceneValue) -> String {
    match value {
        SceneValue::Array(values) => format!("{} item(s)", values.len()),
        SceneValue::Object(fields) => format!("{} field(s)", fields.len()),
        _ => scene_value_edit_text(value),
    }
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
        Some(inspector_script_var_rows(node.script_vars.as_ref()))
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
        if !set_value_at_path(node.script_vars.to_mut(), &row.path, value) {
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

fn set_value_at_path(
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
