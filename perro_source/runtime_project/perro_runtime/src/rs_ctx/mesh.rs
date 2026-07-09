use super::core::RuntimeResourceApi;
use perro_ids::{MeshID, string_to_u64};
use perro_render_bridge::{Mesh3D, RenderCommand, ResourceCommand};
use perro_resource_api::sub_apis::MeshAPI;
use std::sync::Arc;

impl MeshAPI for RuntimeResourceApi {
    fn load_mesh(&self, source: &str) -> MeshID {
        if let Some(hash) = perro_ids::parse_hashed_source_uri(source) {
            self.load_mesh_hashed(hash, None)
        } else {
            self.load_mesh_hashed(perro_ids::string_to_u64(source), Some(source))
        }
    }

    fn reserve_mesh(&self, source: &str) -> MeshID {
        if let Some(hash) = perro_ids::parse_hashed_source_uri(source) {
            self.reserve_mesh_hashed(hash, None)
        } else {
            self.reserve_mesh_hashed(perro_ids::string_to_u64(source), Some(source))
        }
    }

    fn create_mesh_data(&self, data: Mesh3D) -> MeshID {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let request = state.allocate_request();
        let id = state.allocate_mesh_id();
        let source = format!("runtime://mesh/{}:{}", id.index(), id.generation());
        let source_hash = string_to_u64(&source);
        state.mesh_data_by_id.insert(id, data.clone());
        state.mesh_revision_by_id.insert(id, 1);
        state.mesh_by_source.insert(source_hash, id);
        state.mesh_source_by_id.insert(id, source.clone());
        state.mesh_pending_by_source.insert(source_hash, request);
        state
            .mesh_pending_source_by_request
            .insert(request, source.clone());
        state.mesh_pending_id_by_request.insert(request, id);
        state.queued_commands.push(RenderCommand::Resource(
            ResourceCommand::CreateRuntimeMesh {
                request,
                id,
                source,
                reserved: false,
                mesh: data,
            },
        ));
        id
    }

    fn create_mesh_from_bytes(&self, bytes: &[u8]) -> MeshID {
        if bytes.is_empty() {
            return MeshID::nil();
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let request = state.allocate_request();
        let id = state.allocate_mesh_id();
        let source = format!("runtime://mesh-bytes/{}:{}", id.index(), id.generation());
        let source_hash = string_to_u64(&source);
        state.mesh_by_source.insert(source_hash, id);
        state.mesh_source_by_id.insert(id, source.clone());
        state.mesh_pending_by_source.insert(source_hash, request);
        state
            .mesh_pending_source_by_request
            .insert(request, source.clone());
        state.mesh_pending_id_by_request.insert(request, id);
        state.queued_commands.push(RenderCommand::Resource(
            ResourceCommand::CreateRuntimeMeshBytes {
                request,
                id,
                source,
                reserved: false,
                bytes: Arc::from(bytes),
            },
        ));
        id
    }

    fn get_mesh_data(&self, id: MeshID) -> Option<Mesh3D> {
        let state = self.state.lock().expect("resource api mutex poisoned");
        state.mesh_data_by_id.get(&id).cloned()
    }

    fn write_mesh_data(&self, id: MeshID, data: Mesh3D) -> bool {
        if id.is_nil() {
            return false;
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.mesh_data_by_id.insert(id, data.clone());
        let revision = state.mesh_revision_by_id.entry(id).or_insert(0);
        *revision = revision.wrapping_add(1).max(1);
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::WriteMeshData {
                id,
                mesh: data,
            }));
        true
    }

    fn is_mesh_loaded(&self, id: MeshID) -> bool {
        if id.is_nil() {
            return true;
        }
        let canonical = self.canonical_mesh_id(id);
        let state = self.state.lock().expect("resource api mutex poisoned");
        (state.mesh_data_by_id.contains_key(&canonical)
            || state.mesh_loaded_by_id.contains(&canonical))
            && !state
                .mesh_pending_id_by_request
                .values()
                .copied()
                .any(|pending| {
                    state
                        .mesh_id_alias
                        .get(&pending)
                        .copied()
                        .unwrap_or(pending)
                        == canonical
                })
    }

