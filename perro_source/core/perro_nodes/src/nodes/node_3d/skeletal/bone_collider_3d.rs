use crate::node_3d::Node3D;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug)]
pub struct BoneCollider3D {
    pub base: Node3D,
    pub enabled: bool,
}

impl Default for BoneCollider3D {
    fn default() -> Self {
        Self::new()
    }
}

impl BoneCollider3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            enabled: true,
        }
    }
}

impl Deref for BoneCollider3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for BoneCollider3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
