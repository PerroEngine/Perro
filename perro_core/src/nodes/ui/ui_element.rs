use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    ast::FurStyle,
    ui_elements::ui_panel::*,
    Transform2D,
    Vector2,
};

/// Insets for margin/padding
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct EdgeInsets {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

/// Base data shared by all UI elements
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BaseUIElement {
    pub name: Option<String>,

    pub parent: Option<String>,
    pub children: Vec<String>,

    pub visible: bool,

    pub transform: Transform2D,
    pub global_transform: Transform2D,

    pub size: Vector2,
    pub pivot: Vector2,

    #[serde(default)]
    pub margin: EdgeInsets,

    #[serde(default)]
    pub padding: EdgeInsets,
}

impl Default for BaseUIElement {
    fn default() -> Self {
        Self {
            name: Some(Uuid::new_v4().to_string()),
            parent: None,
            children: Vec::new(),
            visible: true,
            transform: Transform2D::default(),
            global_transform: Transform2D::default(),
            size: Vector2::new(32.0, 32.0),
            pivot: Vector2::new(0.5, 0.5),
            margin: EdgeInsets::default(),
            padding: EdgeInsets::default(),
        }
    }
}

/// Trait implemented by all UI elements
#[enum_dispatch]
pub trait BaseElement {
    fn get_name(&self) -> &str;
    fn set_name(&mut self, name: &str);

    fn get_visible(&self) -> bool;
    fn set_visible(&mut self, visible: bool);

    fn get_parent(&self) -> Option<&String>;
    fn set_parent(&mut self, parent: Option<String>);

    fn get_children(&self) -> &[String];
    fn set_children(&mut self, children: Vec<String>);

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
}

/// Macro to implement BaseElement for a UI type
#[macro_export]
macro_rules! impl_ui_element {
    ($ty:ty) => {
        impl crate::ui_element::BaseElement for $ty {
            fn get_name(&self) -> &str {
                self.name.as_deref().unwrap_or("")
            }
            fn set_name(&mut self, name: &str) {
                self.name = Some(name.to_string());
            }

            fn get_visible(&self) -> bool {
                self.visible
            }
            fn set_visible(&mut self, visible: bool) {
                self.visible = visible;
            }

            fn get_parent(&self) -> Option<&String> {
                self.parent.as_ref()
            }
            fn set_parent(&mut self, parent: Option<String>) {
                self.parent = parent;
            }

            fn get_children(&self) -> &[String] {
                &self.children
            }
            fn set_children(&mut self, children: Vec<String>) {
                self.children = children;
            }

            // Local transform
            fn get_transform(&self) -> &crate::Transform2D {
                &self.transform
            }
            fn get_transform_mut(&mut self) -> &mut crate::Transform2D {
                &mut self.transform
            }

            // Global transform
            fn get_global_transform(&self) -> &crate::Transform2D {
                &self.global_transform
            }
            fn set_global_transform(&mut self, transform: crate::Transform2D) {
                self.global_transform = transform;
            }

            // Size
            fn get_size(&self) -> &crate::Vector2 {
                &self.size
            }
            fn set_size(&mut self, size: crate::Vector2) {
                self.size = size;
            }

            // Pivot
            fn get_pivot(&self) -> &crate::Vector2 {
                &self.pivot
            }
            fn set_pivot(&mut self, pivot: crate::Vector2) {
                self.pivot = pivot;
            }
        }
    };
}

/// Enum of all UI elements
#[derive(Serialize, Deserialize, Clone, Debug)]
#[enum_dispatch(BaseElement)]
pub enum UIElement {
    Panel(UIPanel),
}