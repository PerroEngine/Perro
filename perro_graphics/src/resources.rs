use perro_ids::{MaterialID, MeshID, TextureID};
use perro_render_bridge::Material3D;
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

#[derive(Debug, Clone, Copy, Default)]
struct ResourceMeta {
    ref_count: u32,
    zero_ref_frames: u32,
    used_once: bool,
    reserved: bool,
}

#[derive(Default)]
pub struct ResourceStore {
    meshes: SlotArena,
    textures: SlotArena,
    materials: SlotArena,
    mesh_by_source: HashMap<String, MeshID>,
    mesh_source_by: HashMap<MeshID, String>,
    texture_by_source: HashMap<String, TextureID>,
    texture_source_by: HashMap<TextureID, String>,
    material_by: HashMap<MaterialID, Material3D>,
    material_by_source: HashMap<String, MaterialID>,
    material_source_by: HashMap<MaterialID, String>,
    mesh_meta_by: HashMap<MeshID, ResourceMeta>,
    texture_meta_by: HashMap<TextureID, ResourceMeta>,
    material_meta_by: HashMap<MaterialID, ResourceMeta>,
}

impl ResourceStore {
    pub const DEFAULT_ZERO_REF_TTL_FRAMES: u32 = 60;

    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn create_mesh(&mut self, source: &str, reserved: bool) -> MeshID {
        if let Some(id) = self.mesh_by_source.get(source).copied() {
            if reserved {
                self.set_mesh_reserved(id, true);
            }
            return id;
        }
        let (index, generation) = self.meshes.create_parts();
        let id = MeshID::from_parts(index, generation);
        self.mesh_by_source.insert(source.to_string(), id);
        self.mesh_source_by.insert(id, source.to_string());
        self.mesh_meta_by.insert(
            id,
            ResourceMeta {
                reserved,
                ..ResourceMeta::default()
            },
        );
        id
    }

    #[inline]
    pub fn create_texture(&mut self, source: &str, reserved: bool) -> TextureID {
        if let Some(id) = self.texture_by_source.get(source).copied() {
            if reserved {
                self.set_texture_reserved(id, true);
            }
            return id;
        }
        let (index, generation) = self.textures.create_parts();
        let id = TextureID::from_parts(index, generation);
        self.texture_by_source.insert(source.to_string(), id);
        self.texture_source_by.insert(id, source.to_string());
        self.texture_meta_by.insert(
            id,
            ResourceMeta {
                reserved,
                ..ResourceMeta::default()
            },
        );
        id
    }

