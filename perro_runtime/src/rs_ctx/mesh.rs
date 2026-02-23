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
            }));
        MeshID::nil()
    }
}
