use std::collections::HashMap;

use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    fur_ast::FurAnchor,
    structs::Color,
    structs2d::{Transform2D, Vector2},
    ui_elements::{
        ui_container::{BoxContainer, GridLayout, Layout, UIPanel},
        ui_text::UIText,
        ui_button::UIButton,
        ui_text_input::UITextInput,
        ui_text_edit::UITextEdit,
        ui_list_tree::UIListTree,
        ui_context_menu::UIContextMenu,
    },
};

// Helper function for serde default
fn uuid_nil() -> Uuid {
    Uuid::nil()
}

/// Base data shared by all UI elements
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BaseUIElement {
    pub id: Uuid,
    pub name: String,
    #[serde(rename = "parent", default = "uuid_nil", skip_serializing_if = "Uuid::is_nil")]
    pub parent_id: Uuid,
    pub children: Vec<Uuid>,

    pub visible: bool,

    pub transform: Transform2D,
    pub global_transform: Transform2D,

    pub size: Vector2,
    pub pivot: Vector2,

    // Shared props
    pub anchor: FurAnchor,
    pub modulate: Option<Color>,

    // Z-index for rendering order
    pub z_index: i32,

    pub style_map: HashMap<String, f32>,
}

impl Default for BaseUIElement {
    fn default() -> Self {
        let id = Uuid::new_v4();
        Self {
            id,
            name: id.to_string(),
            parent_id: Uuid::nil(),
            children: Vec::new(),
            visible: true,
            transform: Transform2D::default(),
            global_transform: Transform2D::default(),
            size: Vector2::new(32.0, 32.0),
            pivot: Vector2::new(0.5, 0.5),

            anchor: FurAnchor::Center,
            modulate: None,
            z_index: 0,
            style_map: HashMap::new(),
        }
    }
}

/// Trait implemented by all UI elements
#[enum_dispatch]
pub trait BaseElement {
    fn get_id(&self) -> Uuid;
    fn set_id(&mut self, id: Uuid);

    fn get_name(&self) -> &str;
    fn set_name(&mut self, name: &str);

    fn get_visible(&self) -> bool;
    fn set_visible(&mut self, visible: bool);

    fn get_parent(&self) -> Uuid;
    fn set_parent(&mut self, parent: Option<Uuid>);

    fn get_children(&self) -> &[Uuid];
    fn set_children(&mut self, children: Vec<Uuid>);

    // Local transform
    fn get_transform(&self) -> &Transform2D;
    fn get_transform_mut(&mut self) -> &mut Transform2D;

    // Global transform
    fn get_global_transform(&self) -> &Transform2D;
    fn set_global_transform(&mut self, transform: Transform2D);

    // Size
    fn get_size(&self) -> &Vector2;
    fn set_size(&mut self, size: Vector2);

    // Pivot
    fn get_pivot(&self) -> &Vector2;
    fn set_pivot(&mut self, pivot: Vector2);

    // Anchor
    fn get_anchor(&self) -> &FurAnchor;
    fn set_anchor(&mut self, anchor: FurAnchor);

    // Modulate
    fn get_modulate(&self) -> Option<&crate::structs::Color>;
    fn set_modulate(&mut self, color: Option<crate::structs::Color>);

    // Z-index
    fn get_z_index(&self) -> i32;
    fn set_z_index(&mut self, z_index: i32);

    // Style map
    fn get_style_map(&self) -> &HashMap<String, f32>;
    fn get_style_map_mut(&mut self) -> &mut HashMap<String, f32>;
}

