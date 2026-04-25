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
        let bootstrap_scan = self.render_2d.prev_visible.is_empty()
            && self.render_2d.retained_sprites.is_empty()
            && self.render_2d.last_camera.is_none();
        let has_extraction_work = self.dirty.has_any_dirty()
            || self.dirty.has_pending_transform_roots()
            || !self.render_2d.removed_nodes.is_empty()
            || bootstrap_scan;
        if !has_extraction_work {
            return;
        }

        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();

        let mut traversal_ids = std::mem::take(&mut self.render_2d.traversal_ids);
        traversal_ids.clear();
        traversal_ids.extend(
            self.dirty
                .dirty_indices()
                .iter()
                .filter_map(|&raw_index| self.nodes.slot_get(raw_index as usize).map(|(id, _)| id)),
        );
        if traversal_ids.is_empty() && bootstrap_scan {
            traversal_ids.extend(self.nodes.iter().map(|(id, _)| id));
        }
        let mut traversal_seen: AHashSet<NodeID> = traversal_ids.iter().copied().collect();
        let mut traversal_cursor = 0usize;
        while traversal_cursor < traversal_ids.len() {
            let node = traversal_ids[traversal_cursor];
            traversal_cursor += 1;
            if let Some(node_ref) = self.nodes.get(node) {
                for &child in node_ref.get_children_ids() {
                    if traversal_seen.insert(child) {
                        traversal_ids.push(child);
                    }
                }
            }
        }

        let mut visible_now = std::mem::take(&mut self.render_2d.visible_now);
        visible_now.clear();
        visible_now.extend(self.render_2d.prev_visible.iter().copied());
        let mut removed_nodes = std::mem::take(&mut self.render_2d.removed_nodes);
        for node in removed_nodes.drain(..) {
            visible_now.remove(&node);
        }
        self.render_2d.removed_nodes = removed_nodes;

        for node in traversal_ids.iter().copied() {
            visible_now.remove(&node);
            let effective_visible = self.is_effectively_visible(node);
            let sprite_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::Sprite2D(sprite) => Some((
                    effective_visible && sprite.visible,
                    sprite.texture,
                    sprite.transform,
                    sprite.z_index,
                )),
                _ => None,
            });
            if let Some((visible, texture, local_transform, z_index)) = sprite_data {
                let model = self
                    .get_global_transform_2d(node)
                    .unwrap_or(local_transform)
                    .to_mat3()
                    .to_cols_array_2d();
                self.emit_sprite_2d(node, visible, texture, model, z_index, &mut visible_now);
            }

            let camera_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::Camera2D(camera) if camera.active && effective_visible => Some((
                    camera.transform,
                    camera.zoom,
                    camera.post_processing.clone(),
                )),
                _ => None,
            });
            let camera_data = camera_data.map(|(local_transform, zoom, post_processing)| {
                let global = self
                    .get_global_transform_2d(node)
                    .unwrap_or(local_transform);
                Camera2DState {
                    position: [global.position.x, global.position.y],
                    rotation_radians: global.rotation,
                    zoom,
                    post_processing: Arc::from(post_processing.as_slice()),
                }
            });
            if let Some(camera) = camera_data
                && self.render_2d.last_camera.as_ref() != Some(&camera) {
                    self.queue_render_command(RenderCommand::TwoD(Command2D::SetCamera {
                        camera: camera.clone(),
                    }));
                    self.render_2d.last_camera = Some(camera);
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

        let sprite = Sprite2DCommand {
            texture: resolved_texture,
            model,
            tint: [1.0, 1.0, 1.0, 1.0],
            z_index,
        };
        let needs_upsert = self
            .render_2d
            .retained_sprites
            .get(&node)
            .is_none_or(|cached| *cached != sprite);
        if needs_upsert {
            self.queue_render_command(RenderCommand::TwoD(Command2D::UpsertSprite {
                node,
                sprite,
            }));
            self.render_2d.retained_sprites.insert(node, sprite);
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
            self.render_2d.retained_sprites.remove(&node);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_render_2d_tests.rs"]
mod tests;
