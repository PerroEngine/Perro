use super::*;

impl Runtime {
    pub(crate) fn resolve_sprite_texture(
        &mut self,
        node: NodeID,
        mut texture: TextureID,
    ) -> Option<TextureID> {
        if texture.is_nil() {
            let request = sprite_2d_texture_request(node);
            if let Some(result) = self.take_render_result(request) {
                match result {
                    crate::RuntimeRenderResult::Texture(id) => {
                        texture = id;
                        if let Some(node) = self.nodes.get_mut_untracked(node) {
                            match &mut node.data {
                                SceneNodeData::Sprite2D(sprite) => sprite.texture = id,
                                SceneNodeData::AnimatedSprite2D(sprite) => sprite.texture = id,
                                SceneNodeData::ImageButton2D(button) => button.texture = id,
                                SceneNodeData::NineSliceButton2D(button) => button.texture = id,
                                SceneNodeData::NineSlice2D(nine) => nine.texture = id,
                                SceneNodeData::Sprite3D(sprite) => sprite.texture = id,
                                _ => {}
                            }
                        }
                    }
                    crate::RuntimeRenderResult::Failed(_) => {}
                    crate::RuntimeRenderResult::Mesh(_)
                    | crate::RuntimeRenderResult::Material(_) => {}
                }
            }
        }

        if texture.is_nil() {
            let request = sprite_2d_texture_request(node);
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

        if self.resource_api.is_texture_id_pending(texture) {
            return None;
        }

        Some(texture)
    }

    pub(crate) fn node_2d_has_pending_visual_asset(&self, node: NodeID) -> bool {
        self.nodes
            .get(node)
            .is_some_and(|scene_node| match &scene_node.data {
                SceneNodeData::Sprite2D(sprite) => {
                    self.render_2d.retained_sprites.contains_key(&node)
                        && !sprite.texture.is_nil()
                        && self.resource_api.is_texture_id_pending(sprite.texture)
                }
                SceneNodeData::AnimatedSprite2D(sprite) => {
                    self.render_2d.retained_sprites.contains_key(&node)
                        && !sprite.texture.is_nil()
                        && self.resource_api.is_texture_id_pending(sprite.texture)
                }
                SceneNodeData::ImageButton2D(button) => {
                    self.render_2d.retained_sprites.contains_key(&node)
                        && !button.texture.is_nil()
                        && self.resource_api.is_texture_id_pending(button.texture)
                }
                SceneNodeData::NineSliceButton2D(button) => {
                    !button.texture.is_nil()
                        && self.resource_api.is_texture_id_pending(button.texture)
                }
                SceneNodeData::TileMap2D(_) => {
                    self.render.is_inflight(tilemap_2d_texture_request(node))
                }
                SceneNodeData::NineSlice2D(nine) => {
                    !nine.texture.is_nil() && self.resource_api.is_texture_id_pending(nine.texture)
                }
                _ => false,
            })
    }

    pub(crate) fn resolve_tilemap_texture(
        &mut self,
        node: NodeID,
        source: &str,
    ) -> Option<TextureID> {
        let request = tilemap_2d_texture_request(node);
        if let Some(result) = self.take_render_result(request) {
            return match result {
                crate::RuntimeRenderResult::Texture(id) => Some(id),
                crate::RuntimeRenderResult::Failed(_) => None,
                crate::RuntimeRenderResult::Mesh(_) | crate::RuntimeRenderResult::Material(_) => {
                    None
                }
            };
        }
        if !self.render.is_inflight(request) {
            self.render.mark_inflight(request);
            self.queue_render_command(RenderCommand::Resource(ResourceCommand::CreateTexture {
                request,
                id: TextureID::nil(),
                source: source.to_string(),
                reserved: false,
            }));
        }
        None
    }
}
