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
        if state.texture_pending_by_source.contains_key(source) {
            return TextureID::nil();
        }
        let request = state.allocate_request();
        state
            .texture_pending_by_source
            .insert(source.to_string(), request);
        state
            .texture_pending_source_by_request
            .insert(request, source.to_string());
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::CreateTexture {
                request,
                source: source.to_string(),
                reserved: false,
            }));
        TextureID::nil()
    }

    fn reserve_texture(&self, source: &str) -> TextureID {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.texture_by_source.get(source).copied() {
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
        if !state.texture_pending_by_source.contains_key(source) {
            let request = state.allocate_request();
            state
                .texture_pending_by_source
                .insert(source.to_string(), request);
            state
                .texture_pending_source_by_request
                .insert(request, source.to_string());
            state
                .queued_commands
                .push(RenderCommand::Resource(ResourceCommand::CreateTexture {
                    request,
                    source: source.to_string(),
                    reserved: true,
                }));
        }
        TextureID::nil()
    }

    fn drop_texture(&self, source: &str) -> bool {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.texture_reserve_pending.remove(source);
        if let Some(id) = state.texture_by_source.remove(source) {
            state
                .queued_commands
                .push(RenderCommand::Resource(ResourceCommand::DropTexture { id }));
            return true;
        }
        if state.texture_pending_by_source.contains_key(source) {
            state.texture_drop_pending.insert(source.to_string());
            return true;
        }
        false
    }
}
