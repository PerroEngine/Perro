use std::{collections::HashMap, time::Instant};
use uuid::Uuid;

use crate::{
    asset_io::load_asset, ast::{FurAnchor, FurElement, FurNode}, ui_element::{BaseElement, BaseUIElement, UIElement}, ui_elements::ui_container::{Alignment, BoxContainer, ContainerMode, CornerRadius, GridLayout, Layout, UIPanel}, ui_node::Ui, Color, Vector2
};

// =================== FILE PARSING ===================

/// Parses a `.fur` file into a `Vec<FurNode>` AST
pub fn parse_fur_file(path: &str) -> Result<Vec<FurNode>, String> {
    let bytes = load_asset(path)
        .map_err(|e| format!("Failed to read .fur file {}: {}", path, e))?;
    let code = String::from_utf8(bytes)
        .map_err(|e| format!("Invalid UTF-8 in .fur file {}: {}", path, e))?;

    let mut parser = crate::parser::FurParser::new(&code)
        .map_err(|e| format!("Failed to init FurParser: {}", e))?;

    let start = Instant::now();
    let ast = parser
        .parse()
        .map_err(|e| format!("Failed to parse .fur file {}: {}", path, e))?;
    println!("Parsing {} took: {:?}", path, start.elapsed());

    Ok(ast)
}

// =================== COLOR PARSING ===================

