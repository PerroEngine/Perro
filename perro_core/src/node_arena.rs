use crate::ids::NodeID;
use crate::node_registry::{BaseNode, SceneNode};

/// Slotmap-style arena for scene nodes.
/// NodeID = (index in low 32, generation in high 32). Index 0 is nil; real slots use index 1, 2, ...
/// Slots can be reused; generation is bumped on reuse so stale IDs become invalid.
/// Lookup is valid only when both the slot (from id.index()) and the generation (id.generation()
/// vs arena's stored generation for that slot) match.
pub struct NodeArena {
    /// Slot i (0-based) stores the node. Slot index 0 = NodeID index 1.
    slots: Vec<Option<SceneNode>>,
    /// Generation per slot. When we reuse a slot we bump this.
    generations: Vec<u32>,
    live: u32,
}

impl NodeArena {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            generations: Vec::new(),
            live: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: Vec::with_capacity(capacity),
            generations: Vec::with_capacity(capacity),
            live: 0,
        }
    }

    /// Allocate a new slot and return its NodeID. Caller then inserts the node with `insert_with_id`.
    /// Reuses the first free slot (bumping generation) or appends a new slot.
    pub fn allocate(&mut self) -> NodeID {
        if let Some(slot_idx) = self.slots.iter().position(|s| s.is_none()) {
            self.generations[slot_idx] = self.generations[slot_idx].wrapping_add(1);
            let generation = self.generations[slot_idx];
            NodeID::from_parts((slot_idx + 1) as u32, generation)
        } else {
            let slot_idx = self.slots.len();
            self.slots.push(None);
            self.generations.push(0);
            NodeID::from_parts((slot_idx + 1) as u32, 0)
        }
    }

    /// Insert a node and assign it the next allocated ID. Sets the node's ID and stores it.
    /// Use this when adding a new node at runtime (e.g. add_node).
    pub fn insert(&mut self, mut node: SceneNode) -> NodeID {
        let id = self.allocate();
        node.set_id(id);
        self.insert_with_id(id, node);
        id
    }

    /// Insert a node with a pre-assigned ID (e.g. when loading from scene).
    /// Extends slots/generations if needed. Panics if slot is already occupied.
    pub fn insert_with_id(&mut self, id: NodeID, node: SceneNode) {
        if id.is_nil() {
            panic!("NodeArena::insert_with_id: cannot insert with nil ID");
        }
        let slot_idx = (id.index() as usize).saturating_sub(1);
        if slot_idx >= self.slots.len() {
            self.slots.resize_with(slot_idx + 1, || None);
            self.generations.resize(slot_idx + 1, 0);
        }
        if self.slots[slot_idx].is_some() {
            panic!("NodeArena::insert_with_id: slot {} already occupied", id);
        }
        self.generations[slot_idx] = id.generation();
        self.slots[slot_idx] = Some(node);
        self.live += 1;
    }

    /// Legacy: insert by id only (for call sites that already have an id and node).
    #[inline]
    pub fn insert_legacy(&mut self, id: NodeID, node: SceneNode) {
        self.insert_with_id(id, node);
    }

    #[inline]
    fn slot_index(id: NodeID) -> Option<usize> {
        if id.is_nil() {
            return None;
        }
        let idx = id.index() as usize;
        if idx == 0 { None } else { Some(idx - 1) }
    }

    /// Valid only when slot (from id.index()) and generation (arena vs id) both match.
    #[inline]
    pub fn get(&self, id: NodeID) -> Option<&SceneNode> {
        let slot_idx = Self::slot_index(id)?;
        if self.generations.get(slot_idx) != Some(&id.generation()) {
            return None;
        }
        self.slots.get(slot_idx)?.as_ref()
    }

    #[inline]
    pub fn get_mut(&mut self, id: NodeID) -> Option<&mut SceneNode> {
        let slot_idx = Self::slot_index(id)?;
        if self.generations.get(slot_idx) != Some(&id.generation()) {
            return None;
        }
        self.slots.get_mut(slot_idx)?.as_mut()
    }

    #[inline]
    pub fn remove(&mut self, id: NodeID) -> Option<SceneNode> {
        let slot_idx = Self::slot_index(id)?;
        if self.generations.get(slot_idx) != Some(&id.generation()) {
            return None;
        }
        let out = self.slots.get_mut(slot_idx)?.take()?;
        self.live -= 1;
        Some(out)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.live as usize
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = (NodeID, &SceneNode)> {
        self.slots.iter().enumerate().filter_map(|(idx, slot)| {
            slot.as_ref().map(|node| {
                let generation = self.generations.get(idx).copied().unwrap_or(0);
                let id = NodeID::from_parts((idx + 1) as u32, generation);
                (id, node)
            })
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (NodeID, &mut SceneNode)> {
        let generations = &self.generations[..];
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(move |(idx, slot)| {
                slot.as_mut().map(|node| {
                    let generation = generations.get(idx).copied().unwrap_or(0);
                    let id = NodeID::from_parts((idx + 1) as u32, generation);
                    (id, node)
                })
            })
    }

    pub fn keys(&self) -> impl Iterator<Item = NodeID> + '_ {
        self.slots.iter().enumerate().filter_map(|(idx, slot)| {
            if slot.is_some() {
                let generation = self.generations.get(idx).copied().unwrap_or(0);
                Some(NodeID::from_parts((idx + 1) as u32, generation))
            } else {
                None
            }
        })
    }

    pub fn values(&self) -> impl Iterator<Item = &SceneNode> {
        self.slots.iter().filter_map(|slot| slot.as_ref())
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut SceneNode> {
        self.slots.iter_mut().filter_map(|slot| slot.as_mut())
    }

    pub fn into_values(self) -> impl Iterator<Item = SceneNode> {
        self.slots.into_iter().filter_map(|slot| slot)
    }

    #[inline]
    pub fn contains_key(&self, id: NodeID) -> bool {
        self.get(id).is_some()
    }

    #[inline]
    pub fn reserve(&mut self, _additional: usize) {}

    /// Allocate and return the next NodeID (for callers that will then insert_with_id).
    /// Same as allocate() — name kept for compatibility with scene.next_node_id().
    #[inline]
    pub fn next_id(&mut self) -> NodeID {
        self.allocate()
    }

    #[inline]
    pub fn contains_id(&self, id: NodeID) -> bool {
        self.get(id).is_some()
    }
}

impl Default for NodeArena {
    fn default() -> Self {
        Self::new()
    }
}
