use crate::node_3d::Node3D;
use perro_ids::NodeID;
use std::ops::{Deref, DerefMut};

impl Deref for BoneAttachment3D {
    type Target = Node3D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for BoneAttachment3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug, Default)]
pub struct BoneAttachment3D {
    pub base: Node3D,
    pub skeleton: NodeID,
    pub bone_index: i32,
}

impl BoneAttachment3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            skeleton: NodeID::nil(),
            bone_index: -1,
        }
    }
}
