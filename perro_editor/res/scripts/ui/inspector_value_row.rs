use crate::scripts_app_editor_app_rs as editor_app;
use crate::scripts_ui_editor_inspector_values_rs::InspectorValueRow;
use crate::scripts_ui_editor_ui_rs::{find_named, set_ui_display};
use perro_api::prelude::*;
use std::borrow::Cow;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InspectorRowTemplate {
    Generic,
    Vec2,
    Vec3,
    Quat,
    CustomStruct,
    Array,
    Enum,
    Bool,
    Color,
    NodeRef,
    AssetRef,
}

static INSPECTOR_ROW_NAMES: OnceLock<Mutex<Vec<InspectorRowNames>>> = OnceLock::new();
static INSPECTOR_ROW_TEMPLATES: OnceLock<Mutex<Vec<Option<InspectorRowTemplate>>>> =
    OnceLock::new();
const INSPECTOR_ROW_CLEANUP_LIMIT: usize = 512;

pub fn ensure_inspector_value_row<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
    row: Option<&InspectorValueRow>,
) {
    let names = inspector_row_names(idx);
    let template = inspector_row_template(row);
    if let Some(row_id) = find_named(ctx, &names.row) {
        if inspector_cached_row_template(idx) == Some(template)
            && find_named(ctx, &names.header).is_some()
            && find_named(ctx, &names.quat_mode).is_some()
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
        .load(inspector_row_scene(template).to_string())
    else {
        return;
    };
    let _ = ctx.run.Nodes().reparent(content_id, root_id);
    rename_value_row(ctx, root_id, idx);
    ensure_inspector_default_button(ctx, idx);
    set_cached_row_template(idx, Some(template));
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

fn ensure_inspector_default_button<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    idx: usize,
) {
    let button_name = format!("inspector_var_{idx}_default_button");
    if find_named(ctx, &button_name).is_some() {
        return;
    }
    let Some(parent) = inspector_value_row_inner(ctx, idx) else {
        return;
    };
    let button = ctx.run.Nodes().create::<UiButton>();
    let label = ctx.run.Nodes().create::<UiLabel>();
    let _ = ctx.run.Nodes().set_node_name(button, button_name);
    let _ = ctx
        .run
        .Nodes()
        .set_node_name(label, format!("inspector_var_{idx}_default_label"));
    let _ = ctx.run.Nodes().reparent(parent, button);
    let _ = ctx.run.Nodes().reparent(button, label);
    let _ = with_node_mut!(ctx.run, UiButton, button, |node| {
        node.layout.size = UiVector2::ratio(0.045, 0.62);
        node.visible = false;
        node.clicked_signals = vec![SignalID::from_string("editor_inspector_var_7")];
        node.style.fill = Color::from_hex("#3A3020").unwrap_or(node.style.fill);
        node.style.stroke = Color::from_hex("#D9A24A").unwrap_or(node.style.stroke);
        node.style.stroke_width = 1.0;
        node.style.corner_radius = 0.2;
        node.hover_style.fill = Color::from_hex("#4A3A24").unwrap_or(node.hover_style.fill);
        node.hover_style.stroke = Color::from_hex("#D9A24A").unwrap_or(node.hover_style.stroke);
        node.pressed_style.fill = Color::from_hex("#5A4328").unwrap_or(node.pressed_style.fill);
        node.pressed_style.stroke = Color::from_hex("#E2B45E").unwrap_or(node.pressed_style.stroke);
    });
    let _ = with_node_mut!(ctx.run, UiLabel, label, |node| {
        node.layout.size = UiVector2::ratio(1.0, 1.0);
        node.text = Cow::Borrowed("!");
        node.text_size_ratio = 0.48;
        node.color = Color::from_hex("#F0C96D").unwrap_or(node.color);
        node.input_enabled = false;
        node.mouse_filter = UiMouseFilter::Pass;
    });
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
    for idx in start..INSPECTOR_ROW_CLEANUP_LIMIT {
        let row_name = inspector_row_names(idx).row;
        if let Some(id) = find_named(ctx, &row_name) {
            let _ = ctx.run.Nodes().remove_node(id);
        }
        set_cached_row_template(idx, None);
    }
}

pub fn clear_inspector_value_rows<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) {
    for idx in 0..INSPECTOR_ROW_CLEANUP_LIMIT {
        let row_name = inspector_row_names(idx).row;
        if let Some(id) = find_named(ctx, &row_name) {
            let _ = ctx.run.Nodes().remove_node(id);
        }
        set_cached_row_template(idx, None);
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

fn inspector_row_template(row: Option<&InspectorValueRow>) -> InspectorRowTemplate {
    let Some(row) = row else {
        return InspectorRowTemplate::Generic;
    };
    match row.kind.as_str() {
        "Vec2" | "IVec2" | "UVec2" | "UnitVector2" => InspectorRowTemplate::Vec2,
        "Vec3" | "IVec3" | "UVec3" | "UnitVector3" => InspectorRowTemplate::Vec3,
        "Quat" => InspectorRowTemplate::Quat,
        "Enum" => InspectorRowTemplate::Enum,
        "Bool" => InspectorRowTemplate::Bool,
        "Color" => InspectorRowTemplate::Color,
        kind if kind.starts_with("Node") => InspectorRowTemplate::NodeRef,
        kind if kind.starts_with("Asset(") => InspectorRowTemplate::AssetRef,
        _ if row.addable => InspectorRowTemplate::Array,
        _ if row.expandable => InspectorRowTemplate::CustomStruct,
        _ => InspectorRowTemplate::Generic,
    }
}

fn inspector_row_scene(template: InspectorRowTemplate) -> &'static str {
    match template {
        InspectorRowTemplate::Generic => editor_app::INSPECTOR_VALUE_ROW_SCENE,
        InspectorRowTemplate::Vec2 => editor_app::INSPECTOR_VEC2_ROW_SCENE,
        InspectorRowTemplate::Vec3 => editor_app::INSPECTOR_VEC3_ROW_SCENE,
        InspectorRowTemplate::Quat => editor_app::INSPECTOR_QUAT_ROW_SCENE,
        InspectorRowTemplate::CustomStruct => editor_app::INSPECTOR_CUSTOM_STRUCT_ROW_SCENE,
        InspectorRowTemplate::Array => editor_app::INSPECTOR_ARRAY_ROW_SCENE,
        InspectorRowTemplate::Enum => editor_app::INSPECTOR_ENUM_ROW_SCENE,
        InspectorRowTemplate::Bool => editor_app::INSPECTOR_BOOL_ROW_SCENE,
        InspectorRowTemplate::Color => editor_app::INSPECTOR_COLOR_ROW_SCENE,
        InspectorRowTemplate::NodeRef => editor_app::INSPECTOR_NODE_REF_ROW_SCENE,
        InspectorRowTemplate::AssetRef => editor_app::INSPECTOR_ASSET_REF_ROW_SCENE,
    }
}

fn inspector_cached_row_template(idx: usize) -> Option<InspectorRowTemplate> {
    let cache = INSPECTOR_ROW_TEMPLATES.get_or_init(|| Mutex::new(Vec::new()));
    let Ok(guard) = cache.lock() else {
        return None;
    };
    guard.get(idx).copied().flatten()
}

fn set_cached_row_template(idx: usize, template: Option<InspectorRowTemplate>) {
    let cache = INSPECTOR_ROW_TEMPLATES.get_or_init(|| Mutex::new(Vec::new()));
    let Ok(mut guard) = cache.lock() else {
        return;
    };
    while guard.len() <= idx {
        guard.push(None);
    }
    guard[idx] = template;
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
            } else if let Some(component) = name
                .strip_prefix("inspector_value_")
                .and_then(|value| value.strip_suffix("_label"))
            {
                format!("inspector_var_{idx}_{component}_label")
            } else {
                return None;
            }
        }
    };
    Some(next)
}
