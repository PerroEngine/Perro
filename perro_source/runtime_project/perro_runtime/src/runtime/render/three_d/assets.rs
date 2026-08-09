use super::*;

impl Runtime {
    // Resolve a mesh's surface materials into a render-bridge binding list using a
    // caller-owned `surfaces` buffer. Taking `&mut Vec` lets the per-frame
    // extraction path recycle one scratch allocation instead of cloning a fresh
    // Vec per moving mesh (see resolve_mesh_surfaces_modulated).
    pub(crate) fn resolve_render_mesh_assets_scratch(
        &mut self,
        node: NodeID,
        mut mesh: MeshID,
        surfaces: &mut Vec<MeshSurfaceBinding>,
    ) -> Option<(MeshID, std::sync::Arc<[MeshSurfaceBinding3D]>)> {
        mesh = self.resolve_render_mesh_id(node, mesh)?;

        for surface_index in 0..surfaces.len().max(1) {
            if surfaces.len() <= surface_index {
                surfaces.push(MeshSurfaceBinding::default());
            }
            let material = surfaces[surface_index]
                .material
                .unwrap_or(MaterialID::nil());
            if !material.is_nil() {
                if self.resource_api.is_material_id_pending(material) {
                    return None;
                }
                continue;
            }

            let request = material_3d_request(node, surface_index as u32);
            if let Some(result) = self.take_render_result(request) {
                match result {
                    crate::RuntimeRenderResult::Material(id) => {
                        surfaces[surface_index].material = Some(id);
                        if let Some(node) = self.nodes.get_mut_untracked(node) {
                            match &mut node.data {
                                SceneNodeData::MeshInstance3D(mesh_instance) => {
                                    mesh_instance.set_surface_material(surface_index, Some(id));
                                }
                                SceneNodeData::MultiMeshInstance3D(mesh_instance) => {
                                    mesh_instance.ensure_surface_mut(surface_index).material =
                                        Some(id);
                                }
                                _ => {}
                            }
                        }
                        continue;
                    }
                    crate::RuntimeRenderResult::Failed(_)
                    | crate::RuntimeRenderResult::Texture(_)
                    | crate::RuntimeRenderResult::Mesh(_) => {}
                }
            }

            let source = self
                .render_3d
                .material_surface_sources
                .get(&node)
                .and_then(|sources| sources.get(surface_index))
                .cloned()
                .flatten();
            let material_override = self
                .render_3d
                .material_surface_overrides
                .get(&node)
                .and_then(|overrides| overrides.get(surface_index))
                .cloned()
                .flatten();
            if material_override.is_none()
                && let Some(source) = source.as_deref()
                && let Some(id) = (!source.trim().is_empty())
                    .then(|| self.resource_api.load_material_source(source))
                && !id.is_nil()
            {
                surfaces[surface_index].material = Some(id);
                if let Some(node) = self.nodes.get_mut_untracked(node) {
                    match &mut node.data {
                        SceneNodeData::MeshInstance3D(mesh_instance) => {
                            mesh_instance.set_surface_material(surface_index, Some(id));
                        }
                        SceneNodeData::MultiMeshInstance3D(mesh_instance) => {
                            mesh_instance.ensure_surface_mut(surface_index).material = Some(id);
                        }
                        _ => {}
                    }
                }
                continue;
            }

            if source.is_none() {
                let id = if let Some(material) = material_override.clone() {
                    self.resource_api.shared_inline_material_id(material)
                } else {
                    self.resource_api.default_material_id()
                };
                surfaces[surface_index].material = Some(id);
                if let Some(node) = self.nodes.get_mut_untracked(node) {
                    match &mut node.data {
                        SceneNodeData::MeshInstance3D(mesh_instance) => {
                            mesh_instance.set_surface_material(surface_index, Some(id));
                        }
                        SceneNodeData::MultiMeshInstance3D(mesh_instance) => {
                            mesh_instance.ensure_surface_mut(surface_index).material = Some(id);
                        }
                        _ => {}
                    }
                }
                continue;
            }

            let material = material_override.unwrap_or_else(Material3D::default);
            if !self.render.is_inflight(request) {
                self.render.mark_inflight(request);
                self.queue_render_command(RenderCommand::Resource(Box::new(
                    ResourceCommand::CreateMaterial {
                        request,
                        id: MaterialID::nil(),
                        material: std::sync::Arc::new(material),
                        source,
                        reserved: false,
                    },
                )));
            }
            return None;
        }

        if self.render_3d.material_surface_sources.get(&node).is_none()
            && self
                .render_3d
                .material_surface_overrides
                .get(&node)
                .is_none()
            && surfaces.iter().all(|surface| surface.overrides.is_empty())
            && let Some(retained) = self.render_3d.retained_mesh_draws.get(&node)
            && retained.mesh == mesh
            && simple_surfaces_match(surfaces.as_slice(), &retained.surfaces)
        {
            return Some((mesh, retained.surfaces.clone()));
        }

        let converted: Vec<MeshSurfaceBinding3D> = surfaces
            .iter()
            .map(|surface| MeshSurfaceBinding3D {
                material: surface.material,
                overrides: surface
                    .overrides
                    .iter()
                    .map(|ovr| MaterialParamOverride3D {
                        name: ovr.name.clone(),
                        value: ovr.value,
                    })
                    .collect::<Vec<_>>()
                    .into(),
                modulate: surface.modulate,
            })
            .collect();
        Some((mesh, std::sync::Arc::from(converted)))
    }

