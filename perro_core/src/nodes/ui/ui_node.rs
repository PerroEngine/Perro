use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    ops::{Deref, DerefMut},
};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use crate::uid32::UIElementID;


use crate::{
    Node,
    nodes::node_registry::NodeType,

    rendering::graphics::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH},

    scripting::api::ScriptApi,
    structs2d::Vector2,
    ui_element::{BaseElement, IntoUIInner, UIElement, UIElementUpdate, UIUpdateContext},

};

fn default_visible() -> bool {
    true
}
fn is_default_visible(v: &bool) -> bool {
    *v == default_visible()
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct UINode {
    #[serde(rename = "type")]
    pub ty: NodeType,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fur_path: Option<Cow<'static, str>>,

    #[serde(skip)]
    pub loaded_fur_path: Option<Cow<'static, str>>,

    #[serde(skip)]
    pub elements: Option<IndexMap<UIElementID, UIElement>>,
    #[serde(skip)]
    pub root_ids: Option<Vec<UIElementID>>,

    #[serde(
        default = "default_visible",
        skip_serializing_if = "is_default_visible"
    )]
    pub visible: bool,

    #[serde(skip)]
    pub needs_rerender: HashSet<UIElementID>,
    
    #[serde(skip)]
    pub needs_layout_recalc: HashSet<UIElementID>,
    
    /// Elements marked for deletion - will be removed from primitive renderer and then from elements map
    /// This set tracks element IDs (including all descendants) that should be deleted
    #[serde(skip)]
    pub pending_deletion: HashSet<UIElementID>,
    
    /// Store initial z-indices from FUR file to prevent accumulation across frames
    #[serde(skip)]
    pub initial_z_indices: HashMap<UIElementID, i32>,
    
    /// Currently focused UI element (for text input, etc.)
    #[serde(skip)]
    pub focused_element: Option<UIElementID>,

    /// Previous cursor icon state to avoid redundant updates
    #[serde(skip)]
    pub last_cursor_icon: Option<u8>, // Store as u8 to avoid importing CursorIcon here

    pub base: Node,
}

impl UINode {
    pub fn new() -> Self {
        let mut base = Node::new();
        base.name = Cow::Borrowed("UINode");
        Self {
            ty: NodeType::UINode,
            visible: default_visible(),
            // Base node
            base,
            fur_path: None,
            loaded_fur_path: None,
            elements: None,
            root_ids: None,
            needs_rerender: HashSet::new(),
            needs_layout_recalc: HashSet::new(),
            pending_deletion: HashSet::new(),
            initial_z_indices: HashMap::new(),
            focused_element: None,
            last_cursor_icon: None,
        }
    }
    
    /// Mark an element as needing rerender (visual only, no layout recalculation)
    pub fn mark_element_needs_rerender(&mut self, element_id: UIElementID) {
        self.needs_rerender.insert(element_id);
    }
    
    /// Mark an element as needing layout recalculation (triggers full layout update)
    pub fn mark_element_needs_layout(&mut self, element_id: UIElementID) {
        self.needs_rerender.insert(element_id);
        self.needs_layout_recalc.insert(element_id);
    }
    
    /// Mark all elements as needing rerender (for initial render or full refresh)
    pub fn mark_all_needs_rerender(&mut self) {
        if let Some(elements) = &self.elements {
            self.needs_rerender.extend(elements.keys().copied());
            self.needs_layout_recalc.extend(elements.keys().copied());
        }
    }
    
    /// Clear all rerender flags
    pub fn clear_rerender_flags(&mut self) {
        self.needs_rerender.clear();
        self.needs_layout_recalc.clear();
    }
    pub fn get_visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Find an element by name (ID) in the UI tree
    pub fn find_element_by_name(&self, name: &str) -> Option<&UIElement> {
        if let Some(elements) = &self.elements {
            elements.values().find(|el| el.get_name() == name)
        } else {
            None
        }
    }

    /// Find a mutable element by name (ID) in the UI tree
    pub fn find_element_by_name_mut(&mut self, name: &str) -> Option<&mut UIElement> {
        if let Some(elements) = &mut self.elements {
            elements.values_mut().find(|el| el.get_name() == name)
        } else {
            None
        }
    }

