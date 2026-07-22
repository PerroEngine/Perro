use super::*;

pub struct NodeModule<'rt, R: NodeAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: NodeAPI + ?Sized> NodeModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn create<T>(&mut self) -> NodeID
    where
        T: Default + Into<SceneNodeData>,
    {
        self.rt.create::<T>()
    }

    pub fn create_nodes<'a, B>(&mut self, requests: B, parent_id: NodeID) -> Vec<NodeID>
    where
        B: IntoNodeCreateBatch<'a>,
    {
        self.rt.create_nodes(requests, parent_id)
    }

    pub fn with_node_mut<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        T: NodeTypeDispatch,
        F: FnOnce(&mut T) -> V,
    {
        self.rt.with_node_mut::<T, V, F>(id, f)
    }

    pub fn with_node<T, V>(&mut self, node_id: NodeID, f: impl FnOnce(&T) -> V) -> Option<V>
    where
        T: NodeTypeDispatch,
    {
        self.rt.with_node::<T, V>(node_id, f)
    }

    pub fn with_base_node<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        T: NodeBaseDispatch,
        F: FnOnce(&T) -> V,
    {
        self.rt.with_base_node::<T, V, F>(id, f)
    }

    pub fn with_base_node_mut<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V>
    where
        T: NodeBaseDispatch,
        F: FnOnce(&mut T) -> V,
    {
        self.rt.with_base_node_mut::<T, V, F>(id, f)
    }

    pub fn get_node_name(&mut self, node_id: NodeID) -> Option<Cow<'static, str>> {
        self.rt.get_node_name(node_id)
    }

    pub fn name(&mut self, node_id: NodeID) -> Option<Cow<'static, str>> {
        self.get_node_name(node_id)
    }

    pub fn set_node_name<S>(&mut self, node_id: NodeID, name: S) -> bool
    where
        S: Into<Cow<'static, str>>,
    {
        self.rt.set_node_name(node_id, name)
    }

    pub fn find_node_by_name<S>(&mut self, root: NodeID, name: S) -> Option<NodeID>
    where
        S: AsRef<str>,
    {
        self.rt.find_node_by_name(root, name)
    }

    pub fn subtree_node_ids(&mut self, root: NodeID) -> Vec<NodeID> {
        self.rt.subtree_node_ids(root)
    }

    pub fn set_subtree_visible(&mut self, root: NodeID, visible: bool) -> usize {
        self.rt.set_subtree_visible(root, visible)
    }

    pub fn get_skeleton_bone_name(
        &mut self,
        skeleton_id: NodeID,
        bone_index: usize,
    ) -> Option<Cow<'static, str>> {
        self.rt.get_skeleton_bone_name(skeleton_id, bone_index)
    }

    pub fn get_skeleton_bone_index<S>(&mut self, skeleton_id: NodeID, bone_name: S) -> Option<usize>
    where
        S: AsRef<str>,
    {
        self.rt.get_skeleton_bone_index(skeleton_id, bone_name)
    }

    pub fn set_ui_rotation(&mut self, node_id: NodeID, rotation: f32) -> bool {
        self.rt.set_ui_rotation(node_id, rotation)
    }

    pub fn bind_locale_text<S>(&mut self, node_id: NodeID, key: S) -> bool
    where
        S: AsRef<str>,
    {
        self.rt.bind_locale_text(node_id, key)
    }

    pub fn bind_locale_placeholder<S>(&mut self, node_id: NodeID, key: S) -> bool
    where
        S: AsRef<str>,
    {
        self.rt.bind_locale_placeholder(node_id, key)
    }

    pub fn get_node_parent_id(&mut self, node_id: NodeID) -> Option<NodeID> {
        self.rt.get_node_parent_id(node_id)
    }

    pub fn get_node_children_ids(&mut self, node_id: NodeID) -> Option<Vec<NodeID>> {
        self.rt.get_node_children_ids(node_id)
    }

    pub fn children_ids(&mut self, node_id: NodeID) -> Option<Vec<NodeID>> {
        self.get_node_children_ids(node_id)
    }

    pub fn get_children(&mut self, node_id: NodeID) -> Vec<NodeID> {
        self.get_node_children_ids(node_id).unwrap_or_default()
    }

    pub fn get_child_at(&mut self, parent_id: NodeID, index: usize) -> Option<NodeID> {
        self.get_children(parent_id).into_iter().nth(index)
    }

    pub fn get_child_by_name<S>(&mut self, parent_id: NodeID, name: S) -> Option<NodeID>
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

    pub fn get_children_by_name<S>(&mut self, parent_id: NodeID, name: S) -> Vec<NodeID>
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

    pub fn get_child<T>(&mut self, parent_id: NodeID, selector: T) -> Option<NodeID>
    where
        T: IntoChildSelector,
    {
        match selector.into_child_selector() {
            ChildSelector::Index(index) => self.get_child_at(parent_id, index),
            ChildSelector::Name(name) => self.get_child_by_name(parent_id, name),
        }
    }

    pub fn get_node_type(&mut self, node_id: NodeID) -> Option<NodeType> {
        self.rt.get_node_type(node_id)
    }

    pub fn reparent(&mut self, parent_id: NodeID, child_id: NodeID) -> bool {
        self.rt.reparent(parent_id, child_id)
    }

    pub fn force_rerender(&mut self, root_id: NodeID) -> bool {
        self.rt.force_rerender(root_id)
    }

    pub fn mark_needs_rerender(&mut self, node_id: NodeID) -> bool {
        self.rt.mark_needs_rerender(node_id)
    }

    pub fn is_mesh_instance_ready(&mut self, node_id: NodeID) -> bool {
        self.rt.is_mesh_instance_ready(node_id)
    }

    pub fn reparent_multi<I>(&mut self, parent_id: NodeID, child_ids: I) -> usize
    where
        I: IntoIterator<Item = NodeID>,
    {
        self.rt.reparent_multi(parent_id, child_ids)
    }

    pub fn remove_node(&mut self, node_id: NodeID) -> bool {
        self.rt.remove_node(node_id)
    }

    pub fn get_node_tags(&mut self, node_id: NodeID) -> Option<Vec<Cow<'static, str>>> {
        self.rt.get_node_tags(node_id)
    }

    pub fn set_tags<T>(&mut self, node_id: NodeID, tags: Option<T>) -> bool
    where
        T: IntoNodeTags,
    {
        self.rt.set_tags(node_id, tags)
    }

    #[deprecated(note = "use set_tags")]
    pub fn tag_set<T>(&mut self, node_id: NodeID, tags: Option<T>) -> bool
    where
        T: IntoNodeTags,
    {
        self.set_tags(node_id, tags)
    }

    pub fn add_node_tag<T>(&mut self, node_id: NodeID, tag: T) -> bool
    where
        T: IntoNodeTag,
    {
        self.rt.add_node_tag(node_id, tag)
    }

    pub fn add_node_tags<T>(&mut self, node_id: NodeID, tags: T) -> bool
    where
        T: IntoNodeTags,
    {
        let node_tags = tags.into_node_tags();
        if node_tags.is_empty() {
            return true;
        }

        for tag in node_tags {
            if !self.rt.add_node_tag(node_id, tag) {
                return false;
            }
        }
        true
    }

    pub fn remove_node_tag<T>(&mut self, node_id: NodeID, tag: T) -> bool
    where
        T: IntoTagID,
    {
        self.rt.remove_node_tag(node_id, tag)
    }

    pub fn get_global_transform_2d(&mut self, node_id: NodeID) -> Option<Transform2D> {
        self.rt.get_global_transform_2d(node_id)
    }

    pub fn get_global_transform_3d(&mut self, node_id: NodeID) -> Option<Transform3D> {
        self.rt.get_global_transform_3d(node_id)
    }

    pub fn get_local_transform_2d(&mut self, node_id: NodeID) -> Option<Transform2D> {
        self.with_base_node::<Node2D, _, _>(node_id, |node| node.transform)
    }

    pub fn get_local_transform_3d(&mut self, node_id: NodeID) -> Option<Transform3D> {
        self.with_base_node::<Node3D, _, _>(node_id, |node| node.transform)
    }

    pub fn set_local_transform_2d(&mut self, node_id: NodeID, transform: Transform2D) -> bool {
        self.with_base_node_mut::<Node2D, _, _>(node_id, |node| {
            node.transform = transform;
        })
        .is_some()
    }

    pub fn set_local_transform_3d(&mut self, node_id: NodeID, transform: Transform3D) -> bool {
        self.with_base_node_mut::<Node3D, _, _>(node_id, |node| {
            node.transform = transform;
        })
        .is_some()
    }

    pub fn set_global_transform_2d(&mut self, node_id: NodeID, global: Transform2D) -> bool {
        self.rt.set_global_transform_2d(node_id, global)
    }

    pub fn set_global_transform_3d(&mut self, node_id: NodeID, global: Transform3D) -> bool {
        self.rt.set_global_transform_3d(node_id, global)
    }

    pub fn get_local_pos_2d(&mut self, node_id: NodeID) -> Option<Vector2> {
        self.get_local_transform_2d(node_id)
            .map(|transform| transform.position)
    }

    pub fn get_local_pos_3d(&mut self, node_id: NodeID) -> Option<Vector3> {
        self.get_local_transform_3d(node_id)
            .map(|transform| transform.position)
    }

    pub fn set_local_pos_2d(&mut self, node_id: NodeID, pos: Vector2) -> bool {
        self.with_base_node_mut::<Node2D, _, _>(node_id, |node| {
            node.transform.position = pos;
        })
        .is_some()
    }

    pub fn set_local_pos_3d(&mut self, node_id: NodeID, pos: Vector3) -> bool {
        self.with_base_node_mut::<Node3D, _, _>(node_id, |node| {
            node.transform.position = pos;
        })
        .is_some()
    }

    pub fn get_global_pos_2d(&mut self, node_id: NodeID) -> Option<Vector2> {
        self.get_global_transform_2d(node_id)
            .map(|transform| transform.position)
    }

    pub fn get_global_pos_3d(&mut self, node_id: NodeID) -> Option<Vector3> {
        self.get_global_transform_3d(node_id)
            .map(|transform| transform.position)
    }

    pub fn set_global_pos_2d(&mut self, node_id: NodeID, pos: Vector2) -> bool {
        let Some(mut transform) = self.get_global_transform_2d(node_id) else {
            return false;
        };
        transform.position = pos;
        self.set_global_transform_2d(node_id, transform)
    }

    pub fn set_global_pos_3d(&mut self, node_id: NodeID, pos: Vector3) -> bool {
        let Some(mut transform) = self.get_global_transform_3d(node_id) else {
            return false;
        };
        transform.position = pos;
        self.set_global_transform_3d(node_id, transform)
    }

    pub fn get_local_rot_2d(&mut self, node_id: NodeID) -> Option<f32> {
        self.get_local_transform_2d(node_id)
            .map(|transform| transform.rotation)
    }

    pub fn get_local_rot_3d(&mut self, node_id: NodeID) -> Option<Quaternion> {
        self.get_local_transform_3d(node_id)
            .map(|transform| transform.rotation)
    }

    pub fn set_local_rot_2d(&mut self, node_id: NodeID, rot: f32) -> bool {
        self.with_base_node_mut::<Node2D, _, _>(node_id, |node| {
            node.transform.rotation = rot;
        })
        .is_some()
    }

    pub fn set_local_rot_3d(&mut self, node_id: NodeID, rot: Quaternion) -> bool {
        self.with_base_node_mut::<Node3D, _, _>(node_id, |node| {
            node.transform.rotation = rot;
        })
        .is_some()
    }

    pub fn get_global_rot_2d(&mut self, node_id: NodeID) -> Option<f32> {
        self.get_global_transform_2d(node_id)
            .map(|transform| transform.rotation)
    }

    pub fn get_global_rot_3d(&mut self, node_id: NodeID) -> Option<Quaternion> {
        self.get_global_transform_3d(node_id)
            .map(|transform| transform.rotation)
    }

    pub fn set_global_rot_2d(&mut self, node_id: NodeID, rot: f32) -> bool {
        let Some(mut transform) = self.get_global_transform_2d(node_id) else {
            return false;
        };
        transform.rotation = rot;
        self.set_global_transform_2d(node_id, transform)
    }

    pub fn set_global_rot_3d(&mut self, node_id: NodeID, rot: Quaternion) -> bool {
        let Some(mut transform) = self.get_global_transform_3d(node_id) else {
            return false;
        };
        transform.rotation = rot;
        self.set_global_transform_3d(node_id, transform)
    }

    pub fn get_local_scale_2d(&mut self, node_id: NodeID) -> Option<Vector2> {
        self.get_local_transform_2d(node_id)
            .map(|transform| transform.scale)
    }

    pub fn get_local_scale_3d(&mut self, node_id: NodeID) -> Option<Vector3> {
        self.get_local_transform_3d(node_id)
            .map(|transform| transform.scale)
    }

    pub fn set_local_scale_2d(&mut self, node_id: NodeID, scale: Vector2) -> bool {
        self.with_base_node_mut::<Node2D, _, _>(node_id, |node| {
            node.transform.scale = scale;
        })
        .is_some()
    }

    pub fn set_local_scale_3d(&mut self, node_id: NodeID, scale: Vector3) -> bool {
        self.with_base_node_mut::<Node3D, _, _>(node_id, |node| {
            node.transform.scale = scale;
        })
        .is_some()
    }

    pub fn get_global_scale_2d(&mut self, node_id: NodeID) -> Option<Vector2> {
        self.get_global_transform_2d(node_id)
            .map(|transform| transform.scale)
    }

    pub fn get_global_scale_3d(&mut self, node_id: NodeID) -> Option<Vector3> {
        self.get_global_transform_3d(node_id)
            .map(|transform| transform.scale)
    }

    pub fn set_global_scale_2d(&mut self, node_id: NodeID, scale: Vector2) -> bool {
        let Some(mut transform) = self.get_global_transform_2d(node_id) else {
            return false;
        };
        transform.scale = scale;
        self.set_global_transform_2d(node_id, transform)
    }

    pub fn set_global_scale_3d(&mut self, node_id: NodeID, scale: Vector3) -> bool {
        let Some(mut transform) = self.get_global_transform_3d(node_id) else {
            return false;
        };
        transform.scale = scale;
        self.set_global_transform_3d(node_id, transform)
    }

    pub fn to_global_point_2d(&mut self, node_id: NodeID, local: Vector2) -> Option<Vector2> {
        self.rt.to_global_point_2d(node_id, local)
    }

    pub fn to_local_point_2d(&mut self, node_id: NodeID, global: Vector2) -> Option<Vector2> {
        self.rt.to_local_point_2d(node_id, global)
    }

    pub fn to_global_point_3d(&mut self, node_id: NodeID, local: Vector3) -> Option<Vector3> {
        self.rt.to_global_point_3d(node_id, local)
    }

    pub fn to_local_point_3d(&mut self, node_id: NodeID, global: Vector3) -> Option<Vector3> {
        self.rt.to_local_point_3d(node_id, global)
    }

    pub fn camera_screen_ray_3d(
        &mut self,
        camera_id: NodeID,
        pixel: Vector2,
        viewport_size: Vector2,
    ) -> Option<CameraRay3D> {
        self.rt
            .camera_screen_ray_3d(camera_id, pixel, viewport_size)
    }

    pub fn to_global_transform_2d(
        &mut self,
        node_id: NodeID,
        local: Transform2D,
    ) -> Option<Transform2D> {
        self.rt.to_global_transform_2d(node_id, local)
    }

    pub fn to_local_transform_2d(
        &mut self,
        node_id: NodeID,
        global: Transform2D,
    ) -> Option<Transform2D> {
        self.rt.to_local_transform_2d(node_id, global)
    }

    pub fn to_global_transform_3d(
        &mut self,
        node_id: NodeID,
        local: Transform3D,
    ) -> Option<Transform3D> {
        self.rt.to_global_transform_3d(node_id, local)
    }

    pub fn to_local_transform_3d(
        &mut self,
        node_id: NodeID,
        global: Transform3D,
    ) -> Option<Transform3D> {
        self.rt.to_local_transform_3d(node_id, global)
    }

    pub fn mesh_instance_surface_at_global_point(
        &mut self,
        node_id: NodeID,
        global_point: Vector3,
    ) -> Option<MeshSurfaceHit3D> {
        self.rt
            .mesh_instance_surface_at_global_point(node_id, global_point)
    }

    pub fn mesh_instance_surface_global_point(
        &mut self,
        node_id: NodeID,
        triangle_index: u32,
        barycentric: Vector3,
    ) -> Option<Vector3> {
        self.rt
            .mesh_instance_surface_global_point(node_id, triangle_index, barycentric)
    }

    pub fn mesh_instance_surface_on_global_ray(
        &mut self,
        node_id: NodeID,
        ray_origin: Vector3,
        ray_direction: Vector3,
        max_distance: f32,
    ) -> Option<MeshSurfaceHit3D> {
        self.rt.mesh_instance_surface_on_global_ray(
            node_id,
            ray_origin,
            ray_direction,
            max_distance,
        )
    }

    pub fn mesh_instance_surfaces_on_global_rays(
        &mut self,
        node_id: NodeID,
        rays: &[MeshSurfaceRay3D],
        resolve_material: bool,
    ) -> Vec<Option<MeshSurfaceHit3D>> {
        self.rt
            .mesh_instance_surfaces_on_global_rays(node_id, rays, resolve_material)
    }

    pub fn mesh_instance_material_regions(
        &mut self,
        node_id: NodeID,
        material: MaterialID,
    ) -> Vec<MeshMaterialRegion3D> {
        self.rt.mesh_instance_material_regions(node_id, material)
    }

    pub fn mesh_data_surface_at_local_point(
        &mut self,
        mesh_id: MeshID,
        local_point: Vector3,
    ) -> Option<MeshDataSurfaceHit3D> {
        self.rt
            .mesh_data_surface_at_local_point(mesh_id, local_point)
    }

    pub fn mesh_data_surface_on_local_ray(
        &mut self,
        mesh_id: MeshID,
        ray_origin_local: Vector3,
        ray_direction_local: Vector3,
        max_distance: f32,
    ) -> Option<MeshDataSurfaceHit3D> {
        self.rt.mesh_data_surface_on_local_ray(
            mesh_id,
            ray_origin_local,
            ray_direction_local,
            max_distance,
        )
    }

    pub fn mesh_data_surface_regions(
        &mut self,
        mesh_id: MeshID,
        surface_index: u32,
    ) -> Vec<MeshDataSurfaceRegion3D> {
        self.rt.mesh_data_surface_regions(mesh_id, surface_index)
    }
}
