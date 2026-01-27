use crate::node_registry::SceneNode;
use crate::uid32::NodeID;

/// Arena-based storage for scene nodes.
/// Uses a Vec<Option<SceneNode>> indexed by NodeID for O(1) lookups.
/// Since NodeIDs are issued sequentially and 0 is reserved, we can use
/// the NodeID value directly as an index (with appropriate bounds checking).
pub struct NodeArena {
    slots: Vec<Option<SceneNode>>,
    live: u32,
}

impl NodeArena {
    /// Create a new empty arena.
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            live: 0,
        }
    }

    /// Create a new arena with the specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: Vec::with_capacity(capacity),
            live: 0,
        }
    }

    /// Insert a node into the arena.
    /// The node's ID is used as the index (subtracting 1 since 0 is reserved).
    pub fn insert(&mut self, id: NodeID, node: SceneNode) {
        // NodeID 0 is reserved (nil), so we subtract 1 to map NodeID 1 -> index 0
        let id_val = id.as_uid32().as_u32();
        if id_val == 0 {
            panic!("NodeArena::insert: cannot insert node with nil ID (0)");
        }
        let idx = (id_val as usize) - 1;

        if idx >= self.slots.len() {
            self.slots.resize_with(idx + 1, || None);
        }

        if self.slots[idx].is_some() {
            panic!("NodeArena::insert: slot already occupied (id={})", id);
        }

        self.slots[idx] = Some(node);
        self.live += 1;
    }

    /// Get a reference to the node (if present).
    #[inline]
    pub fn get(&self, id: NodeID) -> Option<&SceneNode> {
        let id_val = id.as_uid32().as_u32();
        if id_val == 0 {
            return None; // NodeID 0 is reserved (nil)
        }
        let idx = (id_val as usize) - 1;
        self.slots.get(idx)?.as_ref()
    }

    /// Get a mutable reference to the node (if present).
    #[inline]
    pub fn get_mut(&mut self, id: NodeID) -> Option<&mut SceneNode> {
        let id_val = id.as_uid32().as_u32();
        if id_val == 0 {
            return None; // NodeID 0 is reserved (nil)
        }
        let idx = (id_val as usize) - 1;
        self.slots.get_mut(idx)?.as_mut()
    }

    /// Remove a node, leaving a hole (`None`).
    #[inline]
    pub fn remove(&mut self, id: NodeID) -> Option<SceneNode> {
        let id_val = id.as_uid32().as_u32();
        if id_val == 0 {
            return None; // NodeID 0 is reserved (nil)
        }
        let idx = (id_val as usize) - 1;
        let slot = self.slots.get_mut(idx)?;
        let out = slot.take()?;
        self.live -= 1;
        Some(out)
    }

    /// Get the number of live nodes in the arena.
    #[inline]
    pub fn len(&self) -> usize {
        self.live as usize
    }

    /// Check if the arena is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Iterate over all live nodes (non-None slots).
    /// Returns `(NodeID, &SceneNode)` - the ID is owned (but Copy, so cheap).
    pub fn iter(&self) -> impl Iterator<Item = (NodeID, &SceneNode)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(idx, slot)| {
                slot.as_ref().map(|node| {
                    // Add 1 back since we subtracted 1 when storing
                    let id = NodeID::from_u32((idx + 1) as u32);
                    (id, node)
                })
            })
    }

    /// Iterate mutably over all live nodes (non-None slots).
    /// Returns `(NodeID, &mut SceneNode)` - the ID is owned (but Copy, so cheap).
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (NodeID, &mut SceneNode)> {
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(|(idx, slot)| {
                slot.as_mut().map(|node| {
                    // Add 1 back since we subtracted 1 when storing
                    let id = NodeID::from_u32((idx + 1) as u32);
                    (id, node)
                })
            })
    }

    /// Get all node IDs in the arena.
    pub fn keys(&self) -> impl Iterator<Item = NodeID> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(idx, slot)| {
                if slot.is_some() {
                    // Add 1 back since we subtracted 1 when storing
                    Some(NodeID::from_u32((idx + 1) as u32))
                } else {
                    None
                }
            })
    }

    /// Get all nodes in the arena.
    pub fn values(&self) -> impl Iterator<Item = &SceneNode> {
        self.slots.iter().filter_map(|slot| slot.as_ref())
    }

    /// Get mutable references to all nodes in the arena.
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut SceneNode> {
        self.slots.iter_mut().filter_map(|slot| slot.as_mut())
    }

    /// Consume the arena and return an iterator over all nodes.
    pub fn into_values(self) -> impl Iterator<Item = SceneNode> {
        self.slots.into_iter().filter_map(|slot| slot)
    }

    /// Check if a node with the given ID exists.
    #[inline]
    pub fn contains_key(&self, id: NodeID) -> bool {
        self.get(id).is_some()
    }

    /// Reserve capacity for at least `additional` more nodes.
    /// This is a no-op for NodeArena since Vec grows automatically,
    /// but provided for API compatibility with HashMap.
    #[inline]
    pub fn reserve(&mut self, _additional: usize) {
        // Vec grows automatically, so this is a no-op
        // But we could pre-allocate if needed in the future
    }
    
    /// Get the next available node ID based on the current highest ID in the arena.
    /// This is used as a fallback when static counter collisions are detected (DLL scenarios).
    #[inline]
    pub fn next_id(&self) -> NodeID {
        // Find the highest ID currently in use
        let max_id = self.slots
            .iter()
            .enumerate()
            .filter_map(|(idx, slot)| {
                if slot.is_some() {
                    // Add 1 back since we subtracted 1 when storing
                    Some((idx + 1) as u32)
                } else {
                    None
                }
            })
            .max()
            .unwrap_or(0); // If arena is empty, start at 1 (0 is reserved for nil)
        
        // Return the next ID (max_id + 1, but at least 1)
        NodeID::from_u32((max_id + 1).max(1))
    }
    
    /// Check if an ID is already in use (without bounds checking).
    /// Returns true if the slot exists and is occupied.
    #[inline]
    pub fn contains_id(&self, id: NodeID) -> bool {
        let id_val = id.as_uid32().as_u32();
        if id_val == 0 {
            return false; // NodeID 0 is reserved (nil)
        }
        let idx = (id_val as usize) - 1;
        self.slots.get(idx).map_or(false, |slot| slot.is_some())
    }
}

impl Default for NodeArena {
    fn default() -> Self {
        Self::new()
    }
}
