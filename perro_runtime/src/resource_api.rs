use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_render_bridge::{Material3D, RenderCommand, RenderEvent, RenderRequestID, ResourceCommand};
use perro_resource_context::sub_apis::{MaterialAPI, MeshAPI, TextureAPI};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct RuntimeResourceState {
    next_request: u64,
    queued_commands: Vec<RenderCommand>,
    texture_by_source: HashMap<String, TextureID>,
    texture_pending_by_source: HashMap<String, RenderRequestID>,
    texture_pending_source_by_request: HashMap<RenderRequestID, String>,
    mesh_by_source: HashMap<String, MeshID>,
    mesh_pending_by_source: HashMap<String, RenderRequestID>,
    mesh_pending_source_by_request: HashMap<RenderRequestID, String>,
    material_by_source: HashMap<String, MaterialID>,
    material_pending_by_source: HashMap<String, RenderRequestID>,
    material_pending_source_by_request: HashMap<RenderRequestID, String>,
}

impl RuntimeResourceState {
    const REQUEST_BASE: u64 = 0x1000_0000_0000_0000;

    fn new() -> Self {
        Self {
            next_request: Self::REQUEST_BASE,
            ..Self::default()
        }
    }

    fn allocate_request(&mut self) -> RenderRequestID {
        let request = RenderRequestID::new(self.next_request);
        self.next_request = self.next_request.wrapping_add(1);
        request
    }
}

pub struct RuntimeResourceApi {
    state: Mutex<RuntimeResourceState>,
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
                owner: NodeID::nil(),
                source: source.to_string(),
            }));
        TextureID::nil()
    }
}

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
        state
            .mesh_pending_by_source
            .insert(source.to_string(), request);
        state
            .mesh_pending_source_by_request
            .insert(request, source.to_string());
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::CreateMesh {
                request,
                owner: NodeID::nil(),
                source: source.to_string(),
            }));
        MeshID::nil()
    }
}

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
                owner: NodeID::nil(),
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
                owner: NodeID::nil(),
                material,
                source: None,
            }));
        MaterialID::nil()
    }
}
