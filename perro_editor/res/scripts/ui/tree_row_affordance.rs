use crate::scripts_app_editor_app_rs as editor_app;
use crate::scripts_ui_editor_ui_rs::find_named;
use perro_api::prelude::*;

type SelfNodeType = UiPanel;

lifecycle!({});
methods!({});

pub fn ensure_tree_row_affordances<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    row_name: &str,
    icon_color: &str,
) {
    let Some(row_id) = find_named(ctx, row_name) else {
        return;
    };
    let root_name = format!("{row_name}_affordance");
    if find_named(ctx, &root_name).is_some() {
        return;
    }
    let Ok(root_id) = ctx
        .run
        .Scene()
        .load(editor_app::TREE_ROW_AFFORDANCE_SCENE.to_string())
    else {
        return;
    };
    let _ = ctx.run.Nodes().reparent(row_id, root_id);
    rename_affordance(ctx, root_id, row_name);
    let icon_name = format!("{row_name}_icon");
    if let Some(icon_id) = find_named(ctx, &icon_name) {
        let _ = with_node_mut!(ctx.run, UiLabel, icon_id, |node| {
            node.color = Color::from_hex(icon_color).unwrap_or(node.color);
        });
    }
}

fn rename_affordance<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root_id: NodeID,
    row_name: &str,
) {
    let mut stack = vec![root_id];
    while let Some(id) = stack.pop() {
        if let Some(children) = ctx.run.Nodes().get_node_children_ids(id) {
            stack.extend(children);
        }
        let Some(name) = ctx.run.Nodes().get_node_name(id).map(|name| name.to_string()) else {
            continue;
        };
        let next = match name.as_str() {
            "tree_row_affordance" => format!("{row_name}_affordance"),
            "tree_row_indicator" => format!("{row_name}_indicator"),
            "tree_row_icon" => format!("{row_name}_icon"),
            _ => continue,
        };
        let _ = ctx.run.Nodes().set_node_name(id, next);
    }
}
