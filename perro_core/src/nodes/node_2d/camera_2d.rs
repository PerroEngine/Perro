use std::ops::{Deref, DerefMut};

use crate::node_2d::node_2d::Node2D;

impl Deref for Camera2D {
    type Target = Node2D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Camera2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug, Default)]
pub struct Camera2D {
    pub base: Node2D,
    pub zoom: f32,
    pub active: bool
}

impl Camera2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            zoom: 0f32,
            active: false
        }
    }
}
