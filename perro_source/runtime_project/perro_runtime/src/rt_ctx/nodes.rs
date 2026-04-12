use perro_ids::{IntoTagID, MaterialID, TagID};
use perro_nodes::{
    Node2D, Node3D, NodeBaseDispatch, NodeType, NodeTypeDispatch, Renderable, SceneNode,
    SceneNodeData,
};
use perro_runtime_context::sub_apis::{MeshMaterialRegion3D, MeshSurfaceHit3D, NodeAPI, TagQuery};
use perro_structs::{Transform2D, Transform3D, Vector2, Vector3};
use std::borrow::Cow;

use crate::Runtime;

#[inline]
fn cached_slot_for(runtime: &mut Runtime, id: perro_ids::NodeID) -> Option<(usize, u32)> {
    if id.is_nil() {
        return None;
    }

    if let Some(&(_, active_id)) = runtime.script_runtime.active_script_stack.last()
        && active_id == id
    {
        let resolved = (active_id.index() as usize, active_id.generation());
        runtime.script_runtime.last_node_lookup = Some((active_id, resolved.0, resolved.1));
        return Some(resolved);
    }

    if let Some((cached_id, cached_index, cached_generation)) =
        runtime.script_runtime.last_node_lookup
        && cached_id == id
        && runtime
            .nodes
            .slot_get_checked(cached_index, cached_generation)
            .is_some()
    {
        return Some((cached_index, cached_generation));
    }

    let resolved = (id.index() as usize, id.generation());
    if runtime
        .nodes
        .slot_get_checked(resolved.0, resolved.1)
        .is_some()
    {
        runtime.script_runtime.last_node_lookup = Some((id, resolved.0, resolved.1));
        return Some(resolved);
    }

    runtime.script_runtime.last_node_lookup = None;
    None
}

impl NodeAPI for Runtime {
    fn create<T>(&mut self) -> perro_ids::NodeID
    where
        T: Default + Into<SceneNodeData>,
    {
        let id = self.nodes.insert(SceneNode::new(T::default().into()));
        if let Some(node) = self.nodes.get(id) {
            let ty = node.node_type();
            self.register_internal_node_schedules(id, ty);
        }
        id
    }

    fn with_node_mut<T, V, F>(&mut self, id: perro_ids::NodeID, f: F) -> Option<V>
    where
        T: NodeTypeDispatch,
        F: FnOnce(&mut T) -> V,
    {
        if id.is_nil() {
            return None;
        }

        let slot = cached_slot_for(self, id);
        let (transform_changed, value) = {
            let node = if let Some((index, generation)) = slot {
                self.nodes.slot_get_mut_checked(index, generation)?
            } else {
                self.nodes.get_mut(id)?
            };

            let mut changed = false;
            let mut value = None;
            let result = node.with_typed_mut::<T, _>(|typed| {
                let before = T::snapshot_transform(typed);
                value = Some(f(typed));
                let after = T::snapshot_transform(typed);
                changed = before != after;
            });
            result?;
            (changed, value)
        };

        if matches!(T::RENDERABLE, Renderable::True) {
            self.mark_needs_rerender(id);
        }
        if transform_changed {
            self.mark_transform_dirty_recursive(id);
        }
        value
    }

    fn with_node<T, V: Clone + Default>(
        &mut self,
        node_id: perro_ids::NodeID,
        f: impl FnOnce(&T) -> V,
    ) -> V
    where
        T: NodeTypeDispatch,
    {
        if node_id.is_nil() {
            return V::default();
        }

        let node_ref = if let Some((index, generation)) = cached_slot_for(self, node_id) {
            self.nodes.slot_get_checked(index, generation)
        } else {
            self.nodes.get(node_id)
        };
        let Some(node_ref) = node_ref else {
            return V::default();
        };

        node_ref.with_typed_ref::<T, _>(f).unwrap_or_default()
    }

    fn with_base_node<T, V, F>(&mut self, id: perro_ids::NodeID, f: F) -> Option<V>
    where
        T: NodeBaseDispatch,
        F: FnOnce(&T) -> V,
    {
        if id.is_nil() {
            return None;
        }
        let node = if let Some((index, generation)) = cached_slot_for(self, id) {
            self.nodes.slot_get_checked(index, generation)?
        } else {
            self.nodes.get(id)?
        };
        if !node.node_type().is_a(T::BASE_NODE_TYPE) {
            return None;
        }
        node.with_base_ref::<T, _>(f)
    }

