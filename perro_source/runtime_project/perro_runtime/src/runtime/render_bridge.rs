use super::Runtime;
use crate::render_result::RuntimeRenderResult;
use perro_ids::NodeID;
use perro_render_bridge::{RenderCommand, RenderEvent, RenderRequestID};

impl Runtime {
    pub(crate) const UI_DIRTY_TRANSFORM: u16 = crate::runtime::state::DirtyState::DIRTY_TRANSFORM;
    pub(crate) const UI_DIRTY_LAYOUT_SELF: u16 =
        crate::runtime::state::DirtyState::DIRTY_LAYOUT_SELF;
    pub(crate) const UI_DIRTY_LAYOUT_PARENT: u16 =
        crate::runtime::state::DirtyState::DIRTY_LAYOUT_PARENT;
    pub(crate) const UI_DIRTY_COMMANDS: u16 = crate::runtime::state::DirtyState::DIRTY_COMMANDS;
    pub(crate) const UI_DIRTY_TEXT: u16 = crate::runtime::state::DirtyState::DIRTY_TEXT;

    pub fn queue_render_command(&mut self, command: RenderCommand) {
        self.render.queue_command(command);
    }

    pub fn drain_render_commands(&mut self, out: &mut Vec<RenderCommand>) {
        let mut queued_resource_commands = self.render.take_resource_queue_scratch();
        queued_resource_commands.clear();
        self.resource_api
            .drain_commands(&mut queued_resource_commands);
        if !queued_resource_commands.is_empty() {
            self.render.queue_commands(&mut queued_resource_commands);
        }
        self.render
            .restore_resource_queue_scratch(queued_resource_commands);
        self.render.drain_commands(out);
    }

    pub fn apply_render_event(&mut self, event: RenderEvent) {
        if let RenderEvent::MeshCreated { request, id } = &event
            && let Some(node) = decode_3d_mesh_request_node(*request)
            && let Some(source) = self.render_3d.mesh_sources.get(&node).cloned()
        {
            self.resource_api.register_loaded_mesh_source(&source, *id);
        }
        if let Some(node) = decode_render_request_node_from_event(&event) {
            self.mark_needs_rerender(node);
        }
        self.resource_api.apply_render_event(&event);
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

    pub fn has_inflight_render_requests(&self) -> bool {
        self.render.has_inflight_requests()
    }

    pub fn has_resolved_render_requests(&self) -> bool {
        self.render.has_resolved_requests()
    }

    pub fn is_render_request_inflight(&self, request: RenderRequestID) -> bool {
        self.render.is_request_inflight(request)
    }

    pub fn copy_inflight_render_requests(&self, out: &mut Vec<RenderRequestID>) {
        self.render.copy_inflight_requests(out);
    }

    pub fn mark_needs_rerender(&mut self, id: NodeID) {
        self.dirty.mark_rerender(id);
    }

    pub fn force_rerender(&mut self, root_id: NodeID) {
        if root_id.is_nil() || self.nodes.get(root_id).is_none() {
            return;
        }

        let mut stack = vec![root_id];
        while let Some(id) = stack.pop() {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            self.dirty.mark_rerender(id);
            stack.extend(node.children_slice().iter().copied());
        }
    }

    pub(crate) fn mark_ui_dirty(&mut self, id: NodeID, flags: u16) {
        self.dirty.mark_ui(id, flags);
    }

    pub fn mark_transform_dirty_recursive(&mut self, root: NodeID) {
        self.dirty.mark_transform_root(root);
    }

    pub fn clear_dirty_flags(&mut self) {
        self.dirty.clear();
    }

    pub fn clear_dirty_flags_keep_ui(&mut self) {
        self.dirty.clear_keep_ui_dirty();
    }

    pub fn dirty_node_count(&self) -> usize {
        self.dirty.dirty_count()
    }

    pub(crate) fn note_removed_render_node(&mut self, node: NodeID) {
        self.render_2d.removed_nodes.push(node);
        self.render_3d.removed_nodes.push(node);
        self.render_ui.removed_nodes.push(node);
    }
}

#[inline]
fn decode_3d_mesh_request_node(request: RenderRequestID) -> Option<NodeID> {
    if (request.0 & 0xFF) != 0x3E {
        return None;
    }
    Some(NodeID::from_u64(request.0 >> 8))
}

#[inline]
fn decode_2d_texture_request_node(request: RenderRequestID) -> Option<NodeID> {
    if (request.0 & 0xFF) != 0x2D {
        return None;
    }
    Some(NodeID::from_u64(request.0 >> 8))
}

#[inline]
fn decode_3d_material_request_node(request: RenderRequestID) -> Option<NodeID> {
    if (request.0 & 0xFF) != 0x3F {
        return None;
    }
    Some(NodeID::from_u64(request.0 >> 16))
}

#[inline]
fn decode_render_request_node(request: RenderRequestID) -> Option<NodeID> {
    decode_2d_texture_request_node(request)
        .or_else(|| decode_3d_mesh_request_node(request))
        .or_else(|| decode_3d_material_request_node(request))
}

#[inline]
fn decode_render_request_node_from_event(event: &RenderEvent) -> Option<NodeID> {
    let request = match event {
        RenderEvent::MeshCreated { request, .. }
        | RenderEvent::TextureCreated { request, .. }
        | RenderEvent::MaterialCreated { request, .. }
        | RenderEvent::Failed { request, .. } => *request,
    };
    decode_render_request_node(request)
}