    // Build the modulated surface list for `node` into a recycled scratch buffer
    // and resolve its materials. WHITE modulate skips the per-surface fold.
    pub(super) fn resolve_mesh_surfaces_modulated(
        &mut self,
        node: NodeID,
        mesh: MeshID,
        modulate: perro_structs::Color,
    ) -> Option<(MeshID, std::sync::Arc<[MeshSurfaceBinding3D]>)> {
        let mut surfaces = std::mem::take(&mut self.mesh_surface_scratch);
        surfaces.clear();
        if let Some(scene_node) = self.nodes.get(node) {
            match &scene_node.data {
                SceneNodeData::MeshInstance3D(mesh) => {
                    surfaces.extend(mesh.surfaces.iter().cloned());
                }
                SceneNodeData::MultiMeshInstance3D(mesh) => {
                    surfaces.extend(mesh.surfaces.iter().cloned());
                }
                _ => {}
            }
        }
        if modulate != perro_structs::Color::WHITE {
            for surface in &mut surfaces {
                surface.modulate = Self::color_modulate(surface.modulate, modulate);
            }
        }
        let result = self.resolve_render_mesh_assets_scratch(node, mesh, &mut surfaces);
        surfaces.clear();
        self.mesh_surface_scratch = surfaces;
        result
    }

    pub(crate) fn mesh_draw_has_pending_asset(&self, node: NodeID) -> bool {
        self.nodes
            .get(node)
            .is_some_and(|scene_node| match &scene_node.data {
                SceneNodeData::MeshInstance3D(mesh) => {
                    (!mesh.mesh.is_nil() && self.resource_api.is_mesh_id_pending(mesh.mesh))
                        || mesh.surfaces.iter().any(|surface| {
                            surface.material.is_some_and(|material| {
                                self.resource_api.is_material_id_pending(material)
                            })
                        })
                }
                SceneNodeData::MultiMeshInstance3D(mesh) => {
                    (!mesh.mesh.is_nil() && self.resource_api.is_mesh_id_pending(mesh.mesh))
                        || mesh.surfaces.iter().any(|surface| {
                            surface.material.is_some_and(|material| {
                                self.resource_api.is_material_id_pending(material)
                            })
                        })
                }
                _ => false,
            })
    }

    // `(pending, total)` mesh draws in the live graph. A draw counts as pending
    // when `resolve_render_mesh_assets` would bail on it: unresolved mesh id,
    // mesh still loading, or any surface material still loading. Those draws are
    // skipped silently, so this is what a loading screen should wait on.
    pub(crate) fn scene_mesh_asset_progress(&self) -> (u32, u32) {
        let mut pending = 0u32;
        let mut total = 0u32;
        // node_types lane pre-filter (1B/slot): only mesh-draw slots deref
        // their SceneNode.
        for index in 1..self.nodes.slot_count() {
            if !matches!(
                self.nodes.node_type_slots()[index],
                perro_nodes::NodeType::MeshInstance3D | perro_nodes::NodeType::MultiMeshInstance3D
            ) {
                continue;
            }
            let Some((node, scene_node)) = self.nodes.slot_get(index) else {
                continue;
            };
            let mesh = match &scene_node.data {
                SceneNodeData::MeshInstance3D(mesh) => mesh.mesh,
                SceneNodeData::MultiMeshInstance3D(mesh) => mesh.mesh,
                _ => continue,
            };
            total += 1;
            let unresolved = mesh.is_nil()
                && self
                    .render_3d
                    .mesh_sources
                    .get(&node)
                    .is_some_and(|source| !source.trim().is_empty());
            if unresolved || self.mesh_draw_has_pending_asset(node) {
                pending += 1;
            }
        }
        (pending, total)
    }

