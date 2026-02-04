//! Complete egui integration for Perro UI system
//! Maps FUR elements to egui widgets and handles rendering

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
use egui::{Color32, FontId, Rect, Stroke, Ui};
use std::collections::HashMap;

/// Converts Perro Color to egui Color32
fn color_to_egui(color: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

fn apply_opacity(color: Color32, opacity: f32) -> Color32 {
    let a = (color.a() as f32 * opacity.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), a)
}

/// Converts Perro CornerRadius to egui CornerRadius (points)
fn corner_radius_to_egui(
    corner: &CornerRadius,
    rect: &Rect,
    size_virtual: Vector2,
) -> egui::CornerRadius {
    let max_radius = rect.width().min(rect.height()) * 0.5;
    let scale = if size_virtual.x != 0.0 {
        rect.width() / size_virtual.x
    } else {
        1.0
    };
    let resolve = |v: f32| {
        let r = if v < 0.0 {
            // Percent encoded as negative (e.g., -50 == 50%)
            max_radius * (v.abs() / 100.0)
        } else if v <= 1.0 {
            // Normalized (0.0..1.0) of max radius
            max_radius * v
        } else {
            // Absolute virtual units -> scale to screen points
            v * scale
        };
        r.round().clamp(0.0, 255.0) as u8
    };
    egui::CornerRadius {
        nw: resolve(corner.top_left),
        ne: resolve(corner.top_right),
        sw: resolve(corner.bottom_left),
        se: resolve(corner.bottom_right),
    }
}

/// Converts Transform2D + size + pivot to egui Rect in screen pixels.
/// Perro: center-origin (0,0 at center), Y-up (+y is up), pivot at (0.5, 0.5) by default
/// egui: top-left origin (0,0 at top-left), Y-down
fn transform_to_rect(
    transform: &Transform2D,
    size: &Vector2,
    pivot: &Vector2,
    virtual_size: Vector2,
    screen_size: Vector2,
) -> Rect {
    let pos = transform.position;
    let scaled_size = Vector2::new(
        size.x * transform.scale.x,
        size.y * transform.scale.y
    );

    let min_x_ui = pos.x - scaled_size.x * pivot.x;
    let max_x_ui = pos.x + scaled_size.x * (1.0 - pivot.x);
    let min_y_ui = pos.y - scaled_size.y * pivot.y;
    let max_y_ui = pos.y + scaled_size.y * (1.0 - pivot.y);

    let half_virtual_w = virtual_size.x * 0.5;
    let half_virtual_h = virtual_size.y * 0.5;
    
    let min_x_virtual = min_x_ui + half_virtual_w;
    let max_x_virtual = max_x_ui + half_virtual_w;
    let min_y_virtual = -max_y_ui + half_virtual_h;
    let max_y_virtual = -min_y_ui + half_virtual_h;

    let scale_x = screen_size.x / virtual_size.x;
    let scale_y = screen_size.y / virtual_size.y;
    
    let screen_min_x = min_x_virtual * scale_x;
    let screen_max_x = max_x_virtual * scale_x;
    let screen_min_y = min_y_virtual * scale_y;
    let screen_max_y = max_y_virtual * scale_y;

    let rect = Rect::from_min_max(
        egui::pos2(screen_min_x, screen_min_y),
        egui::pos2(screen_max_x, screen_max_y),
    );
    
    let center_x_virtual = (min_x_virtual + max_x_virtual) * 0.5;
    let center_y_virtual = (min_y_virtual + max_y_virtual) * 0.5;
    let center_x_screen = (screen_min_x + screen_max_x) * 0.5;
    let center_y_screen = (screen_min_y + screen_max_y) * 0.5;
    
    println!(
        "🔍 ui({:.1},{:.1}) -> virt_center({:.1},{:.1}) -> screen_center({:.1},{:.1}) size={:.1}x{:.1}",
        pos.x, pos.y,
        center_x_virtual, center_y_virtual,
        center_x_screen, center_y_screen,
        rect.width(), rect.height()
    );
    
    rect
}

/// State tracked per element for egui rendering
#[derive(Default)]
pub struct ElementState {
    pub is_pressed: bool, // For buttons
    pub is_hovered: bool, // For buttons
}

/// Events emitted by egui widgets
#[derive(Debug, Clone)]
pub enum ElementEvent {
    ButtonClicked(UIElementID, String), // element_id, element_name
    ButtonHovered(UIElementID, bool),   // element_id, is_hovered
    ButtonPressed(UIElementID, bool),   // element_id, is_pressed
}

