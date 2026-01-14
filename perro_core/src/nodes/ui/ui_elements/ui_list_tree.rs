// ui_list_tree.rs - List tree/explorer UI component with context menu and rename support

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, Duration};

use winit::keyboard::KeyCode;

use crate::{
    structs2d::Vector2,
    ui_element::{BaseElement, BaseUIElement, UIElementUpdate, UIUpdateContext},
    Color,
};

/// Represents a single item in the tree (generic - can represent files, nodes, game objects, etc.)
/// 
/// The tree item stores display information (name, icon) and hierarchy (parent/children).
/// Use `user_data` to store custom data that maps to your game objects (e.g., node IDs, entity IDs, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListTreeItem {
    /// Unique identifier for this tree item
    pub id: Uuid,
    /// Display name
    pub name: String,
    /// Full path (e.g., "res://textures/player.png") - optional for non-file items
    pub path: String,
    /// Asset UID (if registered) - optional for non-file items
    /// Editor-specific: used for tracking file renames
    pub uid: Option<Uuid>,
    /// Whether this is a directory/folder (can have children)
    pub is_directory: bool,
    /// Whether this item is expanded (for directories)
    pub is_expanded: bool,
    /// Children items (for directories)
    pub children: Vec<Uuid>,
    /// Parent item ID
    pub parent: Option<Uuid>,
    /// Depth level in the tree (0 = root)
    pub depth: usize,
    /// Custom icon (optional)
    pub icon: Option<String>,
    /// Custom user data - store anything here to map this item to your game data
    /// (e.g., node ID, entity ID, custom struct as JSON, etc.)
    #[serde(skip)]
    pub user_data: Option<serde_json::Value>,
}

impl ListTreeItem {
    /// Create a new file item (for file trees)
    pub fn new_file(name: String, path: String, uid: Option<Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            path,
            uid,
            is_directory: false,
            is_expanded: false,
            children: Vec::new(),
            parent: None,
            depth: 0,
            icon: None,
            user_data: None,
        }
    }

    /// Create a new directory/folder item (for file trees)
    pub fn new_directory(name: String, path: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            path,
            uid: None,
            is_directory: true,
            is_expanded: false,
            children: Vec::new(),
            parent: None,
            depth: 0,
            icon: None,
            user_data: None,
        }
    }

    /// Create a new generic tree item (for scene graphs, game objects, etc.)
    /// 
    /// Use this when you want to display non-file data in a tree.
    /// Set `user_data` to store a reference to your actual game object (e.g., node ID as JSON).
    /// 
    /// Example:
    /// ```rust
    /// let node_id = some_node.get_id();
    /// let item = ListTreeItem::new_item(
    ///     "Player".to_string(),
    ///     false, // not a folder
    ///     Some(serde_json::json!({ "node_id": node_id.to_string() }))
    /// );
    /// ```
    pub fn new_item(name: String, is_directory: bool, user_data: Option<serde_json::Value>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            path: String::new(), // Empty for non-file items
            uid: None,
            is_directory,
            is_expanded: false,
            children: Vec::new(),
            parent: None,
            depth: 0,
            icon: None,
            user_data,
        }
    }

    /// Set custom user data (e.g., node ID, entity ID, etc.)
    pub fn set_user_data(&mut self, data: serde_json::Value) {
        self.user_data = Some(data);
    }

    /// Get custom user data
    pub fn get_user_data(&self) -> Option<&serde_json::Value> {
        self.user_data.as_ref()
    }

    /// Get user data as a specific type (e.g., extract node ID)
    /// 
    /// Example:
    /// ```rust
    /// if let Some(node_id_str) = item.get_user_data_string("node_id") {
    ///     // Use node_id_str to find your node
    /// }
    /// ```
    pub fn get_user_data_string(&self, key: &str) -> Option<String> {
        self.user_data.as_ref()
            .and_then(|data| data.get(key))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
}

/// Context menu state
#[derive(Debug, Clone, PartialEq)]
pub enum ContextMenuState {
    Hidden,
    Visible {
        item_id: Uuid,
        position: Vector2,
        /// Time when the menu was opened (for click-away detection)
        opened_at: SystemTime,
    },
}

