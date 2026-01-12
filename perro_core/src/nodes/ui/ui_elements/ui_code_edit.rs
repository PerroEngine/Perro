use serde::{Deserialize, Serialize};

use crate::{
    fur_ast::FurAnchor,
    impl_ui_element,
    structs2d::Vector2,
    ui_element::BaseUIElement,
    ui_elements::{
        ui_container::UIPanel,
        ui_text::UIText,
        ui_text_edit::UITextEdit,
    },
};

/// A code editor with line numbers on the left side
/// Composes UITextEdit with a line number panel
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UICodeEdit {
    pub base: BaseUIElement,
    
    // The text editor component
    pub text_edit: UITextEdit,
    
    // Line number panel (displayed on the left)
    pub line_number_panel: UIPanel,
    
    // Line number text (for rendering line numbers)
    pub line_number_text: UIText,
    
    // Width of the line number area (in pixels)
    #[serde(default = "default_line_number_width")]
    pub line_number_width: f32,
    
    // Optional hover and focused background colors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover_bg: Option<crate::structs::Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_bg: Option<crate::structs::Color>,
    
    // Internal state (not serialized)
    #[serde(skip)]
    pub is_hovered: bool,
    #[serde(skip)]
    pub is_focused: bool,
}

fn default_line_number_width() -> f32 {
    60.0 // Default width for line numbers
}

impl Default for UICodeEdit {
    fn default() -> Self {
        let base = BaseUIElement::default();
        let text_edit = UITextEdit::default();
        let mut line_number_panel = UIPanel::default();
        let mut line_number_text = UIText::default();
        
        // Set IDs
        line_number_panel.base.id = uuid::Uuid::new_v5(&base.id, b"line_numbers_panel");
        line_number_text.base.id = uuid::Uuid::new_v5(&base.id, b"line_numbers_text");
        
        // Configure line number panel
        line_number_panel.props.background_color = Some(crate::structs::Color::new(40, 40, 40, 255));
        line_number_panel.base.size = Vector2::new(60.0, base.size.y);
        
        // Configure line number text
        line_number_text.props.content = "1".to_string();
        line_number_text.props.font_size = 12.0;
        line_number_text.props.color = crate::structs::Color::new(128, 128, 128, 255);
        line_number_text.props.align = crate::ui_elements::ui_text::TextFlow::End; // Right-aligned
        
        Self {
            base,
            text_edit,
            line_number_panel,
            line_number_text,
            line_number_width: 60.0,
            hover_bg: None,
            focused_bg: None,
            is_hovered: false,
            is_focused: false,
        }
    }
}

impl_ui_element!(UICodeEdit);

impl UICodeEdit {
    /// Create a new code edit with default properties
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Sync the code edit's base properties to its children
    pub fn sync_base_to_children(&mut self) {
        // Sync text edit
        self.text_edit.base.id = uuid::Uuid::new_v5(&self.base.id, b"text_edit");
        self.text_edit.base.name = format!("{}_text_edit", self.base.name);
        self.text_edit.base.parent_id = self.base.parent_id;
        self.text_edit.base.visible = self.base.visible;
        self.text_edit.base.transform = self.base.transform;
        self.text_edit.base.size = Vector2::new(
            self.base.size.x - self.line_number_width,
            self.base.size.y,
        );
        self.text_edit.base.anchor = FurAnchor::TopLeft;
        self.text_edit.base.modulate = self.base.modulate;
        self.text_edit.base.z_index = self.base.z_index;
        self.text_edit.sync_base_to_children();
        
        // Adjust text edit position to account for line numbers
        self.text_edit.base.transform.position.x += self.line_number_width * 0.5;
        
        // Sync line number panel
        self.line_number_panel.base.id = uuid::Uuid::new_v5(&self.base.id, b"line_numbers_panel");
        self.line_number_panel.base.name = format!("{}_line_numbers_panel", self.base.name);
        self.line_number_panel.base.parent_id = self.base.parent_id;
        self.line_number_panel.base.visible = self.base.visible;
        self.line_number_panel.base.transform = self.base.transform.clone();
        self.line_number_panel.base.size = Vector2::new(self.line_number_width, self.base.size.y);
        self.line_number_panel.base.anchor = FurAnchor::TopLeft;
        self.line_number_panel.base.pivot = Vector2::new(0.0, 0.5);
        self.line_number_panel.base.modulate = self.base.modulate;
        self.line_number_panel.base.z_index = self.base.z_index;
        
        // Position line number panel on the left
        self.line_number_panel.base.transform.position.x -= (self.base.size.x - self.line_number_width) * 0.5;
        
        // Update line number text content
        self.update_line_numbers();
    }
    
