use super::*;

impl NodeAPI for Runtime {
    fn create<T>(&mut self) -> perro_ids::NodeID
    where
        T: Default + Into<SceneNodeData>,
    {
        // Read type + camera-active flag from the owned node before it moves
        // into the arena, eliminating both post-insert arena lookups.
        let node = SceneNode::new(T::default().into());
        let node_type = node.node_type();
        let camera_3d_active =
            matches!(&node.data, SceneNodeData::Camera3D(camera) if camera.active);
        let id = self.nodes.insert(node);
        self.register_internal_node_schedules(id, node_type);
        if camera_3d_active {
            self.note_camera_3d_activated(id);
        }
        // Ensure freshly created nodes participate in render/transform extraction
        // even before caller-side mutation paths run.
        self.mark_needs_rerender(id);
        self.mark_created_ui_node_dirty(id);
        self.mark_transform_dirty_recursive(id);
        id
    }

    fn create_nodes<'a, B>(
        &mut self,
        requests: B,
        parent_id: perro_ids::NodeID,
    ) -> Vec<perro_ids::NodeID>
    where
        B: IntoNodeCreateBatch<'a>,
    {
        match requests.into_node_create_batch() {
            NodeCreateBatch::Specs(specs) => self.create_node_specs(specs, parent_id),
            NodeCreateBatch::Collection(collection) => {
                self.create_node_collection(collection, parent_id)
            }
            NodeCreateBatch::OwnedSpecs(specs) => self.create_owned_node_specs(specs, parent_id),
            NodeCreateBatch::OwnedCollection(collection) => {
                self.create_node_collection(&collection, parent_id)
            }
        }
    }

    fn with_node_mut<T, V, F>(&mut self, id: perro_ids::NodeID, f: F) -> Option<V>
    where
        T: NodeTypeDispatch,
        F: FnOnce(&mut T) -> V,
    {
        if id.is_nil() {
            return None;
        }

        let (
            transform_changed,
            ui_before,
            ui_after,
            camera_2d_changed,
            camera_3d_changed,
            camera_3d_activated,
            visibility_changed,
            modulate_changed,
            value,
        ) = {
            // Non-physics types skip the physics-version bump (const-gated);
            // physics types mark the change after the mutation below.
            let node = self.nodes.get_mut_untracked_non_physics(id)?;

            // Const-gated so the optimizer strips camera/UI capture for node
            // types that can never be those variants.
            let track_ui = T::NODE_TYPE.is_a(NodeType::UiNode);
            let track_camera_2d = T::NODE_TYPE == NodeType::Camera2D;
            let track_camera_3d = T::NODE_TYPE == NodeType::Camera3D;
            // Single-pass snapshots replace the old before/after deep clone of
            // `SceneNodeData`. `local_snapshot` folds visibility + base modulate
            // into one match; `ui_snapshot` captures the UI base + payload
            // fingerprints only when this type is a UI node.
            let ui_before = track_ui.then(|| ui_snapshot(&node.data)).flatten();
            let (visible_before, modulate_before) = local_snapshot(&node.data);
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
            let ui_after = track_ui.then(|| ui_snapshot(&node.data)).flatten();
            let (visible_after, modulate_after) = local_snapshot(&node.data);
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
                visible_before != visible_after,
                modulate_before != modulate_after,
                value,
            )
        };

        if T::NODE_TYPE.is_physics() {
            self.nodes.mark_physics_change();
        }
        if T::NODE_TYPE == NodeType::UiViewport {
            self.invalidate_physics_query_sync();
        }
        if matches!(T::RENDERABLE, Renderable::True) {
            self.mark_needs_rerender(id);
        }
        if T::NODE_TYPE == NodeType::Webcam {
            self.mark_camera_stream_users_dirty(id);
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
        let is_ui_node = ui_before.is_some();
        if let (Some(before), Some(after)) = (ui_before.as_ref(), ui_after.as_ref()) {
            self.mark_ui_snapshot_change(id, before, after);
        }
        if visibility_changed && !is_ui_node {
            self.mark_ui_visibility_dirty_subtree(id);
        }
        if modulate_changed {
            self.force_rerender(id);
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

        let Some(node_ref) = self.nodes.get(node_id) else {
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
        let node = self.nodes.get(id)?;
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
        let concrete_type = self.nodes.get(id)?.node_type();

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
            modulate_changed,
        ) = {
            // Base kinds (2D/3D) cannot tell physics nodes apart at compile
            // time; keep the conservative full bump for physics nodes and skip
            // only for the rest.
            let node = if self.nodes.get(id)?.node_type().is_physics() {
                self.nodes.get_mut_untracked(id)?
            } else {
                self.nodes.get_mut_untracked_non_physics(id)?
            };
            if !node.node_type().is_a(T::BASE_NODE_TYPE) {
                return None;
            }
            // A base mutation touches exactly one base kind (2D / 3D / UI).
            // `base_spatial_snapshot` folds the transform + visible + modulate
            // probes for the 2D/3D bases into one match, and the UI base is
            // captured once (Copy-content clone) for `mark_ui_base_change`.
            let before = base_spatial_snapshot(&node.data);
            let before_camera_2d = match &node.data {
                SceneNodeData::Camera2D(camera) if camera.active => Some(camera.transform),
                _ => None,
            };
            let before_camera_3d = match &node.data {
                SceneNodeData::Camera3D(camera) if camera.active => Some(camera.transform),
                _ => None,
            };
            let ui_before = node.with_base_ref::<UiNode, _>(Clone::clone);
            let value = node.with_base_mut::<T, _>(f)?;
            let after = base_spatial_snapshot(&node.data);
            let after_camera_2d = match &node.data {
                SceneNodeData::Camera2D(camera) if camera.active => Some(camera.transform),
                _ => None,
            };
            let after_camera_3d = match &node.data {
                SceneNodeData::Camera3D(camera) if camera.active => Some(camera.transform),
                _ => None,
            };
            let ui_after = node.with_base_ref::<UiNode, _>(Clone::clone);
            let changed = before.transform_2d != after.transform_2d
                || before.transform_3d != after.transform_3d;
            (
                value,
                changed,
                ui_before,
                ui_after,
                before.visible_2d != after.visible_2d,
                before.visible_3d != after.visible_3d,
                before_camera_2d != after_camera_2d,
                before_camera_3d != after_camera_3d,
                before_camera_3d.is_none() && after_camera_3d.is_some(),
                before.modulate != after.modulate,
            )
        };

        self.mark_needs_rerender(id);
        if concrete_type == NodeType::UiViewport {
            self.invalidate_physics_query_sync();
        }
        if vis_2d_changed || vis_3d_changed {
            self.force_rerender(id);
        }
        if modulate_changed {
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
        // Route through the arena so the name index stays in sync.
        self.nodes.rename(node_id, name.into())
    }

    fn find_node_by_name<S>(
        &mut self,
        root: perro_ids::NodeID,
        name: S,
    ) -> Option<perro_ids::NodeID>
    where
        S: AsRef<str>,
    {
        let name = name.as_ref();
        for id in self.nodes.named_ids(name) {
            if root.is_nil() || self.node_is_descendant_of(*id, root) {
                return Some(*id);
            }
        }
        None
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
        if parent_id == child_id {
            return false;
        }
        if !parent_id.is_nil() && self.nodes.get(parent_id).is_none() {
            return false;
        }

        // Reject a parent inside the child's subtree. Bound the upward walk by
        // the live-node count so an already-corrupt parent cycle also fails
        // closed instead of hanging this API.
        let mut ancestor = parent_id;
        let mut remaining = self.nodes.len();
        while !ancestor.is_nil() {
            if ancestor == child_id || remaining == 0 {
                return false;
            }
            let Some(node) = self.nodes.get(ancestor) else {
                return false;
            };
            ancestor = node.get_parent();
            remaining -= 1;
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
            && let Some(mut parent) = self.nodes.get_mut(old_parent)
        {
            parent.remove_child(child_id);
        }

        if !self.nodes.set_parent(child_id, parent_id) {
            return false;
        }

        if !parent_id.is_nil() {
            if let Some(mut parent) = self.nodes.get_mut(parent_id) {
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
                if let Some(mut child) = self.nodes.get_mut(child_id) {
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
                        let local_mat = inverse_basis_mat4(parent_global) * global.to_mat4();
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
                if let Some(mut child) = self.nodes.get_mut(child_id) {
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

    fn is_mesh_instance_ready(&mut self, node_id: perro_ids::NodeID) -> bool {
        Runtime::mesh_instance_render_ready(self, node_id)
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
        let node_id = self
            .scene_ownership_roots
            .get(&node_id)
            .copied()
            .unwrap_or(node_id);
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
            let ty = match self.nodes.get(current) {
                Some(node) => node.node_type(),
                None => continue,
            };
            self.note_removed_render_node(current, ty);
            self.remove_attached_audio_for_node(current);

            // Remove script state first so script-side lookups cannot outlive node removal.
            let _ = self.remove_script_instance(current);

            let parent_id = match self.nodes.get(current) {
                Some(node) => node.get_parent(),
                None => continue,
            };

            // Only unlink from parents that are NOT part of the removed subtree.
            // Every in-subtree parent is itself about to be removed, so its
            // `remove_child` retain scan is wasted work (O(n*k)). `visited`
            // already tracks subtree membership, so a cheap hash check replaces
            // the retain scan for the common case. A node whose parent field
            // points outside the subtree (stale/reparent edge) is still
            // correctly unlinked from that live parent.
            if !parent_id.is_nil()
                && !visited.contains(&parent_id)
                && let Some(mut parent) = self.nodes.get_mut(parent_id)
            {
                parent.remove_child(current);
            }

            self.unregister_internal_node_schedules(current, ty);
            let _ = self.nodes.remove(current);
        }

        self.scene_ownership_roots
            .retain(|scene_root, owner| !visited.contains(scene_root) && !visited.contains(owner));

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
        // Arena keeps the tag index in sync.
        self.nodes
            .set_node_tags(node_id, tags.map(|tags| tags.into_node_tags()))
    }

    fn add_node_tag<T>(&mut self, node_id: perro_ids::NodeID, tag: T) -> bool
    where
        T: IntoNodeTag,
    {
        self.nodes.add_node_tag(node_id, tag.into_node_tag())
    }

    fn remove_node_tag<T>(&mut self, node_id: perro_ids::NodeID, tag: T) -> bool
    where
        T: IntoTagID,
    {
        self.nodes.remove_node_tag(node_id, tag.into_tag_id())
    }

    fn query_nodes(&mut self, query: NodeQueryView<'_>) -> Vec<perro_ids::NodeID> {
        // Spatial queries hoist candidate computation up front so a rare
        // tag/name index can also restrict the spatial-index fill below,
        // not just the post-fill scan; the candidates are then handed to
        // `query_node_ids_with_candidates` so they aren't computed twice.
        let has_spatial = query.expr.as_ref().is_some_and(QueryExpr::has_spatial);
        let candidates = if has_spatial && matches!(query.scope, QueryScope::Root) {
            super::super::query::candidate_ids_from_index(
                query.expr,
                Some(self.nodes.tag_index()),
                self.nodes.slot_count(),
            )
        } else {
            None
        };

        let spatial = self.build_query_spatial_index(
            query.expr,
            query.scope,
            candidates
                .as_ref()
                .map(|candidates| candidates.ids.as_slice()),
        );
        let out = super::super::query::query_node_ids_with_candidates(
            &self.nodes,
            query,
            spatial.as_ref(),
            Some(self.nodes.tag_index()),
            candidates,
        );
        self.recycle_query_spatial_index(spatial);
        out
    }

    fn query_first_node(&mut self, query: NodeQueryView<'_>) -> Option<perro_ids::NodeID> {
        let has_spatial = query.expr.as_ref().is_some_and(QueryExpr::has_spatial);
        let candidates = if has_spatial && matches!(query.scope, QueryScope::Root) {
            super::super::query::candidate_ids_from_index(
                query.expr,
                Some(self.nodes.tag_index()),
                self.nodes.slot_count(),
            )
        } else {
            None
        };

        let spatial = self.build_query_spatial_index(
            query.expr,
            query.scope,
            candidates
                .as_ref()
                .map(|candidates| candidates.ids.as_slice()),
        );
        let out = super::super::query::query_first_node_id_with_candidates(
            &self.nodes,
            query,
            spatial.as_ref(),
            Some(self.nodes.tag_index()),
            Some(candidates),
        );
        self.recycle_query_spatial_index(spatial);
        out
    }

    fn get_global_transform_2d(&mut self, node_id: perro_ids::NodeID) -> Option<Transform2D> {
        Runtime::get_global_transform_2d(self, node_id)
    }

    fn get_global_transform_3d(&mut self, node_id: perro_ids::NodeID) -> Option<Transform3D> {
        Runtime::get_global_transform_3d(self, node_id)
    }

    fn set_global_transform_2d(&mut self, node_id: perro_ids::NodeID, global: Transform2D) -> bool {
        let (parent, top_level) = match self.nodes.get(node_id) {
            Some(node) => {
                match node.with_base_ref::<Node2D, _>(|base| (node.parent, base.top_level)) {
                    Some(value) => value,
                    None => return false,
                }
            }
            None => return false,
        };
        let parent_global = if top_level || parent.is_nil() {
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
        let (parent, top_level) = match self.nodes.get(node_id) {
            Some(node) => {
                match node.with_base_ref::<Node3D, _>(|base| (node.parent, base.top_level)) {
                    Some(value) => value,
                    None => return false,
                }
            }
            None => return false,
        };
        let parent_global = if top_level || parent.is_nil() {
            None
        } else {
            self.nodes
                .get(parent)
                .and_then(|node| node.with_base_ref::<Node3D, _>(|_| ()))
                .and_then(|_| Runtime::get_global_transform_3d(self, parent))
        };
        let local = match parent_global {
            Some(parent_global) => {
                let local_mat = inverse_basis_mat4(parent_global) * global.to_mat4();
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

    fn camera_screen_ray_3d(
        &mut self,
        camera_id: NodeID,
        pixel: Vector2,
        viewport_size: Vector2,
    ) -> Option<CameraRay3D> {
        if !pixel.x.is_finite()
            || !pixel.y.is_finite()
            || !viewport_size.x.is_finite()
            || !viewport_size.y.is_finite()
            || viewport_size.x <= 0.0
            || viewport_size.y <= 0.0
        {
            return None;
        }
        let projection = match &self.nodes.get(camera_id)?.data {
            SceneNodeData::Camera3D(camera) => camera.projection.clone(),
            _ => return None,
        };
        let transform = Runtime::get_global_transform_3d(self, camera_id)?;
        let rotation: glam::Quat = transform.rotation.into();
        if !rotation.is_finite() || rotation.length_squared() <= 1.0e-8 {
            return None;
        }
        let rotation = rotation.normalize();
        let x = pixel.x.mul_add(2.0 / viewport_size.x, -1.0);
        let y = 1.0 - pixel.y * (2.0 / viewport_size.y);
        let aspect = viewport_size.x / viewport_size.y;
        let camera_position: glam::Vec3 = transform.position.into();
        let (origin_local, direction_local, max_distance) = match projection {
            CameraProjection::Perspective {
                fov_y_degrees, far, ..
            } => {
                let half_y = (fov_y_degrees.to_radians() * 0.5).tan();
                (
                    glam::Vec3::ZERO,
                    glam::Vec3::new(x * half_y * aspect, y * half_y, -1.0).normalize_or_zero(),
                    far,
                )
            }
            CameraProjection::Orthographic { size, near, far } => (
                glam::Vec3::new(x * size * aspect * 0.5, y * size * 0.5, -near),
                glam::Vec3::NEG_Z,
                far - near,
            ),
            CameraProjection::Frustum {
                left,
                right,
                bottom,
                top,
                near,
                far,
            } => {
                let near_point = glam::Vec3::new(
                    left + (x + 1.0) * 0.5 * (right - left),
                    bottom + (y + 1.0) * 0.5 * (top - bottom),
                    -near,
                );
                let direction = near_point.normalize_or_zero();
                (glam::Vec3::ZERO, direction, far / (-direction.z))
            }
        };
        let origin = camera_position + rotation * origin_local;
        let direction = rotation * direction_local;
        Some(CameraRay3D {
            origin: origin.into(),
            direction: direction.into(),
            max_distance,
        })
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
        let inv_basis = inverse_basis_mat4(Runtime::get_global_transform_3d(self, node_id)?);
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

    fn mesh_instance_surface_global_point(
        &mut self,
        node_id: perro_ids::NodeID,
        triangle_index: u32,
        barycentric: Vector3,
    ) -> Option<Vector3> {
        self.query_mesh_instance_surface_global_point(node_id, triangle_index, barycentric)
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
