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

/// Cursor position in multiline text (line and column)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorPos {
    pub line: usize,
    pub column: usize,
}

impl Default for CursorPos {
    fn default() -> Self {
        Self { line: 0, column: 0 }
    }
}

/// A multiline text editor that can be focused and edited
/// Similar to UITextInput but supports multiple lines
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UITextEdit {
    pub base: BaseUIElement,
    
    // Composed elements - the text edit IS a panel with text
    pub panel: UIPanel,
    pub text: UIText,
    
    // Text anchor - controls where text is positioned within the editor
    // Defaults to TopLeft for multiline text
    #[serde(default)]
    pub text_anchor: FurAnchor,
    
    // Optional hover and focused background colors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover_bg: Option<crate::structs::Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_bg: Option<crate::structs::Color>,
    
    // Internal state for interactions (not serialized)
    #[serde(skip)]
    pub is_hovered: bool,
    #[serde(skip)]
    pub is_focused: bool,
    
    // Cursor position (line and column)
    #[serde(skip)]
    pub cursor_pos: CursorPos,
    
    // Cached line starts for efficient navigation
    #[serde(skip)]
    pub line_starts: Vec<usize>,
    
    // Scroll offset (for scrolling through long text)
    #[serde(skip)]
    pub scroll_offset: Vector2,
    
    // Cursor blink timer (for visual blinking effect)
    #[serde(skip)]
    pub cursor_blink_timer: f32,
}

impl Default for UITextEdit {
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
            text_anchor: FurAnchor::TopLeft, // Default to top-left for multiline
            hover_bg: None,
            focused_bg: None,
            is_hovered: false,
            is_focused: false,
            cursor_pos: CursorPos::default(),
            line_starts: vec![0], // First line starts at position 0
            scroll_offset: Vector2::new(0.0, 0.0),
            cursor_blink_timer: 0.0,
        }
    }
}

impl_ui_element!(UITextEdit);

impl UITextEdit {
    /// Create a new text edit with default properties
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Sync the text edit's base properties to the panel and text
    pub fn sync_base_to_children(&mut self) {
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
        
        self.panel.base.size = self.base.size;
        self.text.base.size = self.base.size;
        
        self.panel.base.pivot = self.base.pivot;
        self.text.base.pivot = Vector2::new(0.5, 0.5);
        
        self.panel.base.anchor = self.base.anchor;
        self.text.base.anchor = self.text_anchor;
        
        self.panel.base.modulate = self.base.modulate;
        self.text.base.modulate = self.base.modulate;
        
        self.panel.base.z_index = self.base.z_index;
        self.text.base.z_index = self.base.z_index + 1;
        
        self.panel.base.style_map = self.base.style_map.clone();
        self.text.base.style_map = self.base.style_map.clone();
        
        // Rebuild line starts when syncing
        self.rebuild_line_starts();
    }
    
    /// Rebuild the line_starts cache from the current text content
    pub fn rebuild_line_starts(&mut self) {
        let content = &self.text.props.content;
        self.line_starts.clear();
        self.line_starts.push(0);
        
        for (i, ch) in content.char_indices() {
            if ch == '\n' {
                self.line_starts.push(i + 1);
            }
        }
        
        // Ensure cursor position is valid
        self.clamp_cursor_pos();
    }
    
    /// Clamp cursor position to valid range
    fn clamp_cursor_pos(&mut self) {
        let line_count = self.line_starts.len();
        if self.cursor_pos.line >= line_count {
            self.cursor_pos.line = line_count.saturating_sub(1);
        }
        
        let line_start = self.line_starts[self.cursor_pos.line];
        let line_end = if self.cursor_pos.line + 1 < self.line_starts.len() {
            self.line_starts[self.cursor_pos.line + 1] - 1
        } else {
            self.text.props.content.len()
        };
        
        let line_len = line_end.saturating_sub(line_start);
        if self.cursor_pos.column > line_len {
            self.cursor_pos.column = line_len;
        }
    }
    
    /// Convert cursor position (line, column) to character index
    pub fn cursor_pos_to_char_index(&self) -> usize {
        if self.cursor_pos.line >= self.line_starts.len() {
            return self.text.props.content.len();
        }
        
        let line_start = self.line_starts[self.cursor_pos.line];
        line_start + self.cursor_pos.column.min(
            if self.cursor_pos.line + 1 < self.line_starts.len() {
                self.line_starts[self.cursor_pos.line + 1] - line_start - 1
            } else {
                self.text.props.content.len() - line_start
            }
        )
    }
    
    /// Convert character index to cursor position (line, column)
    pub fn char_index_to_cursor_pos(&self, char_index: usize) -> CursorPos {
        let char_index = char_index.min(self.text.props.content.len());
        
        // Find which line this character index belongs to
        for (line_idx, &line_start) in self.line_starts.iter().enumerate() {
            let line_end = if line_idx + 1 < self.line_starts.len() {
                self.line_starts[line_idx + 1] - 1
            } else {
                self.text.props.content.len()
            };
            
            if char_index >= line_start && char_index <= line_end {
                return CursorPos {
                    line: line_idx,
                    column: char_index - line_start,
                };
            }
        }
        
        // Fallback: return position at end of last line
        let last_line = self.line_starts.len().saturating_sub(1);
        CursorPos {
            line: last_line,
            column: self.text.props.content.len().saturating_sub(
                if last_line < self.line_starts.len() {
                    self.line_starts[last_line]
                } else {
                    0
                }
            ),
        }
    }
    
    /// Get panel props
    pub fn panel_props(&self) -> &crate::ui_elements::ui_container::UIPanelProps {
        &self.panel.props
    }
    
