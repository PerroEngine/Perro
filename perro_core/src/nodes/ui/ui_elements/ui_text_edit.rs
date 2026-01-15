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

/// A text edit field that can be focused and edited
/// Similar to UIButton but handles keyboard input and displays a cursor
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UITextEdit {
    pub base: BaseUIElement,
    
    // Composed elements - the text edit IS a panel with text
    pub panel: UIPanel,
    pub text: UIText,
    
    // Text anchor - controls where text is positioned within the input
    // Defaults to Center if not specified
    #[serde(default)]
    pub text_anchor: FurAnchor,
    
    // Padding - controls spacing inside the text edit
    #[serde(default = "default_padding")]
    pub padding: crate::ui_elements::ui_container::Padding,
    
    // Optional hover and focused background colors
    // If None, will use lightened/darkened version of base bg color
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover_bg: Option<crate::structs::Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_bg: Option<crate::structs::Color>,
    
    // Optional custom selection highlight color
    // If None, will use lightened version of background color
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight_color: Option<crate::structs::Color>,
    
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
    
    // Vertical scroll offset in pixels (how many pixels to shift text up)
    #[serde(skip)]
    pub scroll_offset_y: f32,
    
    // Text selection state
    #[serde(skip)]
    pub selection_start: Option<usize>,  // None = no selection
    #[serde(skip)]
    pub selection_end: Option<usize>,
    #[serde(skip)]
    pub is_dragging: bool,  // True while mouse is being dragged for selection
    
    // Cached text measurements for performance
    #[serde(skip)]
    pub cached_text_width: f32,
    #[serde(skip)]
    pub cached_text_content: String,
    #[serde(skip)]
    pub cached_char_positions: Vec<f32>, // Cumulative width at each character position
    
    // Multi-line support: line information
    #[serde(skip)]
    pub line_breaks: Vec<usize>, // Character positions where line breaks occur (newline positions)
    #[serde(skip)]
    pub line_heights: Vec<f32>, // Height of each line
    #[serde(skip)]
    pub line_start_positions: Vec<usize>, // Character index where each line starts
    
    // Backspace repeat state (for speed-up when held)
    #[serde(skip)]
    pub backspace_repeat_timer: f32, // Timer for backspace repeat
    #[serde(skip)]
    pub backspace_last_delete_time: f32, // Time when we last performed a delete (for repeat)
    #[serde(skip)]
    pub delete_repeat_timer: f32, // Timer for delete key repeat
    #[serde(skip)]
    pub delete_last_delete_time: f32, // Time when we last performed a delete (for repeat)
}

