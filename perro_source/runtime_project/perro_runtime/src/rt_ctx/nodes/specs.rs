use super::*;

impl Runtime {
    /// Cheap up-front validation shared by the borrowed and owned spec paths.
    ///
    /// Runs on a borrowed slice so the borrowed path can reject invalid batches
    /// (empty, missing parent, forward parent reference) before paying for any
    /// clone of the specs.
    pub(super) fn node_specs_valid(&self, specs: &[NodeSpec], parent_id: NodeID) -> bool {
        if specs.is_empty() {
            return false;
        }
        if !parent_id.is_nil() && self.nodes.get(parent_id).is_none() {
            return false;
        }
        specs
            .iter()
            .enumerate()
            .all(|(index, spec)| spec.parent.is_none_or(|parent| parent < index))
    }

    pub(super) fn create_node_specs(
        &mut self,
        specs: &[NodeSpec],
        parent_id: NodeID,
    ) -> Vec<NodeID> {
        // Validate on the borrowed slice first; only clone once the batch is
        // known-good, so invalid batches never pay the deep clone.
        if !self.node_specs_valid(specs, parent_id) {
            return Vec::new();
        }
        self.create_owned_node_specs(specs.to_vec(), parent_id)
    }

    pub(super) fn create_owned_node_specs(
        &mut self,
        specs: Vec<NodeSpec>,
        parent_id: NodeID,
    ) -> Vec<NodeID> {
        if !self.node_specs_valid(&specs, parent_id) {
            return Vec::new();
        }

        let mut child_counts = vec![0usize; specs.len()];
        let mut root_count = 0usize;
        for spec in &specs {
            if let Some(parent) = spec.parent {
                child_counts[parent] += 1;
            } else {
                root_count += 1;
            }
        }

        self.nodes.reserve(specs.len());

        let mut ids = Vec::with_capacity(specs.len());
        let mut root_ids = Vec::with_capacity(root_count);
        for (index, spec) in specs.into_iter().enumerate() {
            let parent = spec.parent.map(|parent| ids[parent]).unwrap_or(parent_id);
            let mut node = SceneNode::new(spec.data);
            if let Some(name) = spec.name {
                node.set_name(name);
            }
            node.set_tags(Some(spec.tags));
            node.parent = parent;
            node.children.reserve(child_counts[index]);

            let node_type = node.node_type();
            let id = self.nodes.insert(node);
            ids.push(id);

            self.register_internal_node_schedules(id, node_type);
            if self.nodes.get(id).is_some_and(
                |node| matches!(&node.data, SceneNodeData::Camera3D(camera) if camera.active),
            ) {
                self.note_camera_3d_activated(id);
            }
            self.mark_needs_rerender(id);
            self.mark_created_ui_node_dirty(id);
            if let Some(script) = spec.script {
                let Some(vars) = resolve_script_vars(&script, &ids) else {
                    return Vec::new();
                };
                let _ = <Self as ScriptAPI>::script_attach_with_vars(
                    self,
                    id,
                    script.path.as_ref(),
                    vars,
                );
            }
            if let Some(parent_index) = spec.parent {
                self.nodes.push_child(ids[parent_index], id);
            } else if parent_id.is_nil() {
                self.mark_transform_dirty_recursive(id);
            } else {
                root_ids.push(id);
            }
        }

        if !parent_id.is_nil() {
            self.attach_created_children(parent_id, &root_ids);
        }

        ids
    }

    pub(super) fn attach_created_children(&mut self, parent_id: NodeID, ids: &[NodeID]) {
        if ids.is_empty() {
            return;
        }
        self.nodes.extend_children(parent_id, ids);
        self.mark_transform_dirty_recursive(parent_id);
        let parent_ui_ancestor = self.closest_ui_ancestor(parent_id);
        for &id in ids {
            let child_is_ui = self
                .nodes
                .get(id)
                .and_then(|node| ui_base_from_data(&node.data))
                .is_some();
            if child_is_ui || parent_ui_ancestor.is_some() {
                self.mark_ui_reparent_dirty(id, NodeID::nil(), parent_id);
            }
        }
    }

