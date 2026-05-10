use crate::node_2d::Node2D;
use perro_structs::IKTargetParams;
use std::ops::{Deref, DerefMut};

/// IK goal for a 2D skeleton chain.
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
