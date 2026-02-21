use perro_core::SceneNode;
use perro_ids::NodeID;

pub struct NodeArena {
    nodes: Vec<Option<SceneNode>>,
    generations: Vec<u32>,
    free_indices: Vec<usize>,
}

impl NodeArena {
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
        }
    }

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
        }
    }

    /// Insert a node, returns NodeID with index and generation
    pub fn insert(&mut self, node: SceneNode) -> NodeID {
        // Reuse a previously freed slot in O(1).
        if let Some(index) = self.free_indices.pop() {
            self.nodes[index] = Some(node);
            let generation = self.generations[index];
            return NodeID::from_parts(index as u32, generation);
        }

        // No free slots, push to end
        let index = self.nodes.len();
        self.nodes.push(Some(node));
        self.generations.push(0);
        NodeID::from_parts(index as u32, 0)
    }

    /// Get a node by ID, returns None if generation doesn't match
    pub fn get(&self, id: NodeID) -> Option<&SceneNode> {
        if id.is_nil()
            || id.index() == 0
            || id.index() >= self.nodes.len() as u32
            || self.generations[id.index() as usize] != id.generation()
        {
            return None;
        }
        self.nodes[id.index() as usize].as_ref()
    }

    /// Get mutable reference to a node
    pub fn get_mut(&mut self, id: NodeID) -> Option<&mut SceneNode> {
        if id.is_nil()
            || id.index() == 0
            || id.index() >= self.nodes.len() as u32
            || self.generations[id.index() as usize] != id.generation()
        {
            return None;
        }
        self.nodes[id.index() as usize].as_mut()
    }

    /// Remove a node, bumping the generation counter
    pub fn remove(&mut self, id: NodeID) -> Option<SceneNode> {
        if id.is_nil()
            || id.index() == 0
            || id.index() >= self.nodes.len() as u32
            || self.generations[id.index() as usize] != id.generation()
        {
            return None;
        }

        let index = id.index() as usize;
        self.generations[index] = self.generations[index].wrapping_add(1);
        let removed = self.nodes[index].take();
        if removed.is_some() {
            self.free_indices.push(index);
        }
        removed
    }

    /// Check if a NodeID is still valid
    pub fn contains(&self, id: NodeID) -> bool {
        !id.is_nil()
            && id.index() != 0
            && id.index() < self.nodes.len() as u32
            && self.generations[id.index() as usize] == id.generation()
            && self.nodes[id.index() as usize].is_some()
    }

    /// Iterator over all valid nodes
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

    /// Mutable iterator
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

    /// Clear all nodes
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.generations.clear();
        self.free_indices.clear();
        self.nodes.push(None);
        self.generations.push(0);
    }

    /// Number of active nodes
    pub fn len(&self) -> usize {
        self.nodes.iter().filter(|n| n.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.iter().all(|n| n.is_none())
    }
}
