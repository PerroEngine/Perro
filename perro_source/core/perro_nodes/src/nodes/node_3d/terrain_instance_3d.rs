use crate::node_3d::Node3D;
use perro_ids::TerrainID;
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
}

impl TerrainInstance3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            terrain: TerrainID::nil(),
        }
    }
}
