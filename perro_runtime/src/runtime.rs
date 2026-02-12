use crate::{
    NodeArena, ScriptCollection,
    render_result::RuntimeRenderResult,
    runtime_project::{ProviderMode, RuntimeProject},
};
use ahash::AHashMap;
use perro_ids::NodeID;
use perro_render_bridge::{RenderCommand, RenderEvent, RenderRequestID};
use std::sync::Arc;

pub struct Runtime {
    pub nodes: NodeArena,
    pub scripts: ScriptCollection<Self>,
    pub time: Timing,
    project: Option<Arc<RuntimeProject>>,
    provider_mode: ProviderMode,
    schedules: ScriptSchedules,
    render: RenderState,
}

pub struct Timing {
    pub delta: f32,
    pub elapsed: f32,
}

/// Scratch buffers used to snapshot script update/fixed schedules without allocating each frame.
struct ScriptSchedules {
    update_ids: Vec<NodeID>,
    fixed_ids: Vec<NodeID>,
}

impl ScriptSchedules {
    fn new() -> Self {
        Self {
            update_ids: Vec::new(),
            fixed_ids: Vec::new(),
        }
    }

    fn snapshot_update<R: perro_api::api::RuntimeAPI + ?Sized>(
        &mut self,
        scripts: &ScriptCollection<R>,
    ) {
        self.update_ids.clear();
        scripts.append_update_ids(&mut self.update_ids);
    }

    fn snapshot_fixed<R: perro_api::api::RuntimeAPI + ?Sized>(
        &mut self,
        scripts: &ScriptCollection<R>,
    ) {
        self.fixed_ids.clear();
        scripts.append_fixed_update_ids(&mut self.fixed_ids);
    }
}

/// Runtime-side render exchange state:
/// queued outgoing commands and resolved incoming request results.
struct RenderState {
    pending_commands: Vec<RenderCommand>,
    resolved_requests: AHashMap<RenderRequestID, RuntimeRenderResult>,
}

impl RenderState {
    fn new() -> Self {
        Self {
            pending_commands: Vec::new(),
            resolved_requests: AHashMap::default(),
        }
    }

    fn queue_command(&mut self, command: RenderCommand) {
        self.pending_commands.push(command);
    }

    fn drain_commands(&mut self, out: &mut Vec<RenderCommand>) {
        out.append(&mut self.pending_commands);
    }

    fn apply_event(&mut self, event: RenderEvent) {
        match event {
            RenderEvent::MeshCreated { request, id } => {
                self.resolved_requests
                    .insert(request, RuntimeRenderResult::Mesh(id));
            }
            RenderEvent::TextureCreated { request, id } => {
                self.resolved_requests
                    .insert(request, RuntimeRenderResult::Texture(id));
            }
            RenderEvent::MaterialCreated { request, id } => {
                self.resolved_requests
                    .insert(request, RuntimeRenderResult::Material(id));
            }
            RenderEvent::Failed { request, reason } => {
                self.resolved_requests
                    .insert(request, RuntimeRenderResult::Failed(reason));
            }
        }
    }

    fn take_result(&mut self, request: RenderRequestID) -> Option<RuntimeRenderResult> {
        self.resolved_requests.remove(&request)
    }
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            nodes: NodeArena::new(),
            scripts: ScriptCollection::new(),
            time: Timing {
                delta: 0.0,
                elapsed: 0.0,
            },
            project: None,
            provider_mode: ProviderMode::Dynamic,
            schedules: ScriptSchedules::new(),
            render: RenderState::new(),
        }
    }

    pub fn from_project(project: RuntimeProject, provider_mode: ProviderMode) -> Self {
        let mut runtime = Self::new();
        runtime.project = Some(Arc::new(project));
        runtime.provider_mode = provider_mode;
        runtime
    }

    pub fn project(&self) -> Option<&RuntimeProject> {
        self.project.as_deref()
    }

    pub fn provider_mode(&self) -> ProviderMode {
        self.provider_mode
    }

    pub fn update(&mut self, delta_time: f32) {
        self.time.delta = delta_time;
        self.schedules.snapshot_update(&self.scripts);

        let mut i = 0;
        while i < self.schedules.update_ids.len() {
            let id = self.schedules.update_ids[i];
            self.call_update_script(id);
            i += 1;
        }
    }

    pub fn fixed_update(&mut self, delta_time: f32) {
        self.time.delta = delta_time;
        self.schedules.snapshot_fixed(&self.scripts);

        let mut i = 0;
        while i < self.schedules.fixed_ids.len() {
            let id = self.schedules.fixed_ids[i];
            self.call_fixed_update_script(id);
            i += 1;
        }
    }

    pub fn queue_render_command(&mut self, command: RenderCommand) {
        self.render.queue_command(command);
    }

    pub fn drain_render_commands(&mut self, out: &mut Vec<RenderCommand>) {
        self.render.drain_commands(out);
    }

    pub fn apply_render_event(&mut self, event: RenderEvent) {
        self.render.apply_event(event);
    }

    pub fn apply_render_events<I>(&mut self, events: I)
    where
        I: IntoIterator<Item = RenderEvent>,
    {
        for event in events {
            self.apply_render_event(event);
        }
    }

    pub fn take_render_result(&mut self, request: RenderRequestID) -> Option<RuntimeRenderResult> {
        self.render.take_result(request)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
