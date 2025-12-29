use crate::nodes::node::Node;
use crate::nodes::node_registry::NodeType;
use crate::structs3d::{Transform3D, Vector3};
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
pub struct Node3D {
    #[serde(rename = "type")]
    pub ty: NodeType,

    #[serde(
        skip_serializing_if = "Transform3D::is_default",
        default = "Transform3D::default"
    )]
    pub transform: Transform3D,

    /// Optional pivot point for scaling/rotation center (defaults to {0.5,0.5,0.5})
    #[serde(
        skip_serializing_if = "Vector3::is_half_half_half",
        default = "Vector3::default_pivot"
    )]
    pub pivot: Vector3,

    #[serde(
        default = "default_visible",
        skip_serializing_if = "is_default_visible"
    )]
    pub visible: bool,

    /// Wrapped base node with name, uuid, parent relationship, etc.
    #[serde(rename = "base")]
    pub base: Node,
}

impl Node3D {
    /// Create a new Node3D.
    pub fn new() -> Self {
        let mut base = Node::new();
        base.name = Cow::Borrowed("Node3D");
        Self {
            ty: NodeType::Node3D,
            transform: Transform3D::default(),
            pivot: Vector3::new(0.5, 0.5, 0.5),
            visible: default_visible(),
            base,
        }
    }

    /// Returns if the node is visible
    pub fn get_visible(&self) -> bool {
        self.visible
    }

    /// Sets node visibility
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

impl Deref for Node3D {
    type Target = Node;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Node3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
