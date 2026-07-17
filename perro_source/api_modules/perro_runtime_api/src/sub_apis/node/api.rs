use super::*;

pub trait NodeAPI {
    /// Creates a new node with default value of `T`.
    fn create<T>(&mut self) -> NodeID
    where
        T: Default + Into<SceneNodeData>;

    /// Creates many nodes and optionally attaches them under one parent.
    ///
    /// Returns created IDs in request order.
    fn create_nodes<'a, B>(&mut self, requests: B, parent_id: NodeID) -> Vec<NodeID>
    where
        B: IntoNodeCreateBatch<'a>;

    /// Runs closure against an exact concrete node type.
    ///
    /// Returns `None` if `id` is invalid or node type does not exactly match `T`.
    fn with_node_mut<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        T: NodeTypeDispatch,
        F: FnOnce(&mut T) -> V;

    /// Reads from an exact concrete node type.
    ///
    /// Returns `V::default()` if `id` is invalid or node type does not exactly match `T`.
    fn with_node<T, V: Clone + Default>(&mut self, node_id: NodeID, f: impl FnOnce(&T) -> V) -> V
    where
        T: NodeTypeDispatch;

    /// Runs closure against a base type (`T`) with runtime ancestry check.
    ///
    /// This allows descendant concrete types to be treated as a shared base type.
    fn with_base_node<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        T: NodeBaseDispatch,
        F: FnOnce(&T) -> V;

    /// Mutable variant of [`NodeAPI::with_base_node`].
    fn with_base_node_mut<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        T: NodeBaseDispatch,
        F: FnOnce(&mut T) -> V;

    /// Returns node display name if node exists.
    fn get_node_name(&mut self, node_id: NodeID) -> Option<Cow<'static, str>>;

    /// Sets node display name; returns `true` on success.
    fn set_node_name<S>(&mut self, node_id: NodeID, name: S) -> bool
    where
        S: Into<Cow<'static, str>>;

    /// Finds a node by name inside `root`'s subtree (including `root`
    /// itself). Pass `NodeID::nil()` as `root` to search the whole scene.
    /// Index-backed: O(nodes sharing the name), not a tree walk.
    fn find_node_by_name<S>(&mut self, root: NodeID, name: S) -> Option<NodeID>
    where
        S: AsRef<str>;

    /// Collects `root` followed by every descendant (depth-first, `root`
    /// included). Empty when `root` is nil. Backs the `descendants!`,
    /// `set_tree_visible!`, and `broadcast_var!` macros.
    fn subtree_node_ids(&mut self, root: NodeID) -> Vec<NodeID> {
        collect_subtree_ids(root, |id| self.get_children(id))
    }

    /// Sets `visible` on every `UiNode` in `root`'s subtree (including `root`).
    /// Non-UI nodes are skipped. Returns the count of UI nodes updated.
    fn set_subtree_visible(&mut self, root: NodeID, visible: bool) -> usize {
        let mut updated = 0;
        for id in self.subtree_node_ids(root) {
            if self
                .with_base_node_mut::<UiNode, _, _>(id, |node| node.visible = visible)
                .is_some()
            {
                updated += 1;
            }
        }
        updated
    }