    fn with_base_node_mut<T, V, F>(&mut self, id: perro_ids::NodeID, f: F) -> Option<V>
    where
        T: NodeBaseDispatch,
        F: FnOnce(&mut T) -> V,
    {
        if id.is_nil() {
            return None;
        }

        let slot = cached_slot_for(self, id);
        if let Some((index, generation)) = slot {
            let node = self.nodes.slot_get_checked(index, generation)?;
            if !node.node_type().is_a(T::BASE_NODE_TYPE) {
                return None;
            }
        } else if let Some(node) = self.nodes.get(id) {
            if !node.node_type().is_a(T::BASE_NODE_TYPE) {
                return None;
            }
        } else {
            return None;
        }

        let (value, transform_changed) = {
            let node = if let Some((index, generation)) = slot {
                self.nodes.slot_get_mut_checked(index, generation)?
            } else {
                self.nodes.get_mut(id)?
            };
            let before_2d = node.with_base_ref::<Node2D, _>(|base| base.transform);
            let before_3d = node.with_base_ref::<Node3D, _>(|base| base.transform);
            let value = node.with_base_mut::<T, _>(f)?;
            let after_2d = node.with_base_ref::<Node2D, _>(|base| base.transform);
            let after_3d = node.with_base_ref::<Node3D, _>(|base| base.transform);
            let changed = before_2d != after_2d || before_3d != after_3d;
            (value, changed)
        };

        self.mark_needs_rerender(id);
        if transform_changed {
            self.mark_transform_dirty_recursive(id);
        }
        Some(value)
    }

