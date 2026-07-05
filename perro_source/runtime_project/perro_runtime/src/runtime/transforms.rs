use super::Runtime;
use crate::runtime::state::DirtyState;
use glam::{Mat3, Mat4};
use perro_ids::NodeID;
use perro_nodes::{Node2D, Node3D, SceneNodeData, Spatial};
use perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};

const PHYSICS_POSE_EPS_SQ_2D: f32 = 0.0001;
const PHYSICS_POSE_EPS_SQ_3D: f32 = 0.0001;
const PHYSICS_POSE_ROT_EPS: f32 = 0.001;

impl Runtime {
    #[cfg(feature = "bench")]
    pub fn bench_refresh_dirty_global_transforms(&mut self) {
        self.refresh_dirty_global_transforms();
    }

    pub(crate) fn propagate_pending_transform_dirty(&mut self) {
        let mut roots = std::mem::take(&mut self.transforms.pending_transform_roots);
        self.dirty.take_pending_transform_roots(&mut roots);
        if roots.is_empty() {
            self.transforms.pending_transform_roots = roots;
            return;
        }

        let mut stack = std::mem::take(&mut self.transforms.traversal_stack);
        stack.clear();
        let slot_count = self.nodes.slot_count();
        if self.transforms.transform_visit_flags.len() < slot_count {
            self.transforms.transform_visit_flags.resize(slot_count, 0);
        }
        if self.transforms.transform_visit_indices.capacity() < slot_count {
            self.transforms
                .transform_visit_indices
                .reserve(slot_count - self.transforms.transform_visit_indices.capacity());
        }

        for root in roots.iter().copied() {
            if self.nodes.get(root).is_none() {
                continue;
            }
            stack.push(root);
            while let Some(id) = stack.pop() {
                let index = id.index() as usize;
                if self.transforms.transform_visit_flags[index] != 0 {
                    continue;
                }
                self.transforms.transform_visit_flags[index] = 1;
                self.transforms.transform_visit_indices.push(index as u32);

                let Some(node) = self.nodes.get(id) else {
                    continue;
                };
                // physics flag frm node type -> scoped gate; non-physics
                // subtree moves not force full physics world re-sync.
                let physics = node.node_type().is_physics();
                self.dirty.mark_transform(id, node.spatial(), physics);
                stack.extend_from_slice(node.children_slice());
            }
        }

        for &index in &self.transforms.transform_visit_indices {
            let i = index as usize;
            if i < self.transforms.transform_visit_flags.len() {
                self.transforms.transform_visit_flags[i] = 0;
            }
        }
        self.transforms.transform_visit_indices.clear();

        stack.clear();
        self.transforms.traversal_stack = stack;
        roots.clear();
        self.transforms.pending_transform_roots = roots;
    }

    #[inline]
    fn ensure_global_2d_capacity(&mut self, index: usize) {
        if self.transforms.global_transform_2d.len() <= index {
            self.transforms
                .global_transform_2d
                .resize(index + 1, Transform2D::IDENTITY);
        }
        if self.transforms.global_transform_2d_valid.len() <= index {
            self.transforms
                .global_transform_2d_valid
                .resize(index + 1, 0);
        }
        if self.transforms.global_transform_2d_generation.len() <= index {
            self.transforms
                .global_transform_2d_generation
                .resize(index + 1, 0);
        }
    }

    #[inline]
    fn ensure_global_3d_capacity(&mut self, index: usize) {
        if self.transforms.global_transform_3d.len() <= index {
            self.transforms
                .global_transform_3d
                .resize(index + 1, Transform3D::IDENTITY);
        }
        if self.transforms.global_transform_3d_valid.len() <= index {
            self.transforms
                .global_transform_3d_valid
                .resize(index + 1, 0);
        }
        if self.transforms.global_transform_3d_generation.len() <= index {
            self.transforms
                .global_transform_3d_generation
                .resize(index + 1, 0);
        }
    }

