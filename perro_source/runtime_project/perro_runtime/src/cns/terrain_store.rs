use perro_ids::TerrainID;
use perro_terrain::TerrainData;

pub(crate) struct TerrainStore {
    slots: Vec<Option<TerrainData>>,
    generations: Vec<u32>,
    free_indices: Vec<usize>,
}

impl TerrainStore {
    pub(crate) fn new() -> Self {
        let mut slots = Vec::with_capacity(2);
        let mut generations = Vec::with_capacity(2);
        slots.push(None);
        generations.push(0);
        Self {
            slots,
            generations,
            free_indices: Vec::new(),
        }
    }

    pub(crate) fn insert(&mut self, data: TerrainData) -> TerrainID {
        if let Some(index) = self.free_indices.pop() {
            self.slots[index] = Some(data);
            let generation = self.generations[index];
            return TerrainID::from_parts(index as u32, generation);
        }

        let index = self.slots.len();
        self.slots.push(Some(data));
        self.generations.push(0);
        TerrainID::from_parts(index as u32, 0)
    }

    pub(crate) fn get(&self, id: TerrainID) -> Option<&TerrainData> {
        if !self.is_valid(id) {
            return None;
        }
        self.slots[id.index() as usize].as_ref()
    }

    #[cfg(test)]
    pub(crate) fn get_mut(&mut self, id: TerrainID) -> Option<&mut TerrainData> {
        if !self.is_valid(id) {
            return None;
        }
        self.slots[id.index() as usize].as_mut()
    }

    pub(crate) fn remove(&mut self, id: TerrainID) -> Option<TerrainData> {
        if !self.is_valid(id) {
            return None;
        }
        let index = id.index() as usize;
        self.generations[index] = self.generations[index].wrapping_add(1);
        let removed = self.slots[index].take();
        if removed.is_some() {
            self.free_indices.push(index);
        }
        removed
    }

    pub(crate) fn clear(&mut self) {
        self.slots.clear();
        self.generations.clear();
        self.free_indices.clear();
        self.slots.push(None);
        self.generations.push(0);
    }

    fn is_valid(&self, id: TerrainID) -> bool {
        !id.is_nil()
            && id.index() != 0
            && id.index() < self.slots.len() as u32
            && self.generations[id.index() as usize] == id.generation()
    }
}

impl Default for TerrainStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "../../tests/unit/cns_terrain_store_tests.rs"]
mod tests;