    pub(super) fn create_node_collection(
        &mut self,
        collection: &NodeCollection,
        parent_id: NodeID,
    ) -> Vec<NodeID> {
        // Borrowed input: the body already reads specs/scenes by reference and
        // clones only the individual spec that is materialized into a node (that
        // clone is unavoidable). Taking `&NodeCollection` drops the wholesale
        // clone of `entries`/`scenes` that the caller previously paid up front.
        if collection.is_specs_only() {
            // Validate before cloning the spec vec so invalid batches pay nothing.
            if !self.node_specs_valid(&collection.specs, parent_id) {
                return Vec::new();
            }
            return self.create_owned_node_specs(collection.specs.clone(), parent_id);
        }
        if !parent_id.is_nil() && self.nodes.get(parent_id).is_none() {
            return Vec::new();
        }
        if !collection.entries.iter().enumerate().all(|(index, entry)| {
            let parent = match entry {
                NodeCollectionEntry::Node(spec_index) => collection.specs[*spec_index].parent,
                NodeCollectionEntry::Scene(scene_index) => collection.scenes[*scene_index].parent,
            };
            parent.is_none_or(|parent| parent < index)
        }) {
            return Vec::new();
        }
        for scene in &collection.scenes {
            if self.preload_scene_at_runtime(scene.path.as_ref()).is_err() {
                return Vec::new();
            }
        }

        let mut ids = Vec::with_capacity(collection.entries.len());
        for entry in &collection.entries {
            match entry {
                NodeCollectionEntry::Node(spec_index) => {
                    let mut spec = collection.specs[*spec_index].clone();
                    let parent = spec.parent.map(|parent| ids[parent]).unwrap_or(parent_id);
                    spec.parent = None;
                    let mut made = self.create_owned_node_specs(vec![spec], parent);
                    if made.len() != 1 {
                        return Vec::new();
                    }
                    ids.append(&mut made);
                }
                NodeCollectionEntry::Scene(scene_index) => {
                    let scene = &collection.scenes[*scene_index];
                    let parent = scene.parent.map(|parent| ids[parent]).unwrap_or(parent_id);
                    let Ok(id) = self.load_scene_at_runtime(scene.path.as_ref()) else {
                        return Vec::new();
                    };
                    let scene_loader_parent = self
                        .nodes
                        .get(id)
                        .map(|node| node.parent)
                        .unwrap_or(NodeID::nil());
                    if let Some(name) = &scene.name {
                        let _ = <Self as NodeAPI>::set_node_name(self, id, name.clone());
                    }
                    if !scene.tags.is_empty() {
                        let _ = <Self as NodeAPI>::tag_set(self, id, Some(scene.tags.clone()));
                    }
                    for patch in &scene.patches {
                        let Some(mut node) = self.nodes.get_mut(id) else {
                            return Vec::new();
                        };
                        if !patch.apply(&mut node.data) {
                            return Vec::new();
                        }
                    }
                    if !scene.patches.is_empty() {
                        self.mark_needs_rerender(id);
                        self.mark_transform_dirty_recursive(id);
                        self.mark_created_ui_node_dirty(id);
                    }
                    if let Some(script) = &scene.script {
                        let Some(vars) = resolve_script_vars(script, &ids) else {
                            return Vec::new();
                        };
                        let _ = <Self as ScriptAPI>::script_attach_with_vars(
                            self,
                            id,
                            script.path.as_ref(),
                            vars,
                        );
                    }
                    if !parent.is_nil() {
                        let _ = <Self as NodeAPI>::reparent(self, parent, id);
                    }
                    if !scene_loader_parent.is_nil()
                        && self.nodes.get(scene_loader_parent).is_some_and(|node| {
                            node.name.as_ref() == "Game Root" && node.children.is_empty()
                        })
                    {
                        let _ = <Self as NodeAPI>::remove_node(self, scene_loader_parent);
                    }
                    ids.push(id);
                }
            }
        }
        ids
    }
}

pub(super) fn resolve_script_vars(
    script: &NodeScriptSpec,
    ids: &[NodeID],
) -> Option<Vec<(perro_ids::ScriptMemberID, perro_variant::Variant)>> {
    let mut out = Vec::with_capacity(script.vars.len());
    for (member, value) in &script.vars {
        let value = match value {
            NodeScriptVar::Value(value) => value.clone(),
            NodeScriptVar::NodeRef(index) => perro_variant::Variant::from(*ids.get(*index)?),
        };
        out.push((*member, value));
    }
    Some(out)
}