/// Main egui integration manager
pub struct EguiIntegration {
    pub element_states: HashMap<UIElementID, ElementState>,
    pub events: Vec<ElementEvent>,
    pub last_output: Option<egui::FullOutput>,
}

impl EguiIntegration {
    pub fn new() -> Self {
        Self {
            element_states: HashMap::new(),
            events: Vec::new(),
            last_output: None,
        }
    }

    /// Render a UIElement tree using egui
    pub fn render_element_tree(
        &mut self,
        elements: &std::collections::HashMap<UIElementID, UIElement>,
        root_ids: &[UIElementID],
        virtual_size: Vector2,
        screen_size: Vector2,
        ui: &mut Ui,
        api: &mut Option<&mut crate::scripting::api::ScriptApi>,
    ) {
        println!(
            "[egui_integration] render_element_tree ENTER: {} elements, {} roots",
            elements.len(),
            root_ids.len()
        );
        
        // Render root elements
        for &root_id in root_ids {
            if let Some(element) = elements.get(&root_id) {
                self.render_element_recursive(
                    element,
                    elements,
                    virtual_size,
                    screen_size,
                    ui,
                    api,
                );
            }
        }
    }

    /// Recursively render an element and its children
    fn render_element_recursive(
        &mut self,
        element: &UIElement,
        elements: &std::collections::HashMap<UIElementID, UIElement>,
        virtual_size: Vector2,
        screen_size: Vector2,
        ui: &mut Ui,
        api: &mut Option<&mut crate::scripting::api::ScriptApi>,
    ) {
        if !element.get_visible() {
            return;
        }

        match element {
            UIElement::Panel(panel) => {
                self.render_panel_with_children(panel, elements, virtual_size, screen_size, ui, api);
            }
            UIElement::Button(button) => {
                self.render_button_with_children(button, elements, virtual_size, screen_size, ui, api);
            }
            UIElement::Text(text) => {
                self.render_text(text, virtual_size, screen_size, ui);
            }
        }
    }

    /// Render children of an element
    fn render_children(
        &mut self,
        element: &UIElement,
        elements: &std::collections::HashMap<UIElementID, UIElement>,
        virtual_size: Vector2,
        screen_size: Vector2,
        ui: &mut Ui,
        api: &mut Option<&mut crate::scripting::api::ScriptApi>,
    ) {
        for &child_id in element.get_children() {
            if let Some(child) = elements.get(&child_id) {
                self.render_element_recursive(child, elements, virtual_size, screen_size, ui, api);
            }
        }
    }

    /// Render a Panel with children - PAINT BEFORE SCOPE
    pub fn render_panel_with_children(
        &mut self,
        panel: &UIPanel,
        elements: &std::collections::HashMap<UIElementID, UIElement>,
        virtual_size: Vector2,
        screen_size: Vector2,
        ui: &mut Ui,
        api: &mut Option<&mut crate::scripting::api::ScriptApi>,
    ) {
        println!(
            "[egui_integration] Panel '{}' id={} -> egui Frame",
            panel.base.name,
            panel.base.id
        );
        
        let rect = transform_to_rect(
            &panel.base.global_transform,
            &panel.base.size,
            &panel.base.pivot,
            virtual_size,
            screen_size,
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

        let rounding = corner_radius_to_egui(&panel.props.corner_radius, &rect, panel.base.size);

        // PAINT DIRECTLY TO PARENT UI (before creating scope)
        let bg_color = apply_opacity(bg_color, panel.props.opacity);
        let border_color = apply_opacity(border_color, panel.props.opacity);
        
        if bg_color != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, rounding, bg_color);
        }
        if panel.props.border_thickness > 0.0 && border_color != Color32::TRANSPARENT {
            ui.painter().rect_stroke(
                rect,
                rounding,
                Stroke::new(panel.props.border_thickness, border_color),
                egui::StrokeKind::Inside,
            );
        }

