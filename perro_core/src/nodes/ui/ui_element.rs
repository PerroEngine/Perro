use std::collections::HashMap;

use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    ast::FurAnchor,
    structs2d::{Color, Transform2D, Vector2},
    ui_elements::{
        ui_container::{BoxContainer, GridLayout, Layout, UIPanel},
        ui_text::UIText,
    },
};

/// Base data shared by all UI elements
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BaseUIElement {
    pub id: Uuid,
    pub name: String,
    pub parent: Option<Uuid>,
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
            parent: None,
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

    fn get_parent(&self) -> Option<Uuid>;
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
    fn get_modulate(&self) -> Option<&crate::structs2d::Color>;
    fn set_modulate(&mut self, color: Option<crate::structs2d::Color>);

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

            fn get_parent(&self) -> Option<uuid::Uuid> {
                self.base.parent
            }
            fn set_parent(&mut self, parent: Option<uuid::Uuid>) {
                self.base.parent = parent;
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

            fn get_anchor(&self) -> &crate::ast::FurAnchor {
                &self.base.anchor
            }
            fn set_anchor(&mut self, anchor: crate::ast::FurAnchor) {
                self.base.anchor = anchor;
            }

            fn get_modulate(&self) -> Option<&crate::structs2d::Color> {
                self.base.modulate.as_ref()
            }
            fn set_modulate(&mut self, color: Option<crate::structs2d::Color>) {
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

/// Enum of all UI elements
#[derive(Serialize, Deserialize, Clone, Debug)]
#[enum_dispatch(BaseElement)]
pub enum UIElement {
    BoxContainer(BoxContainer),
    Panel(UIPanel),
    Layout(Layout),
    GridLayout(GridLayout),

    Text(UIText),
}
