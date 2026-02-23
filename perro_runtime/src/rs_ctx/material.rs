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
                reserved: false,
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
                reserved: false,
            }));
        MaterialID::nil()
    }

    fn reserve_material_source(&self, source: &str) -> MaterialID {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.material_by_source.get(source).copied() {
            state
                .queued_commands
                .push(RenderCommand::Resource(ResourceCommand::SetMaterialReserved {
                    id,
                    reserved: true,
                }));
            return id;
        }
        state.material_drop_pending.remove(source);
        state.material_reserve_pending.insert(source.to_string());
        if !state.material_pending_by_source.contains_key(source) {
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
                    reserved: true,
                }));
        }
        MaterialID::nil()
    }

    fn drop_material_source(&self, source: &str) -> bool {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.material_reserve_pending.remove(source);
        if let Some(id) = state.material_by_source.remove(source) {
            state
                .queued_commands
                .push(RenderCommand::Resource(ResourceCommand::DropMaterial { id }));
            return true;
        }
        if state.material_pending_by_source.contains_key(source) {
            state.material_drop_pending.insert(source.to_string());
            return true;
        }
        false
    }
}
