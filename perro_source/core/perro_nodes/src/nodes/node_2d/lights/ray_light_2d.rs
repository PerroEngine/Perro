use crate::node_2d::Node2D;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug)]
pub struct RayLight2D {
    pub base: Node2D,
    pub color: [f32; 3],
    pub intensity: f32,
    pub cast_shadows: bool,
    pub active: bool,
}

impl RayLight2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
            cast_shadows: false,
            active: true,
        }
    }
}

impl Default for RayLight2D {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for RayLight2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for RayLight2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
