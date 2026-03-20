use super::Runtime;
use ahash::AHashSet;
use perro_ids::{NodeID, TextureID};
use perro_nodes::SceneNodeData;
use perro_render_bridge::{
    Camera2DState, Command2D, RenderCommand, RenderRequestID, ResourceCommand, Sprite2DCommand,
};
use std::sync::Arc;

impl Runtime {
    fn sprite_texture_request(node: NodeID) -> RenderRequestID {
        RenderRequestID::new((node.as_u64() << 8) | 0x2D)
    }

    pub fn extract_render_2d_commands(&mut self) {
        self.propagate_pending_transform_dirty();

        let mut traversal_ids = std::mem::take(&mut self.render_2d.traversal_ids);
        traversal_ids.clear();
        traversal_ids.extend(self.nodes.iter().map(|(id, _)| id));

        let mut visible_now = std::mem::take(&mut self.render_2d.visible_now);
        visible_now.clear();
        self.render_2d.removed_nodes.clear();

        for node in traversal_ids.iter().copied() {
            let effective_visible = self.is_effectively_visible(node);
            let sprite_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::Sprite2D(sprite) => Some((
                    effective_visible && sprite.visible,
                    sprite.texture,
                    sprite.transform.to_mat3().to_cols_array_2d(),
                    sprite.z_index,
                )),
                _ => None,
            });
            if let Some((visible, texture, model, z_index)) = sprite_data {
                self.emit_sprite_2d(node, visible, texture, model, z_index, &mut visible_now);
            }

            let camera_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::Camera2D(camera) if camera.active && effective_visible => {
                    Some(Camera2DState {
                        position: [camera.transform.position.x, camera.transform.position.y],
                        rotation_radians: camera.transform.rotation,
                        zoom: camera.zoom,
                        post_processing: Arc::from(camera.post_processing.as_slice()),
                    })
                }
                _ => None,
            });
            if let Some(camera) = camera_data {
                self.queue_render_command(RenderCommand::TwoD(Command2D::SetCamera { camera }));
            }
        }
        self.remove_no_longer_visible_render_2d_nodes(&visible_now);

        std::mem::swap(&mut self.render_2d.prev_visible, &mut visible_now);
        visible_now.clear();
        self.render_2d.visible_now = visible_now;

        traversal_ids.clear();
        self.render_2d.traversal_ids = traversal_ids;
    }

    fn emit_sprite_2d(
        &mut self,
        node: NodeID,
        visible: bool,
        texture: TextureID,
        model: [[f32; 3]; 3],
        z_index: i32,
        visible_now: &mut AHashSet<NodeID>,
    ) {
        if !visible {
            return;
        }

        let Some(resolved_texture) = self.resolve_sprite_texture(node, texture) else {
            return;
        };

        let needs_upsert = self
            .render_2d
            .retained_sprite_textures
            .get(&node)
            .is_none_or(|cached| *cached != resolved_texture);
        if needs_upsert {
            self.queue_render_command(RenderCommand::TwoD(Command2D::UpsertSprite {
                node,
                sprite: Sprite2DCommand {
                    texture: resolved_texture,
                    model,
                    z_index,
                },
            }));
            self.render_2d
                .retained_sprite_textures
                .insert(node, resolved_texture);
        } else {
            self.queue_render_command(RenderCommand::TwoD(Command2D::UpsertSprite {
                node,
                sprite: Sprite2DCommand {
                    texture: resolved_texture,
                    model,
                    z_index,
                },
            }));
        }
        visible_now.insert(node);
    }

    fn resolve_sprite_texture(
        &mut self,
        node: NodeID,
        mut texture: TextureID,
    ) -> Option<TextureID> {
        if texture.is_nil() {
            let request = Self::sprite_texture_request(node);
            if let Some(result) = self.take_render_result(request) {
                match result {
                    crate::RuntimeRenderResult::Texture(id) => {
                        texture = id;
                        if let Some(node) = self.nodes.get_mut(node)
                            && let SceneNodeData::Sprite2D(sprite) = &mut node.data
                        {
                            sprite.texture = id;
                        }
                    }
                    crate::RuntimeRenderResult::Failed(_) => {}
                    crate::RuntimeRenderResult::Mesh(_)
                    | crate::RuntimeRenderResult::Material(_) => {}
                }
            }
        }

        if texture.is_nil() {
            let request = Self::sprite_texture_request(node);
            if !self.render.is_inflight(request) {
                let source = self
                    .render_2d
                    .texture_sources
                    .get(&node)
                    .cloned()
                    .unwrap_or_else(|| "__default__".to_string());
                self.render.mark_inflight(request);
                self.queue_render_command(RenderCommand::Resource(
                    ResourceCommand::CreateTexture {
                        request,
                        id: TextureID::nil(),
                        source,
                        reserved: false,
                    },
                ));
            }
            return None;
        }

        Some(texture)
    }

    fn remove_no_longer_visible_render_2d_nodes(&mut self, visible_now: &AHashSet<NodeID>) {
        for node in self.render_2d.prev_visible.iter().copied() {
            if !visible_now.contains(&node) {
                self.render_2d.removed_nodes.push(node);
            }
        }
        while let Some(node) = self.render_2d.removed_nodes.pop() {
            self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
            self.render_2d.retained_sprite_textures.remove(&node);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_render_2d_tests.rs"]
mod tests;
