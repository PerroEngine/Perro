use super::Runtime;
use crate::render_result::RuntimeRenderResult;
use perro_ids::NodeID;
use perro_render_bridge::{RenderCommand, RenderEvent, RenderRequestID};

impl Runtime {
    pub fn queue_render_command(&mut self, command: RenderCommand) {
        self.render.queue_command(command);
    }

    pub fn drain_render_commands(&mut self, out: &mut Vec<RenderCommand>) {
        let mut queued_resource_commands = Vec::new();
        self.resource_api.drain_commands(&mut queued_resource_commands);
        for command in queued_resource_commands {
            self.render.queue_command(command);
        }
        self.render.drain_commands(out);
    }

    pub fn apply_render_event(&mut self, event: RenderEvent) {
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