fn default_padding() -> crate::ui_elements::ui_container::Padding {
    crate::ui_elements::ui_container::Padding::uniform(5.0)
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
            text_anchor: FurAnchor::TopLeft, // Default text anchor to top-left
            padding: default_padding(),
            hover_bg: None,
            focused_bg: None,
            highlight_color: None,
            is_hovered: false,
            is_focused: false,
            cursor_position: 0,
            cursor_blink_timer: 0.0,
            scroll_offset: 0.0,
            scroll_offset_y: 0.0,
            selection_start: None,
            selection_end: None,
            is_dragging: false,
            cached_text_width: 0.0,
            cached_text_content: String::new(),
            cached_char_positions: Vec::new(),
            line_breaks: Vec::new(),
            line_heights: Vec::new(),
            line_start_positions: vec![0], // First line starts at position 0
            backspace_repeat_timer: 0.0,
            backspace_last_delete_time: 0.0,
            delete_repeat_timer: 0.0,
            delete_last_delete_time: 0.0,
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
    /// This should be called before rendering or layout calculations
    pub fn sync_base_to_children(&mut self) {
        // Sync all base properties from text edit to panel and text
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
        
        // Panel uses the text edit's anchor (visual container)
        self.panel.base.anchor = self.base.anchor;
        // Text uses the text edit's text_anchor (defaults to center)
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
        self.update_line_info();
    }
    
    /// Insert text at the current cursor position
    pub fn insert_text(&mut self, text: &str) {
        let content = &mut self.text.props.content;
        let pos = self.cursor_position.min(content.len());
        content.insert_str(pos, text);
        self.cursor_position += text.len();
        // Invalidate cache when text changes
        self.invalidate_cache();
        self.update_line_info();
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
            self.update_line_info();
        }
    }
    
    /// Delete the character at the cursor (delete key)
    pub fn delete_forward(&mut self) {
        let content = &mut self.text.props.content;
        if self.cursor_position < content.len() {
            content.remove(self.cursor_position);
            // Invalidate cache when text changes
            self.invalidate_cache();
            self.update_line_info();
        }
    }
    
    /// Invalidate cached measurements when text content changes
    fn invalidate_cache(&mut self) {
        self.cached_text_content.clear();
        self.cached_text_width = 0.0;
        self.cached_char_positions.clear();
        self.line_breaks.clear();
        self.line_heights.clear();
        self.line_start_positions.clear();
        self.line_start_positions.push(0); // First line always starts at 0
    }
    
    /// Update line information from text content
    pub fn update_line_info(&mut self) {
        let content = &self.text.props.content;
        let font_size = self.text.props.font_size;
        
        // Calculate line height from font metrics
        use crate::font::{Font, Style, Weight};
        use fontdue::Font as Fontdue;
        use fontdue::FontSettings;
        
        const DESIGN_SIZE: f32 = 192.0;
        let line_height = if let Some(font) = Font::from_name("NotoSans", Weight::Regular, Style::Normal) {
            if let Ok(fd_font) = Fontdue::from_bytes(font.data, FontSettings::default()) {
                if let Some(metrics) = fd_font.horizontal_line_metrics(DESIGN_SIZE) {
                    let scale = font_size / DESIGN_SIZE;
                    (metrics.ascent + metrics.descent) * scale * 1.2 // Add 20% line spacing
                } else {
                    font_size * 1.2
                }
            } else {
                font_size * 1.2
            }
        } else {
            font_size * 1.2
        };
        
        // Find all line breaks
        self.line_breaks.clear();
        self.line_start_positions.clear();
        self.line_start_positions.push(0);
        
        for (i, ch) in content.char_indices() {
            if ch == '\n' {
                self.line_breaks.push(i);
                // Next line starts after the newline
                self.line_start_positions.push(i + 1);
            }
        }
        
        // Calculate line heights (all lines have the same height for now)
        let num_lines = self.line_start_positions.len();
        self.line_heights = vec![line_height; num_lines];
    }
    
    /// Get line number and column from cursor position
    pub fn get_line_and_column(&self, cursor_pos: usize) -> (usize, usize) {
        // Find which line this cursor position is on
        let mut line = 0;
        for (line_idx, &line_start) in self.line_start_positions.iter().enumerate() {
            if cursor_pos >= line_start {
                line = line_idx;
            } else {
                break;
            }
        }
        
        // Column is the offset from the start of the line
        let line_start = self.line_start_positions.get(line).copied().unwrap_or(0);
        let column = cursor_pos - line_start;
        
        (line, column)
    }
    
    /// Get character position from line and column
    fn get_position_from_line_column(&self, line: usize, column: usize) -> usize {
        let line_start = self.line_start_positions.get(line).copied().unwrap_or(0);
        let content = &self.text.props.content;
        
        // Find the end of this line (either next line start or end of text)
        let line_end = if line + 1 < self.line_start_positions.len() {
            self.line_start_positions[line + 1] - 1 // -1 to exclude the newline character
        } else {
            content.len()
        };
        
        // Clamp column to valid range
        let max_column = line_end - line_start;
        let clamped_column = column.min(max_column);
        
        line_start + clamped_column
    }
    
    /// Get the text content of a specific line
    pub fn get_line_text(&self, line: usize) -> &str {
        let content = &self.text.props.content;
        let line_start = self.line_start_positions.get(line).copied().unwrap_or(0);
        
        let line_end = if line + 1 < self.line_start_positions.len() {
            self.line_start_positions[line + 1] - 1 // -1 to exclude the newline
        } else {
            content.len()
        };
        
        if line_start < content.len() && line_start < line_end {
            &content[line_start..line_end.min(content.len())]
        } else {
            ""
        }
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
        // Blink every 2 seconds (1 second visible, 1 second hidden)
        if self.cursor_blink_timer >= 2.0 {
            self.cursor_blink_timer = 0.0;
        }
    }

    /// Check if cursor should be visible (for blinking effect)
    pub fn is_cursor_visible(&self) -> bool {
        self.cursor_blink_timer < 1.0
    }
    
    // ===== Selection Methods =====
    
    /// Check if there is an active selection
    pub fn has_selection(&self) -> bool {
        self.selection_start.is_some() && self.selection_end.is_some()
    }
    
    /// Get the selection range (start, end) in sorted order (start <= end)
    pub fn get_selection_range(&self) -> Option<(usize, usize)> {
        match (self.selection_start, self.selection_end) {
            (Some(start), Some(end)) => {
                let min = start.min(end);
                let max = start.max(end);
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
        self.selection_start = Some(0);
        self.selection_end = Some(self.text.props.content.len());
        self.cursor_position = self.text.props.content.len();
    }
    
    /// Start a new selection from the current cursor position
    pub fn start_selection(&mut self) {
        self.selection_start = Some(self.cursor_position);
        self.selection_end = Some(self.cursor_position);
    }
    
    /// Update selection end to current cursor position
    pub fn update_selection_to_cursor(&mut self) {
        if self.selection_start.is_some() {
            self.selection_end = Some(self.cursor_position);
        }
    }
    
    /// Delete the selected text
    pub fn delete_selection(&mut self) {
        if let Some((start, end)) = self.get_selection_range() {
            self.text.props.content.replace_range(start..end, "");
            self.cursor_position = start;
            self.clear_selection();
            self.invalidate_cache();
            self.update_line_info();
        }
    }
    
    /// Insert text at cursor, replacing selection if any
    pub fn insert_text_at_cursor(&mut self, text: &str) {
        // If there's a selection, delete it first
        if self.has_selection() {
            self.delete_selection();
        }
        
        // Insert the new text
        let content = &mut self.text.props.content;
        let pos = self.cursor_position.min(content.len());
        content.insert_str(pos, text);
        self.cursor_position += text.len();
        self.invalidate_cache();
        self.update_line_info();
    }
    
    /// Move cursor up one line
    pub fn move_cursor_up(&mut self) {
        let (line, column) = self.get_line_and_column(self.cursor_position);
        if line > 0 {
            // Move to the same column on the previous line
            let new_line = line - 1;
            self.cursor_position = self.get_position_from_line_column(new_line, column);
        } else {
            // Already at first line, move to start
            self.cursor_position = 0;
        }
    }
    
    /// Move cursor down one line
    pub fn move_cursor_down(&mut self) {
        let (line, column) = self.get_line_and_column(self.cursor_position);
        if line + 1 < self.line_start_positions.len() {
            // Move to the same column on the next line
            let new_line = line + 1;
            self.cursor_position = self.get_position_from_line_column(new_line, column);
        } else {
            // Already at last line, move to end
            self.cursor_position = self.text.props.content.len();
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
            self.insert_text_at_cursor(&text);
        }
        Ok(())
    }
    
    /// Calculate cursor position from mouse click position
    fn calculate_cursor_from_mouse(&mut self, mouse_pos: Vector2) -> usize {
        // Update line info if needed
        if self.line_start_positions.is_empty() {
            self.update_line_info();
        }
        
        let font_size = self.text.props.font_size;
        
        // Calculate line height
        let line_height = self.line_heights.first().copied().unwrap_or(font_size * 1.2);
        
        // Get panel position
        let panel_top = self.panel.base.global_transform.position.y 
            + (self.panel.base.size.y * (1.0 - self.panel.base.pivot.y));
        let panel_left = self.panel.base.global_transform.position.x 
            - (self.panel.base.size.x * self.panel.base.pivot.x);
        
        // Calculate which line was clicked based on Y position
        // Account for vertical scroll
        let click_y_relative = panel_top - mouse_pos.y + self.scroll_offset_y;
        let line_index = (click_y_relative / line_height).floor() as usize;
        let clicked_line = line_index.min(self.line_start_positions.len().saturating_sub(1));
        
        // Get the text of the clicked line
        let line_text = self.get_line_text(clicked_line);
        
        // Calculate character positions for this line
        use crate::nodes::ui::ui_renderer::calculate_character_positions;
        let line_positions = calculate_character_positions(line_text, font_size);
        
        // Calculate X position within the line
        // Get text transform position (already positioned by layout system)
        let text_transform = self.text.base.global_transform;
        // Apply padding offset: padding moves content INSIDE the bounds
        let text_base_x = text_transform.position.x + self.padding.left;
        
        let click_x_in_text = if line_positions.is_empty() {
            0.0
        } else {
            // Account for horizontal scroll
            (mouse_pos.x - text_base_x) + self.scroll_offset
        };
        
        // Find column within the line
        let mut column = 0;
        if !line_positions.is_empty() && click_x_in_text > 0.0 {
            for (i, &char_end_x) in line_positions.iter().enumerate() {
                let char_start_x = if i == 0 { 0.0 } else { line_positions[i - 1] };
                let char_mid_x = (char_start_x + char_end_x) / 2.0;
                
                if click_x_in_text <= char_mid_x {
                    column = i;
                    break;
                } else if i == line_positions.len() - 1 {
                    column = i + 1;
                } else if click_x_in_text <= line_positions[i] {
                    column = i + 1;
                    break;
                }
            }
        }
        
        // Convert line and column to absolute cursor position
        self.get_position_from_line_column(clicked_line, column)
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
            // Mouse is being dragged - update selection
            let cursor_pos = self.calculate_cursor_from_mouse(ctx.mouse_pos);
            self.cursor_position = cursor_pos;
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
                // Position cursor at click location when focusing
                let cursor_pos = self.calculate_cursor_from_mouse(ctx.mouse_pos);
                self.cursor_position = cursor_pos;
                // Clear selection when focusing
                self.clear_selection();
                self.is_focused = true;
                self.cursor_blink_timer = 0.0;
                needs_rerender = true;
            } else if ctx.mouse_just_pressed && was_focused {
                // New click - position cursor and prepare for potential drag
                self.is_dragging = true;
                let cursor_pos = self.calculate_cursor_from_mouse(ctx.mouse_pos);
                self.cursor_position = cursor_pos;
                self.start_selection();
                self.cursor_blink_timer = 0.0;
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
            // Clear focus in UINode (will be handled by UINode after all updates)
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
                // Normalize line endings: convert \r\n to \n, and preserve standalone \n
                let normalized = text_to_insert.replace("\r\n", "\n").replace('\r', "\n");
                // Insert the text (which may contain newlines for multi-line input)
                self.insert_text_at_cursor(&normalized);
                self.cursor_blink_timer = 0.0;
                text_changed = true;
                needs_rerender = true;
            }
            
            // Handle Ctrl+A (Select All)
            if ctrl_pressed && ctx.is_key_triggered(KeyCode::KeyA) {
                self.select_all();
                self.cursor_blink_timer = 0.0;
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
                    self.cursor_blink_timer = 0.0;
                    needs_rerender = true;
                }
            }
            // Handle Ctrl+V (Paste)
            else if ctrl_pressed && ctx.is_key_triggered(KeyCode::KeyV) {
                if let Ok(()) = self.paste_from_clipboard() {
                    text_changed = true;
                    self.cursor_blink_timer = 0.0;
                    needs_rerender = true;
                }
            }
            
            // Handle backspace with repeat when held down
            const BACKSPACE_INITIAL_DELAY: f32 = 0.5; // 500ms before repeat starts
            const BACKSPACE_REPEAT_INTERVAL: f32 = 0.05; // 50ms between repeats (20 chars/sec)
            const DELTA_TIME: f32 = 0.016; // ~60fps
            
            let backspace_pressed = ctx.is_key_pressed(KeyCode::Backspace);
            let backspace_triggered = ctx.is_key_triggered(KeyCode::Backspace);
            
            if backspace_triggered {
                // Initial press - delete immediately and start timer
                if self.has_selection() {
                    self.delete_selection();
                } else {
                    self.delete_backward();
                }
                self.cursor_blink_timer = 0.0;
                self.backspace_repeat_timer = 0.0;
                self.backspace_last_delete_time = 0.0;
                text_changed = true;
                needs_rerender = true;
            } else if backspace_pressed {
                // Key is held - check if we should repeat
                self.backspace_repeat_timer += DELTA_TIME;
                
                // Check if initial delay has passed
                if self.backspace_repeat_timer >= BACKSPACE_INITIAL_DELAY {
                    // Check if enough time has passed since last delete
                    let time_since_last_delete = self.backspace_repeat_timer - self.backspace_last_delete_time;
                    
                    if self.backspace_last_delete_time == 0.0 {
                        // First repeat after initial delay
                        if self.has_selection() {
                            self.delete_selection();
                        } else {
                            self.delete_backward();
                        }
                        self.cursor_blink_timer = 0.0;
                        self.backspace_last_delete_time = self.backspace_repeat_timer;
                        text_changed = true;
                        needs_rerender = true;
                    } else if time_since_last_delete >= BACKSPACE_REPEAT_INTERVAL {
                        // Repeat interval passed - delete again
                        if self.has_selection() {
                            self.delete_selection();
                        } else {
                            self.delete_backward();
                        }
                        self.cursor_blink_timer = 0.0;
                        self.backspace_last_delete_time = self.backspace_repeat_timer;
                        text_changed = true;
                        needs_rerender = true;
                    }
                }
            } else {
                // Key released - reset timers
                self.backspace_repeat_timer = 0.0;
                self.backspace_last_delete_time = 0.0;
            }
            
            // Handle delete key with repeat when held down
            let delete_pressed = ctx.is_key_pressed(KeyCode::Delete);
            let delete_triggered = ctx.is_key_triggered(KeyCode::Delete);
            
            if delete_triggered {
                // Initial press - delete immediately and start timer
                if self.has_selection() {
                    self.delete_selection();
                } else {
                    self.delete_forward();
                }
                self.cursor_blink_timer = 0.0;
                self.delete_repeat_timer = 0.0;
                self.delete_last_delete_time = 0.0;
                text_changed = true;
                needs_rerender = true;
            } else if delete_pressed {
                // Key is held - check if we should repeat
                self.delete_repeat_timer += DELTA_TIME;
                
                // Check if initial delay has passed
                if self.delete_repeat_timer >= BACKSPACE_INITIAL_DELAY {
                    // Check if enough time has passed since last delete
                    let time_since_last_delete = self.delete_repeat_timer - self.delete_last_delete_time;
                    
                    if self.delete_last_delete_time == 0.0 {
                        // First repeat after initial delay
                        if self.has_selection() {
                            self.delete_selection();
                        } else {
                            self.delete_forward();
                        }
                        self.cursor_blink_timer = 0.0;
                        self.delete_last_delete_time = self.delete_repeat_timer;
                        text_changed = true;
                        needs_rerender = true;
                    } else if time_since_last_delete >= BACKSPACE_REPEAT_INTERVAL {
                        // Repeat interval passed - delete again
                        if self.has_selection() {
                            self.delete_selection();
                        } else {
                            self.delete_forward();
                        }
                        self.cursor_blink_timer = 0.0;
                        self.delete_last_delete_time = self.delete_repeat_timer;
                        text_changed = true;
                        needs_rerender = true;
                    }
                }
            } else {
                // Key released - reset timers
                self.delete_repeat_timer = 0.0;
                self.delete_last_delete_time = 0.0;
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
                self.cursor_blink_timer = 0.0;
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
                self.cursor_blink_timer = 0.0;
                needs_rerender = true;
            }
            if ctx.is_key_triggered(KeyCode::Home) {
                if shift_pressed {
                    if !self.has_selection() {
                        self.start_selection();
                    }
                    self.move_cursor_home();
                    self.update_selection_to_cursor();
                } else {
                    self.clear_selection();
                    self.move_cursor_home();
                }
                self.cursor_blink_timer = 0.0;
                needs_rerender = true;
            }
            if ctx.is_key_triggered(KeyCode::End) {
                if shift_pressed {
                    if !self.has_selection() {
                        self.start_selection();
                    }
                    self.move_cursor_end();
                    self.update_selection_to_cursor();
                } else {
                    self.clear_selection();
                    self.move_cursor_end();
                }
                self.cursor_blink_timer = 0.0;
                needs_rerender = true;
            }
            
            // Handle ArrowUp (move cursor up one line)
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
                self.cursor_blink_timer = 0.0;
                needs_rerender = true;
            }
            
            // Handle ArrowDown (move cursor down one line)
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
                self.cursor_blink_timer = 0.0;
                needs_rerender = true;
            }
            
            // Handle Enter key (insert newline)
            if ctx.is_key_triggered(KeyCode::Enter) {
                if self.has_selection() {
                    self.delete_selection();
                }
                self.insert_text_at_cursor("\n");
                self.cursor_blink_timer = 0.0;
                text_changed = true;
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
