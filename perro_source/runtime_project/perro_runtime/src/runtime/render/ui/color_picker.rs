use perro_ids::NodeID;
use perro_structs::{Color, Vector2};
use perro_ui::ComputedUiRect;

const PICKER_PAD: f32 = 14.0;
const PICKER_GAP: f32 = 10.0;
const FIELD_HEIGHT: f32 = 32.0;

#[derive(Clone, Copy)]
pub(super) struct ColorPickerLayout {
    pub(super) popup_size: [f32; 2],
    pub(super) selector_y: f32,
    pub(super) rgba_y: f32,
    pub(super) hsl_y: f32,
    pub(super) hex_y: f32,
}

pub(super) fn color_picker_layout(
    popup_size: [f32; 2],
    radius: f32,
    show_selector: bool,
    show_rgba: bool,
    show_hsl: bool,
    show_hex: bool,
) -> ColorPickerLayout {
    let diameter = radius.max(8.0) * 2.0;
    let mut cursor = PICKER_PAD;
    let selector_y = cursor + diameter * 0.5;
    if show_selector {
        cursor += diameter + PICKER_GAP;
    }
    let rgba_y = cursor + FIELD_HEIGHT * 0.5;
    if show_rgba {
        cursor += FIELD_HEIGHT + PICKER_GAP;
    }
    let hsl_y = cursor + FIELD_HEIGHT * 0.5;
    if show_hsl {
        cursor += FIELD_HEIGHT + PICKER_GAP;
    }
    let hex_y = cursor + FIELD_HEIGHT * 0.5;
    if show_hex {
        cursor += FIELD_HEIGHT;
    }
    ColorPickerLayout {
        popup_size: [
            popup_size[0].max(340.0),
            popup_size[1].max(cursor + PICKER_PAD),
        ],
        selector_y,
        rgba_y,
        hsl_y,
        hex_y,
    }
}

#[derive(Clone, Copy)]
pub(super) struct ColorPickerComponentLayout {
    pub(super) popup_size: [f32; 2],
    pub(super) y_from_top: f32,
    pub(super) col: usize,
    pub(super) cols: usize,
}