    fn is_global_2d_cache_valid_for_id(&self, id: NodeID) -> bool {
        let index = id.index() as usize;
        if self
            .transforms
            .global_transform_2d_valid
            .get(index)
            .copied()
            .unwrap_or(0)
            == 0
        {
            return false;
        }
        if self
            .transforms
            .global_transform_2d_generation
            .get(index)
            .copied()
            .unwrap_or(u32::MAX)
            != id.generation()
        {
            return false;
        }
        true
    }

    #[inline]
    fn is_global_2d_cached_clean(&self, id: NodeID) -> bool {
        self.is_global_2d_cache_valid_for_id(id)
            && !self.dirty.has_transform_dirty(id, Spatial::TwoD)
    }

    fn is_global_3d_cache_valid_for_id(&self, id: NodeID) -> bool {
        let index = id.index() as usize;
        if self
            .transforms
            .global_transform_3d_valid
            .get(index)
            .copied()
            .unwrap_or(0)
            == 0
        {
            return false;
        }
        if self
            .transforms
            .global_transform_3d_generation
            .get(index)
            .copied()
            .unwrap_or(u32::MAX)
            != id.generation()
        {
            return false;
        }
        true
    }

    #[inline]
    fn is_global_3d_cached_clean(&self, id: NodeID) -> bool {
        self.is_global_3d_cache_valid_for_id(id)
            && !self.dirty.has_transform_dirty(id, Spatial::ThreeD)
    }

    /// Reads the cached global 2D transform without recomputing. Returns
    /// `None` when the cache is stale or missing; callers fall back to
    /// [`Self::get_global_transform_2d`].
    #[inline]
    pub(crate) fn cached_clean_global_2d(&self, id: NodeID) -> Option<Transform2D> {
        if !self.is_global_2d_cached_clean(id) {
            return None;
        }
        self.transforms
            .global_transform_2d
            .get(id.index() as usize)
            .copied()
    }

    /// Reads the cached global 3D transform without recomputing. Returns
    /// `None` when the cache is stale or missing; callers fall back to
    /// [`Self::get_global_transform_3d`].
    #[inline]
    pub(crate) fn cached_clean_global_3d(&self, id: NodeID) -> Option<Transform3D> {
        if !self.is_global_3d_cached_clean(id) {
            return None;
        }
        self.transforms
            .global_transform_3d
            .get(id.index() as usize)
            .copied()
    }

