use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ast::FurAnchor, ui_elements::ui_panel::UIPanel, Color, Transform2D, Vector2};

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

    // 🔹 Shared props
    pub anchor: FurAnchor,
    pub modulate: Option<Color>,
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
            anchor: FurAnchor::Center,
            modulate: None,
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

    // Anchor
    fn get_anchor(&self) -> &FurAnchor;
    fn set_anchor(&mut self, anchor: FurAnchor);

    // Modulate
    fn get_modulate(&self) -> Option<&crate::Color>;
    fn set_modulate(&mut self, color: Option<crate::Color>);
}

/// Macro to implement BaseElement for a UI type
#[macro_export]
macro_rules! impl_ui_element {
    ($ty:ty) => {
        impl crate::ui_element::BaseElement for $ty {
            fn get_name(&self) -> &str {
                self.base.name.as_deref().unwrap_or("")
            }
            fn set_name(&mut self, name: &str) {
                self.base.name = Some(name.to_string());
            }

            fn get_visible(&self) -> bool {
                self.base.visible
            }
            fn set_visible(&mut self, visible: bool) {
                self.base.visible = visible;
            }

            fn get_parent(&self) -> Option<&String> {
                self.base.parent.as_ref()
            }
            fn set_parent(&mut self, parent: Option<String>) {
                self.base.parent = parent;
            }

            fn get_children(&self) -> &[String] {
                &self.base.children
            }
            fn set_children(&mut self, children: Vec<String>) {
                self.base.children = children;
            }

            fn get_transform(&self) -> &crate::Transform2D {
                &self.base.transform
            }
            fn get_transform_mut(&mut self) -> &mut crate::Transform2D {
                &mut self.base.transform
            }

            fn get_global_transform(&self) -> &crate::Transform2D {
                &self.base.global_transform
            }
            fn set_global_transform(&mut self, transform: crate::Transform2D) {
                self.base.global_transform = transform;
            }

            fn get_size(&self) -> &crate::Vector2 {
                &self.base.size
            }
            fn set_size(&mut self, size: crate::Vector2) {
                self.base.size = size;
            }

            fn get_pivot(&self) -> &crate::Vector2 {
                &self.base.pivot
            }
            fn set_pivot(&mut self, pivot: crate::Vector2) {
                self.base.pivot = pivot;
            }

            fn get_anchor(&self) -> &crate::ast::FurAnchor {
                &self.base.anchor
            }
            fn set_anchor(&mut self, anchor: crate::ast::FurAnchor) {
                self.base.anchor = anchor;
            }

            fn get_modulate(&self) -> Option<&crate::Color> {
                self.base.modulate.as_ref()
            }
            fn set_modulate(&mut self, color: Option<crate::Color>) {
                self.base.modulate = color;
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