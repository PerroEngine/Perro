use crate::node_2d::Node2D;
use perro_structs::Transform2D;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug)]
pub struct AmbientLight2D {
    pub transform: Transform2D,
    pub visible: bool,
    pub color: [f32; 3],
    pub intensity: f32,
    pub cast_shadows: bool,
    pub active: bool,
}

impl AmbientLight2D {
    pub const fn new() -> Self {
        Self {
            transform: Transform2D::IDENTITY,
            visible: true,
            color: [1.0, 1.0, 1.0],
            intensity: 0.0,
            cast_shadows: false,
            active: true,
        }
    }
}

impl Default for AmbientLight2D {
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

impl Deref for PointLight2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for PointLight2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct PointLight2D {
    pub base: Node2D,
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub cast_shadows: bool,
    pub active: bool,
}

impl PointLight2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
            range: 256.0,
            cast_shadows: false,
            active: true,
        }
    }
}

impl Default for PointLight2D {
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

#[derive(Clone, Debug)]
pub struct SpotLight2D {
    pub base: Node2D,
    pub color: [f32; 3],
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
            color: [1.0, 1.0, 1.0],
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
