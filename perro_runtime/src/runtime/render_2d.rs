use super::Runtime;
use ahash::AHashSet;
use perro_core::SceneNodeData;
use perro_ids::{NodeID, TextureID};
use perro_render_bridge::{
    Camera2DState, Command2D, RenderCommand, RenderRequestID, ResourceCommand, Sprite2DCommand,
};

impl Runtime {
    fn sprite_texture_request_id(node: NodeID) -> RenderRequestID {
        RenderRequestID::new((node.as_u64() << 8) | 0x2D)
    }

    pub fn extract_render_2d_commands(&mut self) {
        let mut traversal_ids = std::mem::take(&mut self.render_2d.traversal_ids);
        traversal_ids.clear();
        traversal_ids.extend(self.nodes.iter().map(|(id, _)| id));

        let mut visible_now = std::mem::take(&mut self.render_2d.visible_now);
        visible_now.clear();
        self.render_2d.removed_nodes.clear();

        for node_id in traversal_ids.iter().copied() {
            let sprite_data = self.nodes.get(node_id).and_then(|node| match &node.data {
                SceneNodeData::Sprite2D(sprite) => Some((
                    sprite.visible,
                    sprite.texture_id,
                    sprite.base.transform.to_mat3().to_cols_array_2d(),
                    sprite.base.z_index,
                )),
                _ => None,
            });
            if let Some((visible, texture_id, model, z_index)) = sprite_data {
                self.emit_sprite_2d(
                    node_id,
                    visible,
                    texture_id,
                    model,
                    z_index,
                    &mut visible_now,
                );
            }

            let camera_data = self.nodes.get(node_id).and_then(|node| match &node.data {
                SceneNodeData::Camera2D(camera) if camera.active => Some(Camera2DState {
                    position: [
                        camera.base.transform.position.x,
                        camera.base.transform.position.y,
                    ],
                    rotation_radians: camera.base.transform.rotation,
                    zoom: camera.zoom,
                }),
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
        node_id: NodeID,
        visible: bool,
        texture_id: TextureID,
        model: [[f32; 3]; 3],
        z_index: i32,
        visible_now: &mut AHashSet<NodeID>,
    ) {
        if !visible {
            return;
        }

        let Some(resolved_texture) = self.resolve_sprite_texture(node_id, texture_id) else {
            return;
        };

        let needs_upsert = self
            .render_2d
            .retained_sprite_textures
            .get(&node_id)
            .is_none_or(|cached| *cached != resolved_texture);
        if needs_upsert {
            self.queue_render_command(RenderCommand::TwoD(Command2D::UpsertSprite {
                node: node_id,
                sprite: Sprite2DCommand {
                    texture: resolved_texture,
                    model,
                    z_index,
                },
            }));
            self.render_2d
                .retained_sprite_textures
                .insert(node_id, resolved_texture);
        } else {
            self.queue_render_command(RenderCommand::TwoD(Command2D::UpsertSprite {
                node: node_id,
                sprite: Sprite2DCommand {
                    texture: resolved_texture,
                    model,
                    z_index,
                },
            }));
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
                    crate::RuntimeRenderResult::Mesh(_)
                    | crate::RuntimeRenderResult::Material(_) => {}
                }
            }
        }

        if texture_id.is_nil() {
            let request = Self::sprite_texture_request_id(node_id);
            if !self.render.is_inflight(request) {
                self.render.mark_inflight(request);
                self.queue_render_command(RenderCommand::Resource(
                    ResourceCommand::CreateTexture {
                        request,
                        owner: node_id,
                    },
                ));
            }
            return None;
        }

        Some(texture_id)
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
mod tests {
    use super::Runtime;
    use perro_core::{SceneNode, SceneNodeData, camera_2d::Camera2D, sprite_2d::Sprite2D};
    use perro_ids::TextureID;
    use perro_render_bridge::{Command2D, RenderCommand, RenderEvent, ResourceCommand};

    fn collect_commands(runtime: &mut Runtime) -> Vec<RenderCommand> {
        let mut out = Vec::new();
        runtime.drain_render_commands(&mut out);
        out
    }

    #[test]
    fn sprite_requests_texture_once_until_created() {
        let mut runtime = Runtime::new();
        let node_id = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Sprite2D(Sprite2D::new())));

        runtime.extract_render_2d_commands();
        let first = collect_commands(&mut runtime);
        assert_eq!(first.len(), 1);
        let request = match first[0] {
            RenderCommand::Resource(ResourceCommand::CreateTexture { request, owner }) => {
                assert_eq!(owner, node_id);
                request
            }
            _ => panic!("expected CreateTexture"),
        };

        runtime.extract_render_2d_commands();
        assert!(collect_commands(&mut runtime).is_empty());

        let texture_id = TextureID::from_parts(3, 1);
        runtime.apply_render_event(RenderEvent::TextureCreated {
            request,
            id: texture_id,
        });
        runtime.extract_render_2d_commands();
        let third = collect_commands(&mut runtime);
        assert_eq!(third.len(), 1);
        assert!(matches!(
            third[0],
            RenderCommand::TwoD(Command2D::UpsertSprite { node, sprite })
            if node == node_id && sprite.texture == texture_id
        ));
    }

    #[test]
    fn sprite_becoming_invisible_emits_remove_node() {
        let mut runtime = Runtime::new();
        let mut sprite = Sprite2D::new();
        sprite.texture_id = TextureID::from_parts(7, 0);
        let node_id = runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Sprite2D(sprite)));

        runtime.extract_render_2d_commands();
        let first = collect_commands(&mut runtime);
        assert_eq!(first.len(), 1);
        assert!(matches!(
            first[0],
            RenderCommand::TwoD(Command2D::UpsertSprite { node, .. }) if node == node_id
        ));

        let node = runtime
            .nodes
            .get_mut(node_id)
            .expect("sprite node must exist");
        if let SceneNodeData::Sprite2D(sprite) = &mut node.data {
            sprite.visible = false;
        }

        runtime.extract_render_2d_commands();
        let second = collect_commands(&mut runtime);
        assert_eq!(second.len(), 1);
        assert!(matches!(
            second[0],
            RenderCommand::TwoD(Command2D::RemoveNode { node }) if node == node_id
        ));
    }

    #[test]
    fn active_camera_2d_emits_set_camera_command() {
        let mut runtime = Runtime::new();
        let mut camera = Camera2D::new();
        camera.active = true;
        camera.zoom = 2.0;
        camera.base.transform.position.x = 128.0;
        camera.base.transform.position.y = -32.0;
        camera.base.transform.rotation = 0.5;
        runtime
            .nodes
            .insert(SceneNode::new(SceneNodeData::Camera2D(camera)));

        runtime.extract_render_2d_commands();
        let commands = collect_commands(&mut runtime);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::TwoD(Command2D::SetCamera { camera })
            if camera.position == [128.0, -32.0]
                && camera.rotation_radians == 0.5
                && camera.zoom == 2.0
        )));
    }
}
