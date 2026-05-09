use crate::node_2d::Node2D;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug)]
pub struct TileMap2D {
    pub base: Node2D,
    pub tileset: String,
    pub width: u32,
    pub height: u32,
    pub empty_tile: i32,
    pub tiles: Vec<i32>,
    pub collision_enabled: bool,
    pub collision_layer: u32,
    pub collision_mask: u32,
}

impl Default for TileMap2D {
    fn default() -> Self {
        Self::new()
    }
}

impl TileMap2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            tileset: String::new(),
            width: 0,
            height: 0,
            empty_tile: -1,
            tiles: Vec::new(),
            collision_enabled: false,
            collision_layer: 1,
            collision_mask: u32::MAX,
        }
    }
}

impl Deref for TileMap2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TileMap2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
