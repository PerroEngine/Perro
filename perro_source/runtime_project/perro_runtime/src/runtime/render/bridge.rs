//! Render bridge result intake and retained command output.

use super::Runtime;
use crate::render_result::RuntimeRenderResult;
use perro_ids::NodeID;
use perro_render_bridge::{Command2D, Command3D, RenderCommand, RenderEvent, RenderRequestID};
use perro_runtime_render::{decode_3d_mesh_request_node, decode_render_request_node_from_event};

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
        self.resource_api
            .drain_commands(&mut queued_resource_commands);
        if !queued_resource_commands.is_empty() {
            self.render.queue_commands(&mut queued_resource_commands);
        }
        self.render
            .restore_resource_queue_scratch(queued_resource_commands);
        self.render.drain_commands(out);
    }

    pub fn extract_render_snapshot_commands(&mut self, out: &mut Vec<RenderCommand>) {
        self.extract_render_2d_commands();
        self.extract_render_3d_commands();
        self.extract_render_ui_commands();
        self.drain_render_commands(out);
        self.clear_dirty_flags();
    }

    pub fn apply_render_event(&mut self, event: RenderEvent) {
        if let RenderEvent::WaterSamples { samples } = &event {
            for sample in samples.iter() {
                let sample_time = self.time.elapsed;
                let velocity_y = self
                    .water_samples
                    .get(&sample.node)
                    .zip(self.water_sample_times.get(&sample.node))
                    .and_then(|(prev, prev_time)| {
                        let dt = (sample_time - *prev_time).max(0.0);
                        (dt > 1.0e-5).then_some((sample.height - prev.height) / dt)
                    })
                    .unwrap_or(0.0);
                self.water_samples.insert(
                    sample.node,
                    perro_nodes::WaterPhysicsSample {
                        height: sample.height,
                        velocity: perro_structs::Vector2::new(sample.velocity[0], velocity_y),
                        foam: sample.foam,
                    },
                );
                self.water_sample_times.insert(sample.node, sample_time);
            }
        }
        if let RenderEvent::WaterBodySamples { samples } = &event {
            for sample in samples.iter() {
                let sample_time = self.time.elapsed;
                let velocity_y = self
                    .water_body_samples
                    .get(&crate::runtime::WaterBodySampleKey {
                        water: sample.water,
                        body: sample.body,
                        point: sample.point,
                    })
                    .and_then(|prev| {
                        let dt = (sample_time - prev.sample_time).max(0.0);
                        if dt <= 1.0e-5
                            || (prev.local.x - sample.local[0]).abs() > 0.35
                            || (prev.local.y - sample.local[1]).abs() > 0.35
                        {
                            None
                        } else {
                            Some((sample.height - prev.height) / dt)
                        }
                    })
                    .unwrap_or(0.0);
                self.water_body_samples.insert(
                    crate::runtime::WaterBodySampleKey {
                        water: sample.water,
                        body: sample.body,
                        point: sample.point,
                    },
                    crate::runtime::WaterBodySampleCache {
                        local: perro_structs::Vector2::new(sample.local[0], sample.local[1]),
                        height: sample.height,
                        velocity: perro_structs::Vector2::new(sample.velocity[0], velocity_y),
                        foam: sample.foam,
                        sample_time,
                    },
                );
            }
        }
        if let RenderEvent::MeshCreated { request, id, .. } = &event
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

    pub(crate) fn request_full_3d_scan_once(&mut self) {
        self.render_3d.request_full_scan_once();
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
        self.render_2d.note_removed_node(node);
        self.render_3d.note_removed_node(node);
        self.render_ui.note_removed_node(node);
        self.locale_text.remove_node_bindings(node);
        self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
        self.queue_render_command(RenderCommand::ThreeD(Box::new(Command3D::RemoveNode {
            node,
        })));
        self.queue_render_command(RenderCommand::Ui(
            perro_render_bridge::UiCommand::RemoveNode { node },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn water_body_samples_derive_vertical_velocity_from_height_delta() {
        let mut runtime = Runtime::new();
        let water = NodeID::from_parts(10, 0);
        let body = NodeID::from_parts(20, 0);

        runtime.time.elapsed = 1.0;
        runtime.apply_render_event(RenderEvent::WaterBodySamples {
            samples: Arc::from([perro_render_bridge::WaterBodySampleState {
                water,
                body,
                point: 0,
                local: [0.0, 0.0],
                height: 1.0,
                velocity: [0.0, 0.0],
                foam: 0.0,
            }]),
        });
        runtime.time.elapsed = 1.1;
        runtime.apply_render_event(RenderEvent::WaterBodySamples {
            samples: Arc::from([perro_render_bridge::WaterBodySampleState {
                water,
                body,
                point: 0,
                local: [0.0, 0.0],
                height: 1.3,
                velocity: [0.0, 0.0],
                foam: 0.0,
            }]),
        });

        let cached = runtime
            .water_body_samples
            .get(&crate::runtime::WaterBodySampleKey {
                water,
                body,
                point: 0,
            })
            .copied()
            .expect("cached water body sample");
        assert!(cached.velocity.y > 2.9);
    }
}
