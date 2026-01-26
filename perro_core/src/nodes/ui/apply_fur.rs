use indexmap::IndexMap;
use std::{borrow::Cow, collections::HashMap, time::Instant};
use crate::uid32::{Uid32, UIElementID};

use crate::{
    asset_io::load_asset,
    fur_ast::{FurAnchor, FurElement, FurNode},
    structs::Color,
    structs2d::Vector2,
    ui_element::{BaseElement, BaseUIElement, UIElement},
    ui_elements::{
        ui_container::{CornerRadius, UIPanel},
        ui_text::{TextFlow, UIText},
    },
    ui_node::UINode,
};

// =================== UTILITIES ===================

// OPT: Reuse static field names to avoid tiny heap allocs
const SIZE_X: &str = "size.x";
const SIZE_Y: &str = "size.y";
const SCALE_X: &str = "transform.scale.x";
const SCALE_Y: &str = "transform.scale.y";
const POS_X: &str = "transform.position.x";
const POS_Y: &str = "transform.position.y";
const ROT: &str = "transform.rotation";

// Small inline splitters for < 3 elements — avoid Vec allocs
fn split2<'a>(val: &'a str, sep: char) -> (Option<&'a str>, Option<&'a str>) {
    let mut iter = val.split(sep).map(str::trim);
    (iter.next(), iter.next())
}

// Compact numeric parse with fallback and optional '%' handling
fn parse_f32_percent(v: &str, default: f32) -> (f32, bool) {
    let has_pct = v.ends_with('%');
    let trimmed = v.trim_end_matches('%');
    (trimmed.parse::<f32>().unwrap_or(default), has_pct)
}

// Parse opacity value: supports 0-1 range or percentage (e.g., "0.5" or "50%")
fn parse_opacity(v: &str) -> Option<f32> {
    let trimmed = v.trim();
    if trimmed.ends_with('%') {
        let num_str = &trimmed[..trimmed.len() - 1];
        num_str.parse::<f32>().ok().map(|n| (n / 100.0).clamp(0.0, 1.0))
    } else {
        trimmed.parse::<f32>().ok().map(|n| n.clamp(0.0, 1.0))
    }
}

// =================== FILE PARSING ===================

use std::sync::RwLock;
use once_cell::sync::Lazy;

// Global registry for statically compiled FUR (used in release mode)
static STATIC_FUR_MAP: Lazy<RwLock<Option<&'static HashMap<&'static str, &'static [FurElement]>>>> = 
    Lazy::new(|| RwLock::new(None));

/// Set the static FUR map (called by runtime at startup in release mode)
pub fn set_static_fur_map(map: &'static HashMap<&'static str, &'static [FurElement]>) {
    *STATIC_FUR_MAP.write().unwrap() = Some(map);
}

/// Try to load FUR elements, checking static assets first, then parsing from disk/BRK
fn try_load_fur_elements(path: &str) -> Result<Vec<FurElement>, String> {
    // First: Check static FUR map (release mode)
    if let Ok(guard) = STATIC_FUR_MAP.read() {
        if let Some(fur_map) = *guard {
            if let Some(elements) = fur_map.get(path) {
                return Ok(elements.to_vec());
            }
        }
    }
    
    // Fallback: Parse from disk/BRK (dev mode)
    let ast = parse_fur_file(path)?;
    let elements: Vec<FurElement> = ast
        .into_iter()
        .filter_map(|node| {
            if let FurNode::Element(elem) = node {
                Some(elem)
            } else {
                None
            }
        })
        .collect();
    
    Ok(elements)
}

pub fn parse_fur_file(path: &str) -> Result<Vec<FurNode>, String> {
    let bytes =
        load_asset(path).map_err(|e| format!("Failed to read .fur file {}: {}", path, e))?;

    let code = String::from_utf8_lossy(&bytes);
    let mut parser =
        crate::parser::FurParser::new(&code).map_err(|e| format!("Init parser: {}", e))?;

    let _start = Instant::now();
    let ast = parser
        .parse()
        .map_err(|e| format!("Parse fail {}: {}", path, e))?;

    Ok(ast)
}

// =================== COLOR PARSING ===================

