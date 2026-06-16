use crate::scripts_app_editor_app_rs as editor_app;
use crate::scripts_ui_editor_ui_rs::{find_named, set_ui_display};
use perro_api::prelude::*;
use std::sync::{Mutex, OnceLock};

type SelfNodeType = UiPanel;

#[State]
pub struct InspectorValueRowState {
    pub ready: bool,
}

lifecycle!({});
methods!({});

#[derive(Clone, Default)]
struct InspectorRowNames {
    row: String,
    header: String,
    inner: String,
    children: String,
    quat_mode: String,
}

static INSPECTOR_ROW_NAMES: OnceLock<Mutex<Vec<InspectorRowNames>>> = OnceLock::new();

pub fn ensure_inspector_value_row<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
) {
    let names = inspector_row_names(idx);
    if let Some(row_id) = find_named(ctx, &names.row) {
        if find_named(ctx, &names.header).is_some() && find_named(ctx, &names.quat_mode).is_some()
        {
            return;
        }
        let _ = ctx.run.Nodes().remove_node(row_id);
    }
    let Some(content_id) = find_named(ctx, "inspector_content") else {
        return;
    };
    let Ok(root_id) = ctx
        .run
        .Scene()
        .load(editor_app::INSPECTOR_VALUE_ROW_SCENE.to_string())
    else {
        return;
    };
    let _ = ctx.run.Nodes().reparent(content_id, root_id);
    rename_value_row(ctx, root_id, idx);
}

pub fn inspector_value_row_inner<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
) -> Option<NodeID> {
    let names = inspector_row_names(idx);
    if let Some(id) = find_named(ctx, &names.inner) {
        Some(id)
    } else {
        find_named(ctx, &names.row)
    }
}

pub fn inspector_value_row_children<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
) -> Option<NodeID> {
    find_named(ctx, &inspector_row_names(idx).children)
}

pub fn place_inspector_value_row<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
    parent_idx: Option<usize>,
) {
    let Some(row_id) = find_named(ctx, &inspector_row_names(idx).row) else {
        return;
    };
    let parent_id = parent_idx
        .and_then(|idx| inspector_value_row_children(ctx, idx))
        .or_else(|| find_named(ctx, "inspector_content"));
    let Some(parent_id) = parent_id else {
        return;
    };
    if ctx.run.Nodes().get_node_parent_id(row_id) == Some(parent_id) {
        return;
    }
    let _ = ctx.run.Nodes().reparent(parent_id, row_id);
}

pub fn apply_inspector_value_row_panel<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
    depth: usize,
    source: &str,
    has_children: bool,
) {
    let names = inspector_row_names(idx);
    let Some(row_id) = find_named(ctx, &names.row) else {
        return;
    };
    let Some(header_id) = find_named(ctx, &names.header) else {
        return;
    };
    let palette = [
        ("#23272D", "#4D84D1"),
        ("#2A2F36", "#4A525D"),
        ("#2A2F36", "#D95F5F"),
        ("#2A2F36", "#D98B3A"),
        ("#2A2F36", "#D9A24A"),
        ("#2A2F36", "#5EA868"),
    ];
    let group_depth = depth.saturating_sub(1);
    let (fill, stroke) = palette[group_depth % palette.len()];
    let (fill, stroke) = if source == "section" {
        ("#23272D", "#343A43")
    } else {
        (fill, stroke)
    };
    let script_stroke = if source == "script" && depth == 0 {
        "#4A525D"
    } else {
        stroke
    };
    let _ = with_node_mut!(ctx.run, UiPanel, row_id, |node| {
        node.style.fill = Color::from_hex(fill).unwrap_or(node.style.fill);
        node.style.stroke = Color::from_hex(script_stroke).unwrap_or(node.style.stroke);
        node.style.stroke_width = 1.0;
        node.style.corner_radius = 0.1;
    });
    let _ = with_node_mut!(ctx.run, UiPanel, header_id, |node| {
        node.style.fill = Color::TRANSPARENT;
        node.style.stroke = Color::TRANSPARENT;
        node.style.stroke_width = 0.0;
        node.style.corner_radius = 0.0;
    });
}