/// Macro to implement BaseElement for a UI type
#[macro_export]
macro_rules! impl_ui_element {
    ($ty:ty) => {
        impl crate::ui_element::BaseElement for $ty {
            fn get_id(&self) -> uuid::Uuid {
                self.base.id
            }
            fn set_id(&mut self, id: uuid::Uuid) {
                self.base.id = id;
            }

            fn get_name(&self) -> &str {
                &self.base.name
            }
            fn set_name(&mut self, name: &str) {
                self.base.name = name.to_string();
            }

            fn get_visible(&self) -> bool {
                self.base.visible
            }
            fn set_visible(&mut self, visible: bool) {
                self.base.visible = visible;
            }

            fn get_parent(&self) -> uuid::Uuid {
                self.base.parent_id
            }
            fn set_parent(&mut self, parent: Option<uuid::Uuid>) {
                self.base.parent_id = parent.unwrap_or(uuid::Uuid::nil());
            }

            fn get_children(&self) -> &[uuid::Uuid] {
                &self.base.children
            }
            fn set_children(&mut self, children: Vec<uuid::Uuid>) {
                self.base.children = children;
            }

            fn get_transform(&self) -> &crate::structs2d::Transform2D {
                &self.base.transform
            }
            fn get_transform_mut(&mut self) -> &mut crate::structs2d::Transform2D {
                &mut self.base.transform
            }

            fn get_global_transform(&self) -> &crate::structs2d::Transform2D {
                &self.base.global_transform
            }
            fn set_global_transform(&mut self, transform: crate::structs2d::Transform2D) {
                self.base.global_transform = transform;
            }

            fn get_size(&self) -> &crate::structs2d::Vector2 {
                &self.base.size
            }
            fn set_size(&mut self, size: crate::structs2d::Vector2) {
                self.base.size = size;
            }

            fn get_pivot(&self) -> &crate::structs2d::Vector2 {
                &self.base.pivot
            }
            fn set_pivot(&mut self, pivot: crate::structs2d::Vector2) {
                self.base.pivot = pivot;
            }

            fn get_anchor(&self) -> &crate::fur_ast::FurAnchor {
                &self.base.anchor
            }
            fn set_anchor(&mut self, anchor: crate::fur_ast::FurAnchor) {
                self.base.anchor = anchor;
            }

            fn get_modulate(&self) -> Option<&crate::structs::Color> {
                self.base.modulate.as_ref()
            }
            fn set_modulate(&mut self, color: Option<crate::structs::Color>) {
                self.base.modulate = color;
            }

            fn get_z_index(&self) -> i32 {
                self.base.z_index
            }
            fn set_z_index(&mut self, z_index: i32) {
                self.base.z_index = z_index;
            }

            fn get_style_map(&self) -> &std::collections::HashMap<String, f32> {
                &self.base.style_map
            }
            fn get_style_map_mut(&mut self) -> &mut std::collections::HashMap<String, f32> {
                &mut self.base.style_map
            }
        }
        // Deref implementation
        impl std::ops::Deref for $ty {
            type Target = crate::ui_element::BaseUIElement;
            fn deref(&self) -> &Self::Target {
                &self.base
            }
        }

        impl std::ops::DerefMut for $ty {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.base
            }
        }
    };
}

/// Trait used to unwrap `UIElement` variants back into their concrete types.
/// Similar to `IntoInner` for `SceneNode`.
pub trait IntoUIInner<T> {
    fn into_ui_inner(self) -> T;
}

/// Context passed to UI elements during update
/// Contains shared state needed for element updates
pub struct UIUpdateContext<'a> {
    /// Mouse position in virtual UI space
    pub mouse_pos: crate::structs2d::Vector2,
    /// Whether mouse button is currently pressed
    pub mouse_pressed: bool,
    /// Whether mouse button was just pressed this frame
    pub mouse_just_pressed: bool,
    /// Whether mouse button is currently held down
    pub mouse_is_held: bool,
    /// Script API for emitting signals and accessing scene
    pub api: &'a mut crate::scripting::api::ScriptApi<'a>,
    /// Currently focused element ID (for text inputs, etc.)
    pub focused_element: Option<uuid::Uuid>,
    /// Callback to mark an element as needing rerender
    /// Uses 'static because closures are move closures that only capture owned values (Rc<RefCell<>>)
    pub mark_dirty: Box<dyn FnMut(uuid::Uuid) + 'static>,
    /// Callback to mark UI node as needing rerender
    /// Uses 'static because closures are move closures that only capture owned values (Rc<RefCell<>>)
    pub mark_ui_dirty: Box<dyn FnMut() + 'static>,
    /// Callback to mark an element as needing layout recalculation
    /// Uses 'static because closures are move closures that only capture owned values (Rc<RefCell<>>)
    pub mark_layout_dirty: Box<dyn FnMut(uuid::Uuid) + 'static>,
    /// Whether this element is currently focused
    pub is_focused: bool,
    /// Callback to request focus for this element (returns the previously focused element ID)
    /// Uses 'static because closures are move closures that only capture owned values (Rc<RefCell<>>)
    pub request_focus: Option<Box<dyn FnMut(uuid::Uuid) -> Option<uuid::Uuid> + 'static>>,
}

impl<'a> UIUpdateContext<'a> {
    /// Check if a key is currently pressed
    pub fn is_key_pressed(&self, key: winit::keyboard::KeyCode) -> bool {
        if let Some(input_mgr) = self.api.scene.get_input_manager() {
            let mgr = input_mgr.lock().unwrap();
            mgr.is_key_pressed(key)
        } else {
            false
        }
    }
    
    /// Check if a key was just triggered this frame
    pub fn is_key_triggered(&mut self, key: winit::keyboard::KeyCode) -> bool {
        if let Some(input_mgr) = self.api.scene.get_input_manager() {
            let mut mgr = input_mgr.lock().unwrap();
            mgr.is_key_triggered(key)
        } else {
            false
        }
    }
    
