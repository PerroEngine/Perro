use std::ops::{Deref, DerefMut};

use crate::node_3d::node_3d::Node3D;

impl Deref for Camera3D {
    type Target = Node3D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Camera3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug, Default)]
pub struct Camera3D {
    pub base: Node3D,
    pub zoom: f32,
    pub active: bool,
}

impl Camera3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            zoom: 0f32,
            active: false,
        }
    }
}
