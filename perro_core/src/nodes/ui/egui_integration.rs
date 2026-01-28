//! Complete egui integration for Perro UI system
//! Maps FUR elements to egui widgets and handles rendering

use std::collections::HashMap;
use crate::ids::UIElementID;
use egui::{Context, Ui, Rect, Vec2, Color32, Rounding, Stroke, FontId, TextEdit, Button, Frame, Layout, Align, Direction, RichText};
use crate::{
    ui_element::{UIElement, BaseElement},
    structs::Color,
    structs2d::{Vector2, Transform2D},
    fur_ast::FurAnchor,
    ui_elements::{
        ui_container::{UIPanel, CornerRadius, Layout as UILayout, GridLayout, VLayout, HLayout, LayoutAlignment},
        ui_text::UIText,
    },
    prelude::string_to_u64,
};

/// Converts Perro Color to egui Color32
fn color_to_egui(color: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

/// Converts Perro CornerRadius to egui Rounding
/// egui 0.33 uses u8 for rounding values
fn corner_radius_to_egui(corner: &CornerRadius) -> Rounding {
    Rounding {
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
    let scaled_size = Vector2::new(
        size.x * transform.scale.x,
        size.y * transform.scale.y,
    );
    
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

/// Converts FurAnchor to egui Align
fn anchor_to_egui_align(anchor: FurAnchor) -> Align {
    match anchor {
        FurAnchor::TopLeft | FurAnchor::Left | FurAnchor::BottomLeft => Align::LEFT,
        FurAnchor::TopRight | FurAnchor::Right | FurAnchor::BottomRight => Align::RIGHT,
        FurAnchor::Top | FurAnchor::Bottom | FurAnchor::Center => Align::Center,
    }
}

/// Converts LayoutAlignment to egui Align (for vertical layouts)
fn layout_align_to_egui_align_vertical(align: LayoutAlignment) -> Align {
    match align {
        LayoutAlignment::Start => Align::TOP,    // Top in vertical layout
        LayoutAlignment::Center => Align::Center,
        LayoutAlignment::End => Align::BOTTOM,   // Bottom in vertical layout
    }
}

/// Converts LayoutAlignment to egui Align (for horizontal layouts)
fn layout_align_to_egui_align_horizontal(align: LayoutAlignment) -> Align {
    match align {
        LayoutAlignment::Start => Align::LEFT,   // Left in horizontal layout
        LayoutAlignment::Center => Align::Center,
        LayoutAlignment::End => Align::RIGHT,    // Right in horizontal layout
    }
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
    ButtonHovered(UIElementID, bool),    // element_id, is_hovered
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
        elements: &indexmap::IndexMap<UIElementID, UIElement>,
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
        elements: &indexmap::IndexMap<UIElementID, UIElement>,
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
            UIElement::Text(text) => {
                self.render_text(text, ui);
            }
            _ => {
                // Other elements - just render children
                self.render_children(element, elements, ui, api);
            }
        }
    }

    /// Render children of an element
    fn render_children(
        &mut self,
        element: &UIElement,
        elements: &indexmap::IndexMap<UIElementID, UIElement>,
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
        elements: &indexmap::IndexMap<UIElementID, UIElement>,
        ui: &mut Ui,
        api: &mut Option<&mut crate::scripting::api::ScriptApi>,
    ) {
        log::debug!("🎨 [EGUI] render_panel_with_children: '{}' -> egui Frame", panel.base.name);
        let rect = transform_to_rect(&panel.base.global_transform, &panel.base.size, &panel.base.pivot);
        
        let bg_color = panel.props.background_color
            .map(color_to_egui)
            .unwrap_or(Color32::TRANSPARENT);
        let border_color = panel.props.border_color
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
                .font(font_id)
        );
    }

    /// Render Button (wraps egui Button) with signal support
    /// NOTE: UIButton type no longer exists - this function is kept for potential future use
    #[allow(dead_code)]
    pub fn render_button(
        &mut self,
        _button: &dyn std::any::Any, // Placeholder - UIButton was removed
        _ui: &mut Ui,
        _api: &mut Option<&mut crate::scripting::api::ScriptApi>,
    ) {
        // UIButton type was removed - function body removed
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

    /// Render Layout (deprecated - use VLayout or HLayout) using egui layouts with children
    pub fn render_layout_with_children(
        &mut self,
        layout: &UILayout,
        elements: &indexmap::IndexMap<UIElementID, UIElement>,
        ui: &mut Ui,
        api: &mut Option<&mut crate::scripting::api::ScriptApi>,
    ) {
        let rect = transform_to_rect(&layout.base.global_transform, &layout.base.size, &layout.base.pivot);
        let direction = match layout.container.mode {
            crate::ui_elements::ui_container::ContainerMode::Horizontal => Direction::LeftToRight,
            crate::ui_elements::ui_container::ContainerMode::Vertical => Direction::TopDown,
            crate::ui_elements::ui_container::ContainerMode::Grid => Direction::LeftToRight, // Grid handled separately
        };
        let align = layout_align_to_egui_align_horizontal(layout.container.align);
        
        // Create egui layout
        let egui_layout = match direction {
            Direction::LeftToRight => Layout::left_to_right(align),
            Direction::TopDown => Layout::top_down(align),
            Direction::RightToLeft => Layout::right_to_left(align),
            Direction::BottomUp => Layout::bottom_up(align),
        };
        
        ui.allocate_ui_at_rect(rect, |ui| {
            ui.with_layout(egui_layout, |ui| {
                // Apply padding
                ui.add_space(layout.container.padding.left);
                
                // Render children directly from layout
                for &child_id in &layout.base.children {
                    if let Some(child) = elements.get(&child_id) {
                        self.render_element_recursive(child, elements, ui, api);
                    }
                }
            });
        });
    }

    /// Render VLayout (vertical layout) using egui layouts with children
    pub fn render_vlayout_with_children(
        &mut self,
        vlayout: &VLayout,
        elements: &indexmap::IndexMap<UIElementID, UIElement>,
        ui: &mut Ui,
        api: &mut Option<&mut crate::scripting::api::ScriptApi>,
    ) {
        let rect = transform_to_rect(&vlayout.base.global_transform, &vlayout.base.size, &vlayout.base.pivot);
        let align = layout_align_to_egui_align_vertical(vlayout.align);
        
        // Create vertical egui layout
        let egui_layout = Layout::top_down(align);
        
        ui.allocate_ui_at_rect(rect, |ui| {
            ui.with_layout(egui_layout, |ui| {
                // Apply padding
                ui.add_space(vlayout.padding.top);
                
                // Render children with gap
                for (idx, &child_id) in vlayout.base.children.iter().enumerate() {
                    if idx > 0 {
                        ui.add_space(vlayout.gap.y);
                    }
                    if let Some(child) = elements.get(&child_id) {
                        self.render_element_recursive(child, elements, ui, api);
                    }
                }
            });
        });
    }

    /// Render HLayout (horizontal layout) using egui layouts with children
    pub fn render_hlayout_with_children(
        &mut self,
        hlayout: &HLayout,
        elements: &indexmap::IndexMap<UIElementID, UIElement>,
        ui: &mut Ui,
        api: &mut Option<&mut crate::scripting::api::ScriptApi>,
    ) {
        let rect = transform_to_rect(&hlayout.base.global_transform, &hlayout.base.size, &hlayout.base.pivot);
        let align = layout_align_to_egui_align_horizontal(hlayout.align);
        
        // Create horizontal egui layout
        let egui_layout = Layout::left_to_right(align);
        
        ui.allocate_ui_at_rect(rect, |ui| {
            ui.with_layout(egui_layout, |ui| {
                // Apply padding
                ui.add_space(hlayout.padding.left);
                
                // Render children with gap
                for (idx, &child_id) in hlayout.base.children.iter().enumerate() {
                    if idx > 0 {
                        ui.add_space(hlayout.gap.x);
                    }
                    if let Some(child) = elements.get(&child_id) {
                        self.render_element_recursive(child, elements, ui, api);
                    }
                }
            });
        });
    }

    /// Render GridLayout using egui Grid with children
    pub fn render_grid_with_children(
        &mut self,
        grid: &GridLayout,
        elements: &indexmap::IndexMap<UIElementID, UIElement>,
        ui: &mut Ui,
        api: &mut Option<&mut crate::scripting::api::ScriptApi>,
    ) {
        let rect = transform_to_rect(&grid.base.global_transform, &grid.base.size, &grid.base.pivot);
        
        ui.allocate_ui_at_rect(rect, |ui| {
            // Apply padding
            ui.add_space(grid.padding.left);
            ui.add_space(grid.padding.top);
            
            // Use egui Grid for proper grid layout
            use egui::Grid;
            Grid::new("grid_layout")
                .num_columns(grid.cols)
                .spacing([grid.gap.x, grid.gap.y])
                .show(ui, |ui| {
                    for &child_id in &grid.base.children {
                        if let Some(child) = elements.get(&child_id) {
                            self.render_element_recursive(child, elements, ui, api);
                        }
                    }
                });
        });
    }

    /// Clear events (call after processing)
    pub fn clear_events(&mut self) {
        self.events.clear();
    }
}
