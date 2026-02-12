use perro_ids::{MaterialID, MeshID, TextureID};
use std::collections::HashSet;

#[derive(Default)]
pub struct ResourceStore {
    next_mesh_index: u32,
    next_texture_index: u32,
    next_material_index: u32,
    live_meshes: HashSet<MeshID>,
    live_textures: HashSet<TextureID>,
    live_materials: HashSet<MaterialID>,
}

impl ResourceStore {
    pub fn new() -> Self {
        Self {
            next_mesh_index: 1,
            next_texture_index: 1,
            next_material_index: 1,
            live_meshes: HashSet::new(),
            live_textures: HashSet::new(),
            live_materials: HashSet::new(),
        }
    }

    pub fn create_mesh(&mut self) -> MeshID {
        let id = MeshID::from_parts(self.next_mesh_index, 0);
        self.next_mesh_index = self.next_mesh_index.saturating_add(1);
        self.live_meshes.insert(id);
        id
    }

    pub fn create_texture(&mut self) -> TextureID {
        let id = TextureID::from_parts(self.next_texture_index, 0);
        self.next_texture_index = self.next_texture_index.saturating_add(1);
        self.live_textures.insert(id);
        id
    }

    pub fn create_material(&mut self) -> MaterialID {
        let id = MaterialID::from_parts(self.next_material_index, 0);
        self.next_material_index = self.next_material_index.saturating_add(1);
        self.live_materials.insert(id);
        id
    }
    
    #[inline]
    pub fn has_texture(&self, id: TextureID) -> bool {
        self.live_textures.contains(&id)
    }
}
