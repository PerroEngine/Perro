use std::collections::HashMap;
use indexmap::IndexMap;
use uuid::Uuid;

use crate::{
    asset_io::load_asset,
    ast::{FurAnchor, FurElement, FurNode, FurStyle, ValueOrPercent},
    parser::FurParser,
    ui_element::{BaseElement, BaseUIElement, EdgeInsets, UIElement},
    ui_elements::ui_panel::{CornerRadius, UIPanel},
    ui_node::Ui,
    Transform2D, Vector2,
};

/// Parses a `.fur` file into a `Vec<FurNode>` AST
pub fn parse_fur_file(path: &str) -> Result<Vec<FurNode>, String> {
    let bytes = load_asset(path)
        .map_err(|e| format!("Failed to read .fur file {}: {}", path, e))?;

    let code = String::from_utf8(bytes)
        .map_err(|e| format!("Invalid UTF-8 in .fur file {}: {}", path, e))?;

    let mut parser = FurParser::new(&code)
        .map_err(|e| format!("Failed to init FurParser: {}", e))?;

    let ast = parser
        .parse()
        .map_err(|e| format!("Failed to parse .fur file {}: {}", path, e))?;

    Ok(ast)
}

/// Apply FurStyle to BaseUIElement, handling ValueOrPercent
fn apply_base_style(base: &mut BaseUIElement, style: &FurStyle) {
    base.style_map.clear();

    // --- Size ---
    if let Some(ValueOrPercent::Abs(val)) = style.size.x { base.size.x = val; }
    if let Some(ValueOrPercent::Percent(pct)) = style.size.x { base.style_map.insert("size.x".into(), pct); }

    if let Some(ValueOrPercent::Abs(val)) = style.size.y { base.size.y = val; }
    if let Some(ValueOrPercent::Percent(pct)) = style.size.y { base.style_map.insert("size.y".into(), pct); }

    // --- Translation (position) ---
    if let Some(ValueOrPercent::Abs(val)) = style.translation.x { base.transform.position.x = val; }
    if let Some(ValueOrPercent::Percent(pct)) = style.translation.x { base.style_map.insert("transform.position.x".into(), pct); }

    if let Some(ValueOrPercent::Abs(val)) = style.translation.y { base.transform.position.y = val; }
    if let Some(ValueOrPercent::Percent(pct)) = style.translation.y { base.style_map.insert("transform.position.y".into(), pct); }

    // --- Scale ---
    if let Some(ValueOrPercent::Abs(val)) = style.transform.scale.x { base.transform.scale.x = val; }
    if let Some(ValueOrPercent::Percent(pct)) = style.transform.scale.x { base.style_map.insert("transform.scale.x".into(), pct); }

    if let Some(ValueOrPercent::Abs(val)) = style.transform.scale.y { base.transform.scale.y = val; }
    if let Some(ValueOrPercent::Percent(pct)) = style.transform.scale.y { base.style_map.insert("transform.scale.y".into(), pct); }

    // --- Rotation ---
    if let Some(ValueOrPercent::Abs(val)) = style.transform.rotation { base.transform.rotation = val; }
    if let Some(ValueOrPercent::Percent(pct)) = style.transform.rotation { base.style_map.insert("transform.rotation".into(), pct); }

    // --- Margins ---
    base.margin.left = style.margin.left.map(|v| match v {
        ValueOrPercent::Abs(val) => val,
        ValueOrPercent::Percent(pct) => { base.style_map.insert("margin.left".into(), pct); 0.0 }
    }).unwrap_or(0.0);

    base.margin.right = style.margin.right.map(|v| match v {
        ValueOrPercent::Abs(val) => val,
        ValueOrPercent::Percent(pct) => { base.style_map.insert("margin.right".into(), pct); 0.0 }
    }).unwrap_or(0.0);

    base.margin.top = style.margin.top.map(|v| match v {
        ValueOrPercent::Abs(val) => val,
        ValueOrPercent::Percent(pct) => { base.style_map.insert("margin.top".into(), pct); 0.0 }
    }).unwrap_or(0.0);

    base.margin.bottom = style.margin.bottom.map(|v| match v {
        ValueOrPercent::Abs(val) => val,
        ValueOrPercent::Percent(pct) => { base.style_map.insert("margin.bottom".into(), pct); 0.0 }
    }).unwrap_or(0.0);

    // --- Padding ---
    base.padding.left = style.padding.left.map(|v| match v {
        ValueOrPercent::Abs(val) => val,
        ValueOrPercent::Percent(pct) => { base.style_map.insert("padding.left".into(), pct); 0.0 }
    }).unwrap_or(0.0);

    base.padding.right = style.padding.right.map(|v| match v {
        ValueOrPercent::Abs(val) => val,
        ValueOrPercent::Percent(pct) => { base.style_map.insert("padding.right".into(), pct); 0.0 }
    }).unwrap_or(0.0);

    base.padding.top = style.padding.top.map(|v| match v {
        ValueOrPercent::Abs(val) => val,
        ValueOrPercent::Percent(pct) => { base.style_map.insert("padding.top".into(), pct); 0.0 }
    }).unwrap_or(0.0);

    base.padding.bottom = style.padding.bottom.map(|v| match v {
        ValueOrPercent::Abs(val) => val,
        ValueOrPercent::Percent(pct) => { base.style_map.insert("padding.bottom".into(), pct); 0.0 }
    }).unwrap_or(0.0);

    // --- Z-index & anchor ---
    base.z_index = style.z_index;
    base.anchor = style.anchor;

}

