use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    ops::{Deref, DerefMut},
};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    Node,
    nodes::node_registry::NodeType,
    prelude::string_to_u64,
    rendering::graphics::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH},
    script::Var,
    scripting::api::ScriptApi,
    structs2d::Vector2,
    ui_element::{BaseElement, IntoUIInner, UIElement},
    ui_elements::ui_container::CornerRadius,
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
    pub props: Option<HashMap<String, Var>>,

    #[serde(skip)]
    pub elements: Option<IndexMap<Uuid, UIElement>>,
    #[serde(skip)]
    pub root_ids: Option<Vec<Uuid>>,

    #[serde(
        default = "default_visible",
        skip_serializing_if = "is_default_visible"
    )]
    pub visible: bool,

    #[serde(skip)]
    pub needs_rerender: HashSet<Uuid>,
    
    #[serde(skip)]
    pub needs_layout_recalc: HashSet<Uuid>,
    
    /// Store initial z-indices from FUR file to prevent accumulation across frames
    #[serde(skip)]
    pub initial_z_indices: HashMap<Uuid, i32>,
    
    /// Currently focused UI element (for text input, etc.)
    #[serde(skip)]
    pub focused_element: Option<Uuid>,

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
            props: None,
            elements: None,
            root_ids: None,
            needs_rerender: HashSet::new(),
            needs_layout_recalc: HashSet::new(),
            initial_z_indices: HashMap::new(),
            focused_element: None,
        }
    }
    
    /// Mark an element as needing rerender (visual only, no layout recalculation)
    pub fn mark_element_needs_rerender(&mut self, element_id: Uuid) {
        self.needs_rerender.insert(element_id);
    }
    
    /// Mark an element as needing layout recalculation (triggers full layout update)
    pub fn mark_element_needs_layout(&mut self, element_id: Uuid) {
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
    fn collect_all_descendants(&self, element_id: Uuid) -> Vec<Uuid> {
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
    pub fn mark_element_with_descendants_needs_rerender(&mut self, element_id: Uuid) {
        self.needs_rerender.insert(element_id);
        
        let descendants = self.collect_all_descendants(element_id);
        for descendant_id in descendants {
            self.needs_rerender.insert(descendant_id);
        }
    }

    /// Mark an element and all its descendants as needing layout recalculation
    /// Use this when changing visibility to ensure all descendants are properly updated
    pub fn mark_element_with_descendants_needs_layout(&mut self, element_id: Uuid) {
        self.needs_rerender.insert(element_id);
        self.needs_layout_recalc.insert(element_id);
        
        let descendants = self.collect_all_descendants(element_id);
        for descendant_id in descendants {
            self.needs_rerender.insert(descendant_id);
            self.needs_layout_recalc.insert(descendant_id);
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

/// Check if a point (in local space, centered at origin) is inside a rounded rectangle
/// This accounts for corner radius and handles "full" rounding (circular/pill-shaped buttons)
fn is_point_in_rounded_rect(
    local_pos: Vector2,
    size: Vector2,
    corner_radius: CornerRadius,
) -> bool {
    let half_w = size.x * 0.5;
    let half_h = size.y * 0.5;
    
    // Rounding values are normalized (0.0 to 1.0), where 1.0 = fully rounded
    // The maximum possible radius is the minimum of half width and half height
    let max_radius = half_w.min(half_h);
    
    // Convert normalized rounding values to actual pixel radii
    // Values >= 0.99 are treated as "full" (100% of max radius)
    // Other values are treated as percentages of max radius
    let tl_radius = if corner_radius.top_left >= 0.99 {
        max_radius
    } else {
        corner_radius.top_left * max_radius
    };
    let tr_radius = if corner_radius.top_right >= 0.99 {
        max_radius
    } else {
        corner_radius.top_right * max_radius
    };
    let br_radius = if corner_radius.bottom_right >= 0.99 {
        max_radius
    } else {
        corner_radius.bottom_right * max_radius
    };
    let bl_radius = if corner_radius.bottom_left >= 0.99 {
        max_radius
    } else {
        corner_radius.bottom_left * max_radius
    };
    
    // Quick AABB rejection test
    if local_pos.x.abs() > half_w || local_pos.y.abs() > half_h {
        return false;
    }
    
    let abs_x = local_pos.x.abs();
    let abs_y = local_pos.y.abs();
    
    // Determine which corner region we're in (if any)
    let (corner_radius, corner_center_x, corner_center_y) = if local_pos.x >= 0.0 && local_pos.y >= 0.0 {
        // Top-right corner
        (tr_radius, half_w - tr_radius, half_h - tr_radius)
    } else if local_pos.x >= 0.0 && local_pos.y < 0.0 {
        // Bottom-right corner
        (br_radius, half_w - br_radius, -(half_h - br_radius))
    } else if local_pos.x < 0.0 && local_pos.y >= 0.0 {
        // Top-left corner
        (tl_radius, -(half_w - tl_radius), half_h - tl_radius)
    } else {
        // Bottom-left corner
        (bl_radius, -(half_w - bl_radius), -(half_h - bl_radius))
    };
    
    // Check if point is in the central rectangular area (not in corner region)
    // A point is in the central area if it's not in any corner's rounded region
    let in_corner_region = abs_x > (half_w - corner_radius) && abs_y > (half_h - corner_radius);
    
    if !in_corner_region {
        // Point is in the central rectangular area
        return true;
    }
    
    // Point is in a corner region - check if it's inside the corner's circular arc
    // If no rounding on this corner, it's inside (shouldn't happen if in_corner_region is true, but be safe)
    if corner_radius <= 0.0 {
        return true;
    }
    
    // Check if point is inside the corner's circular arc
    let dx = local_pos.x - corner_center_x;
    let dy = local_pos.y - corner_center_y;
    let dist_sq = dx * dx + dy * dy;
    
    dist_sq <= corner_radius * corner_radius
}

impl UINode {
    pub fn internal_render_update(&mut self, api: &mut ScriptApi) {
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
            for (_, el) in elements.iter_mut() {
                match el {
                    UIElement::Button(b) => {
                        b.is_hovered = false;
                        b.is_pressed = false;
                    }
                    UIElement::TextInput(ti) => {
                        ti.is_hovered = false;
                    }
                    _ => {}
                }
            }
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
        let mut dirty_element_ids = Vec::new();
        let mut any_button_hovered = false;
        let mut any_text_input_hovered = false;
        let mut clicked_text_input_id = None;
        
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
        
        // Handle buttons
        for (_, element) in elements.iter_mut() {
            let UIElement::Button(button) = element else {
                continue;
            };
    
            if !button.get_visible() {
                continue;
            }
    
            let was_hovered = button.is_hovered;
            let was_pressed = button.is_pressed;
    
            // Size is stored as full size (not half-extents)
            // The renderer treats it as full size and halves it internally
            let size = *button.get_size();
            // Apply scale from transform
            let scaled_size = Vector2::new(
                size.x * button.global_transform.scale.x,
                size.y * button.global_transform.scale.y,
            );
    
            let center = button.global_transform.position;
            let corner_radius = button.panel_props().corner_radius;
            
            // Convert mouse position to button's local space (centered at origin)
            let local_pos = Vector2::new(
                mouse_pos.x - center.x,
                mouse_pos.y - center.y,
            );
            
            // Use rounded rectangle hit test
            let is_hovered = is_point_in_rounded_rect(
                local_pos,
                scaled_size,
                corner_radius,
            );
    
            button.is_hovered = is_hovered;
            button.is_pressed = is_hovered && mouse_pressed;
            
            if is_hovered {
                any_button_hovered = true;
            }
    
            let name = button.get_name();
            
            // Mark only this button element as needing rerender when button state changes
            let state_changed = (is_hovered != was_hovered) || (button.is_pressed != was_pressed);
            if state_changed {
                // Collect ID to mark after loop (avoid borrow conflict)
                dirty_element_ids.push(button.get_id());
                // Also mark the UI node so it gets processed
                api.scene.mark_needs_rerender(self.base.id);
            }
    
            if is_hovered != was_hovered {
                let signal = if is_hovered {
                    format!("{}_Hovered", name)
                } else {
                    format!("{}_NotHovered", name)
                };
                api.emit_signal_id(string_to_u64(&signal), &[]);
            }
    
            if button.is_pressed && !was_pressed {
                api.emit_signal_id(
                    string_to_u64(&format!("{}_Pressed", name)),
                    &[],
                );
            } else if !button.is_pressed && was_pressed && was_hovered {
                api.emit_signal_id(
                    string_to_u64(&format!("{}_Released", name)),
                    &[],
                );
            }
    
            button.was_pressed_last_frame = button.is_pressed;
        }
        
        // Handle TextInput, TextEdit, and CodeEdit elements
        for (_, element) in elements.iter_mut() {
            let (_is_hovered_result, _element_id, _was_focused) = match element {
                UIElement::TextInput(text_input) => {
                    if !text_input.get_visible() {
                        continue;
                    }
                    let was_hovered = text_input.is_hovered;
                    let was_focused = text_input.is_focused;
                    
                    let size = *text_input.get_size();
                    let scaled_size = Vector2::new(
                        size.x * text_input.global_transform.scale.x,
                        size.y * text_input.global_transform.scale.y,
                    );
                    
                    let center = text_input.global_transform.position;
                    let corner_radius = text_input.panel_props().corner_radius;
                    let local_pos = Vector2::new(
                        mouse_pos.x - center.x,
                        mouse_pos.y - center.y,
                    );
                    
                    let is_hovered = is_point_in_rounded_rect(
                        local_pos,
                        scaled_size,
                        corner_radius,
                    );
                    
                    text_input.is_hovered = is_hovered;
                    
                    if is_hovered {
                        any_text_input_hovered = true;
                        if mouse_just_pressed && !was_focused {
                            clicked_text_input_id = Some(text_input.get_id());
                        }
                    }
                    
                    if mouse_just_pressed && was_focused && !is_hovered {
                        text_input.is_focused = false;
                        self.focused_element = None;
                        dirty_element_ids.push(text_input.get_id());
                    }
                    
                    let state_changed = (is_hovered != was_hovered) || (text_input.is_focused != was_focused);
                    if state_changed {
                        dirty_element_ids.push(text_input.get_id());
                        api.scene.mark_needs_rerender(self.base.id);
                    }
                    
                    (is_hovered, text_input.get_id(), was_focused)
                }
                UIElement::TextEdit(text_edit) => {
                    if !text_edit.get_visible() {
                        continue;
                    }
                    let was_hovered = text_edit.is_hovered;
                    let was_focused = text_edit.is_focused;
                    
                    let size = *text_edit.get_size();
                    let scaled_size = Vector2::new(
                        size.x * text_edit.global_transform.scale.x,
                        size.y * text_edit.global_transform.scale.y,
                    );
                    
                    let center = text_edit.global_transform.position;
                    let corner_radius = text_edit.panel_props().corner_radius;
                    let local_pos = Vector2::new(
                        mouse_pos.x - center.x,
                        mouse_pos.y - center.y,
                    );
                    
                    let is_hovered = is_point_in_rounded_rect(
                        local_pos,
                        scaled_size,
                        corner_radius,
                    );
                    
                    text_edit.is_hovered = is_hovered;
                    
                    if is_hovered {
                        any_text_input_hovered = true;
                        if mouse_just_pressed && !was_focused {
                            clicked_text_input_id = Some(text_edit.get_id());
                        }
                    }
                    
                    if mouse_just_pressed && was_focused && !is_hovered {
                        text_edit.is_focused = false;
                        self.focused_element = None;
                        dirty_element_ids.push(text_edit.get_id());
                    }
                    
                    let state_changed = (is_hovered != was_hovered) || (text_edit.is_focused != was_focused);
                    if state_changed {
                        dirty_element_ids.push(text_edit.get_id());
                        api.scene.mark_needs_rerender(self.base.id);
                    }
                    
                    (is_hovered, text_edit.get_id(), was_focused)
                }
                UIElement::CodeEdit(code_edit) => {
                    if !code_edit.get_visible() {
                        continue;
                    }
                    let was_hovered = code_edit.is_hovered;
                    let was_focused = code_edit.is_focused;
                    
                    let size = *code_edit.get_size();
                    let scaled_size = Vector2::new(
                        size.x * code_edit.global_transform.scale.x,
                        size.y * code_edit.global_transform.scale.y,
                    );
                    
                    let center = code_edit.global_transform.position;
                    let corner_radius = code_edit.panel_props().corner_radius;
                    let local_pos = Vector2::new(
                        mouse_pos.x - center.x,
                        mouse_pos.y - center.y,
                    );
                    
                    let is_hovered = is_point_in_rounded_rect(
                        local_pos,
                        scaled_size,
                        corner_radius,
                    );
                    
                    code_edit.is_hovered = is_hovered;
                    
                    if is_hovered {
                        any_text_input_hovered = true;
                        if mouse_just_pressed && !was_focused {
                            clicked_text_input_id = Some(code_edit.get_id());
                        }
                    }
                    
                    if mouse_just_pressed && was_focused && !is_hovered {
                        code_edit.is_focused = false;
                        self.focused_element = None;
                        dirty_element_ids.push(code_edit.get_id());
                    }
                    
                    let state_changed = (is_hovered != was_hovered) || (code_edit.is_focused != was_focused);
                    if state_changed {
                        dirty_element_ids.push(code_edit.get_id());
                        api.scene.mark_needs_rerender(self.base.id);
                    }
                    
                    (is_hovered, code_edit.get_id(), was_focused)
                }
                _ => continue,
            };
        }
        
        // Handle focus change
        if let Some(clicked_id) = clicked_text_input_id {
            // Unfocus previously focused element
            if let Some(old_focused) = self.focused_element {
                if let Some(old_element) = elements.get_mut(&old_focused) {
                    match old_element {
                        UIElement::TextInput(ti) => {
                            ti.is_focused = false;
                            dirty_element_ids.push(old_focused);
                        }
                        UIElement::TextEdit(te) => {
                            te.is_focused = false;
                            dirty_element_ids.push(old_focused);
                        }
                        UIElement::CodeEdit(ce) => {
                            ce.is_focused = false;
                            dirty_element_ids.push(old_focused);
                        }
                        _ => {}
                    }
                }
            }
            
            // Focus new element
            if let Some(new_element) = elements.get_mut(&clicked_id) {
                match new_element {
                    UIElement::TextInput(ti) => {
                        ti.is_focused = true;
                        self.focused_element = Some(clicked_id);
                        dirty_element_ids.push(clicked_id);
                        // Immediately mark for rerender
                        api.scene.mark_needs_rerender(self.base.id);
                    }
                    UIElement::TextEdit(te) => {
                        te.is_focused = true;
                        self.focused_element = Some(clicked_id);
                        dirty_element_ids.push(clicked_id);
                        // Immediately mark for rerender
                        api.scene.mark_needs_rerender(self.base.id);
                    }
                    UIElement::CodeEdit(ce) => {
                        ce.is_focused = true;
                        self.focused_element = Some(clicked_id);
                        dirty_element_ids.push(clicked_id);
                        // Immediately mark for rerender
                        api.scene.mark_needs_rerender(self.base.id);
                    }
                    _ => {}
                }
            }
        }
        
        // Update cursor icon based on hover state
        if let Some(tx) = api.scene.get_command_sender() {
            use crate::scripting::app_command::{AppCommand, CursorIcon};
            let cursor_icon = if any_button_hovered {
                CursorIcon::Hand
            } else if any_text_input_hovered {
                CursorIcon::Text
            } else {
                CursorIcon::Default
            };
            let _ = tx.send(AppCommand::SetCursorIcon(cursor_icon));
        }
        
        // Handle keyboard input for focused TextInput, TextEdit, or CodeEdit
        if let Some(focused_id) = self.focused_element {
            if let Some(element) = elements.get_mut(&focused_id) {
                let input_mgr = api.scene.get_input_manager();
                if let Some(mgr) = input_mgr {
                    let mut mgr = mgr.lock().unwrap();
                    use winit::keyboard::KeyCode;
                    
                    // Get text input from IME
                    let text_input_from_ime = mgr.get_text_input().to_string();
                    
                    // Clear the buffer immediately after reading so we don't process it twice
                    mgr.clear_text_input();
                    
                    
                    // Drop the lock before processing
                    drop(mgr);
                    let mut needs_rerender = false;
                    
                    // Reacquire the lock for processing keys
                    let input_mgr = api.scene.get_input_manager();
                    if let Some(mgr) = input_mgr {
                        let mut mgr = mgr.lock().unwrap();
                    
                    match element {
                        UIElement::TextInput(text_input) => {
                            let mut text_changed = false;
                            
                            // Handle text input from IME
                            let text_to_insert = &text_input_from_ime;
                            if !text_to_insert.is_empty() {
                                println!("[DEBUG] Inserting text: {:?}, current content: {:?}", text_to_insert, text_input.get_text());
                                text_input.insert_text(text_to_insert);
                                text_input.cursor_blink_timer = 0.0; // Reset blink timer to make cursor visible
                                println!("[DEBUG] After insert, content: {:?}", text_input.get_text());
                                text_changed = true;
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            
                            // Handle special keys with repeat
                            if mgr.is_key_triggered(KeyCode::Backspace) {
                                text_input.delete_backward();
                                text_input.cursor_blink_timer = 0.0; // Reset blink timer to make cursor visible
                                text_changed = true;
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                                println!("[DEBUG] Backspace - text now: '{}' (len: {})", text_input.get_text(), text_input.get_text().len());
                            }
                            if mgr.is_key_triggered(KeyCode::Delete) {
                                text_input.delete_forward();
                                text_input.cursor_blink_timer = 0.0; // Reset blink timer to make cursor visible
                                text_changed = true;
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            if mgr.is_key_triggered(KeyCode::ArrowLeft) {
                                text_input.move_cursor_left();
                                text_input.cursor_blink_timer = 0.0; // Reset blink timer to make cursor visible
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            if mgr.is_key_triggered(KeyCode::ArrowRight) {
                                text_input.move_cursor_right();
                                text_input.cursor_blink_timer = 0.0; // Reset blink timer to make cursor visible
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            if mgr.is_key_triggered(KeyCode::Home) {
                                text_input.move_cursor_home();
                                text_input.cursor_blink_timer = 0.0; // Reset blink timer to make cursor visible
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            if mgr.is_key_triggered(KeyCode::End) {
                                text_input.move_cursor_end();
                                text_input.cursor_blink_timer = 0.0; // Reset blink timer to make cursor visible
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            
                            text_input.update_cursor_blink(0.016);
                            
                            // If text changed, mark for layout recalculation
                            if text_changed {
                                self.mark_element_needs_layout(focused_id);
                            }
                        }
                        UIElement::TextEdit(text_edit) => {
                            let mut text_changed = false;
                            
                            // Handle text input from IME
                            let text_to_insert = mgr.get_text_input();
                            if !text_to_insert.is_empty() {
                                text_edit.insert_text(&text_to_insert);
                                text_changed = true;
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            
                            // Handle Enter key for newline
                            if mgr.is_key_triggered(KeyCode::Enter) {
                                text_edit.insert_newline();
                                text_changed = true;
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            
                            // Handle special keys with repeat
                            if mgr.is_key_triggered(KeyCode::Backspace) {
                                text_edit.delete_backward();
                                text_changed = true;
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            if mgr.is_key_triggered(KeyCode::Delete) {
                                text_edit.delete_forward();
                                text_changed = true;
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            if mgr.is_key_triggered(KeyCode::ArrowLeft) {
                                text_edit.move_cursor_left();
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            if mgr.is_key_triggered(KeyCode::ArrowRight) {
                                text_edit.move_cursor_right();
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            if mgr.is_key_triggered(KeyCode::ArrowUp) {
                                text_edit.move_cursor_up();
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            if mgr.is_key_triggered(KeyCode::ArrowDown) {
                                text_edit.move_cursor_down();
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            if mgr.is_key_triggered(KeyCode::Home) {
                                text_edit.move_cursor_line_start();
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            if mgr.is_key_triggered(KeyCode::End) {
                                text_edit.move_cursor_line_end();
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            
                            text_edit.update_cursor_blink(0.016);
                            
                            // If text changed, mark for layout recalculation
                            if text_changed {
                                self.mark_element_needs_layout(focused_id);
                            }
                        }
                        UIElement::CodeEdit(code_edit) => {
                            let mut text_changed = false;
                            
                            // Handle text input from IME
                            let text_to_insert = mgr.get_text_input();
                            if !text_to_insert.is_empty() {
                                code_edit.insert_text(&text_to_insert);
                                text_changed = true;
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            
                            // Handle Enter key for newline
                            if mgr.is_key_triggered(KeyCode::Enter) {
                                code_edit.insert_newline();
                                text_changed = true;
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            
                            // Handle special keys with repeat
                            if mgr.is_key_triggered(KeyCode::Backspace) {
                                code_edit.delete_backward();
                                text_changed = true;
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            if mgr.is_key_triggered(KeyCode::Delete) {
                                code_edit.delete_forward();
                                text_changed = true;
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            if mgr.is_key_triggered(KeyCode::ArrowLeft) {
                                code_edit.move_cursor_left();
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            if mgr.is_key_triggered(KeyCode::ArrowRight) {
                                code_edit.move_cursor_right();
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            if mgr.is_key_triggered(KeyCode::ArrowUp) {
                                code_edit.move_cursor_up();
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            if mgr.is_key_triggered(KeyCode::ArrowDown) {
                                code_edit.move_cursor_down();
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            if mgr.is_key_triggered(KeyCode::Home) {
                                code_edit.move_cursor_line_start();
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            if mgr.is_key_triggered(KeyCode::End) {
                                code_edit.move_cursor_line_end();
                                dirty_element_ids.push(focused_id);
                                needs_rerender = true;
                            }
                            
                            code_edit.update_cursor_blink(0.016);
                            
                            // If text changed, mark for layout recalculation
                            if text_changed {
                                self.mark_element_needs_layout(focused_id);
                            }
                        }
                        _ => {}
                    }
                    
                    // Drop the mutex guard before calling mark_needs_rerender
                    drop(mgr);
                    
                    if needs_rerender {
                        api.scene.mark_needs_rerender(self.base.id);
                    }
                    }
                }
            }
        }
        
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