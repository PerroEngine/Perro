use indexmap::IndexMap;
use std::{borrow::Cow, collections::HashMap, time::Instant};
use uuid::Uuid;

use crate::{
    asset_io::load_asset,
    fur_ast::{FurAnchor, FurElement, FurNode},
    structs::Color,
    structs2d::Vector2,
    ui_element::{BaseElement, BaseUIElement, UIElement},
    ui_elements::{
        ui_container::{BoxContainer, ContainerMode, CornerRadius, GridLayout, Layout, UIPanel},
        ui_text::{TextAlignment, TextFlow, UIText},
        ui_button::UIButton,
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

pub fn parse_fur_file(path: &str) -> Result<Vec<FurNode>, String> {
    let bytes =
        load_asset(path).map_err(|e| format!("Failed to read .fur file {}: {}", path, e))?;

    let code = String::from_utf8_lossy(&bytes);
    let mut parser =
        crate::parser::FurParser::new(&code).map_err(|e| format!("Init parser: {}", e))?;

    let start = Instant::now();
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
                    base.style_map.insert(POS_X.into(), n);
                } else {
                    base.transform.position.x = n;
                }
            }
            "ty" => {
                let (n, pct) = parse_f32_percent(v, 0.0);
                if pct {
                    base.style_map.insert(POS_Y.into(), n);
                } else {
                    base.transform.position.y = n;
                }
            }
            "scl" => {
                let (x, y) = parse_compound(v);
                if let Some(xv) = x {
                    let (f, pct) = parse_f32_percent(xv, 1.0);
                    if pct {
                        base.style_map.insert(SCALE_X.into(), f);
                    } else {
                        base.transform.scale.x = f;
                    }
                }
                if let Some(yv) = y {
                    let (f, pct) = parse_f32_percent(yv, 1.0);
                    if pct {
                        base.style_map.insert(SCALE_Y.into(), f);
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
                        base.style_map.insert(SIZE_X.into(), f);
                    } else {
                        base.size.x = f;
                    }
                }
                if let Some(yv) = y_val {
                    let (f, pct) = parse_f32_percent(yv, base.size.y);
                    if pct {
                        base.style_map.insert(SIZE_Y.into(), f);
                    } else {
                        base.size.y = f;
                    }
                }
            }

            "w" | "sz-x" => {
                let (f, pct) = parse_f32_percent(v, base.size.x);
                if pct {
                    base.style_map.insert(SIZE_X.into(), f);
                } else {
                    base.size.x = f;
                }
            }
            "h" | "sz-y" => {
                let (f, pct) = parse_f32_percent(v, base.size.y);
                if pct {
                    base.style_map.insert(SIZE_Y.into(), f);
                } else {
                    base.size.y = f;
                }
            }

            "rot" => {
                let (f, pct) = parse_f32_percent(v, base.transform.rotation);
                if pct {
                    base.style_map.insert(ROT.into(), f);
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
            _ => {}
        }
    }
}

// =================== ELEMENT CONVERSION ===================

