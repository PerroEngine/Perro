use super::core::RuntimeResourceApi;
use perro_ids::MeshID;
use perro_render_bridge::{RenderCommand, ResourceCommand};
use perro_resource_context::sub_apis::MeshAPI;

impl MeshAPI for RuntimeResourceApi {
    fn load_mesh(&self, source: &str) -> MeshID {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.mesh_by_source.get(source).copied() {
            return id;
        }
        let request = state.allocate_request();
        let id = state.allocate_mesh_id();
        state.mesh_by_source.insert(source.to_string(), id);
        state
            .mesh_pending_by_source
            .insert(source.to_string(), request);
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

    fn reserve_mesh(&self, source: &str) -> MeshID {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.mesh_by_source.get(source).copied() {
            if state.mesh_pending_by_source.contains_key(source) {
                state.mesh_reserve_pending.insert(source.to_string());
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
        state.mesh_drop_pending.remove(source);
        state.mesh_reserve_pending.insert(source.to_string());
        let request = state.allocate_request();
        let id = state.allocate_mesh_id();
        state.mesh_by_source.insert(source.to_string(), id);
        state
            .mesh_pending_by_source
            .insert(source.to_string(), request);
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

    fn drop_mesh(&self, source: &str) -> bool {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.mesh_reserve_pending.remove(source);
        if state.mesh_pending_by_source.contains_key(source) {
            state.mesh_drop_pending.insert(source.to_string());
            return true;
        }
        if let Some(id) = state.mesh_by_source.remove(source) {
            let _ = state.free_mesh_id(id);
            state
                .queued_commands
                .push(RenderCommand::Resource(ResourceCommand::DropMesh { id }));
            return true;
        }
        false
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
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if source.trim().is_empty() || id.is_nil() {
            return;
        }
        state.mesh_by_source.insert(source.to_string(), id);
        if let Some(request) = state.mesh_pending_by_source.remove(source) {
            state.mesh_pending_source_by_request.remove(&request);
            state.mesh_pending_id_by_request.remove(&request);
        }
        state.mesh_reserve_pending.remove(source);
        state.mesh_drop_pending.remove(source);
    }
}
