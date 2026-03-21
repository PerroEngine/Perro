use super::Runtime;
use crate::runtime::state::DirtyState;
use glam::{Mat3, Mat4};
use perro_ids::NodeID;
use perro_nodes::{Node2D, Node3D, Spatial};
use perro_structs::{Transform2D, Transform3D};

impl Runtime {
    pub(crate) fn propagate_pending_transform_dirty(&mut self) {
        let mut roots = std::mem::take(&mut self.transforms.pending_transform_roots);
        self.dirty.take_pending_transform_roots(&mut roots);
        if roots.is_empty() {
            self.transforms.pending_transform_roots = roots;
            return;
        }

        let mut stack = std::mem::take(&mut self.transforms.traversal_stack);
        stack.clear();

        for root in roots.iter().copied() {
            if self.nodes.get(root).is_none() {
                continue;
            }
            stack.push(root);
            while let Some(id) = stack.pop() {
                let index = id.index() as usize;
                if self.transforms.transform_visit_flags.len() <= index {
                    self.transforms.transform_visit_flags.resize(index + 1, 0);
                }
                if self.transforms.transform_visit_flags[index] != 0 {
                    continue;
                }
                self.transforms.transform_visit_flags[index] = 1;
                self.transforms.transform_visit_indices.push(index as u32);

                let Some(node) = self.nodes.get(id) else {
                    continue;
                };
                self.dirty.mark_transform(id, node.spatial());
                stack.extend(node.children_slice().iter().copied());
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

    fn is_global_2d_cached_clean(&self, id: NodeID) -> bool {
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
        !self.dirty.has_transform_dirty(id, Spatial::TwoD)
    }

    fn is_global_3d_cached_clean(&self, id: NodeID) -> bool {
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
        !self.dirty.has_transform_dirty(id, Spatial::ThreeD)
    }

    pub(crate) fn get_global_transform_2d(&mut self, id: NodeID) -> Option<Transform2D> {
        if id.is_nil() || self.nodes.get(id).is_none() {
            return None;
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
            let Some((parent, _local)) = self.nodes.get(cursor).and_then(|node| {
                node.with_base_ref::<Node2D, _>(|base| (node.parent, base.transform))
            }) else {
                break;
            };
            let index = cursor.index() as usize;
            self.ensure_global_2d_capacity(index);
            let dirty = self.dirty.has_transform_dirty(cursor, Spatial::TwoD);
            let cached_clean = self.is_global_2d_cached_clean(cursor);
            if cached_clean && !dirty {
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
            self.ensure_global_2d_capacity(index);
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
            let Some((parent, _local)) = self.nodes.get(cursor).and_then(|node| {
                node.with_base_ref::<Node3D, _>(|base| (node.parent, base.transform))
            }) else {
                break;
            };
            let index = cursor.index() as usize;
            self.ensure_global_3d_capacity(index);
            let dirty = self.dirty.has_transform_dirty(cursor, Spatial::ThreeD);
            let cached_clean = self.is_global_3d_cached_clean(cursor);
            if cached_clean && !dirty {
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
            self.ensure_global_3d_capacity(index);
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

    pub(crate) fn refresh_dirty_global_transforms(&mut self) {
        let dirty_indices = self.dirty.dirty_indices().to_vec();
        for raw_index in dirty_indices {
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
    }
}