    // batched: 1 node pass covers every material in the slice. load
    // storms deliver many MaterialLoaded events per frame; per-material passes
    // were O(materials × nodes).
    pub(crate) fn invalidate_3d_mesh_draws_using_materials(&mut self, materials: &[MaterialID]) {
        // recycled scratch sets: this runs once per resource-event batch and
        // must not allocate fresh maps every load storm.
        let mut material_set = std::mem::take(&mut self.material_invalidation_ids_scratch);
        material_set.clear();
        material_set.extend(
            materials
                .iter()
                .copied()
                .filter(|material| !material.is_nil()),
        );
        if material_set.is_empty() {
            self.material_invalidation_ids_scratch = material_set;
            return;
        }
        let materials = &material_set;
        let uses_any = |surfaces: &[MeshSurfaceBinding]| {
            surfaces.iter().any(|surface| {
                surface
                    .material
                    .is_some_and(|material| materials.contains(&material))
            })
        };
        let mut nodes = std::mem::take(&mut self.material_invalidation_nodes_scratch);
        nodes.clear();
        for (node, scene_node) in self.nodes.iter() {
            let uses_material = match &scene_node.data {
                SceneNodeData::MeshInstance3D(mesh) => uses_any(&mesh.surfaces),
                SceneNodeData::MultiMeshInstance3D(mesh) => uses_any(&mesh.surfaces),
                _ => false,
            };
            if uses_material {
                nodes.insert(node);
            }
        }
        for (node, draw) in self.render_3d.retained_mesh_draws.iter() {
            if draw.surfaces.iter().any(|surface| {
                surface
                    .material
                    .is_some_and(|material| materials.contains(&material))
            }) {
                nodes.insert(*node);
            }
        }
        for node in nodes.drain() {
            self.render_3d.retained_mesh_draws.remove(&node);
            self.mark_needs_rerender(node);
        }
        self.material_invalidation_ids_scratch = material_set;
        self.material_invalidation_nodes_scratch = nodes;
    }

    pub(crate) fn resolve_render_mesh_id(
        &mut self,
        node: NodeID,
        mut mesh: MeshID,
    ) -> Option<MeshID> {
        let canonical = self.resource_api.canonical_mesh_id(mesh);
        if canonical != mesh {
            mesh = canonical;
            if let Some(node) = self.nodes.get_mut_untracked(node) {
                match &mut node.data {
                    SceneNodeData::MeshInstance3D(mesh_instance) => {
                        mesh_instance.mesh = mesh;
                    }
                    SceneNodeData::MultiMeshInstance3D(mesh_instance) => {
                        mesh_instance.mesh = mesh;
                    }
                    _ => {}
                }
            }
        }

        if !mesh.is_nil() && self.resource_api.is_mesh_id_pending(mesh) {
            // Runtime script/resource paths can assign a non-nil MeshID before the
            // render backend finishes CreateMesh; defer draw until ready.
            return None;
        }

        if mesh.is_nil() {
            let request = mesh_3d_request(node);
            if let Some(result) = self.take_render_result(request) {
                match result {
                    crate::RuntimeRenderResult::Mesh(id) => {
                        mesh = id;
                        if let Some(node) = self.nodes.get_mut_untracked(node) {
                            match &mut node.data {
                                SceneNodeData::MeshInstance3D(mesh_instance) => {
                                    mesh_instance.mesh = id;
                                }
                                SceneNodeData::MultiMeshInstance3D(mesh_instance) => {
                                    mesh_instance.mesh = id;
                                }
                                _ => {}
                            }
                        }
                    }
                    crate::RuntimeRenderResult::Failed(_)
                    | crate::RuntimeRenderResult::Texture(_)
                    | crate::RuntimeRenderResult::Material(_) => {}
                }
            }
            if mesh.is_nil() {
                let source = self
                    .render_3d
                    .mesh_sources
                    .get(&node)
                    .map(|source| source.trim().to_string())
                    .filter(|source| !source.is_empty())?;
                if source.is_empty() {
                    return None;
                }
                if !self.render.is_inflight(request) {
                    self.render.mark_inflight(request);
                    self.queue_render_command(RenderCommand::Resource(Box::new(
                        ResourceCommand::CreateMesh {
                            request,
                            id: MeshID::nil(),
                            source,
                            reserved: false,
                        },
                    )));
                }
                return None;
            }
        }
        Some(mesh)
    }
}
