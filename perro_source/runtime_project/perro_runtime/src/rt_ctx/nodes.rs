//! Runtime node API implementation.
//!
//! API methods stay here. Node creation prep, UI dirty classification, and
//! small helper scans live in `nodes/helpers.rs`.

use perro_ids::{IntoTagID, MaterialID, NodeID, TagID};
use perro_nodes::{
    Node2D, Node3D, NodeBaseDispatch, NodeType, NodeTypeDispatch, Renderable, SceneNode,
    SceneNodeData, UiBox,
};
use perro_runtime_api::sub_apis::{
    IntoNodeTag, IntoNodeTags, MeshDataSurfaceHit3D, MeshDataSurfaceRegion3D, MeshMaterialRegion3D,
    MeshSurfaceHit3D, MeshSurfaceRay3D, NodeAPI, NodeCreationTemplate, NodeQueryView,
};
use perro_structs::{Transform2D, Transform3D, Vector2, Vector3};
use rayon::prelude::*;
use std::borrow::Cow;

use crate::Runtime;

const CREATE_NODES_PARALLEL_MIN: usize = 16_384;

mod helpers;
use helpers::*;

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
        if self.nodes.get(id).is_some_and(
            |node| matches!(&node.data, SceneNodeData::Camera3D(camera) if camera.active),
        ) {
            self.note_camera_3d_activated(id);
        }
        // Ensure freshly created nodes participate in render/transform extraction
        // even before caller-side mutation paths run.
        self.mark_needs_rerender(id);
        self.mark_transform_dirty_recursive(id);
        id
    }

    fn create_nodes(
        &mut self,
        requests: &[NodeCreationTemplate],
        parent_id: perro_ids::NodeID,
    ) -> Vec<perro_ids::NodeID> {
        if !parent_id.is_nil() && self.nodes.get(parent_id).is_none() {
            return Vec::new();
        }

        self.nodes.reserve(requests.len());

        let mut ids = Vec::with_capacity(requests.len());
        if requests.len() >= CREATE_NODES_PARALLEL_MIN {
            let prepared: Vec<PreparedNode> = requests
                .par_iter()
                .map(|request| prepare_created_node(request, parent_id))
                .collect();

            for prepared in prepared {
                let id = self.nodes.insert(prepared.node);
                ids.push(id);

                self.register_internal_node_schedules(id, prepared.node_type);
                if self.nodes.get(id).is_some_and(
                    |node| matches!(&node.data, SceneNodeData::Camera3D(camera) if camera.active),
                ) {
                    self.note_camera_3d_activated(id);
                }
                self.mark_needs_rerender(id);
                if parent_id.is_nil() {
                    self.mark_transform_dirty_recursive(id);
                }
                for tag in prepared.tag_ids {
                    self.node_index
                        .node_tag_index
                        .entry(tag)
                        .or_default()
                        .insert(id);
                }
            }
        } else {
            for request in requests {
                let prepared = prepare_created_node(request, parent_id);
                let id = self.nodes.insert(prepared.node);
                ids.push(id);

                self.register_internal_node_schedules(id, prepared.node_type);
                if self.nodes.get(id).is_some_and(
                    |node| matches!(&node.data, SceneNodeData::Camera3D(camera) if camera.active),
                ) {
                    self.note_camera_3d_activated(id);
                }
                self.mark_needs_rerender(id);
                if parent_id.is_nil() {
                    self.mark_transform_dirty_recursive(id);
                }
                for tag in prepared.tag_ids {
                    self.node_index
                        .node_tag_index
                        .entry(tag)
                        .or_default()
                        .insert(id);
                }
            }
        }

        if !parent_id.is_nil() {
            if let Some(parent) = self.nodes.get_mut(parent_id) {
                parent.children.reserve(ids.len());
                parent.children.extend(ids.iter().copied());
            }
            self.mark_transform_dirty_recursive(parent_id);
            let parent_ui_ancestor = self.closest_ui_ancestor(parent_id);
            for &id in &ids {
                let child_is_ui = self
                    .nodes
                    .get(id)
                    .and_then(|node| ui_base_from_data(&node.data))
                    .is_some();
                if child_is_ui || parent_ui_ancestor.is_some() {
                    self.mark_ui_reparent_dirty(id, perro_ids::NodeID::nil(), parent_id);
                }
            }
        }

        ids
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
        let (
            transform_changed,
            ui_before,
            ui_after,
            camera_2d_changed,
            camera_3d_changed,
            camera_3d_activated,
            value,
        ) = {
            let node = if let Some((index, generation)) = slot {
                self.nodes.slot_get_mut_checked(index, generation)?
            } else {
                self.nodes.get_mut(id)?
            };

            let track_ui = T::NODE_TYPE.is_a(NodeType::UiBox);
            let track_camera_2d = T::NODE_TYPE == NodeType::Camera2D;
            let track_camera_3d = T::NODE_TYPE == NodeType::Camera3D;
            let ui_before = track_ui.then(|| node.data.clone());
            let cam_2d_before = if track_camera_2d {
                match &node.data {
                    SceneNodeData::Camera2D(cam) => Some((cam.active, cam.transform, cam.zoom)),
                    _ => None,
                }
            } else {
                None
            };
            let cam_3d_before = if track_camera_3d {
                match &node.data {
                    SceneNodeData::Camera3D(cam) => Some(cam.active),
                    _ => None,
                }
            } else {
                None
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
            let ui_after = track_ui.then(|| node.data.clone());
            let cam_2d_after = if track_camera_2d {
                match &node.data {
                    SceneNodeData::Camera2D(cam) => Some((cam.active, cam.transform, cam.zoom)),
                    _ => None,
                }
            } else {
                None
            };
            let cam_3d_after = if track_camera_3d {
                match &node.data {
                    SceneNodeData::Camera3D(cam) => Some(cam.active),
                    _ => None,
                }
            } else {
                None
            };
            (
                changed,
                ui_before,
                ui_after,
                cam_2d_before != cam_2d_after,
                cam_3d_before != cam_3d_after,
                cam_3d_before != Some(true) && cam_3d_after == Some(true),
                value,
            )
        };

        if matches!(T::RENDERABLE, Renderable::True) {
            self.mark_needs_rerender(id);
        }
        if transform_changed {
            self.mark_transform_dirty_recursive(id);
        }
        if camera_2d_changed {
            self.request_full_2d_scan_once();
        }
        if camera_3d_changed {
            self.request_full_3d_scan_once();
        }
        if camera_3d_activated {
            self.note_camera_3d_activated(id);
        }
        if let (Some(before), Some(after)) = (ui_before.as_ref(), ui_after.as_ref()) {
            self.mark_ui_data_change(id, before, after);
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

        let (
            value,
            transform_changed,
            ui_before,
            ui_after,
            vis_2d_changed,
            vis_3d_changed,
            active_camera_2d_changed,
            active_camera_3d_changed,
            active_camera_3d_activated,
        ) = {
            let node = if let Some((index, generation)) = slot {
                self.nodes.slot_get_mut_checked(index, generation)?
            } else {
                self.nodes.get_mut(id)?
            };
            let before_2d = node.with_base_ref::<Node2D, _>(|base| base.transform);
            let before_3d = node.with_base_ref::<Node3D, _>(|base| base.transform);
            let before_vis_2d = node.with_base_ref::<Node2D, _>(|base| base.visible);
            let before_vis_3d = node.with_base_ref::<Node3D, _>(|base| base.visible);
            let before_camera_2d = match &node.data {
                SceneNodeData::Camera2D(camera) if camera.active => Some(camera.transform),
                _ => None,
            };
            let before_camera_3d = match &node.data {
                SceneNodeData::Camera3D(camera) if camera.active => Some(camera.transform),
                _ => None,
            };
            let ui_before = node.with_base_ref::<UiBox, _>(Clone::clone);
            let value = node.with_base_mut::<T, _>(f)?;
            let after_2d = node.with_base_ref::<Node2D, _>(|base| base.transform);
            let after_3d = node.with_base_ref::<Node3D, _>(|base| base.transform);
            let after_vis_2d = node.with_base_ref::<Node2D, _>(|base| base.visible);
            let after_vis_3d = node.with_base_ref::<Node3D, _>(|base| base.visible);
            let after_camera_2d = match &node.data {
                SceneNodeData::Camera2D(camera) if camera.active => Some(camera.transform),
                _ => None,
            };
            let after_camera_3d = match &node.data {
                SceneNodeData::Camera3D(camera) if camera.active => Some(camera.transform),
                _ => None,
            };
            let ui_after = node.with_base_ref::<UiBox, _>(Clone::clone);
            let changed = before_2d != after_2d || before_3d != after_3d;
            (
                value,
                changed,
                ui_before,
                ui_after,
                before_vis_2d != after_vis_2d,
                before_vis_3d != after_vis_3d,
                before_camera_2d != after_camera_2d,
                before_camera_3d != after_camera_3d,
                before_camera_3d.is_none() && after_camera_3d.is_some(),
            )
        };

        self.mark_needs_rerender(id);
        if vis_2d_changed || vis_3d_changed {
            self.force_rerender(id);
        }
        if transform_changed {
            self.mark_transform_dirty_recursive(id);
        }
        if active_camera_2d_changed {
            self.request_full_2d_scan_once();
        }
        if active_camera_3d_changed {
            self.request_full_3d_scan_once();
        }
        if active_camera_3d_activated {
            self.note_camera_3d_activated(id);
        }
        if let (Some(before), Some(after)) = (ui_before.as_ref(), ui_after.as_ref()) {
            self.mark_ui_base_change(id, before, after);
        }
        Some(value)
    }

    fn bind_locale_text<S>(&mut self, node_id: perro_ids::NodeID, key: S) -> bool
    where
        S: AsRef<str>,
    {
        Runtime::bind_locale_text(self, node_id, key.as_ref())
    }

    fn bind_locale_placeholder<S>(&mut self, node_id: perro_ids::NodeID, key: S) -> bool
    where
        S: AsRef<str>,
    {
        Runtime::bind_locale_placeholder(self, node_id, key.as_ref())
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
        self.mark_needs_rerender(child_id);
        self.mark_ui_reparent_dirty(child_id, old_parent, parent_id);
        true
    }

    fn force_rerender(&mut self, root_id: perro_ids::NodeID) -> bool {
        if root_id.is_nil() || self.nodes.get(root_id).is_none() {
            return false;
        }
        Runtime::force_rerender(self, root_id);
        true
    }

    fn mark_needs_rerender(&mut self, node_id: perro_ids::NodeID) -> bool {
        if node_id.is_nil() || self.nodes.get(node_id).is_none() {
            return false;
        }
        Runtime::mark_needs_rerender(self, node_id);
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
            self.note_removed_render_node(current);
            self.remove_attached_audio_for_node(current);

            // Remove script state first so script-side lookups cannot outlive node removal.
            let _ = self.remove_script_instance(current);

            let parent_id = match self.nodes.get(current) {
                Some(node) => {
                    for tag in node.get_tag_ids() {
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

    fn get_node_tags(&mut self, node_id: perro_ids::NodeID) -> Option<Vec<Cow<'static, str>>> {
        self.nodes.get(node_id).map(|node| {
            node.tags_slice()
                .iter()
                .map(|tag| tag.name.clone())
                .collect()
        })
    }

    fn tag_set<T>(&mut self, node_id: perro_ids::NodeID, tags: Option<T>) -> bool
    where
        T: IntoNodeTags,
    {
        let old_tags = match self.nodes.get(node_id) {
            Some(node) => node.get_tag_ids(),
            None => return false,
        };

        let Some(node) = self.nodes.get_mut(node_id) else {
            return false;
        };
        if let Some(tags) = tags {
            node.set_tags(Some(tags.into_node_tags()));
        } else {
            node.clear_tags();
        }
        let new_tags = node.get_tag_ids();

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
        T: IntoNodeTag,
    {
        let Some(node) = self.nodes.get_mut(node_id) else {
            return false;
        };
        let tag = tag.into_node_tag();
        let tag_id = tag.id;
        let mut added = false;
        if !node.has_tag(tag_id) {
            node.add_tag(tag);
            added = true;
        }
        if added {
            self.node_index
                .node_tag_index
                .entry(tag_id)
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

    fn query_nodes(&mut self, query: NodeQueryView<'_>) -> Vec<perro_ids::NodeID> {
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
        if self
            .nodes
            .get(node_id)
            .and_then(|node| node.with_base_ref::<Node2D, _>(|base| base.transform))
            == Some(local)
        {
            return true;
        }
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
        if self
            .nodes
            .get(node_id)
            .and_then(|node| node.with_base_ref::<Node3D, _>(|base| base.transform))
            == Some(local)
        {
            return true;
        }
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
        let global = global_basis * local.to_mat3();
        Some(Transform2D::from_mat3(global))
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
        let global = global_basis * local.to_mat4();
        Some(Transform3D::from_mat4(global))
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

    fn mesh_instance_surface_at_global_point(
        &mut self,
        node_id: perro_ids::NodeID,
        global_point: Vector3,
    ) -> Option<MeshSurfaceHit3D> {
        self.query_mesh_instance_surface_at_global_point(node_id, global_point)
    }

    fn mesh_instance_surface_on_global_ray(
        &mut self,
        node_id: perro_ids::NodeID,
        ray_origin: Vector3,
        ray_direction: Vector3,
        max_distance: f32,
    ) -> Option<MeshSurfaceHit3D> {
        self.query_mesh_instance_surface_on_global_ray(
            node_id,
            ray_origin,
            ray_direction,
            max_distance,
        )
    }

    fn mesh_instance_surfaces_on_global_rays(
        &mut self,
        node_id: perro_ids::NodeID,
        rays: &[MeshSurfaceRay3D],
        resolve_material: bool,
    ) -> Vec<Option<MeshSurfaceHit3D>> {
        self.query_mesh_instance_surfaces_on_global_rays(node_id, rays, resolve_material)
    }

    fn mesh_instance_material_regions(
        &mut self,
        node_id: perro_ids::NodeID,
        material: MaterialID,
    ) -> Vec<MeshMaterialRegion3D> {
        self.query_mesh_instance_material_regions(node_id, material)
    }

    fn mesh_data_surface_at_local_point(
        &mut self,
        mesh_id: perro_ids::MeshID,
        local_point: Vector3,
    ) -> Option<MeshDataSurfaceHit3D> {
        self.query_mesh_data_surface_at_local_point(mesh_id, local_point)
    }

    fn mesh_data_surface_on_local_ray(
        &mut self,
        mesh_id: perro_ids::MeshID,
        ray_origin_local: Vector3,
        ray_direction_local: Vector3,
        max_distance: f32,
    ) -> Option<MeshDataSurfaceHit3D> {
        self.query_mesh_data_surface_on_local_ray(
            mesh_id,
            ray_origin_local,
            ray_direction_local,
            max_distance,
        )
    }

    fn mesh_data_surface_regions(
        &mut self,
        mesh_id: perro_ids::MeshID,
        surface_index: u32,
    ) -> Vec<MeshDataSurfaceRegion3D> {
        self.query_mesh_data_surface_regions(mesh_id, surface_index)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/rt_ctx_nodes_transform_api_tests.rs"]
mod tests;
