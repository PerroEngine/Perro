use crate::node_2d::Node2D;
use perro_structs::Color;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug)]
pub struct SpotLight2D {
    pub base: Node2D,
    pub color: Color,
    pub intensity: f32,
    pub range: f32,
    pub inner_angle_radians: f32,
    pub outer_angle_radians: f32,
    pub cast_shadows: bool,
    pub active: bool,
}

impl SpotLight2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            color: Color::WHITE,
            intensity: 1.0,
            range: 256.0,
            inner_angle_radians: 20.0_f32.to_radians(),
            outer_angle_radians: 30.0_f32.to_radians(),
            cast_shadows: false,
            active: true,
        }
    }
}

impl Default for SpotLight2D {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for SpotLight2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for SpotLight2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
