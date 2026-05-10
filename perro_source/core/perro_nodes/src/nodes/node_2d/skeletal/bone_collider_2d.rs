use crate::node_2d::Node2D;
use std::ops::{Deref, DerefMut};

/// Collision toggle node used by 2D bone physics.
#[derive(Clone, Debug)]
pub struct BoneCollider2D {
    pub base: Node2D,
    pub enabled: bool,
}

impl Default for BoneCollider2D {
    fn default() -> Self {
        Self::new()
    }
}

impl BoneCollider2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            enabled: true,
        }
    }
}

impl Deref for BoneCollider2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for BoneCollider2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
