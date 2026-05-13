use crate::node_3d::Node3D;
use perro_ids::NodeID;
use perro_structs::{AudioInteraction, BitMask, CollisionPolicy, Vector3};
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug, PartialEq)]
pub enum Shape3D {
    Cube { size: Vector3 },
    Sphere { radius: f32 },
    Capsule { radius: f32, half_height: f32 },
    Cylinder { radius: f32, half_height: f32 },
    Cone { radius: f32, half_height: f32 },
    TriPrism { size: Vector3 },
    TriangularPyramid { size: Vector3 },
    SquarePyramid { size: Vector3 },
    TriMesh { source: String },
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
    pub debug: bool,
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
            debug: false,
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
    pub collision_layers: BitMask,
    pub collision_mask: BitMask,
    pub friction: f32,
    pub restitution: f32,
    pub density: f32,
    pub audio_interaction: Option<AudioInteraction>,
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
    pub collision_layers: BitMask,
    pub collision_mask: BitMask,
    pub audio_interaction: Option<AudioInteraction>,
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
    pub collision_layers: BitMask,
    pub collision_mask: BitMask,
    pub continuous_collision_detection: bool,
    pub mass: f32,
    pub linear_velocity: Vector3,
    pub angular_velocity: Vector3,
    pub gravity_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub can_sleep: bool,
    pub friction: f32,
    pub restitution: f32,
    pub density: f32,
    pub audio_interaction: Option<AudioInteraction>,
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
            collision_layers: BitMask::ALL,
            collision_mask: BitMask::NONE,
            continuous_collision_detection: true,
            mass: 1.0,
            linear_velocity: Vector3::ZERO,
            angular_velocity: Vector3::ZERO,
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.0,
            can_sleep: true,
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

#[derive(Clone, Debug)]
pub struct BallJoint3D {
    pub base: Node3D,
    pub body_a: NodeID,
    pub body_b: NodeID,
    pub anchor_a: Vector3,
    pub anchor_b: Vector3,
    pub enabled: bool,
    pub collide_connected: bool,
}

impl Default for BallJoint3D {
    fn default() -> Self {
        Self::new()
    }
}

impl BallJoint3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            body_a: NodeID::nil(),
            body_b: NodeID::nil(),
            anchor_a: Vector3::ZERO,
            anchor_b: Vector3::ZERO,
            enabled: true,
            collide_connected: false,
        }
    }
}

impl Deref for BallJoint3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for BallJoint3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct HingeJoint3D {
    pub base: Node3D,
    pub body_a: NodeID,
    pub body_b: NodeID,
    pub anchor_a: Vector3,
    pub anchor_b: Vector3,
    pub axis: Vector3,
    pub enabled: bool,
    pub collide_connected: bool,
}

impl Default for HingeJoint3D {
    fn default() -> Self {
        Self::new()
    }
}

impl HingeJoint3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            body_a: NodeID::nil(),
            body_b: NodeID::nil(),
            anchor_a: Vector3::ZERO,
            anchor_b: Vector3::ZERO,
            axis: Vector3::new(0.0, 1.0, 0.0),
            enabled: true,
            collide_connected: false,
        }
    }
}

impl Deref for HingeJoint3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for HingeJoint3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct FixedJoint3D {
    pub base: Node3D,
    pub body_a: NodeID,
    pub body_b: NodeID,
    pub anchor_a: Vector3,
    pub anchor_b: Vector3,
    pub enabled: bool,
    pub collide_connected: bool,
}

impl Default for FixedJoint3D {
    fn default() -> Self {
        Self::new()
    }
}

impl FixedJoint3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            body_a: NodeID::nil(),
            body_b: NodeID::nil(),
            anchor_a: Vector3::ZERO,
            anchor_b: Vector3::ZERO,
            enabled: true,
            collide_connected: false,
        }
    }
}

impl Deref for FixedJoint3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for FixedJoint3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
