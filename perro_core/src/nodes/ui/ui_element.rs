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
