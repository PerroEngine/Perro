use crate::node_3d::Node3D;
use perro_ids::TerrainID;
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

impl Deref for TerrainInstance3D {
    type Target = Node3D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for TerrainInstance3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug, Default)]
pub struct TerrainInstance3D {
    pub base: Node3D,
    pub terrain: TerrainID,
    pub terrain_source: Option<Cow<'static, str>>,
    pub show_debug_vertices: bool,
    pub show_debug_edges: bool,
}

impl TerrainInstance3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            terrain: TerrainID::nil(),
            terrain_source: None,
            show_debug_vertices: false,
            show_debug_edges: false,
        }
    }
}
