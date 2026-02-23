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
        if state.mesh_pending_by_source.contains_key(source) {
            return MeshID::nil();
        }
        let request = state.allocate_request();
        state.mesh_pending_by_source.insert(source.to_string(), request);
        state
            .mesh_pending_source_by_request
            .insert(request, source.to_string());
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::CreateMesh {
                request,
                source: source.to_string(),
                reserved: false,
            }));
        MeshID::nil()
    }

    fn reserve_mesh(&self, source: &str) -> MeshID {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.mesh_by_source.get(source).copied() {
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
        if !state.mesh_pending_by_source.contains_key(source) {
            let request = state.allocate_request();
            state.mesh_pending_by_source.insert(source.to_string(), request);
            state
                .mesh_pending_source_by_request
                .insert(request, source.to_string());
            state
                .queued_commands
                .push(RenderCommand::Resource(ResourceCommand::CreateMesh {
                    request,
                    source: source.to_string(),
                    reserved: true,
                }));
        }
        MeshID::nil()
    }

    fn drop_mesh(&self, source: &str) -> bool {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.mesh_reserve_pending.remove(source);
        if let Some(id) = state.mesh_by_source.remove(source) {
            state
                .queued_commands
                .push(RenderCommand::Resource(ResourceCommand::DropMesh { id }));
            return true;
        }
        if state.mesh_pending_by_source.contains_key(source) {
            state.mesh_drop_pending.insert(source.to_string());
            return true;
        }
        false
    }
}
