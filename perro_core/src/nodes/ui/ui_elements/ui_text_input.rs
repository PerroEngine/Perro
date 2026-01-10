use serde::{Deserialize, Serialize};

use crate::{
    fur_ast::FurAnchor,
    impl_ui_element,
    structs2d::Vector2,
    ui_element::BaseUIElement,
    ui_elements::{
        ui_container::UIPanel,
        ui_text::UIText,
    },
};

/// A text input field that can be focused and edited
/// Similar to UIButton but handles keyboard input and displays a cursor
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UITextInput {
    pub base: BaseUIElement,
    
    // Composed elements - the text input IS a panel with text
    pub panel: UIPanel,
    pub text: UIText,
    
    // Text anchor - controls where text is positioned within the input
    // Defaults to Center if not specified
    #[serde(default)]
    pub text_anchor: FurAnchor,
    
    // Optional hover and focused background colors
    // If None, will use lightened/darkened version of base bg color
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover_bg: Option<crate::structs::Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_bg: Option<crate::structs::Color>,
    
    // Internal state for interactions (not serialized)
    #[serde(skip)]
    pub is_hovered: bool,
    #[serde(skip)]
    pub is_focused: bool,
    
    // Cursor position (character index in the text)
    #[serde(skip)]
    pub cursor_position: usize,
    
    // Cursor blink timer (for visual blinking effect)
    #[serde(skip)]
    pub cursor_blink_timer: f32,
    
    // Horizontal scroll offset in pixels (how many pixels to shift text left)
    #[serde(skip)]
    pub scroll_offset: f32,
    
    // Cached text measurements for performance
    #[serde(skip)]
    pub cached_text_width: f32,
    #[serde(skip)]
    pub cached_text_content: String,
    #[serde(skip)]
    pub cached_char_positions: Vec<f32>, // Cumulative width at each character position
}

impl Default for UITextInput {
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
            focused_bg: None,
            is_hovered: false,
            is_focused: false,
            cursor_position: 0,
            cursor_blink_timer: 0.0,
            scroll_offset: 0.0,
            cached_text_width: 0.0,
            cached_text_content: String::new(),
            cached_char_positions: Vec::new(),
        }
    }
}

impl_ui_element!(UITextInput);

impl UITextInput {
    /// Create a new text input with default properties
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Sync the text input's base properties to the panel and text
    /// This should be called before rendering or layout calculations
    pub fn sync_base_to_children(&mut self) {
        // Sync all base properties from text input to panel and text
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
        
        self.panel.base.size = self.base.size;
        self.text.base.size = self.base.size;
        
        self.panel.base.pivot = self.base.pivot;
        // Text pivot is always center so text is centered on its anchor point
        self.text.base.pivot = Vector2::new(0.5, 0.5);
        
        // Panel uses the text input's anchor (visual container)
        self.panel.base.anchor = self.base.anchor;
        // Text uses the text input's text_anchor (defaults to center)
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
    
    /// Get the current text content
    pub fn get_text(&self) -> &str {
        &self.text.props.content
    }
    
    /// Set the text content
    pub fn set_text(&mut self, text: &str) {
        self.text.props.content = text.to_string();
        // Clamp cursor position to valid range
        self.cursor_position = self.cursor_position.min(self.text.props.content.len());
        // Invalidate cache when text changes
        self.invalidate_cache();
    }
    
    /// Insert text at the current cursor position
    pub fn insert_text(&mut self, text: &str) {
        let content = &mut self.text.props.content;
        let pos = self.cursor_position.min(content.len());
        content.insert_str(pos, text);
        self.cursor_position += text.len();
        // Invalidate cache when text changes
        self.invalidate_cache();
    }
    
    /// Delete the character before the cursor (backspace)
    pub fn delete_backward(&mut self) {
        if self.cursor_position > 0 {
            let content = &mut self.text.props.content;
            let pos = self.cursor_position - 1;
            content.remove(pos);
            self.cursor_position -= 1;
            // Invalidate cache when text changes
            self.invalidate_cache();
        }
    }
    
    /// Delete the character at the cursor (delete key)
    pub fn delete_forward(&mut self) {
        let content = &mut self.text.props.content;
        if self.cursor_position < content.len() {
            content.remove(self.cursor_position);
            // Invalidate cache when text changes
            self.invalidate_cache();
        }
    }
    
    /// Invalidate cached measurements when text content changes
    fn invalidate_cache(&mut self) {
        self.cached_text_content.clear();
        self.cached_text_width = 0.0;
        self.cached_char_positions.clear();
    }
    
    /// Move cursor left
    pub fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }
    
    /// Move cursor right
    pub fn move_cursor_right(&mut self) {
        let content = &self.text.props.content;
        if self.cursor_position < content.len() {
            self.cursor_position += 1;
        }
    }
    
    /// Move cursor to the start of the text
    pub fn move_cursor_home(&mut self) {
        self.cursor_position = 0;
    }
    
    /// Move cursor to the end of the text
    pub fn move_cursor_end(&mut self) {
        self.cursor_position = self.text.props.content.len();
    }
    
    /// Update cursor blink timer (call each frame)
    pub fn update_cursor_blink(&mut self, delta_time: f32) {
        self.cursor_blink_timer += delta_time;
        // Blink every 0.5 seconds
        if self.cursor_blink_timer >= 1.0 {
            self.cursor_blink_timer = 0.0;
        }
    }
    
    /// Check if cursor should be visible (for blinking effect)
    pub fn is_cursor_visible(&self) -> bool {
        self.cursor_blink_timer < 0.5
    }
}