    fn load_mesh_hashed(&self, source_hash: u64, source: Option<&str>) -> MeshID {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.mesh_by_source.get(&source_hash).copied() {
            return id;
        }
        let Some(source) = source else {
            return MeshID::nil();
        };
        let normalized = normalize_source_slashes(source);
        let source = normalized.as_ref();
        let source_hash = string_to_u64(source);
        if let Some(id) = state.mesh_by_source.get(&source_hash).copied() {
            return id;
        }
        let request = state.allocate_request();
        let id = state.allocate_mesh_id();
        state.mesh_by_source.insert(source_hash, id);
        state.mesh_source_by_id.insert(id, source.to_string());
        state.mesh_pending_by_source.insert(source_hash, request);
        state
            .mesh_pending_source_by_request
            .insert(request, source.to_string());
        state.mesh_pending_id_by_request.insert(request, id);
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::CreateMesh {
                request,
                id,
                source: source.to_string(),
                reserved: false,
            }));
        id
    }

    fn reserve_mesh_hashed(&self, source_hash: u64, source: Option<&str>) -> MeshID {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.mesh_by_source.get(&source_hash).copied() {
            if state.mesh_pending_by_source.contains_key(&source_hash) {
                state.mesh_reserve_pending.insert(source_hash);
                return id;
            }
            state
                .queued_commands
                .push(RenderCommand::Resource(ResourceCommand::SetMeshReserved {
                    id,
                    reserved: true,
                }));
            return id;
        }
        let Some(source) = source else {
            return MeshID::nil();
        };
        let normalized = normalize_source_slashes(source);
        let source = normalized.as_ref();
        let source_hash = string_to_u64(source);
        if let Some(id) = state.mesh_by_source.get(&source_hash).copied() {
            if state.mesh_pending_by_source.contains_key(&source_hash) {
                state.mesh_reserve_pending.insert(source_hash);
                return id;
            }
            state
                .queued_commands
                .push(RenderCommand::Resource(ResourceCommand::SetMeshReserved {
                    id,
                    reserved: true,
                }));
            return id;
        }
        state.mesh_drop_pending.remove(&source_hash);
        state.mesh_reserve_pending.insert(source_hash);
        let request = state.allocate_request();
        let id = state.allocate_mesh_id();
        state.mesh_by_source.insert(source_hash, id);
        state.mesh_source_by_id.insert(id, source.to_string());
        state.mesh_pending_by_source.insert(source_hash, request);
        state
            .mesh_pending_source_by_request
            .insert(request, source.to_string());
        state.mesh_pending_id_by_request.insert(request, id);
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::CreateMesh {
                request,
                id,
                source: source.to_string(),
                reserved: true,
            }));
        id
    }

    fn reserve_mesh_id(&self, id: MeshID) -> bool {
        if id.is_nil() {
            return false;
        }
        let id = self.canonical_mesh_id(id);
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let known = state.mesh_data_by_id.contains_key(&id)
            || state
                .mesh_by_source
                .values()
                .any(|existing| *existing == id)
            || state.mesh_pending_id_by_request.values().any(|pending| {
                state
                    .mesh_id_alias
                    .get(pending)
                    .copied()
                    .unwrap_or(*pending)
                    == id
            });
        if !known {
            return false;
        }
        if let Some(source_hash) = state
            .mesh_by_source
            .iter()
            .find_map(|(source_hash, existing)| (*existing == id).then_some(*source_hash))
            .or_else(|| {
                state
                    .mesh_pending_id_by_request
                    .iter()
                    .find_map(|(request, pending_id)| {
                        (state
                            .mesh_id_alias
                            .get(pending_id)
                            .copied()
                            .unwrap_or(*pending_id)
                            == id)
                            .then(|| state.mesh_pending_source_by_request.get(request))
                            .flatten()
                            .map(|source| string_to_u64(source))
                    })
            })
        {
            state.mesh_reserve_pending.insert(source_hash);
            state.mesh_drop_pending.remove(&source_hash);
        }
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::SetMeshReserved {
                id,
                reserved: true,
            }));
        true
    }

    fn drop_mesh(&self, id: MeshID) -> bool {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.mesh_data_by_id.remove(&id);
        state.mesh_revision_by_id.remove(&id);
        let source = state
            .mesh_by_source
            .iter()
            .find_map(|(source_hash, existing)| (*existing == id).then_some(*source_hash));
        if let Some(source_hash) = source {
            state.mesh_reserve_pending.remove(&source_hash);
            if state.mesh_pending_by_source.contains_key(&source_hash) {
                state.mesh_drop_pending.insert(source_hash);
                return true;
            }
            state.mesh_by_source.remove(&source_hash);
        }
        state.mesh_source_by_id.remove(&id);
        let _ = state.free_mesh_id(id);
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::DropMesh { id }));
        true
    }
}

