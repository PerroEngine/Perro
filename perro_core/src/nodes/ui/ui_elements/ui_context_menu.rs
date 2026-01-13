// ui_context_menu.rs - Context menu UI component (right-click menu)

use serde::{Deserialize, Serialize};

use crate::{
    structs2d::Vector2,
    ui_element::BaseUIElement,
    Color,
};

/// A single menu item in the context menu
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMenuItem {
    /// Unique identifier
    pub id: String,
    /// Display label
    pub label: String,
    /// Icon (optional)
    pub icon: Option<String>,
    /// Whether the item is enabled
    pub enabled: bool,
    /// Whether this is a separator
    pub is_separator: bool,
    /// Keyboard shortcut hint (e.g., "F2", "Ctrl+C")
    pub shortcut: Option<String>,
    /// Sub-menu items (for nested menus)
    pub submenu: Option<Vec<ContextMenuItem>>,
}

impl ContextMenuItem {
    /// Create a new menu item
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            enabled: true,
            is_separator: false,
            shortcut: None,
            submenu: None,
        }
    }

    /// Create a separator
    pub fn separator() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            icon: None,
            enabled: false,
            is_separator: true,
            shortcut: None,
            submenu: None,
        }
    }

    /// Set icon
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Set enabled state
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set shortcut hint
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Set submenu
    pub fn with_submenu(mut self, submenu: Vec<ContextMenuItem>) -> Self {
        self.submenu = Some(submenu);
        self
    }
}

/// Context menu UI element - displays a popup menu with action buttons
#[derive(Clone, Serialize, Deserialize)]
pub struct UIContextMenu {
    #[serde(flatten)]
    pub base: BaseUIElement,

    /// Menu items
    pub items: Vec<ContextMenuItem>,

    /// Position of the menu (top-left corner)
    pub position: Vector2,

    /// Whether the menu is visible
    pub visible: bool,

    /// Background color
    pub background_color: Color,

    /// Border color
    pub border_color: Color,

    /// Text color
    pub text_color: Color,

    /// Hover color
    pub hover_color: Color,

    /// Disabled text color
    pub disabled_color: Color,

    /// Item height in pixels
    pub item_height: f32,

    /// Padding in pixels
    pub padding: f32,

    /// Minimum width in pixels
    pub min_width: f32,

    /// Font size
    pub font_size: f32,

    /// Currently hovered item index
    #[serde(skip)]
    pub hovered_item: Option<usize>,

    /// Callback for when an item is clicked
    /// Note: Not cloneable, will be None after clone
    #[serde(skip)]
    pub on_item_clicked: Option<std::sync::Arc<dyn Fn(&str) + Send + Sync>>,
}

impl Default for UIContextMenu {
    fn default() -> Self {
        Self {
            base: BaseUIElement::default(),
            items: Vec::new(),
            position: Vector2::ZERO,
            visible: false,
            background_color: Color::new(40, 40, 40, 255),
            border_color: Color::new(80, 80, 80, 255),
            text_color: Color::new(255, 255, 255, 255),
            hover_color: Color::new(70, 130, 200, 255),
            disabled_color: Color::new(128, 128, 128, 255),
            item_height: 28.0,
            padding: 4.0,
            min_width: 150.0,
            font_size: 14.0,
            hovered_item: None,
            on_item_clicked: None,
        }
    }
}

impl UIContextMenu {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set menu items
    pub fn set_items(&mut self, items: Vec<ContextMenuItem>) {
        self.items = items;
    }

    /// Show the menu at a specific position
    pub fn show_at(&mut self, position: Vector2) {
        self.position = position;
        self.visible = true;
        self.hovered_item = None;
    }

    /// Hide the menu
    pub fn hide(&mut self) {
        self.visible = false;
        self.hovered_item = None;
    }

    /// Check if the menu contains a point (for click-away detection)
    pub fn contains_point(&self, point: Vector2) -> bool {
        if !self.visible {
            return false;
        }

        let width = self.calculate_width();
        let height = self.calculate_height();

        point.x >= self.position.x
            && point.x <= self.position.x + width
            && point.y >= self.position.y
            && point.y <= self.position.y + height
    }

    /// Calculate the total width of the menu
    pub fn calculate_width(&self) -> f32 {
        // In a real implementation, this would measure text widths
        // For now, use min_width
        self.min_width
    }

    /// Calculate the total height of the menu
    pub fn calculate_height(&self) -> f32 {
        let separator_height = 8.0;
        let mut total_height = self.padding * 2.0;

        for item in &self.items {
            if item.is_separator {
                total_height += separator_height;
            } else {
                total_height += self.item_height;
            }
        }

        total_height
    }

    /// Handle mouse move (for hover detection)
    pub fn handle_mouse_move(&mut self, mouse_pos: Vector2) {
        if !self.visible {
            self.hovered_item = None;
            return;
        }

        let _width = self.calculate_width();
        let separator_height = 8.0;

        // Check if mouse is within menu bounds
        if !self.contains_point(mouse_pos) {
            self.hovered_item = None;
            return;
        }

        // Calculate which item is hovered
        let mut y_offset = self.position.y + self.padding;
        for (i, item) in self.items.iter().enumerate() {
            if item.is_separator {
                y_offset += separator_height;
                continue;
            }

            let item_top = y_offset;
            let item_bottom = y_offset + self.item_height;

            if mouse_pos.y >= item_top && mouse_pos.y < item_bottom {
                if item.enabled {
                    self.hovered_item = Some(i);
                } else {
                    self.hovered_item = None;
                }
                return;
            }

            y_offset += self.item_height;
        }

        self.hovered_item = None;
    }

    /// Handle click on the menu
    pub fn handle_click(&mut self, mouse_pos: Vector2) -> Option<String> {
        if !self.visible || !self.contains_point(mouse_pos) {
            return None;
        }

        if let Some(idx) = self.hovered_item {
            if let Some(item) = self.items.get(idx) {
                if item.enabled && !item.is_separator {
                    let id = item.id.clone();
                    
                    // Call callback
                    if let Some(ref callback) = self.on_item_clicked {
                        callback(&id);
                    }
                    
                    // Hide menu after click
                    self.hide();
                    
                    return Some(id);
                }
            }
        }

        None
    }

    /// Create a standard file tree context menu
    pub fn create_file_tree_menu() -> Vec<ContextMenuItem> {
        vec![
            ContextMenuItem::new("open", "Open"),
            ContextMenuItem::new("rename", "Rename").with_shortcut("F2"),
            ContextMenuItem::separator(),
            ContextMenuItem::new("new_folder", "New Folder"),
            ContextMenuItem::new("new_file", "New File"),
            ContextMenuItem::separator(),
            ContextMenuItem::new("copy", "Copy").with_shortcut("Ctrl+C"),
            ContextMenuItem::new("paste", "Paste").with_shortcut("Ctrl+V"),
            ContextMenuItem::new("duplicate", "Duplicate"),
            ContextMenuItem::separator(),
            ContextMenuItem::new("delete", "Delete").with_shortcut("Del"),
            ContextMenuItem::separator(),
            ContextMenuItem::new("show_in_explorer", "Show in Explorer"),
            ContextMenuItem::new("copy_path", "Copy Path"),
        ]
    }
}

// Implement Debug manually to avoid issues with callback functions
impl std::fmt::Debug for UIContextMenu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UIContextMenu")
            .field("base", &self.base)
            .field("items", &self.items)
            .field("position", &self.position)
            .field("visible", &self.visible)
            .field("hovered_item", &self.hovered_item)
            .finish()
    }
}

// Implement BaseElement trait using the macro
crate::impl_ui_element!(UIContextMenu);
