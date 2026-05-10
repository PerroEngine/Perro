use crate::node_2d::Node2D;
use perro_ids::NodeID;
use std::ops::{Deref, DerefMut};

/// Attach a 2D node to one skeleton bone.
#[derive(Clone, Debug, Default)]
pub struct BoneAttachment2D {
    pub base: Node2D,
    pub skeleton: NodeID,
    pub bone_index: i32,
}

impl BoneAttachment2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            skeleton: NodeID::nil(),
            bone_index: -1,
        }
    }
}

impl Deref for BoneAttachment2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for BoneAttachment2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
