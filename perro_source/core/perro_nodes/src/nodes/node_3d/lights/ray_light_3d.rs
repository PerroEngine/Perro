use crate::node_3d::Node3D;
use perro_structs::Color;
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
    pub color: Color,
    pub intensity: f32,
    pub cast_shadows: bool,
    pub shadow_strength: f32,
    pub shadow_depth_bias: f32,
    pub shadow_normal_bias: f32,
    pub active: bool,
}

impl RayLight3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            color: Color::WHITE,
            intensity: 1.0,
            cast_shadows: true,
            shadow_strength: 0.82,
            shadow_depth_bias: 0.00003,
            shadow_normal_bias: 0.005,
            active: true,
        }
    }
}

impl Default for RayLight3D {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::RayLight3D;

    #[test]
    fn default_shadow_bias_stays_contact_scale() {
        let light = RayLight3D::default();
        assert!(light.shadow_depth_bias <= 0.00003);
        assert!(light.shadow_normal_bias <= 0.005);
    }
}