    pub(crate) fn get_global_transform_2d(&mut self, id: NodeID) -> Option<Transform2D> {
        if id.is_nil() || self.nodes.get(id).is_none() {
            return None;
        }
        // pending subtree roots aren't in per-node dirty flags yet; the clean
        // check + chain walk below only read per-node flags, so skipping this
        // returns a stale global (set_local_rot on a node w/ children, then
        // set_global_pos writes the stale rot back over it).
        if self.dirty.has_pending_transform_roots() {
            self.propagate_pending_transform_dirty();
        }
        let start_index = id.index() as usize;
        self.ensure_global_2d_capacity(start_index);
        if self.is_global_2d_cached_clean(id) {
            return self
                .transforms
                .global_transform_2d
                .get(start_index)
                .copied();
        }

        let mut chain = std::mem::take(&mut self.transforms.global_chain_scratch);
        chain.clear();

        let mut cursor = id;
        let mut parent_world = Mat3::IDENTITY;
        let max_hops = self.nodes.len().saturating_add(1);
        let mut hops = 0usize;

        while hops < max_hops {
            let Some(parent) = self
                .nodes
                .get(cursor)
                .and_then(|node| node.with_base_ref::<Node2D, _>(|_| node.parent))
            else {
                break;
            };
            let index = cursor.index() as usize;
            self.ensure_global_2d_capacity(index);
            let dirty = self.dirty.has_transform_dirty(cursor, Spatial::TwoD);
            let cached_valid = self.is_global_2d_cache_valid_for_id(cursor);
            if cached_valid && !dirty {
                parent_world = self.transforms.global_transform_2d[index].to_mat3();
                break;
            }
            chain.push(cursor);

            if parent.is_nil() {
                break;
            }
            if self.nodes.get(parent).is_none() {
                break;
            }
            cursor = parent;
            hops += 1;
        }

        for chain_id in chain.iter().rev().copied() {
            let Some((local, parent)) = self.nodes.get(chain_id).and_then(|node| {
                node.with_base_ref::<Node2D, _>(|base| (base.transform, node.parent))
            }) else {
                continue;
            };
            let parent_is_2d = !parent.is_nil()
                && self
                    .nodes
                    .get(parent)
                    .and_then(|node| node.with_base_ref::<Node2D, _>(|_| ()))
                    .is_some();
            let (global, world) = if parent_is_2d {
                let world = parent_world * local.to_mat3();
                (Transform2D::from_mat3(world), world)
            } else {
                (local, local.to_mat3())
            };
            let index = chain_id.index() as usize;
            self.transforms.global_transform_2d[index] = global;
            self.transforms.global_transform_2d_valid[index] = 1;
            self.transforms.global_transform_2d_generation[index] = chain_id.generation();
            self.dirty.clear_transform_dirty(chain_id, Spatial::TwoD);
            parent_world = world;
        }

        let result = self
            .transforms
            .global_transform_2d
            .get(start_index)
            .copied();
        chain.clear();
        self.transforms.global_chain_scratch = chain;
        result
    }

    pub(crate) fn get_global_transform_3d(&mut self, id: NodeID) -> Option<Transform3D> {
        if id.is_nil() || self.nodes.get(id).is_none() {
            return None;
        }
        // pending subtree roots aren't in per-node dirty flags yet; the clean
        // check + chain walk below only read per-node flags, so skipping this
        // returns a stale global (set_local_rot on a node w/ children, then
        // set_global_pos writes the stale rot back over it).
        if self.dirty.has_pending_transform_roots() {
            self.propagate_pending_transform_dirty();
        }
        let start_index = id.index() as usize;
        self.ensure_global_3d_capacity(start_index);
        if self.is_global_3d_cached_clean(id) {
            return self
                .transforms
                .global_transform_3d
                .get(start_index)
                .copied();
        }

        let mut chain = std::mem::take(&mut self.transforms.global_chain_scratch);
        chain.clear();

        let mut cursor = id;
        let mut parent_world = Mat4::IDENTITY;
        let max_hops = self.nodes.len().saturating_add(1);
        let mut hops = 0usize;

        while hops < max_hops {
            let Some(parent) = self
                .nodes
                .get(cursor)
                .and_then(|node| node.with_base_ref::<Node3D, _>(|_| node.parent))
            else {
                break;
            };
            let index = cursor.index() as usize;
            self.ensure_global_3d_capacity(index);
            let dirty = self.dirty.has_transform_dirty(cursor, Spatial::ThreeD);
            let cached_valid = self.is_global_3d_cache_valid_for_id(cursor);
            if cached_valid && !dirty {
                parent_world = self.transforms.global_transform_3d[index].to_mat4();
                break;
            }
            chain.push(cursor);

            if parent.is_nil() {
                break;
            }
            if self.nodes.get(parent).is_none() {
                break;
            }
            cursor = parent;
            hops += 1;
        }

        for chain_id in chain.iter().rev().copied() {
            let Some((local, parent)) = self.nodes.get(chain_id).and_then(|node| {
                node.with_base_ref::<Node3D, _>(|base| (base.transform, node.parent))
            }) else {
                continue;
            };
            let parent_is_3d = !parent.is_nil()
                && self
                    .nodes
                    .get(parent)
                    .and_then(|node| node.with_base_ref::<Node3D, _>(|_| ()))
                    .is_some();
            let (global, world) = if parent_is_3d {
                let world = parent_world * local.to_mat4();
                (Transform3D::from_mat4(world), world)
            } else {
                (local, local.to_mat4())
            };
            let index = chain_id.index() as usize;
            self.transforms.global_transform_3d[index] = global;
            self.transforms.global_transform_3d_valid[index] = 1;
            self.transforms.global_transform_3d_generation[index] = chain_id.generation();
            self.dirty.clear_transform_dirty(chain_id, Spatial::ThreeD);
            parent_world = world;
        }

        let result = self
            .transforms
            .global_transform_3d
            .get(start_index)
            .copied();
        chain.clear();
        self.transforms.global_chain_scratch = chain;
        result
    }

