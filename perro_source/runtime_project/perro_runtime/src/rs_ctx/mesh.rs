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
    pub(crate) fn register_loaded_mesh_source(&self, source: &str, id: MeshID) {
        if source.trim().is_empty() || id.is_nil() {
            return;
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let _ = state.occupy_mesh_id(id);
        state.mesh_by_source.insert(source.to_string(), id);
    }

    pub(crate) fn is_mesh_id_pending(&self, id: MeshID) -> bool {
        if id.is_nil() {
            return false;
        }
        let state = self.state.lock().expect("resource api mutex poisoned");
        state
            .mesh_pending_id_by_request
            .values()
            .any(|pending| *pending == id)
    }

    pub(crate) fn canonical_mesh_id(&self, id: MeshID) -> MeshID {
        if id.is_nil() {
            return id;
        }
        let state = self.state.lock().expect("resource api mutex poisoned");
        let mut out = id;
        // Follow remap chain pending->final, bounded to avoid accidental loops.
        for _ in 0..8 {
            let Some(next) = state.mesh_id_alias.get(&out).copied() else {
                break;
            };
            if next == out {
                break;
            }
            out = next;
        }
        out
    }
}
