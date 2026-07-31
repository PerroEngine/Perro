use crate::retained::{
    decode_3d_material_request_node, mesh_3d_request, sprite_2d_texture_request,
    tilemap_2d_texture_request, ui_image_texture_request,
};
use ahash::{AHashMap, AHashSet};
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_render_bridge::{RenderCommand, RenderEvent, RenderRequestID};

#[derive(Debug, Clone)]
pub enum RuntimeRenderResult {
    Mesh(MeshID),
    Texture(TextureID),
    Material(MaterialID),
    Failed(String),
}

/// Runtime-side render exchange state:
/// queued outgoing commands and resolved incoming request results.
pub struct RenderState {
    pending_commands: Vec<RenderCommand>,
    queued_resource_commands_scratch: Vec<RenderCommand>,
    resolved_requests: AHashMap<RenderRequestID, RuntimeRenderResult>,
    inflight_requests: AHashSet<RenderRequestID>,
}

impl RenderState {
    pub fn new() -> Self {
        Self {
            pending_commands: Vec::new(),
            queued_resource_commands_scratch: Vec::new(),
            resolved_requests: AHashMap::default(),
            inflight_requests: AHashSet::default(),
        }
    }

    pub fn queue_command(&mut self, command: RenderCommand) {
        self.pending_commands.push(command);
    }

    pub fn queue_commands(&mut self, commands: &mut Vec<RenderCommand>) {
        self.pending_commands.reserve(commands.len());
        self.pending_commands.append(commands);
    }

    pub fn drain_commands(&mut self, out: &mut Vec<RenderCommand>) {
        out.reserve(self.pending_commands.len());
        out.append(&mut self.pending_commands);
    }

    pub fn take_resource_queue_scratch(&mut self) -> Vec<RenderCommand> {
        std::mem::take(&mut self.queued_resource_commands_scratch)
    }

    pub fn restore_resource_queue_scratch(&mut self, mut scratch: Vec<RenderCommand>) {
        scratch.clear();
        self.queued_resource_commands_scratch = scratch;
    }

    pub fn apply_event(&mut self, event: RenderEvent) {
        match event {
            RenderEvent::MeshCreated { request, id, .. } => {
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
            RenderEvent::HdrStatusChanged(_)
            | RenderEvent::TextureLoaded { .. }
            | RenderEvent::TextureTexelsUpdated { .. }
            | RenderEvent::MaterialLoaded { .. }
            | RenderEvent::MeshDropped { .. }
            | RenderEvent::TextureDropped { .. }
            | RenderEvent::MaterialDropped { .. }
            | RenderEvent::WaterSamples { .. }
            | RenderEvent::WaterBodySamples { .. } => {}
        }
    }

    pub fn take_result(&mut self, request: RenderRequestID) -> Option<RuntimeRenderResult> {
        self.resolved_requests.remove(&request)
    }

    pub fn is_inflight(&self, request: RenderRequestID) -> bool {
        self.inflight_requests.contains(&request)
    }

    pub fn mark_inflight(&mut self, request: RenderRequestID) {
        self.inflight_requests.insert(request);
    }

    pub fn has_inflight_requests(&self) -> bool {
        !self.inflight_requests.is_empty()
    }

    pub fn has_resolved_requests(&self) -> bool {
        !self.resolved_requests.is_empty()
    }

    pub fn is_request_inflight(&self, request: RenderRequestID) -> bool {
        self.inflight_requests.contains(&request)
    }

    pub fn copy_inflight_requests(&self, out: &mut Vec<RenderRequestID>) {
        out.clear();
        out.extend(self.inflight_requests.iter().copied());
    }

    /// Drop every request id belonging to a freed node. Request ids embed the
    /// full `NodeID` (generation included), so once the node is gone nothing
    /// ever calls `take_result` for them again and both maps grow forever.
    /// The four fixed-tag kinds are rebuilt directly; material ids also carry
    /// a surface index, so they are decoded instead. Both maps only hold
    /// unresolved / untaken requests, so the scan is short and skipped when
    /// they are empty.
    pub fn remove_node_requests(&mut self, node: NodeID) {
        for request in [
            sprite_2d_texture_request(node),
            tilemap_2d_texture_request(node),
            ui_image_texture_request(node),
            mesh_3d_request(node),
        ] {
            self.inflight_requests.remove(&request);
            self.resolved_requests.remove(&request);
        }
        if !self.inflight_requests.is_empty() {
            self.inflight_requests
                .retain(|request| decode_3d_material_request_node(*request) != Some(node));
        }
        if !self.resolved_requests.is_empty() {
            self.resolved_requests
                .retain(|request, _| decode_3d_material_request_node(*request) != Some(node));
        }
    }

    /// Drop every pending request. Used on project reload, where
    /// `nodes.clear()` bypasses per-node teardown and would otherwise strand
    /// every id keyed by a now-dead node. Queued commands are left alone --
    /// the renderer still needs the teardown commands already staged in them.
    pub fn clear_requests(&mut self) {
        self.inflight_requests.clear();
        self.resolved_requests.clear();
    }
}

impl Default for RenderState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retained::material_3d_request;
    use perro_render_bridge::VisualAccessibilityCommand;

