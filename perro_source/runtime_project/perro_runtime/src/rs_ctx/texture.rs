use super::core::RuntimeResourceApi;
use perro_ids::{TextureID, WebcamID, string_to_u64};
use perro_render_bridge::{RenderCommand, ResourceCommand};
use perro_resource_api::sub_apis::TextureAPI;
use std::sync::Arc;

fn expected_rgba_len(width: u32, height: u32) -> Option<usize> {
    (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)
}

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

    fn create_texture_from_rgba(&self, width: u32, height: u32, rgba: &[u8]) -> TextureID {
        if width == 0 || height == 0 || expected_rgba_len(width, height) != Some(rgba.len()) {
            return TextureID::nil();
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let request = state.allocate_request();
        let id = state.allocate_texture_id();
        let source = format!("runtime://texture/{}:{}", id.index(), id.generation());
        let source_hash = string_to_u64(&source);
        state.texture_by_source.insert(source_hash, id);
        state.texture_pending_by_source.insert(source_hash, request);
        state
            .texture_pending_source_by_request
            .insert(request, source.clone());
        state.texture_pending_id_by_request.insert(request, id);
        state.queued_commands.push(RenderCommand::Resource(
            ResourceCommand::CreateRuntimeTexture {
                request,
                id,
                source,
                reserved: false,
                width,
                height,
                rgba: Arc::from(rgba),
            },
        ));
        id
    }

    fn create_texture_from_bytes(&self, bytes: &[u8]) -> TextureID {
        if bytes.is_empty() {
            return TextureID::nil();
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let request = state.allocate_request();
        let id = state.allocate_texture_id();
        let source = format!("runtime://texture-bytes/{}:{}", id.index(), id.generation());
        let source_hash = string_to_u64(&source);
        state.texture_by_source.insert(source_hash, id);
        state.texture_pending_by_source.insert(source_hash, request);
        state
            .texture_pending_source_by_request
            .insert(request, source.clone());
        state.texture_pending_id_by_request.insert(request, id);
        state.queued_commands.push(RenderCommand::Resource(
            ResourceCommand::CreateRuntimeTextureBytes {
                request,
                id,
                source,
                reserved: false,
                bytes: Arc::from(bytes),
            },
        ));
        id
    }

    fn write_texture_rgba(&self, id: TextureID, width: u32, height: u32, rgba: &[u8]) -> bool {
        if id.is_nil()
            || width == 0
            || height == 0
            || expected_rgba_len(width, height) != Some(rgba.len())
        {
            return false;
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if !texture_id_known(&state, id) {
            return false;
        }
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::WriteTextureRgba {
                id,
                width,
                height,
                // owned copy of caller slice; backend moves it into the resident
                // buffer path (StreamRgba::Owned, no Arc round trip).
                rgba: rgba.to_vec().into(),
            }));
        true
    }

    fn write_texture_rgba_region(
        &self,
        id: TextureID,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> bool {
        if id.is_nil()
            || x.checked_add(width).is_none()
            || y.checked_add(height).is_none()
            || width == 0
            || height == 0
            || expected_rgba_len(width, height) != Some(rgba.len())
        {
            return false;
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if !texture_id_known(&state, id) {
            return false;
        }
        state.queued_commands.push(RenderCommand::Resource(
            ResourceCommand::WriteTextureRgbaRegion {
                id,
                x,
                y,
                width,
                height,
                rgba: Arc::from(rgba),
            },
        ));
        true
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

    fn webcam_texture(&self, webcam: WebcamID) -> TextureID {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.webcam_texture_by_id.get(&webcam).copied())
            .unwrap_or_else(TextureID::nil)
    }
}

fn texture_id_known(state: &super::state::RuntimeResourceState, id: TextureID) -> bool {
    state.texture_loaded_by_id.contains(&id)
        || state.texture_by_source.values().any(|known| *known == id)
        || state
            .texture_pending_id_by_request
            .values()
            .any(|pending| *pending == id)
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

// Texture arms of the render-event stream; called from
// `RuntimeResourceApi::apply_render_event` under the state lock.
impl super::state::RuntimeResourceState {
    pub(super) fn apply_texture_created(
        &mut self,
        request: perro_render_bridge::RenderRequestID,
        id: TextureID,
    ) {
        let _ = self.occupy_texture_id(id);
        if let Some(source) = self.texture_pending_source_by_request.remove(&request) {
            let source_hash = string_to_u64(&source);
            self.texture_pending_by_source.remove(&source_hash);
            let pending_id = self.texture_pending_id_by_request.remove(&request);
            if self.texture_drop_pending.remove(&source_hash) {
                self.queued_commands
                    .push(RenderCommand::Resource(ResourceCommand::DropTexture { id }));
                self.texture_by_source.remove(&source_hash);
                if let Some(pending_id) = pending_id {
                    let _ = self.free_texture_id(pending_id);
                }
            } else {
                self.texture_by_source.insert(source_hash, id);
                if self.texture_reserve_pending.remove(&source_hash) {
                    self.queued_commands.push(RenderCommand::Resource(
                        ResourceCommand::SetTextureReserved { id, reserved: true },
                    ));
                }
            }
        }
    }

    pub(super) fn apply_texture_dropped(&mut self, id: TextureID) {
        self.texture_loaded_by_id.remove(&id);
        let source = self
            .texture_by_source
            .iter()
            .find_map(|(source_hash, existing)| (*existing == id).then_some(*source_hash));
        if let Some(source_hash) = source {
            self.texture_by_source.remove(&source_hash);
            self.texture_pending_by_source.remove(&source_hash);
            self.texture_reserve_pending.remove(&source_hash);
            self.texture_drop_pending.remove(&source_hash);
        }
        let _ = self.free_texture_id(id);
    }

    pub(super) fn apply_texture_failed(&mut self, request: perro_render_bridge::RenderRequestID) {
        if let Some(source) = self.texture_pending_source_by_request.remove(&request) {
            let source_hash = string_to_u64(&source);
            self.texture_pending_by_source.remove(&source_hash);
            if let Some(pending_id) = self.texture_pending_id_by_request.remove(&request) {
                let _ = self.free_texture_id(pending_id);
            }
            self.texture_by_source.remove(&source_hash);
            self.texture_reserve_pending.remove(&source_hash);
            self.texture_drop_pending.remove(&source_hash);
        }
    }
}
