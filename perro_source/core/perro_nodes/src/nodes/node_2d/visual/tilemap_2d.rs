use crate::node_2d::Node2D;
use perro_ids::TileSetRef;
use perro_structs::BitMask;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug)]
pub struct TileMap2D {
    pub base: Node2D,
    pub tileset: TileSetRef,
    pub width: u32,
    pub height: u32,
    pub empty_tile: i32,
    pub tiles: Vec<i32>,
    pub collision_enabled: bool,
    pub collision_layers: BitMask,
    pub collision_mask: BitMask,
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
            tileset: TileSetRef::empty(),
            width: 0,
            height: 0,
            empty_tile: -1,
            tiles: Vec::new(),
            collision_enabled: false,
            collision_layers: BitMask::ALL,
            collision_mask: BitMask::NONE,
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