impl RuntimeResourceApi {
    pub(crate) fn canonical_mesh_id(&self, mesh: MeshID) -> MeshID {
        let state = self.state.lock().expect("resource api mutex poisoned");
        state.mesh_id_alias.get(&mesh).copied().unwrap_or(mesh)
    }

    pub(crate) fn is_mesh_id_pending(&self, mesh: MeshID) -> bool {
        let canonical = self.canonical_mesh_id(mesh);
        let state = self.state.lock().expect("resource api mutex poisoned");
        state
            .mesh_pending_id_by_request
            .values()
            .copied()
            .any(|pending| {
                state
                    .mesh_id_alias
                    .get(&pending)
                    .copied()
                    .unwrap_or(pending)
                    == canonical
            })
    }

    pub(crate) fn mesh_revision(&self, mesh: MeshID) -> Option<u64> {
        let canonical = self.canonical_mesh_id(mesh);
        let state = self.state.lock().expect("resource api mutex poisoned");
        state.mesh_data_by_id.get(&canonical)?;
        Some(
            state
                .mesh_revision_by_id
                .get(&canonical)
                .copied()
                .unwrap_or(0),
        )
    }

    pub(crate) fn with_mesh_data_and_revision<R>(
        &self,
        mesh: MeshID,
        f: impl FnOnce(&Mesh3D, u64) -> R,
    ) -> Option<R> {
        let canonical = self.canonical_mesh_id(mesh);
        let state = self.state.lock().expect("resource api mutex poisoned");
        let data = state.mesh_data_by_id.get(&canonical)?;
        let revision = state
            .mesh_revision_by_id
            .get(&canonical)
            .copied()
            .unwrap_or(0);
        Some(f(data, revision))
    }

    pub(crate) fn mesh_source(&self, mesh: MeshID) -> Option<String> {
        let canonical = self.canonical_mesh_id(mesh);
        let state = self.state.lock().expect("resource api mutex poisoned");
        state.mesh_source_by_id.get(&canonical).cloned()
    }

    pub(crate) fn register_loaded_mesh_source(&self, source: &str, id: MeshID) {
        let normalized = normalize_source_slashes(source);
        let source = normalized.as_ref();
        let source_hash = string_to_u64(source);
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if source.trim().is_empty() || id.is_nil() {
            return;
        }
        state.mesh_by_source.insert(source_hash, id);
        state.mesh_source_by_id.insert(id, source.to_string());
        state.mesh_loaded_by_id.insert(id);
        if let Some(request) = state.mesh_pending_by_source.remove(&source_hash) {
            state.mesh_pending_source_by_request.remove(&request);
            state.mesh_pending_id_by_request.remove(&request);
        }
        state.mesh_reserve_pending.remove(&source_hash);
        state.mesh_drop_pending.remove(&source_hash);
    }
}