pub fn parse_color_with_opacity(value: &str) -> Result<Color, String> {
    let (base, opacity_part) = match value.split_once('_') {
        Some((b, o)) => (b, Some(o)),
        None => (value, None),
    };

    let mut color = if let Some(hex) = base.strip_prefix('#') {
        Color::from_hex(hex).map_err(|e| format!("Invalid hex '{}': {}", base, e))?
    } else {
        Color::from_preset(base).ok_or_else(|| format!("Unknown preset color '{}'", base))?
    };

    if let Some(op) = opacity_part {
        let p = op
            .parse::<u8>()
            .map_err(|_| format!("Bad opacity '{}'", op))?;
        if p > 100 {
            return Err(format!("Opacity '{}' out of 0–100", p));
        }
        color.a = ((p as f32 / 100.0) * 255.0) as u8;
    }

    Ok(color)
}

// =================== BASE ATTRIBUTES ===================

fn parse_compound(value: &str) -> (Option<&str>, Option<&str>) {
    split2(value, ',')
}

fn apply_base_attributes(
    base: &mut BaseUIElement,
    attrs: &HashMap<Cow<'static, str>, Cow<'static, str>>,
) {
    base.style_map.clear();
    // Pre-allocate style_map capacity based on number of attributes (heuristic: ~30% will be style attributes)
    base.style_map.reserve(attrs.len() / 3);

    // OPT: static defaults cached in BaseUIElement::default() as well
    base.size = Vector2::new(32.0, 32.0);
    base.transform.scale = Vector2::new(1.0, 1.0);
    base.transform.position = Vector2::new(0.0, 0.0);
    base.transform.rotation = 0.0;
    base.pivot = Vector2::new(0.5, 0.5);

    for (key, val) in attrs.iter() {
        let v = val.trim();
        match key.as_ref() {
            // Pivot
            "pv" => {
                let (x, y) = parse_compound(v);
                if let Some(xv) = x {
                    base.pivot.x = match xv {
                        "tl" | "l" => 0.0,
                        "t" | "c" => 0.5,
                        "tr" | "r" => 1.0,
                        "bl" => 0.0,
                        "b" => 0.5,
                        "br" => 1.0,
                        _ => xv.parse().unwrap_or(0.5),
                    };
                }
                if let Some(yv) = y {
                    base.pivot.y = match yv {
                        "tl" | "t" | "tr" => 1.0,
                        "l" | "c" | "r" => 0.5,
                        "bl" | "b" | "br" => 0.0,
                        _ => yv.parse().unwrap_or(0.5),
                    };
                }
            }
            "pv-x" => base.pivot.x = v.parse().unwrap_or(base.pivot.x),
            "pv-y" => base.pivot.y = v.parse().unwrap_or(base.pivot.y),

            "tx" => {
                let (n, pct) = parse_f32_percent(v, 0.0);
                if pct {
                    base.style_map.insert(POS_X.to_string(), n);
                } else {
                    base.transform.position.x = n;
                }
            }
            "ty" => {
                let (n, pct) = parse_f32_percent(v, 0.0);
                if pct {
                    base.style_map.insert(POS_Y.to_string(), n);
                } else {
                    base.transform.position.y = n;
                }
            }
            "scl" => {
                let (x, y) = parse_compound(v);
                if let Some(xv) = x {
                    let (f, pct) = parse_f32_percent(xv, 1.0);
                    if pct {
                        base.style_map.insert(SCALE_X.to_string(), f);
                    } else {
                        base.transform.scale.x = f;
                    }
                }
                if let Some(yv) = y {
                    let (f, pct) = parse_f32_percent(yv, 1.0);
                    if pct {
                        base.style_map.insert(SCALE_Y.to_string(), f);
                    } else {
                        base.transform.scale.y = f;
                    }
                }
            }

            "sz" => {
                let (x, y) = parse_compound(v);
                // If only one value provided, apply to both x and y
                let (x_val, y_val) = if y.is_none() && x.is_some() {
                    (x, x) // Single value applies to both
                } else {
                    (x, y) // Two values or none
                };
                
                if let Some(xv) = x_val {
                    let (f, pct) = parse_f32_percent(xv, base.size.x);
                    if pct {
                        base.style_map.insert(SIZE_X.to_string(), f);
                    } else {
                        // Store absolute values in style_map with a marker (> 10000) to distinguish from percentages
                        // This allows the layout system to know it's an explicit absolute size
                        base.size.x = f;
                        base.style_map.insert(SIZE_X.to_string(), 10000.0 + f);
                    }
                }
                if let Some(yv) = y_val {
                    let (f, pct) = parse_f32_percent(yv, base.size.y);
                    if pct {
                        base.style_map.insert(SIZE_Y.to_string(), f);
                    } else {
                        // Store absolute values in style_map with a marker (> 10000) to distinguish from percentages
                        // This allows the layout system to know it's an explicit absolute size
                        base.size.y = f;
                        base.style_map.insert(SIZE_Y.to_string(), 10000.0 + f);
                    }
                }
            }

            "w" | "sz-x" => {
                if v.trim().eq_ignore_ascii_case("auto") {
                    // Use -1.0 as sentinel value for auto-sizing
                    base.style_map.insert(SIZE_X.to_string(), -1.0);
                } else {
                    let (f, pct) = parse_f32_percent(v, base.size.x);
                    if pct {
                        base.style_map.insert(SIZE_X.to_string(), f);
                    } else {
                        // Store absolute values in style_map with a marker (> 10000) to distinguish from percentages
                        // This allows the layout system to know it's an explicit absolute size
                        base.size.x = f;
                        base.style_map.insert(SIZE_X.to_string(), 10000.0 + f);
                    }
                }
            }
            "h" | "sz-y" => {
                if v.trim().eq_ignore_ascii_case("auto") {
                    // Use -1.0 as sentinel value for auto-sizing
                    base.style_map.insert(SIZE_Y.to_string(), -1.0);
                } else {
                    let (f, pct) = parse_f32_percent(v, base.size.y);
                    if pct {
                        base.style_map.insert(SIZE_Y.to_string(), f);
                    } else {
                        // Store absolute values in style_map with a marker (> 10000) to distinguish from percentages
                        // This allows the layout system to know it's an explicit absolute size
                        base.size.y = f;
                        base.style_map.insert(SIZE_Y.to_string(), 10000.0 + f);
                    }
                }
            }

            "rot" => {
                let (f, pct) = parse_f32_percent(v, base.transform.rotation);
                if pct {
                    base.style_map.insert(ROT.to_string(), f);
                } else {
                    base.transform.rotation = f;
                }
            }

            "z" => base.z_index = v.parse().unwrap_or(base.z_index),
            "anchor" => {
                base.anchor = match v {
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
                }
            }
            "visible" => {
                base.visible = match v.to_lowercase().as_str() {
                    "true" | "1" | "yes" => true,
                    "false" | "0" | "no" => false,
                    _ => v.parse().unwrap_or(true), // Default to true if unparseable
                }
            }
            _ => {}
        }
    }
}

