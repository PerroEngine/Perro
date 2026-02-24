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
