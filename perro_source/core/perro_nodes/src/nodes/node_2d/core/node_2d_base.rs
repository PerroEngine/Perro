use perro_structs::{BitMask, NodeModulate, Transform2D};
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug)]
pub struct Node2D {
    pub transform: Transform2D,
    pub top_level: bool,
    pub z_index: i32,
    pub visible: bool,
    pub render_layers: BitMask,
    pub modulate: NodeModulate,
}

impl Node2D {
    pub const fn new() -> Self {
        Self {
            transform: Transform2D::IDENTITY,
            top_level: false,
            visible: true,
            z_index: 0,
            render_layers: BitMask::ALL,
            modulate: NodeModulate::WHITE,
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

impl Default for Node2D {
    fn default() -> Self {
        Self::new()
    }
}
