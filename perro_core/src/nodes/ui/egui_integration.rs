//! Complete egui integration for Perro UI system
//! Maps FUR elements to egui widgets and handles rendering

#![allow(deprecated)] // egui 0.33: rounding -> corner_radius, allocate_ui_at_rect -> scope_builder; keep current API for now

use crate::ids::UIElementID;
use crate::{
    structs::Color,
    structs2d::{Transform2D, Vector2},
    ui_element::{BaseElement, UIElement},
    ui_elements::{
        ui_button::UIButton,
        ui_container::{CornerRadius, UIPanel},
        ui_text::UIText,
    },
};
use egui::{Color32, Context, FontId, Frame, Rect, RichText, Stroke, Ui};
use std::collections::HashMap;

/// Converts Perro Color to egui Color32
fn color_to_egui(color: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

/// Converts Perro CornerRadius to egui CornerRadius
/// egui 0.33 uses u8 for rounding values
fn corner_radius_to_egui(corner: &CornerRadius) -> egui::CornerRadius {
    egui::CornerRadius {
        nw: corner.top_left as u8,
        ne: corner.top_right as u8,
        sw: corner.bottom_left as u8,
        se: corner.bottom_right as u8,
    }
}

/// Converts Transform2D + size + pivot to egui Rect
/// Perro: center-origin (0,0 at center), Y-up (+y is up), pivot at (0.5, 0.5) by default
/// egui: top-left origin (0,0 at top-left), Y-down
///
/// Coordinate system:
/// - 0,0 is center of screen
/// - +x +y is up-right
/// - -x -y is down-left
fn transform_to_rect(transform: &Transform2D, size: &Vector2, pivot: &Vector2) -> Rect {
    let pos = transform.position;
    let scaled_size = Vector2::new(size.x * transform.scale.x, size.y * transform.scale.y);

    // Calculate bounds from pivot point
    // In Perro's Y-up system: top has highest Y, bottom has lowest Y
    // Pivot (0.5, 0.5) means center, (0, 0) means bottom-left, (1, 1) means top-right
    let min_x = pos.x - scaled_size.x * pivot.x;
    let max_x = pos.x + scaled_size.x * (1.0 - pivot.x);
    let max_y = pos.y + scaled_size.y * (1.0 - pivot.y); // Top (highest Y in Y-up)
    let min_y = pos.y - scaled_size.y * pivot.y; // Bottom (lowest Y in Y-up)

    // Convert to egui coordinates (top-left origin, Y-down)
    // For now, we'll convert in the renderer with screen dimensions
    // This function returns virtual coordinates
    let min = egui::pos2(min_x, min_y);
    let max = egui::pos2(max_x, max_y);

    Rect::from_min_max(min, max)
}

/// State tracked per element for egui rendering
#[derive(Default)]
pub struct ElementState {
    pub text_buffer: String,
    pub is_focused: bool,
    pub is_pressed: bool, // For buttons
    pub is_hovered: bool, // For buttons
}

/// Events emitted by egui widgets
#[derive(Debug, Clone)]
pub enum ElementEvent {
    ButtonClicked(UIElementID, String), // element_id, element_name
    TextChanged(UIElementID, String),   // element_id, new_text
    ButtonHovered(UIElementID, bool),   // element_id, is_hovered
    ButtonPressed(UIElementID, bool),   // element_id, is_pressed
}

/// Main egui integration manager
pub struct EguiIntegration {
    pub context: Context,
    pub element_states: HashMap<UIElementID, ElementState>,
    pub events: Vec<ElementEvent>,
    pub last_output: Option<egui::FullOutput>, // Store last frame's output for rendering
}

impl EguiIntegration {
    pub fn new() -> Self {
        Self {
            context: Context::default(),
            element_states: HashMap::new(),
            events: Vec::new(),
            last_output: None,
        }
    }

    /// Render a UIElement tree using egui
    /// elements: Map of all UI elements
    /// root_ids: IDs of root elements to render
    pub fn render_element_tree(
        &mut self,
        elements: &std::collections::HashMap<UIElementID, UIElement>,
        root_ids: &[UIElementID],
        ui: &mut Ui,
        api: &mut Option<&mut crate::scripting::api::ScriptApi>,
    ) {
        // Render root elements
        for &root_id in root_ids {
            if let Some(element) = elements.get(&root_id) {
                self.render_element_recursive(element, elements, ui, api);
            }
        }
    }

    /// Recursively render an element and its children
    fn render_element_recursive(
        &mut self,
        element: &UIElement,
        elements: &std::collections::HashMap<UIElementID, UIElement>,
        ui: &mut Ui,
        api: &mut Option<&mut crate::scripting::api::ScriptApi>,
    ) {
        if !element.get_visible() {
            return;
        }

        match element {
            UIElement::Panel(panel) => {
                self.render_panel_with_children(panel, elements, ui, api);
            }
            UIElement::Button(button) => {
                self.render_button_with_children(button, elements, ui, api);
            }
            UIElement::Text(text) => {
                self.render_text(text, ui);
            }
        }
    }

    /// Render children of an element
    fn render_children(
        &mut self,
        element: &UIElement,
        elements: &std::collections::HashMap<UIElementID, UIElement>,
        ui: &mut Ui,
        api: &mut Option<&mut crate::scripting::api::ScriptApi>,
    ) {
        for &child_id in element.get_children() {
            if let Some(child) = elements.get(&child_id) {
                self.render_element_recursive(child, elements, ui, api);
            }
        }
    }

    /// Render a Panel (wraps egui Frame) with children
    pub fn render_panel_with_children(
        &mut self,
        panel: &UIPanel,
        elements: &std::collections::HashMap<UIElementID, UIElement>,
        ui: &mut Ui,
        api: &mut Option<&mut crate::scripting::api::ScriptApi>,
    ) {
        log::debug!(
            "🎨 [EGUI] render_panel_with_children: '{}' -> egui Frame",
            panel.base.name
        );
        let rect = transform_to_rect(
            &panel.base.global_transform,
            &panel.base.size,
            &panel.base.pivot,
        );

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
            .rounding(rounding);

        ui.allocate_ui_at_rect(rect, |ui| {
            frame.show(ui, |ui| {
                ui.set_clip_rect(rect);
                // Render children
                self.render_children(&UIElement::Panel(panel.clone()), elements, ui, api);
            });
        });
    }

    /// Render Text (maps to egui label)
    pub fn render_text(&self, text: &UIText, ui: &mut Ui) {
        if !text.base.visible {
            return;
        }

        log::debug!("🎨 [EGUI] render_text: '{}' -> egui Label", text.base.name);
        let font_id = FontId::proportional(text.props.font_size);
        let color = color_to_egui(text.props.color);

        ui.label(
            RichText::new(&text.props.content)
                .color(color)
                .font(font_id),
        );
    }

    /// Render Button (wraps egui Button) with children and signal support
    pub fn render_button_with_children(
        &mut self,
        button: &UIButton,
        elements: &std::collections::HashMap<UIElementID, UIElement>,
        ui: &mut Ui,
        api: &mut Option<&mut crate::scripting::api::ScriptApi>,
    ) {
        let label = if button.label.is_empty() {
            &button.base.name
        } else {
            &button.label
        };
        let rect = transform_to_rect(
            &button.base.global_transform,
            &button.base.size,
            &button.base.pivot,
        );
        ui.allocate_ui_at_rect(rect, |ui| {
            let response = ui.button(label);
            if response.clicked() {
                self.events.push(ElementEvent::ButtonClicked(
                    button.base.id,
                    button.base.name.clone(),
                ));
            }
            if response.hovered() {
                self.events.push(ElementEvent::ButtonHovered(button.base.id, true));
            }
            if response.contains_pointer() {
                self.events.push(ElementEvent::ButtonPressed(
                    button.base.id,
                    ui.input(|i| i.pointer.primary_down()),
                ));
            }
            // Render children inside the button area
            for &child_id in button.get_children() {
                if let Some(child) = elements.get(&child_id) {
                    self.render_element_recursive(child, elements, ui, api);
                }
            }
        });
    }

    /// Render TextInput (maps to egui TextEdit::singleline)
    /// NOTE: UITextInput type no longer exists - this function is kept for potential future use
    #[allow(dead_code)]
    pub fn render_text_input(&mut self, _text_input: &dyn std::any::Any, _ui: &mut Ui) {
        // UITextInput type was removed - function body removed
    }

    /// Render TextEdit (maps to egui TextEdit::multiline)
    /// NOTE: UITextEdit type no longer exists - this function is kept for potential future use
    #[allow(dead_code)]
    pub fn render_text_edit(&mut self, _text_edit: &dyn std::any::Any, _ui: &mut Ui) {
        // UITextEdit type was removed - function body removed
    }

    /// Clear events (call after processing)
    pub fn clear_events(&mut self) {
        self.events.clear();
    }
}
