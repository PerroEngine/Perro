use crate::{nodes::_2d::node_2d::Node2D, nodes::node_registry::NodeType, structs2d::Shape2D};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct CollisionShape2D {
    #[serde(rename = "type")]
    pub ty: NodeType,

    pub base: Node2D,

    /// The shape type and dimensions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<Shape2D>,

    /// Rapier collider handle (runtime only, not serialized)
    #[serde(skip)]
    pub collider_handle: Option<rapier2d::prelude::ColliderHandle>,
}

impl CollisionShape2D {
    pub fn new() -> Self {
        let mut base = Node2D::new();
        base.name = Cow::Borrowed("CollisionShape2D");
        Self {
            ty: NodeType::CollisionShape2D,
            base,
            shape: None,
            collider_handle: None,
        }
    }

    pub fn set_shape(&mut self, shape: Shape2D) {
        self.shape = Some(shape);
    }

    pub fn get_shape(&self) -> Option<Shape2D> {
        self.shape
    }
}

impl Deref for CollisionShape2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for CollisionShape2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
