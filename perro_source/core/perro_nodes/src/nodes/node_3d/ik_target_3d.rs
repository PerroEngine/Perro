use crate::node_3d::Node3D;
use perro_ids::NodeID;
use std::ops::{Deref, DerefMut};

impl Deref for IKTarget3D {
    type Target = Node3D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for IKTarget3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct IKTarget3D {
    pub base: Node3D,
    pub skeleton: NodeID,
    pub bone_index: i32,
    pub chain_length: u32,
    pub iterations: u32,
    pub tolerance: f32,
    pub weight: f32,
    pub match_rotation: bool,
}

impl Default for IKTarget3D {
    fn default() -> Self {
        Self::new()
    }
}

impl IKTarget3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            skeleton: NodeID::nil(),
            bone_index: -1,
            chain_length: 2,
            iterations: 8,
            tolerance: 0.01,
            weight: 1.0,
            match_rotation: true,
        }
    }
}
