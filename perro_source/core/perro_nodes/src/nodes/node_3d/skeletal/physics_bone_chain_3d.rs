use crate::node_3d::Node3D;
use perro_ids::NodeID;
use perro_structs::Vector3;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug)]
pub struct PhysicsBoneChain3D {
    pub base: Node3D,
    pub skeleton: NodeID,
    pub bone_index: i32,
    pub chain_length: u32,
    pub enabled: bool,
    pub gravity: Vector3,
    pub damping: f32,
    pub stiffness: f32,
    pub radius: f32,
    pub collisions: bool,
    pub iterations: u32,
    pub internal_bones: Vec<usize>,
    pub internal_positions: Vec<Vector3>,
    pub internal_prev_positions: Vec<Vector3>,
    pub internal_rest_world: Vec<Vector3>,
    pub internal_lengths: Vec<f32>,
    pub internal_local_positions: Vec<Vector3>,
}

impl Default for PhysicsBoneChain3D {
    fn default() -> Self {
        Self::new()
    }
}

impl PhysicsBoneChain3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            skeleton: NodeID::nil(),
            bone_index: -1,
            chain_length: 4,
            enabled: true,
            gravity: Vector3::new(0.0, -9.81, 0.0),
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

impl Deref for PhysicsBoneChain3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for PhysicsBoneChain3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
