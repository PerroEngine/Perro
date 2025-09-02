use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    asset_io::load_asset,
    ast::{FurAnchor, FurElement, FurNode},
    ui_element::{BaseElement, BaseUIElement, UIElement},
    ui_elements::{
        ui_container::{BoxContainer, GridContainer},
        ui_panel::{CornerRadius, UIPanel},
    },
    ui_node::Ui, Color,
};

/// Parses a `.fur` file into a `Vec<FurNode>` AST
pub fn parse_fur_file(path: &str) -> Result<Vec<FurNode>, String> {
    let bytes = load_asset(path)
        .map_err(|e| format!("Failed to read .fur file {}: {}", path, e))?;
    let code = String::from_utf8(bytes)
        .map_err(|e| format!("Invalid UTF-8 in .fur file {}: {}", path, e))?;
    let mut parser = crate::parser::FurParser::new(&code)
        .map_err(|e| format!("Failed to init FurParser: {}", e))?;
    
    let ast = parser
        .parse()
        .map_err(|e| format!("Failed to parse .fur file {}: {}", path, e))?;


    Ok(ast)
}


/// Parse color string with optional opacity (e.g., "#FF0000/50" or "red/80")
pub fn parse_color_with_opacity(value: &str) -> Result<Color, String> {
    let mut parts = value.splitn(2, '/');
    let base = parts.next().unwrap();
    let opacity_part = parts.next();

    let mut color = if base.starts_with('#') {
        Color::from_hex(base).map_err(|e| format!("Invalid hex color '{}': {}", base, e))?
    } else {
        Color::from_preset(base).ok_or_else(|| format!("Unknown preset color '{}'", base))?
    };

    if let Some(opacity_str) = opacity_part {
        let opacity_percent = opacity_str.parse::<u8>()
            .map_err(|_| format!("Invalid opacity '{}'", opacity_str))?;
        if opacity_percent > 100 {
            return Err(format!("Opacity '{}' out of range 0-100", opacity_percent));
        }
        color.a = (opacity_percent as f32 / 100.0 * 255.0).round() as u8;
    }

    Ok(color)
}

/// Apply generic FurElement attributes to BaseUIElement
fn apply_base_attributes(base: &mut BaseUIElement, attributes: &HashMap<String, String>) {
    base.style_map.clear();

    // Set defaults
    base.size.x = 32.0;
    base.size.y = 32.0;
    base.transform.scale.x = 1.0;
    base.transform.scale.y = 1.0;

    for (key, value) in attributes {
        let value = value.trim();
        let is_percent = value.ends_with('%');
        let clean_val = value.trim_end_matches('%').trim();

        let parsed_val = clean_val.parse::<f32>().unwrap_or_else(|e| {
            eprintln!("Failed to parse attribute '{}': '{}': {}", key, clean_val, e);
            0.0
        });

        match key.as_str() {
            // Translation / Position
            "tx" => {
                if is_percent { base.style_map.insert("transform.position.x".into(), parsed_val); }
                else { base.transform.position.x = parsed_val; }
            }
            "ty" => {
                if is_percent { base.style_map.insert("transform.position.y".into(), parsed_val); }
                else { base.transform.position.y = parsed_val; }
            }

            // Scale
            "scl-x" => {
                if is_percent { base.style_map.insert("transform.scale.x".into(), parsed_val); }
                else { base.transform.scale.x = parsed_val; }
            }
            "scl-y" => {
                if is_percent { base.style_map.insert("transform.scale.y".into(), parsed_val); }
                else { base.transform.scale.y = parsed_val; }
            }
            "scl" => {
                if is_percent {
                    base.style_map.insert("transform.scale.x".into(), parsed_val);
                    base.style_map.insert("transform.scale.y".into(), parsed_val);
                } else {
                    base.transform.scale.x = parsed_val;
                    base.transform.scale.y = parsed_val;
                }
            }

            // Size
            "w" | "sz-x" => {
                if is_percent { base.style_map.insert("size.x".into(), parsed_val); }
                else { base.size.x = parsed_val; }
            }
            "h" | "sz-y" => {
                if is_percent { base.style_map.insert("size.y".into(), parsed_val); }
                else { base.size.y = parsed_val; }
            }
            "sz" => {
                if is_percent {
                    base.style_map.insert("size.x".into(), parsed_val);
                    base.style_map.insert("size.y".into(), parsed_val);
                } else {
                    base.size.x = parsed_val;
                    base.size.y = parsed_val;
                }
            }


            // Rotation
            "rot" => {
                if is_percent { base.style_map.insert("transform.rotation".into(), parsed_val); }
                else { base.transform.rotation = parsed_val; }
            }

            // Padding
            "pl" => base.padding.left = parsed_val,
            "pr" => base.padding.right = parsed_val,
            "pt" => base.padding.top = parsed_val,
            "pb" => base.padding.bottom = parsed_val,

            // Z-index
            "z" => base.z_index = parsed_val as i32,

            // Anchor
            "anchor" => base.anchor = match clean_val {
                "tl" => FurAnchor::TopLeft,
                "t" => FurAnchor::Top,
                "tr" => FurAnchor::TopRight,
                "l" => FurAnchor::Left,
                "c" => FurAnchor::Center,
                "r" => FurAnchor::Right,
                "bl" => FurAnchor::BottomLeft,
                "b" => FurAnchor::Bottom,
                "br" => FurAnchor::BottomRight,
                _ => FurAnchor::Center,
            },

            _ => {}
        }
    }
}