// =================== ELEMENT CONVERSION ===================

fn convert_fur_element_to_ui_element(fur: &FurElement) -> Option<UIElement> {
    let tag = fur.tag_name.as_ref();

    match tag {
        "Panel" => {
            let mut panel = UIPanel::default();
            panel.set_name(&fur.id);
            apply_base_attributes(&mut panel.base, &fur.attributes);

            if let Some(bg) = fur.attributes.get("bg") {
                if let Ok(c) = parse_color_with_opacity(bg) {
                    panel.props.background_color = Some(c);
                }
            }
            if let Some(c) = fur.attributes.get("border-c") {
                if let Ok(c) = parse_color_with_opacity(c) {
                    panel.props.border_color = Some(c);
                }
            }
            if let Some(b) = fur.attributes.get("border") {
                panel.props.border_thickness = b.parse().unwrap_or(0.0);
            }

            if let Some(opacity_str) = fur.attributes.get("opacity") {
                if let Some(opacity) = parse_opacity(opacity_str) {
                    panel.props.opacity = opacity;
                }
            }

            let mut corner = CornerRadius::default();

            // Helper to parse a single float value
            fn parse_val(v: Option<&std::borrow::Cow<'_, str>>) -> Option<f32> {
                v.and_then(|s| s.trim().parse().ok())
            }

            // Step 1: base rounding list (like "rounding: 1,2,3,4")
            if let Some(value) = fur.attributes.get("rounding") {
                let mut vals = [0.0; 4];
                for (i, v) in value.split(',').map(str::trim).take(4).enumerate() {
                    vals[i] = v.parse().unwrap_or(0.0);
                }

                match value.split(',').count() {
                    0 | 1 => corner = CornerRadius::uniform(vals[0]),
                    2 => {
                        corner.top_left = vals[0];
                        corner.top_right = vals[0];
                        corner.bottom_left = vals[1];
                        corner.bottom_right = vals[1];
                    }
                    3 => {
                        corner.top_left = vals[0];
                        corner.top_right = vals[1];
                        corner.bottom_left = vals[1];
                        corner.bottom_right = vals[2];
                    }
                    4 => {
                        corner.top_left = vals[0];
                        corner.top_right = vals[1];
                        corner.bottom_left = vals[2];
                        corner.bottom_right = vals[3];
                    }
                    _ => {}
                }
            }

            // Step 2: directional overrides (t, b, l, r)
            if let Some(v) = parse_val(fur.attributes.get("rounding-t")) {
                corner.top_left = v;
                corner.top_right = v;
            }
            if let Some(v) = parse_val(fur.attributes.get("rounding-b")) {
                corner.bottom_left = v;
                corner.bottom_right = v;
            }
            if let Some(v) = parse_val(fur.attributes.get("rounding-l")) {
                corner.top_left = v;
                corner.bottom_left = v;
            }
            if let Some(v) = parse_val(fur.attributes.get("rounding-r")) {
                corner.top_right = v;
                corner.bottom_right = v;
            }

            // Step 3: individual corner overrides (tl, tr, bl, br) – highest priority
            if let Some(v) = parse_val(fur.attributes.get("rounding-tl")) {
                corner.top_left = v;
            }
            if let Some(v) = parse_val(fur.attributes.get("rounding-tr")) {
                corner.top_right = v;
            }
            if let Some(v) = parse_val(fur.attributes.get("rounding-bl")) {
                corner.bottom_left = v;
            }
            if let Some(v) = parse_val(fur.attributes.get("rounding-br")) {
                corner.bottom_right = v;
            }

            panel.props.corner_radius = corner;
            Some(UIElement::Panel(panel))
        }

        "Text" => {
            let mut text = UIText::default();
            text.set_name(&fur.id);
            apply_base_attributes(&mut text.base, &fur.attributes);

            // Extract text content from children and trim whitespace
            // Optimized: collect directly into String without intermediate Vec
            let mut text_content = String::new();
            for n in &fur.children {
                if let FurNode::Text(s) = n {
                    text_content.push_str(s.as_ref());
                }
            }
            
            // Trim the text content to remove any leading/trailing whitespace
            text.props.content = text_content.trim().to_string();

            if let Some(fs) = fur
                .attributes
                .get("fsz")
                .or(fur.attributes.get("font-size"))
            {
                text.props.font_size = fs.parse().unwrap_or(text.props.font_size);
            }

            // Parse font specification (file path or system font name)
            if let Some(font_spec) = fur.attributes.get("font") {
                let font_str = font_spec.trim();
                if !font_str.is_empty() {
                    text.props.font = Some(font_str.to_string());
                }
            }

            // Parse text flow alignment (how text flows relative to anchor point)
            // align=start: left alignment (text starts at anchor, flows right)
            // align=center: text is centered on anchor
            // align=end: right alignment (text ends at anchor, flows left)
            if let Some(align_str) = fur.attributes.get("align") {
                text.props.align = match align_str.as_ref() {
                    "start" | "s" => TextFlow::Start,
                    "center" | "c" => TextFlow::Center,
                    "end" | "e" => TextFlow::End,
                    // Backward compatibility: map old values
                    "left" | "l" | "top" | "t" => TextFlow::Start,
                    "right" | "r" | "bottom" | "b" => TextFlow::End,
                    _ => TextFlow::Center,
                };
            }
            
            // Backward compatibility: support old align-h and align-v
            // These are deprecated but still work for migration
            if let Some(align_str) = fur.attributes.get("align-h") {
                let align = match align_str.as_ref() {
                    "left" | "l" => TextFlow::Start,
                    "center" | "c" => TextFlow::Center,
                    "right" | "r" => TextFlow::End,
                    _ => TextFlow::Center,
                };
                text.props.align = align; // Use horizontal alignment as primary
            }
            // Note: align-v is deprecated and no longer used - vertical alignment is always Center

            Some(UIElement::Text(text))
        }

        _ => {
            println!("Warning: Unsupported element '{}'", tag);
            None
        }
    }
}

