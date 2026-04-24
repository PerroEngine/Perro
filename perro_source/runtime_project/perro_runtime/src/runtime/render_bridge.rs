use super::Runtime;
use crate::render_result::RuntimeRenderResult;
use perro_ids::NodeID;
use perro_render_bridge::{RenderCommand, RenderEvent, RenderRequestID};

impl Runtime {
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

    pub fn mark_transform_dirty_recursive(&mut self, root: NodeID) {
        self.dirty.mark_transform_root(root);
    }

    pub fn clear_dirty_flags(&mut self) {
        self.dirty.clear();
    }
}

#[inline]
fn decode_3d_mesh_request_node(request: RenderRequestID) -> Option<NodeID> {
    if (request.0 & 0xFF) != 0x3E {
        return None;
    }
    Some(NodeID::from_u64(request.0 >> 8))
}
