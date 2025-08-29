use indexmap::IndexMap;
use uuid::Uuid;

use crate::{
    asset_io::load_asset,
    ast::{FurAnchor, FurElement, FurNode, FurStyle},
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

    base.z_index = style.z_index;

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

            // FUR id is really a name
            panel.set_name(&fur_element.id);

            // Apply all shared fields
            apply_base_style(&mut panel.base, &fur_element.style);

            // Panel-specific props
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

/// Recursively converts a FurElement and all its descendants into flat list of (Uuid, UIElement),
/// generating fresh UUIDs for each element and setting up parent/child relationships.
fn convert_fur_element_to_ui_elements(
    fur_element: &FurElement,
    parent_uuid: Option<Uuid>,
) -> Vec<(Uuid, UIElement)> {
    // Try convert current element, skip if None (e.g. "UI" tag)
    let maybe_ui_element = convert_fur_element_to_ui_element(fur_element);
    if maybe_ui_element.is_none() {
        // For "UI" root element, just recurse into children without adding this node
        let mut results = Vec::new();
        for child_node in &fur_element.children {
            if let FurNode::Element(child_element) = child_node {
                results.extend(convert_fur_element_to_ui_elements(
                    child_element,
                    parent_uuid,
                ));
            }
        }
        return results;
    }

    // Generate a fresh UUID for this element
    let current_uuid = Uuid::new_v4();

    // Safe unwrap since we handled None above
    let mut ui_element = maybe_ui_element.unwrap();

    // Store the UUID in the element’s BaseUIElement
    ui_element.set_id(current_uuid);

    // Process children first to get their UUIDs
    let mut children_uuids = Vec::new();
    let mut results = Vec::new();

    for child_node in &fur_element.children {
        if let FurNode::Element(child_element) = child_node {
            let child_elements = convert_fur_element_to_ui_elements(
                child_element,
                Some(current_uuid),
            );

            // The first element in the results is the direct child
            if let Some((child_uuid, _)) = child_elements.first() {
                children_uuids.push(*child_uuid);
            }

            results.extend(child_elements);
        }
    }

    // Set up parent/child relationships
    ui_element.set_parent(parent_uuid);
    ui_element.set_children(children_uuids);

    // Insert this element at the beginning
    results.insert(0, (current_uuid, ui_element));

    results
}

/// Entry point to build UI elements in `Ui` from the root FurElements AST slice
pub fn build_ui_elements_from_fur(ui: &mut Ui, fur_elements: &[FurElement]) {
    // Clear existing elements
    ui.elements.clear();
    ui.root_ids.clear();

    // Build all elements with fresh UUIDs
    for fur_element in fur_elements {
        let elements = convert_fur_element_to_ui_elements(fur_element, None);
        for (uuid, ui_element) in elements {
            
            ui.elements.insert(uuid, ui_element);
            
        }
    }

    

    // Collect root UUIDs (elements with no parent)
    for (uuid, element) in &ui.elements {
        if element.get_parent().is_none() {
            ui.root_ids.push(*uuid);
        }
    }
}