    /// Get mutable panel props
    pub fn panel_props_mut(&mut self) -> &mut crate::ui_elements::ui_container::UIPanelProps {
        &mut self.panel.props
    }
    
    /// Get text props
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
        self.rebuild_line_starts();
        self.clamp_cursor_pos();
    }
    
    /// Insert text at the current cursor position
    pub fn insert_text(&mut self, text: &str) {
        let char_index = self.cursor_pos_to_char_index();
        let content = &mut self.text.props.content;
        content.insert_str(char_index, text);
        
        // Rebuild line starts and update cursor position
        self.rebuild_line_starts();
        
        // Move cursor forward by the number of characters inserted
        let new_char_index = char_index + text.len();
        self.cursor_pos = self.char_index_to_cursor_pos(new_char_index);
    }
    
    /// Insert a newline at the current cursor position
    pub fn insert_newline(&mut self) {
        self.insert_text("\n");
    }
    
    /// Delete the character before the cursor (backspace)
    pub fn delete_backward(&mut self) {
        let char_index = self.cursor_pos_to_char_index();
        if char_index > 0 {
            let content = &mut self.text.props.content;
            content.remove(char_index - 1);
            self.rebuild_line_starts();
            self.cursor_pos = self.char_index_to_cursor_pos(char_index - 1);
        }
    }
    
    /// Delete the character at the cursor (delete key)
    pub fn delete_forward(&mut self) {
        let char_index = self.cursor_pos_to_char_index();
        let content = &mut self.text.props.content;
        if char_index < content.len() {
            content.remove(char_index);
            self.rebuild_line_starts();
            // Cursor position stays the same (character was deleted)
        }
    }
    
    /// Move cursor left
    pub fn move_cursor_left(&mut self) {
        let char_index = self.cursor_pos_to_char_index();
        if char_index > 0 {
            self.cursor_pos = self.char_index_to_cursor_pos(char_index - 1);
        }
    }
    
    /// Move cursor right
    pub fn move_cursor_right(&mut self) {
        let char_index = self.cursor_pos_to_char_index();
        let content = &self.text.props.content;
        if char_index < content.len() {
            self.cursor_pos = self.char_index_to_cursor_pos(char_index + 1);
        }
    }
    
    /// Move cursor up
    pub fn move_cursor_up(&mut self) {
        if self.cursor_pos.line > 0 {
            let prev_line = self.cursor_pos.line - 1;
            let prev_line_start = self.line_starts[prev_line];
            let prev_line_end = if prev_line + 1 < self.line_starts.len() {
                self.line_starts[prev_line + 1] - 1
            } else {
                self.text.props.content.len()
            };
            let prev_line_len = prev_line_end.saturating_sub(prev_line_start);
            
            // Try to maintain column position, but clamp to line length
            let target_column = self.cursor_pos.column.min(prev_line_len);
            self.cursor_pos = CursorPos {
                line: prev_line,
                column: target_column,
            };
        }
    }
    
    /// Move cursor down
    pub fn move_cursor_down(&mut self) {
        if self.cursor_pos.line + 1 < self.line_starts.len() {
            let next_line = self.cursor_pos.line + 1;
            let next_line_start = self.line_starts[next_line];
            let next_line_end = if next_line + 1 < self.line_starts.len() {
                self.line_starts[next_line + 1] - 1
            } else {
                self.text.props.content.len()
            };
            let next_line_len = next_line_end.saturating_sub(next_line_start);
            
            // Try to maintain column position, but clamp to line length
            let target_column = self.cursor_pos.column.min(next_line_len);
            self.cursor_pos = CursorPos {
                line: next_line,
                column: target_column,
            };
        }
    }
    
    /// Move cursor to the start of the current line
    pub fn move_cursor_line_start(&mut self) {
        self.cursor_pos.column = 0;
    }
    
    /// Move cursor to the end of the current line
    pub fn move_cursor_line_end(&mut self) {
        let line_start = self.line_starts[self.cursor_pos.line];
        let line_end = if self.cursor_pos.line + 1 < self.line_starts.len() {
            self.line_starts[self.cursor_pos.line + 1] - 1
        } else {
            self.text.props.content.len()
        };
        self.cursor_pos.column = line_end.saturating_sub(line_start);
    }
    
    /// Move cursor to the start of the text
    pub fn move_cursor_home(&mut self) {
        self.cursor_pos = CursorPos::default();
    }
    
    /// Move cursor to the end of the text
    pub fn move_cursor_end(&mut self) {
        let last_line = self.line_starts.len().saturating_sub(1);
        let line_start = self.line_starts[last_line];
        let line_end = self.text.props.content.len();
        self.cursor_pos = CursorPos {
            line: last_line,
            column: line_end.saturating_sub(line_start),
        };
    }
    
    /// Update cursor blink timer (call each frame)
    pub fn update_cursor_blink(&mut self, delta_time: f32) {
        self.cursor_blink_timer += delta_time;
        if self.cursor_blink_timer >= 1.0 {
            self.cursor_blink_timer = 0.0;
        }
    }
    
    /// Check if cursor should be visible (for blinking effect)
    pub fn is_cursor_visible(&self) -> bool {
        self.cursor_blink_timer < 0.5
    }
    
    /// Get the number of lines in the text
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }
    
    /// Get a specific line of text
    pub fn get_line(&self, line_index: usize) -> &str {
        if line_index >= self.line_starts.len() {
            return "";
        }
        
        let line_start = self.line_starts[line_index];
        let line_end = if line_index + 1 < self.line_starts.len() {
            self.line_starts[line_index + 1] - 1
        } else {
            self.text.props.content.len()
        };
        
        &self.text.props.content[line_start..line_end]
    }
}
