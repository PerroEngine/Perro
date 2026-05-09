use crate::node_2d::Node2D;
use perro_structs::Transform2D;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug, Default)]
pub struct Bone2D {
    pub base: Node2D,
    pub rest: Transform2D,
    pub pose: Transform2D,
    pub inv_bind: Transform2D,
}

impl Bone2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            rest: Transform2D::IDENTITY,
            pose: Transform2D::IDENTITY,
            inv_bind: Transform2D::IDENTITY,
        }
    }
}

impl Deref for Bone2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Bone2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug, Default)]
pub struct Skeleton2D {
    pub base: Node2D,
}

impl Skeleton2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
        }
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
