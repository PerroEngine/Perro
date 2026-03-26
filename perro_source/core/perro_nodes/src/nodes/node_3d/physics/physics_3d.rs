use crate::node_3d::Node3D;
use perro_structs::Vector3;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape3D {
    Cube { size: Vector3 },
    Sphere { radius: f32 },
    Capsule { radius: f32, half_height: f32 },
    Cylinder { radius: f32, half_height: f32 },
    Cone { radius: f32, half_height: f32 },
    TriPrism { size: Vector3 },
    TriangularPyramid { size: Vector3 },
    SquarePyramid { size: Vector3 },
}

impl Default for Shape3D {
    fn default() -> Self {
        Self::Cube { size: Vector3::ONE }
    }
}

#[derive(Clone, Debug)]
pub struct CollisionShape3D {
    pub base: Node3D,
    pub shape: Shape3D,
    pub sensor: bool,
    pub friction: f32,
    pub restitution: f32,
    pub density: f32,
}

impl Default for CollisionShape3D {
    fn default() -> Self {
        Self::new()
    }
}

impl CollisionShape3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            shape: Shape3D::Cube { size: Vector3::ONE },
            sensor: false,
            friction: 0.7,
            restitution: 0.0,
            density: 1.0,
        }
    }
}

impl Deref for CollisionShape3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for CollisionShape3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct StaticBody3D {
    pub base: Node3D,
    pub enabled: bool,
    pub physics_handle: Option<u64>,
}

impl Default for StaticBody3D {
    fn default() -> Self {
        Self::new()
    }
}

impl StaticBody3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            enabled: true,
            physics_handle: None,
        }
    }
}

impl Deref for StaticBody3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for StaticBody3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct Area3D {
    pub base: Node3D,
    pub enabled: bool,
    pub physics_handle: Option<u64>,
}

impl Default for Area3D {
    fn default() -> Self {
        Self::new()
    }
}

impl Area3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            enabled: true,
            physics_handle: None,
        }
    }
}

impl Deref for Area3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Area3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct RigidBody3D {
    pub base: Node3D,
    pub enabled: bool,
    pub physics_handle: Option<u64>,
    pub mass: f32,
    pub linear_velocity: Vector3,
    pub angular_velocity: Vector3,
    pub gravity_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub can_sleep: bool,
}

impl Default for RigidBody3D {
    fn default() -> Self {
        Self::new()
    }
}

impl RigidBody3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            enabled: true,
            physics_handle: None,
            mass: 1.0,
            linear_velocity: Vector3::ZERO,
            angular_velocity: Vector3::ZERO,
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            can_sleep: true,
        }
    }
}

impl Deref for RigidBody3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for RigidBody3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
