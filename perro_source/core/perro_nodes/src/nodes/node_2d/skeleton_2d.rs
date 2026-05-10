use crate::node_2d::Node2D;
use perro_ids::NodeID;
use perro_structs::{IKTargetParams, Transform2D};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

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
    pub fn new() -> Self {
        Self {
            base: Node2D::new(),
            bones: Vec::new(),
        }
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

#[derive(Clone, Debug)]
pub struct IKTarget2D {
    pub base: Node2D,
    pub params: IKTargetParams,
}

impl Default for IKTarget2D {
    fn default() -> Self {
        Self::new()
    }
}

impl IKTarget2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            params: IKTargetParams::new(),
        }
    }
}

impl Deref for IKTarget2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for IKTarget2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct PhysicsBoneChain2D {
    pub base: Node2D,
    pub skeleton: NodeID,
    pub bone_index: i32,
    pub chain_length: u32,
    pub enabled: bool,
    pub gravity: perro_structs::Vector2,
    pub damping: f32,
    pub stiffness: f32,
    pub radius: f32,
    pub collisions: bool,
    pub iterations: u32,
    pub internal_bones: Vec<usize>,
    pub internal_positions: Vec<perro_structs::Vector2>,
    pub internal_prev_positions: Vec<perro_structs::Vector2>,
    pub internal_rest_world: Vec<perro_structs::Vector2>,
    pub internal_lengths: Vec<f32>,
    pub internal_local_positions: Vec<perro_structs::Vector2>,
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
            gravity: perro_structs::Vector2::new(0.0, -9.81),
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

#[derive(Clone, Debug)]
pub struct BoneCollider2D {
    pub base: Node2D,
    pub enabled: bool,
}

impl Default for BoneCollider2D {
    fn default() -> Self {
        Self::new()
    }
}

impl BoneCollider2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            enabled: true,
        }
    }
}

impl Deref for BoneCollider2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for BoneCollider2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
