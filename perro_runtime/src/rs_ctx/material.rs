use super::core::RuntimeResourceApi;
use perro_ids::MaterialID;
use perro_render_bridge::{Material3D, RenderCommand, ResourceCommand};
use perro_resource_context::sub_apis::MaterialAPI;

impl MaterialAPI for RuntimeResourceApi {
    fn load_material_source(&self, source: &str) -> MaterialID {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.material_by_source.get(source).copied() {
            return id;
        }
        if state.material_pending_by_source.contains_key(source) {
            return MaterialID::nil();
        }
        let request = state.allocate_request();
        state
            .material_pending_by_source
            .insert(source.to_string(), request);
        state
            .material_pending_source_by_request
            .insert(request, source.to_string());
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::CreateMaterial {
                request,
                material: Material3D::default(),
                source: Some(source.to_string()),
            }));
        MaterialID::nil()
    }

    fn create_material(&self, material: Material3D) -> MaterialID {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let request = state.allocate_request();
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::CreateMaterial {
                request,
                material,
                source: None,
            }));
        MaterialID::nil()
    }
}
