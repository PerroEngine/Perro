use super::{Render2DInput, Runtime};
use ahash::AHashSet;
use perro_core::SceneNodeData;
use perro_ids::{NodeID, TextureID};
use perro_render_bridge::{Command2D, RenderCommand, RenderRequestID, ResourceCommand, Rect2DCommand};

impl Runtime {
    const DEBUG_RECT_NODE_ID: NodeID = NodeID::from_parts(u32::MAX, 0);

    fn sprite_texture_request_id(node: NodeID) -> RenderRequestID {
        RenderRequestID::new((node.as_u64() << 8) | 0x2D)
    }

    pub fn extract_render_2d_commands(&mut self) {
        let mut render_2d_inputs = std::mem::take(&mut self.render_2d_inputs);
        render_2d_inputs.clear();
        let mut visible_now = std::mem::take(&mut self.visible_render_2d_nodes);
        visible_now.clear();
        self.removed_render_2d_nodes.clear();
        self.collect_render_2d_inputs(&mut render_2d_inputs);
        self.emit_render_2d_commands(&render_2d_inputs, &mut visible_now);
        self.remove_no_longer_visible_render_2d_nodes(&visible_now);
        self.update_debug_rect_state();

        std::mem::swap(&mut self.prev_visible_render_2d_nodes, &mut visible_now);
        visible_now.clear();
        self.visible_render_2d_nodes = visible_now;

        render_2d_inputs.clear();
        self.render_2d_inputs = render_2d_inputs;
    }

    fn collect_render_2d_inputs(&self, out: &mut Vec<Render2DInput>) {
        for (id, node) in self.nodes.iter() {
            match &node.data {
                SceneNodeData::Sprite2D(sprite) => {
                    out.push(Render2DInput::Sprite {
                        node_id: id,
                        visible: sprite.visible,
                        texture_id: sprite.texture_id,
                    });
                }
                _ => {}
            }
        }
    }

    fn emit_render_2d_commands(
        &mut self,
        inputs: &[Render2DInput],
        visible_now: &mut AHashSet<NodeID>,
    ) {
        for input in inputs.iter().copied() {
            match input {
                Render2DInput::Sprite {
                    node_id,
                    visible,
                    texture_id,
                } => {
                    self.emit_sprite_2d(node_id, visible, texture_id, visible_now);
                }
            }
        }
    }

    fn emit_sprite_2d(
        &mut self,
        node_id: NodeID,
        visible: bool,
        texture_id: TextureID,
        visible_now: &mut AHashSet<NodeID>,
    ) {
        if !visible {
            return;
        }

        let Some(resolved_texture) = self.resolve_sprite_texture(node_id, texture_id) else {
            return;
        };

        let needs_upsert = self
            .retained_sprite_textures
            .get(&node_id)
            .is_none_or(|cached| *cached != resolved_texture);
        if needs_upsert {
            self.queue_render_command(RenderCommand::TwoD(Command2D::UpsertTexture {
                texture: resolved_texture,
                node: node_id,
            }));
            self.retained_sprite_textures.insert(node_id, resolved_texture);
        }
        visible_now.insert(node_id);
    }

    fn resolve_sprite_texture(
        &mut self,
        node_id: NodeID,
        mut texture_id: TextureID,
    ) -> Option<TextureID> {
        if texture_id.is_nil() {
            let request = Self::sprite_texture_request_id(node_id);
            if let Some(result) = self.take_render_result(request) {
                match result {
                    crate::RuntimeRenderResult::Texture(id) => {
                        texture_id = id;
                        if let Some(node) = self.nodes.get_mut(node_id) {
                            if let SceneNodeData::Sprite2D(sprite) = &mut node.data {
                                sprite.texture_id = id;
                            }
                        }
                    }
                    crate::RuntimeRenderResult::Failed(_) => {}
                    crate::RuntimeRenderResult::Mesh(_) | crate::RuntimeRenderResult::Material(_) => {}
                }
            }
        }

        if texture_id.is_nil() {
            let request = Self::sprite_texture_request_id(node_id);
            if !self.render.is_inflight(request) {
                self.render.mark_inflight(request);
                self.queue_render_command(RenderCommand::Resource(ResourceCommand::CreateTexture {
                    request,
                    owner: node_id,
                }));
            }
            return None;
        }

        Some(texture_id)
    }

    fn remove_no_longer_visible_render_2d_nodes(&mut self, visible_now: &AHashSet<NodeID>) {
        for node in self.prev_visible_render_2d_nodes.iter().copied() {
            if !visible_now.contains(&node) {
                self.removed_render_2d_nodes.push(node);
            }
        }
        while let Some(node) = self.removed_render_2d_nodes.pop() {
            self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
            self.retained_sprite_textures.remove(&node);
        }
    }

    fn update_debug_rect_state(&mut self) {
        if self.debug_draw_rect && !self.debug_rect_was_active {
            self.queue_render_command(RenderCommand::TwoD(Command2D::UpsertRect {
                node: Self::DEBUG_RECT_NODE_ID,
                rect: Rect2DCommand {
                    center: [0.0, 0.0],
                    size: [120.0, 120.0],
                    color: [1.0, 0.2, 0.2, 1.0],
                    z_index: 0,
                },
            }));
            self.debug_rect_was_active = true;
        } else if !self.debug_draw_rect && self.debug_rect_was_active {
            self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode {
                node: Self::DEBUG_RECT_NODE_ID,
            }));
            self.debug_rect_was_active = false;
        }
    }
}
