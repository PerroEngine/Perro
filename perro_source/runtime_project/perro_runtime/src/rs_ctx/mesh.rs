use super::core::RuntimeResourceApi;
use perro_ids::{MeshID, string_to_u64};
use perro_render_bridge::{RenderCommand, ResourceCommand};
use perro_resource_context::sub_apis::MeshAPI;

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

    fn drop_mesh(&self, id: MeshID) -> bool {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
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

    pub(crate) fn register_loaded_mesh_source(&self, source: &str, id: MeshID) {
        let normalized = normalize_source_slashes(source);
        let source = normalized.as_ref();
        let source_hash = string_to_u64(source);
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if source.trim().is_empty() || id.is_nil() {
            return;
        }
        state.mesh_by_source.insert(source_hash, id);
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