    pub fn set_physics_render_alpha(&mut self, alpha: f32) {
        let alpha = if alpha.is_finite() {
            alpha.clamp(0.0, 1.0)
        } else {
            1.0
        };
        if (self.transforms.render_alpha - alpha).abs() <= f32::EPSILON {
            return;
        }
        self.transforms.render_alpha = alpha;
        self.mark_physics_interp_rerender_2d();
        self.mark_physics_interp_rerender_3d();
    }

    pub(crate) fn record_physics_pose_2d(
        &mut self,
        id: NodeID,
        parent: NodeID,
        before: Transform2D,
        curr: Transform2D,
    ) {
        let index = id.index() as usize;
        if self.transforms.physics_pose_2d.len() <= index {
            self.transforms
                .physics_pose_2d
                .resize(index + 1, Default::default());
        }
        if self.transforms.physics_pose_id_flags_2d.len() <= index {
            self.transforms
                .physics_pose_id_flags_2d
                .resize(index + 1, 0);
        }
        if self.transforms.physics_pose_id_flags_2d[index] == 0 {
            self.transforms.physics_pose_id_flags_2d[index] = 1;
            self.transforms.physics_pose_ids_2d.push(id);
        }
        let entry = &mut self.transforms.physics_pose_2d[index];
        let snap = !entry.valid
            || entry.generation != id.generation()
            || entry.parent != parent
            || !transform_close_2d(before, entry.curr);
        if snap {
            entry.prev = curr;
        } else {
            entry.prev = entry.curr;
        }
        entry.curr = curr;
        entry.parent = parent;
        entry.generation = id.generation();
        entry.valid = true;
        self.mark_needs_rerender(id);
    }

    pub(crate) fn record_physics_pose_3d(
        &mut self,
        id: NodeID,
        parent: NodeID,
        before: Transform3D,
        curr: Transform3D,
    ) {
        let index = id.index() as usize;
        if self.transforms.physics_pose_3d.len() <= index {
            self.transforms
                .physics_pose_3d
                .resize(index + 1, Default::default());
        }
        if self.transforms.physics_pose_id_flags_3d.len() <= index {
            self.transforms
                .physics_pose_id_flags_3d
                .resize(index + 1, 0);
        }
        if self.transforms.physics_pose_id_flags_3d[index] == 0 {
            self.transforms.physics_pose_id_flags_3d[index] = 1;
            self.transforms.physics_pose_ids_3d.push(id);
        }
        let entry = &mut self.transforms.physics_pose_3d[index];
        let snap = !entry.valid
            || entry.generation != id.generation()
            || entry.parent != parent
            || !transform_close_3d(before, entry.curr);
        if snap {
            entry.prev = curr;
        } else {
            entry.prev = entry.curr;
        }
        entry.curr = curr;
        entry.parent = parent;
        entry.generation = id.generation();
        entry.valid = true;
        self.mark_needs_rerender(id);
    }

