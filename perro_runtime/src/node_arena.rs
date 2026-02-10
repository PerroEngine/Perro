use perro_core::SceneNode;
use perro_ids::NodeID;

pub struct NodeArena {
    nodes: Vec<Option<SceneNode>>,
    generations: Vec<u32>,
}

impl NodeArena {
    pub fn new() -> Self {
        Self {
            nodes: Vec::with_capacity(1),
            generations: Vec::with_capacity(1),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(capacity),
            generations: Vec::with_capacity(capacity),
        }
    }

    /// Insert a node, returns NodeID with index and generation
    pub fn insert(&mut self, node: SceneNode) -> NodeID {
        // Try to find a free slot first
        for (index, slot) in self.nodes.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(node);
                let generation = self.generations[index];
                return NodeID::from_parts(index as u32, generation);
            }
        }

        // No free slots, push to end
        let index = self.nodes.len();
        self.nodes.push(Some(node));
        self.generations.push(0);
        NodeID::from_parts(index as u32, 0)
    }

    /// Get a node by ID, returns None if generation doesn't match
    pub fn get(&self, id: NodeID) -> Option<&SceneNode> {
        if id.index() >= self.nodes.len() as u32
            || self.generations[id.index() as usize] != id.generation()
        {
            return None;
        }
        self.nodes[id.index() as usize].as_ref()
    }

    /// Get mutable reference to a node
    pub fn get_mut(&mut self, id: NodeID) -> Option<&mut SceneNode> {
        if id.index() >= self.nodes.len() as u32
            || self.generations[id.index() as usize] != id.generation()
        {
            return None;
        }
        self.nodes[id.index() as usize].as_mut()
    }

    /// Remove a node, bumping the generation counter
    pub fn remove(&mut self, id: NodeID) -> Option<SceneNode> {
        if id.index() >= self.nodes.len() as u32
            || self.generations[id.index() as usize] != id.generation()
        {
            return None;
        }

        self.generations[id.index() as usize] =
            self.generations[id.index() as usize].wrapping_add(1);
        self.nodes[id.index() as usize].take()
    }

    /// Check if a NodeID is still valid
    pub fn contains(&self, id: NodeID) -> bool {
        id.index() < self.nodes.len() as u32
            && self.generations[id.index() as usize] == id.generation()
            && self.nodes[id.index() as usize].is_some()
    }

    /// Iterator over all valid nodes
    pub fn iter(&self) -> impl Iterator<Item = (NodeID, &SceneNode)> {
        self.nodes.iter().enumerate().filter_map(|(index, node)| {
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
            .filter_map(|(index, (node, &generation))| {
                node.as_mut()
                    .map(|n| (NodeID::from_parts(index as u32, generation), n))
            })
    }

    /// Clear all nodes
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.generations.clear();
    }

    /// Number of active nodes
    pub fn len(&self) -> usize {
        self.nodes.iter().filter(|n| n.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.iter().all(|n| n.is_none())
    }
}
