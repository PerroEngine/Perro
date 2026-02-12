use std::ops::{Deref, DerefMut};

use crate::node_2d::node_2d::Node2D;
use perro_ids::TextureID;

impl Deref for Sprite2D {
    type Target = Node2D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Sprite2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug, Default)]
pub struct Sprite2D {
    pub base: Node2D,
    pub texture_id: TextureID,
}

impl Sprite2D {
    pub fn new() -> Self {
        Self {
            base: Node2D::new(),
            texture_id: TextureID::nil(),
        }
    }
}