    fn get_node_name(&mut self, node_id: perro_ids::NodeID) -> Option<Cow<'static, str>> {
        self.nodes.get(node_id).map(|node| node.name.clone())
    }

    fn set_node_name<S>(&mut self, node_id: perro_ids::NodeID, name: S) -> bool
    where
        S: Into<Cow<'static, str>>,
    {
        let Some(node) = self.nodes.get_mut(node_id) else {
            return false;
        };
        node.set_name(name);
        true
    }

    fn get_node_parent_id(&mut self, node_id: perro_ids::NodeID) -> Option<perro_ids::NodeID> {
        self.nodes.get(node_id).map(|node| node.get_parent())
    }

    fn get_node_children_ids(
        &mut self,
        node_id: perro_ids::NodeID,
    ) -> Option<Vec<perro_ids::NodeID>> {
        self.nodes
            .get(node_id)
            .map(|node| node.get_children_ids().to_vec())
    }

    fn get_node_type(&mut self, node_id: perro_ids::NodeID) -> Option<NodeType> {
        self.nodes.get(node_id).map(|node| node.node_type())
    }

    fn reparent(&mut self, parent_id: perro_ids::NodeID, child_id: perro_ids::NodeID) -> bool {
        enum SpatialGlobal {
            TwoD(Transform2D),
            ThreeD(Transform3D),
            None,
        }

        if child_id.is_nil() {
            return false;
        }
        if !parent_id.is_nil() && self.nodes.get(parent_id).is_none() {
            return false;
        }

        let old_parent = match self.nodes.get(child_id) {
            Some(node) => node.get_parent(),
            None => return false,
        };

        if old_parent == parent_id {
            return true;
        }

        let child_global = if self
            .nodes
            .get(child_id)
            .and_then(|node| node.with_base_ref::<Node2D, _>(|_| ()))
            .is_some()
        {
            Runtime::get_global_transform_2d(self, child_id)
                .map(SpatialGlobal::TwoD)
                .unwrap_or(SpatialGlobal::None)
        } else if self
            .nodes
            .get(child_id)
            .and_then(|node| node.with_base_ref::<Node3D, _>(|_| ()))
            .is_some()
        {
            Runtime::get_global_transform_3d(self, child_id)
                .map(SpatialGlobal::ThreeD)
                .unwrap_or(SpatialGlobal::None)
        } else {
            SpatialGlobal::None
        };

        if !old_parent.is_nil()
            && let Some(parent) = self.nodes.get_mut(old_parent)
        {
            parent.remove_child(child_id);
        }

        if let Some(child) = self.nodes.get_mut(child_id) {
            child.parent = parent_id;
        } else {
            return false;
        }

        if !parent_id.is_nil() {
            if let Some(parent) = self.nodes.get_mut(parent_id) {
                if !parent.get_children_ids().contains(&child_id) {
                    parent.add_child(child_id);
                }
            } else {
                return false;
            }
        }

        match child_global {
            SpatialGlobal::TwoD(global) => {
                let parent_global = if parent_id.is_nil() {
                    None
                } else {
                    self.nodes
                        .get(parent_id)
                        .and_then(|node| node.with_base_ref::<Node2D, _>(|_| ()))
                        .and_then(|_| Runtime::get_global_transform_2d(self, parent_id))
                };
                let local = match parent_global {
                    Some(parent_global) => {
                        let local_mat = parent_global.to_mat3().inverse() * global.to_mat3();
                        Transform2D::from_mat3(local_mat)
                    }
                    None => global,
                };
                if let Some(child) = self.nodes.get_mut(child_id) {
                    let _ = child.with_base_mut::<Node2D, _>(|node| {
                        node.transform = local;
                    });
                }
            }
            SpatialGlobal::ThreeD(global) => {
                let parent_global = if parent_id.is_nil() {
                    None
                } else {
                    self.nodes
                        .get(parent_id)
                        .and_then(|node| node.with_base_ref::<Node3D, _>(|_| ()))
                        .and_then(|_| Runtime::get_global_transform_3d(self, parent_id))
                };
                let local = match parent_global {
                    Some(parent_global) => {
                        let local_mat = parent_global.to_mat4().inverse() * global.to_mat4();
                        let local = Transform3D::from_mat4(local_mat);
                        // Detect affine shear (or other non-TRS artifacts) that cannot be
                        // represented exactly by Transform3D's TRS fields.
                        let reconstructed = local.to_mat4();
                        let a = local_mat.to_cols_array();
                        let b = reconstructed.to_cols_array();
                        let mut max_abs_err = 0.0_f32;
                        for i in 0..16 {
                            let d = (a[i] - b[i]).abs();
                            if d > max_abs_err {
                                max_abs_err = d;
                            }
                        }
                        if max_abs_err > 1.0e-3 {
                            println!(
                                "[runtime][warn] reparent({} -> {}): non-TRS local transform detected (shear/affine), max reconstruction error = {:.6}. Visual distortion may occur; use a uniform-scale attachment parent/socket.",
                                child_id.as_u64(),
                                parent_id.as_u64(),
                                max_abs_err
                            );
                        }
                        local
                    }
                    None => global,
                };
                if let Some(child) = self.nodes.get_mut(child_id) {
                    let _ = child.with_base_mut::<Node3D, _>(|node| {
                        node.transform = local;
                    });
                }
            }
            SpatialGlobal::None => {}
        }

        self.mark_transform_dirty_recursive(child_id);
        true
    }

    fn reparent_multi<I>(&mut self, parent_id: perro_ids::NodeID, child_ids: I) -> usize
    where
        I: IntoIterator<Item = perro_ids::NodeID>,
    {
        let mut updated = 0_usize;
        for child_id in child_ids {
            if self.reparent(parent_id, child_id) {
                updated += 1;
            }
        }
        updated
    }

    fn remove_node(&mut self, node_id: perro_ids::NodeID) -> bool {
        if node_id.is_nil() || self.nodes.get(node_id).is_none() {
            return false;
        }

        // Gather subtree ids iteratively to avoid recursion depth issues.
        // We delete in post-order so children are removed before their parents.
        let mut stack = std::mem::take(&mut self.node_api_scratch.remove_stack);
        let mut postorder = std::mem::take(&mut self.node_api_scratch.remove_postorder);
        let mut visited = std::mem::take(&mut self.node_api_scratch.remove_visited);
        stack.clear();
        postorder.clear();
        visited.clear();
        stack.push(node_id);
        while let Some(current) = stack.pop() {
            if !visited.insert(current) {
                continue;
            }
            let Some(node) = self.nodes.get(current) else {
                continue;
            };
            postorder.push(current);
            stack.extend(node.get_children_ids().iter().copied());
        }

        for current in postorder.iter().rev().copied() {
            if self.nodes.get(current).is_none() {
                continue;
            }

            // Remove script state first so script-side lookups cannot outlive node removal.
            let _ = self.remove_script_instance(current);

            let parent_id = match self.nodes.get(current) {
                Some(node) => {
                    for &tag in node.tags_slice() {
                        let mut remove_entry = false;
                        if let Some(set) = self.node_index.node_tag_index.get_mut(&tag) {
                            set.remove(&current);
                            remove_entry = set.is_empty();
                        }
                        if remove_entry {
                            self.node_index.node_tag_index.remove(&tag);
                        }
                    }
                    node.get_parent()
                }
                None => continue,
            };

            if !parent_id.is_nil()
                && let Some(parent) = self.nodes.get_mut(parent_id)
            {
                parent.remove_child(current);
            }

            self.unregister_internal_node_schedules(current);
            let _ = self.nodes.remove(current);
        }

        stack.clear();
        postorder.clear();
        visited.clear();
        self.node_api_scratch.remove_stack = stack;
        self.node_api_scratch.remove_postorder = postorder;
        self.node_api_scratch.remove_visited = visited;

        true
    }

    fn get_node_tags(&mut self, node_id: perro_ids::NodeID) -> Option<Vec<TagID>> {
        self.nodes
            .get(node_id)
            .map(|node| node.tags_slice().to_vec())
    }

    fn tag_set<T>(&mut self, node_id: perro_ids::NodeID, tags: Option<T>) -> bool
    where
        T: Into<Cow<'static, [TagID]>>,
    {
        let old_tags = match self.nodes.get(node_id) {
            Some(node) => node.tags_slice().to_vec(),
            None => return false,
        };

        let Some(node) = self.nodes.get_mut(node_id) else {
            return false;
        };
        if let Some(tags) = tags {
            node.set_tag_ids(Some(tags));
        } else {
            node.clear_tags();
        }
        let new_tags = node.tags_slice().to_vec();

        for tag in old_tags {
            if !new_tags.contains(&tag)
                && let Some(set) = self.node_index.node_tag_index.get_mut(&tag)
            {
                set.remove(&node_id);
                let remove_entry = set.is_empty();
                if remove_entry {
                    self.node_index.node_tag_index.remove(&tag);
                }
            }
        }
        for tag in new_tags {
            self.node_index
                .node_tag_index
                .entry(tag)
                .or_default()
                .insert(node_id);
        }
        true
    }

    fn add_node_tag<T>(&mut self, node_id: perro_ids::NodeID, tag: T) -> bool
    where
        T: IntoTagID,
    {
        let Some(node) = self.nodes.get_mut(node_id) else {
            return false;
        };
        let tag = tag.into_tag_id();
        let mut added = false;
        if !node.has_tag(tag) {
            node.add_tag(tag);
            added = true;
        }
        if added {
            self.node_index
                .node_tag_index
                .entry(tag)
                .or_default()
                .insert(node_id);
        }
        true
    }

    fn remove_node_tag<T>(&mut self, node_id: perro_ids::NodeID, tag: T) -> bool
    where
        T: IntoTagID,
    {
        let Some(node) = self.nodes.get_mut(node_id) else {
            return false;
        };
        let tag = tag.into_tag_id();
        if node.has_tag(tag) {
            node.remove_tag(tag);
            if let Some(set) = self.node_index.node_tag_index.get_mut(&tag) {
                set.remove(&node_id);
                let remove_entry = set.is_empty();
                if remove_entry {
                    self.node_index.node_tag_index.remove(&tag);
                }
            }
        }
        true
    }

    fn query_nodes(&mut self, query: TagQuery) -> Vec<perro_ids::NodeID> {
        super::query::query_node_ids(&self.nodes, query, Some(&self.node_index.node_tag_index))
    }

    fn get_global_transform_2d(&mut self, node_id: perro_ids::NodeID) -> Option<Transform2D> {
        Runtime::get_global_transform_2d(self, node_id)
    }

    fn get_global_transform_3d(&mut self, node_id: perro_ids::NodeID) -> Option<Transform3D> {
        Runtime::get_global_transform_3d(self, node_id)
    }

    fn set_global_transform_2d(&mut self, node_id: perro_ids::NodeID, global: Transform2D) -> bool {
        let parent = match self.nodes.get(node_id) {
            Some(node) => node.parent,
            None => return false,
        };
        let parent_global = if parent.is_nil() {
            None
        } else {
            self.nodes
                .get(parent)
                .and_then(|node| node.with_base_ref::<Node2D, _>(|_| ()))
                .and_then(|_| Runtime::get_global_transform_2d(self, parent))
        };
        let local = match parent_global {
            Some(parent_global) => {
                let local_mat = parent_global.to_mat3().inverse() * global.to_mat3();
                Transform2D::from_mat3(local_mat)
            }
            None => global,
        };
        self.with_base_node_mut::<Node2D, _, _>(node_id, |node| {
            node.transform = local;
        })
        .is_some()
    }

    fn set_global_transform_3d(&mut self, node_id: perro_ids::NodeID, global: Transform3D) -> bool {
        let parent = match self.nodes.get(node_id) {
            Some(node) => node.parent,
            None => return false,
        };
        let parent_global = if parent.is_nil() {
            None
        } else {
            self.nodes
                .get(parent)
                .and_then(|node| node.with_base_ref::<Node3D, _>(|_| ()))
                .and_then(|_| Runtime::get_global_transform_3d(self, parent))
        };
        let local = match parent_global {
            Some(parent_global) => {
                let local_mat = parent_global.to_mat4().inverse() * global.to_mat4();
                Transform3D::from_mat4(local_mat)
            }
            None => global,
        };
        self.with_base_node_mut::<Node3D, _, _>(node_id, |node| {
            node.transform = local;
        })
        .is_some()
    }

    fn to_global_point_2d(
        &mut self,
        node_id: perro_ids::NodeID,
        local: Vector2,
    ) -> Option<Vector2> {
        let global = Runtime::get_global_transform_2d(self, node_id)?;
        let p = global.to_mat3() * glam::Vec3::new(local.x, local.y, 1.0);
        Some(Vector2::new(p.x, p.y))
    }

    fn to_local_point_2d(
        &mut self,
        node_id: perro_ids::NodeID,
        global: Vector2,
    ) -> Option<Vector2> {
        let basis = Runtime::get_global_transform_2d(self, node_id)?
            .to_mat3()
            .inverse();
        let p = basis * glam::Vec3::new(global.x, global.y, 1.0);
        Some(Vector2::new(p.x, p.y))
    }

    fn to_global_point_3d(
        &mut self,
        node_id: perro_ids::NodeID,
        local: Vector3,
    ) -> Option<Vector3> {
        let global = Runtime::get_global_transform_3d(self, node_id)?;
        let p = global.to_mat4().transform_point3(local.into());
        Some(p.into())
    }

    fn to_local_point_3d(
        &mut self,
        node_id: perro_ids::NodeID,
        global: Vector3,
    ) -> Option<Vector3> {
        let basis = Runtime::get_global_transform_3d(self, node_id)?
            .to_mat4()
            .inverse();
        let p = basis.transform_point3(global.into());
        Some(p.into())
    }

    fn to_global_transform_2d(
        &mut self,
        node_id: perro_ids::NodeID,
        local: Transform2D,
    ) -> Option<Transform2D> {
        let global_basis = Runtime::get_global_transform_2d(self, node_id)?.to_mat3();
        let world = global_basis * local.to_mat3();
        Some(Transform2D::from_mat3(world))
    }

    fn to_local_transform_2d(
        &mut self,
        node_id: perro_ids::NodeID,
        global: Transform2D,
    ) -> Option<Transform2D> {
        let inv_basis = Runtime::get_global_transform_2d(self, node_id)?
            .to_mat3()
            .inverse();
        let local = inv_basis * global.to_mat3();
        Some(Transform2D::from_mat3(local))
    }

    fn to_global_transform_3d(
        &mut self,
        node_id: perro_ids::NodeID,
        local: Transform3D,
    ) -> Option<Transform3D> {
        let global_basis = Runtime::get_global_transform_3d(self, node_id)?.to_mat4();
        let world = global_basis * local.to_mat4();
        Some(Transform3D::from_mat4(world))
    }

    fn to_local_transform_3d(
        &mut self,
        node_id: perro_ids::NodeID,
        global: Transform3D,
    ) -> Option<Transform3D> {
        let inv_basis = Runtime::get_global_transform_3d(self, node_id)?
            .to_mat4()
            .inverse();
        let local = inv_basis * global.to_mat4();
        Some(Transform3D::from_mat4(local))
    }

    fn mesh_surface_at_world_point(
        &mut self,
        node_id: perro_ids::NodeID,
        world_point: Vector3,
    ) -> Option<MeshSurfaceHit3D> {
        self.query_mesh_surface_at_world_point(node_id, world_point)
    }

    fn mesh_material_regions(
        &mut self,
        node_id: perro_ids::NodeID,
        material: MaterialID,
    ) -> Vec<MeshMaterialRegion3D> {
        self.query_mesh_material_regions(node_id, material)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/rt_ctx_nodes_transform_api_tests.rs"]
mod tests;
