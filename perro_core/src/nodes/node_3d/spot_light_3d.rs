use crate::node_3d::node_3d::Node3D;
use std::ops::{Deref, DerefMut};

impl Deref for SpotLight3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for SpotLight3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct SpotLight3D {
    pub base: Node3D,
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub inner_angle_radians: f32,
    pub outer_angle_radians: f32,
    pub active: bool,
}

impl SpotLight3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
            range: 12.0,
            inner_angle_radians: 20.0_f32.to_radians(),
            outer_angle_radians: 30.0_f32.to_radians(),
            active: true,
        }
    }
}

impl Default for SpotLight3D {
    fn default() -> Self {
        Self::new()
    }
}