    pub(crate) fn get_render_global_transform_2d(&mut self, id: NodeID) -> Option<Transform2D> {
        if id.is_nil() || self.nodes.get(id).is_none() {
            return None;
        }
        if self.transforms.physics_pose_ids_2d.is_empty() {
            return self.get_global_transform_2d(id);
        }
        if let Some(pose) = self.interpolated_physics_pose_2d(id) {
            return Some(pose);
        }
        if let Some((local, parent)) = self
            .nodes
            .get(id)
            .and_then(|node| node.with_base_ref::<Node2D, _>(|base| (base.transform, node.parent)))
            && let Some(parent_pose) = self.interpolated_physics_pose_2d(parent)
        {
            return Some(Transform2D::from_mat3(
                parent_pose.to_mat3() * local.to_mat3(),
            ));
        }
        let mut chain = std::mem::take(&mut self.transforms.global_chain_scratch);
        chain.clear();
        let mut cursor = id;
        let mut has_interp = false;
        let max_hops = self.nodes.len().saturating_add(1);
        for _ in 0..max_hops {
            let Some(node) = self.nodes.get(cursor) else {
                break;
            };
            chain.push(cursor);
            if matches!(node.data, SceneNodeData::RigidBody2D(_))
                && self
                    .transforms
                    .physics_pose_2d
                    .get(cursor.index() as usize)
                    .is_some_and(|pose| pose.valid && pose.generation == cursor.generation())
            {
                has_interp = true;
            }
            let Some(parent) = node.with_base_ref::<Node2D, _>(|_| node.parent) else {
                break;
            };
            if parent.is_nil() {
                break;
            }
            cursor = parent;
        }
        if !has_interp {
            chain.clear();
            self.transforms.global_chain_scratch = chain;
            return self.get_global_transform_2d(id);
        }

        let mut parent_world = Mat3::IDENTITY;
        let mut result = None;
        for chain_id in chain.iter().rev().copied() {
            let Some((local, parent, is_rigid)) = self.nodes.get(chain_id).and_then(|node| {
                node.with_base_ref::<Node2D, _>(|base| {
                    (
                        base.transform,
                        node.parent,
                        matches!(node.data, SceneNodeData::RigidBody2D(_)),
                    )
                })
            }) else {
                continue;
            };
            let global = if is_rigid {
                self.interpolated_physics_pose_2d(chain_id).unwrap_or(local)
            } else {
                let parent_is_2d = !parent.is_nil()
                    && self
                        .nodes
                        .get(parent)
                        .and_then(|node| node.with_base_ref::<Node2D, _>(|_| ()))
                        .is_some();
                if parent_is_2d {
                    Transform2D::from_mat3(parent_world * local.to_mat3())
                } else {
                    local
                }
            };
            parent_world = global.to_mat3();
            result = Some(global);
        }
        chain.clear();
        self.transforms.global_chain_scratch = chain;
        result
    }

