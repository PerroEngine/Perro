//! Bridge between Perro UI system and egui
//! Maps UIElement types to egui widgets while preserving .fur file compatibility

use std::collections::HashMap;
use crate::ids::UIElementID;
use crate::{
    structs::Color,
    structs2d::{Transform2D, Vector2},
    ui_element::UIElement,
    ui_elements::{
        ui_button::UIButton,
        ui_container::{CornerRadius, UIPanel},
        ui_text::UIText,
    },
};
use egui::{Color32, Context, FontId, Frame, Rect, Stroke, Ui};

/// Converts Perro Color to egui Color32
fn color_to_egui(color: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

/// Converts Perro CornerRadius (top_left, etc.) to egui CornerRadius (nw, ne, sw, se)
fn corner_radius_to_egui(corner: &CornerRadius) -> egui::CornerRadius {
    egui::CornerRadius {
        nw: corner.top_left as u8,
        ne: corner.top_right as u8,
        sw: corner.bottom_left as u8,
        se: corner.bottom_right as u8,
    }
}

/// Converts Transform2D + size to egui Rect
fn transform_to_rect(transform: &Transform2D, size: &Vector2) -> Rect {
    let pos = transform.position;
    let scaled_size = Vector2::new(size.x * transform.scale.x, size.y * transform.scale.y);

    // egui uses top-left origin, Perro uses center origin
    let min = egui::pos2(pos.x - scaled_size.x * 0.5, pos.y - scaled_size.y * 0.5);
    let max = egui::pos2(pos.x + scaled_size.x * 0.5, pos.y + scaled_size.y * 0.5);

    Rect::from_min_max(min, max)
}

/// Renders a UIElement using egui
/// Returns a list of events that occurred (button clicks, text changes, etc.)
pub fn render_element_to_egui(
    element: &UIElement,
    ctx: &Context,
    ui: &mut Ui,
    element_states: &mut HashMap<u64, ElementState>,
) -> Vec<ElementEvent> {
    match element {
        UIElement::Panel(panel) => {
            render_panel_egui(panel, ctx, ui, element_states);
            vec![]
        }
        UIElement::Button(button) => {
            let clicked = render_button_egui(button, ctx, ui, element_states);
            if clicked {
                vec![ElementEvent::ButtonClicked(button.base.id, button.base.name.clone())]
            } else {
                vec![]
            }
        }
        UIElement::Text(text) => {
            render_text_egui(text, ctx, ui);
            vec![]
        }
    }
}

/// Events emitted by egui widgets
#[derive(Debug, Clone)]
pub enum ElementEvent {
    ButtonClicked(UIElementID, String), // element_id, element_name
    TextChanged(UIElementID, String),   // element_id, new_text
}

/// State tracked per element for egui rendering
#[derive(Default)]
pub struct ElementState {
    pub text_buffer: String,
    pub is_focused: bool,
    pub is_pressed: bool, // For buttons
}

/// Render a Panel using egui
fn render_panel_egui(
    panel: &UIPanel,
    _ctx: &Context,
    ui: &mut Ui,
    _element_states: &mut HashMap<u64, ElementState>,
) {
    let rect = transform_to_rect(&panel.base.global_transform, &panel.base.size);

    let bg_color = panel
        .props
        .background_color
        .map(color_to_egui)
        .unwrap_or(Color32::TRANSPARENT);
    let border_color = panel
        .props
        .border_color
        .map(color_to_egui)
        .unwrap_or(Color32::TRANSPARENT);
    let rounding = corner_radius_to_egui(&panel.props.corner_radius);

    let frame = Frame::default()
        .fill(bg_color)
        .stroke(Stroke::new(panel.props.border_thickness, border_color))
        .corner_radius(rounding);

    frame.show(ui, |ui| {
        ui.set_clip_rect(rect);
        // Panel content will be rendered by children
    });
}

/// Render Text using egui
fn render_text_egui(text: &UIText, _ctx: &Context, ui: &mut Ui) {
    let font_id = FontId::proportional(text.props.font_size);
    let color = color_to_egui(text.props.color);

    ui.label(
        egui::RichText::new(&text.props.content)
            .color(color)
            .font(font_id),
    );
}

/// Render Button using egui
/// Returns true if button was clicked (for signal emission)
fn render_button_egui(
    button: &UIButton,
    _ctx: &Context,
    ui: &mut Ui,
    _element_states: &mut HashMap<u64, ElementState>,
) -> bool {
    let label = if button.label.is_empty() {
        &button.base.name
    } else {
        &button.label
    };
    ui.button(label).clicked()
}

/// Render TextInput using egui (native text editing!)
/// Returns true if text changed
/// NOTE: UITextInput type was removed - this function is a stub
#[allow(dead_code, unused_variables)]
fn render_text_input_egui(
    _text_input: &dyn std::any::Any,
    _ctx: &Context,
    _ui: &mut Ui,
    _element_states: &mut HashMap<u64, ElementState>,
) -> bool {
    false
}

/// Render TextEdit (multi-line) using egui
/// Returns true if text changed
/// NOTE: UITextEdit type was removed - this function is a stub
#[allow(dead_code, unused_variables)]
fn render_text_edit_egui(
    _text_edit: &dyn std::any::Any,
    _ctx: &Context,
    _ui: &mut Ui,
    _element_states: &mut HashMap<u64, ElementState>,
) -> bool {
    false
}