    /// Returns skeleton bone name by index.
    fn get_skeleton_bone_name(
        &mut self,
        skeleton_id: NodeID,
        bone_index: usize,
    ) -> Option<Cow<'static, str>> {
        self.with_node::<Skeleton3D, _>(skeleton_id, |skeleton| {
            skeleton
                .bone_name(bone_index)
                .map(|name| Cow::Owned(name.to_string()))
        })
    }

    /// Returns first skeleton bone index matching name.
    fn get_skeleton_bone_index<S>(&mut self, skeleton_id: NodeID, bone_name: S) -> Option<usize>
    where
        S: AsRef<str>,
    {
        self.with_node::<Skeleton3D, _>(skeleton_id, |skeleton| {
            skeleton.bone_index(bone_name.as_ref())
        })
    }

    /// Sets UI rotation in radians. Works on `UiNode` and descendants.
    fn set_ui_rotation(&mut self, node_id: NodeID, rotation: f32) -> bool {
        self.with_base_node_mut::<UiNode, _, _>(node_id, |node| {
            node.transform.rotation = rotation;
        })
        .is_some()
    }

    /// Binds a UI text node's main text field to a localization key.
    ///
    /// Works on `UiLabel.text`, `UiTextBox.text`, and `UiTextBlock.text`.
    fn bind_locale_text<S>(&mut self, node_id: NodeID, key: S) -> bool
    where
        S: AsRef<str>;

    /// Binds a text-edit node's placeholder field to a localization key.
    ///
    /// Works on `UiTextBox.placeholder` and `UiTextBlock.placeholder`.
    fn bind_locale_placeholder<S>(&mut self, node_id: NodeID, key: S) -> bool
    where
        S: AsRef<str>;

    /// Returns parent node id if node exists.
    fn get_node_parent_id(&mut self, node_id: NodeID) -> Option<NodeID>;

    /// Returns children ids if node exists.
    fn get_node_children_ids(&mut self, node_id: NodeID) -> Option<Vec<NodeID>>;

    /// Returns direct children ids. Invalid parent returns empty vec.
    fn get_children(&mut self, node_id: NodeID) -> Vec<NodeID> {
        self.get_node_children_ids(node_id).unwrap_or_default()
    }

    /// Returns direct child by index.
    fn get_child_at(&mut self, parent_id: NodeID, index: usize) -> Option<NodeID> {
        self.get_node_children_ids(parent_id)
            .and_then(|children| children.into_iter().nth(index))
    }

    /// Returns first direct child matching name.
    fn get_child_by_name<S>(&mut self, parent_id: NodeID, name: S) -> Option<NodeID>
    where
        S: AsRef<str>,
    {
        let target = name.as_ref();
        for child_id in self.get_children(parent_id) {
            if let Some(child_name) = self.get_node_name(child_id)
                && child_name.as_ref() == target
            {
                return Some(child_id);
            }
        }
        None
    }

    /// Returns all direct children matching name.
    fn get_children_by_name<S>(&mut self, parent_id: NodeID, name: S) -> Vec<NodeID>
    where
        S: AsRef<str>,
    {
        let target = name.as_ref();
        let mut out = Vec::new();
        for child_id in self.get_children(parent_id) {
            if let Some(child_name) = self.get_node_name(child_id)
                && child_name.as_ref() == target
            {
                out.push(child_id);
            }
        }
        out
    }

    /// Returns direct child selected by index or name.
    fn get_child<T>(&mut self, parent_id: NodeID, selector: T) -> Option<NodeID>
    where
        T: IntoChildSelector,
    {
        match selector.into_child_selector() {
            ChildSelector::Index(index) => self.get_child_at(parent_id, index),
            ChildSelector::Name(name) => self.get_child_by_name(parent_id, name),
        }
    }

    /// Returns concrete runtime node type if node exists.
    fn get_node_type(&mut self, node_id: NodeID) -> Option<NodeType>;

    /// Reparents a child under parent. `parent_id = nil` detaches to root.
    fn reparent(&mut self, parent_id: NodeID, child_id: NodeID) -> bool;

    /// Marks one node + all descendants dirty for render extraction this frame.
    fn force_rerender(&mut self, root_id: NodeID) -> bool;

    /// Marks one node dirty for render extraction this frame.
    fn mark_needs_rerender(&mut self, node_id: NodeID) -> bool;

    /// Returns true when a MeshInstance3D/MultiMeshInstance3D has a retained draw
    /// using loaded mesh and material resources.
    fn is_mesh_instance_ready(&mut self, node_id: NodeID) -> bool;

    /// Batch reparent. Returns count of successful operations.
    fn reparent_multi<I>(&mut self, parent_id: NodeID, child_ids: I) -> usize
    where
        I: IntoIterator<Item = NodeID>;

    /// Removes a node from the scene graph.
    fn remove_node(&mut self, node_id: NodeID) -> bool;

    /// Returns node tag names if node exists.
    fn get_node_tags(&mut self, node_id: NodeID) -> Option<Vec<Cow<'static, str>>>;

    /// Sets node tags (`Some`) or clears all tags (`None`).
    fn set_tags<T>(&mut self, node_id: NodeID, tags: Option<T>) -> bool
    where
        T: IntoNodeTags,
    {
        self.tag_set(node_id, tags)
    }

    /// Compatibility hook for runtimes that still implement `tag_set`.
    fn tag_set<T>(&mut self, node_id: NodeID, tags: Option<T>) -> bool
    where
        T: IntoNodeTags;

    /// Adds one tag to node (idempotent).
    fn add_node_tag<T>(&mut self, node_id: NodeID, tag: T) -> bool
    where
        T: IntoNodeTag;

    /// Removes one tag from node.
    fn remove_node_tag<T>(&mut self, node_id: NodeID, tag: T) -> bool
    where
        T: IntoTagID;

    /// Executes a node query and returns matching node IDs.
    fn query_nodes(&mut self, query: NodeQueryView<'_>) -> Vec<NodeID>;

    /// Executes a node query and returns the first matching node ID.
    fn query_first_node(&mut self, query: NodeQueryView<'_>) -> Option<NodeID> {
        self.query_nodes(query).into_iter().next()
    }

    /// Returns the current global transform for a 2D spatial node.
    fn get_global_transform_2d(&mut self, node_id: NodeID) -> Option<Transform2D>;

    /// Returns the current global transform for a 3D spatial node.
    fn get_global_transform_3d(&mut self, node_id: NodeID) -> Option<Transform3D>;

    /// Sets a 2D node's local transform so its resulting global transform matches `global`.
    fn set_global_transform_2d(&mut self, node_id: NodeID, global: Transform2D) -> bool;

    /// Sets a 3D node's local transform so its resulting global transform matches `global`.
    fn set_global_transform_3d(&mut self, node_id: NodeID, global: Transform3D) -> bool;

    /// Converts a point from node-local 2D space to global 2D space.
    fn to_global_point_2d(&mut self, node_id: NodeID, local: Vector2) -> Option<Vector2>;

    /// Converts a point from global 2D space to node-local 2D space.
    fn to_local_point_2d(&mut self, node_id: NodeID, global: Vector2) -> Option<Vector2>;

    /// Converts a point from node-local 3D space to global 3D space.
    fn to_global_point_3d(&mut self, node_id: NodeID, local: Vector3) -> Option<Vector3>;

    /// Converts a point from global 3D space to node-local 3D space.
    fn to_local_point_3d(&mut self, node_id: NodeID, global: Vector3) -> Option<Vector3>;

    /// Builds a global-space ray through a top-left-origin viewport pixel.
    fn camera_screen_ray_3d(
        &mut self,
        camera_id: NodeID,
        pixel: Vector2,
        viewport_size: Vector2,
    ) -> Option<CameraRay3D> {
        let _ = (camera_id, pixel, viewport_size);
        None
    }

    /// Converts a local 2D transform (relative to `node_id`) into global space.
    fn to_global_transform_2d(
        &mut self,
        node_id: NodeID,
        local: Transform2D,
    ) -> Option<Transform2D>;

    /// Converts a global 2D transform into local space relative to `node_id`.
    fn to_local_transform_2d(
        &mut self,
        node_id: NodeID,
        global: Transform2D,
    ) -> Option<Transform2D>;

    /// Converts a local 3D transform (relative to `node_id`) into global space.
    fn to_global_transform_3d(
        &mut self,
        node_id: NodeID,
        local: Transform3D,
    ) -> Option<Transform3D>;

    /// Converts a global 3D transform into local space relative to `node_id`.
    fn to_local_transform_3d(
        &mut self,
        node_id: NodeID,
        global: Transform3D,
    ) -> Option<Transform3D>;

    /// Finds mesh-instance surface nearest to global-space point for a 3D mesh node.
    ///
    /// Returns `None` when:
    /// - node does not exist
    /// - node is not a mesh-bearing 3D node
    /// - mesh source cannot be resolved/decoded
    fn mesh_instance_surface_at_global_point(
        &mut self,
        node_id: NodeID,
        global_point: Vector3,
    ) -> Option<MeshSurfaceHit3D>;

    /// Resolves one mesh query triangle + barycentric coordinate to global space.
    ///
    /// `triangle_index` uses the same numbering returned by mesh surface hit queries.
    /// Skinned `MeshInstance3D` nodes use the current skeleton pose.
    fn mesh_instance_surface_global_point(
        &mut self,
        node_id: NodeID,
        triangle_index: u32,
        barycentric: Vector3,
    ) -> Option<Vector3>;

    /// Finds the first mesh surface hit along a global-space ray for a 3D mesh node.
    ///
    /// `ray_direction` does not need to be normalized.
    /// Returns `None` when:
    /// - node does not exist
    /// - node is not a mesh-bearing 3D node
    /// - mesh source cannot be resolved/decoded
    /// - ray misses all triangles within `max_distance`
    fn mesh_instance_surface_on_global_ray(
        &mut self,
        node_id: NodeID,
        ray_origin: Vector3,
        ray_direction: Vector3,
        max_distance: f32,
    ) -> Option<MeshSurfaceHit3D>;

    /// Finds mesh surface hits for many global-space rays against the same mesh node.
    ///
    /// Reuses node lookup, mesh decode/cache lookup, node global transform, and instance data
    /// across all rays. `resolve_material=false` skips material lookup and leaves hit material
    /// as `None`, useful when scripts only need `surface_index`.
    fn mesh_instance_surfaces_on_global_rays(
        &mut self,
        node_id: NodeID,
        rays: &[MeshSurfaceRay3D],
        resolve_material: bool,
    ) -> Vec<Option<MeshSurfaceHit3D>>;

    /// Returns regions (one per matching surface) where `material` exists on a mesh node.
    ///
    /// Region bounds/centers are coarse geometric summaries for gameplay queries.
    fn mesh_instance_material_regions(
        &mut self,
        node_id: NodeID,
        material: MaterialID,
    ) -> Vec<MeshMaterialRegion3D>;

    /// Finds raw mesh-data surface nearest to mesh-local point.
    ///
    /// Uses mesh data directly, with no node transform, instances, global values, or material resolve.
    fn mesh_data_surface_at_local_point(
        &mut self,
        mesh_id: MeshID,
        local_point: Vector3,
    ) -> Option<MeshDataSurfaceHit3D>;

    /// Finds raw mesh-data surface hit on mesh-local ray.
    ///
    /// Uses mesh data directly, with no node transform, instances, global values, or material resolve.
    fn mesh_data_surface_on_local_ray(
        &mut self,
        mesh_id: MeshID,
        ray_origin_local: Vector3,
        ray_direction_local: Vector3,
        max_distance: f32,
    ) -> Option<MeshDataSurfaceHit3D>;

    /// Returns regions for one raw mesh-data surface index.
    fn mesh_data_surface_regions(
        &mut self,
        mesh_id: MeshID,
        surface_index: u32,
    ) -> Vec<MeshDataSurfaceRegion3D>;
}
