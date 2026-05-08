use crate::node_3d::Node3D;
use perro_ids::NodeID;
use std::ops::{Deref, DerefMut};

pub type BoneIndex = i32;

#[derive(Clone, Debug)]
pub struct BoneAttachment3D {
    pub base: Node3D,
    pub skeleton: Option<NodeID>,
    pub bone_index: BoneIndex,
    pub enabled: bool,
}

impl BoneAttachment3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            skeleton: None,
            bone_index: -1,
            enabled: true,
        }
    }

    pub fn set_skeleton(&mut self, skeleton: Option<NodeID>) {
        self.skeleton = skeleton;
    }

    pub fn skeleton(&self) -> Option<NodeID> {
        self.skeleton
    }

    pub fn set_bone_index(&mut self, bone_index: BoneIndex) {
        self.bone_index = bone_index;
    }

    pub fn bone_index(&self) -> BoneIndex {
        self.bone_index
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for BoneAttachment3D {
    fn default() -> Self {
        Self::new()
    }
}

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
