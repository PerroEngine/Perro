use ahash::AHashMap;
use perro_ids::{MaterialID, MeshID, TextureID};
use perro_render_bridge::{Material3D, Mesh3D};

#[derive(Debug, Clone)]
pub(crate) struct DecodedTextureRgba {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

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

    #[inline]
    fn occupy_parts(&mut self, index: u32, generation: u32) -> bool {
        if index == 0 {
            return false;
        }
        let slot = index as usize - 1;
        if self.generations.len() <= slot {
            let start = self.generations.len();
            self.generations.resize(slot + 1, 0);
            self.occupied.resize(slot + 1, false);
            for s in start..slot {
                self.free_slots.push(s);
            }
        }
        if self.occupied[slot] {
            return self.generations[slot] == generation;
        }
        if self.generations[slot] > generation {
            return false;
        }
        self.generations[slot] = generation;
        self.occupied[slot] = true;
        if let Some(pos) = self.free_slots.iter().position(|s| *s == slot) {
            self.free_slots.swap_remove(pos);
        }
        true
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ResourceMeta {
    ref_count: u32,
    zero_ref_frames: u32,
    used_once: bool,
    reserved: bool,
    gc_queued: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ResourceGcDrops {
    pub textures: Vec<TextureID>,
    pub meshes: Vec<MeshID>,
    pub materials: Vec<MaterialID>,
}

#[derive(Default)]
pub struct ResourceStore {
    meshes: SlotArena,
    textures: SlotArena,
    materials: SlotArena,
    mesh_by_source: AHashMap<String, MeshID>,
    mesh_source_by: AHashMap<MeshID, String>,
    mesh_source_by_slot: Vec<Option<String>>,
    runtime_mesh_by_source: AHashMap<String, Mesh3D>,
    runtime_mesh_by_id: AHashMap<MeshID, Mesh3D>,
    mesh_revision_by_id: AHashMap<MeshID, u64>,
    texture_by_source: AHashMap<String, TextureID>,
    texture_source_by: AHashMap<TextureID, String>,
    texture_source_by_slot: Vec<Option<String>>,
    decoded_texture_by_source: AHashMap<String, DecodedTextureRgba>,
    decoded_texture_by_id: AHashMap<TextureID, DecodedTextureRgba>,
    material_by: AHashMap<MaterialID, Material3D>,
    material_by_source: AHashMap<String, MaterialID>,
    material_source_by: AHashMap<MaterialID, String>,
    mesh_meta_by: AHashMap<MeshID, ResourceMeta>,
    texture_meta_by: AHashMap<TextureID, ResourceMeta>,
    material_meta_by: AHashMap<MaterialID, ResourceMeta>,
    mesh_meta_by_slot: Vec<Option<ResourceMeta>>,
    texture_meta_by_slot: Vec<Option<ResourceMeta>>,
    material_meta_by_slot: Vec<Option<ResourceMeta>>,
    mesh_gc_candidates: Vec<MeshID>,
    texture_gc_candidates: Vec<TextureID>,
    material_gc_candidates: Vec<MaterialID>,
    mesh_ref_ids: Vec<MeshID>,
    texture_ref_ids: Vec<TextureID>,
    material_ref_ids: Vec<MaterialID>,
}

impl ResourceStore {
    pub const DEFAULT_ZERO_REF_TTL_FRAMES: u32 = 60;

    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn active_mesh_count(&self) -> usize {
        self.mesh_meta_by.len()
    }

    #[inline]
    pub fn active_material_count(&self) -> usize {
        self.material_meta_by.len()
    }

    #[inline]
    pub fn active_texture_count(&self) -> usize {
        self.texture_meta_by.len()
    }

    #[cfg(test)]
    pub(crate) fn texture_ref_count(&self, id: TextureID) -> u32 {
        self.texture_meta_by
            .get(&id)
            .map(|meta| meta.ref_count)
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub(crate) fn mesh_ref_count(&self, id: MeshID) -> u32 {
        self.mesh_meta_by
            .get(&id)
            .map(|meta| meta.ref_count)
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub(crate) fn material_ref_count(&self, id: MaterialID) -> u32 {
        self.material_meta_by
            .get(&id)
            .map(|meta| meta.ref_count)
            .unwrap_or(0)
    }

    #[inline]
    fn set_mesh_source_slot(&mut self, index: u32, source: &str) {
        if index == 0 {
            return;
        }
        let slot = index as usize - 1;
        if self.mesh_source_by_slot.len() <= slot {
            self.mesh_source_by_slot.resize(slot + 1, None);
        }
        self.mesh_source_by_slot[slot] = Some(source.to_string());
    }

    #[inline]
    fn clear_mesh_source_slot(&mut self, index: u32) {
        if index == 0 {
            return;
        }
        let slot = index as usize - 1;
        if let Some(entry) = self.mesh_source_by_slot.get_mut(slot) {
            *entry = None;
        }
    }

    #[inline]
    fn clear_mesh_source_slot_if(&mut self, index: u32, source: &str) {
        if index == 0 {
            return;
        }
        let slot = index as usize - 1;
        if let Some(entry) = self.mesh_source_by_slot.get_mut(slot)
            && entry.as_deref() == Some(source)
        {
            *entry = None;
        }
    }

    #[inline]
    fn set_texture_source_slot(&mut self, index: u32, source: &str) {
        if index == 0 {
            return;
        }
        let slot = index as usize - 1;
        if self.texture_source_by_slot.len() <= slot {
            self.texture_source_by_slot.resize(slot + 1, None);
        }
        self.texture_source_by_slot[slot] = Some(source.to_string());
    }

    #[inline]
    fn clear_texture_source_slot(&mut self, index: u32) {
        if index == 0 {
            return;
        }
        let slot = index as usize - 1;
        if let Some(entry) = self.texture_source_by_slot.get_mut(slot) {
            *entry = None;
        }
    }

    #[inline]
    fn clear_texture_source_slot_if(&mut self, index: u32, source: &str) {
        if index == 0 {
            return;
        }
        let slot = index as usize - 1;
        if let Some(entry) = self.texture_source_by_slot.get_mut(slot)
            && entry.as_deref() == Some(source)
        {
            *entry = None;
        }
    }

    #[inline]
    fn set_mesh_meta(&mut self, id: MeshID, meta: ResourceMeta) {
        let slot = id.index() as usize - 1;
        if self.mesh_meta_by_slot.len() <= slot {
            self.mesh_meta_by_slot.resize(slot + 1, None);
        }
        self.mesh_meta_by_slot[slot] = Some(meta);
        self.mesh_meta_by.insert(id, meta);
    }

    #[inline]
    fn set_texture_meta(&mut self, id: TextureID, meta: ResourceMeta) {
        let slot = id.index() as usize - 1;
        if self.texture_meta_by_slot.len() <= slot {
            self.texture_meta_by_slot.resize(slot + 1, None);
        }
        self.texture_meta_by_slot[slot] = Some(meta);
        self.texture_meta_by.insert(id, meta);
    }

    #[inline]
    fn set_material_meta(&mut self, id: MaterialID, meta: ResourceMeta) {
        let slot = id.index() as usize - 1;
        if self.material_meta_by_slot.len() <= slot {
            self.material_meta_by_slot.resize(slot + 1, None);
        }
        self.material_meta_by_slot[slot] = Some(meta);
        self.material_meta_by.insert(id, meta);
    }

    #[inline]
    fn clear_mesh_meta(&mut self, id: MeshID) {
        if id.index() > 0 {
            let slot = id.index() as usize - 1;
            if let Some(meta) = self.mesh_meta_by_slot.get_mut(slot) {
                *meta = None;
            }
        }
        self.mesh_meta_by.remove(&id);
    }

    #[inline]
    fn clear_texture_meta(&mut self, id: TextureID) {
        if id.index() > 0 {
            let slot = id.index() as usize - 1;
            if let Some(meta) = self.texture_meta_by_slot.get_mut(slot) {
                *meta = None;
            }
        }
        self.texture_meta_by.remove(&id);
    }

    #[inline]
    fn clear_material_meta(&mut self, id: MaterialID) {
        if id.index() > 0 {
            let slot = id.index() as usize - 1;
            if let Some(meta) = self.material_meta_by_slot.get_mut(slot) {
                *meta = None;
            }
        }
        self.material_meta_by.remove(&id);
    }

    #[inline]
    fn mesh_meta_mut(&mut self, id: MeshID) -> Option<&mut ResourceMeta> {
        if !self.has_mesh(id) {
            return None;
        }
        self.mesh_meta_by_slot
            .get_mut(id.index() as usize - 1)
            .and_then(Option::as_mut)
    }

    #[inline]
    fn texture_meta_mut(&mut self, id: TextureID) -> Option<&mut ResourceMeta> {
        if !self.has_texture(id) {
            return None;
        }
        self.texture_meta_by_slot
            .get_mut(id.index() as usize - 1)
            .and_then(Option::as_mut)
    }

    #[inline]
    fn material_meta_mut(&mut self, id: MaterialID) -> Option<&mut ResourceMeta> {
        if !self.has_material(id) {
            return None;
        }
        self.material_meta_by_slot
            .get_mut(id.index() as usize - 1)
            .and_then(Option::as_mut)
    }

    #[inline]
    fn queue_texture_gc(&mut self, id: TextureID) {
        if let Some(meta) = self.texture_meta_mut(id)
            && meta.used_once
            && !meta.reserved
            && !meta.gc_queued
        {
            meta.gc_queued = true;
            let updated = *meta;
            self.texture_meta_by.insert(id, updated);
            self.texture_gc_candidates.push(id);
        }
    }

    #[inline]
    fn queue_mesh_gc(&mut self, id: MeshID) {
        if let Some(meta) = self.mesh_meta_mut(id)
            && meta.used_once
            && !meta.reserved
            && !meta.gc_queued
        {
            meta.gc_queued = true;
            let updated = *meta;
            self.mesh_meta_by.insert(id, updated);
            self.mesh_gc_candidates.push(id);
        }
    }

    #[inline]
    fn queue_material_gc(&mut self, id: MaterialID) {
        if let Some(meta) = self.material_meta_mut(id)
            && meta.used_once
            && !meta.reserved
            && !meta.gc_queued
        {
            meta.gc_queued = true;
            let updated = *meta;
            self.material_meta_by.insert(id, updated);
            self.material_gc_candidates.push(id);
        }
    }

    #[inline]
    fn purge_stale_mesh_source(&mut self, source: &str, id: MeshID) {
        self.mesh_by_source.remove(source);
        self.mesh_source_by.remove(&id);
        self.runtime_mesh_by_source.remove(source);
        self.runtime_mesh_by_id.remove(&id);
        self.mesh_revision_by_id.remove(&id);
        self.clear_mesh_meta(id);
        self.clear_mesh_source_slot_if(id.index(), source);
    }

    #[inline]
    fn purge_stale_texture_source(&mut self, source: &str, id: TextureID) {
        self.texture_by_source.remove(source);
        self.texture_source_by.remove(&id);
        self.decoded_texture_by_source.remove(source);
        self.decoded_texture_by_id.remove(&id);
        self.clear_texture_meta(id);
        self.clear_texture_source_slot_if(id.index(), source);
    }

    #[inline]
    fn purge_stale_material_source(&mut self, source: &str, id: MaterialID) {
        self.material_by_source.remove(source);
        self.material_source_by.remove(&id);
        self.material_by.remove(&id);
        self.clear_material_meta(id);
    }

    #[inline]
    pub fn create_mesh(&mut self, source: &str, reserved: bool) -> MeshID {
        if let Some(id) = self.mesh_by_source.get(source).copied() {
            if self.has_mesh(id) {
                if reserved {
                    self.set_mesh_reserved(id, true);
                }
                self.log_resource_reused("mesh", id.index(), id.generation(), source, reserved);
                return id;
            }
            self.purge_stale_mesh_source(source, id);
        }
        let (index, generation) = self.meshes.create_parts();
        let id = MeshID::from_parts(index, generation);
        self.mesh_by_source.insert(source.to_string(), id);
        self.mesh_source_by.insert(id, source.to_string());
        self.set_mesh_source_slot(index, source);
        self.set_mesh_meta(
            id,
            ResourceMeta {
                reserved,
                ..ResourceMeta::default()
            },
        );
        self.mesh_revision_by_id.insert(id, 0);
        self.log_resource_created("mesh", index, generation, source, reserved);
        id
    }

    #[inline]
    pub fn create_mesh_with_id(&mut self, id: MeshID, source: &str, reserved: bool) -> MeshID {
        if let Some(existing) = self.mesh_by_source.get(source).copied() {
            if self.has_mesh(existing) {
                if reserved {
                    self.set_mesh_reserved(existing, true);
                }
                self.log_resource_reused(
                    "mesh",
                    existing.index(),
                    existing.generation(),
                    source,
                    reserved,
                );
                return existing;
            }
            self.purge_stale_mesh_source(source, existing);
        }
        if !self.meshes.occupy_parts(id.index(), id.generation()) {
            // Requested slot already occupied; allocate a fresh slot instead of
            // returning nil, so source->mesh mapping stays valid.
            return self.create_mesh(source, reserved);
        }
        self.mesh_by_source.insert(source.to_string(), id);
        self.mesh_source_by.insert(id, source.to_string());
        self.set_mesh_source_slot(id.index(), source);
        self.set_mesh_meta(
            id,
            ResourceMeta {
                reserved,
                ..ResourceMeta::default()
            },
        );
        self.mesh_revision_by_id.insert(id, 0);
        self.log_resource_created("mesh", id.index(), id.generation(), source, reserved);
        id
    }

    #[inline]
    pub fn create_texture(&mut self, source: &str, reserved: bool) -> TextureID {
        if let Some(id) = self.texture_by_source.get(source).copied() {
            if self.has_texture(id) {
                if reserved {
                    self.set_texture_reserved(id, true);
                }
                self.log_resource_reused("texture", id.index(), id.generation(), source, reserved);
                return id;
            }
            self.purge_stale_texture_source(source, id);
        }
        let (index, generation) = self.textures.create_parts();
        let id = TextureID::from_parts(index, generation);
        self.texture_by_source.insert(source.to_string(), id);
        self.texture_source_by.insert(id, source.to_string());
        self.set_texture_source_slot(index, source);
        self.set_texture_meta(
            id,
            ResourceMeta {
                reserved,
                ..ResourceMeta::default()
            },
        );
        self.log_resource_created("texture", index, generation, source, reserved);
        id
    }

    #[inline]
    pub fn create_texture_with_id(
        &mut self,
        id: TextureID,
        source: &str,
        reserved: bool,
    ) -> TextureID {
        if let Some(existing) = self.texture_by_source.get(source).copied() {
            if self.has_texture(existing) {
                if reserved {
                    self.set_texture_reserved(existing, true);
                }
                self.log_resource_reused(
                    "texture",
                    existing.index(),
                    existing.generation(),
                    source,
                    reserved,
                );
                return existing;
            }
            self.purge_stale_texture_source(source, existing);
        }
        if !self.textures.occupy_parts(id.index(), id.generation()) {
            return self.create_texture(source, reserved);
        }
        self.texture_by_source.insert(source.to_string(), id);
        self.texture_source_by.insert(id, source.to_string());
        self.set_texture_source_slot(id.index(), source);
        self.set_texture_meta(
            id,
            ResourceMeta {
                reserved,
                ..ResourceMeta::default()
            },
        );
        self.log_resource_created("texture", id.index(), id.generation(), source, reserved);
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
            if self.has_material(id) {
                if reserved {
                    self.set_material_reserved(id, true);
                }
                self.log_resource_reused("material", id.index(), id.generation(), source, reserved);
                return id;
            }
            self.purge_stale_material_source(source, id);
        }
        let (index, generation) = self.materials.create_parts();
        let id = MaterialID::from_parts(index, generation);
        self.material_by.insert(id, material);
        if let Some(source) = source {
            let source = source.to_string();
            self.material_by_source.insert(source.clone(), id);
            self.material_source_by.insert(id, source);
        }
        self.set_material_meta(
            id,
            ResourceMeta {
                reserved,
                ..ResourceMeta::default()
            },
        );
        self.log_resource_created(
            "material",
            index,
            generation,
            source.unwrap_or("<inline>"),
            reserved,
        );
        id
    }

    #[inline]
    pub fn create_material_with_id(
        &mut self,
        id: MaterialID,
        material: Material3D,
        source: Option<&str>,
        reserved: bool,
    ) -> MaterialID {
        if let Some(source) = source
            && let Some(existing) = self.material_by_source.get(source).copied()
        {
            if self.has_material(existing) {
                if reserved {
                    self.set_material_reserved(existing, true);
                }
                self.log_resource_reused(
                    "material",
                    existing.index(),
                    existing.generation(),
                    source,
                    reserved,
                );
                return existing;
            }
            self.purge_stale_material_source(source, existing);
        }
        if !self.materials.occupy_parts(id.index(), id.generation()) {
            return self.create_material(material, source, reserved);
        }
        self.material_by.insert(id, material);
        if let Some(source) = source {
            let source = source.to_string();
            self.material_by_source.insert(source.clone(), id);
            self.material_source_by.insert(id, source);
        }
        self.set_material_meta(
            id,
            ResourceMeta {
                reserved,
                ..ResourceMeta::default()
            },
        );
        self.log_resource_created(
            "material",
            id.index(),
            id.generation(),
            source.unwrap_or("<inline>"),
            reserved,
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
        if !self.has_texture(id) {
            return None;
        }
        self.texture_source_by.get(&id).map(String::as_str)
    }

    #[inline]
    pub(crate) fn set_decoded_texture_data(
        &mut self,
        id: TextureID,
        texture: DecodedTextureRgba,
    ) -> bool {
        if !self.has_texture(id) {
            return false;
        }
        self.decoded_texture_by_id.insert(id, texture.clone());
        if let Some(source) = self.texture_source(id).map(str::to_string) {
            self.decoded_texture_by_source.insert(source, texture);
        }
        true
    }

    #[inline]
    pub(crate) fn decoded_texture_data(&self, id: TextureID) -> Option<&DecodedTextureRgba> {
        self.decoded_texture_by_id.get(&id)
    }

    #[inline]
    pub(crate) fn decoded_texture_data_by_source(
        &self,
        source: &str,
    ) -> Option<&DecodedTextureRgba> {
        self.decoded_texture_by_source.get(source)
    }

    #[inline]
    pub fn texture_source_by_index(&self, index: u32) -> Option<&str> {
        if index == 0 {
            return None;
        }
        let source = self
            .texture_source_by_slot
            .get(index as usize - 1)
            .and_then(|s| s.as_deref())?;
        let id = self.texture_by_source.get(source).copied()?;
        (id.index() == index && self.has_texture(id)).then_some(source)
    }

    #[inline]
    pub fn mesh_source(&self, id: MeshID) -> Option<&str> {
        if !self.has_mesh(id) {
            return None;
        }
        self.mesh_source_by.get(&id).map(String::as_str)
    }

    #[inline]
    pub fn has_mesh_source(&self, source: &str) -> bool {
        self.mesh_by_source
            .get(source)
            .copied()
            .is_some_and(|id| self.has_mesh(id))
    }

    #[inline]
    pub fn mesh_id_for_source(&self, source: &str) -> Option<MeshID> {
        self.mesh_by_source
            .get(source)
            .copied()
            .filter(|id| self.has_mesh(*id))
    }

    #[inline]
    pub fn has_texture_source(&self, source: &str) -> bool {
        self.texture_by_source
            .get(source)
            .copied()
            .is_some_and(|id| self.has_texture(id))
    }

    #[inline]
    pub fn set_runtime_mesh_data(&mut self, source: &str, mesh: Mesh3D) {
        self.runtime_mesh_by_source.insert(source.to_string(), mesh);
    }

    #[inline]
    pub fn runtime_mesh_data(&self, source: &str) -> Option<&Mesh3D> {
        self.runtime_mesh_by_source.get(source)
    }

    #[inline]
    pub fn set_runtime_mesh_data_by_id(&mut self, id: MeshID, mesh: Mesh3D) -> bool {
        if !self.has_mesh(id) {
            return false;
        }
        self.runtime_mesh_by_id.insert(id, mesh.clone());
        let entry = self.mesh_revision_by_id.entry(id).or_insert(0);
        *entry = entry.wrapping_add(1);
        if let Some(source) = self.mesh_source(id).map(str::to_string) {
            self.runtime_mesh_by_source.insert(source, mesh);
        }
        true
    }

    #[inline]
    pub fn runtime_mesh_data_by_id(&self, id: MeshID) -> Option<&Mesh3D> {
        self.runtime_mesh_by_id.get(&id)
    }

    #[inline]
    pub fn mesh_revision(&self, id: MeshID) -> u64 {
        self.mesh_revision_by_id.get(&id).copied().unwrap_or(0)
    }

    #[inline]
    pub fn set_material_data(&mut self, id: MaterialID, material: Material3D) -> bool {
        if !self.has_material(id) {
            return false;
        }
        self.material_by.insert(id, material);
        true
    }

    #[inline]
    pub fn has_material(&self, id: MaterialID) -> bool {
        self.materials.contains_parts(id.index(), id.generation())
    }

    #[inline]
    pub fn material_id_for_source(&self, source: &str) -> Option<MaterialID> {
        self.material_by_source
            .get(source)
            .copied()
            .filter(|id| self.has_material(*id))
    }

    #[inline]
    pub fn material(&self, id: MaterialID) -> Option<Material3D> {
        self.material_by.get(&id).cloned()
    }

    #[inline]
    pub fn reset_ref_counts(&mut self) {
        for id in std::mem::take(&mut self.texture_ref_ids) {
            if let Some(meta) = self.texture_meta_mut(id) {
                meta.ref_count = 0;
                let updated = *meta;
                self.texture_meta_by.insert(id, updated);
            }
        }
        for id in std::mem::take(&mut self.mesh_ref_ids) {
            if let Some(meta) = self.mesh_meta_mut(id) {
                meta.ref_count = 0;
                let updated = *meta;
                self.mesh_meta_by.insert(id, updated);
            }
        }
        for id in std::mem::take(&mut self.material_ref_ids) {
            if let Some(meta) = self.material_meta_mut(id) {
                meta.ref_count = 0;
                let updated = *meta;
                self.material_meta_by.insert(id, updated);
            }
        }
    }

    #[inline]
    pub fn mark_texture_used(&mut self, id: TextureID) {
        self.mark_texture_used_count(id, 1);
    }

    #[inline]
    pub fn mark_texture_used_count(&mut self, id: TextureID, count: u32) {
        if count == 0 {
            return;
        }
        let Some(meta) = self.texture_meta_mut(id) else {
            return;
        };
        let should_log = !meta.used_once;
        let should_queue = !meta.reserved && !meta.gc_queued;
        let should_track_ref = meta.ref_count == 0;
        let updated = {
            meta.ref_count = meta.ref_count.saturating_add(count);
            meta.used_once = true;
            meta.zero_ref_frames = 0;
            *meta
        };
        self.texture_meta_by.insert(id, updated);
        if should_track_ref {
            self.texture_ref_ids.push(id);
        }
        if should_queue {
            self.queue_texture_gc(id);
        }
        if should_log {
            let source = self
                .texture_source_by
                .get(&id)
                .map(String::as_str)
                .unwrap_or("<unknown>");
            self.log_used_once("texture", id.index(), id.generation(), source);
        }
    }

    #[inline]
    pub fn mark_mesh_used(&mut self, id: MeshID) {
        self.mark_mesh_used_count(id, 1);
    }

    #[inline]
    pub fn mark_mesh_used_count(&mut self, id: MeshID, count: u32) {
        if count == 0 {
            return;
        }
        let Some(meta) = self.mesh_meta_mut(id) else {
            return;
        };
        let should_log = !meta.used_once;
        let should_queue = !meta.reserved && !meta.gc_queued;
        let should_track_ref = meta.ref_count == 0;
        let updated = {
            meta.ref_count = meta.ref_count.saturating_add(count);
            meta.used_once = true;
            meta.zero_ref_frames = 0;
            *meta
        };
        self.mesh_meta_by.insert(id, updated);
        if should_track_ref {
            self.mesh_ref_ids.push(id);
        }
        if should_queue {
            self.queue_mesh_gc(id);
        }
        if should_log {
            let source = self
                .mesh_source_by
                .get(&id)
                .map(String::as_str)
                .unwrap_or("<unknown>");
            self.log_used_once("mesh", id.index(), id.generation(), source);
        }
    }

    #[inline]
    pub fn mark_material_used(&mut self, id: MaterialID) {
        self.mark_material_used_count(id, 1);
    }

    #[inline]
    pub fn mark_material_used_count(&mut self, id: MaterialID, count: u32) {
        if count == 0 {
            return;
        }
        let Some(meta) = self.material_meta_mut(id) else {
            return;
        };
        let should_log = !meta.used_once;
        let should_queue = !meta.reserved && !meta.gc_queued;
        let should_track_ref = meta.ref_count == 0;
        let updated = {
            meta.ref_count = meta.ref_count.saturating_add(count);
            meta.used_once = true;
            meta.zero_ref_frames = 0;
            *meta
        };
        self.material_meta_by.insert(id, updated);
        if should_track_ref {
            self.material_ref_ids.push(id);
        }
        if should_queue {
            self.queue_material_gc(id);
        }
        if should_log {
            let source = self
                .material_source_by
                .get(&id)
                .map(String::as_str)
                .unwrap_or("<inline>");
            self.log_used_once("material", id.index(), id.generation(), source);
        }
    }

    pub fn gc_unused(&mut self, ttl_frames: u32) -> ResourceGcDrops {
        self.gc_unused_after_frames(ttl_frames, 1, usize::MAX)
    }

    pub fn gc_unused_after_frames(
        &mut self,
        ttl_frames: u32,
        elapsed_frames: u32,
        max_drops_per_kind: usize,
    ) -> ResourceGcDrops {
        let ttl_frames = ttl_frames.max(1);
        let elapsed_frames = elapsed_frames.max(1);
        let mut drops = ResourceGcDrops::default();

        let texture_candidates = std::mem::take(&mut self.texture_gc_candidates);
        for id in texture_candidates {
            let Some(meta) = self.texture_meta_mut(id) else {
                continue;
            };
            if meta.ref_count > 0 {
                let updated = {
                    meta.zero_ref_frames = 0;
                    *meta
                };
                self.texture_meta_by.insert(id, updated);
                self.texture_gc_candidates.push(id);
                continue;
            }
            if meta.reserved || !meta.used_once || !meta.gc_queued {
                continue;
            }
            let updated = {
                meta.zero_ref_frames = meta.zero_ref_frames.saturating_add(elapsed_frames);
                *meta
            };
            self.texture_meta_by.insert(id, updated);
            if updated.zero_ref_frames >= ttl_frames && drops.textures.len() < max_drops_per_kind {
                drops.textures.push(id);
            } else {
                self.texture_gc_candidates.push(id);
            }
        }
        let mut write = 0usize;
        for read in 0..drops.textures.len() {
            let id = drops.textures[read];
            let source = self
                .texture_source_by
                .get(&id)
                .map(String::as_str)
                .unwrap_or("<unknown>");
            self.log_auto_drop("texture", id.index(), id.generation(), source, ttl_frames);
            if self.drop_texture_inner(id, false) {
                drops.textures[write] = id;
                write += 1;
            }
        }
        drops.textures.truncate(write);
        let mesh_candidates = std::mem::take(&mut self.mesh_gc_candidates);
        for id in mesh_candidates {
            let Some(meta) = self.mesh_meta_mut(id) else {
                continue;
            };
            if meta.ref_count > 0 {
                let updated = {
                    meta.zero_ref_frames = 0;
                    *meta
                };
                self.mesh_meta_by.insert(id, updated);
                self.mesh_gc_candidates.push(id);
                continue;
            }
            if meta.reserved || !meta.used_once || !meta.gc_queued {
                continue;
            }
            let updated = {
                meta.zero_ref_frames = meta.zero_ref_frames.saturating_add(elapsed_frames);
                *meta
            };
            self.mesh_meta_by.insert(id, updated);
            if updated.zero_ref_frames >= ttl_frames && drops.meshes.len() < max_drops_per_kind {
                drops.meshes.push(id);
            } else {
                self.mesh_gc_candidates.push(id);
            }
        }
        let mut write = 0usize;
        for read in 0..drops.meshes.len() {
            let id = drops.meshes[read];
            let source = self
                .mesh_source_by
                .get(&id)
                .map(String::as_str)
                .unwrap_or("<unknown>");
            self.log_auto_drop("mesh", id.index(), id.generation(), source, ttl_frames);
            if self.drop_mesh_inner(id, false) {
                drops.meshes[write] = id;
                write += 1;
            }
        }
        drops.meshes.truncate(write);
        let material_candidates = std::mem::take(&mut self.material_gc_candidates);
        for id in material_candidates {
            let Some(meta) = self.material_meta_mut(id) else {
                continue;
            };
            if meta.ref_count > 0 {
                let updated = {
                    meta.zero_ref_frames = 0;
                    *meta
                };
                self.material_meta_by.insert(id, updated);
                self.material_gc_candidates.push(id);
                continue;
            }
            if meta.reserved || !meta.used_once || !meta.gc_queued {
                continue;
            }
            let updated = {
                meta.zero_ref_frames = meta.zero_ref_frames.saturating_add(elapsed_frames);
                *meta
            };
            self.material_meta_by.insert(id, updated);
            if updated.zero_ref_frames >= ttl_frames && drops.materials.len() < max_drops_per_kind {
                drops.materials.push(id);
            } else {
                self.material_gc_candidates.push(id);
            }
        }
        let mut write = 0usize;
        for read in 0..drops.materials.len() {
            let id = drops.materials[read];
            let source = self
                .material_source_by
                .get(&id)
                .map(String::as_str)
                .unwrap_or("<inline>");
            self.log_auto_drop("material", id.index(), id.generation(), source, ttl_frames);
            if self.drop_material_inner(id, false) {
                drops.materials[write] = id;
                write += 1;
            }
        }
        drops.materials.truncate(write);
        drops
    }

    #[inline]
    pub fn set_texture_reserved(&mut self, id: TextureID, reserved: bool) -> bool {
        if let Some(meta) = self.texture_meta_mut(id) {
            meta.reserved = reserved;
            if reserved {
                meta.zero_ref_frames = 0;
                meta.gc_queued = false;
            }
            let updated = *meta;
            self.texture_meta_by.insert(id, updated);
            if !reserved {
                self.queue_texture_gc(id);
            }
            return true;
        }
        false
    }

    #[inline]
    pub fn set_mesh_reserved(&mut self, id: MeshID, reserved: bool) -> bool {
        if let Some(meta) = self.mesh_meta_mut(id) {
            meta.reserved = reserved;
            if reserved {
                meta.zero_ref_frames = 0;
                meta.gc_queued = false;
            }
            let updated = *meta;
            self.mesh_meta_by.insert(id, updated);
            if !reserved {
                self.queue_mesh_gc(id);
            }
            return true;
        }
        false
    }

    #[inline]
    pub fn set_material_reserved(&mut self, id: MaterialID, reserved: bool) -> bool {
        if let Some(meta) = self.material_meta_mut(id) {
            meta.reserved = reserved;
            if reserved {
                meta.zero_ref_frames = 0;
                meta.gc_queued = false;
            }
            let updated = *meta;
            self.material_meta_by.insert(id, updated);
            if !reserved {
                self.queue_material_gc(id);
            }
            return true;
        }
        false
    }

    pub fn drop_texture(&mut self, id: TextureID) -> bool {
        self.drop_texture_inner(id, true)
    }

    fn drop_texture_inner(&mut self, id: TextureID, log_manual: bool) -> bool {
        let removed = self.textures.remove_parts(id.index(), id.generation());
        let source = self.texture_source_by.remove(&id).or_else(|| {
            self.texture_by_source
                .iter()
                .find_map(|(source, existing)| (*existing == id).then_some(source.clone()))
        });
        if let Some(source) = source {
            if log_manual {
                self.log_manual_drop("texture", id.index(), id.generation(), &source);
            }
            if self.texture_by_source.get(&source).copied() == Some(id) {
                self.texture_by_source.remove(&source);
            }
            self.decoded_texture_by_source.remove(&source);
            self.clear_texture_source_slot_if(id.index(), &source);
        }
        self.decoded_texture_by_id.remove(&id);
        if removed {
            self.clear_texture_source_slot(id.index());
        }
        self.clear_texture_meta(id);
        removed
    }

    pub fn drop_mesh(&mut self, id: MeshID) -> bool {
        self.drop_mesh_inner(id, true)
    }

    fn drop_mesh_inner(&mut self, id: MeshID, log_manual: bool) -> bool {
        let removed = self.meshes.remove_parts(id.index(), id.generation());
        let source = self.mesh_source_by.remove(&id).or_else(|| {
            self.mesh_by_source
                .iter()
                .find_map(|(source, existing)| (*existing == id).then_some(source.clone()))
        });
        if let Some(source) = source {
            if log_manual {
                self.log_manual_drop("mesh", id.index(), id.generation(), &source);
            }
            if self.mesh_by_source.get(&source).copied() == Some(id) {
                self.mesh_by_source.remove(&source);
            }
            self.runtime_mesh_by_source.remove(&source);
            self.clear_mesh_source_slot_if(id.index(), &source);
        }
        self.runtime_mesh_by_id.remove(&id);
        self.mesh_revision_by_id.remove(&id);
        if removed {
            self.clear_mesh_source_slot(id.index());
        }
        self.clear_mesh_meta(id);
        removed
    }

    pub fn drop_material(&mut self, id: MaterialID) -> bool {
        self.drop_material_inner(id, true)
    }

    fn drop_material_inner(&mut self, id: MaterialID, log_manual: bool) -> bool {
        let removed = self.materials.remove_parts(id.index(), id.generation());
        self.material_by.remove(&id);
        let source = self.material_source_by.remove(&id).or_else(|| {
            self.material_by_source
                .iter()
                .find_map(|(source, existing)| (*existing == id).then_some(source.clone()))
        });
        if let Some(source) = source {
            if log_manual {
                self.log_manual_drop("material", id.index(), id.generation(), &source);
            }
            if self.material_by_source.get(&source).copied() == Some(id) {
                self.material_by_source.remove(&source);
            }
        }
        self.clear_material_meta(id);
        removed
    }

    fn log_resource_created(
        &self,
        _kind: &str,
        _index: u32,
        _generation: u32,
        _source: &str,
        _reserved: bool,
    ) {
    }

    fn log_resource_reused(
        &self,
        _kind: &str,
        _index: u32,
        _generation: u32,
        _source: &str,
        _reserved: bool,
    ) {
    }

    fn log_used_once(&self, _kind: &str, _index: u32, _generation: u32, _source: &str) {}

    fn log_auto_drop(
        &self,
        _kind: &str,
        _index: u32,
        _generation: u32,
        _source: &str,
        _ttl_frames: u32,
    ) {
    }

    fn log_manual_drop(&self, _kind: &str, _index: u32, _generation: u32, _source: &str) {}
}

#[cfg(test)]
#[path = "../tests/unit/resources_tests.rs"]
mod tests;