pub fn hide_inspector_value_rows_from<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    start: usize,
) {
    let mut idx = start;
    loop {
        let row_name = inspector_row_names(idx).row;
        if find_named(ctx, &row_name).is_none() {
            break;
        }
        set_ui_display(ctx, &row_name, false);
        idx += 1;
    }
}

fn inspector_row_names(idx: usize) -> InspectorRowNames {
    let cache = INSPECTOR_ROW_NAMES.get_or_init(|| Mutex::new(Vec::new()));
    let Ok(mut guard) = cache.lock() else {
        return build_inspector_row_names(idx);
    };
    while guard.len() <= idx {
        let next = guard.len();
        guard.push(build_inspector_row_names(next));
    }
    guard[idx].clone()
}

fn build_inspector_row_names(idx: usize) -> InspectorRowNames {
    InspectorRowNames {
        row: format!("inspector_var_row_{idx}"),
        header: format!("inspector_var_row_{idx}_header"),
        inner: format!("inspector_var_row_{idx}_inner"),
        children: format!("inspector_var_row_{idx}_children"),
        quat_mode: format!("inspector_var_{idx}_quat_mode"),
    }
}

fn rename_value_row<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root_id: NodeID,
    idx: usize,
) {
    let mut stack = vec![root_id];
    while let Some(id) = stack.pop() {
        if let Some(children) = ctx.run.Nodes().get_node_children_ids(id) {
            stack.extend(children);
        }
        let Some(name) = ctx
            .run
            .Nodes()
            .get_node_name(id)
            .map(|name| name.to_string())
        else {
            continue;
        };
        let next = match value_row_instance_name(&name, idx) {
            Some(next) => next,
            None => continue,
        };
        let _ = ctx.run.Nodes().set_node_name(id, next);
    }
}

fn value_row_instance_name(name: &str, idx: usize) -> Option<String> {
    let next = match name {
        "inspector_value_row" => format!("inspector_var_row_{idx}"),
        "inspector_value_row_header" => format!("inspector_var_row_{idx}_header"),
        "inspector_value_row_stack" => format!("inspector_var_row_{idx}_stack"),
        "inspector_value_row_inner" => format!("inspector_var_row_{idx}_inner"),
        "inspector_value_row_children" => format!("inspector_var_row_{idx}_children"),
        "inspector_value_name" => format!("inspector_var_{idx}_name"),
        "inspector_value_box" => format!("inspector_var_{idx}_value"),
        "inspector_value_check" => format!("inspector_var_{idx}_check"),
        "inspector_value_pick_button" => format!("inspector_var_{idx}_pick_button"),
        "inspector_value_pick_label" => format!("inspector_var_{idx}_pick_label"),
        "inspector_value_type" => format!("inspector_var_{idx}_type"),
        "inspector_value_add_button" => format!("inspector_var_{idx}_add_button"),
        "inspector_value_add_label" => format!("inspector_var_{idx}_add_button_label"),
        "inspector_value_remove_button" => format!("inspector_var_{idx}_remove_button"),
        "inspector_value_remove_label" => format!("inspector_var_{idx}_remove_button_label"),
        "inspector_value_color_swatch" => format!("inspector_var_{idx}_color_swatch"),
        "inspector_value_quat_button" => format!("inspector_var_{idx}_quat_button"),
        "inspector_value_quat_label" => format!("inspector_var_{idx}_quat_label"),
        "inspector_value_euler_button" => format!("inspector_var_{idx}_euler_button"),
        "inspector_value_euler_label" => format!("inspector_var_{idx}_euler_label"),
        "inspector_value_quat_mode" => format!("inspector_var_{idx}_quat_mode"),
        "inspector_value_dropdown" => format!("inspector_var_{idx}_dropdown"),
        _ => {
            if let Some(component) = name
                .strip_prefix("inspector_value_")
                .and_then(|value| value.strip_suffix("_box"))
            {
                format!("inspector_var_{idx}_{component}_box")
            } else {
                return None;
            }
        }
    };
    Some(next)
}
