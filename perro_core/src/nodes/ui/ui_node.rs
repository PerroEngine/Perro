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
use serde_json::Value;
use smallvec::SmallVec;

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
            props: None,
            elements: None,
            root_ids: None,
            needs_rerender: HashSet::new(),
            needs_layout_recalc: HashSet::new(),
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
                if let UIElement::Button(b) = el {
                    b.is_hovered = false;
                    b.is_pressed = false;
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
        let mut dirty_button_ids = Vec::new();
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
    
            let name = button.get_name();
            
            // Mark only this button element as needing rerender when button state changes
            let state_changed = (is_hovered != was_hovered) || (button.is_pressed != was_pressed);
            if state_changed {
                // Collect ID to mark after loop (avoid borrow conflict)
                dirty_button_ids.push(button.get_id());
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
        
        // Mark all dirty buttons after the loop (avoid borrow conflict)
        for button_id in dirty_button_ids {
            self.mark_element_needs_rerender(button_id);
        }
    }
}

impl crate::nodes::node_registry::NodeWithInternalRenderUpdate for UINode {
    fn internal_render_update(&mut self, api: &mut crate::scripting::api::ScriptApi) {
        self.internal_render_update(api);
    }
}