    pub(crate) fn get_render_global_transform_3d(&mut self, id: NodeID) -> Option<Transform3D> {
        if id.is_nil() || self.nodes.get(id).is_none() {
            return None;
        }
        if self.transforms.physics_pose_ids_3d.is_empty() {
            return self.get_global_transform_3d(id);
        }
        if let Some(pose) = self.interpolated_physics_pose_3d(id) {
            return Some(pose);
        }
        if let Some((local, parent)) = self
            .nodes
            .get(id)
            .and_then(|node| node.with_base_ref::<Node3D, _>(|base| (base.transform, node.parent)))
            && let Some(parent_pose) = self.interpolated_physics_pose_3d(parent)
        {
            return Some(Transform3D::from_mat4(
                parent_pose.to_mat4() * local.to_mat4(),
            ));
        }
        let mut chain = std::mem::take(&mut self.transforms.global_chain_scratch);
        chain.clear();
        let mut cursor = id;
        let mut has_interp = false;
        let max_hops = self.nodes.len().saturating_add(1);
        for _ in 0..max_hops {
            let Some(node) = self.nodes.get(cursor) else {
                break;
            };
            chain.push(cursor);
            if matches!(node.data, SceneNodeData::RigidBody3D(_))
                && self
                    .transforms
                    .physics_pose_3d
                    .get(cursor.index() as usize)
                    .is_some_and(|pose| pose.valid && pose.generation == cursor.generation())
            {
                has_interp = true;
            }
            let Some(parent) = node.with_base_ref::<Node3D, _>(|_| node.parent) else {
                break;
            };
            if parent.is_nil() {
                break;
            }
            cursor = parent;
        }
        if !has_interp {
            chain.clear();
            self.transforms.global_chain_scratch = chain;
            return self.get_global_transform_3d(id);
        }

        let mut parent_world = Mat4::IDENTITY;
        let mut result = None;
        for chain_id in chain.iter().rev().copied() {
            let Some((local, parent, is_rigid)) = self.nodes.get(chain_id).and_then(|node| {
                node.with_base_ref::<Node3D, _>(|base| {
                    (
                        base.transform,
                        node.parent,
                        matches!(node.data, SceneNodeData::RigidBody3D(_)),
                    )
                })
            }) else {
                continue;
            };
            let global = if is_rigid {
                self.interpolated_physics_pose_3d(chain_id).unwrap_or(local)
            } else {
                let parent_is_3d = !parent.is_nil()
                    && self
                        .nodes
                        .get(parent)
                        .and_then(|node| node.with_base_ref::<Node3D, _>(|_| ()))
                        .is_some();
                if parent_is_3d {
                    Transform3D::from_mat4(parent_world * local.to_mat4())
                } else {
                    local
                }
            };
            parent_world = global.to_mat4();
            result = Some(global);
        }
        chain.clear();
        self.transforms.global_chain_scratch = chain;
        result
    }

    fn interpolated_physics_pose_2d(&self, id: NodeID) -> Option<Transform2D> {
        let entry = self.transforms.physics_pose_2d.get(id.index() as usize)?;
        if !entry.valid || entry.generation != id.generation() {
            return None;
        }
        let mut out = entry.curr;
        let t = self.transforms.render_alpha.clamp(0.0, 1.0);
        out.position = entry.prev.position.lerped(entry.curr.position, t);
        out.rotation = lerp_angle(entry.prev.rotation, entry.curr.rotation, t);
        Some(out)
    }

    fn interpolated_physics_pose_3d(&self, id: NodeID) -> Option<Transform3D> {
        let entry = self.transforms.physics_pose_3d.get(id.index() as usize)?;
        if !entry.valid || entry.generation != id.generation() {
            return None;
        }
        let mut out = entry.curr;
        let t = self.transforms.render_alpha.clamp(0.0, 1.0);
        out.position = entry.prev.position.lerped(entry.curr.position, t);
        out.rotation = entry.prev.rotation.slerped(entry.curr.rotation, t);
        Some(out)
    }

    fn mark_physics_interp_rerender_2d(&mut self) {
        let mut ids = std::mem::take(&mut self.transforms.physics_pose_ids_2d);
        let mut children = std::mem::take(&mut self.transforms.traversal_stack);
        children.clear();
        let mut write = 0usize;
        for read in 0..ids.len() {
            let id = ids[read];
            let index = id.index() as usize;
            if self
                .transforms
                .physics_pose_2d
                .get(index)
                .is_some_and(|pose| {
                    pose.valid && pose.generation == id.generation() && self.nodes.get(id).is_some()
                })
            {
                children.clear();
                if let Some(node) = self.nodes.get(id) {
                    children.extend_from_slice(node.children_slice());
                }
                if children.is_empty() {
                    self.mark_needs_rerender(id);
                } else {
                    for child in children.iter().copied() {
                        self.mark_needs_rerender(child);
                    }
                }
                ids[write] = id;
                write += 1;
            } else if index < self.transforms.physics_pose_id_flags_2d.len() {
                self.transforms.physics_pose_id_flags_2d[index] = 0;
            }
        }
        children.clear();
        self.transforms.traversal_stack = children;
        ids.truncate(write);
        self.transforms.physics_pose_ids_2d = ids;
    }