/// Converts a single FurElement into a UIElement (without children)
fn convert_fur_element_to_ui_element(fur_element: &FurElement) -> Option<UIElement> {
    match fur_element.tag_name.as_str() {
        "UI" => None,

        "Panel" => {
            let mut panel = UIPanel::default();
            panel.set_name(&fur_element.id);
            apply_base_attributes(&mut panel.base, &fur_element.attributes);

            // Panel-specific attributes
            if let Some(bg) = fur_element.attributes.get("bg") {
                if let Ok(color) = parse_color_with_opacity(bg) {
                    panel.props.background_color = Some(color);
                }
            }

            if let Some(border_c) = fur_element.attributes.get("border-c") {
                if let Ok(color) = parse_color_with_opacity(border_c) {
                    panel.props.border_color = Some(color);
                }
            }

            if let Some(border) = fur_element.attributes.get("border") {
                panel.props.border_thickness = border.parse::<f32>().unwrap_or(0.0);
            }

             // --- Corner radius handling ---
            let mut corner = CornerRadius::default();

            // Apply general "rounding" to all corners first
            if let Some(rounding) = fur_element.attributes.get("rounding") {
                let r = rounding.parse::<f32>().unwrap_or(0.0);
                corner.top_left = r;
                corner.top_right = r;
                corner.bottom_left = r;
                corner.bottom_right = r;
            }

            // Override individual corners if present
            if let Some(val) = fur_element.attributes.get("rounding-tl") { corner.top_left = val.parse::<f32>().unwrap_or(corner.top_left); }
            if let Some(val) = fur_element.attributes.get("rounding-tr") { corner.top_right = val.parse::<f32>().unwrap_or(corner.top_right); }
            if let Some(val) = fur_element.attributes.get("rounding-bl") { corner.bottom_left = val.parse::<f32>().unwrap_or(corner.bottom_left); }
            if let Some(val) = fur_element.attributes.get("rounding-br") { corner.bottom_right = val.parse::<f32>().unwrap_or(corner.bottom_right); }

            // Shorthand for sides
            if let Some(val) = fur_element.attributes.get("rounding-t") {
                let r = val.parse::<f32>().unwrap_or(0.0);
                corner.top_left = r;
                corner.top_right = r;
            }
            if let Some(val) = fur_element.attributes.get("rounding-b") {
                let r = val.parse::<f32>().unwrap_or(0.0);
                corner.bottom_left = r;
                corner.bottom_right = r;
            }
            if let Some(val) = fur_element.attributes.get("rounding-l") {
                let r = val.parse::<f32>().unwrap_or(0.0);
                corner.top_left = r;
                corner.bottom_left = r;
            }
            if let Some(val) = fur_element.attributes.get("rounding-r") {
                let r = val.parse::<f32>().unwrap_or(0.0);
                corner.top_right = r;
                corner.bottom_right = r;
            }

            // Finally assign to panel
            panel.props.corner_radius = corner;

            Some(UIElement::Panel(panel))
        }

        "Box" => {
            let mut container = BoxContainer::default();
            container.set_name(&fur_element.id);
            apply_base_attributes(&mut container.base, &fur_element.attributes);
            Some(UIElement::BoxContainer(container))
        }

        "Grid" => {
            let mut grid = GridContainer::default();
            grid.set_name(&fur_element.id);
            apply_base_attributes(&mut grid.base, &fur_element.attributes);
            Some(UIElement::GridContainer(grid))
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
            for (child_uuid, _) in &child_elements {
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
