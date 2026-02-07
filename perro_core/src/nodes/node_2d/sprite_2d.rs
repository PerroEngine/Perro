use std::{
    borrow::Cow,
    ops::{Deref, DerefMut},
};

use crate::{TextureID, node_2d::node_2d::Node2D};

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
    pub texture_path: Option<Cow<'static, str>>,
    pub texture_id: Option<TextureID>,
}

impl Sprite2D {
    pub fn new() -> Self {
        Self {
            base: Node2D::new(),
            texture_path: None,
            texture_id: None,
        }
    }
}
