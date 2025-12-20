use std::{
    borrow::Cow,
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    Node,
    prelude::string_to_u64,
    rendering::graphics::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH},
    script::Var,
    scripting::api::ScriptApi,
    structs2d::Vector2,
    ui_element::{BaseElement, IntoUIInner, UIElement},
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
    pub ty: Cow<'static, str>,

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

    pub base: Node,
}

impl UINode {
    pub fn new() -> Self {
        let mut base = Node::new();
        base.name = Cow::Borrowed("UINode");
        Self {
            ty: Cow::Borrowed("UINode"),
            visible: default_visible(),
            // Base node
            base,
            fur_path: None,
            props: None,
            elements: None,
            root_ids: None,
        }
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

impl UINode {
    pub fn internal_fixed_update(&mut self, api: &mut ScriptApi) {
        // Only process if UI is visible and has elements
        if !self.visible {
            return;
        }
        
        let elements = match &mut self.elements {
            Some(e) => e,
            None => return,
        };
        
        // Get mouse position in screen space (pixels)
        let screen_mouse_pos = if let Some(mgr) = api.scene.get_input_manager() {
            let mgr = mgr.lock().unwrap();
            mgr.get_mouse_position()
        } else {
            return;
        };
        
        // Ignore initial (0, 0) mouse position - mouse hasn't moved yet
        // This prevents false hover detection at startup
        if screen_mouse_pos.x == 0.0 && screen_mouse_pos.y == 0.0 {
            // Reset all button hover states to false on first frame
            if let Some(elements) = &mut self.elements {
                for (_, element) in elements.iter_mut() {
                    if let UIElement::Button(button) = element {
                        button.is_hovered = false;
                        button.is_pressed = false;
                    }
                }
            }
            return;
        }
        
        // Convert screen coordinates to UI virtual coordinates
        // We need window size - try to get it from graphics if available
        // For now, we'll use a simple conversion assuming we can get window size
        // TODO: Get actual window size from graphics
        // For UI, we convert screen pixels to virtual space (1920x1080)
        // The conversion matches how the UI renderer works
        let window_width = 1920.0; // Default, should get from graphics
        let window_height = 1080.0; // Default, should get from graphics
        
        // Convert screen to UI virtual coordinates
        // UI uses centered coordinate system: (0,0) is center, ranges from -width/2 to +width/2
        let virtual_aspect = VIRTUAL_WIDTH / VIRTUAL_HEIGHT;
        let window_aspect = window_width / window_height;
        
        let (scale_x, scale_y) = if window_aspect > virtual_aspect {
            (virtual_aspect / window_aspect, 1.0)
        } else {
            (1.0, window_aspect / virtual_aspect)
        };
        
        // Normalize screen position to [0, 1]
        let normalized_x = screen_mouse_pos.x / window_width;
        let normalized_y = screen_mouse_pos.y / window_height;
        
        // Convert to virtual space coordinates (centered at 0,0)
        let mouse_pos = Vector2::new(
            (normalized_x - 0.5) * VIRTUAL_WIDTH * scale_x,
            (normalized_y - 0.5) * VIRTUAL_HEIGHT * scale_y,
        );
        
        // Get mouse button state
        let mouse_pressed = if let Some(mgr) = api.scene.get_input_manager() {
            let mgr = mgr.lock().unwrap();
            use crate::input::manager::MouseButton;
            mgr.is_mouse_button_pressed(MouseButton::Left)
        } else {
            false
        };
        
        // Check all button elements for mouse interaction
        for (_, element) in elements.iter_mut() {
            if let UIElement::Button(button) = element {
                // Skip if button is not visible
                if !button.get_visible() {
                    continue;
                }
                
                // Get signal base name (button's ID/name) - needed for debug output
                // Clone to avoid borrow issues
                let signal_base = button.get_name().to_string();
                
                // Get button bounds in screen space
                let button_pos = button.global_transform.position;
                let button_size = *button.get_size();
                let pivot = *button.get_pivot();
                
                // Calculate button bounds (accounting for pivot)
                // Pivot is typically (0.5, 0.5) for center, which means no offset needed
                // For other pivots, we need to adjust the bounds
                let half_width = button_size.x * 0.5;
                let half_height = button_size.y * 0.5;
                
                // Calculate offset from center based on pivot
                let pivot_offset_x = (pivot.x - 0.5) * button_size.x;
                let pivot_offset_y = (pivot.y - 0.5) * button_size.y;
                
                let left = button_pos.x - half_width + pivot_offset_x;
                let right = button_pos.x + half_width + pivot_offset_x;
                let top = button_pos.y - half_height - pivot_offset_y;
                let bottom = button_pos.y + half_height - pivot_offset_y;
                
                // Check if mouse is over button
                let is_hovered = mouse_pos.x >= left && mouse_pos.x <= right &&
                                 mouse_pos.y >= top && mouse_pos.y <= bottom;
                
                // Store previous state BEFORE updating (critical for state transitions)
                let was_hovered = button.is_hovered;
                let was_pressed = button.is_pressed;
                
                // Debug: Print every frame
                
                // Update button state immediately
                button.is_hovered = is_hovered;
                button.is_pressed = is_hovered && mouse_pressed;
                
                // Emit hover signals - check state transitions
                // Only emit if there's an actual state change
                if is_hovered != was_hovered {
                    if is_hovered {
                        // Mouse entered button (transition: not hovered -> hovered)
                        println!("Button hovered: {} (mouse: {:?}, bounds: [{:.1}, {:.1}] to [{:.1}, {:.1}], was_hovered: {})", 
                            signal_base, mouse_pos, left, top, right, bottom, was_hovered);
                        let signal = format!("{}_Hovered", signal_base);
                        let signal_id = string_to_u64(&signal);
                        api.emit_signal_id(signal_id, &[]);
                    } else {
                        // Mouse exited button (transition: hovered -> not hovered)
                        println!("Button not hovered: {} (mouse: {:?}, bounds: [{:.1}, {:.1}] to [{:.1}, {:.1}], was_hovered: {})", 
                            signal_base, mouse_pos, left, top, right, bottom, was_hovered);
                        let signal = format!("{}_NotHovered", signal_base);
                        let signal_id = string_to_u64(&signal);
                        api.emit_signal_id(signal_id, &[]);
                    }
                }
                
                // Emit press/release signals
                if button.is_pressed && !was_pressed {
                    // Button was just pressed
                    println!("Button pressed: {}", signal_base);
                    let signal = format!("{}_Pressed", signal_base);
                    let signal_id = string_to_u64(&signal);
                    api.emit_signal_id(signal_id, &[]);
                } else if !button.is_pressed && was_pressed && was_hovered {
                    // Button was just released (only if it was pressed and still hovered)
                    println!("Button released: {}", signal_base);
                    let signal = format!("{}_Released", signal_base);
                    let signal_id = string_to_u64(&signal);
                    api.emit_signal_id(signal_id, &[]);
                }
                
                // Store previous frame state
                button.was_pressed_last_frame = button.is_pressed;
            }
        }
    }
}
