use crate::Vector2;
use crate::nodes::node::Node;
use crate::structs2d::Transform2D;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

fn default_visible() -> bool {
    true
}
fn is_default_visible(v: &bool) -> bool {
    *v == default_visible()
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct Node2D {
    #[serde(rename = "type")]
    pub ty: Cow<'static, str>,

    #[serde(
        skip_serializing_if = "Transform2D::is_default",
        default = "Transform2D::default"
    )]
    pub transform: Transform2D,

    #[serde(
        skip_serializing_if = "Vector2::is_half_half",
        default = "Vector2::default_pivot"
    )]
    pub pivot: Vector2,

    #[serde(skip_serializing_if = "is_zero_i32", default)]
    pub z_index: i32,

    #[serde(
        default = "default_visible",
        skip_serializing_if = "is_default_visible"
    )]
    pub visible: bool,

    // Parent
    pub node: Node,
}

fn is_zero_i32(value: &i32) -> bool {
    *value == 0
}

impl Node2D {
    pub fn new(name: &str) -> Self {
        Self {
            ty: Cow::Borrowed("Node2D"),
            transform: Transform2D::default(),

            pivot: Vector2 { x: 0.5, y: 0.5 },

            z_index: 0,

            visible: default_visible(),
            // Parent
            node: Node::new(name, None),
        }
    }
    pub fn get_visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

impl Deref for Node2D {
    type Target = Node;

    fn deref(&self) -> &Self::Target {
        &self.node
    }
}

impl DerefMut for Node2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.node
    }
}
