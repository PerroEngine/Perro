use crate::Transform2D;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug, Default)]
pub struct Node2D {
    pub transform: Transform2D,
    pub z_index: i32,
    pub visible: bool,
}

impl Node2D {
    pub const fn new() -> Self {
        Self {
            transform: Transform2D::IDENTITY,
            visible: true,
            z_index: 0,
        }
    }
}

impl Deref for Node2D {
    type Target = Transform2D;

    fn deref(&self) -> &Self::Target {
        &self.transform
    }
}

impl DerefMut for Node2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.transform
    }
}
