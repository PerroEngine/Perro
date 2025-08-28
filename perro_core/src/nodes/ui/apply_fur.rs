use std::collections::HashMap;

use indexmap::IndexMap;

use crate::{
    asset_io::load_asset,
    ast::{FurAnchor, FurElement, FurNode, FurStyle},
    graphics::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH},
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

fn apply_base_style(base: &mut BaseUIElement, style: &FurStyle) {
    base.size.x = style.size.x as f32;
    base.size.y = style.size.y as f32;

    base.margin = EdgeInsets {
        left: style.margin.left.unwrap_or(0.0),
        right: style.margin.right.unwrap_or(0.0),
        top: style.margin.top.unwrap_or(0.0),
        bottom: style.margin.bottom.unwrap_or(0.0),
    };

    base.padding = EdgeInsets {
        left: style.padding.left.unwrap_or(0.0),
        right: style.padding.right.unwrap_or(0.0),
        top: style.padding.top.unwrap_or(0.0),
        bottom: style.padding.bottom.unwrap_or(0.0),
    };

    base.anchor = style.anchor;
    base.modulate = style.modulate.clone();

    base.transform.position.x = style.translation.x.unwrap_or(0.0) as f32;
    base.transform.position.y = style.translation.y.unwrap_or(0.0) as f32;
    base.transform.scale.x = style.transform.scale.x as f32;
    base.transform.scale.y = style.transform.scale.y as f32;
    base.transform.rotation = style.transform.rotation as f32;
}

/// Converts a single FurElement into a UIElement (without children)
fn convert_fur_element_to_ui_element(fur_element: &FurElement) -> Option<UIElement> {
    match fur_element.tag_name.as_str() {
        "UI" => None,
        "Panel" => {
            let mut panel = UIPanel::default();
            panel.set_name(&fur_element.id);

            // ✅ one call for all shared fields
            apply_base_style(&mut panel.base, &fur_element.style);

            // panel-specific props
            panel.props.background_color = fur_element.style.background_color.clone();
            panel.props.corner_radius = CornerRadius {
                top_left: fur_element.style.corner_radius.top_left.unwrap_or(0.0),
                top_right: fur_element.style.corner_radius.top_right.unwrap_or(0.0),
                bottom_left: fur_element.style.corner_radius.bottom_left.unwrap_or(0.0),
                bottom_right: fur_element.style.corner_radius.bottom_right.unwrap_or(0.0),
            };
            panel.props.border_color = fur_element.style.border_color.clone();
            panel.props.border_thickness = fur_element.style.border.unwrap_or(0.0);

            Some(UIElement::Panel(panel))
        }
        _ => {
            unimplemented!("Element type {} not supported yet", fur_element.tag_name);
        }
    }
}

/// Recursively converts a FurElement and all its descendants into flat list of (id, UIElement),
/// assigning parent and children links.
fn convert_fur_element_to_ui_elements(
    fur_element: &FurElement,
    parent_id: Option<String>,
) -> Vec<(String, UIElement)> {
    let id = fur_element.id.clone();

    // Try convert current element, skip if None (e.g. "UI" tag)
    let maybe_ui_element = convert_fur_element_to_ui_element(fur_element);
    if maybe_ui_element.is_none() {
        // For "UI" root element, just recurse into children without adding this node
        let mut results = Vec::new();
        for child_node in &fur_element.children {
            if let FurNode::Element(child_element) = child_node {
                results.extend(convert_fur_element_to_ui_elements(
                    child_element,
                    parent_id.clone(),
                ));
            }
        }
        return results;
    }

    // Safe unwrap since we handled None above
    let mut ui_element = maybe_ui_element.unwrap();

    // Collect children IDs and recurse
    let mut children_ids = Vec::new();
    let mut results = Vec::new();

    for child_node in &fur_element.children {
        if let FurNode::Element(child_element) = child_node {
            children_ids.push(child_element.id.clone());
            results.extend(convert_fur_element_to_ui_elements(
                child_element,
                Some(id.clone()),
            ));
        }
    }

    ui_element.set_parent(parent_id.clone());
    ui_element.set_children(children_ids);

    results.insert(0, (id, ui_element));

    results
}