    /// Update the line number text to show current line numbers
    pub fn update_line_numbers(&mut self) {
        let line_count = self.text_edit.line_count();
        let mut line_numbers = String::new();
        
        for i in 1..=line_count {
            line_numbers.push_str(&format!("{}\n", i));
        }
        
        // Remove trailing newline
        if line_numbers.ends_with('\n') {
            line_numbers.pop();
        }
        
        self.line_number_text.props.content = line_numbers;
    }
    
    /// Get panel props (from text edit's panel)
    pub fn panel_props(&self) -> &crate::ui_elements::ui_container::UIPanelProps {
        self.text_edit.panel_props()
    }
    
    /// Get mutable panel props
    pub fn panel_props_mut(&mut self) -> &mut crate::ui_elements::ui_container::UIPanelProps {
        self.text_edit.panel_props_mut()
    }
    
    /// Get text props (from text edit's text)
    pub fn text_props(&self) -> &crate::ui_elements::ui_text::TextProps {
        self.text_edit.text_props()
    }
    
    /// Get mutable text props
    pub fn text_props_mut(&mut self) -> &mut crate::ui_elements::ui_text::TextProps {
        self.text_edit.text_props_mut()
    }
    
    /// Get the current text content
    pub fn get_text(&self) -> &str {
        self.text_edit.get_text()
    }
    
    /// Set the text content
    pub fn set_text(&mut self, text: &str) {
        self.text_edit.set_text(text);
        self.update_line_numbers();
    }
    
    /// Insert text at the current cursor position
    pub fn insert_text(&mut self, text: &str) {
        self.text_edit.insert_text(text);
        self.update_line_numbers();
    }
    
    /// Insert a newline
    pub fn insert_newline(&mut self) {
        self.text_edit.insert_newline();
        self.update_line_numbers();
    }
    
    /// Delete backward
    pub fn delete_backward(&mut self) {
        self.text_edit.delete_backward();
        self.update_line_numbers();
    }
    
    /// Delete forward
    pub fn delete_forward(&mut self) {
        self.text_edit.delete_forward();
        self.update_line_numbers();
    }
    
    /// Move cursor left
    pub fn move_cursor_left(&mut self) {
        self.text_edit.move_cursor_left();
    }
    
    /// Move cursor right
    pub fn move_cursor_right(&mut self) {
        self.text_edit.move_cursor_right();
    }
    
    /// Move cursor up
    pub fn move_cursor_up(&mut self) {
        self.text_edit.move_cursor_up();
    }
    
    /// Move cursor down
    pub fn move_cursor_down(&mut self) {
        self.text_edit.move_cursor_down();
    }
    
    /// Move cursor to line start
    pub fn move_cursor_line_start(&mut self) {
        self.text_edit.move_cursor_line_start();
    }
    
    /// Move cursor to line end
    pub fn move_cursor_line_end(&mut self) {
        self.text_edit.move_cursor_line_end();
    }
    
    /// Move cursor home
    pub fn move_cursor_home(&mut self) {
        self.text_edit.move_cursor_home();
    }
    
    /// Move cursor end
    pub fn move_cursor_end(&mut self) {
        self.text_edit.move_cursor_end();
    }
    
    /// Update cursor blink
    pub fn update_cursor_blink(&mut self, delta_time: f32) {
        self.text_edit.update_cursor_blink(delta_time);
    }
    
    /// Check if cursor is visible
    pub fn is_cursor_visible(&self) -> bool {
        self.text_edit.is_cursor_visible()
    }
    
    /// Get cursor position
    pub fn cursor_pos(&self) -> crate::ui_elements::ui_text_edit::CursorPos {
        self.text_edit.cursor_pos
    }
    
    // ===== Selection Methods (forward to text_edit) =====
    
    /// Check if there is an active selection
    pub fn has_selection(&self) -> bool {
        self.text_edit.has_selection()
    }
    
    /// Get selected text
    pub fn get_selected_text(&self) -> Option<String> {
        self.text_edit.get_selected_text()
    }
    
    /// Clear selection
    pub fn clear_selection(&mut self) {
        self.text_edit.clear_selection();
    }
    
    /// Select all text
    pub fn select_all(&mut self) {
        self.text_edit.select_all();
    }
    
    /// Copy selected text to clipboard
    pub fn copy_to_clipboard(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.text_edit.copy_to_clipboard()
    }
    
    /// Cut selected text to clipboard
    pub fn cut_to_clipboard(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.text_edit.cut_to_clipboard()
    }
    
    /// Paste text from clipboard
    pub fn paste_from_clipboard(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.text_edit.paste_from_clipboard()
    }
}
