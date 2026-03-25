use crate::node_2d::Node2D;
use perro_structs::Vector2;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Triangle2DKind {
    #[default]
    Equilateral,
    Right,
    Isosceles,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape2D {
    Quad {
        width: f32,
        height: f32,
    },
    Circle {
        radius: f32,
    },
    Triangle {
        kind: Triangle2DKind,
        width: f32,
        height: f32,
    },
}

impl Default for Shape2D {
    fn default() -> Self {
        Self::Quad {
            width: 1.0,
            height: 1.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CollisionShape2D {
    pub base: Node2D,
    pub shape: Shape2D,
    pub sensor: bool,
    pub friction: f32,
    pub restitution: f32,
    pub density: f32,
}

impl Default for CollisionShape2D {
    fn default() -> Self {
        Self::new()
    }
}

impl CollisionShape2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            shape: Shape2D::Quad {
                width: 1.0,
                height: 1.0,
            },
            sensor: false,
            friction: 0.7,
            restitution: 0.0,
            density: 1.0,
        }
    }
}

impl Deref for CollisionShape2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for CollisionShape2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct StaticBody2D {
    pub base: Node2D,
    pub enabled: bool,
    pub physics_handle: Option<u64>,
}

impl Default for StaticBody2D {
    fn default() -> Self {
        Self::new()
    }
}

impl StaticBody2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            enabled: true,
            physics_handle: None,
        }
    }
}

impl Deref for StaticBody2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for StaticBody2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct Area2D {
    pub base: Node2D,
    pub enabled: bool,
    pub physics_handle: Option<u64>,
}

impl Default for Area2D {
    fn default() -> Self {
        Self::new()
    }
}

impl Area2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            enabled: true,
            physics_handle: None,
        }
    }
}

impl Deref for Area2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Area2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct RigidBody2D {
    pub base: Node2D,
    pub enabled: bool,
    pub physics_handle: Option<u64>,
    pub linear_velocity: Vector2,
    pub angular_velocity: f32,
    pub gravity_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub can_sleep: bool,
    pub lock_rotation: bool,
}

impl Default for RigidBody2D {
    fn default() -> Self {
        Self::new()
    }
}

impl RigidBody2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            enabled: true,
            physics_handle: None,
            linear_velocity: Vector2::ZERO,
            angular_velocity: 0.0,
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            can_sleep: true,
            lock_rotation: false,
        }
    }
}

impl Deref for RigidBody2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for RigidBody2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
