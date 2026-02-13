use crate::{
    NodeArena, ScriptCollection,
    render_result::RuntimeRenderResult,
    runtime_project::{ProviderMode, RuntimeProject},
};
use ahash::{AHashMap, AHashSet};
use perro_core::Spatial;
use perro_ids::{NodeID, TextureID};
use perro_render_bridge::{RenderCommand, RenderEvent, RenderRequestID};
use std::sync::Arc;

mod render_2d;
mod render_3d;

pub struct Runtime {
    pub time: Timing,
    provider_mode: ProviderMode,
    project: Option<Arc<RuntimeProject>>,

    // Core world state
    pub nodes: NodeArena,
    pub scripts: ScriptCollection<Self>,
    schedules: ScriptSchedules,
    render: RenderState,
    dirty: DirtyState,
    traversal_stack: Vec<NodeID>,


    render_2d: Render2DState,
    render_3d: Render3DState,
    debug: DebugRenderState,
}

pub struct Timing {
    pub fixed_delta: f32,
    pub delta: f32,
    pub elapsed: f32,
}

/// Scratch buffers used to snapshot script update/fixed schedules without allocating each frame.
struct ScriptSchedules {
    update_ids: Vec<NodeID>,
    fixed_ids: Vec<NodeID>,
}

impl ScriptSchedules {
    #[inline]
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
        let needed = scripts.update_schedule_len();
        if self.update_ids.capacity() < needed {
            self.update_ids
                .reserve_exact(needed - self.update_ids.capacity());
        }
        self.update_ids.clear();
        scripts.append_update_ids(&mut self.update_ids);
    }

    fn snapshot_fixed<R: perro_api::api::RuntimeAPI + ?Sized>(
        &mut self,
        scripts: &ScriptCollection<R>,
    ) {
        let needed = scripts.fixed_schedule_len();
        if self.fixed_ids.capacity() < needed {
            self.fixed_ids
                .reserve_exact(needed - self.fixed_ids.capacity());
        }
        self.fixed_ids.clear();
        scripts.append_fixed_update_ids(&mut self.fixed_ids);
    }
}

/// Runtime-side render exchange state:
/// queued outgoing commands and resolved incoming request results.
struct RenderState {
    pending_commands: Vec<RenderCommand>,
    resolved_requests: AHashMap<RenderRequestID, RuntimeRenderResult>,
    inflight_requests: AHashSet<RenderRequestID>,
}

impl RenderState {
    fn new() -> Self {
        Self {
            pending_commands: Vec::new(),
            resolved_requests: AHashMap::default(),
            inflight_requests: AHashSet::default(),
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
                self.inflight_requests.remove(&request);
                self.resolved_requests
                    .insert(request, RuntimeRenderResult::Mesh(id));
            }
            RenderEvent::TextureCreated { request, id } => {
                self.inflight_requests.remove(&request);
                self.resolved_requests
                    .insert(request, RuntimeRenderResult::Texture(id));
            }
            RenderEvent::MaterialCreated { request, id } => {
                self.inflight_requests.remove(&request);
                self.resolved_requests
                    .insert(request, RuntimeRenderResult::Material(id));
            }
            RenderEvent::Failed { request, reason } => {
                self.inflight_requests.remove(&request);
                self.resolved_requests
                    .insert(request, RuntimeRenderResult::Failed(reason));
            }
        }
    }

    fn take_result(&mut self, request: RenderRequestID) -> Option<RuntimeRenderResult> {
        self.resolved_requests.remove(&request)
    }

    fn is_inflight(&self, request: RenderRequestID) -> bool {
        self.inflight_requests.contains(&request)
    }

    fn mark_inflight(&mut self, request: RenderRequestID) {
        self.inflight_requests.insert(request);
    }
}

/// Runtime-side dirty tracking for downstream systems (rendering, transform propagation).
struct DirtyState {
    node_flags: AHashMap<NodeID, u8>,
}

struct Render2DState {
    traversal_ids: Vec<NodeID>,
    visible_now: AHashSet<NodeID>,
    prev_visible: AHashSet<NodeID>,
    retained_sprite_textures: AHashMap<NodeID, TextureID>,
    removed_nodes: Vec<NodeID>,
}

