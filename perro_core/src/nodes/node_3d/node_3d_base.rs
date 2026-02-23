use crate::Transform3D;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug, Default)]
pub struct Node3D {
    pub transform: Transform3D,
    pub visible: bool,
}

impl Node3D {
    pub const fn new() -> Self {
        Self {
            transform: Transform3D::IDENTITY,
            visible: true,
        }
    }
}

impl Deref for Node3D {
    type Target = Transform3D;

    fn deref(&self) -> &Self::Target {
        &self.transform
    }
}

impl DerefMut for Node3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.transform
    }
}