        // NOW create scope for children
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.set_clip_rect(rect);
            ui.set_opacity(panel.props.opacity);
            self.render_children(
                &UIElement::Panel(panel.clone()),
                elements,
                virtual_size,
                screen_size,
                ui,
                api,
            );
        });
    }

    /// Render Text (maps to egui label)
    pub fn render_text(
        &self,
        text: &UIText,
        virtual_size: Vector2,
        screen_size: Vector2,
        ui: &mut Ui,
    ) {
        if !text.base.visible {
            return;
        }

        println!("[egui_integration] Text '{}' -> egui Label", text.base.name);
        let font_id = FontId::proportional(text.props.font_size);
        let color = color_to_egui(text.props.color);
        let rect = transform_to_rect(
            &text.base.global_transform,
            &text.base.size,
            &text.base.pivot,
            virtual_size,
            screen_size,
        );

        let (pos, align) = match text.props.align {
            crate::ui_elements::ui_text::TextFlow::Start => {
                (rect.left_center(), egui::Align2::LEFT_CENTER)
            }
            crate::ui_elements::ui_text::TextFlow::Center => {
                (rect.center(), egui::Align2::CENTER_CENTER)
            }
            crate::ui_elements::ui_text::TextFlow::End => {
                (rect.right_center(), egui::Align2::RIGHT_CENTER)
            }
        };
        ui.painter().text(pos, align, &text.props.content, font_id, color);
    }

    /// Render Button with children - PAINT BEFORE SCOPE
    pub fn render_button_with_children(
        &mut self,
        button: &UIButton,
        elements: &std::collections::HashMap<UIElementID, UIElement>,
        virtual_size: Vector2,
        screen_size: Vector2,
        ui: &mut Ui,
        api: &mut Option<&mut crate::scripting::api::ScriptApi>,
    ) {
        println!(
            "[egui_integration] Button '{}' id={} -> egui Button",
            button.base.name,
            button.base.id
        );
        
        let rect = transform_to_rect(
            &button.base.global_transform,
            &button.base.size,
            &button.base.pivot,
            virtual_size,
            screen_size,
        );
        
        let base_bg = button.props.background_color;
        let hover_bg = button.hover_color.or(base_bg.map(|c| c.lighten(0.08)));
        let pressed_bg = button.pressed_color.or(base_bg.map(|c| c.darken(0.12)));

        let base_border = button.props.border_color;
        let hover_border = button
            .hover_border_color
            .or(base_border.map(|c| c.lighten(0.08)));
        let pressed_border = button
            .pressed_border_color
            .or(base_border.map(|c| c.darken(0.12)));

        let mut bg_color = base_bg.map(color_to_egui).unwrap_or(Color32::TRANSPARENT);
        let mut border_color = base_border
            .map(color_to_egui)
            .unwrap_or(Color32::TRANSPARENT);
        let rounding = corner_radius_to_egui(&button.props.corner_radius, &rect, button.base.size);

        // Create interaction OUTSIDE scope using ui.interact
        let interact_id = ui.make_persistent_id(format!("button_{}", button.base.id));
        let response = ui.interact(rect, interact_id, egui::Sense::click());

        // Update colors based on interaction state
        let is_pressed = response.is_pointer_button_down_on();
        if is_pressed {
            if let Some(c) = pressed_bg {
                bg_color = color_to_egui(c);
            }
            if let Some(c) = pressed_border {
                border_color = color_to_egui(c);
            }
        } else if response.hovered() {
            if let Some(c) = hover_bg {
                bg_color = color_to_egui(c);
            }
            if let Some(c) = hover_border {
                border_color = color_to_egui(c);
            }
        }

        // Apply opacity
        let bg_color = apply_opacity(bg_color, button.props.opacity);
        let border_color = apply_opacity(border_color, button.props.opacity);

        // PAINT DIRECTLY TO PARENT UI (before creating scope)
        if bg_color != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, rounding, bg_color);
        }
        if button.props.border_thickness > 0.0 && border_color != Color32::TRANSPARENT {
            ui.painter().rect_stroke(
                rect,
                rounding,
                Stroke::new(button.props.border_thickness, border_color),
                egui::StrokeKind::Inside,
            );
        }

        // Handle events
        if response.clicked() {
            println!(
                "[UI] button clicked id={} name='{}'",
                button.base.id,
                button.base.name
            );
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

        // NOW create scope for children
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.set_clip_rect(rect);
            ui.set_opacity(button.props.opacity);
            for &child_id in button.get_children() {
                if let Some(child) = elements.get(&child_id) {
                    self.render_element_recursive(
                        child,
                        elements,
                        virtual_size,
                        screen_size,
                        ui,
                        api,
                    );
                }
            }
        });
    }

    /// Clear events (call after processing)
    pub fn clear_events(&mut self) {
        self.events.clear();
    }
}