impl ColorPickerComponentLayout {
    pub(super) fn new(popup_size: [f32; 2], y_from_top: f32, col: usize, cols: usize) -> Self {
        Self {
            popup_size,
            y_from_top,
            col,
            cols,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum ColorPickerTextField {
    Rgba(usize),
    Hsv(usize),
    Hex,
}

pub(super) fn color_to_rgba_components(color: Color) -> [String; 4] {
    [
        format!("R  {:.3}", color.r()),
        format!("G  {:.3}", color.g()),
        format!("B  {:.3}", color.b()),
        format!("A  {:.3}", color.a()),
    ]
}

pub(super) fn color_picker_wheel_render_node(picker_id: NodeID) -> NodeID {
    NodeID::from_u64(0xC010_0000_0000_0000 ^ picker_id.as_u64())
}

pub(super) fn color_picker_wheel_rect(
    popup_rect: ComputedUiRect,
    wheel_radius: f32,
    selector_y: f32,
) -> ComputedUiRect {
    let radius = wheel_radius.max(8.0);
    ComputedUiRect::new(
        Vector2::new(popup_rect.center.x, popup_rect.max().y - selector_y),
        Vector2::new(radius * 2.0, radius * 2.0),
    )
}

pub(super) fn color_to_hsl_components(color: Color) -> [String; 3] {
    let (h, s, l) = rgb_to_hsl(color);
    [
        format!("H  {:.1}°", h * 360.0),
        format!("S  {:.3}", s),
        format!("L  {:.3}", l),
    ]
}

pub(super) fn color_to_hex_text(color: Color) -> String {
    let [r, g, b, a] = color.to_rgba();
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        (r.clamp(0.0, 1.0) * 255.0).round() as u8,
        (g.clamp(0.0, 1.0) * 255.0).round() as u8,
        (b.clamp(0.0, 1.0) * 255.0).round() as u8,
        (a.clamp(0.0, 1.0) * 255.0).round() as u8
    )
}

fn rgb_to_hsl(color: Color) -> (f32, f32, f32) {
    let r = color.r();
    let g = color.g();
    let b = color.b();
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let h = if delta <= f32::EPSILON {
        0.0
    } else if max == r {
        ((g - b) / delta).rem_euclid(6.0) / 6.0
    } else if max == g {
        (((b - r) / delta) + 2.0) / 6.0
    } else {
        (((r - g) / delta) + 4.0) / 6.0
    };
    let l = (max + min) * 0.5;
    let s = if delta <= f32::EPSILON {
        0.0
    } else {
        delta / (1.0 - (2.0 * l - 1.0).abs()).max(f32::EPSILON)
    };
    (h, s, l)
}

pub(super) fn parse_color_picker_text(
    field: ColorPickerTextField,
    text: &str,
    current: Color,
) -> Option<Color> {
    match field {
        ColorPickerTextField::Rgba(idx) => {
            let val = parse_component_value(text)?.clamp(0.0, 1.0);
            let mut rgba = current.to_rgba();
            rgba[idx.min(3)] = val;
            Some(Color::new(rgba[0], rgba[1], rgba[2], rgba[3]))
        }
        ColorPickerTextField::Hsv(idx) => {
            let val = parse_component_value(text)?;
            let (h, s, l) = rgb_to_hsl(current);
            let mut hsl = [h * 360.0, s, l];
            hsl[idx.min(2)] = if idx == 0 {
                val.rem_euclid(360.0)
            } else {
                val.clamp(0.0, 1.0)
            };
            Some(hsl_to_color(
                (hsl[0] / 360.0).rem_euclid(1.0),
                hsl[1].clamp(0.0, 1.0),
                hsl[2].clamp(0.0, 1.0),
                current.a(),
            ))
        }
        ColorPickerTextField::Hex => parse_hex_color(text, current.a()),
    }
}

fn parse_component_value(text: &str) -> Option<f32> {
    let value = text.split_whitespace().last()?.trim_end_matches(['°', '%']);
    let parsed = value.parse::<f32>().ok()?;
    Some(if text.trim_end().ends_with('%') {
        parsed / 100.0
    } else {
        parsed
    })
}

fn parse_hex_color(text: &str, alpha: f32) -> Option<Color> {
    let hex = text.trim().trim_start_matches('#');
    let expanded;
    let hex = match hex.len() {
        3 => {
            expanded = hex.chars().flat_map(|ch| [ch, ch]).collect::<String>();
            expanded.as_str()
        }
        4 => {
            expanded = hex.chars().flat_map(|ch| [ch, ch]).collect::<String>();
            expanded.as_str()
        }
        6 | 8 => hex,
        _ => return None,
    };
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
    let a = if hex.len() >= 8 {
        u8::from_str_radix(&hex[6..8], 16).ok()? as f32 / 255.0
    } else {
        alpha
    };
    Some(Color::new(r, g, b, a))
}

fn hsl_to_color(h: f32, s: f32, l: f32, a: f32) -> Color {
    let chroma = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(1.0) * 6.0;
    let x = chroma * (1.0 - (hp.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match hp as i32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = l - chroma * 0.5;
    Color::new(r + m, g + m, b + m, a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_keeps_large_selector_above_fields() {
        let layout = color_picker_layout([240.0, 120.0], 100.0, true, true, true, true);
        assert!(layout.popup_size[0] >= 340.0);
        assert!(layout.rgba_y > layout.selector_y + 100.0);
        assert!(layout.hsl_y > layout.rgba_y);
        assert!(layout.hex_y > layout.hsl_y);
        assert!(layout.popup_size[1] >= layout.hex_y + 30.0);
    }

    #[test]
    fn labeled_hsl_value_round_trips() {
        let current = Color::new(1.0, 0.0, 0.0, 0.75);
        let color = parse_color_picker_text(ColorPickerTextField::Hsv(0), "H  120.0°", current)
            .expect("valid HSL hue");
        assert!(color.r() < 0.001);
        assert!(color.g() > 0.999);
        assert!(color.b() < 0.001);
        assert!((color.a() - 0.75).abs() < 0.001);
    }

    #[test]
    fn wheel_uses_popup_top_edge() {
        let popup = ComputedUiRect::new(Vector2::new(0.0, 0.0), Vector2::new(360.0, 344.0));
        let wheel = color_picker_wheel_rect(popup, 88.0, 102.0);
        assert!((wheel.center.y - 70.0).abs() < 0.001);
    }
}
