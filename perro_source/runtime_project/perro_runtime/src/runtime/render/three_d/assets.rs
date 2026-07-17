use super::*;

impl Runtime {
    pub(crate) fn resolve_render_mesh_assets(
        &mut self,
        node: NodeID,
        mesh: MeshID,
        mut surfaces: Vec<MeshSurfaceBinding>,
    ) -> Option<(MeshID, std::sync::Arc<[MeshSurfaceBinding3D]>)> {
        self.resolve_render_mesh_assets_scratch(node, mesh, &mut surfaces)
    }

    // Resolve a mesh's surface materials into a render-bridge binding list using a
    // caller-owned `surfaces` buffer. Taking `&mut Vec` lets the per-frame
    // extraction path recycle one scratch allocation instead of cloning a fresh
    // Vec per moving mesh (see resolve_mesh_surfaces_modulated).
    pub(super) fn resolve_render_mesh_assets_scratch(
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
                self.queue_render_command(RenderCommand::Resource(
                    ResourceCommand::CreateMaterial {
                        request,
                        id: MaterialID::nil(),
                        material,
                        source,
                        reserved: false,
                    },
                ));
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
                        value: ovr.value.clone(),
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

    pub(crate) fn invalidate_3d_mesh_draws_using_material(&mut self, material: MaterialID) {
        if material.is_nil() {
            return;
        }
        let mut nodes = Vec::new();
        for (node, scene_node) in self.nodes.iter() {
            let uses_material = match &scene_node.data {
                SceneNodeData::MeshInstance3D(mesh) => mesh
                    .surfaces
                    .iter()
                    .any(|surface| surface.material == Some(material)),
                SceneNodeData::MultiMeshInstance3D(mesh) => mesh
                    .surfaces
                    .iter()
                    .any(|surface| surface.material == Some(material)),
                _ => false,
            };
            if uses_material {
                nodes.push(node);
            }
        }
        for (node, draw) in self.render_3d.retained_mesh_draws.iter() {
            if draw
                .surfaces
                .iter()
                .any(|surface| surface.material == Some(material))
                && !nodes.contains(node)
            {
                nodes.push(*node);
            }
        }
        for node in nodes {
            self.render_3d.retained_mesh_draws.remove(&node);
            self.mark_needs_rerender(node);
        }
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
                    self.queue_render_command(RenderCommand::Resource(
                        ResourceCommand::CreateMesh {
                            request,
                            id: MeshID::nil(),
                            source,
                            reserved: false,
                        },
                    ));
                }
                return None;
            }
        }
        Some(mesh)
    }
}
