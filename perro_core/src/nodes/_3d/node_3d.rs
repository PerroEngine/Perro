use crate::nodes::node::Node;
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
    pub ty: Cow<'static, str>,

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
    pub node: Node,
}

impl Node3D {
    /// Create a new Node3D with the given name.
    pub fn new(name: &str) -> Self {
        Self {
            ty: Cow::Borrowed("Node3D"),
            transform: Transform3D::default(),
            pivot: Vector3 {
                x: 0.5,
                y: 0.5,
                z: 0.5,
            },
            visible: default_visible(),
            node: Node::new(name, None),
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
        &self.node
    }
}

impl DerefMut for Node3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.node
    }
}