pub fn parse_color_with_opacity(value: &str) -> Result<Color, String> {
    let mut parts = value.splitn(2, '_');
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

// =================== BASE ATTRIBUTES ===================

fn parse_compound(value: &str) -> (Option<&str>, Option<&str>) {
    let parts: Vec<&str> = value.split(',').map(|p| p.trim()).collect();
    match parts.len() {
        0 => (None, None),
        1 => (Some(parts[0]), Some(parts[0])),
        _ => (Some(parts[0]), Some(parts[1])),
    }
}

fn apply_base_attributes(base: &mut BaseUIElement, attributes: &HashMap<String, String>) {
    base.style_map.clear();

    // Defaults
    base.size = Vector2 { x: 32.0, y: 32.0 };
    base.transform.scale = Vector2 { x: 1.0, y: 1.0 };
    base.transform.position = Vector2 { x: 0.0, y: 0.0 };
    base.transform.rotation = 0.0;
    base.pivot = Vector2 { x: 0.5, y: 0.5 };

    for (key, val) in attributes {
        let val = val.trim();

        match key.as_str() {
            // Pivot
            "pv" => {
                let (x, y) = parse_compound(val);
                if let Some(x_val) = x {
                    base.pivot.x = match x_val {
                        "tl" | "l" => 0.0, "t" | "c" => 0.5, "tr" | "r" => 1.0,
                        "bl" => 0.0, "b" => 0.5, "br" => 1.0,
                        _ => x_val.parse().unwrap_or(0.5)
                    };
                }
                if let Some(y_val) = y {
                    base.pivot.y = match y_val {
                        "tl" | "t" | "tr" => 1.0,
                        "l" | "c" | "r" => 0.5,
                        "bl" | "b" | "br" => 0.0,
                        _ => y_val.parse().unwrap_or(0.5)
                    };
                }
            }
            "pv-x" => base.pivot.x = val.parse().unwrap_or(base.pivot.x),
            "pv-y" => base.pivot.y = val.parse().unwrap_or(base.pivot.y),

            // Translation
            "tx" => if let Some(x_val) = parse_compound(val).0 {
                let parsed = x_val.trim_end_matches('%').parse().unwrap_or(0.0);
                if x_val.ends_with('%') { base.style_map.insert("transform.position.x".into(), parsed); }
                else { base.transform.position.x = parsed; }
            },
            "ty" => if let Some(y_val) = parse_compound(val).1 {
                let parsed = y_val.trim_end_matches('%').parse().unwrap_or(0.0);
                if y_val.ends_with('%') { base.style_map.insert("transform.position.y".into(), parsed); }
                else { base.transform.position.y = parsed; }
            },

            // Scale
            "scl" => {
                let (x_val, y_val) = parse_compound(val);
                if let Some(xv) = x_val {
                    let parsed = xv.trim_end_matches('%').parse().unwrap_or(1.0);
                    if xv.ends_with('%') { base.style_map.insert("transform.scale.x".into(), parsed); }
                    else { base.transform.scale.x = parsed; }
                }
                if let Some(yv) = y_val {
                    let parsed = yv.trim_end_matches('%').parse().unwrap_or(1.0);
                    if yv.ends_with('%') { base.style_map.insert("transform.scale.y".into(), parsed); }
                    else { base.transform.scale.y = parsed; }
                }
            }
            "scl-x" => {
                let parsed = val.trim_end_matches('%').parse().unwrap_or(base.transform.scale.x);
                if val.ends_with('%') { base.style_map.insert("transform.scale.x".into(), parsed); }
                else { base.transform.scale.x = parsed; }
            }
            "scl-y" => {
                let parsed = val.trim_end_matches('%').parse().unwrap_or(base.transform.scale.y);
                if val.ends_with('%') { base.style_map.insert("transform.scale.y".into(), parsed); }
                else { base.transform.scale.y = parsed; }
            }

            // Size
            "sz" => {
                let (x_val, y_val) = parse_compound(val);
                if let Some(xv) = x_val {
                    let parsed = xv.trim_end_matches('%').parse().unwrap_or(base.size.x);
                    if xv.ends_with('%') { base.style_map.insert("size.x".into(), parsed); }
                    else { base.size.x = parsed; }
                }
                if let Some(yv) = y_val {
                    let parsed = yv.trim_end_matches('%').parse().unwrap_or(base.size.y);
                    if yv.ends_with('%') { base.style_map.insert("size.y".into(), parsed); }
                    else { base.size.y = parsed; }
                }
            }
            "w" | "sz-x" => {
                let parsed = val.trim_end_matches('%').parse().unwrap_or(base.size.x);
                if val.ends_with('%') { base.style_map.insert("size.x".into(), parsed); }
                else { base.size.x = parsed; }
            }
            "h" | "sz-y" => {
                let parsed = val.trim_end_matches('%').parse().unwrap_or(base.size.y);
                if val.ends_with('%') { base.style_map.insert("size.y".into(), parsed); }
                else { base.size.y = parsed; }
            }

            // Rotation
            "rot" => {
                let parsed = val.trim_end_matches('%').parse().unwrap_or(base.transform.rotation);
                if val.ends_with('%') { base.style_map.insert("transform.rotation".into(), parsed); }
                else { base.transform.rotation = parsed; }
            }




            // Z-index
            "z" => base.z_index = val.parse().unwrap_or(base.z_index),

            // Anchor
            "anchor" => base.anchor = match val {
                "tl" => FurAnchor::TopLeft,
                "t"  => FurAnchor::Top,
                "tr" => FurAnchor::TopRight,
                "l"  => FurAnchor::Left,
                "c"  => FurAnchor::Center,
                "r"  => FurAnchor::Right,
                "bl" => FurAnchor::BottomLeft,
                "b"  => FurAnchor::Bottom,
                "br" => FurAnchor::BottomRight,
                _    => FurAnchor::Center,
            },

            _ => {}
        }
    }
}

fn parse_alignment(val: &str) -> Alignment {
    match val.to_lowercase().as_str() {
        "start" | "s" => Alignment::Start,
        "center" | "c" => Alignment::Center,
        "end" | "e" => Alignment::End,
        _ => Alignment::Center, // default fallback
    }
}

fn default_to_full_size(fur: &FurElement, element: &mut BaseUIElement) {
    let needs_default = !fur.attributes.contains_key("sz")
        && !fur.attributes.contains_key("sz-x")
        && !fur.attributes.contains_key("sz-y")
        && !fur.attributes.contains_key("w")
        && !fur.attributes.contains_key("h");

    if needs_default {
        element.style_map.insert("size.x".into(), 100.0);
        element.style_map.insert("size.y".into(), 100.0);
    }
}



// =================== ELEMENT CONVERSION ===================

fn convert_fur_element_to_ui_element(fur: &FurElement) -> Option<UIElement> {
    match fur.tag_name.as_str() {
        "UI" => {
        // UI is a full-size BoxContainer
        let mut container = BoxContainer::default();
        container.set_name(&fur.id);
        apply_base_attributes(&mut container.base, &fur.attributes);

        // Force 100% width and height
        container.base.style_map.insert("size.x".into(), 100.0);
        container.base.style_map.insert("size.y".into(), 100.0);

        Some(UIElement::BoxContainer(container))
        },

        "Box" => {
            let mut container = BoxContainer::default();
            container.set_name(&fur.id);

            apply_base_attributes(&mut container.base, &fur.attributes);

            default_to_full_size(fur, &mut container);

            Some(UIElement::BoxContainer(container))
        },
        "Panel" => {
            let mut panel = UIPanel::default();
            panel.set_name(&fur.id);
            apply_base_attributes(&mut panel.base, &fur.attributes);

            if let Some(bg) = fur.attributes.get("bg") {
                if let Ok(color) = parse_color_with_opacity(bg) { panel.props.background_color = Some(color); }
            }
            if let Some(border_c) = fur.attributes.get("border-c") {
                if let Ok(color) = parse_color_with_opacity(border_c) { panel.props.border_color = Some(color); }
            }
            if let Some(border) = fur.attributes.get("border") {
                panel.props.border_thickness = border.parse().unwrap_or(0.0);
            }

            // Corner radius
            let mut corner = CornerRadius::default();
            if let Some(rounding) = fur.attributes.get("rounding") {
                let parts: Vec<f32> = rounding.split(',').map(|p| p.trim().parse().unwrap_or(0.0)).collect();
                match parts.len() {
                    1 => { let r = parts[0]; corner.top_left = r; corner.top_right = r; corner.bottom_left = r; corner.bottom_right = r; }
                    2 => { let (t,b) = (parts[0], parts[1]); corner.top_left = t; corner.top_right = t; corner.bottom_left = b; corner.bottom_right = b; }
                    3 => { let (tl,tr_bl,br) = (parts[0], parts[1], parts[2]); corner.top_left=tl; corner.top_right=tr_bl; corner.bottom_left=tr_bl; corner.bottom_right=br; }
                    4 => { corner.top_left=parts[0]; corner.top_right=parts[1]; corner.bottom_left=parts[2]; corner.bottom_right=parts[3]; }
                    _ => {}
                }
            }

            // Individual corner overrides
            if let Some(val) = fur.attributes.get("rounding-tl") { corner.top_left = val.parse().unwrap_or(corner.top_left); }
            if let Some(val) = fur.attributes.get("rounding-tr") { corner.top_right = val.parse().unwrap_or(corner.top_right); }
            if let Some(val) = fur.attributes.get("rounding-bl") { corner.bottom_left = val.parse().unwrap_or(corner.bottom_left); }
            if let Some(val) = fur.attributes.get("rounding-br") { corner.bottom_right = val.parse().unwrap_or(corner.bottom_right); }
            // Sides
            if let Some(val) = fur.attributes.get("rounding-t") { let r = val.parse().unwrap_or(0.0); corner.top_left = r; corner.top_right = r; }
            if let Some(val) = fur.attributes.get("rounding-b") { let r = val.parse().unwrap_or(0.0); corner.bottom_left = r; corner.bottom_right = r; }
            if let Some(val) = fur.attributes.get("rounding-l") { let r = val.parse().unwrap_or(0.0); corner.top_left = r; corner.bottom_left = r; }
            if let Some(val) = fur.attributes.get("rounding-r") { let r = val.parse().unwrap_or(0.0); corner.top_right = r; corner.bottom_right = r; }

            panel.props.corner_radius = corner;
            Some(UIElement::Panel(panel))
        }
        "Layout" => {
                // Check if mode is "g" for Grid
                if let Some(mode_val) = fur.attributes.get("mode") {
                    if matches!(mode_val.as_str(), "g" | "G") {
                        // Build a GridLayout instead
                        let mut grid = GridLayout::default();
                        grid.set_name(&fur.id);
                        apply_base_attributes(&mut grid.base, &fur.attributes);
                        default_to_full_size(fur, &mut grid);

                        if let Some(val) = fur.attributes.get("cols").or(fur.attributes.get("col")) {
                            if let Ok(parsed) = val.parse::<usize>() { grid.cols = parsed; }
                        }

                        if let Some(val) = fur.attributes.get("gap") {
                        let parts: Vec<&str> = val.split(',').map(|s| s.trim()).collect();
                        match parts.len() {
                            1 => {
                                if let Ok(parsed) = parts[0].parse::<f32>() {
                                    grid.container.gap.x = parsed;
                                    grid.container.gap.y = parsed;
                                }
                            }
                            2 => {
                                if let (Ok(x), Ok(y)) = (parts[0].parse::<f32>(), parts[1].parse::<f32>()) {
                                    grid.container.gap.x = x;
                                    grid.container.gap.y = y;
                                }
                            }
                            _ => {} // ignore invalid formats
                        }
                    }

                    grid.container.align = fur
                    .attributes
                    .get("align")
                    .map(|v| parse_alignment(v))
                    .unwrap_or(Alignment::Center);


                        return Some(UIElement::GridLayout(grid));
                    }
                }

                // Otherwise, normal Layout (horizontal or vertical)
                let mut layout = Layout::default();
                layout.set_name(&fur.id);
                apply_base_attributes(&mut layout.base, &fur.attributes);
                default_to_full_size(fur, &mut layout);

                layout.container.mode = match fur.attributes.get("mode").map(|v| v.as_str()) {
                    Some("v") | Some("V") => ContainerMode::Vertical,
                    _ => ContainerMode::Horizontal, // default
                };

                if let Some(val) = fur.attributes.get("gap") {
                    if let Ok(parsed) = val.parse::<f32>() {
                        match layout.container.mode {
                            ContainerMode::Horizontal => layout.container.gap.x = parsed,
                            ContainerMode::Vertical => layout.container.gap.y = parsed,
                            _ => {}
                        }
                    }
                }

                layout.container.align = fur
                .attributes
                .get("align")
                .map(|v| parse_alignment(v))
                .unwrap_or(Alignment::Center);


                Some(UIElement::Layout(layout))
            }

            // Convenience constructors
            "HLayout" => {
                let mut layout = Layout::default();
                layout.set_name(&fur.id);
                layout.container.mode = ContainerMode::Horizontal;
                apply_base_attributes(&mut layout.base, &fur.attributes);
                default_to_full_size(fur, &mut layout);

                if let Some(val) = fur.attributes.get("gap") {
                    if let Ok(parsed) = val.parse::<f32>() { layout.container.gap.x = parsed; }
                }

                Some(UIElement::Layout(layout))
            }

            "VLayout" => {
                let mut layout = Layout::default();
                layout.set_name(&fur.id);
                layout.container.mode = ContainerMode::Vertical;
                apply_base_attributes(&mut layout.base, &fur.attributes);
                default_to_full_size(fur, &mut layout);

                if let Some(val) = fur.attributes.get("gap") {
                    if let Ok(parsed) = val.parse::<f32>() { layout.container.gap.y = parsed; }
                }

                Some(UIElement::Layout(layout))
            }

            // Explicit GridLayout node
            "Grid" => {
                let mut grid = GridLayout::default();
                grid.set_name(&fur.id);
                apply_base_attributes(&mut grid.base, &fur.attributes);
                default_to_full_size(fur, &mut grid);

                if let Some(val) = fur.attributes.get("cols").or(fur.attributes.get("col")) {
                    if let Ok(parsed) = val.parse::<usize>() { grid.cols = parsed; }
                }

               if let Some(val) = fur.attributes.get("gap") {
                let parts: Vec<&str> = val.split(',').map(|s| s.trim()).collect();
                match parts.len() {
                    1 => {
                        if let Ok(parsed) = parts[0].parse::<f32>() {
                            grid.container.gap.x = parsed;
                            grid.container.gap.y = parsed;
                        }
                    }
                    2 => {
                        if let (Ok(x), Ok(y)) = (parts[0].parse::<f32>(), parts[1].parse::<f32>()) {
                            grid.container.gap.x = x;
                            grid.container.gap.y = y;
                        }
                    }
                    _ => {} // ignore invalid formats
                }
            }

            grid.container.align = fur
            .attributes
            .get("align")
            .map(|v| parse_alignment(v))
            .unwrap_or(Alignment::Center);


                Some(UIElement::GridLayout(grid))
            }
                _ => unimplemented!("Element type {} not supported", fur.tag_name),
            }
}

// =================== RECURSIVE CONVERSION ===================

fn convert_fur_element_to_ui_elements(fur: &FurElement, parent_uuid: Option<Uuid>) -> Vec<(Uuid, UIElement)> {
    let mut results = Vec::with_capacity(fur.children.len() + 1);
    let maybe_ui = convert_fur_element_to_ui_element(fur);

    if maybe_ui.is_none() {
        for child_node in &fur.children {
            if let FurNode::Element(child) = child_node {
                results.extend(convert_fur_element_to_ui_elements(child, parent_uuid));
            }
        }
        return results;
    }

    let current_uuid = Uuid::new_v4();
    let mut ui_element = maybe_ui.unwrap();
    ui_element.set_id(current_uuid);

    let mut children_uuids = Vec::with_capacity(fur.children.len());
    for child_node in &fur.children {
        if let FurNode::Element(child) = child_node {
            let child_elements = convert_fur_element_to_ui_elements(child, Some(current_uuid));
            for (child_uuid, _) in &child_elements { children_uuids.push(*child_uuid); }
            results.extend(child_elements);
        }
    }

    ui_element.set_parent(parent_uuid);
    ui_element.set_children(children_uuids);

    results.push((current_uuid, ui_element)); // push instead of insert at 0
    results
}

// =================== BUILD UI ===================

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