// =================== RECURSIVE CONVERSION ===================

fn convert_fur_element_to_ui_elements(
    fur: &FurElement,
    parent_uuid: Option<UIElementID>,
) -> Vec<(UIElementID, UIElement)> {
    convert_fur_element_to_ui_elements_with_includes(fur, parent_uuid, &mut std::collections::HashSet::new())
}

fn convert_fur_element_to_ui_elements_with_includes(
    fur: &FurElement,
    parent_uuid: Option<UIElementID>,
    included_paths: &mut std::collections::HashSet<String>,
) -> Vec<(UIElementID, UIElement)> {
    // Handle Include elements
    if fur.tag_name.eq_ignore_ascii_case("Include") {
        if let Some(path) = fur.attributes.get("path") {
            let path_str = path.as_ref();
            // Prevent circular includes
            if included_paths.contains(path_str) {
                eprintln!("⚠️ Circular include detected for path: {}. Skipping.", path_str);
                return Vec::new();
            }
            
            // Store path string for later removal
            let path_string = path.to_string();
            included_paths.insert(path_string.clone());
            
            // Check if Include has a visible attribute
            let include_visible = fur.attributes.get("visible")
                .map(|v| {
                    match v.to_lowercase().as_str() {
                        "true" | "1" | "yes" => true,
                        "false" | "0" | "no" => false,
                        _ => v.parse().unwrap_or(true),
                    }
                })
                .unwrap_or(true); // Default to visible if not specified
            
            // Load the included FUR file
            // Try to get pre-parsed elements from static assets first (release mode),
            // then fall back to parsing from disk/BRK (dev mode)
            let elements: Vec<FurElement> = match try_load_fur_elements(path_str) {
                Ok(elems) => elems,
                Err(e) => {
                    eprintln!("⚠️ Failed to load included FUR file {}: {}", path_str, e);
                    included_paths.remove(&path_string);
                    return Vec::new();
                }
            };
            
            let mut results = Vec::new();
            // Process all root elements from the included file
            for elem in &elements {
                let mut included_results = convert_fur_element_to_ui_elements_with_includes(
                    elem,
                    parent_uuid,
                    included_paths,
                );
                
                // Apply visibility from Include tag ONLY to the root element
                // Children will inherit visibility through the parent chain check in is_effectively_visible
                if !include_visible {
                    if let Some((_, root_element)) = included_results.first_mut() {
                        root_element.set_visible(false);
                    }
                }
                
                results.extend(included_results);
            }
            included_paths.remove(&path_string);
            return results;
        } else {
            eprintln!("⚠️ Include element missing 'path' attribute. Skipping.");
            return Vec::new();
        }
    }

    let Some(mut ui) = convert_fur_element_to_ui_element(fur) else {
        return fur
            .children
            .iter()
            .filter_map(|n| {
                if let FurNode::Element(e) = n {
                    Some(convert_fur_element_to_ui_elements_with_includes(e, parent_uuid, included_paths))
                } else {
                    None
                }
            })
            .flatten()
            .collect();
    };

    let id = UIElementID::new();
    ui.set_id(id);
    ui.set_parent(parent_uuid);

    let mut results = Vec::with_capacity(fur.children.len() + 1);
    let mut children = Vec::with_capacity(fur.children.len());

    // Panels and Buttons are now fully compositional - they only accept explicit child elements
    // Text nodes are IGNORED - you must use [Text] elements explicitly
    for child in &fur.children {
        if let FurNode::Element(e) = child {
            let child_nodes = convert_fur_element_to_ui_elements_with_includes(e, Some(id), included_paths);
            if let Some((cid, _)) = child_nodes.first() {
                children.push(*cid);
            }
            results.extend(child_nodes);
        }
        // Text nodes (FurNode::Text) are ignored - use [Text] elements instead
    }

    ui.set_children(children);
    results.insert(0, (id, ui));
    results
}

