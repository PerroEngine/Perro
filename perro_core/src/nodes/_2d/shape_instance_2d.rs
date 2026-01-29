use crate::nodes::_2d::node_2d::Node2D;
use crate::nodes::node_registry::NodeType;
use crate::structs::Color;
use crate::structs2d::Shape2D;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

// Optimized field order: ty (1 byte), filled (1 byte), then larger fields
// Groups small fields together to minimize padding
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct ShapeInstance2D {
    #[serde(rename = "type")]
    pub ty: NodeType,

    /// Fill or outline only
    #[serde(skip_serializing_if = "is_false", default = "default_false")]
    pub filled: bool,

    #[serde(rename = "base")]
    pub base: Node2D,

    /// Shape type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<Shape2D>,

    /// Color for the shape
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

fn default_false() -> bool {
    false
}
fn is_false(v: &bool) -> bool {
    !*v
}

impl ShapeInstance2D {
    pub fn new() -> Self {
        let mut base = Node2D::new();
        base.name = Cow::Borrowed("ShapeInstance2D");
        Self {
            ty: NodeType::ShapeInstance2D,
            filled: false,
            base,
            shape: None,
            color: None,
        }
    }

    pub fn set_shape(&mut self, shape: Shape2D) {
        self.shape = Some(shape);
    }

    pub fn get_shape(&self) -> Option<Shape2D> {
        self.shape
    }
}

impl Deref for ShapeInstance2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for ShapeInstance2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
