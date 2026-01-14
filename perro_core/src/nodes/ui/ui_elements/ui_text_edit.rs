use serde::{Deserialize, Serialize};
use winit::keyboard::KeyCode;

use crate::{
    fur_ast::FurAnchor,
    impl_ui_element,
    structs2d::Vector2,
    ui_element::{BaseElement, BaseUIElement, UIElementUpdate, UIUpdateContext, is_point_in_rounded_rect},
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
    
    // Text selection state
    #[serde(skip)]
    pub selection_start: Option<CursorPos>,  // None = no selection
    #[serde(skip)]
    pub selection_end: Option<CursorPos>,
    #[serde(skip)]
    pub is_dragging: bool,  // True while mouse is being dragged for selection
    
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
            selection_start: None,
            selection_end: None,
            is_dragging: false,
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
        // Blink every 2 seconds (1 second visible, 1 second hidden)
        if self.cursor_blink_timer >= 2.0 {
            self.cursor_blink_timer = 0.0;
        }
    }

    /// Check if cursor should be visible (for blinking effect)
    pub fn is_cursor_visible(&self) -> bool {
        self.cursor_blink_timer < 1.0
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
    
    // ===== Selection Methods =====
    
    /// Convert CursorPos to byte index in the text
    fn cursor_pos_to_index(&self, pos: &CursorPos) -> usize {
        if pos.line >= self.line_starts.len() {
            return self.text.props.content.len();
        }
        let line_start = self.line_starts[pos.line];
        let line_end = if pos.line + 1 < self.line_starts.len() {
            self.line_starts[pos.line + 1] - 1
        } else {
            self.text.props.content.len()
        };
        (line_start + pos.column).min(line_end)
    }
    
    /// Check if there is an active selection
    pub fn has_selection(&self) -> bool {
        self.selection_start.is_some() && self.selection_end.is_some()
    }
    
    /// Get the selection range as byte indices (start, end) in sorted order
    pub fn get_selection_range(&self) -> Option<(usize, usize)> {
        match (self.selection_start.as_ref(), self.selection_end.as_ref()) {
            (Some(start), Some(end)) => {
                let start_idx = self.cursor_pos_to_index(start);
                let end_idx = self.cursor_pos_to_index(end);
                let min = start_idx.min(end_idx);
                let max = start_idx.max(end_idx);
                Some((min, max))
            }
            _ => None,
        }
    }
    
    /// Get the currently selected text
    pub fn get_selected_text(&self) -> Option<String> {
        self.get_selection_range().map(|(start, end)| {
            self.text.props.content[start..end].to_string()
        })
    }
    
    /// Clear the current selection
    pub fn clear_selection(&mut self) {
        self.selection_start = None;
        self.selection_end = None;
    }
    
    /// Select all text
    pub fn select_all(&mut self) {
        self.selection_start = Some(CursorPos { line: 0, column: 0 });
        let last_line = self.line_starts.len().saturating_sub(1);
        let last_column = if last_line < self.line_starts.len() {
            let line_start = self.line_starts[last_line];
            self.text.props.content.len() - line_start
        } else {
            0
        };
        self.selection_end = Some(CursorPos { line: last_line, column: last_column });
        self.cursor_pos = CursorPos { line: last_line, column: last_column };
    }
    
    /// Start a new selection from the current cursor position
    pub fn start_selection(&mut self) {
        self.selection_start = Some(self.cursor_pos.clone());
        self.selection_end = Some(self.cursor_pos.clone());
    }
    
    /// Update selection end to current cursor position
    pub fn update_selection_to_cursor(&mut self) {
        if self.selection_start.is_some() {
            self.selection_end = Some(self.cursor_pos.clone());
        }
    }
    
    /// Delete the selected text
    pub fn delete_selection(&mut self) {
        if let Some((start, end)) = self.get_selection_range() {
            self.text.props.content.replace_range(start..end, "");
            // Recalculate line starts after deletion
            self.rebuild_line_starts();
            // Position cursor at start of deleted selection
            if let Some(start_pos) = &self.selection_start {
                self.cursor_pos = start_pos.clone();
            }
            self.clear_selection();
        }
    }
    
    /// Copy selected text to clipboard
    pub fn copy_to_clipboard(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(text) = self.get_selected_text() {
            let mut clipboard = arboard::Clipboard::new()?;
            clipboard.set_text(text)?;
        }
        Ok(())
    }
    
    /// Cut selected text to clipboard
    pub fn cut_to_clipboard(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(text) = self.get_selected_text() {
            let mut clipboard = arboard::Clipboard::new()?;
            clipboard.set_text(text)?;
            self.delete_selection();
        }
        Ok(())
    }
    
    /// Paste text from clipboard
    pub fn paste_from_clipboard(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut clipboard = arboard::Clipboard::new()?;
        if let Ok(text) = clipboard.get_text() {
            // Delete selection if any
            if self.has_selection() {
                self.delete_selection();
            }
            // Insert pasted text
            let index = self.cursor_pos_to_index(&self.cursor_pos);
            self.text.props.content.insert_str(index, &text);
            self.rebuild_line_starts();
            // Move cursor to end of pasted text
            // (simplified - just moves to end for now)
            self.cursor_pos.column += text.len();
        }
        Ok(())
    }
    
    /// Calculate cursor position from mouse click position (multiline)
    fn calculate_cursor_from_mouse(&mut self, mouse_pos: Vector2) -> CursorPos {
        let font_size = self.text.props.font_size;
        let line_height = font_size * 1.3;
        
        let panel_pos = self.panel.base.global_transform.position;
        let panel_left = panel_pos.x - (self.panel.base.size.x * self.panel.base.pivot.x);
        let panel_top = panel_pos.y - (self.panel.base.size.y * self.panel.base.pivot.y);
        
        let padding = 5.0;
        let relative_y = (mouse_pos.y - panel_top - padding).max(0.0);
        let relative_x = (mouse_pos.x - panel_left - padding).max(0.0);
        
        let line_index = ((relative_y / line_height).floor() as usize).min(self.line_count().saturating_sub(1));
        
        let line_text = self.get_line(line_index);
        use crate::nodes::ui::ui_renderer::calculate_character_positions;
        let char_positions = calculate_character_positions(line_text, font_size);
        
        let mut column = 0;
        if !char_positions.is_empty() && relative_x > 0.0 {
            for (i, &char_end_x) in char_positions.iter().enumerate() {
                let char_start_x = if i == 0 { 0.0 } else { char_positions[i - 1] };
                let char_mid_x = (char_start_x + char_end_x) / 2.0;
                
                if relative_x <= char_mid_x {
                    column = i;
                    break;
                } else if i == char_positions.len() - 1 {
                    column = i + 1;
                }
            }
        }
        
        CursorPos { line: line_index, column }
    }
}

