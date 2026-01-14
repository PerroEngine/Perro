// ui_list_tree_manager.rs - Manager for list tree operations with UID registry integration

use uuid::Uuid;
use winit::keyboard::KeyCode;

use crate::{
    project::asset_io::{resolve_path, ResolvedPath, get_project_root, ProjectRoot},
    ui_elements::ui_list_tree::{UIListTree, ListTreeItem},
    ui_elements::ui_context_menu::UIContextMenu,
    structs2d::Vector2,
    input::manager::InputManager,
};

/// List tree manager - handles high-level operations for list trees
pub struct ListTreeManager {
    /// Reference to the list tree
    pub tree_id: String,
    /// Reference to the context menu
    pub context_menu_id: String,
}

impl ListTreeManager {
    pub fn new(tree_id: impl Into<String>, context_menu_id: impl Into<String>) -> Self {
        Self {
            tree_id: tree_id.into(),
            context_menu_id: context_menu_id.into(),
        }
    }

    /// Handle input events for the list tree
    /// Returns true if the event was handled
    pub fn handle_input(
        tree: &mut UIListTree,
        context_menu: &mut UIContextMenu,
        input: &InputManager,
        _mouse_pos: Vector2,
    ) -> bool {
        // Handle F2 key for rename
        if input.is_key_pressed(KeyCode::F2) {
            if let Some(&selected_id) = tree.selected_items.iter().next() {
                tree.start_rename(selected_id);
                return true;
            }
        }

        // Handle Escape key to cancel rename
        if input.is_key_pressed(KeyCode::Escape) {
            if tree.rename_state.is_some() {
                tree.cancel_rename();
                return true;
            }
            // Also hide context menu
            if context_menu.visible {
                context_menu.hide();
                return true;
            }
        }

        // Handle Enter key to commit rename
        // NOTE: Rename signal emission should be handled by caller (ui_node.rs) after this returns
        if input.is_key_pressed(KeyCode::Enter) {
            if tree.rename_state.is_some() {
                // Just commit the rename in the tree (updates item path)
                // The caller should check if rename was successful and emit signal
                if let Err(e) = tree.commit_rename() {
                    eprintln!("Failed to commit rename: {}", e);
                }
                return true;
            }
        }

        // Handle Delete key
        if input.is_key_pressed(KeyCode::Delete) {
            if let Some(&selected_id) = tree.selected_items.iter().next() {
                Self::delete_item(tree, selected_id);
                return true;
            }
        }

        false
    }

    /// Commit rename and update file system + UID registry
    /// Returns (old_path, new_path) so the caller can update scene nodes
    pub fn commit_rename_with_fs(tree: &mut UIListTree) -> Result<(String, String), String> {
        // Get the rename state
        let state = tree.rename_state.as_ref()
            .ok_or_else(|| "No rename in progress".to_string())?;
        
        let item_id = state.item_id;
        let new_name = state.text.clone();
        
        // Get the item
        let item = tree.get_item(item_id)
            .ok_or_else(|| "Item not found".to_string())?;
        
        let old_path = item.path.clone();
        let uid = item.uid;

        // Validate new name
        if new_name.is_empty() {
            return Err("Name cannot be empty".to_string());
        }
        
        if new_name.contains('/') || new_name.contains('\\') {
            return Err("Name cannot contain path separators".to_string());
        }

        // Calculate new path
        let parent_path = if let Some(idx) = old_path.rfind('/') {
            &old_path[..=idx]
        } else {
            ""
        };
        let new_path = format!("{}{}", parent_path, new_name);

        // Rename the file on disk
        // NOTE: UID registry updates should be handled by the caller (manager script)
        let old_fs_path = match resolve_path(&old_path) {
            ResolvedPath::Disk(p) => p,
            ResolvedPath::Brk(_) => {
                return Err("Cannot rename assets in BRK archives".to_string());
            }
        };

        let new_fs_path = match resolve_path(&new_path) {
            ResolvedPath::Disk(p) => p,
            ResolvedPath::Brk(_) => {
                return Err("Cannot rename assets in BRK archives".to_string());
            }
        };

        std::fs::rename(&old_fs_path, &new_fs_path)
            .map_err(|e| format!("Failed to rename file: {}", e))?;

        // Update the tree
        tree.commit_rename()?;

        // Return old and new paths so caller can update scene nodes
        Ok((old_path, new_path))
    }

