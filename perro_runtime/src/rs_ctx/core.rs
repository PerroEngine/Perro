use super::state::RuntimeResourceState;
use perro_render_bridge::{RenderCommand, RenderEvent};
use std::sync::{Arc, Mutex};

pub struct RuntimeResourceApi {
    pub(super) state: Mutex<RuntimeResourceState>,
}

impl RuntimeResourceApi {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(RuntimeResourceState::new()),
        })
    }

    pub(crate) fn drain_commands(&self, out: &mut Vec<RenderCommand>) {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        out.append(&mut state.queued_commands);
    }

    pub(crate) fn apply_render_event(&self, event: &RenderEvent) {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        match event {
            RenderEvent::TextureCreated { request, id } => {
                if let Some(source) = state.texture_pending_source_by_request.remove(request) {
                    state.texture_pending_by_source.remove(&source);
                    state.texture_by_source.insert(source, *id);
                }
            }
            RenderEvent::MeshCreated { request, id } => {
                if let Some(source) = state.mesh_pending_source_by_request.remove(request) {
                    state.mesh_pending_by_source.remove(&source);
                    state.mesh_by_source.insert(source, *id);
                }
            }
            RenderEvent::MaterialCreated { request, id } => {
                if let Some(source) = state.material_pending_source_by_request.remove(request) {
                    state.material_pending_by_source.remove(&source);
                    state.material_by_source.insert(source, *id);
                }
            }
            RenderEvent::Failed { request, .. } => {
                if let Some(source) = state.texture_pending_source_by_request.remove(request) {
                    state.texture_pending_by_source.remove(&source);
                }
                if let Some(source) = state.mesh_pending_source_by_request.remove(request) {
                    state.mesh_pending_by_source.remove(&source);
                }
                if let Some(source) = state.material_pending_source_by_request.remove(request) {
                    state.material_pending_by_source.remove(&source);
                }
            }
        }
    }
}