impl UIElementUpdate for UITextEdit {
    fn internal_render_update(&mut self, ctx: &mut UIUpdateContext) -> bool {
        if !self.get_visible() {
            return false;
        }

        let mut needs_rerender = false;
        let was_hovered = self.is_hovered;
        let was_focused = self.is_focused;
        
        let size = *self.get_size();
        let scaled_size = Vector2::new(
            size.x * self.global_transform.scale.x,
            size.y * self.global_transform.scale.y,
        );
        
        let center = self.global_transform.position;
        let corner_radius = self.panel_props().corner_radius;
        let local_pos = Vector2::new(
            ctx.mouse_pos.x - center.x,
            ctx.mouse_pos.y - center.y,
        );
        
        let is_hovered = is_point_in_rounded_rect(
            local_pos,
            scaled_size,
            corner_radius,
        );
        
        self.is_hovered = is_hovered;
        
        // Handle dragging even when mouse is outside (for continuous selection)
        if self.is_dragging && was_focused && ctx.mouse_is_held {
            let cursor_pos = self.calculate_cursor_from_mouse(ctx.mouse_pos);
            self.cursor_pos = cursor_pos;
            self.update_selection_to_cursor();
            needs_rerender = true;
        }
        
        // Handle mouse clicks
        if is_hovered {
            if ctx.mouse_just_pressed && !was_focused {
                // Request focus
                if let Some(ref mut request_focus) = ctx.request_focus {
                    let _ = request_focus(self.get_id());
                }
                self.clear_selection();
                self.is_focused = true;
                needs_rerender = true;
            } else if ctx.mouse_just_pressed && was_focused {
                // New click - position cursor and prepare for potential drag
                self.is_dragging = true;
                let cursor_pos = self.calculate_cursor_from_mouse(ctx.mouse_pos);
                self.cursor_pos = cursor_pos;
                self.start_selection();
                needs_rerender = true;
            }
        }
        
        // Stop dragging when mouse is released
        if !ctx.mouse_is_held && self.is_dragging {
            self.is_dragging = false;
            // Clear selection if it's empty (just a click, not a drag)
            if let Some((start, end)) = self.get_selection_range() {
                if start == end {
                    self.clear_selection();
                }
            }
        }
        
        // Handle unfocus when clicking outside
        if ctx.mouse_just_pressed && was_focused && !is_hovered {
            self.is_focused = false;
            self.clear_selection();
            needs_rerender = true;
        }
        
        // Handle keyboard input if focused
        if ctx.is_focused {
            let mut text_changed = false;
            
            // Check for Ctrl/Cmd modifier
            let ctrl_pressed = ctx.is_key_pressed(KeyCode::ControlLeft) || 
                             ctx.is_key_pressed(KeyCode::ControlRight) ||
                             ctx.is_key_pressed(KeyCode::SuperLeft) ||
                             ctx.is_key_pressed(KeyCode::SuperRight);
            
            // Handle text input from IME (skip if Ctrl is pressed)
            let text_to_insert = ctx.get_text_input();
            if !text_to_insert.is_empty() && !ctrl_pressed {
                self.insert_text(&text_to_insert);
                text_changed = true;
                needs_rerender = true;
            }
            
            // Handle Ctrl+A (Select All)
            if ctrl_pressed && ctx.is_key_triggered(KeyCode::KeyA) {
                self.select_all();
                needs_rerender = true;
            }
            // Handle Ctrl+C (Copy)
            else if ctrl_pressed && ctx.is_key_triggered(KeyCode::KeyC) {
                let _ = self.copy_to_clipboard();
            }
            // Handle Ctrl+X (Cut)
            else if ctrl_pressed && ctx.is_key_triggered(KeyCode::KeyX) {
                if let Ok(()) = self.cut_to_clipboard() {
                    text_changed = true;
                    needs_rerender = true;
                }
            }
            // Handle Ctrl+V (Paste)
            else if ctrl_pressed && ctx.is_key_triggered(KeyCode::KeyV) {
                if let Ok(()) = self.paste_from_clipboard() {
                    text_changed = true;
                    needs_rerender = true;
                }
            }
            
            // Handle Enter key for newline
            if ctx.is_key_triggered(KeyCode::Enter) {
                self.insert_newline();
                text_changed = true;
                needs_rerender = true;
            }
            
            // Handle special keys
            if ctx.is_key_triggered(KeyCode::Backspace) {
                if self.has_selection() {
                    self.delete_selection();
                } else {
                    self.delete_backward();
                }
                text_changed = true;
                needs_rerender = true;
            }
            if ctx.is_key_triggered(KeyCode::Delete) {
                if self.has_selection() {
                    self.delete_selection();
                } else {
                    self.delete_forward();
                }
                text_changed = true;
                needs_rerender = true;
            }
            
            // Check if shift is pressed for selection extension
            let shift_pressed = ctx.is_key_pressed(KeyCode::ShiftLeft) || 
                              ctx.is_key_pressed(KeyCode::ShiftRight);
            
            if ctx.is_key_triggered(KeyCode::ArrowLeft) {
                if shift_pressed {
                    if !self.has_selection() {
                        self.start_selection();
                    }
                    self.move_cursor_left();
                    self.update_selection_to_cursor();
                } else {
                    self.clear_selection();
                    self.move_cursor_left();
                }
                needs_rerender = true;
            }
            if ctx.is_key_triggered(KeyCode::ArrowRight) {
                if shift_pressed {
                    if !self.has_selection() {
                        self.start_selection();
                    }
                    self.move_cursor_right();
                    self.update_selection_to_cursor();
                } else {
                    self.clear_selection();
                    self.move_cursor_right();
                }
                needs_rerender = true;
            }
            if ctx.is_key_triggered(KeyCode::ArrowUp) {
                if shift_pressed {
                    if !self.has_selection() {
                        self.start_selection();
                    }
                    self.move_cursor_up();
                    self.update_selection_to_cursor();
                } else {
                    self.clear_selection();
                    self.move_cursor_up();
                }
                needs_rerender = true;
            }
            if ctx.is_key_triggered(KeyCode::ArrowDown) {
                if shift_pressed {
                    if !self.has_selection() {
                        self.start_selection();
                    }
                    self.move_cursor_down();
                    self.update_selection_to_cursor();
                } else {
                    self.clear_selection();
                    self.move_cursor_down();
                }
                needs_rerender = true;
            }
            if ctx.is_key_triggered(KeyCode::Home) {
                if shift_pressed {
                    if !self.has_selection() {
                        self.start_selection();
                    }
                    self.move_cursor_line_start();
                    self.update_selection_to_cursor();
                } else {
                    self.clear_selection();
                    self.move_cursor_line_start();
                }
                needs_rerender = true;
            }
            if ctx.is_key_triggered(KeyCode::End) {
                if shift_pressed {
                    if !self.has_selection() {
                        self.start_selection();
                    }
                    self.move_cursor_line_end();
                    self.update_selection_to_cursor();
                } else {
                    self.clear_selection();
                    self.move_cursor_line_end();
                }
                needs_rerender = true;
            }
            
            // Update cursor blink
            let was_visible = self.is_cursor_visible();
            self.update_cursor_blink(0.016);
            let is_visible = self.is_cursor_visible();
            
            if was_visible != is_visible {
                needs_rerender = true;
            }

            // If text changed, mark for layout recalculation
            if text_changed {
                (ctx.mark_layout_dirty)(self.get_id());
            }
        }
        
        let state_changed = (is_hovered != was_hovered) || (self.is_focused != was_focused);
        if state_changed || needs_rerender {
            (ctx.mark_dirty)(self.get_id());
            (ctx.mark_ui_dirty)();
        }
        
        state_changed || needs_rerender
    }
}