    /// Delete an item from the tree and file system
    pub fn delete_item(tree: &mut UIListTree, item_id: Uuid) -> Result<(), String> {
        let item = tree.get_item(item_id)
            .ok_or_else(|| "Item not found".to_string())?;
        
        let path = item.path.clone();
        let is_directory = item.is_directory;

        // Resolve path to file system
        let fs_path = match resolve_path(&path) {
            ResolvedPath::Disk(p) => p,
            ResolvedPath::Brk(_) => {
                return Err("Cannot delete assets in BRK archives".to_string());
            }
        };

        // Delete from file system
        if is_directory {
            std::fs::remove_dir_all(&fs_path)
                .map_err(|e| format!("Failed to delete directory: {}", e))?;
        } else {
            std::fs::remove_file(&fs_path)
                .map_err(|e| format!("Failed to delete file: {}", e))?;
        }

        // Remove from tree
        tree.remove_item(item_id);

        // TODO: Remove from UID registry if it has a UID

        Ok(())
    }

    /// Create a new file in the tree
    pub fn create_file(tree: &mut UIListTree, parent_id: Option<Uuid>, name: String) -> Result<Uuid, String> {
        // Calculate path
        let parent_path = if let Some(pid) = parent_id {
            tree.get_item(pid)
                .map(|item| item.path.clone())
                .unwrap_or_else(|| tree.root_path.clone())
        } else {
            tree.root_path.clone()
        };

        let file_path = format!("{}/{}", parent_path.trim_end_matches('/'), name);

        // Resolve to file system path
        let fs_path = match resolve_path(&file_path) {
            ResolvedPath::Disk(p) => p,
            ResolvedPath::Brk(_) => {
                return Err("Cannot create files in BRK archives".to_string());
            }
        };

        // Create the file
        std::fs::File::create(&fs_path)
            .map_err(|e| format!("Failed to create file: {}", e))?;

        // Add to tree (UID will be set by caller if needed)
        let item = ListTreeItem::new_file(name, file_path, None);
        let item_id = item.id;
        tree.add_item(item, parent_id);

        Ok(item_id)
    }

    /// Create a new directory in the tree
    pub fn create_directory(tree: &mut UIListTree, parent_id: Option<Uuid>, name: String) -> Result<Uuid, String> {
        // Calculate path
        let parent_path = if let Some(pid) = parent_id {
            tree.get_item(pid)
                .map(|item| item.path.clone())
                .unwrap_or_else(|| tree.root_path.clone())
        } else {
            tree.root_path.clone()
        };

        let dir_path = format!("{}/{}", parent_path.trim_end_matches('/'), name);

        // Resolve to file system path
        let fs_path = match resolve_path(&dir_path) {
            ResolvedPath::Disk(p) => p,
            ResolvedPath::Brk(_) => {
                return Err("Cannot create directories in BRK archives".to_string());
            }
        };

        // Create the directory
        std::fs::create_dir_all(&fs_path)
            .map_err(|e| format!("Failed to create directory: {}", e))?;

        // Add to tree
        let item = ListTreeItem::new_directory(name, dir_path);
        let item_id = item.id;
        tree.add_item(item, parent_id);

        Ok(item_id)
    }