impl Default for ContextMenuState {
    fn default() -> Self {
        ContextMenuState::Hidden
    }
}

/// Selection mode for the List tree
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionMode {
    /// Single item selection
    Single,
    /// Multiple item selection
    Multiple,
}

/// Rename state
#[derive(Debug, Clone)]
pub struct RenameState {
    /// Item being renamed
    pub item_id: Uuid,
    /// Current text in the rename field
    pub text: String,
    /// Whether the rename is in progress
    pub active: bool,
}

/// Tree UI element - displays hierarchical data (files, scene nodes, game objects, etc.)
/// 
/// This is a generic tree component that can display any hierarchical data.
/// - For file trees: use `load_from_directory()` or `new_file()`/`new_directory()`
/// - For scene graphs/game objects: use `new_item()` with `user_data` to map to your objects
/// - Items are stored in a HashMap keyed by UUID, with parent/child relationships
#[derive(Clone, Serialize, Deserialize)]
pub struct UIListTree {
    #[serde(flatten)]
    pub base: BaseUIElement,

    /// Root directory path (e.g., "res://")
    pub root_path: String,

    /// All items in the tree (keyed by ID)
    #[serde(skip)]
    pub items: HashMap<Uuid, ListTreeItem>,

    /// Root item IDs (top-level items)
    #[serde(skip)]
    pub root_items: Vec<Uuid>,

    /// Currently selected item IDs
    #[serde(skip)]
    pub selected_items: HashSet<Uuid>,

    /// Selection mode
    pub selection_mode: SelectionMode,

    /// Context menu state
    #[serde(skip)]
    pub context_menu: ContextMenuState,

    /// Rename state
    #[serde(skip)]
    pub rename_state: Option<RenameState>,

    /// Whether to show file extensions
    pub show_extensions: bool,

    /// Whether to show hidden files (starting with .)
    pub show_hidden: bool,

    /// Item height in pixels
    pub item_height: f32,

    /// Indent per depth level in pixels
    pub indent_size: f32,

    /// Background color
    pub background_color: Color,

    /// Selected item color
    pub selected_color: Color,

    /// Hover color
    pub hover_color: Color,

    /// Text color
    pub text_color: Color,

    /// Folder icon (optional)
    pub folder_icon: Option<String>,

    /// File icon (optional)
    pub file_icon: Option<String>,

    /// Last click time and item (for double-click detection)
    #[serde(skip)]
    pub last_click: Option<(Uuid, SystemTime)>,
    
    /// Currently hovered item (for hover highlighting)
    #[serde(skip)]
    pub hovered_item: Option<Uuid>,

    /// Double-click threshold in milliseconds
    pub double_click_threshold_ms: u64,

    /// Callback for when an item is activated (double-clicked)
    /// Note: Not cloneable, will be None after clone
    #[serde(skip)]
    pub on_item_activated: Option<std::sync::Arc<dyn Fn(Uuid, &str) + Send + Sync>>,

    /// Callback for when an item is renamed
    /// Note: Not cloneable, will be None after clone
    #[serde(skip)]
    pub on_item_renamed: Option<std::sync::Arc<dyn Fn(Uuid, &str, &str) + Send + Sync>>,

    /// Callback for when a context menu action is triggered
    /// Note: Not cloneable, will be None after clone
    #[serde(skip)]
    pub on_context_action: Option<std::sync::Arc<dyn Fn(Uuid, &str) + Send + Sync>>,
}

impl Default for UIListTree {
    fn default() -> Self {
        Self {
            base: BaseUIElement::default(),
            root_path: "res://".to_string(),
            items: HashMap::new(),
            root_items: Vec::new(),
            selected_items: HashSet::new(),
            selection_mode: SelectionMode::Single,
            context_menu: ContextMenuState::Hidden,
            rename_state: None,
            show_extensions: true,
            show_hidden: false,
            item_height: 24.0,
            indent_size: 20.0,
            background_color: Color::new(30, 30, 30, 255),
            selected_color: Color::new(60, 120, 200, 255),
            hover_color: Color::new(50, 50, 50, 255),
            text_color: Color::new(255, 255, 255, 255),
            folder_icon: None,
            file_icon: None,
            last_click: None,
            hovered_item: None,
            double_click_threshold_ms: 300,
            on_item_activated: None,
            on_item_renamed: None,
            on_context_action: None,
        }
    }
}