// =================== BUILD UI ===================

pub fn build_ui_elements_from_fur(ui: &mut UINode, elems: &[FurElement]) {
    let elements = ui
        .elements
        .get_or_insert_with(|| IndexMap::with_capacity(elems.len()));
    elements.clear();

    let root_ids = ui
        .root_ids
        .get_or_insert_with(|| Vec::with_capacity(elems.len()));
    root_ids.clear();

    for el in elems {
        let flat = convert_fur_element_to_ui_elements(el, None);
        for (uuid, e) in flat {
            if e.get_parent().is_nil() {
                root_ids.push(uuid);
            }
            elements.insert(uuid, e);
        }
    }

    // Store element count before dropping the borrow
    let _element_count = elements.len();
    
    // Store initial z-indices for all elements to prevent accumulation
    ui.initial_z_indices.clear();
    for (uuid, element) in elements.iter() {
        ui.initial_z_indices.insert(*uuid, element.get_z_index());
    }

    // Mark all newly created elements as needing rerender so they get rendered
    ui.mark_all_needs_rerender();
}

/// Append FUR elements to an existing UINode without clearing existing elements
/// This allows dynamically adding UI elements at runtime
/// parent_id: If Some, the new elements will be children of that parent element. If None, they'll be root elements.
pub fn append_fur_elements_to_ui(ui: &mut UINode, elems: &[FurElement], parent_id: Option<UIElementID>) {
    // Collect all UUIDs that will be added, so we can mark them for rerender after the borrow is released
    let mut added_uuids = Vec::new();

    // Use a scope block to limit the lifetime of the mutable borrows
    {
        let elements = ui
            .elements
            .get_or_insert_with(|| IndexMap::new());

        let root_ids = ui
            .root_ids
            .get_or_insert_with(|| Vec::new());

        for el in elems {
            let flat = convert_fur_element_to_ui_elements(el, parent_id);
            for (uuid, e) in flat {
                // Store initial z-index for new element
                ui.initial_z_indices.insert(uuid, e.get_z_index());
                
                // Get the actual parent ID from the element (might be set during conversion)
                let actual_parent_id = e.get_parent();
                
                // If parent is nil, add to root_ids
                if actual_parent_id.is_nil() {
                    root_ids.push(uuid);
                } else {
                    // If parent is set, add to parent's children list
                    // Use actual_parent_id (from element) or parent_id (parameter) - prefer actual_parent_id
                    let parent_to_use = if !actual_parent_id.is_nil() {
                        actual_parent_id
                    } else if let Some(pid) = parent_id {
                        pid
                    } else {
                        continue; // No parent to add to
                    };
                    
                    if let Some(parent_element) = elements.get_mut(&parent_to_use) {
                        let mut children = parent_element.get_children().to_vec();
                        if !children.contains(&uuid) {
                            children.push(uuid);
                            parent_element.set_children(children);
                        }
                    }
                }
                elements.insert(uuid, e);
                added_uuids.push(uuid);
            }
        }
    } // Borrows are released here
    
    // Now mark all added elements for rerender (after the borrow is released)
    for uuid in added_uuids {
        ui.mark_element_needs_layout(uuid);
        
        // Also mark the element's actual parent for layout (in case it's different from parent_id parameter)
        if let Some(elements) = &ui.elements {
            if let Some(element) = elements.get(&uuid) {
                let actual_parent_id = element.get_parent();
                if !actual_parent_id.is_nil() {
                    ui.mark_element_needs_layout(actual_parent_id);
                }
            }
        }
    }
    
    // Also mark parent as needing layout if provided
    if let Some(parent_uuid) = parent_id {
        ui.mark_element_needs_layout(parent_uuid);
    }
}
