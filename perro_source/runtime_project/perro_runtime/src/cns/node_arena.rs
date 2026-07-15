use ahash::{AHashMap, AHashSet};
use perro_ids::{NodeID, NodeTag, TagID};
use perro_nodes::{NodeType, SceneNode};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

/// Generational node store used by runtime hot paths.
///
/// Slot 0 is always empty so `NodeID::nil()` and raw index 0 never alias a real
/// node. Removing a node bumps its generation and adds the slot to the free
/// list. Every public lookup checks both slot bounds and generation before it
/// returns a node reference.
///
/// Node names and tags are indexed for O(1) lookup via [`Self::named_ids`] /
/// [`Self::tag_index`]. Both indices are maintained by insert/remove/clear
/// plus the tracked mutating accessors ([`Self::edit`], [`Self::rename`],
/// [`Self::set_node_tags`], [`Self::add_node_tag`],
/// [`Self::remove_node_tag`], [`Self::set_parent`]). Prefer [`Self::edit`] when
/// one operation may change both indexed and ordinary fields.
///
/// [`Self::get_mut`] returns a tracked guard. Any name, tag, parent, or node
/// type change repairs the matching index or slot mirror when the guard drops,
/// including during unwinding.
pub struct NodeArena {
    nodes: Vec<Option<SceneNode>>,
    generations: Vec<u32>,
    /// Slot-indexed hot mirror of each node's type tag. Contiguous scan lane
    /// so type filters skip the wide `SceneNode` slots entirely. Value only
    /// meaningful while the slot is occupied (stale after remove; scans must
    /// still validate occupancy via `slot_get`). Node type is fixed at insert
    /// unless the data variant is replaced through [`Self::edit`].
    node_types: Vec<NodeType>,
    /// Slot-indexed hot mirror of each node's parent id. Kept in sync by
    /// insert/remove/clear + [`Self::set_parent`]. Nil while slot is free.
    parents: Vec<NodeID>,
    free_indices: Vec<usize>,
    name_index: AHashMap<Cow<'static, str>, Vec<NodeID>>,
    tag_index: AHashMap<TagID, AHashSet<NodeID>>,
    active_len: usize,
    /// bump on any mut access / structural chg; cache invalidation key 4 systems
    /// that mirror node data (resource-ref scan)
    mutation_revision: u64,
    /// bump on structural chg + physics-relevant mut access. Split frm
    /// mutation_revision so per-frame non-physics data mut (UI text, sprite
    /// frames) not invalidate physics world sync gate. Tracked `get_mut` bump
    /// both; only `get_mut_untracked_non_physics` skip physics bump.
    /// and must cal `mark_physics_change` when the node is physics-relevant.
    physics_revision: u64,
    /// bump ONLY on structural chg: insert / remove / clear / reparent. Data
    /// mut (transform, script var, UI text) never move this. Cheap gate 4
    /// systems that care only whether node set/topology chg (audio scene-flag
    /// rescan). Structural bumps also move mutation_revision + physics_revision.
    structural_revision: u64,
}

/// Tracked mutable node access from [`NodeArena::get_mut`].
///
/// Indexed fields and slot mirrors repair when this guard drops, including
/// during unwinding. The arena stays exclusively borrowed for the guard's
/// lifetime.
#[must_use = "dropping the guard ends the tracked mutation"]
pub struct NodeMut<'a> {
    arena: &'a mut NodeArena,
    id: NodeID,
    index: usize,
    old_name: Cow<'static, str>,
    old_tags: Vec<TagID>,
    old_parent: NodeID,
    old_node_type: NodeType,
}

impl Deref for NodeMut<'_> {
    type Target = SceneNode;

    fn deref(&self) -> &Self::Target {
        self.arena.nodes[self.index]
            .as_ref()
            .expect("tracked node slot stays live while borrowed")
    }
}

impl DerefMut for NodeMut<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.arena.nodes[self.index]
            .as_mut()
            .expect("tracked node slot stays live while borrowed")
    }
}