    fn mark_physics_interp_rerender_3d(&mut self) {
        let mut ids = std::mem::take(&mut self.transforms.physics_pose_ids_3d);
        let mut children = std::mem::take(&mut self.transforms.traversal_stack);
        children.clear();
        let mut write = 0usize;
        for read in 0..ids.len() {
            let id = ids[read];
            let index = id.index() as usize;
            if self
                .transforms
                .physics_pose_3d
                .get(index)
                .is_some_and(|pose| {
                    pose.valid && pose.generation == id.generation() && self.nodes.get(id).is_some()
                })
            {
                children.clear();
                if let Some(node) = self.nodes.get(id) {
                    children.extend_from_slice(node.children_slice());
                }
                if children.is_empty() {
                    self.mark_needs_rerender(id);
                } else {
                    for child in children.iter().copied() {
                        self.mark_needs_rerender(child);
                    }
                }
                ids[write] = id;
                write += 1;
            } else if index < self.transforms.physics_pose_id_flags_3d.len() {
                self.transforms.physics_pose_id_flags_3d[index] = 0;
            }
        }
        children.clear();
        self.transforms.traversal_stack = children;
        ids.truncate(write);
        self.transforms.physics_pose_ids_3d = ids;
    }

    pub(crate) fn refresh_dirty_global_transforms(&mut self) {
        if !self.dirty.has_transform_dirty_any() {
            return;
        }
        let mut dirty_indices = std::mem::take(&mut self.transforms.dirty_indices_scratch);
        dirty_indices.clear();
        dirty_indices.extend_from_slice(self.dirty.dirty_indices());
        for raw_index in dirty_indices.iter().copied() {
            let index = raw_index as usize;
            let flags = self.dirty.transform_flags_at(index);
            if flags == 0 {
                continue;
            }
            let Some((id, _)) = self.nodes.slot_get(index) else {
                self.dirty.clear_transform_dirty_at_index(
                    index,
                    DirtyState::FLAG_DIRTY_2D_TRANSFORM | DirtyState::FLAG_DIRTY_3D_TRANSFORM,
                );
                continue;
            };

            if (flags & DirtyState::FLAG_DIRTY_2D_TRANSFORM) != 0 {
                let _ = self.get_global_transform_2d(id);
            }
            if (flags & DirtyState::FLAG_DIRTY_3D_TRANSFORM) != 0 {
                let _ = self.get_global_transform_3d(id);
            }
        }
        dirty_indices.clear();
        self.transforms.dirty_indices_scratch = dirty_indices;
    }
}

fn transform_close_2d(a: Transform2D, b: Transform2D) -> bool {
    vec2_close(a.position, b.position, PHYSICS_POSE_EPS_SQ_2D)
        && angle_delta(a.rotation, b.rotation).abs() <= PHYSICS_POSE_ROT_EPS
}

fn transform_close_3d(a: Transform3D, b: Transform3D) -> bool {
    vec3_close(a.position, b.position, PHYSICS_POSE_EPS_SQ_3D) && quat_close(a.rotation, b.rotation)
}

fn vec2_close(a: Vector2, b: Vector2, eps_sq: f32) -> bool {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy <= eps_sq
}

fn vec3_close(a: Vector3, b: Vector3, eps_sq: f32) -> bool {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    dx * dx + dy * dy + dz * dz <= eps_sq
}

fn quat_close(a: Quaternion, b: Quaternion) -> bool {
    let dot = a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w;
    (1.0 - dot.abs()).abs() <= PHYSICS_POSE_ROT_EPS
}

fn lerp_angle(from: f32, to: f32, t: f32) -> f32 {
    from + angle_delta(from, to) * t
}

fn angle_delta(from: f32, to: f32) -> f32 {
    let mut delta = (to - from) % std::f32::consts::TAU;
    if delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    } else if delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    delta
}
