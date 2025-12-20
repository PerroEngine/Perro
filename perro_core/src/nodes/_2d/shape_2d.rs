use crate::nodes::_2d::node_2d::Node2D;
use crate::structs::Color;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct Shape2D {
    #[serde(rename = "type")]
    pub ty: Cow<'static, str>,

    #[serde(rename = "base")]
    pub base: Node2D,

    /// Shape type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape_type: Option<ShapeType>,

    /// Color for the shape
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,

    /// Fill or outline only
    #[serde(skip_serializing_if = "is_false", default = "default_false")]
    pub filled: bool,
}

fn default_false() -> bool {
    false
}
fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ShapeType {
    Rectangle { width: f32, height: f32 },
    Circle { radius: f32 },
}

impl Shape2D {
    pub fn new() -> Self {
        let mut base = Node2D::new();
        base.name = Cow::Borrowed("Shape2D");
        Self {
            ty: Cow::Borrowed("Shape2D"),
            base,
            shape_type: None,
            color: None,
            filled: false,
        }
    }

    pub fn set_shape_type(&mut self, shape_type: ShapeType) {
        self.shape_type = Some(shape_type);
    }

    pub fn get_shape_type(&self) -> Option<ShapeType> {
        self.shape_type
    }
}

impl Deref for Shape2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Shape2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