    #[test]
    fn queue_and_scratch_reuse_clear_inputs() {
        let mut state = RenderState::new();
        let command =
            RenderCommand::VisualAccessibility(VisualAccessibilityCommand::DisableColorBlind);
        state.queue_command(command);

        let mut more = vec![RenderCommand::VisualAccessibility(
            VisualAccessibilityCommand::DisableColorBlind,
        )];
        state.queue_commands(&mut more);
        assert!(more.is_empty());

        let mut drained = Vec::new();
        state.drain_commands(&mut drained);
        assert_eq!(drained.len(), 2);

        let mut scratch = state.take_resource_queue_scratch();
        scratch.push(RenderCommand::VisualAccessibility(
            VisualAccessibilityCommand::DisableColorBlind,
        ));
        state.restore_resource_queue_scratch(scratch);
        assert!(state.take_resource_queue_scratch().is_empty());
    }

    #[test]
    fn created_event_resolves_inflight_once() {
        let request = RenderRequestID::new(42);
        let texture = TextureID::new(7);
        let mut state = RenderState::new();

        state.mark_inflight(request);
        assert!(state.is_inflight(request));
        assert!(state.has_inflight_requests());

        state.apply_event(RenderEvent::TextureCreated {
            request,
            id: texture,
        });
        assert!(!state.is_inflight(request));
        assert!(!state.has_inflight_requests());
        assert!(state.has_resolved_requests());

        match state.take_result(request) {
            Some(RuntimeRenderResult::Texture(id)) => assert_eq!(id, texture),
            other => panic!("unexpected render result: {other:?}"),
        }
        assert!(state.take_result(request).is_none());
        assert!(!state.has_resolved_requests());
    }

    #[test]
    fn failed_event_resolves_and_loaded_event_ignores() {
        let request = RenderRequestID::new(99);
        let texture = TextureID::new(5);
        let mut state = RenderState::new();

        state.apply_event(RenderEvent::TextureLoaded { id: texture });
        assert!(!state.has_resolved_requests());

        state.mark_inflight(request);
        state.apply_event(RenderEvent::Failed {
            request,
            reason: "missing texture".to_owned(),
        });
        assert!(!state.is_request_inflight(request));

        match state.take_result(request) {
            Some(RuntimeRenderResult::Failed(reason)) => assert_eq!(reason, "missing texture"),
            other => panic!("unexpected render result: {other:?}"),
        }
    }

    #[test]
    fn removing_a_node_drops_every_request_variant() {
        let freed = NodeID::from_u64(7);
        let kept = NodeID::from_u64(8);
        let mut state = RenderState::new();

        for node in [freed, kept] {
            state.mark_inflight(sprite_2d_texture_request(node));
            state.mark_inflight(tilemap_2d_texture_request(node));
            state.mark_inflight(ui_image_texture_request(node));
            state.mark_inflight(mesh_3d_request(node));
            state.mark_inflight(material_3d_request(node, 0));
            state.mark_inflight(material_3d_request(node, 4));
            state.apply_event(RenderEvent::TextureCreated {
                request: sprite_2d_texture_request(node),
                id: TextureID::new(1),
            });
            state.apply_event(RenderEvent::MaterialCreated {
                request: material_3d_request(node, 4),
                id: MaterialID::new(2),
            });
        }

        state.remove_node_requests(freed);

        for request in [
            sprite_2d_texture_request(freed),
            tilemap_2d_texture_request(freed),
            ui_image_texture_request(freed),
            mesh_3d_request(freed),
            material_3d_request(freed, 0),
            material_3d_request(freed, 4),
        ] {
            assert!(!state.is_inflight(request));
            assert!(state.take_result(request).is_none());
        }

        assert!(state.is_inflight(mesh_3d_request(kept)));
        assert!(state.is_inflight(material_3d_request(kept, 0)));
        assert!(state.take_result(sprite_2d_texture_request(kept)).is_some());
        assert!(state.take_result(material_3d_request(kept, 4)).is_some());

        state.clear_requests();
        assert!(!state.has_inflight_requests());
        assert!(!state.has_resolved_requests());
    }

    #[test]
    fn copy_inflight_requests_replaces_output() {
        let first = RenderRequestID::new(1);
        let second = RenderRequestID::new(2);
        let mut state = RenderState::new();
        let mut out = vec![RenderRequestID::new(999)];

        state.mark_inflight(first);
        state.mark_inflight(second);
        state.copy_inflight_requests(&mut out);
        out.sort_by_key(|request| request.0);

        assert_eq!(out, vec![first, second]);
    }
}