impl Drop for NodeMut<'_> {
    fn drop(&mut self) {
        let (new_name, new_tags, new_parent, new_node_type) = {
            let node = self.arena.nodes[self.index]
                .as_ref()
                .expect("tracked node slot stays live while borrowed");
            (
                node.name.clone(),
                node.get_tag_ids(),
                node.parent,
                node.node_type(),
            )
        };

        if self.old_name != new_name {
            self.arena.unindex_name(&self.old_name, self.id);
            if !new_name.is_empty() {
                self.arena
                    .name_index
                    .entry(new_name)
                    .or_default()
                    .push(self.id);
            }
        }

        for tag in &self.old_tags {
            if !new_tags.contains(tag) {
                self.arena.unindex_tag(*tag, self.id);
            }
        }
        for tag in new_tags {
            self.arena.tag_index.entry(tag).or_default().insert(self.id);
        }

        self.arena.parents[self.index] = new_parent;
        self.arena.node_types[self.index] = new_node_type;
        if self.old_parent != new_parent || self.old_node_type != new_node_type {
            self.arena.bump_structural_revision();
        } else {
            self.arena.bump_mutation_revision();
        }
    }
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
            node_types: vec![NodeType::Node],
            parents: vec![NodeID::nil()],
            free_indices: Vec::new(),
            name_index: AHashMap::default(),
            tag_index: AHashMap::default(),
            active_len: 0,
            mutation_revision: 0,
            physics_revision: 0,
            structural_revision: 0,
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
        let mut node_types = Vec::with_capacity(capacity.saturating_add(1));
        node_types.push(NodeType::Node);
        let mut parents = Vec::with_capacity(capacity.saturating_add(1));
        parents.push(NodeID::nil());
        Self {
            nodes,
            generations,
            node_types,
            parents,
            free_indices: Vec::new(),
            name_index: AHashMap::default(),
            tag_index: AHashMap::default(),
            active_len: 0,
            mutation_revision: 0,
            physics_revision: 0,
            structural_revision: 0,
        }
    }

    /// Current mutation revision. Chg every time node data may have chg.
    #[inline]
    pub fn mutation_revision(&self) -> u64 {
        self.mutation_revision
    }

    /// Physics-facing revision: chg on structural changes + mutations that may
    /// touch physics-relevant node data. Non-physics data mutations routed
    /// through [`Self::get_mut_untracked_non_physics`] do NOT move this revision.
    #[inline]
    pub fn physics_revision(&self) -> u64 {
        self.physics_revision
    }

    /// Record a possible physics-relevant data change. Pairs with
    /// [`Self::get_mut_untracked_non_physics`].
    #[inline]
    pub fn mark_physics_change(&mut self) {
        self.physics_revision = self.physics_revision.wrapping_add(1);
    }

    /// Structural revision: chg only on insert / remove / clear / reparent —
    /// NOT on data mutations. Cheap gate 4 systems that care only whether the
    /// node set or topology chg (e.g. audio scene-flag rescan).
    #[inline]
    pub fn structural_revision(&self) -> u64 {
        self.structural_revision
    }

    #[inline]
    fn bump_mutation_revision(&mut self) {
        self.mutation_revision = self.mutation_revision.wrapping_add(1);
        self.physics_revision = self.physics_revision.wrapping_add(1);
    }

    /// Structural change: bump structural + mutation + physics revisions.
    #[inline]
    fn bump_structural_revision(&mut self) {
        self.structural_revision = self.structural_revision.wrapping_add(1);
        self.mutation_revision = self.mutation_revision.wrapping_add(1);
        self.physics_revision = self.physics_revision.wrapping_add(1);
    }

    #[inline]
    fn bump_data_revision_only(&mut self) {
        self.mutation_revision = self.mutation_revision.wrapping_add(1);
    }

    // ---- Allocation ----

    /// Reserve slots for additional node inserts.
    pub fn reserve(&mut self, additional: usize) {
        self.nodes.reserve(additional);
        self.generations.reserve(additional);
        self.node_types.reserve(additional);
        self.parents.reserve(additional);
    }

    /// Insert a node and return its current slot/generation id.
    ///
    /// Reuses a free slot when available. Otherwise appends a new slot.
    pub fn insert(&mut self, node: SceneNode) -> NodeID {
        self.bump_structural_revision();
        let name = node.name.clone();
        let node_type = node.node_type();
        let parent = node.parent;
        // Reuse a previously freed slot in O(1).
        let id = if let Some(index) = self.free_indices.pop() {
            self.nodes[index] = Some(node);
            self.node_types[index] = node_type;
            self.parents[index] = parent;
            self.active_len = self.active_len.saturating_add(1);
            let generation = self.generations[index];
            NodeID::from_parts(index as u32, generation)
        } else {
            // No free slots, push to end
            let index = self.nodes.len();
            self.nodes.push(Some(node));
            let generation = if let Some(generation) = self.generations.get(index).copied() {
                generation
            } else {
                self.generations.push(0);
                0
            };
            self.node_types.push(node_type);
            self.parents.push(parent);
            self.active_len = self.active_len.saturating_add(1);
            NodeID::from_parts(index as u32, generation)
        };
        if !name.is_empty() {
            self.name_index.entry(name).or_default().push(id);
        }
        // Read tags from the stored node. Building a temporary Vec<TagID> here
        // made every tagged insert allocate once before the real index update.
        let index = id.index() as usize;
        for tag_index in 0..self.nodes[index].as_ref().map_or(0, |node| node.tags.len()) {
            let tag = self.nodes[index]
                .as_ref()
                .expect("inserted slot stays live")
                .tags[tag_index]
                .id;
            self.tag_index.entry(tag).or_default().insert(id);
        }
        id
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

    /// Get a tracked mutable node by id.
    ///
    /// Returns `None` for nil ids, stale generations, out-of-bounds slots, or
    /// empty slots.
    ///
    /// Name, tag, parent, and node type changes repair arena indices and slot
    /// mirrors when the returned guard drops. Repair also runs during unwind.
    pub fn get_mut(&mut self, id: NodeID) -> Option<NodeMut<'_>> {
        let index = self.valid_slot(id)?;
        let node = self.nodes[index].as_ref()?;
        Some(NodeMut {
            id,
            index,
            old_name: node.name.clone(),
            old_tags: node.get_tag_ids(),
            old_parent: node.parent,
            old_node_type: node.node_type(),
            arena: self,
        })
    }

    /// Edit a node while keeping its name/tag indices and parent mirror in
    /// sync.
    ///
    /// The callback may mutate any public [`SceneNode`] field. Index/mirror
    /// repair and revision updates run when the callback returns or unwinds.
    /// Returns `None` without calling the callback when `id` is not live.
    pub fn edit<R>(&mut self, id: NodeID, edit: impl FnOnce(&mut SceneNode) -> R) -> Option<R> {
        let mut tracked = self.get_mut(id)?;
        Some(edit(&mut tracked))
    }

    /// Raw mutable hot path for typed node-data edits that cannot touch name,
    /// tags, parent, or replace the data variant.
    ///
    /// # Invariant contract
    ///
    /// Caller must prove indexed fields + mirrored fields stay unchanged.
    /// Prefer [`Self::get_mut`] outside profiled internal paths.
    pub(crate) fn get_mut_untracked(&mut self, id: NodeID) -> Option<&mut SceneNode> {
        let index = self.valid_slot(id)?;
        self.bump_mutation_revision();
        debug_assert_eq!(self.parents[index], self.nodes[index].as_ref()?.parent);
        debug_assert_eq!(
            self.node_types[index],
            self.nodes[index].as_ref()?.node_type()
        );
        self.nodes[index].as_mut()
    }

    /// Raw non-physics variant of [`Self::get_mut_untracked`].
    ///
    /// # Invariant contract
    ///
    /// Caller must also prove the edit cannot affect physics state.
    pub(crate) fn get_mut_untracked_non_physics(&mut self, id: NodeID) -> Option<&mut SceneNode> {
        let index = self.valid_slot(id)?;
        self.bump_data_revision_only();
        debug_assert_eq!(self.parents[index], self.nodes[index].as_ref()?.parent);
        debug_assert_eq!(
            self.node_types[index],
            self.nodes[index].as_ref()?.node_type()
        );
        self.nodes[index].as_mut()
    }

    // ---- Removal ----

    /// Remove a node and invalidate old ids for that slot.
    ///
    /// Successful removal bumps the slot generation and pushes the slot onto
    /// the free list for later reuse.
    pub fn remove(&mut self, id: NodeID) -> Option<SceneNode> {
        let index = self.valid_slot(id)?;
        self.bump_structural_revision();
        self.generations[index] = self.generations[index].wrapping_add(1);
        let removed = self.nodes[index].take();
        if let Some(node) = &removed {
            self.active_len = self.active_len.saturating_sub(1);
            self.parents[index] = NodeID::nil();
            self.free_indices.push(index);
            let name = node.name.clone();
            self.unindex_name(&name, id);
            for tag in node.get_tag_ids() {
                self.unindex_tag(tag, id);
            }
        }
        removed
    }

    // ---- Name index ----

    /// All live nodes currently carrying `name`, in insertion order.
    pub fn named_ids(&self, name: &str) -> &[NodeID] {
        self.name_index.get(name).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Rename a node, keeping the name index in sync. Bumps the mutation
    /// revision like any `get_mut` write. Returns `false` for dead ids.
    pub fn rename(&mut self, id: NodeID, name: Cow<'static, str>) -> bool {
        let Some(index) = self.valid_slot(id) else {
            return false;
        };
        let Some(node) = self.nodes[index].as_mut() else {
            return false;
        };
        if node.name == name {
            return true;
        }
        self.bump_mutation_revision();
        let node = self.nodes[index].as_mut().expect("slot checked live above");
        let old = std::mem::replace(&mut node.name, name.clone());
        self.unindex_name(&old, id);
        if !name.is_empty() {
            self.name_index.entry(name).or_default().push(id);
        }
        true
    }

    fn unindex_name(&mut self, name: &str, id: NodeID) {
        if name.is_empty() {
            return;
        }
        if let Some(ids) = self.name_index.get_mut(name) {
            ids.retain(|item| *item != id);
            if ids.is_empty() {
                self.name_index.remove(name);
            }
        }
    }

    // ---- Tag index ----

    /// Tag → live node ids, kept in sync with node tag state.
    pub fn tag_index(&self) -> &AHashMap<TagID, AHashSet<NodeID>> {
        &self.tag_index
    }

    /// Replace (or clear, with `None`) a node's tags, keeping the tag index
    /// in sync. Returns `false` for dead ids.
    pub fn set_node_tags(&mut self, id: NodeID, tags: Option<Vec<NodeTag>>) -> bool {
        let Some(index) = self.valid_slot(id) else {
            return false;
        };
        let Some(node) = self.nodes[index].as_mut() else {
            return false;
        };
        let old = node.get_tag_ids();
        match tags {
            Some(tags) => node.set_tags(Some(tags)),
            None => node.clear_tags(),
        }
        let new = node.get_tag_ids();
        self.bump_mutation_revision();
        for tag in old {
            if !new.contains(&tag) {
                self.unindex_tag(tag, id);
            }
        }
        for tag in new {
            self.tag_index.entry(tag).or_default().insert(id);
        }
        true
    }

    /// Add a tag to a node (no-op when already present). Returns `false` for
    /// dead ids.
    pub fn add_node_tag(&mut self, id: NodeID, tag: NodeTag) -> bool {
        let Some(index) = self.valid_slot(id) else {
            return false;
        };
        let Some(node) = self.nodes[index].as_mut() else {
            return false;
        };
        let tag_id = tag.id;
        let added = if node.has_tag(tag_id) {
            false
        } else {
            node.add_tag(tag);
            true
        };
        self.bump_mutation_revision();
        if added {
            self.tag_index.entry(tag_id).or_default().insert(id);
        }
        true
    }

    /// Remove a tag from a node (no-op when absent). Returns `false` for
    /// dead ids.
    pub fn remove_node_tag(&mut self, id: NodeID, tag: TagID) -> bool {
        let Some(index) = self.valid_slot(id) else {
            return false;
        };
        let Some(node) = self.nodes[index].as_mut() else {
            return false;
        };
        let removed = node.has_tag(tag);
        if removed {
            node.remove_tag(tag);
        }
        self.bump_mutation_revision();
        if removed {
            self.unindex_tag(tag, id);
        }
        true
    }

    // ---- Parent mirror ----

    /// Reparent a live node, keeping the slot parent mirror in sync. Bumps
    /// the mutation revision like any `get_mut` write (reparent = structural,
    /// so the physics revision moves too). Returns `false` for dead ids.
    ///
    /// Writing `node.parent` through `get_mut` on an arena-resident node
    /// bypasses the mirror — always use this method instead.
    pub fn set_parent(&mut self, id: NodeID, parent: NodeID) -> bool {
        let Some(index) = self.valid_slot(id) else {
            return false;
        };
        let Some(node) = self.nodes[index].as_mut() else {
            return false;
        };
        node.parent = parent;
        self.parents[index] = parent;
        self.bump_structural_revision();
        true
    }

    /// Append one child without paying the tracked-edit snapshot cost.
    #[inline]
    pub(crate) fn push_child(&mut self, id: NodeID, child: NodeID) -> bool {
        let Some(index) = self.valid_slot(id) else {
            return false;
        };
        let Some(node) = self.nodes[index].as_mut() else {
            return false;
        };
        node.children.push(child);
        self.bump_data_revision_only();
        true
    }

    /// Append children without cloning name/tags for a tracked edit guard.
    #[inline]
    pub(crate) fn extend_children(&mut self, id: NodeID, children: &[NodeID]) -> bool {
        if children.is_empty() {
            return self.contains(id);
        }
        let Some(index) = self.valid_slot(id) else {
            return false;
        };
        let Some(node) = self.nodes[index].as_mut() else {
            return false;
        };
        node.children.reserve(children.len());
        node.children.extend_from_slice(children);
        self.bump_data_revision_only();
        true
    }

    fn unindex_tag(&mut self, tag: TagID, id: NodeID) {
        if let Some(set) = self.tag_index.get_mut(&tag) {
            set.remove(&id);
            if set.is_empty() {
                self.tag_index.remove(&tag);
            }
        }
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

    // ---- Whole-arena state ----

    /// Clear all nodes while preserving generation history for stale IDs.
    pub fn clear(&mut self) {
        self.bump_structural_revision();
        for index in 1..self.nodes.len() {
            self.generations[index] = self.generations[index].wrapping_add(1);
        }
        self.nodes.truncate(1);
        self.node_types.truncate(1);
        self.parents.truncate(1);
        self.free_indices.clear();
        self.name_index.clear();
        self.tag_index.clear();
        self.active_len = 0;
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

    /// Contiguous slot-indexed lane of node type tags (index 0 = nil slot).
    /// Values are only meaningful for occupied slots; freed slots keep their
    /// last occupant's type until reuse. Pair with `slot_get` for occupancy.
    #[inline]
    pub fn node_type_slots(&self) -> &[NodeType] {
        &self.node_types
    }

    /// Contiguous slot-indexed lane of parent ids (index 0 = nil slot).
    /// Nil for free slots.
    #[inline]
    pub fn parent_slots(&self) -> &[NodeID] {
        &self.parents
    }

    /// Type tag at a raw slot index. See [`Self::node_type_slots`] for
    /// staleness semantics on free slots.
    #[inline]
    pub fn slot_node_type(&self, index: usize) -> Option<NodeType> {
        self.node_types.get(index).copied()
    }

    /// Debug-only consistency check: every occupied slot's mirror entries
    /// must match the node they mirror. Called from characterization tests.
    #[cfg(any(test, debug_assertions))]
    pub fn validate_mirrors(&self) {
        for (index, slot) in self.nodes.iter().enumerate() {
            if let Some(node) = slot {
                debug_assert_eq!(
                    self.node_types[index],
                    node.node_type(),
                    "node_types mirror out of sync at slot {index}",
                );
                debug_assert_eq!(
                    self.parents[index], node.parent,
                    "parents mirror out of sync at slot {index}",
                );
            }
        }
        debug_assert_eq!(self.node_types.len(), self.nodes.len());
        debug_assert_eq!(self.parents.len(), self.nodes.len());
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
