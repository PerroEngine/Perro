use super::core::RuntimeResourceApi;
use perro_ids::TextureID;
use perro_render_bridge::{RenderCommand, ResourceCommand};
use perro_resource_context::sub_apis::TextureAPI;

impl TextureAPI for RuntimeResourceApi {
    fn load_texture(&self, source: &str) -> TextureID {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.texture_by_source.get(source).copied() {
            return id;
        }
        let request = state.allocate_request();
        let id = state.allocate_texture_id();
        state.texture_by_source.insert(source.to_string(), id);
        state
            .texture_pending_by_source
            .insert(source.to_string(), request);
        state
            .texture_pending_source_by_request
            .insert(request, source.to_string());
        state.texture_pending_id_by_request.insert(request, id);
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::CreateTexture {
                request,
                id,
                source: source.to_string(),
                reserved: false,
            }));
        id
    }

    fn reserve_texture(&self, source: &str) -> TextureID {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.texture_by_source.get(source).copied() {
            if state.texture_pending_by_source.contains_key(source) {
                state.texture_reserve_pending.insert(source.to_string());
                return id;
            }
            state
                .queued_commands
                .push(RenderCommand::Resource(ResourceCommand::SetTextureReserved {
                    id,
                    reserved: true,
                }));
            return id;
        }
        state.texture_drop_pending.remove(source);
        state.texture_reserve_pending.insert(source.to_string());
        let request = state.allocate_request();
        let id = state.allocate_texture_id();
        state.texture_by_source.insert(source.to_string(), id);
        state
            .texture_pending_by_source
            .insert(source.to_string(), request);
        state
            .texture_pending_source_by_request
            .insert(request, source.to_string());
        state.texture_pending_id_by_request.insert(request, id);
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::CreateTexture {
                request,
                id,
                source: source.to_string(),
                reserved: true,
            }));
        id
    }

    fn drop_texture(&self, source: &str) -> bool {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.texture_reserve_pending.remove(source);
        if state.texture_pending_by_source.contains_key(source) {
            state.texture_drop_pending.insert(source.to_string());
            return true;
        }
        if let Some(id) = state.texture_by_source.remove(source) {
            let _ = state.free_texture_id(id);
            state
                .queued_commands
                .push(RenderCommand::Resource(ResourceCommand::DropTexture { id }));
            return true;
        }
        false
    }
}
