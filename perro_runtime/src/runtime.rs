use crate::{
    NodeArena, ScriptCollection,
    render_result::RuntimeRenderResult,
    runtime_project::{ProviderMode, RuntimeProject},
};
use ahash::{AHashMap, AHashSet};
use perro_core::{SceneNodeData, Spatial};
use perro_ids::{NodeID, TextureID};
use perro_render_bridge::{Rect2DCommand, RenderCommand, RenderEvent, RenderRequestID};
use std::sync::Arc;

pub struct Runtime {
    pub time: Timing,
    provider_mode: ProviderMode,
    pub nodes: NodeArena,
    pub scripts: ScriptCollection<Self>,
    project: Option<Arc<RuntimeProject>>,
    schedules: ScriptSchedules,
    sprite_states: Vec<(NodeID, bool, TextureID)>,
    debug_draw_rect: bool,
    render: RenderState,
    dirty: DirtyState,
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
    rerender_nodes: AHashSet<NodeID>,
    dirty_2d_transforms: AHashSet<NodeID>,
    dirty_3d_transforms: AHashSet<NodeID>,
}

impl DirtyState {
    fn new() -> Self {
        Self {
            rerender_nodes: AHashSet::default(),
            dirty_2d_transforms: AHashSet::default(),
            dirty_3d_transforms: AHashSet::default(),
        }
    }

    fn mark_rerender(&mut self, id: NodeID) {
        self.rerender_nodes.insert(id);
    }

    fn mark_transform(&mut self, id: NodeID, spatial: Spatial) {
        match spatial {
            Spatial::TwoD => {
                self.dirty_2d_transforms.insert(id);
            }
            Spatial::ThreeD => {
                self.dirty_3d_transforms.insert(id);
            }
            Spatial::None => {}
        }
    }

    fn clear(&mut self) {
        self.rerender_nodes.clear();
        self.dirty_2d_transforms.clear();
        self.dirty_3d_transforms.clear();
    }
}

impl Runtime {
    fn sprite_texture_request_id(node: NodeID) -> RenderRequestID {
        RenderRequestID::new((node.as_u64() << 8) | 0x2D)
    }

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
            project: None,
            schedules: ScriptSchedules::new(),
            sprite_states: Vec::new(),
            debug_draw_rect: false,
            render: RenderState::new(),
            dirty: DirtyState::new(),
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

        let mut i = 0;
        while i < self.schedules.update_ids.len() {
            let id = self.schedules.update_ids[i];
            self.call_update_script(id);
            i += 1;
        }
    }

    #[inline]
    pub fn fixed_update(&mut self, fixed_delta_time: f32) {
        self.time.fixed_delta = fixed_delta_time;
        self.schedules.snapshot_fixed(&self.scripts);

        let mut i = 0;
        while i < self.schedules.fixed_ids.len() {
            let id = self.schedules.fixed_ids[i];
            self.call_fixed_update_script(id);
            i += 1;
        }
    }

    pub fn extract_render_2d_commands(&mut self) {
        let mut sprite_states = std::mem::take(&mut self.sprite_states);
        sprite_states.clear();
        for (id, node) in self.nodes.iter() {
            if let SceneNodeData::Sprite2D(sprite) = &node.data {
                sprite_states.push((id, sprite.visible, sprite.texture_id));
            }
        }

        for (node_id, visible, mut texture_id) in sprite_states.iter().copied() {
            if !visible {
                continue;
            }

            if texture_id.is_nil() {
                let request = Self::sprite_texture_request_id(node_id);
                if let Some(result) = self.take_render_result(request) {
                    match result {
                        RuntimeRenderResult::Texture(id) => {
                            texture_id = id;
                            if let Some(node) = self.nodes.get_mut(node_id) {
                                if let SceneNodeData::Sprite2D(sprite) = &mut node.data {
                                    sprite.texture_id = id;
                                }
                            }
                        }
                        RuntimeRenderResult::Failed(_) => {}
                        RuntimeRenderResult::Mesh(_) | RuntimeRenderResult::Material(_) => {}
                    }
                }
            }

            if texture_id.is_nil() {
                let request = Self::sprite_texture_request_id(node_id);
                if !self.render.is_inflight(request) {
                    self.render.mark_inflight(request);
                    self.queue_render_command(RenderCommand::CreateTexture {
                        request,
                        owner: node_id,
                    });
                }
                continue;
            }

            self.queue_render_command(RenderCommand::Draw2DTexture {
                texture: texture_id,
                node: node_id,
            });
        }

        if self.debug_draw_rect {
            self.queue_render_command(RenderCommand::Draw2DRect {
                node: NodeID::ROOT,
                rect: Rect2DCommand {
                    center: [0.0, 0.0],
                    size: [120.0, 120.0],
                    color: [1.0, 0.2, 0.2, 1.0],
                    z_index: 0,
                },
            });
        }

        sprite_states.clear();
        self.sprite_states = sprite_states;
    }

    #[inline]
    pub fn set_debug_draw_rect(&mut self, enabled: bool) {
        self.debug_draw_rect = enabled;
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
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };

            self.dirty.mark_transform(id, node.spatial());
            stack.extend(node.children_slice().iter().copied());
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