    /// Get an element by name (ID) - returns a reference
    /// This is useful for checking if an element exists or reading its properties
    pub fn get_element(&self, name: &str) -> Option<&UIElement> {
        self.find_element_by_name(name)
    }

    /// Get an element by name (ID) and clone it as a specific type
    /// Similar to `get_node_clone` for SceneNode
    ///
    /// This clones the element. After modifying it, use `set_element` to put it back.
    ///
    /// # Example
    /// ```ignore
    /// let mut text: UIText = ui_node.get_element_clone("bob").unwrap();
    /// text.set_content("Hello");
    /// ui_node.set_element("bob", UIElement::Text(text));
    /// ```
    pub fn get_element_clone<T: Clone>(&self, name: &str) -> Option<T>
    where
        UIElement: IntoUIInner<T>,
    {
        if let Some(element) = self.find_element_by_name(name) {
            // Clone the element and convert it
            let cloned = element.clone();
            Some(cloned.into_ui_inner())
        } else {
            None
        }
    }

    /// Set an element by name (ID), replacing the existing element
    /// Use this after cloning and modifying an element with `get_element_clone`
    ///
    /// # Example
    /// ```ignore
    /// let mut text: UIText = ui_node.get_element_clone("bob").unwrap();
    /// text.set_content("Hello");
    /// ui_node.set_element("bob", UIElement::Text(text));
    /// ```
    pub fn set_element(&mut self, name: &str, element: UIElement) -> bool {
        if let Some(elements) = &mut self.elements {
            // Find the element by name and get its UUID
            if let Some((uuid, _)) = elements.iter().find(|(_, el)| el.get_name() == name) {
                let uuid = *uuid;
                elements.insert(uuid, element);
                return true;
            }
        }
        false
    }

    /// Merge a collection of elements back into this UINode
    /// Similar to `merge_nodes` for SceneNode - updates elements by their name/ID
    ///
    /// # Arguments
    /// * `elements_to_merge` - A vector of (element_name, element) tuples
    ///
    /// This is called automatically by the transpiler when elements are cloned and modified
    pub fn merge_elements(&mut self, elements_to_merge: Vec<(String, UIElement)>) {
        if let Some(elements) = &mut self.elements {
            for (name, element) in elements_to_merge {
                // Find the element by name and get its UUID
                if let Some((uuid, _)) = elements.iter().find(|(_, el)| el.get_name() == name) {
                    let uuid = *uuid;
                    elements.insert(uuid, element);
                }
            }
        }
    }

    /// Get a Text element by name (ID) - returns a reference to UIText if the element is a Text element
    /// Returns None if the element doesn't exist or isn't a Text element
    pub fn get_text_element(&self, name: &str) -> Option<&crate::ui_elements::ui_text::UIText> {
        if let Some(element) = self.find_element_by_name(name) {
            if let UIElement::Text(text) = element {
                return Some(text);
            }
        }
        None
    }

    /// Get a mutable Text element by name (ID) - returns a mutable reference to UIText if the element is a Text element
    /// Returns None if the element doesn't exist or isn't a Text element
    pub fn get_text_element_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut crate::ui_elements::ui_text::UIText> {
        if let Some(element) = self.find_element_by_name_mut(name) {
            if let UIElement::Text(text) = element {
                return Some(text);
            }
        }
        None
    }

    /// Recursively collect all descendant IDs of an element
    /// This is useful for marking all children as needing rerender when parent visibility changes
    fn collect_all_descendants(&self, element_id: UIElementID) -> Vec<UIElementID> {
        let mut descendants = Vec::new();
        
        if let Some(elements) = &self.elements {
            let mut to_process = vec![element_id];
            
            while let Some(current_id) = to_process.pop() {
                if let Some(element) = elements.get(&current_id) {
                    for &child_id in element.get_children() {
                        descendants.push(child_id);
                        to_process.push(child_id);
                    }
                }
            }
        }
        
        descendants
    }

    /// Mark an element and all its descendants as needing rerender
    /// Use this when changing visibility to ensure all descendants are properly updated
    pub fn mark_element_with_descendants_needs_rerender(&mut self, element_id: UIElementID) {
        self.needs_rerender.insert(element_id);
        
        let descendants = self.collect_all_descendants(element_id);
        for descendant_id in descendants {
            self.needs_rerender.insert(descendant_id);
        }
    }