    #[inline]
    pub fn create_material(
        &mut self,
        material: Material3D,
        source: Option<&str>,
        reserved: bool,
    ) -> MaterialID {
        if let Some(source) = source
            && let Some(id) = self.material_by_source.get(source).copied()
        {
            if reserved {
                self.set_material_reserved(id, true);
            }
            return id;
        }
        let (index, generation) = self.materials.create_parts();
        let id = MaterialID::from_parts(index, generation);
        self.material_by.insert(id, material);
        if let Some(source) = source {
            let source = source.to_string();
            self.material_by_source.insert(source.clone(), id);
            self.material_source_by.insert(id, source);
        }
        self.material_meta_by.insert(
            id,
            ResourceMeta {
                reserved,
                ..ResourceMeta::default()
            },
        );
        id
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
    pub fn texture_source(&self, id: TextureID) -> Option<&str> {
        self.texture_source_by.get(&id).map(String::as_str)
    }

    #[inline]
    pub fn mesh_source(&self, id: MeshID) -> Option<&str> {
        self.mesh_source_by.get(&id).map(String::as_str)
    }

    #[inline]
    pub fn has_mesh_source(&self, source: &str) -> bool {
        self.mesh_by_source.contains_key(source)
    }

    #[inline]
    pub fn has_material(&self, id: MaterialID) -> bool {
        self.materials.contains_parts(id.index(), id.generation())
    }

    #[inline]
    pub fn material(&self, id: MaterialID) -> Option<Material3D> {
        self.material_by.get(&id).copied()
    }

    #[inline]
    pub fn reset_ref_counts(&mut self) {
        for meta in self.texture_meta_by.values_mut() {
            meta.ref_count = 0;
        }
        for meta in self.mesh_meta_by.values_mut() {
            meta.ref_count = 0;
        }
        for meta in self.material_meta_by.values_mut() {
            meta.ref_count = 0;
        }
    }

    #[inline]
    pub fn mark_texture_used(&mut self, id: TextureID) {
        if let Some(meta) = self.texture_meta_by.get_mut(&id) {
            meta.ref_count = meta.ref_count.saturating_add(1);
            meta.used_once = true;
            meta.zero_ref_frames = 0;
        }
    }

    #[inline]
    pub fn mark_mesh_used(&mut self, id: MeshID) {
        if let Some(meta) = self.mesh_meta_by.get_mut(&id) {
            meta.ref_count = meta.ref_count.saturating_add(1);
            meta.used_once = true;
            meta.zero_ref_frames = 0;
        }
    }

    #[inline]
    pub fn mark_material_used(&mut self, id: MaterialID) {
        if let Some(meta) = self.material_meta_by.get_mut(&id) {
            meta.ref_count = meta.ref_count.saturating_add(1);
            meta.used_once = true;
            meta.zero_ref_frames = 0;
        }
    }

    pub fn gc_unused(&mut self, ttl_frames: u32) {
        let ttl_frames = ttl_frames.max(1);

        let mut drop_textures = Vec::new();
        for (id, meta) in &mut self.texture_meta_by {
            if meta.ref_count > 0 || meta.reserved || !meta.used_once {
                continue;
            }
            meta.zero_ref_frames = meta.zero_ref_frames.saturating_add(1);
            if meta.zero_ref_frames >= ttl_frames {
                drop_textures.push(*id);
            }
        }
        for id in drop_textures {
            let _ = self.drop_texture(id);
        }

        let mut drop_meshes = Vec::new();
        for (id, meta) in &mut self.mesh_meta_by {
            if meta.ref_count > 0 || meta.reserved || !meta.used_once {
                continue;
            }
            meta.zero_ref_frames = meta.zero_ref_frames.saturating_add(1);
            if meta.zero_ref_frames >= ttl_frames {
                drop_meshes.push(*id);
            }
        }
        for id in drop_meshes {
            let _ = self.drop_mesh(id);
        }

        let mut drop_materials = Vec::new();
        for (id, meta) in &mut self.material_meta_by {
            if meta.ref_count > 0 || meta.reserved || !meta.used_once {
                continue;
            }
            meta.zero_ref_frames = meta.zero_ref_frames.saturating_add(1);
            if meta.zero_ref_frames >= ttl_frames {
                drop_materials.push(*id);
            }
        }
        for id in drop_materials {
            let _ = self.drop_material(id);
        }
    }

    #[inline]
    pub fn set_texture_reserved(&mut self, id: TextureID, reserved: bool) -> bool {
        if let Some(meta) = self.texture_meta_by.get_mut(&id) {
            meta.reserved = reserved;
            if reserved {
                meta.zero_ref_frames = 0;
            }
            return true;
        }
        false
    }

    #[inline]
    pub fn set_mesh_reserved(&mut self, id: MeshID, reserved: bool) -> bool {
        if let Some(meta) = self.mesh_meta_by.get_mut(&id) {
            meta.reserved = reserved;
            if reserved {
                meta.zero_ref_frames = 0;
            }
            return true;
        }
        false
    }

    #[inline]
    pub fn set_material_reserved(&mut self, id: MaterialID, reserved: bool) -> bool {
        if let Some(meta) = self.material_meta_by.get_mut(&id) {
            meta.reserved = reserved;
            if reserved {
                meta.zero_ref_frames = 0;
            }
            return true;
        }
        false
    }

    pub fn drop_texture(&mut self, id: TextureID) -> bool {
        if !self.textures.remove_parts(id.index(), id.generation()) {
            return false;
        }
        if let Some(source) = self.texture_source_by.remove(&id) {
            self.texture_by_source.remove(&source);
        }
        self.texture_meta_by.remove(&id);
        true
    }

    pub fn drop_mesh(&mut self, id: MeshID) -> bool {
        if !self.meshes.remove_parts(id.index(), id.generation()) {
            return false;
        }
        if let Some(source) = self.mesh_source_by.remove(&id) {
            self.mesh_by_source.remove(&source);
        }
        self.mesh_meta_by.remove(&id);
        true
    }

    pub fn drop_material(&mut self, id: MaterialID) -> bool {
        if !self.materials.remove_parts(id.index(), id.generation()) {
            return false;
        }
        self.material_by.remove(&id);
        if let Some(source) = self.material_source_by.remove(&id) {
            self.material_by_source.remove(&source);
        }
        self.material_meta_by.remove(&id);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::ResourceStore;
    use perro_render_bridge::Material3D;

    #[test]
    fn texture_slot_reuse_bumps_generation() {
        let mut store = ResourceStore::new();
        let first = store.create_texture("__tmp_a__", false);
        assert!(store.has_texture(first));
        assert!(store.drop_texture(first));
        assert!(!store.has_texture(first));

        let second = store.create_texture("__tmp_b__", false);
        assert_eq!(first.index(), second.index());
        assert_ne!(first.generation(), second.generation());
        assert!(!store.has_texture(first));
        assert!(store.has_texture(second));
    }

    #[test]
    fn material_source_reuses_existing() {
        let mut store = ResourceStore::new();
        let mat = Material3D::default();
        let first = store.create_material(mat, Some("res://materials/base.pmat"), false);
        let second = store.create_material(
            Material3D {
                roughness_factor: 1.0,
                ..Material3D::default()
            },
            Some("res://materials/base.pmat"),
            false,
        );
        assert_eq!(first, second);
    }

    #[test]
    fn loaded_texture_is_not_dropped_before_first_use() {
        let mut store = ResourceStore::new();
        let id = store.create_texture("__tmp_a__", false);
        for _ in 0..120 {
            store.reset_ref_counts();
            store.gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES);
        }
        assert!(store.has_texture(id));
    }

    #[test]
    fn used_texture_drops_after_ttl_when_unreferenced() {
        let mut store = ResourceStore::new();
        let id = store.create_texture("__tmp_a__", false);
        store.reset_ref_counts();
        store.mark_texture_used(id);
        store.gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES);
        assert!(store.has_texture(id));

        for _ in 0..ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES {
            store.reset_ref_counts();
            store.gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES);
        }
        assert!(!store.has_texture(id));
    }

    #[test]
    fn reserved_texture_is_not_auto_dropped() {
        let mut store = ResourceStore::new();
        let id = store.create_texture("__tmp_a__", true);
        store.reset_ref_counts();
        store.mark_texture_used(id);
        store.gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES);
        for _ in 0..(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES * 2) {
            store.reset_ref_counts();
            store.gc_unused(ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES);
        }
        assert!(store.has_texture(id));
    }
}