impl Render2DState {
    fn new() -> Self {
        Self {
            traversal_ids: Vec::new(),
            visible_now: AHashSet::default(),
            prev_visible: AHashSet::default(),
            retained_sprite_textures: AHashMap::default(),
            removed_nodes: Vec::new(),
        }
    }
}

struct Render3DState {
    traversal_ids: Vec<NodeID>,
}

impl Render3DState {
    fn new() -> Self {
        Self {
            traversal_ids: Vec::new(),
        }
    }
}

struct DebugRenderState {
    draw_rect: bool,
    rect_was_active: bool,
}

impl DebugRenderState {
    fn new() -> Self {
        Self {
            draw_rect: false,
            rect_was_active: false,
        }
    }
}

impl DirtyState {
    const FLAG_RERENDER: u8 = 1 << 0;
    const FLAG_DIRTY_2D_TRANSFORM: u8 = 1 << 1;
    const FLAG_DIRTY_3D_TRANSFORM: u8 = 1 << 2;

    fn new() -> Self {
        Self {
            node_flags: AHashMap::default(),
        }
    }

    fn mark_rerender(&mut self, id: NodeID) {
        self.mark(id, Self::FLAG_RERENDER);
    }

    fn mark_transform(&mut self, id: NodeID, spatial: Spatial) {
        match spatial {
            Spatial::TwoD => {
                self.mark(id, Self::FLAG_DIRTY_2D_TRANSFORM);
            }
            Spatial::ThreeD => {
                self.mark(id, Self::FLAG_DIRTY_3D_TRANSFORM);
            }
            Spatial::None => {}
        }
    }

    #[inline]
    fn mark(&mut self, id: NodeID, flag: u8) {
        self.node_flags
            .entry(id)
            .and_modify(|flags| *flags |= flag)
            .or_insert(flag);
    }

    fn clear(&mut self) {
        self.node_flags.clear();
    }
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            time: Timing {
                fixed_delta: 0.0,
                delta: 0.0,
                elapsed: 0.0,
            },
            provider_mode: ProviderMode::Dynamic,
            nodes: NodeArena::new(),
            scripts: ScriptCollection::new(),
            schedules: ScriptSchedules::new(),
            project: None,
            render: RenderState::new(),
            dirty: DirtyState::new(),
            traversal_stack: Vec::new(),
            render_2d: Render2DState::new(),
            render_3d: Render3DState::new(),
            debug: DebugRenderState::new(),
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

    #[inline]
    pub fn update(&mut self, delta_time: f32) {
        self.time.delta = delta_time;
        self.schedules.snapshot_update(&self.scripts);
        self.run_update_schedule();
    }

    #[inline]
    pub fn fixed_update(&mut self, fixed_delta_time: f32) {
        self.time.fixed_delta = fixed_delta_time;
        self.schedules.snapshot_fixed(&self.scripts);
        self.run_fixed_schedule();
    }

    #[inline]
    pub fn set_debug_draw_rect(&mut self, enabled: bool) {
        self.debug.draw_rect = enabled;
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

    pub fn mark_needs_rerender(&mut self, id: NodeID) {
        self.dirty.mark_rerender(id);
    }

    pub fn mark_transform_dirty_recursive(&mut self, root: NodeID) {
        let mut stack = std::mem::take(&mut self.traversal_stack);
        stack.clear();
        stack.push(root);
        while let Some(id) = stack.pop() {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };

            self.dirty.mark_transform(id, node.spatial());
            stack.extend(node.children_slice().iter().copied());
        }
        self.traversal_stack = stack;
    }

    fn run_update_schedule(&mut self) {
        let mut i = 0;
        while i < self.schedules.update_ids.len() {
            let id = self.schedules.update_ids[i];
            self.call_update_script(id);
            i += 1;
        }
    }

    fn run_fixed_schedule(&mut self) {
        let mut i = 0;
        while i < self.schedules.fixed_ids.len() {
            let id = self.schedules.fixed_ids[i];
            self.call_fixed_update_script(id);
            i += 1;
        }
    }

    pub fn clear_dirty_flags(&mut self) {
        self.dirty.clear();
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