fn normalize_source_slashes(source: &str) -> std::borrow::Cow<'_, str> {
    if source.contains('\\') {
        std::borrow::Cow::Owned(source.replace('\\', "/"))
    } else {
        std::borrow::Cow::Borrowed(source)
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeResourceApi;
    use perro_render_bridge::{Material3D, Mesh3D, RenderCommand, RenderEvent, ResourceCommand};
    use perro_resource_api::{
        ResourceWindow, material_load, material_reserve, mesh_load, mesh_reserve, texture_load,
        texture_reserve,
    };

    fn new_api() -> std::sync::Arc<RuntimeResourceApi> {
        RuntimeResourceApi::new(None, None, None, None, None, None, None, None)
    }

    fn static_material_lookup(_: u64) -> &'static Material3D {
        static MATERIAL: Material3D =
            Material3D::Standard(perro_render_bridge::StandardMaterial3D::const_default());
        &MATERIAL
    }

    fn new_static_material_api() -> std::sync::Arc<RuntimeResourceApi> {
        RuntimeResourceApi::new(
            Some(static_material_lookup),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
    }

    fn empty_mesh() -> Mesh3D {
        Mesh3D {
            vertices: Vec::new(),
            indices: Vec::new(),
            surface_ranges: Vec::new(),
            blend_shapes: Vec::new(),
        }
    }

    #[test]
    fn mesh_reserve_macro_promotes_existing_id() {
        let api = new_api();
        let res = ResourceWindow::new(api.as_ref());
        let mesh = mesh_load!(res, "res://meshes/promote.glb:mesh[0]");
        let promoted = mesh_reserve!(res, mesh);

        assert_eq!(promoted, mesh);

        let mut commands = Vec::new();
        api.drain_commands(&mut commands);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Resource(ResourceCommand::SetMeshReserved { id, reserved: true })
                if *id == mesh
        )));
    }

    #[test]
    fn texture_reserve_macro_promotes_existing_id() {
        let api = new_api();
        let res = ResourceWindow::new(api.as_ref());
        let texture = texture_load!(res, "res://textures/promote.png");
        let promoted = texture_reserve!(res, texture);

        assert_eq!(promoted, texture);

        let mut commands = Vec::new();
        api.drain_commands(&mut commands);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Resource(ResourceCommand::SetTextureReserved { id, reserved: true })
                if *id == texture
        )));
    }

    #[test]
    fn material_reserve_macro_promotes_existing_id() {
        let api = new_api();
        let res = ResourceWindow::new(api.as_ref());
        let material = material_load!(res, "res://materials/promote.pmat");
        let promoted = material_reserve!(res, material);

        assert_eq!(promoted, material);

        let mut commands = Vec::new();
        api.drain_commands(&mut commands);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Resource(ResourceCommand::SetMaterialReserved { id, reserved: true })
                if *id == material
        )));
    }

    #[test]
    fn nil_mesh_id_is_loaded() {
        let api = new_api();
        let res = ResourceWindow::new(api.as_ref());

        assert!(res.Meshes().is_loaded(perro_ids::MeshID::nil()));
    }

    #[test]
    fn created_mesh_is_not_loaded_until_backend_slot_exists() {
        let api = new_api();
        let res = ResourceWindow::new(api.as_ref());
        let mesh = res.Meshes().create(empty_mesh());
        assert!(!res.Meshes().is_loaded(mesh));

        let request = {
            let mut commands = Vec::new();
            api.drain_commands(&mut commands);
            commands
                .into_iter()
                .find_map(|command| match command {
                    RenderCommand::Resource(ResourceCommand::CreateRuntimeMesh {
                        request,
                        id,
                        ..
                    }) if id == mesh => Some(request),
                    _ => None,
                })
                .expect("expected runtime mesh create command")
        };

        api.apply_render_event(&RenderEvent::MeshCreated {
            request,
            id: mesh,
            mesh: Some(empty_mesh()),
        });
        assert!(res.Meshes().is_loaded(mesh));
    }

    #[test]
    fn mesh_created_without_cpu_data_counts_loaded_after_backend_ack() {
        let api = new_api();
        let res = ResourceWindow::new(api.as_ref());
        let mesh = mesh_load!(res, "__cube__");
        assert!(!res.Meshes().is_loaded(mesh));

        let request = {
            let mut commands = Vec::new();
            api.drain_commands(&mut commands);
            commands
                .into_iter()
                .find_map(|command| match command {
                    RenderCommand::Resource(ResourceCommand::CreateMesh {
                        request, id, ..
                    }) if id == mesh => Some(request),
                    _ => None,
                })
                .expect("expected mesh create command")
        };

        api.apply_render_event(&RenderEvent::MeshCreated {
            request,
            id: mesh,
            mesh: None,
        });
        assert!(res.Meshes().is_loaded(mesh));
    }

    #[test]
    fn created_material_is_loaded_after_backend_create_ack() {
        let api = new_api();
        let res = ResourceWindow::new(api.as_ref());
        let material = res.Materials().create(Material3D::default());
        assert!(!res.Materials().is_loaded(material));

        let request = {
            let mut commands = Vec::new();
            api.drain_commands(&mut commands);
            commands
                .into_iter()
                .find_map(|command| match command {
                    RenderCommand::Resource(ResourceCommand::CreateMaterial {
                        request,
                        id,
                        ..
                    }) if id == material => Some(request),
                    _ => None,
                })
                .expect("expected material create command")
        };

        api.apply_render_event(&RenderEvent::MaterialCreated {
            request,
            id: material,
        });
        assert!(res.Materials().is_loaded(material));
    }

    #[test]
    fn static_material_load_is_loaded_after_backend_create_ack() {
        let api = new_static_material_api();
        let res = ResourceWindow::new(api.as_ref());
        let material = material_load!(res, "res://materials/static.pmat");
        assert!(!res.Materials().is_loaded(material));

        let request = {
            let mut commands = Vec::new();
            api.drain_commands(&mut commands);
            commands
                .into_iter()
                .find_map(|command| match command {
                    RenderCommand::Resource(ResourceCommand::CreateMaterial {
                        request,
                        id,
                        ..
                    }) if id == material => Some(request),
                    _ => None,
                })
                .expect("expected static material create command")
        };

        api.apply_render_event(&RenderEvent::MaterialCreated {
            request,
            id: material,
        });
        assert!(res.Materials().is_loaded(material));
    }

    #[test]
    fn async_material_waits_for_source_task() {
        let api = new_api();
        let res = ResourceWindow::new(api.as_ref());
        let material = res.Materials().create(Material3D::default());
        {
            let mut state = api.state.lock().expect("resource api mutex poisoned");
            state.material_load_pending_by_id.insert(material);
        }

        let request = {
            let mut commands = Vec::new();
            api.drain_commands(&mut commands);
            commands
                .into_iter()
                .find_map(|command| match command {
                    RenderCommand::Resource(ResourceCommand::CreateMaterial {
                        request,
                        id,
                        ..
                    }) if id == material => Some(request),
                    _ => None,
                })
                .expect("expected material create command")
        };

        api.apply_render_event(&RenderEvent::MaterialCreated {
            request,
            id: material,
        });
        assert!(!res.Materials().is_loaded(material));

        api.apply_render_event(&RenderEvent::MaterialLoaded { id: material });
        assert!(!res.Materials().is_loaded(material));

        {
            let mut state = api.state.lock().expect("resource api mutex poisoned");
            state.material_load_pending_by_id.remove(&material);
        }
        api.apply_render_event(&RenderEvent::MaterialLoaded { id: material });
        assert!(res.Materials().is_loaded(material));
    }

    #[test]
    fn write_material_keeps_material_loaded() {
        let api = new_api();
        let res = ResourceWindow::new(api.as_ref());
        let material = res.Materials().create(Material3D::default());

        let request = {
            let mut commands = Vec::new();
            api.drain_commands(&mut commands);
            commands
                .into_iter()
                .find_map(|command| match command {
                    RenderCommand::Resource(ResourceCommand::CreateMaterial {
                        request,
                        id,
                        ..
                    }) if id == material => Some(request),
                    _ => None,
                })
                .expect("expected material create command")
        };

        api.apply_render_event(&RenderEvent::MaterialCreated {
            request,
            id: material,
        });
        api.apply_render_event(&RenderEvent::MaterialLoaded { id: material });
        assert!(res.Materials().is_loaded(material));

        assert!(res.Materials().write(material, Material3D::default()));
        assert!(res.Materials().is_loaded(material));
    }
}
