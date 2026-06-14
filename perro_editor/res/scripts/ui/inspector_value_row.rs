use crate::scripts_app_editor_app_rs as editor_app;
use crate::scripts_ui_editor_ui_rs::{find_named, set_ui_display};
use perro_api::prelude::*;

type SelfNodeType = UiPanel;

lifecycle!({});
methods!({});

pub fn ensure_inspector_value_row<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
) {
    let row_name = format!("inspector_var_row_{idx}");
    if find_named(ctx, &row_name).is_some() {
        return;
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
    if let Some(id) = find_named(ctx, &format!("inspector_var_row_{idx}_inner")) {
        Some(id)
    } else {
        find_named(ctx, &format!("inspector_var_row_{idx}"))
    }
}

pub fn apply_inspector_value_row_panel<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
    depth: usize,
    source: &str,
) {
    let Some(id) = find_named(ctx, &format!("inspector_var_row_{idx}")) else {
        return;
    };
    let palette = [
        ("#101826D9", "#233147FF"),
        ("#132033D9", "#2D415CFF"),
        ("#17283DD9", "#38516FFF"),
        ("#1B3045D9", "#436180FF"),
    ];
    let (fill, stroke) = if source == "section" {
        ("#0B1220E6", "#334155FF")
    } else {
        palette[depth.min(palette.len() - 1)]
    };
    let script_stroke = if source == "script" { "#4B5563FF" } else { stroke };
    let _ = with_node_mut!(ctx.run, UiPanel, id, |node| {
        node.style.fill = Color::from_hex(fill).unwrap_or(node.style.fill);
        node.style.stroke = Color::from_hex(script_stroke).unwrap_or(node.style.stroke);
        node.style.stroke_width = 1.0;
        node.style.corner_radius = 0.15;
    });
}

pub fn hide_inspector_value_rows_from<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    start: usize,
) {
    let mut idx = start;
    while find_named(ctx, &format!("inspector_var_row_{idx}")).is_some() {
        set_ui_display(ctx, &format!("inspector_var_row_{idx}"), false);
        idx += 1;
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
        let Some(name) = ctx.run.Nodes().get_node_name(id).map(|name| name.to_string()) else {
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
        "inspector_value_row_inner" => format!("inspector_var_row_{idx}_inner"),
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