    /// Handle context menu action
    pub fn handle_context_action(
        tree: &mut UIListTree,
        _context_menu: &mut UIContextMenu,
        action: &str,
    ) -> Result<(), String> {
        let selected_id = tree.selected_items.iter().next()
            .copied()
            .ok_or_else(|| "No item selected".to_string())?;

        match action {
            "open" => {
                // Double-click behavior - expand/activate
                if let Some(item) = tree.get_item(selected_id) {
                    if item.is_directory {
                        tree.toggle_expanded(selected_id);
                    } else {
                        // Trigger activation callback
                        if let Some(ref callback) = tree.on_item_activated {
                            callback(selected_id, &item.path);
                        }
                    }
                }
            }
            "rename" => {
                tree.start_rename(selected_id);
            }
            "delete" => {
                Self::delete_item(tree, selected_id)?;
            }
            "new_folder" => {
                // Create new folder in selected directory or its parent
                let parent_id = if let Some(item) = tree.get_item(selected_id) {
                    if item.is_directory {
                        Some(selected_id)
                    } else {
                        item.parent
                    }
                } else {
                    None
                };
                
                Self::create_directory(tree, parent_id, "New Folder".to_string())?;
            }
            "new_file" => {
                // Create new file in selected directory or its parent
                let parent_id = if let Some(item) = tree.get_item(selected_id) {
                    if item.is_directory {
                        Some(selected_id)
                    } else {
                        item.parent
                    }
                } else {
                    None
                };
                
                Self::create_file(tree, parent_id, "new_file.txt".to_string())?;
            }
            "show_in_explorer" => {
                // Open the file/folder in system file explorer
                if let Some(item) = tree.get_item(selected_id) {
                    Self::show_in_explorer(&item.path)?;
                }
            }
            "copy_path" => {
                // Copy path to clipboard
                if let Some(item) = tree.get_item(selected_id) {
                    use arboard::Clipboard;
                    if let Ok(mut clipboard) = Clipboard::new() {
                        let _ = clipboard.set_text(&item.path);
                    }
                }
            }
            _ => {
                eprintln!("Unknown context action: {}", action);
            }
        }

        Ok(())
    }

    /// Show a file/folder in the system file explorer
    fn show_in_explorer(path: &str) -> Result<(), String> {
        let fs_path = match resolve_path(path) {
            ResolvedPath::Disk(p) => p,
            ResolvedPath::Brk(_) => {
                return Err("Cannot open BRK archive files in explorer".to_string());
            }
        };

        #[cfg(target_os = "windows")]
        {
            use std::process::Command;
            Command::new("explorer")
                .arg("/select,")
                .arg(&fs_path)
                .spawn()
                .map_err(|e| format!("Failed to open explorer: {}", e))?;
        }

        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            Command::new("open")
                .arg("-R")
                .arg(&fs_path)
                .spawn()
                .map_err(|e| format!("Failed to open finder: {}", e))?;
        }

        #[cfg(target_os = "linux")]
        {
            use std::process::Command;
            // Try xdg-open with the parent directory
            if let Some(parent) = fs_path.parent() {
                Command::new("xdg-open")
                    .arg(parent)
                    .spawn()
                    .map_err(|e| format!("Failed to open file manager: {}", e))?;
            }
        }

        Ok(())
    }

    /// Refresh the tree from the file system, preserving UIDs
    pub fn refresh_tree(tree: &mut UIListTree) -> Result<(), String> {
        let project_root = match get_project_root() {
            ProjectRoot::Disk { root, .. } => root,
            ProjectRoot::Brk { .. } => {
                return Err("Cannot refresh tree for BRK projects".to_string());
            }
        };

        let res_dir = project_root.join("res");
        if !res_dir.exists() {
            return Ok(());
        }

        // Load the tree and assign UIDs
        tree.load_from_directory(&res_dir)
            .map_err(|e| format!("Failed to load directory: {}", e))?;

        // UIDs should be assigned by the caller (manager script) if needed
        // This function just refreshes the tree structure

        Ok(())
    }
}
