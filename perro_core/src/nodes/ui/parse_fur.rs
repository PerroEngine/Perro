use std::collections::HashMap;

use indexmap::IndexMap;

use crate::{
    asset_io::load_asset, ast::{FurElement, FurNode}, parser::FurParser, ui_element::{BaseElement, EdgeInsets, UIElement}, ui_elements::ui_panel::{CornerRadius, UIPanel}, ui_node::Ui, Transform2D
};

/// Parses a `.fur` file into a `Vec<FurNode>` AST
pub fn parse_fur_file(path: &str) -> Result<Vec<FurNode>, String> {
    let bytes = load_asset(path)
        .map_err(|e| format!("Failed to read .fur file {}: {}", path, e))?;

    let code = String::from_utf8(bytes)
        .map_err(|e| format!("Invalid UTF-8 in .fur file {}: {}", path, e))?;

    let mut parser = FurParser::new(&code)
        .map_err(|e| format!("Failed to init FurParser: {}", e))?;

    let ast = parser.parse()
        .map_err(|e| format!("Failed to parse .fur file {}: {}", path, e))?;

    Ok(ast)
}

/// Converts a single FurElement into a UIElement (without children)
fn convert_fur_element_to_ui_element(fur_element: &FurElement) -> Option<UIElement> {
    match fur_element.tag_name.as_str() {
        "UI" => {
            None
        }
        "Panel" => {
            let mut panel = UIPanel::default();

            // Set name/id
            panel.set_name(&fur_element.id);

            // Convert style from FurStyle to UIPanel's UIStyle
            if let Some(bg_color) = &fur_element.style.background_color {
                panel.style.background_color = Some(bg_color.clone());
            }

              if let Some(mod_color) = &fur_element.style.modulate {
                panel.style.modulate = Some(mod_color.clone());
            }

            // Corner radius conversion
            panel.style.corner_radius = CornerRadius {
                top_left: fur_element.style.corner_radius.top_left.map(|v| v as f32).unwrap_or(0.0),
                top_right: fur_element.style.corner_radius.top_right.map(|v| v as f32).unwrap_or(0.0),
                bottom_left: fur_element.style.corner_radius.bottom_left.map(|v| v as f32).unwrap_or(0.0),
                bottom_right: fur_element.style.corner_radius.bottom_right.map(|v| v as f32).unwrap_or(0.0),
            };

            // Margin conversion
            panel.margin = EdgeInsets {
                left: fur_element.style.margin.left.map(|v| v as f32).unwrap_or(0.0),
                right: fur_element.style.margin.right.map(|v| v as f32).unwrap_or(0.0),
                top: fur_element.style.margin.top.map(|v| v as f32).unwrap_or(0.0),
                bottom: fur_element.style.margin.bottom.map(|v| v as f32).unwrap_or(0.0),
            };

            // Padding conversion
            panel.padding = EdgeInsets {
                left: fur_element.style.padding.left.map(|v| v as f32).unwrap_or(0.0),
                right: fur_element.style.padding.right.map(|v| v as f32).unwrap_or(0.0),
                top: fur_element.style.padding.top.map(|v| v as f32).unwrap_or(0.0),
                bottom: fur_element.style.padding.bottom.map(|v| v as f32).unwrap_or(0.0),
            };

            // Translation conversion
            panel.transform.position.x = fur_element.style.translation.x.map(|v| v as f32).unwrap_or(0.0);
            panel.transform.position.y = fur_element.style.translation.y.map(|v| v as f32).unwrap_or(0.0);

            panel.transform.scale.x = fur_element.style.transform.scale.x as f32;
            panel.transform.scale.y = fur_element.style.transform.scale.y as f32;

            //size
            panel.size.x = fur_element.style.size.x as f32;
            panel.size.y = fur_element.style.size.y as f32;

            panel.style.border_thickness = fur_element.style.border.map(|v| v as f32).unwrap_or(0.0);
            if let Some(bd_color) = &fur_element.style.border_color {
                    panel.style.border_color = Some(bd_color.clone());
                }

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
                results.extend(convert_fur_element_to_ui_elements(child_element, parent_id.clone()));
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
            results.extend(convert_fur_element_to_ui_elements(child_element, Some(id.clone())));
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
    if let Some(element) = elements.get_mut(current_id) {
        let local = element.get_transform();

        let mut global = Transform2D::default();

        // Combine scales
        global.scale.x = parent_global.scale.x * local.scale.x;
        global.scale.y = parent_global.scale.y * local.scale.y;

        // Combine positions
        global.position.x =
            parent_global.position.x + (local.position.x * parent_global.scale.x);
        global.position.y =
            parent_global.position.y + (local.position.y * parent_global.scale.y);

        // Combine rotation
        global.rotation = parent_global.rotation + local.rotation;

        element.set_global_transform(global.clone());

        for child_id in element.get_children().to_vec() {
            update_global_transforms(elements, &child_id, &global);
        }
    }
}