    /// Mark an element and all its descendants as needing layout recalculation
    /// Use this when changing visibility to ensure all descendants are properly updated
    pub fn mark_element_with_descendants_needs_layout(&mut self, element_id: UIElementID) {
        self.needs_rerender.insert(element_id);
        self.needs_layout_recalc.insert(element_id);
        
        let descendants = self.collect_all_descendants(element_id);
        for descendant_id in descendants {
            self.needs_rerender.insert(descendant_id);
            self.needs_layout_recalc.insert(descendant_id);
        }
    }
    
    /// Mark an element and all its descendants for deletion
    /// Elements marked for deletion will be removed from the primitive renderer cache
    /// and then removed from the elements map by the renderer
    pub fn mark_for_deletion(&mut self, element_id: UIElementID) {
        self.pending_deletion.insert(element_id);
        
        // Recursively mark all descendants for deletion
        let descendants = self.collect_all_descendants(element_id);
        for descendant_id in descendants {
            self.pending_deletion.insert(descendant_id);
        }
    }
}

impl Deref for UINode {
    type Target = Node;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UINode {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

// Helper function moved to ui_element.rs as is_point_in_rounded_rect

impl UINode {
    pub fn internal_render_update(&mut self, api: &mut ScriptApi<'_>) {
        if !self.visible {
            return;
        }
    
        let elements = match &mut self.elements {
            Some(e) => e,
            None => return,
        };
    
        // -----------------------------------------
        // Mouse → VIRTUAL UI SPACE
        // -----------------------------------------
        let screen_mouse = match api.scene.get_input_manager() {
            Some(mgr) => {
                let mgr = mgr.lock().unwrap();
                mgr.get_mouse_position()
            }
            None => return,
        };
    
        if screen_mouse.x == 0.0 && screen_mouse.y == 0.0 {
            // No interactive elements to reset hover state for
            return;
        }
    
        let (window_w, window_h) = api
            .gfx
            .as_ref()
            .map(|g| {
                (
                    g.surface_config.width as f32,
                    g.surface_config.height as f32,
                )
            })
            .unwrap_or((1920.0, 1080.0));
    
        let mouse_pos = Vector2::new(
            (screen_mouse.x / window_w - 0.5) * VIRTUAL_WIDTH,
            (0.5 - screen_mouse.y / window_h) * VIRTUAL_HEIGHT,
        );
    
        // -----------------------------------------
        // Mouse button
        // -----------------------------------------
        let mouse_pressed = api
            .scene
            .get_input_manager()
            .map(|mgr| {
                let mgr = mgr.lock().unwrap();
                use crate::input::manager::MouseButton;
                mgr.is_mouse_button_pressed(MouseButton::Left)
            })
            .unwrap_or(false);
    
        // -----------------------------------------
        // Hit testing (FINAL)
        // -----------------------------------------
        use std::rc::Rc;
        use std::cell::RefCell;
        
        let dirty_element_ids = Rc::new(RefCell::new(Vec::new()));
        let layout_dirty_element_ids = Rc::new(RefCell::new(Vec::new()));
        let any_button_hovered = Rc::new(RefCell::new(false));
        let any_text_input_hovered = Rc::new(RefCell::new(false));
        let clicked_text_input_id = Rc::new(RefCell::new(None));
        let new_focused_element = Rc::new(RefCell::new(None));
        let needs_ui_rerender = Rc::new(RefCell::new(false));
        
        // Store focused_element in local variable to avoid borrow issues
        let current_focused_element = self.focused_element;
        
        // Check for mouse click outside to unfocus
        let mouse_just_pressed = api
            .scene
            .get_input_manager()
            .map(|mgr| {
                let mgr = mgr.lock().unwrap();
                use crate::input::manager::MouseButton;
                mgr.is_mouse_button_pressed(MouseButton::Left)
            })
            .unwrap_or(false);
        
        // Check if mouse button is currently held down
        let mouse_is_held = api
            .scene
            .get_input_manager()
            .map(|mgr| {
                let mgr = mgr.lock().unwrap();
                use crate::input::manager::MouseButton;
                mgr.state().mouse_buttons_pressed.contains(&MouseButton::Left)
            })
            .unwrap_or(false);
        
        // Update all elements using the trait system - UINode is now just a window into the scene
        // Process all elements in a single loop to avoid multiple mutable borrows of api
        // 
        // SAFETY: We use unsafe to create multiple mutable borrows of `api` in the loop.
        // This is safe because:
        // 1. Only one match arm executes per iteration (match is exclusive)
        // 2. Each UIUpdateContext is dropped at the end of its match arm, before the next iteration
        // 3. The closures in UIUpdateContext are 'static (they only capture owned Rc<RefCell<>> values, not api)
        // 
        // Rust's borrow checker can't prove this is safe because it analyzes all match arms
        // together, but we know at runtime only one executes and contexts don't overlap.
        // No interactive elements to update
        
        // Extract values from Rc<RefCell<>>
        let mut dirty_element_ids_vec = dirty_element_ids.borrow_mut();
        let mut layout_dirty_element_ids_vec = layout_dirty_element_ids.borrow_mut();
        let any_button_hovered_val = *any_button_hovered.borrow();
        let any_text_input_hovered_val = *any_text_input_hovered.borrow();
        let clicked_text_input_id_val = *clicked_text_input_id.borrow();
        let new_focused_element_val = *new_focused_element.borrow();
        let needs_ui_rerender_val = *needs_ui_rerender.borrow();
        
        // Move values out
        let mut dirty_element_ids = std::mem::take(&mut *dirty_element_ids_vec);
        let layout_dirty_element_ids = std::mem::take(&mut *layout_dirty_element_ids_vec);
        let any_button_hovered = any_button_hovered_val;
        let any_text_input_hovered = any_text_input_hovered_val;
        let clicked_text_input_id = clicked_text_input_id_val;
        let new_focused_element = new_focused_element_val;
        let needs_ui_rerender = needs_ui_rerender_val;
        
        // Apply focus changes
        if let Some(new_focused) = new_focused_element.or(clicked_text_input_id) {
            if self.focused_element != Some(new_focused) {
                // Unfocus previously focused element
                // No focusable elements to unfocus
                self.focused_element = Some(new_focused);
            }
        }
        
        // Mark layout dirty elements
        for element_id in layout_dirty_element_ids {
            self.mark_element_needs_layout(element_id);
        }
        
        // Mark UI as needing rerender if any element requested it
        if needs_ui_rerender {
            api.scene.mark_needs_rerender(self.base.id);
        }
        
        // Get command sender after marking rerender to avoid borrow conflicts
        let command_sender = api.scene.get_command_sender();
        
        // Update cursor icon based on hover state (only if changed)
        if let Some(tx) = command_sender {
            use crate::scripting::app_command::{AppCommand, CursorIcon};
            let cursor_icon = if any_button_hovered {
                CursorIcon::Hand
            } else if any_text_input_hovered {
                CursorIcon::Text
            } else {
                CursorIcon::Default
            };
            
            // Convert to u8 for comparison (avoid importing CursorIcon type in struct)
            let icon_value = match cursor_icon {
                CursorIcon::Default => 0,
                CursorIcon::Hand => 1,
                CursorIcon::Text => 2,
                CursorIcon::NotAllowed => 3,
                CursorIcon::Wait => 4,
                CursorIcon::Crosshair => 5,
                CursorIcon::Move => 6,
                CursorIcon::ResizeVertical => 7,
                CursorIcon::ResizeHorizontal => 8,
                CursorIcon::ResizeDiagonal1 => 9,
                CursorIcon::ResizeDiagonal2 => 10,
            };
            
            // Only send command if cursor icon changed
            if self.last_cursor_icon != Some(icon_value) {
                self.last_cursor_icon = Some(icon_value);
                let _ = tx.send(AppCommand::SetCursorIcon(cursor_icon));
            }
        }
        
        // Keyboard input is now handled by each element's internal_render_update method
        // No need for separate keyboard handling here
        
        // Mark all dirty elements after the loop (avoid borrow conflict)
        for element_id in dirty_element_ids {
            self.mark_element_needs_rerender(element_id);
        }
    }
}

impl crate::nodes::node_registry::NodeWithInternalRenderUpdate for UINode {
    fn internal_render_update(&mut self, api: &mut crate::scripting::api::ScriptApi) {
        self.internal_render_update(api);
    }
}