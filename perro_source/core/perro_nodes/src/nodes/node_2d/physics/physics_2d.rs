use crate::node_2d::Node2D;
use perro_ids::NodeID;
use perro_structs::{AudioInteraction, BitMask, CollisionPolicy, Vector2};
use std::ops::{Deref, DerefMut};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PhysicsForceProfile {
    #[default]
    Lift,
    Explosion,
    Current,
    Vortex,
    Custom,
}

#[derive(Clone, Debug)]
pub struct PhysicsForceEmitter2D {
    pub base: Node2D,
    pub enabled: bool,
    pub profile: PhysicsForceProfile,
    pub radius: f32,
    pub strength: f32,
    pub duration: f32,
    pub pulse: bool,
    pub falloff: f32,
    pub affect_bodies: bool,
    pub affect_water: bool,
    pub collision_layers: BitMask,
    pub collision_mask: BitMask,
    pub vectors: Vec<Vector2>,
    pub age: f32,
}

impl Default for PhysicsForceEmitter2D {
    fn default() -> Self {
        Self::new()
    }
}

impl PhysicsForceEmitter2D {
    pub fn new() -> Self {
        Self {
            base: Node2D::new(),
            enabled: true,
            profile: PhysicsForceProfile::Lift,
            radius: 8.0,
            strength: 1.0,
            duration: 0.0,
            pulse: false,
            falloff: 1.0,
            affect_bodies: true,
            affect_water: true,
            collision_layers: BitMask::ALL,
            collision_mask: BitMask::NONE,
            vectors: Vec::new(),
            age: 0.0,
        }
    }
}

impl Deref for PhysicsForceEmitter2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for PhysicsForceEmitter2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

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
    pub collision_layers: BitMask,
    pub collision_mask: BitMask,
    pub friction: f32,
    pub restitution: f32,
    pub density: f32,
    pub audio_interaction: Option<AudioInteraction>,
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
            collision_layers: BitMask::ALL,
            collision_mask: BitMask::NONE,
            friction: 0.7,
            restitution: 0.0,
            density: 1.0,
            audio_interaction: Some(AudioInteraction::new()),
        }
    }

    pub const fn collision_policy(&self) -> CollisionPolicy {
        CollisionPolicy::new(self.collision_layers, self.collision_mask)
    }

    pub fn set_collision_policy(&mut self, policy: CollisionPolicy) {
        self.collision_layers = policy.layers;
        self.collision_mask = policy.mask;
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
    pub collision_layers: BitMask,
    pub collision_mask: BitMask,
    pub audio_interaction: Option<AudioInteraction>,
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
            collision_layers: BitMask::ALL,
            collision_mask: BitMask::NONE,
            audio_interaction: Some(AudioInteraction::new()),
        }
    }

    pub const fn collision_policy(&self) -> CollisionPolicy {
        CollisionPolicy::new(self.collision_layers, self.collision_mask)
    }

    pub fn set_collision_policy(&mut self, policy: CollisionPolicy) {
        self.collision_layers = policy.layers;
        self.collision_mask = policy.mask;
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
    pub collision_layers: BitMask,
    pub collision_mask: BitMask,
    pub continuous_collision_detection: bool,
    pub mass: f32,
    pub linear_velocity: Vector2,
    pub angular_velocity: f32,
    pub gravity_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub can_sleep: bool,
    pub lock_rotation: bool,
    pub friction: f32,
    pub restitution: f32,
    pub density: f32,
    pub audio_interaction: Option<AudioInteraction>,
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
            collision_layers: BitMask::ALL,
            collision_mask: BitMask::NONE,
            continuous_collision_detection: true,
            mass: 1.0,
            linear_velocity: Vector2::ZERO,
            angular_velocity: 0.0,
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            can_sleep: true,
            lock_rotation: false,
            friction: 0.7,
            restitution: 0.0,
            density: 1.0,
            audio_interaction: Some(AudioInteraction::new()),
        }
    }

    pub const fn collision_policy(&self) -> CollisionPolicy {
        CollisionPolicy::new(self.collision_layers, self.collision_mask)
    }

    pub fn set_collision_policy(&mut self, policy: CollisionPolicy) {
        self.collision_layers = policy.layers;
        self.collision_mask = policy.mask;
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

/// Script-driven body: collide vs static/rigid, no dynamics, no engine motion.
/// No velocity/force/gravity state; mv via transform or move api only.
#[derive(Clone, Debug)]
pub struct CharacterBody2D {
    pub base: Node2D,
    pub enabled: bool,
    pub physics_handle: Option<u64>,
    pub collision_layers: BitMask,
    pub collision_mask: BitMask,
    pub friction: f32,
    pub restitution: f32,
    pub density: f32,
    pub audio_interaction: Option<AudioInteraction>,
}

impl Default for CharacterBody2D {
    fn default() -> Self {
        Self::new()
    }
}

impl CharacterBody2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            enabled: true,
            physics_handle: None,
            collision_layers: BitMask::ALL,
            collision_mask: BitMask::NONE,
            friction: 0.7,
            restitution: 0.0,
            density: 1.0,
            audio_interaction: Some(AudioInteraction::new()),
        }
    }

    pub const fn collision_policy(&self) -> CollisionPolicy {
        CollisionPolicy::new(self.collision_layers, self.collision_mask)
    }

    pub fn set_collision_policy(&mut self, policy: CollisionPolicy) {
        self.collision_layers = policy.layers;
        self.collision_mask = policy.mask;
    }
}

impl Deref for CharacterBody2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for CharacterBody2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct PinJoint2D {
    pub base: Node2D,
    pub body_a: NodeID,
    pub body_b: NodeID,
    pub anchor_a: Vector2,
    pub anchor_b: Vector2,
    pub enabled: bool,
    pub collide_connected: bool,
}

impl Default for PinJoint2D {
    fn default() -> Self {
        Self::new()
    }
}

impl PinJoint2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            body_a: NodeID::nil(),
            body_b: NodeID::nil(),
            anchor_a: Vector2::ZERO,
            anchor_b: Vector2::ZERO,
            enabled: true,
            collide_connected: false,
        }
    }
}

impl Deref for PinJoint2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for PinJoint2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct DistanceJoint2D {
    pub base: Node2D,
    pub body_a: NodeID,
    pub body_b: NodeID,
    pub anchor_a: Vector2,
    pub anchor_b: Vector2,
    pub enabled: bool,
    pub collide_connected: bool,
    pub min_distance: f32,
    pub max_distance: f32,
}

impl Default for DistanceJoint2D {
    fn default() -> Self {
        Self::new()
    }
}

impl DistanceJoint2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            body_a: NodeID::nil(),
            body_b: NodeID::nil(),
            anchor_a: Vector2::ZERO,
            anchor_b: Vector2::ZERO,
            enabled: true,
            collide_connected: false,
            min_distance: 0.0,
            max_distance: 1.0,
        }
    }
}

impl Deref for DistanceJoint2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for DistanceJoint2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct FixedJoint2D {
    pub base: Node2D,
    pub body_a: NodeID,
    pub body_b: NodeID,
    pub anchor_a: Vector2,
    pub anchor_b: Vector2,
    pub enabled: bool,
    pub collide_connected: bool,
}

impl Default for FixedJoint2D {
    fn default() -> Self {
        Self::new()
    }
}

impl FixedJoint2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            body_a: NodeID::nil(),
            body_b: NodeID::nil(),
            anchor_a: Vector2::ZERO,
            anchor_b: Vector2::ZERO,
            enabled: true,
            collide_connected: false,
        }
    }
}

impl Deref for FixedJoint2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for FixedJoint2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
