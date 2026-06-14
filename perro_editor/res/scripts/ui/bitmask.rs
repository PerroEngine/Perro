use crate::scripts_app_editor_app_rs as editor_app;
use crate::scripts_ui_editor_ui_rs::find_named;
use perro_api::prelude::*;

const BITMASK_TEMPLATE_ROOT: &str = "bitmask_root";

pub fn ensure_inspector_bitmask_grid<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
    parent: NodeID,
) {
    let grid_name = format!("inspector_var_{idx}_bitmask_grid");
    if find_named(ctx, &grid_name).is_some() {
        return;
    }
    let Ok(root_id) = ctx
        .run
        .Scene()
        .load(editor_app::INSPECTOR_BITMASK_SCENE.to_string())
    else {
        return;
    };
    let _ = ctx.run.Nodes().reparent(parent, root_id);
    rename_bitmask_instance(ctx, root_id, idx);
}

pub fn update_inspector_bitmask_grid<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
    mask: u32,
) {
    for bit in 1..=32 {
        let on = mask & (1_u32 << (bit - 1)) != 0;
        let Some(id) = find_named(ctx, &format!("inspector_var_{idx}_bit_{bit}")) else {
            continue;
        };
        let _ = with_node_mut!(ctx.run, UiButton, id, |node| {
            node.style.fill = Color::from_hex(if on { "#2563EBFF" } else { "#111827DD" })
                .unwrap_or(node.style.fill);
            node.style.stroke = Color::from_hex(if on { "#93C5FDFF" } else { "#334155FF" })
                .unwrap_or(node.style.stroke);
        });
    }
}

fn rename_bitmask_instance<API: ScriptAPI + ?Sized>(
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
        let next = match bitmask_instance_name(&name, idx) {
            Some(next) => next,
            None => continue,
        };
        let _ = ctx.run.Nodes().set_node_name(id, next);
    }
}

fn bitmask_instance_name(name: &str, idx: usize) -> Option<String> {
    if name == BITMASK_TEMPLATE_ROOT {
        return Some(format!("inspector_var_{idx}_bitmask_grid"));
    }
    if let Some(row) = name.strip_prefix("bitmask_row_") {
        return Some(format!("inspector_var_{idx}_bitmask_row_{row}"));
    }
    if name == "bitmask_actions" {
        return Some(format!("inspector_var_{idx}_bitmask_actions"));
    }
    if name == "bitmask_none" {
        return Some(format!("inspector_var_{idx}_bit_none"));
    }
    if name == "bitmask_all" {
        return Some(format!("inspector_var_{idx}_bit_all"));
    }
    if name == "bitmask_none_label" {
        return Some(format!("inspector_var_{idx}_bit_none_label"));
    }
    if name == "bitmask_all_label" {
        return Some(format!("inspector_var_{idx}_bit_all_label"));
    }
    if let Some(bit) = name
        .strip_prefix("bitmask_bit_")
        .and_then(|value| value.strip_suffix("_label"))
    {
        return Some(format!("inspector_var_{idx}_bit_{bit}_label"));
    }
    if let Some(bit) = name.strip_prefix("bitmask_bit_") {
        return Some(format!("inspector_var_{idx}_bit_{bit}"));
    }
    None
}
