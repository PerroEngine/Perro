use serde::{Deserialize, Serialize};

use crate::{
    fur_ast::FurAnchor,
    impl_ui_element,
    prelude::string_to_u64,
    structs2d::Vector2,
    ui_element::{BaseElement, BaseUIElement, UIElementUpdate, UIUpdateContext, is_point_in_rounded_rect},
    ui_elements::{
        ui_container::UIPanel,
        ui_text::UIText,
    },
};

/// A modular button that wraps panel and text functionality using composition
/// The button contains a panel and text element, and syncs their base properties
/// It handles mouse interactions and emits signals
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UIButton {
    pub base: BaseUIElement,
    
    // Composed elements - the button IS a panel with text
    pub panel: UIPanel,
    pub text: UIText,
    
    // Text anchor - controls where text is positioned within the button
    // Defaults to Center if not specified
    #[serde(default)]
    pub text_anchor: FurAnchor,
    
    // Optional hover and pressed background colors
    // If None, will use lightened/darkened version of base bg color
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover_bg: Option<crate::structs::Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pressed_bg: Option<crate::structs::Color>,
    
    // Internal state for mouse interactions (not serialized)
    #[serde(skip)]
    pub is_hovered: bool,
    #[serde(skip)]
    pub is_pressed: bool,
    #[serde(skip)]
    pub was_pressed_last_frame: bool,
}

impl Default for UIButton {
    fn default() -> Self {
        let base = BaseUIElement::default();
        let mut panel = UIPanel::default();
        let mut text = UIText::default();
        
        // Sync IDs so they're related but unique
        panel.base.id = uuid::Uuid::new_v5(&base.id, b"panel");
        text.base.id = uuid::Uuid::new_v5(&base.id, b"text");
        
        Self {
            base,
            panel,
            text,
            text_anchor: FurAnchor::Center, // Default text anchor to center
            hover_bg: None,
            pressed_bg: None,
            is_hovered: false,
            is_pressed: false,
            was_pressed_last_frame: false,
        }
    }
}

impl_ui_element!(UIButton);

impl UIButton {
    /// Create a new button with default properties
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Sync the button's base properties to the panel and text
    /// This should be called before rendering or layout calculations
    pub fn sync_base_to_children(&mut self) {
        // Sync all base properties from button to panel and text
        self.panel.base.id = uuid::Uuid::new_v5(&self.base.id, b"panel");
        self.text.base.id = uuid::Uuid::new_v5(&self.base.id, b"text");
        
        self.panel.base.name = format!("{}_panel", self.base.name);
        self.text.base.name = format!("{}_text", self.base.name);
        
        self.panel.base.parent_id = self.base.parent_id;
        self.text.base.parent_id = self.base.id;
        
        self.panel.base.visible = self.base.visible;
        self.text.base.visible = self.base.visible;
        
        self.panel.base.transform = self.base.transform;
        self.text.base.transform = self.base.transform;
        
        // Don't sync global_transform here - it's calculated in the layout system
        // self.panel.base.global_transform = self.base.global_transform;
        // self.text.base.global_transform = self.base.global_transform;
        
        self.panel.base.size = self.base.size;
        self.text.base.size = self.base.size;
        
        self.panel.base.pivot = self.base.pivot;
        // Text pivot is always center so text is centered on its anchor point
        self.text.base.pivot = Vector2::new(0.5, 0.5);
        
        // Panel uses the button's anchor (visual container)
        self.panel.base.anchor = self.base.anchor;
        // Text uses the button's text_anchor (defaults to center)
        self.text.base.anchor = self.text_anchor;
        
        self.panel.base.modulate = self.base.modulate;
        self.text.base.modulate = self.base.modulate;
        
        self.panel.base.z_index = self.base.z_index;
        self.text.base.z_index = self.base.z_index + 1; // Text renders on top
        
        self.panel.base.style_map = self.base.style_map.clone();
        self.text.base.style_map = self.base.style_map.clone();
    }
    
    /// Get a reference to the panel (for direct panel property access)
    pub fn panel(&self) -> &UIPanel {
        &self.panel
    }
    
    /// Get a mutable reference to the panel (for direct panel property access)
    pub fn panel_mut(&mut self) -> &mut UIPanel {
        &mut self.panel
    }
    
    /// Get a reference to the text (for direct text property access)
    pub fn text(&self) -> &UIText {
        &self.text
    }
    
    /// Get a mutable reference to the text (for direct text property access)
    pub fn text_mut(&mut self) -> &mut UIText {
        &mut self.text
    }
    
    // Convenience methods that forward to panel properties
    /// Get panel props (for direct access to panel properties)
    pub fn panel_props(&self) -> &crate::ui_elements::ui_container::UIPanelProps {
        &self.panel.props
    }
    
    /// Get mutable panel props
    pub fn panel_props_mut(&mut self) -> &mut crate::ui_elements::ui_container::UIPanelProps {
        &mut self.panel.props
    }
    
    /// Get text props (for direct access to text properties)
    pub fn text_props(&self) -> &crate::ui_elements::ui_text::TextProps {
        &self.text.props
    }
    
    /// Get mutable text props
    pub fn text_props_mut(&mut self) -> &mut crate::ui_elements::ui_text::TextProps {
        &mut self.text.props
    }
}

impl UIElementUpdate for UIButton {
    fn internal_render_update(&mut self, ctx: &mut UIUpdateContext) -> bool {
        if !self.get_visible() {
            return false;
        }

        let was_hovered = self.is_hovered;
        let was_pressed = self.is_pressed;

        // Size is stored as full size (not half-extents)
        // The renderer treats it as full size and halves it internally
        let size = *self.get_size();
        // Apply scale from transform
        let scaled_size = Vector2::new(
            size.x * self.global_transform.scale.x,
            size.y * self.global_transform.scale.y,
        );

        let center = self.global_transform.position;
        let corner_radius = self.panel_props().corner_radius;
        
        // Convert mouse position to button's local space (centered at origin)
        let local_pos = Vector2::new(
            ctx.mouse_pos.x - center.x,
            ctx.mouse_pos.y - center.y,
        );
        
        // Use rounded rectangle hit test
        let is_hovered = is_point_in_rounded_rect(
            local_pos,
            scaled_size,
            corner_radius,
        );

        self.is_hovered = is_hovered;
        self.is_pressed = is_hovered && ctx.mouse_pressed;
        
        let name = self.get_name();
        
        // Check if state changed
        let state_changed = (is_hovered != was_hovered) || (self.is_pressed != was_pressed);
        
        if is_hovered != was_hovered {
            let signal = if is_hovered {
                format!("{}_Hovered", name)
            } else {
                format!("{}_NotHovered", name)
            };
            ctx.api.emit_signal_id(string_to_u64(&signal), &[]);
        }
        
        if self.is_pressed && !was_pressed {
            ctx.api.emit_signal_id(
                string_to_u64(&format!("{}_Pressed", name)),
                &[],
            );
        } else if !self.is_pressed && was_pressed && was_hovered {
            ctx.api.emit_signal_id(
                string_to_u64(&format!("{}_Released", name)),
                &[],
            );
        }
        
        self.was_pressed_last_frame = self.is_pressed;
        
        if state_changed {
            (ctx.mark_dirty)(self.get_id());
            (ctx.mark_ui_dirty)();
        }
        
        state_changed
    }
}