/// Entry point to build UI elements in `Ui` from the root FurElements AST slice
pub fn build_ui_elements_from_fur(ui: &mut Ui, fur_elements: &[FurElement]) {
    // Step 1: Build all elements
    for fur_element in fur_elements {
        let elements = convert_fur_element_to_ui_elements(fur_element, None);
        for (id, ui_element) in elements {
            ui.elements.insert(id, ui_element);
        }
    }

    // Step 2: Find children of <UI> tags in the AST
    let mut root_ids = Vec::new();
    for fur_element in fur_elements {
        if fur_element.tag_name == "UI" {
            for child in &fur_element.children {
                if let FurNode::Element(child_element) = child {
                    root_ids.push(child_element.id.clone());
                }
            }
        }
    }

    // Step 3: Update global transforms starting from each <UI> child
    for root_id in root_ids {
        update_global_transforms(
            &mut ui.elements,
            &root_id,
            &Transform2D::default(), // identity transform
        );
    }
}

pub fn update_global_transforms(
    elements: &mut IndexMap<String, UIElement>,
    current_id: &str,
    parent_global: &Transform2D,
) {
    // First, figure out parent size without holding a mutable borrow
    let parent_size = {
        let parent_id = elements
            .get(current_id)
            .and_then(|el| el.get_parent().cloned());

        if let Some(parent_id) = parent_id {
            if let Some(parent) = elements.get(&parent_id) {
                *parent.get_size()
            } else {
                Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT)
            }
        } else {
            Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT)
        }
    };

    // Now borrow mutably
    if let Some(element) = elements.get_mut(current_id) {
        let mut local = element.get_transform().clone();

        let child_size = *element.get_size();
        let pivot = *element.get_pivot();

let (anchor_x, anchor_y) = match element.get_anchor() {
    // Corners
    FurAnchor::TopLeft => (
        -parent_size.x * 0.5 + child_size.x * pivot.x,
        parent_size.y * 0.5 - child_size.y * (1.0 - pivot.y),
    ),
    FurAnchor::TopRight => (
        parent_size.x * 0.5 - child_size.x * (1.0 - pivot.x),
        parent_size.y * 0.5 - child_size.y * (1.0 - pivot.y),
    ),
    FurAnchor::BottomLeft => (
        -parent_size.x * 0.5 + child_size.x * pivot.x,
        -parent_size.y * 0.5 + child_size.y * pivot.y,
    ),
    FurAnchor::BottomRight => (
        parent_size.x * 0.5 - child_size.x * (1.0 - pivot.x),
        -parent_size.y * 0.5 + child_size.y * pivot.y,
    ),

    // Edges
    FurAnchor::Top => (
        0.0,
        parent_size.y * 0.5 - child_size.y * (1.0 - pivot.y),
    ),
    FurAnchor::Bottom => (
        0.0,
        -parent_size.y * 0.5 + child_size.y * pivot.y,
    ),
    FurAnchor::Left => (
        -parent_size.x * 0.5 + child_size.x * pivot.x,
        0.0,
    ),
    FurAnchor::Right => (
        parent_size.x * 0.5 - child_size.x * (1.0 - pivot.x),
        0.0,
    ),

    // Center
    FurAnchor::Center => (
        0.0,
        0.0,
    ),
};

        // Apply anchor offset + user translation
        local.position.x = anchor_x + local.position.x;
        local.position.y = anchor_y + local.position.y;

        // --- Combine with parent transform ---
        let mut global = Transform2D::default();

        global.scale.x = parent_global.scale.x * local.scale.x;
        global.scale.y = parent_global.scale.y * local.scale.y;

        global.position.x =
            parent_global.position.x + (local.position.x * parent_global.scale.x);
        global.position.y =
            parent_global.position.y + (local.position.y * parent_global.scale.y);

        global.rotation = parent_global.rotation + local.rotation;

        element.set_global_transform(global.clone());

        // Recurse into children
        for child_id in element.get_children().to_vec() {
            update_global_transforms(elements, &child_id, &global);
        }
    }
}