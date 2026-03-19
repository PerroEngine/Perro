use super::core::RuntimeResourceApi;
use crate::material_schema;
use perro_ids::MaterialID;
use perro_render_bridge::{Material3D, RenderCommand, ResourceCommand};
use perro_resource_context::sub_apis::MaterialAPI;

impl MaterialAPI for RuntimeResourceApi {
    fn load_material_source(&self, source: &str) -> MaterialID {
        let material = self.load_material_source_data(source).unwrap_or_default();
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.material_by_source.get(source).copied() {
            return id;
        }
        let request = state.allocate_request();
        let id = state.allocate_material_id();
        state.material_by_source.insert(source.to_string(), id);
        state
            .material_pending_by_source
            .insert(source.to_string(), request);
        state
            .material_pending_source_by_request
            .insert(request, source.to_string());
        state.material_pending_id_by_request.insert(request, id);
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::CreateMaterial {
                request,
                id,
                material,
                source: Some(source.to_string()),
                reserved: false,
            }));
        id
    }

    fn create_material(&self, material: Material3D) -> MaterialID {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let request = state.allocate_request();
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::CreateMaterial {
                request,
                id: MaterialID::nil(),
                material,
                source: None,
                reserved: false,
            }));
        MaterialID::nil()
    }

    fn reserve_material_source(&self, source: &str) -> MaterialID {
        let material = self.load_material_source_data(source).unwrap_or_default();
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.material_by_source.get(source).copied() {
            if state.material_pending_by_source.contains_key(source) {
                state.material_reserve_pending.insert(source.to_string());
                return id;
            }
            state.queued_commands.push(RenderCommand::Resource(
                ResourceCommand::SetMaterialReserved { id, reserved: true },
            ));
            return id;
        }
        state.material_drop_pending.remove(source);
        state.material_reserve_pending.insert(source.to_string());
        let request = state.allocate_request();
        let id = state.allocate_material_id();
        state.material_by_source.insert(source.to_string(), id);
        state
            .material_pending_by_source
            .insert(source.to_string(), request);
        state
            .material_pending_source_by_request
            .insert(request, source.to_string());
        state.material_pending_id_by_request.insert(request, id);
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::CreateMaterial {
                request,
                id,
                material,
                source: Some(source.to_string()),
                reserved: true,
            }));
        id
    }

    fn drop_material_source(&self, source: &str) -> bool {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.material_reserve_pending.remove(source);
        if state.material_pending_by_source.contains_key(source) {
            state.material_drop_pending.insert(source.to_string());
            return true;
        }
        if let Some(id) = state.material_by_source.remove(source) {
            let _ = state.free_material_id(id);
            state
                .queued_commands
                .push(RenderCommand::Resource(ResourceCommand::DropMaterial {
                    id,
                }));
            return true;
        }
        false
    }
}

impl RuntimeResourceApi {
    fn load_material_source_data(&self, source: &str) -> Option<Material3D> {
        let source = source.trim();
        if source.is_empty() {
            return None;
        }
        if let Some(lookup) = self.static_material_lookup {
            return lookup(source).cloned();
        }
        material_schema::load_from_source(source)
    }
}
