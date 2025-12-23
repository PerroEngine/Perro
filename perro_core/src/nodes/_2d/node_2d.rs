use crate::Vector2;
use crate::nodes::node::Node;
use crate::nodes::node_registry::NodeType;
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Node2D {
    #[serde(rename = "type")]
    pub ty: NodeType,

    #[serde(
        skip_serializing_if = "Transform2D::is_default",
        default = "Transform2D::default"
    )]
    pub transform: Transform2D,

    /// World-space transform (calculated from parent's global_transform + local transform)
    /// This is runtime-only and not serialized
    #[serde(
        skip_serializing_if = "Transform2D::is_default",
        default = "Transform2D::default"
    )]
    pub global_transform: Transform2D,

    /// Flag indicating if the global transform needs to be recalculated
    /// When true, the global transform will be recalculated lazily when accessed
    #[serde(skip, default = "default_transform_dirty")]
    pub transform_dirty: bool,

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

    // Base node with name, uuid, parent relationship, etc.
    #[serde(rename = "base")]
    pub base: Node,
}

fn is_zero_i32(value: &i32) -> bool {
    *value == 0
}

fn default_transform_dirty() -> bool {
    true
}

impl Node2D {
    pub fn new() -> Self {
        let mut base = Node::new();
        base.name = Cow::Borrowed("Node2D");
        Self {
            ty: NodeType::Node2D,
            transform: Transform2D::default(),
            global_transform: Transform2D::default(),
            transform_dirty: true, // New nodes start dirty

            pivot: Vector2 { x: 0.5, y: 0.5 },

            z_index: 0,

            visible: default_visible(),
            // Base node
            base,
        }
    }
    pub fn get_visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Mark the transform as dirty, indicating the global transform needs recalculation
    pub fn mark_transform_dirty(&mut self) {
        self.transform_dirty = true;
    }

    /// Check if the transform is dirty
    pub fn is_transform_dirty(&self) -> bool {
        self.transform_dirty
    }

    /// Mark the transform as clean (after recalculation)
    pub fn mark_transform_clean(&mut self) {
        self.transform_dirty = false;
    }
}

impl Default for Node2D {
    fn default() -> Self {
        Self {
            ty: NodeType::Node2D,
            transform: Transform2D::default(),
            global_transform: Transform2D::default(),
            transform_dirty: true, // Default to dirty
            pivot: Vector2 { x: 0.5, y: 0.5 },
            z_index: 0,
            visible: default_visible(),
            base: {
                let mut base = Node::new();
                base.name = Cow::Borrowed("Node2D");
                base
            },
        }
    }
}

impl Deref for Node2D {
    type Target = Node;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Node2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
