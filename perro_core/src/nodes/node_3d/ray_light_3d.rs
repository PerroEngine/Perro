use crate::node_3d::node_3d::Node3D;
use std::ops::{Deref, DerefMut};

impl Deref for RayLight3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for RayLight3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct RayLight3D {
    pub base: Node3D,
    pub color: [f32; 3],
    pub intensity: f32,
    pub active: bool,
}

impl RayLight3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
            active: true,
        }
    }
}

impl Default for RayLight3D {
    fn default() -> Self {
        Self::new()
    }
}
