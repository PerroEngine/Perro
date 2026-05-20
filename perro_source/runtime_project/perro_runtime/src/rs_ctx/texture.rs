use super::core::RuntimeResourceApi;
use perro_ids::TextureID;
use perro_render_bridge::{RenderCommand, ResourceCommand};
use perro_resource_api::sub_apis::TextureAPI;

impl TextureAPI for RuntimeResourceApi {
    fn load_texture(&self, source: &str) -> TextureID {
        if let Some(hash) = perro_ids::parse_hashed_source_uri(source) {
            self.load_texture_hashed(hash, None)
        } else {
            self.load_texture_hashed(perro_ids::string_to_u64(source), Some(source))
        }
    }

    fn reserve_texture(&self, source: &str) -> TextureID {
        if let Some(hash) = perro_ids::parse_hashed_source_uri(source) {
            self.reserve_texture_hashed(hash, None)
        } else {
            self.reserve_texture_hashed(perro_ids::string_to_u64(source), Some(source))
        }
    }

    fn load_texture_hashed(&self, source_hash: u64, source: Option<&str>) -> TextureID {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.texture_by_source.get(&source_hash).copied() {
            return id;
        }
        let Some(source) = source else {
            return TextureID::nil();
        };
        let request = state.allocate_request();
        let id = state.allocate_texture_id();
        state.texture_by_source.insert(source_hash, id);
        state.texture_pending_by_source.insert(source_hash, request);
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

    fn reserve_texture_hashed(&self, source_hash: u64, source: Option<&str>) -> TextureID {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.texture_by_source.get(&source_hash).copied() {
            if state.texture_pending_by_source.contains_key(&source_hash) {
                state.texture_reserve_pending.insert(source_hash);
                return id;
            }
            state.queued_commands.push(RenderCommand::Resource(
                ResourceCommand::SetTextureReserved { id, reserved: true },
            ));
            return id;
        }
        let Some(source) = source else {
            return TextureID::nil();
        };
        state.texture_drop_pending.remove(&source_hash);
        state.texture_reserve_pending.insert(source_hash);
        let request = state.allocate_request();
        let id = state.allocate_texture_id();
        state.texture_by_source.insert(source_hash, id);
        state.texture_pending_by_source.insert(source_hash, request);
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

    fn reserve_texture_id(&self, id: TextureID) -> bool {
        if id.is_nil() {
            return false;
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let known = state.texture_loaded_by_id.contains(&id)
            || state
                .texture_by_source
                .values()
                .any(|existing| *existing == id)
            || state
                .texture_pending_id_by_request
                .values()
                .any(|pending| *pending == id);
        if !known {
            return false;
        }
        if let Some(source_hash) = state
            .texture_by_source
            .iter()
            .find_map(|(source_hash, existing)| (*existing == id).then_some(*source_hash))
            .or_else(|| {
                state
                    .texture_pending_id_by_request
                    .iter()
                    .find_map(|(request, pending_id)| {
                        (*pending_id == id)
                            .then(|| state.texture_pending_source_by_request.get(request))
                            .flatten()
                            .map(|source| perro_ids::string_to_u64(source))
                    })
            })
        {
            state.texture_reserve_pending.insert(source_hash);
            state.texture_drop_pending.remove(&source_hash);
        }
        state.queued_commands.push(RenderCommand::Resource(
            ResourceCommand::SetTextureReserved { id, reserved: true },
        ));
        true
    }

    fn drop_texture(&self, id: TextureID) -> bool {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let source = state
            .texture_by_source
            .iter()
            .find_map(|(source_hash, existing)| (*existing == id).then_some(*source_hash));
        if let Some(source_hash) = source {
            state.texture_reserve_pending.remove(&source_hash);
            if state.texture_pending_by_source.contains_key(&source_hash) {
                state.texture_drop_pending.insert(source_hash);
                return true;
            }
            state.texture_by_source.remove(&source_hash);
        }
        let _ = state.free_texture_id(id);
        state.texture_loaded_by_id.remove(&id);
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::DropTexture { id }));
        true
    }

    fn is_texture_loaded(&self, id: TextureID) -> bool {
        if id.is_nil() {
            return false;
        }
        let state = self.state.lock().expect("resource api mutex poisoned");
        state.texture_loaded_by_id.contains(&id)
    }
}

impl RuntimeResourceApi {
    pub(crate) fn is_texture_id_pending(&self, texture: TextureID) -> bool {
        if texture.is_nil() {
            return false;
        }
        let state = self.state.lock().expect("resource api mutex poisoned");
        state
            .texture_pending_id_by_request
            .values()
            .any(|pending| *pending == texture)
    }
}