    /// Get text input from IME and clear the buffer
    pub fn get_text_input(&mut self) -> String {
        if let Some(input_mgr) = self.api.scene.get_input_manager() {
            let mut mgr = input_mgr.lock().unwrap();
            let text = mgr.get_text_input().to_string();
            mgr.clear_text_input();
            text
        } else {
            String::new()
        }
    }
    
    /// Check if right mouse button is pressed
    pub fn is_right_mouse_pressed(&self) -> bool {
        if let Some(input_mgr) = self.api.scene.get_input_manager() {
            let mgr = input_mgr.lock().unwrap();
            use crate::input::manager::MouseButton;
            mgr.is_mouse_button_pressed(MouseButton::Right)
        } else {
            false
        }
    }
}

/// Trait for UI elements that can update their internal state
/// Each element type implements this to handle its own update logic
pub trait UIElementUpdate {
    /// Update the element's internal state (mouse interactions, keyboard input, etc.)
    /// Returns true if the element needs to be rerendered
    fn internal_render_update(&mut self, ctx: &mut UIUpdateContext) -> bool;
}

/// Check if a point (in local space, centered at origin) is inside a rounded rectangle
/// This accounts for corner radius and handles "full" rounding (circular/pill-shaped buttons)
pub fn is_point_in_rounded_rect(
    local_pos: Vector2,
    size: Vector2,
    corner_radius: crate::ui_elements::ui_container::CornerRadius,
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

/// Enum of all UI elements
#[derive(Serialize, Deserialize, Clone, Debug)]
#[enum_dispatch(BaseElement)]
pub enum UIElement {
    BoxContainer(BoxContainer),
    Panel(UIPanel),
    Layout(Layout),
    GridLayout(GridLayout),

    Text(UIText),
    Button(UIButton),
    TextInput(UITextInput),
    TextEdit(UITextEdit),
    ListTree(UIListTree),
    ContextMenu(UIContextMenu),
}

// Implement IntoUIInner for each UI element type
impl IntoUIInner<UIText> for UIElement {
    fn into_ui_inner(self) -> UIText {
        match self {
            UIElement::Text(inner) => inner,
            _ => panic!("Cannot extract UIText from UIElement variant {:?}", self),
        }
    }
}

impl IntoUIInner<BoxContainer> for UIElement {
    fn into_ui_inner(self) -> BoxContainer {
        match self {
            UIElement::BoxContainer(inner) => inner,
            _ => panic!(
                "Cannot extract BoxContainer from UIElement variant {:?}",
                self
            ),
        }
    }
}

impl IntoUIInner<UIPanel> for UIElement {
    fn into_ui_inner(self) -> UIPanel {
        match self {
            UIElement::Panel(inner) => inner,
            _ => panic!("Cannot extract UIPanel from UIElement variant {:?}", self),
        }
    }
}

impl IntoUIInner<Layout> for UIElement {
    fn into_ui_inner(self) -> Layout {
        match self {
            UIElement::Layout(inner) => inner,
            _ => panic!("Cannot extract Layout from UIElement variant {:?}", self),
        }
    }
}

impl IntoUIInner<GridLayout> for UIElement {
    fn into_ui_inner(self) -> GridLayout {
        match self {
            UIElement::GridLayout(inner) => inner,
            _ => panic!(
                "Cannot extract GridLayout from UIElement variant {:?}",
                self
            ),
        }
    }
}

impl IntoUIInner<UIButton> for UIElement {
    fn into_ui_inner(self) -> UIButton {
        match self {
            UIElement::Button(inner) => inner,
            _ => panic!("Cannot extract UIButton from UIElement variant {:?}", self),
        }
    }
}

impl IntoUIInner<UITextInput> for UIElement {
    fn into_ui_inner(self) -> UITextInput {
        match self {
            UIElement::TextInput(inner) => inner,
            _ => panic!("Cannot extract UITextInput from UIElement variant {:?}", self),
        }
    }
}

impl IntoUIInner<UITextEdit> for UIElement {
    fn into_ui_inner(self) -> UITextEdit {
        match self {
            UIElement::TextEdit(inner) => inner,
            _ => panic!("Cannot extract UITextEdit from UIElement variant {:?}", self),
        }
    }
}


impl IntoUIInner<UIListTree> for UIElement {
    fn into_ui_inner(self) -> UIListTree {
        match self {
            UIElement::ListTree(inner) => inner,
            _ => panic!("Cannot extract UIListTree from UIElement variant {:?}", self),
        }
    }
}

impl IntoUIInner<UIContextMenu> for UIElement {
    fn into_ui_inner(self) -> UIContextMenu {
        match self {
            UIElement::ContextMenu(inner) => inner,
            _ => panic!("Cannot extract UIContextMenu from UIElement variant {:?}", self),
        }
    }
}
