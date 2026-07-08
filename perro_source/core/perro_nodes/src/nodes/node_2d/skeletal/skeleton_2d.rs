use crate::node_2d::Node2D;
use perro_structs::Transform2D;
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

// Keep old `skeleton_2d::*` imports valid while 2D rig helpers live in
// separate files beside the skeleton data.
pub use crate::node_2d::skeletal::bone_attachment_2d::BoneAttachment2D;
pub use crate::node_2d::skeletal::bone_collider_2d::BoneCollider2D;
pub use crate::node_2d::skeletal::ik_target_2d::IKTarget2D;
pub use crate::node_2d::skeletal::physics_bone_chain_2d::PhysicsBoneChain2D;

/// Bone data shared by 2D skeleton nodes.
#[derive(Clone, Debug, Default)]
pub struct Bone2D {
    pub name: Cow<'static, str>,
    pub parent: i32,
    pub rest: Transform2D,
    pub pose: Transform2D,
    pub inv_bind: Transform2D,
}

impl Bone2D {
    pub const fn new() -> Self {
        Self {
            name: Cow::Borrowed("Bone"),
            parent: -1,
            rest: Transform2D::IDENTITY,
            pose: Transform2D::IDENTITY,
            inv_bind: Transform2D::IDENTITY,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Skeleton2D {
    pub base: Node2D,
    pub bones: Vec<Bone2D>,
}

impl Skeleton2D {
    #[deprecated(note = "use Skeleton2D::default()")]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bone_name(&self, index: usize) -> Option<&str> {
        self.bones.get(index).map(|bone| bone.name.as_ref())
    }

    pub fn bone_index<S: AsRef<str>>(&self, name: S) -> Option<usize> {
        let name = name.as_ref();
        self.bones
            .iter()
            .position(|bone| bone.name.as_ref() == name)
    }
}

impl Deref for Skeleton2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Skeleton2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
