use crate::node_3d::Node3D;
use perro_structs::Transform3D;
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug, Default)]
pub struct Bone3D {
    pub name: Cow<'static, str>,
    pub parent: i32,
    pub rest: Transform3D,
    pub inv_bind: Transform3D,
}

impl Bone3D {
    pub const fn new() -> Self {
        Self {
            name: Cow::Borrowed("Bone"),
            parent: -1,
            rest: Transform3D::IDENTITY,
            inv_bind: Transform3D::IDENTITY,
        }
    }
}

impl Deref for Skeleton3D {
    type Target = Node3D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Skeleton3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug, Default)]
pub struct Skeleton3D {
    pub base: Node3D,
    pub bones: Vec<Bone3D>,
}

impl Skeleton3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            bones: Vec::new(),
        }
    }
}