/// Converts a single FurElement into a UIElement (without children)
fn convert_fur_element_to_ui_element(fur_element: &FurElement) -> Option<UIElement> {
    match fur_element.tag_name.as_str() {
        "UI" => None,
        "Panel" => {
            let mut panel = UIPanel::default();
            panel.set_name(&fur_element.id);

            // Apply all shared fields (handles ValueOrPercent)
            apply_base_style(&mut panel.base, &fur_element.style);

            // Panel-specific props
            panel.props.background_color = fur_element.style.background_color.clone();
            panel.props.corner_radius = CornerRadius {
                top_left: fur_element.style.corner_radius.top_left,
                top_right: fur_element.style.corner_radius.top_right,
                bottom_left: fur_element.style.corner_radius.bottom_left,
                bottom_right: fur_element.style.corner_radius.bottom_right,
            };
            panel.props.border_color = fur_element.style.border_color.clone();
            panel.props.border_thickness = fur_element.style.border;

            Some(UIElement::Panel(panel))
        }
        _ => unimplemented!("Element type {} not supported yet", fur_element.tag_name),
    }
}

/// Recursively converts a FurElement and all descendants into flat list of (Uuid, UIElement)
fn convert_fur_element_to_ui_elements(
    fur_element: &FurElement,
    parent_uuid: Option<Uuid>,
) -> Vec<(Uuid, UIElement)> {
    let maybe_ui_element = convert_fur_element_to_ui_element(fur_element);

    if maybe_ui_element.is_none() {
        // Skip root "UI" node, just recurse into children
        let mut results = Vec::new();
        for child_node in &fur_element.children {
            if let FurNode::Element(child_element) = child_node {
                results.extend(convert_fur_element_to_ui_elements(child_element, parent_uuid));
            }
        }
        return results;
    }

    let current_uuid = Uuid::new_v4();
    let mut ui_element = maybe_ui_element.unwrap();
    ui_element.set_id(current_uuid);

    let mut children_uuids = Vec::new();
    let mut results = Vec::new();

    for child_node in &fur_element.children {
        if let FurNode::Element(child_element) = child_node {
            let child_elements = convert_fur_element_to_ui_elements(child_element, Some(current_uuid));
            if let Some((child_uuid, _)) = child_elements.first() {
                children_uuids.push(*child_uuid);
            }
            results.extend(child_elements);
        }
    }

    ui_element.set_parent(parent_uuid);
    ui_element.set_children(children_uuids);

    results.insert(0, (current_uuid, ui_element));
    results
}

/// Builds UI elements in `Ui` from a slice of root FurElements AST
pub fn build_ui_elements_from_fur(ui: &mut Ui, fur_elements: &[FurElement]) {
    ui.elements.clear();
    ui.root_ids.clear();

    for fur_element in fur_elements {
        let elements = convert_fur_element_to_ui_elements(fur_element, None);
        for (uuid, ui_element) in elements {
            ui.elements.insert(uuid, ui_element);
        }
    }

    for (uuid, element) in &ui.elements {
        if element.get_parent().is_none() {
            ui.root_ids.push(*uuid);
        }
    }
}