impl UIListTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the root path and refresh the tree
    pub fn set_root_path(&mut self, path: String) {
        self.root_path = path;
        // In a real implementation, this would scan the file system
    }

    /// Add an item to the tree
    pub fn add_item(&mut self, mut item: ListTreeItem, parent_id: Option<Uuid>) {
        item.parent = parent_id;
        
        // Calculate depth
        if let Some(parent_id) = parent_id {
            if let Some(parent) = self.items.get(&parent_id) {
                item.depth = parent.depth + 1;
            }
        }

        let item_id = item.id;
        self.items.insert(item_id, item);

        // Add to parent's children or root items
        if let Some(parent_id) = parent_id {
            if let Some(parent) = self.items.get_mut(&parent_id) {
                if !parent.children.contains(&item_id) {
                    parent.children.push(item_id);
                }
            }
        } else {
            if !self.root_items.contains(&item_id) {
                self.root_items.push(item_id);
            }
        }
    }

    /// Remove an item from the tree
    pub fn remove_item(&mut self, item_id: Uuid) -> Option<ListTreeItem> {
        let item = self.items.remove(&item_id)?;

        // Remove from parent's children or root items
        if let Some(parent_id) = item.parent {
            if let Some(parent) = self.items.get_mut(&parent_id) {
                parent.children.retain(|&id| id != item_id);
            }
        } else {
            self.root_items.retain(|&id| id != item_id);
        }

        // Remove from selection
        self.selected_items.remove(&item_id);

        // Recursively remove children
        let children = item.children.clone();
        for child_id in children {
            self.remove_item(child_id);
        }

        Some(item)
    }

    /// Get an item by ID
    pub fn get_item(&self, item_id: Uuid) -> Option<&ListTreeItem> {
        self.items.get(&item_id)
    }

    /// Get a mutable item by ID
    pub fn get_item_mut(&mut self, item_id: Uuid) -> Option<&mut ListTreeItem> {
        self.items.get_mut(&item_id)
    }

    /// Find an item by path
    pub fn find_item_by_path(&self, path: &str) -> Option<&ListTreeItem> {
        self.items.values().find(|item| item.path == path)
    }

    /// Find an item by user data key-value pair
    /// 
    /// Useful for finding items that map to your game objects.
    /// 
    /// Example:
    /// ```rust
    /// // Find item that has node_id in user_data
    /// if let Some(item) = tree.find_item_by_user_data("node_id", &node_id.to_string()) {
    ///     // Found the item for this node
    /// }
    /// ```
    pub fn find_item_by_user_data(&self, key: &str, value: &str) -> Option<&ListTreeItem> {
        self.items.values().find(|item| {
            item.user_data.as_ref()
                .and_then(|data| data.get(key))
                .and_then(|v| v.as_str())
                .map(|s| s == value)
                .unwrap_or(false)
        })
    }

    /// Get all items that match a user data key-value pair
    pub fn find_items_by_user_data(&self, key: &str, value: &str) -> Vec<&ListTreeItem> {
        self.items.values()
            .filter(|item| {
                item.user_data.as_ref()
                    .and_then(|data| data.get(key))
                    .and_then(|v| v.as_str())
                    .map(|s| s == value)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Select an item
    pub fn select_item(&mut self, item_id: Uuid, multi: bool) {
        if !multi {
            self.selected_items.clear();
        }
        self.selected_items.insert(item_id);
    }

    /// Deselect an item
    pub fn deselect_item(&mut self, item_id: Uuid) {
        self.selected_items.remove(&item_id);
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        self.selected_items.clear();
    }

    /// Check if an item is selected
    pub fn is_selected(&self, item_id: Uuid) -> bool {
        self.selected_items.contains(&item_id)
    }

    /// Toggle item expanded state
    pub fn toggle_expanded(&mut self, item_id: Uuid) {
        if let Some(item) = self.items.get_mut(&item_id) {
            if item.is_directory {
                item.is_expanded = !item.is_expanded;
            }
        }
    }

    /// Expand an item
    pub fn expand_item(&mut self, item_id: Uuid) {
        if let Some(item) = self.items.get_mut(&item_id) {
            if item.is_directory {
                item.is_expanded = true;
            }
        }
    }

    /// Collapse an item
    pub fn collapse_item(&mut self, item_id: Uuid) {
        if let Some(item) = self.items.get_mut(&item_id) {
            if item.is_directory {
                item.is_expanded = false;
            }
        }
    }

    /// Handle single click on an item
    pub fn handle_click(&mut self, item_id: Uuid, multi: bool) {
        let now = SystemTime::now();
        
        // Check for double-click
        let is_double_click = if let Some((last_id, last_time)) = self.last_click {
            if last_id == item_id {
                if let Ok(elapsed) = now.duration_since(last_time) {
                    elapsed < Duration::from_millis(self.double_click_threshold_ms)
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        if is_double_click {
            // Double-click - activate item
            if let Some(item) = self.items.get(&item_id) {
                if item.is_directory {
                    self.toggle_expanded(item_id);
                } else {
                    // Call activation callback
                    if let Some(ref callback) = self.on_item_activated {
                        callback(item_id, &item.path);
                    }
                }
            }
            self.last_click = None;
        } else {
            // Single-click - select item
            self.select_item(item_id, multi);
            self.last_click = Some((item_id, now));
        }
    }

    /// Handle right-click on an item (show context menu)
    pub fn handle_right_click(&mut self, item_id: Uuid, position: Vector2) {
        // Select the item if not already selected
        if !self.is_selected(item_id) {
            self.select_item(item_id, false);
        }

        // Show context menu
        self.context_menu = ContextMenuState::Visible {
            item_id,
            position,
            opened_at: SystemTime::now(),
        };
    }

    /// Hide the context menu
    pub fn hide_context_menu(&mut self) {
        self.context_menu = ContextMenuState::Hidden;
    }

    /// Start renaming an item
    pub fn start_rename(&mut self, item_id: Uuid) {
        if let Some(item) = self.items.get(&item_id) {
            self.rename_state = Some(RenameState {
                item_id,
                text: item.name.clone(),
                active: true,
            });
        }
    }

    /// Update rename text
    pub fn update_rename_text(&mut self, text: String) {
        if let Some(ref mut state) = self.rename_state {
            state.text = text;
        }
    }

    /// Commit rename (update the item and call callback)
    pub fn commit_rename(&mut self) -> Result<(), String> {
        if let Some(state) = self.rename_state.take() {
            if let Some(item) = self.items.get_mut(&state.item_id) {
                let old_path = item.path.clone();
                let _old_name = item.name.clone();
                
                // Validate new name
                if state.text.is_empty() {
                    return Err("Name cannot be empty".to_string());
                }
                
                if state.text.contains('/') || state.text.contains('\\') {
                    return Err("Name cannot contain path separators".to_string());
                }

                // Update name and path
                let parent_path = if let Some(idx) = old_path.rfind('/') {
                    &old_path[..=idx]
                } else {
                    ""
                };
                
                item.name = state.text.clone();
                item.path = format!("{}{}", parent_path, state.text);

                // Call rename callback
                if let Some(ref callback) = self.on_item_renamed {
                    callback(state.item_id, &old_path, &item.path);
                }

                return Ok(());
            }
        }
        Err("No rename in progress".to_string())
    }

    /// Cancel rename
    pub fn cancel_rename(&mut self) {
        self.rename_state = None;
    }

    /// Get all visible items in display order (depth-first traversal)
    pub fn get_visible_items(&self) -> Vec<Uuid> {
        let mut result = Vec::new();
        
        fn traverse(
            tree: &UIListTree,
            items: &[Uuid],
            result: &mut Vec<Uuid>,
        ) {
            for &item_id in items {
                result.push(item_id);
                
                if let Some(item) = tree.items.get(&item_id) {
                    if item.is_directory && item.is_expanded {
                        traverse(tree, &item.children, result);
                    }
                }
            }
        }
        
        traverse(self, &self.root_items, &mut result);
        result
    }

    /// Refresh the tree from the file system
    pub fn refresh_from_disk(&mut self) {
        // This would scan the root_path and rebuild the tree
        // For now, this is a placeholder
        self.items.clear();
        self.root_items.clear();
        self.selected_items.clear();
    }

    /// Load tree structure from a directory path
    pub fn load_from_directory(&mut self, directory: &Path) -> std::io::Result<()> {
        use walkdir::WalkDir;
        
        self.items.clear();
        self.root_items.clear();
        
        let mut path_to_id: HashMap<PathBuf, Uuid> = HashMap::new();
        
        for entry in WalkDir::new(directory)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            
            // Skip the root directory itself
            if path == directory {
                continue;
            }
            
            // Skip hidden files if configured
            if !self.show_hidden {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with('.') {
                        continue;
                    }
                }
            }
            
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
                .to_string();
            
            let rel_path = path.strip_prefix(directory).unwrap_or(path);
            let full_path = format!("{}{}", self.root_path, rel_path.to_string_lossy().replace('\\', "/"));
            
            let is_dir = path.is_dir();
            let item = if is_dir {
                ListTreeItem::new_directory(name, full_path)
            } else {
                ListTreeItem::new_file(name, full_path, None)
            };
            
            // Find parent
            let parent_id = path.parent()
                .and_then(|p| {
                    if p == directory {
                        None
                    } else {
                        path_to_id.get(p).copied()
                    }
                });
            
            let item_id = item.id;
            path_to_id.insert(path.to_path_buf(), item_id);
            
            self.add_item(item, parent_id);
        }
        
        Ok(())
    }
}

// Implement Debug manually to avoid issues with callback functions
impl std::fmt::Debug for UIListTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UIListTree")
            .field("base", &self.base)
            .field("root_path", &self.root_path)
            .field("items", &self.items)
            .field("root_items", &self.root_items)
            .field("selected_items", &self.selected_items)
            .field("selection_mode", &self.selection_mode)
            .field("context_menu", &self.context_menu)
            .field("rename_state", &self.rename_state)
            .finish()
    }
}

// Implement BaseElement trait using the macro
crate::impl_ui_element!(UIListTree);

impl UIElementUpdate for UIListTree {
    fn internal_render_update(&mut self, ctx: &mut UIUpdateContext) -> bool {
        if !self.get_visible() {
            return false;
        }

        let mut needs_rerender = false;
        let tree_id = self.get_id();
        
        // Calculate bounds
        let top_left_x = self.base.global_transform.position.x - (self.base.size.x * self.base.pivot.x);
        let top_left_y = self.base.global_transform.position.y - (self.base.size.y * self.base.pivot.y);
        
        let in_bounds = ctx.mouse_pos.x >= top_left_x && ctx.mouse_pos.x <= (top_left_x + self.base.size.x) &&
                       ctx.mouse_pos.y >= top_left_y && ctx.mouse_pos.y <= (top_left_y + self.base.size.y);
        
        // Track hover state (always, even if not clicking)
        if in_bounds {
            // Find hovered item
            let item_start_y = top_left_y + 5.0;
            let mut current_y = item_start_y;
            let mut hovered_item_id: Option<Uuid> = None;
            
            for item_id in self.get_visible_items() {
                let item_bottom = current_y + self.item_height;
                if ctx.mouse_pos.y >= current_y && ctx.mouse_pos.y < item_bottom {
                    hovered_item_id = Some(item_id);
                    break;
                }
                current_y = item_bottom;
            }
            
            // Update hover state if it changed
            if self.hovered_item != hovered_item_id {
                self.hovered_item = hovered_item_id;
                needs_rerender = true;
            }
        } else {
            // Mouse is outside bounds, clear hover
            if self.hovered_item.is_some() {
                self.hovered_item = None;
                needs_rerender = true;
            }
        }
        
        // Find clicked item (reuse the hover detection logic)
        let clicked_item_id = self.hovered_item;
        
        if let Some(item_id) = clicked_item_id {
            // Left click
            if ctx.mouse_just_pressed {
                // Check for double-click
                let now = std::time::SystemTime::now();
                let is_double_click = if let Some((last_id, last_time)) = self.last_click {
                    last_id == item_id && now.duration_since(last_time).map(|d| d.as_millis() < self.double_click_threshold_ms as u128).unwrap_or(false)
                } else {
                    false
                };
                
                if is_double_click {
                    // Double-click: toggle folder and emit signal WITH item_id parameter
                    let is_folder = self.items.get(&item_id).map(|i| i.is_directory).unwrap_or(false);
                    
                    if is_folder {
                        self.toggle_expanded(item_id);
                    }
                    
                    // Emit double-click signal with item_id as parameter
                    let signal_name = format!("{}_DoubleClicked", self.base.name);
                    let params = [serde_json::Value::String(item_id.to_string())];
                    eprintln!("🔔 [ListTree] Emitting signal: '{}' (ListTree name: '{}') with params: {:?}", signal_name, self.base.name, params);
                    ctx.api.emit_signal(&signal_name, &params);
                    
                    self.last_click = None;
                    needs_rerender = true;
                } else {
                    // Single-click: select and emit signal WITH item_id parameter
                    self.select_item(item_id, false);
                    self.last_click = Some((item_id, now));
                    
                    // Emit single-click signal with item_id as parameter
                    let signal_name = format!("{}_Clicked", self.base.name);
                    let params = [serde_json::Value::String(item_id.to_string())];
                    eprintln!("🔔 [ListTree] Emitting signal: '{}' (ListTree name: '{}') with params: {:?}", signal_name, self.base.name, params);
                    ctx.api.emit_signal(&signal_name, &params);
                    
                    needs_rerender = true;
                }
            }
            
            // Right click
            if ctx.mouse_is_held && !ctx.mouse_just_pressed {
                // Check if right mouse button is pressed
                let right_pressed = ctx.is_right_mouse_pressed();
                
                if right_pressed {
                    self.select_item(item_id, false);
                    
                    // Emit right-click signal with item_id as parameter
                    let signal_name = format!("{}_RightClicked", self.base.name);
                    let params = [serde_json::Value::String(item_id.to_string())];
                    eprintln!("🔔 [ListTree] Emitting signal: '{}' (ListTree name: '{}') with params: {:?}", signal_name, self.base.name, params);
                    ctx.api.emit_signal(&signal_name, &params);
                    
                    needs_rerender = true;
                }
            }
        }
        
        // Handle keyboard input for rename
        if self.rename_state.is_some() {
            let should_commit = ctx.is_key_triggered(KeyCode::Enter);
            let should_cancel = ctx.is_key_triggered(KeyCode::Escape);
            
            // Handle Enter key to commit rename
            if should_commit {
                // Get rename state before commit (commit_rename consumes it)
                if let Some(state) = self.rename_state.as_ref() {
                    let item_id = state.item_id;
                    
                    // Get old path before commit
                    let old_path = self.items.get(&item_id)
                        .map(|item| item.path.clone());
                    
                    // Calculate new path from rename state
                    let parent_path = old_path.as_ref()
                        .and_then(|p| p.rfind('/').map(|idx| &p[..=idx]))
                        .unwrap_or("");
                    let new_path = format!("{}{}", parent_path, state.text);
                    
                    // Commit the rename (updates item path, consumes rename_state)
                    if let Ok(()) = self.commit_rename() {
                        // Emit rename signal with old_path and new_path
                        if let Some(old_path) = old_path {
                            let signal_name = format!("{}_Renamed", self.base.name);
                            let params = [
                                serde_json::Value::String(old_path.clone()),
                                serde_json::Value::String(new_path.clone()),
                            ];
                            eprintln!("🔔 [ListTree] Emitting rename signal: '{}' old_path='{}' new_path='{}'", signal_name, old_path, new_path);
                            ctx.api.emit_signal(&signal_name, &params);
                            
                            needs_rerender = true;
                        }
                    }
                }
            }
            
            // Handle Escape key to cancel rename
            if should_cancel {
                self.cancel_rename();
                needs_rerender = true;
            }
        }
        
        if needs_rerender {
            (ctx.mark_dirty)(tree_id);
            (ctx.mark_ui_dirty)();
        }
        
        needs_rerender
    }
}
