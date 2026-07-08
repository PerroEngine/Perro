use crate::node_2d::Node2D;
use perro_ids::NodeID;
use perro_structs::Vector2;
use std::ops::{Deref, DerefMut};

/// Verlet-style secondary motion chain for 2D skeleton bones.
#[derive(Clone, Debug)]
pub struct PhysicsBoneChain2D {
    pub base: Node2D,
    pub skeleton: NodeID,
    pub bone_index: i32,
    pub chain_length: u32,
    pub enabled: bool,
    pub gravity: Vector2,
    pub damping: f32,
    pub stiffness: f32,
    pub radius: f32,
    pub collisions: bool,
    pub iterations: u32,
    #[doc(hidden)]
    pub internal_bones: Vec<usize>,
    #[doc(hidden)]
    pub internal_positions: Vec<Vector2>,
    #[doc(hidden)]
    pub internal_prev_positions: Vec<Vector2>,
    #[doc(hidden)]
    pub internal_rest_world: Vec<Vector2>,
    #[doc(hidden)]
    pub internal_lengths: Vec<f32>,
    #[doc(hidden)]
    pub internal_local_positions: Vec<Vector2>,
}

impl Default for PhysicsBoneChain2D {
    fn default() -> Self {
        Self::new()
    }
}

impl PhysicsBoneChain2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            skeleton: NodeID::nil(),
            bone_index: -1,
            chain_length: 4,
            enabled: true,
            gravity: Vector2::new(0.0, -9.81),
            damping: 0.08,
            stiffness: 0.35,
            radius: 0.05,
            collisions: true,
            iterations: 3,
            internal_bones: Vec::new(),
            internal_positions: Vec::new(),
            internal_prev_positions: Vec::new(),
            internal_rest_world: Vec::new(),
            internal_lengths: Vec::new(),
            internal_local_positions: Vec::new(),
        }
    }
}

impl Deref for PhysicsBoneChain2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for PhysicsBoneChain2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
