use perro_ids::{MaterialID, MeshID, TextureID};
use std::collections::HashMap;

#[derive(Default)]
struct SlotArena {
    generations: Vec<u32>,
    occupied: Vec<bool>,
    free_slots: Vec<usize>,
}

impl SlotArena {
    #[inline]
    fn create_parts(&mut self) -> (u32, u32) {
        if let Some(slot) = self.free_slots.pop() {
            self.generations[slot] = self.generations[slot].wrapping_add(1);
            self.occupied[slot] = true;
            return ((slot + 1) as u32, self.generations[slot]);
        }

        let slot = self.generations.len();
        self.generations.push(0);
        self.occupied.push(true);
        ((slot + 1) as u32, 0)
    }

    #[inline]
    fn contains_parts(&self, index: u32, generation: u32) -> bool {
        if index == 0 {
            return false;
        }
        let idx = index as usize;
        if idx == 0 {
            return false;
        }
        let slot = idx - 1;
        self.occupied.get(slot).copied().unwrap_or(false)
            && self.generations.get(slot).copied() == Some(generation)
    }

    #[cfg(test)]
    #[inline]
    fn remove_parts(&mut self, index: u32, generation: u32) -> bool {
        if !self.contains_parts(index, generation) {
            return false;
        }
        let slot = index as usize - 1;
        self.occupied[slot] = false;
        self.free_slots.push(slot);
        true
    }
}

#[derive(Default)]
pub struct ResourceStore {
    meshes: SlotArena,
    textures: SlotArena,
    materials: SlotArena,
    mesh_by_source: HashMap<String, MeshID>,
    texture_by_source: HashMap<String, TextureID>,
}

impl ResourceStore {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn create_mesh(&mut self, source: &str) -> MeshID {
        if let Some(id) = self.mesh_by_source.get(source).copied() {
            return id;
        }
        let (index, generation) = self.meshes.create_parts();
        let id = MeshID::from_parts(index, generation);
        self.mesh_by_source.insert(source.to_string(), id);
        id
    }

    #[inline]
    pub fn create_texture(&mut self, source: &str) -> TextureID {
        if let Some(id) = self.texture_by_source.get(source).copied() {
            return id;
        }
        let (index, generation) = self.textures.create_parts();
        let id = TextureID::from_parts(index, generation);
        self.texture_by_source.insert(source.to_string(), id);
        id
    }

    #[inline]
    pub fn create_material(&mut self) -> MaterialID {
        let (index, generation) = self.materials.create_parts();
        MaterialID::from_parts(index, generation)
    }

    #[inline]
    pub fn has_texture(&self, id: TextureID) -> bool {
        self.textures.contains_parts(id.index(), id.generation())
    }

    #[inline]
    pub fn has_mesh(&self, id: MeshID) -> bool {
        self.meshes.contains_parts(id.index(), id.generation())
    }

    #[inline]
    pub fn has_material(&self, id: MaterialID) -> bool {
        self.materials.contains_parts(id.index(), id.generation())
    }

    #[cfg(test)]
    #[inline]
    fn remove_texture_for_test(&mut self, id: TextureID) -> bool {
        self.textures.remove_parts(id.index(), id.generation())
    }
}

#[cfg(test)]
mod tests {
    use super::ResourceStore;

    #[test]
    fn texture_slot_reuse_bumps_generation() {
        let mut store = ResourceStore::new();
        let first = store.create_texture("__tmp_a__");
        assert!(store.has_texture(first));
        assert!(store.remove_texture_for_test(first));
        assert!(!store.has_texture(first));

        let second = store.create_texture("__tmp_b__");
        assert_eq!(first.index(), second.index());
        assert_ne!(first.generation(), second.generation());
        assert!(!store.has_texture(first));
        assert!(store.has_texture(second));
    }
}
