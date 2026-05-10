use crate::node_3d::Node3D;
use perro_structs::IKTargetParams;
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
    pub params: IKTargetParams,
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
            params: IKTargetParams::new(),
        }
    }
}