fn convert_fur_element_to_ui_element(fur: &FurElement) -> Option<UIElement> {
    let tag = fur.tag_name.as_ref();

    macro_rules! make {
        ($typ:expr) => {{
            let mut el = $typ;
            el.set_name(&fur.id);
            apply_base_attributes(&mut el.base, &fur.attributes);
            UIElement::$typ(el)
        }};
    }

    match tag {
        "UI" => {
            let mut ui = BoxContainer::default();
            ui.set_name(&fur.id);
            apply_base_attributes(&mut ui.base, &fur.attributes);
            ui.base.style_map.insert(SIZE_X.into(), 100.0);
            ui.base.style_map.insert(SIZE_Y.into(), 100.0);
            Some(UIElement::BoxContainer(ui))
        }

        "Box" => {
            let mut el = BoxContainer::default();
            el.set_name(&fur.id);
            apply_base_attributes(&mut el.base, &fur.attributes);
            Some(UIElement::BoxContainer(el))
        }

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

        "Layout" | "HLayout" | "VLayout" | "Grid" => {
            if tag == "Grid"
                || fur
                    .attributes
                    .get("mode")
                    .map(|s| s.eq_ignore_ascii_case("g"))
                    .unwrap_or(false)
            {
                let mut g = GridLayout::default();
                g.set_name(&fur.id);

                apply_base_attributes(&mut g.base, &fur.attributes);
                if let Some(c) = fur.attributes.get("cols") {
                    g.cols = c.parse().unwrap_or(1);
                }
                if let Some(gap) = fur.attributes.get("gap") {
                    let (x, y) = parse_compound(gap);
                    g.container.gap.x = x.and_then(|x| x.parse().ok()).unwrap_or(0.0);
                    g.container.gap.y = y.and_then(|y| y.parse().ok()).unwrap_or(g.container.gap.x);
                }
                return Some(UIElement::GridLayout(g));
            }

            let mut layout = Layout::default();
            layout.set_name(&fur.id);
            apply_base_attributes(&mut layout.base, &fur.attributes);

            layout.container.mode = match (tag, fur.attributes.get("mode").map(|v| v.as_ref())) {
                ("VLayout", _) | (_, Some("v") | Some("V")) => ContainerMode::Vertical,
                _ => ContainerMode::Horizontal,
            };

            if let Some(g) = fur.attributes.get("gap") {
                let parsed = g.parse::<f32>().unwrap_or(0.0);
                match layout.container.mode {
                    ContainerMode::Horizontal => layout.container.gap.x = parsed,
                    ContainerMode::Vertical => layout.container.gap.y = parsed,
                    _ => {}
                }
            }

            Some(UIElement::Layout(layout))
        }

        "Text" => {
            let mut text = UIText::default();
            text.set_name(&fur.id);
            apply_base_attributes(&mut text.base, &fur.attributes);

            // Extract text content from children and trim whitespace
            let text_content: String = fur
                .children
                .iter()
                .filter_map(|n| {
                    if let FurNode::Text(s) = n {
                        Some(s.as_ref())
                    } else {
                        None
                    }
                })
                .collect::<Vec<&str>>()
                .join("");
            
            // Trim the text content to remove any leading/trailing whitespace
            text.props.content = text_content.trim().to_string();

            if let Some(fs) = fur
                .attributes
                .get("fsz")
                .or(fur.attributes.get("font-size"))
            {
                text.props.font_size = fs.parse().unwrap_or(text.props.font_size);
            }

            // Parse text flow alignment (how text flows relative to anchor point)
            // align=start: text starts at anchor (flows right/down)
            // align=center: text is centered on anchor
            // align=end: text ends at anchor (flows left/up)
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
                text.props.align_h = match align_str.as_ref() {
                    "left" | "l" => TextAlignment::Left,
                    "center" | "c" => TextAlignment::Center,
                    "right" | "r" => TextAlignment::Right,
                    _ => TextAlignment::Center,
                };
            }
            if let Some(align_str) = fur.attributes.get("align-v") {
                text.props.align_v = match align_str.as_ref() {
                    "top" | "t" => TextAlignment::Top,
                    "center" | "c" => TextAlignment::Center,
                    "bottom" | "b" => TextAlignment::Bottom,
                    _ => TextAlignment::Center,
                };
            }

            Some(UIElement::Text(text))
        }

        "Button" => {
            let mut button = UIButton::default();
            button.set_name(&fur.id);
            apply_base_attributes(&mut button.base, &fur.attributes);

            // Apply panel properties (bg, border, rounding)
            if let Some(bg) = fur.attributes.get("bg") {
                if let Ok(c) = parse_color_with_opacity(bg) {
                    button.panel_props_mut().background_color = Some(c);
                }
            }
            
            // Parse hover and pressed background colors
            if let Some(hover_bg) = fur.attributes.get("hover-bg") {
                if let Ok(c) = parse_color_with_opacity(hover_bg) {
                    button.hover_bg = Some(c);
                }
            }
            if let Some(pressed_bg) = fur.attributes.get("pressed-bg") {
                if let Ok(c) = parse_color_with_opacity(pressed_bg) {
                    button.pressed_bg = Some(c);
                }
            }
            if let Some(c) = fur.attributes.get("border-c") {
                if let Ok(c) = parse_color_with_opacity(c) {
                    button.panel_props_mut().border_color = Some(c);
                }
            }
            if let Some(b) = fur.attributes.get("border") {
                button.panel_props_mut().border_thickness = b.parse().unwrap_or(0.0);
            }

            if let Some(opacity_str) = fur.attributes.get("opacity") {
                if let Some(opacity) = parse_opacity(opacity_str) {
                    button.panel_props_mut().opacity = opacity;
                }
            }

            // Apply corner radius (same logic as Panel)
            let mut corner = CornerRadius::default();
            fn parse_val(v: Option<&std::borrow::Cow<'_, str>>) -> Option<f32> {
                v.and_then(|s| s.trim().parse().ok())
            }

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

            // Directional and individual corner overrides
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

            button.panel_props_mut().corner_radius = corner;

            // Apply text properties - extract text from children
            let text_content: String = fur
                .children
                .iter()
                .filter_map(|n| {
                    if let FurNode::Text(s) = n {
                        Some(s.as_ref())
                    } else {
                        None
                    }
                })
                .collect::<Vec<&str>>()
                .join("");
            
            // Trim the text content (whitespace from parsing)
            let trimmed_content = text_content.trim();
            button.text_props_mut().content = trimmed_content.to_string();
            
            // Debug: verify text content is set
            if !trimmed_content.is_empty() {
                println!("Button '{}' text content: '{}' (len: {})", fur.id, trimmed_content, trimmed_content.len());
            } else {
                println!("WARNING: Button '{}' has empty text content! Children: {:?}", fur.id, fur.children.len());
            }
            
            // Set default text color to white if not specified (visible on colored backgrounds)
            if fur.attributes.get("text-c").is_none() {
                button.text_props_mut().color = Color::new(255, 255, 255, 255);
            }
            
            // Parse text-anchor for buttons (controls where text is positioned within button)
            if let Some(anchor_str) = fur.attributes.get("text-anchor") {
                button.text_anchor = match anchor_str.as_ref() {
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
                };
            }
            
            // Parse text flow alignment for buttons (how text flows relative to anchor point)
            if let Some(align_str) = fur.attributes.get("align") {
                button.text_props_mut().align = match align_str.as_ref() {
                    "start" | "s" => TextFlow::Start,
                    "center" | "c" => TextFlow::Center,
                    "end" | "e" => TextFlow::End,
                    // Backward compatibility
                    "left" | "l" | "top" | "t" => TextFlow::Start,
                    "right" | "r" | "bottom" | "b" => TextFlow::End,
                    _ => TextFlow::Center,
                };
            }
            
            // Backward compatibility: support old align-h and align-v
            if let Some(align_str) = fur.attributes.get("align-h") {
                let align = match align_str.as_ref() {
                    "left" | "l" => TextFlow::Start,
                    "center" | "c" => TextFlow::Center,
                    "right" | "r" => TextFlow::End,
                    _ => TextFlow::Center,
                };
                button.text_props_mut().align = align;
                button.text_props_mut().align_h = match align_str.as_ref() {
                    "left" | "l" => TextAlignment::Left,
                    "center" | "c" => TextAlignment::Center,
                    "right" | "r" => TextAlignment::Right,
                    _ => TextAlignment::Center,
                };
            }
            if let Some(align_str) = fur.attributes.get("align-v") {
                button.text_props_mut().align_v = match align_str.as_ref() {
                    "top" | "t" => TextAlignment::Top,
                    "center" | "c" => TextAlignment::Center,
                    "bottom" | "b" => TextAlignment::Bottom,
                    _ => TextAlignment::Center,
                };
            }
            
            // Set a reasonable default font size for buttons if not specified
            if fur.attributes.get("fsz").is_none() && fur.attributes.get("font-size").is_none() {
                button.text_props_mut().font_size = 16.0; // Larger default for buttons
            }
            
            println!("Button '{}' text props: content='{}', font_size={}, color={:?}", 
                fur.id, 
                button.text_props().content, 
                button.text_props().font_size,
                button.text_props().color
            );

            if let Some(fs) = fur
                .attributes
                .get("fsz")
                .or(fur.attributes.get("font-size"))
            {
                button.text_props_mut().font_size = fs.parse().unwrap_or(button.text_props().font_size);
            }

            if let Some(tc) = fur.attributes.get("text-c") {
                if let Ok(c) = parse_color_with_opacity(tc) {
                    button.text_props_mut().color = c;
                }
            }

            Some(UIElement::Button(button))
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
    parent_uuid: Option<Uuid>,
) -> Vec<(Uuid, UIElement)> {
    let Some(mut ui) = convert_fur_element_to_ui_element(fur) else {
        return fur
            .children
            .iter()
            .filter_map(|n| {
                if let FurNode::Element(e) = n {
                    Some(convert_fur_element_to_ui_elements(e, parent_uuid))
                } else {
                    None
                }
            })
            .flatten()
            .collect();
    };

    let id = Uuid::new_v4();
    ui.set_id(id);
    ui.set_parent(parent_uuid);

    let mut results = Vec::with_capacity(fur.children.len() + 1);
    let mut children = Vec::with_capacity(fur.children.len());

    for child in &fur.children {
        if let FurNode::Element(e) = child {
            let child_nodes = convert_fur_element_to_ui_elements(e, Some(id));
            if let Some((cid, _)) = child_nodes.first() {
                children.push(*cid);
            }
            results.extend(child_nodes);
        }
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
    let element_count = elements.len();
    
    // Mark all newly created elements as needing rerender so they get rendered
    ui.mark_all_needs_rerender();
}
