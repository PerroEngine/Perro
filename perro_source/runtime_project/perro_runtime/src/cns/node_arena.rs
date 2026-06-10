use perro_ids::NodeID;
use perro_nodes::SceneNode;

/// Generational node store used by runtime hot paths.
///
/// Slot 0 is always empty so `NodeID::nil()` and raw index 0 never alias a real
/// node. Removing a node bumps its generation and adds the slot to the free
/// list. Every public lookup checks both slot bounds and generation before it
/// returns a node reference.
pub struct NodeArena {
    nodes: Vec<Option<SceneNode>>,
    generations: Vec<u32>,
    free_indices: Vec<usize>,
    active_len: usize,
}

impl Default for NodeArena {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeArena {
    // ---- Construction ----

    /// Create an empty arena.
    ///
    /// Slot 0 is reserved as the nil sentinel, so the first inserted node uses
    /// index 1.
    pub fn new() -> Self {
        // Reserve index 0 as invalid/nil sentinel so first real node ID is 1.
        let mut nodes = Vec::with_capacity(2);
        let mut generations = Vec::with_capacity(2);
        nodes.push(None);
        generations.push(0);
        Self {
            nodes,
            generations,
            free_indices: Vec::new(),
            active_len: 0,
        }
    }

    /// Create an empty arena with capacity for active nodes.
    ///
    /// The internal vectors reserve one extra slot for the nil sentinel.
    pub fn with_capacity(capacity: usize) -> Self {
        // +1 for reserved nil sentinel slot at index 0.
        let mut nodes = Vec::with_capacity(capacity.saturating_add(1));
        let mut generations = Vec::with_capacity(capacity.saturating_add(1));
        nodes.push(None);
        generations.push(0);
        Self {
            nodes,
            generations,
            free_indices: Vec::new(),
            active_len: 0,
        }
    }

    // ---- Allocation ----

    /// Reserve slots for additional node inserts.
    pub fn reserve(&mut self, additional: usize) {
        self.nodes.reserve(additional);
        self.generations.reserve(additional);
    }

    /// Insert a node and return its current slot/generation id.
    ///
    /// Reuses a free slot when available. Otherwise appends a new slot.
    pub fn insert(&mut self, node: SceneNode) -> NodeID {
        // Reuse a previously freed slot in O(1).
        if let Some(index) = self.free_indices.pop() {
            self.nodes[index] = Some(node);
            self.active_len = self.active_len.saturating_add(1);
            let generation = self.generations[index];
            return NodeID::from_parts(index as u32, generation);
        }

        // No free slots, push to end
        let index = self.nodes.len();
        self.nodes.push(Some(node));
        self.generations.push(0);
        self.active_len = self.active_len.saturating_add(1);
        NodeID::from_parts(index as u32, 0)
    }

    // ---- Checked lookup ----

    /// Get a node by id.
    ///
    /// Returns `None` for nil ids, stale generations, out-of-bounds slots, or
    /// empty slots.
    pub fn get(&self, id: NodeID) -> Option<&SceneNode> {
        let index = self.valid_slot(id)?;
        self.nodes[index].as_ref()
    }

    /// Get a mutable node by id.
    ///
    /// Returns `None` for nil ids, stale generations, out-of-bounds slots, or
    /// empty slots.
    pub fn get_mut(&mut self, id: NodeID) -> Option<&mut SceneNode> {
        let index = self.valid_slot(id)?;
        self.nodes[index].as_mut()
    }

    // ---- Removal ----

    /// Remove a node and invalidate old ids for that slot.
    ///
    /// Successful removal bumps the slot generation and pushes the slot onto
    /// the free list for later reuse.
    pub fn remove(&mut self, id: NodeID) -> Option<SceneNode> {
        let index = self.valid_slot(id)?;
        self.generations[index] = self.generations[index].wrapping_add(1);
        let removed = self.nodes[index].take();
        if removed.is_some() {
            self.active_len = self.active_len.saturating_sub(1);
            self.free_indices.push(index);
        }
        removed
    }

    /// Check if a [`NodeID`] currently points at a live node.
    pub fn contains(&self, id: NodeID) -> bool {
        self.valid_slot(id)
            .is_some_and(|index| self.nodes[index].is_some())
    }

    // ---- Iteration ----

    /// Iterate over all live nodes with their current ids.
    pub fn iter(&self) -> impl Iterator<Item = (NodeID, &SceneNode)> {
        self.nodes
            .iter()
            .enumerate()
            .skip(1)
            .filter_map(|(index, node)| {
                node.as_ref()
                    .map(|n| (NodeID::from_parts(index as u32, self.generations[index]), n))
            })
    }

    /// Iterate mutably over all live nodes with their current ids.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (NodeID, &mut SceneNode)> {
        self.nodes
            .iter_mut()
            .zip(self.generations.iter())
            .enumerate()
            .skip(1)
            .filter_map(|(index, (node, &generation))| {
                node.as_mut()
                    .map(|n| (NodeID::from_parts(index as u32, generation), n))
            })
    }

    // ---- Whole-arena state ----

    /// Clear all nodes and reset the arena to only the nil sentinel slot.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.generations.clear();
        self.free_indices.clear();
        self.active_len = 0;
        self.nodes.push(None);
        self.generations.push(0);
    }

    /// Return the number of live nodes.
    pub fn len(&self) -> usize {
        self.active_len
    }

    /// Return whether the arena contains no live nodes.
    pub fn is_empty(&self) -> bool {
        self.active_len == 0
    }

    // ---- Raw slot fast paths ----

    /// Number of internal slots including the reserved nil slot at index 0.
    pub fn slot_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns node at a raw slot index if occupied. Intended for fast linear scans.
    pub fn slot_get(&self, index: usize) -> Option<(NodeID, &SceneNode)> {
        if index == 0 || index >= self.nodes.len() {
            return None;
        }
        let node = self.nodes[index].as_ref()?;
        Some((
            NodeID::from_parts(index as u32, self.generations[index]),
            node,
        ))
    }

    /// Returns node at a known slot when generation matches. Intended for scheduler fast paths.
    #[inline]
    pub fn slot_get_checked(&self, index: usize, generation: u32) -> Option<&SceneNode> {
        if index == 0 || index >= self.nodes.len() {
            return None;
        }
        if self.generations[index] != generation {
            return None;
        }
        self.nodes[index].as_ref()
    }

    /// Mutable variant of `slot_get_checked`.
    #[inline]
    pub fn slot_get_mut_checked(
        &mut self,
        index: usize,
        generation: u32,
    ) -> Option<&mut SceneNode> {
        if index == 0 || index >= self.nodes.len() {
            return None;
        }
        if self.generations[index] != generation {
            return None;
        }
        self.nodes[index].as_mut()
    }

    // ---- Slot validation ----

    /// Validate a public id and return its raw slot index.
    #[inline]
    fn valid_slot(&self, id: NodeID) -> Option<usize> {
        let index = id.index() as usize;
        if id.is_nil()
            || index == 0
            || index >= self.nodes.len()
            || self.generations[index] != id.generation()
        {
            return None;
        }
        Some(index)
    }
}
