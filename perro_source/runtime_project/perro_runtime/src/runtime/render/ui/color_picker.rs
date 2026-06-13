use perro_ids::NodeID;
use perro_structs::{Color, Vector2};
use perro_ui::ComputedUiRect;

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
        format!("{:.3}", color.r()),
        format!("{:.3}", color.g()),
        format!("{:.3}", color.b()),
        format!("{:.3}", color.a()),
    ]
}

pub(super) fn color_picker_wheel_render_node(picker_id: NodeID) -> NodeID {
    NodeID::from_u64(0xC010_0000_0000_0000 ^ picker_id.as_u64())
}

pub(super) fn color_picker_wheel_rect(
    popup_rect: ComputedUiRect,
    wheel_radius: f32,
) -> ComputedUiRect {
    let radius = wheel_radius.max(8.0);
    ComputedUiRect::new(
        Vector2::new(popup_rect.center.x, popup_rect.min().y + radius + 16.0),
        Vector2::new(radius * 2.0, radius * 2.0),
    )
}

pub(super) fn color_to_hsv_components(color: Color) -> [String; 3] {
    let (h, s, v) = rgb_to_hsv(color);
    [
        format!("{:.1}", h * 360.0),
        format!("{:.3}", s),
        format!("{:.3}", v),
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

fn rgb_to_hsv(color: Color) -> (f32, f32, f32) {
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
    let s = if max <= f32::EPSILON {
        0.0
    } else {
        delta / max
    };
    (h, s, max)
}

pub(super) fn parse_color_picker_text(
    field: ColorPickerTextField,
    text: &str,
    current: Color,
) -> Option<Color> {
    match field {
        ColorPickerTextField::Rgba(idx) => {
            let val = text.trim().parse::<f32>().ok()?.clamp(0.0, 1.0);
            let mut rgba = current.to_rgba();
            rgba[idx.min(3)] = val;
            Some(Color::new(rgba[0], rgba[1], rgba[2], rgba[3]))
        }
        ColorPickerTextField::Hsv(idx) => {
            let val = text.trim().parse::<f32>().ok()?;
            let (h, s, v) = rgb_to_hsv(current);
            let mut hsv = [h * 360.0, s, v];
            hsv[idx.min(2)] = if idx == 0 {
                val.rem_euclid(360.0)
            } else {
                val.clamp(0.0, 1.0)
            };
            Some(hsv_to_color(
                (hsv[0] / 360.0).rem_euclid(1.0),
                hsv[1].clamp(0.0, 1.0),
                hsv[2].clamp(0.0, 1.0),
                current.a(),
            ))
        }
        ColorPickerTextField::Hex => parse_hex_color(text, current.a()),
    }
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

fn hsv_to_color(h: f32, s: f32, v: f32, a: f32) -> Color {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor();
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color::new(r, g, b, a)
}
