//! Render bridge result intake and retained command output.

use super::Runtime;
use crate::render_result::RuntimeRenderResult;
use ahash::AHashMap;
use glam::Mat4;
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_nodes::{CameraProjection, CameraStream, SceneNodeData};
use perro_render_bridge::{
    AmbientLight2DState, AmbientLight3DState, Camera2DState, Camera3DState, CameraProjectionState,
    CameraStreamCommand, CameraStreamDraw3DState, CameraStreamLighting3DState,
    CameraStreamSourceState, CameraStreamState, Command2D, Command3D, DenseInstancePose3D,
    LODOptions3D, Light2DState, MeshBlendOptions3D, PointLight2DState, PointLight3DState,
    PointParticles2DState, PointParticles3DState, RayLight2DState, RayLight3DState, RenderCommand,
    RenderEvent, RenderRequestID, ResourceCommand, Sky3DState, SkyShaderPass3DState,
    SkyTime3DState, SpotLight2DState, SpotLight3DState, Sprite2DCommand, Water2DState,
    Water3DState,
};
use perro_runtime_render::{decode_3d_mesh_request_node, decode_render_request_node_from_event};
use perro_structs::BitMask;
use std::sync::Arc;

use crate::runtime::render_2d::{
    TilemapSpriteBuild, build_tilemap_sprites, derived_particle_budget, direction_from_rotation_2d,
    resolve_particle_profile_2d, resolve_particle_sim_mode_2d, resolve_tileset_2d,
    water_idle_mode_state as water_idle_mode_state_2d, water_render_size as water_render_size_2d,
    water_shape_state as water_shape_state_2d,
};
use crate::runtime::render_3d::{
    derived_particle_budget_3d, resolve_particle_profile as resolve_particle_profile_3d,
    resolve_particle_render_mode as resolve_particle_render_mode_3d,
    resolve_particle_sim_mode as resolve_particle_sim_mode_3d,
    water_idle_mode_state as water_idle_mode_state_3d, water_render_size as water_render_size_3d,
    water_shape_state as water_shape_state_3d,
};

fn is_ui_node_data(data: &SceneNodeData) -> bool {
    matches!(
        data,
        SceneNodeData::UiNode(_)
            | SceneNodeData::UiCameraStream(_)
            | SceneNodeData::UiPanel(_)
            | SceneNodeData::UiButton(_)
            | SceneNodeData::UiCheckbox(_)
            | SceneNodeData::UiColorPicker(_)
            | SceneNodeData::UiImage(_)
            | SceneNodeData::UiImageButton(_)
            | SceneNodeData::UiNineSlice(_)
            | SceneNodeData::UiAnimatedImage(_)
            | SceneNodeData::UiLabel(_)
            | SceneNodeData::UiTextBox(_)
            | SceneNodeData::UiTextBlock(_)
            | SceneNodeData::UiScrollContainer(_)
            | SceneNodeData::UiLayout(_)
            | SceneNodeData::UiHLayout(_)
            | SceneNodeData::UiVLayout(_)
            | SceneNodeData::UiGrid(_)
            | SceneNodeData::UiTreeList(_)
    )
}

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

    fn count_scene_resource_refs_into(
        &self,
        textures: &mut AHashMap<TextureID, Vec<NodeID>>,
        meshes: &mut AHashMap<MeshID, Vec<NodeID>>,
        materials: &mut AHashMap<MaterialID, Vec<NodeID>>,
    ) {
        // reuse scratch: drop stale entries but keep vec capacity 4 refill.
        for nodes in textures.values_mut() {
            nodes.clear();
        }
        for nodes in meshes.values_mut() {
            nodes.clear();
        }
        for nodes in materials.values_mut() {
            nodes.clear();
        }
        // reborrow so match arms below can pass `&mut textures` unchanged.
        let textures = &mut *textures;
        let meshes = &mut *meshes;
        let materials = &mut *materials;

        // dedup only guards same node adding same id twice (material surfaces).
        // arena iter visits each node once, so any dup is adjacent -> check last.
        fn add_ref<T: Eq + std::hash::Hash + Copy>(
            refs: &mut AHashMap<T, Vec<NodeID>>,
            id: T,
            node: NodeID,
        ) {
            let nodes = refs.entry(id).or_default();
            if nodes.last() != Some(&node) {
                nodes.push(node);
            }
        }

        for (node_id, node) in self.nodes.iter() {
            match &node.data {
                SceneNodeData::Sprite2D(sprite)
                    if !self.render_2d.retained_sprites.contains_key(&node_id) =>
                {
                    if !sprite.texture.is_nil()
                        && !self.resource_api.is_texture_id_pending(sprite.texture)
                    {
                        add_ref(textures, sprite.texture, node_id);
                    }
                }
                SceneNodeData::AnimatedSprite2D(sprite)
                    if !self.render_2d.retained_sprites.contains_key(&node_id) =>
                {
                    if !sprite.texture.is_nil()
                        && !self.resource_api.is_texture_id_pending(sprite.texture)
                    {
                        add_ref(textures, sprite.texture, node_id);
                    }
                }
                SceneNodeData::ImageButton2D(button)
                    if !self.render_2d.retained_sprites.contains_key(&node_id) =>
                {
                    if !button.texture.is_nil()
                        && !self.resource_api.is_texture_id_pending(button.texture)
                    {
                        add_ref(textures, button.texture, node_id);
                    }
                }
                SceneNodeData::NineSlice2D(nine)
                    if !self.render_2d.retained_sprites.contains_key(&node_id) =>
                {
                    if !nine.texture.is_nil()
                        && !self.resource_api.is_texture_id_pending(nine.texture)
                    {
                        add_ref(textures, nine.texture, node_id);
                    }
                }
                SceneNodeData::UiImage(image)
                    if !self.render_ui.retained_commands.contains_key(&node_id) =>
                {
                    if !image.texture.is_nil()
                        && !self.resource_api.is_texture_id_pending(image.texture)
                    {
                        add_ref(textures, image.texture, node_id);
                    }
                }
                SceneNodeData::UiAnimatedImage(image)
                    if !self.render_ui.retained_commands.contains_key(&node_id) =>
                {
                    if !image.texture.is_nil()
                        && !self.resource_api.is_texture_id_pending(image.texture)
                    {
                        add_ref(textures, image.texture, node_id);
                    }
                }
                SceneNodeData::UiImageButton(button)
                    if !self.render_ui.retained_commands.contains_key(&node_id) =>
                {
                    if !button.texture.is_nil()
                        && !self.resource_api.is_texture_id_pending(button.texture)
                    {
                        add_ref(textures, button.texture, node_id);
                    }
                }
                SceneNodeData::UiNineSlice(nine)
                    if !self.render_ui.retained_commands.contains_key(&node_id) =>
                {
                    if !nine.texture.is_nil()
                        && !self.resource_api.is_texture_id_pending(nine.texture)
                    {
                        add_ref(textures, nine.texture, node_id);
                    }
                }
                SceneNodeData::MeshInstance3D(mesh) => {
                    if !self.render_3d.retained_mesh_draws.contains_key(&node_id) {
                        if !mesh.mesh.is_nil() && !self.resource_api.is_mesh_id_pending(mesh.mesh) {
                            add_ref(meshes, mesh.mesh, node_id);
                        }
                        for material in mesh.surfaces.iter().filter_map(|surface| surface.material)
                        {
                            if !material.is_nil()
                                && !self.resource_api.is_material_id_pending(material)
                            {
                                add_ref(materials, material, node_id);
                            }
                        }
                    }
                }
                SceneNodeData::MultiMeshInstance3D(mesh)
                    if !self.render_3d.retained_mesh_draws.contains_key(&node_id) =>
                {
                    if !mesh.mesh.is_nil() && !self.resource_api.is_mesh_id_pending(mesh.mesh) {
                        add_ref(meshes, mesh.mesh, node_id);
                    }
                    for material in mesh.surfaces.iter().filter_map(|surface| surface.material) {
                        if !material.is_nil() && !self.resource_api.is_material_id_pending(material)
                        {
                            add_ref(materials, material, node_id);
                        }
                    }
                }
                _ => {}
            }
        }

        // drop keys that ended empty so cache-eq matches old fresh-map behavior.
        textures.retain(|_, nodes| !nodes.is_empty());
        meshes.retain(|_, nodes| !nodes.is_empty());
        materials.retain(|_, nodes| !nodes.is_empty());
    }

    pub fn drain_render_commands(&mut self, out: &mut Vec<RenderCommand>) {
        let mut queued_resource_commands = self.render.take_resource_queue_scratch();
        self.resource_api
            .drain_commands(&mut queued_resource_commands);

        // gate scan: node data / structure changes bump arena mutation_revision;
        // resource events (pending resolve / retained invalidation) set dirty.
        let arena_version = self.nodes.mutation_revision();
        if self.scene_resource_refs_dirty
            || arena_version != self.scene_resource_refs_scanned_version
        {
            let mut scratch = std::mem::take(&mut self.scene_resource_refs_scratch);
            self.count_scene_resource_refs_into(
                &mut scratch.textures,
                &mut scratch.meshes,
                &mut scratch.materials,
            );
            self.scene_resource_refs_scanned_version = arena_version;
            self.scene_resource_refs_dirty = false;
            if scratch.textures != self.scene_texture_refs_cache
                || scratch.meshes != self.scene_mesh_refs_cache
                || scratch.materials != self.scene_material_refs_cache
            {
                // swap scratch <-> cache: new refs become cache, old refs recycle
                // into scratch. no deep clone of the three maps.
                std::mem::swap(&mut scratch.textures, &mut self.scene_texture_refs_cache);
                std::mem::swap(&mut scratch.meshes, &mut self.scene_mesh_refs_cache);
                std::mem::swap(&mut scratch.materials, &mut self.scene_material_refs_cache);
                queued_resource_commands.push(RenderCommand::Resource(
                    ResourceCommand::SetSceneResourceRefs {
                        textures: self.scene_texture_refs_cache.clone().into_iter().collect(),
                        meshes: self.scene_mesh_refs_cache.clone().into_iter().collect(),
                        materials: self.scene_material_refs_cache.clone().into_iter().collect(),
                    },
                ));
            }
            self.scene_resource_refs_scratch = scratch;
        }
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
        if let RenderEvent::MeshCreated { request, id, .. } = &event {
            if let Some(node) = decode_3d_mesh_request_node(*request)
                && let Some(source) = self.render_3d.mesh_sources.get(&node).cloned()
            {
                self.resource_api.register_loaded_mesh_source(&source, *id);
            }
            if let Some(source) = self.resource_api.mesh_source(*id) {
                let dirty_nodes = self
                    .render_3d
                    .mesh_sources
                    .iter()
                    .filter_map(|(node, node_source)| (node_source == &source).then_some(*node))
                    .collect::<Vec<_>>();
                for node in dirty_nodes {
                    self.mark_needs_rerender(node);
                }
            }
        }
        if let Some(node) = decode_render_request_node_from_event(&event) {
            self.mark_needs_rerender(node);
        }
        if let RenderEvent::MaterialLoaded { id } = &event {
            self.invalidate_3d_mesh_draws_using_material(*id);
        }
        if matches!(
            event,
            RenderEvent::MeshCreated { .. }
                | RenderEvent::MaterialCreated { .. }
                | RenderEvent::MaterialLoaded { .. }
        ) {
            self.request_full_3d_scan_once();
        }
        // render events resolve pending resources / invalidate retained draws,
        // which arena mutation_revision can't see. force resource-ref re-scan.
        self.scene_resource_refs_dirty = true;
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

    pub(crate) fn request_full_2d_scan_once(&mut self) {
        self.render_2d.request_full_scan_once();
    }

    pub fn force_rerender(&mut self, root_id: NodeID) {
        if root_id.is_nil() || self.nodes.get(root_id).is_none() {
            return;
        }

        let mut stack = std::mem::take(&mut self.force_rerender_stack_scratch);
        stack.clear();
        stack.push(root_id);
        while let Some(id) = stack.pop() {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            let ui_dirty = is_ui_node_data(&node.data);
            // direct field-path split: &self.nodes borrow (via `node`) stays
            // live across the &mut self.dirty calls below (disjoint fields).
            stack.extend_from_slice(node.children_slice());
            self.dirty.mark_rerender(id);
            if ui_dirty {
                self.dirty.mark_ui(
                    id,
                    Self::UI_DIRTY_LAYOUT_SELF
                        | Self::UI_DIRTY_LAYOUT_PARENT
                        | Self::UI_DIRTY_TRANSFORM
                        | Self::UI_DIRTY_COMMANDS,
                );
            }
        }
        stack.clear();
        self.force_rerender_stack_scratch = stack;
    }

    pub(crate) fn mark_ui_dirty(&mut self, id: NodeID, flags: u16) {
        self.dirty.mark_ui(id, flags);
        if self.render_ui.defer_dirty_marks {
            self.render_ui.deferred_dirty.push((id, flags));
        }
    }

    pub fn mark_transform_dirty_recursive(&mut self, root: NodeID) {
        let Some(node) = self.nodes.get(root) else {
            return;
        };
        if node.children_slice().is_empty() {
            // leaf: type known now -> scoped physics gate. w/ children,
            // defer to root walk (propagate) where each descendant typed.
            let physics = node.node_type().is_physics();
            self.dirty.mark_transform(root, node.spatial(), physics);
        } else {
            self.dirty.mark_transform_root(root);
        }
    }

    pub fn clear_dirty_flags(&mut self) {
        self.dirty.clear();
        // Marks made during the post-layout input phase of UI extraction
        // (dropdown open, tree toggle) must survive into the next frame's
        // layout pass; re-apply them after the wholesale clear.
        if !self.render_ui.deferred_dirty.is_empty() {
            let deferred = std::mem::take(&mut self.render_ui.deferred_dirty);
            for (id, flags) in deferred {
                self.dirty.mark_ui(id, flags);
            }
        }
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
        self.queue_render_command(RenderCommand::CameraStream(
            CameraStreamCommand::RemoveNode { node },
        ));
        self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
        self.queue_render_command(RenderCommand::ThreeD(Box::new(Command3D::RemoveNode {
            node,
        })));
        self.queue_render_command(RenderCommand::Ui(
            perro_render_bridge::UiCommand::RemoveNode { node },
        ));
    }

    pub(crate) fn camera_stream_texture_id(node: NodeID) -> TextureID {
        TextureID::from_parts(node.index(), node.generation())
    }

    pub(crate) fn camera_stream_state(
        &mut self,
        stream_node: NodeID,
        stream: &CameraStream,
    ) -> Option<CameraStreamState> {
        if !stream.enabled || stream.camera.is_nil() || stream.camera == stream_node {
            return None;
        }
        let source = self.camera_stream_source_state(stream.camera)?;
        // build node-id list once; collectors below share it via index access
        // instead of each re-collecting the whole arena.
        self.camera_stream_node_scratch.clear();
        self.camera_stream_node_scratch
            .extend(self.nodes.iter().map(|(id, _)| id));
        let mut post_processing = match &source {
            CameraStreamSourceState::TwoD(camera) => camera.post_processing.to_vec(),
            CameraStreamSourceState::ThreeD(camera) => camera.post_processing.to_vec(),
        };
        post_processing.extend(stream.post_processing.to_effects_vec());
        let (
            sprites_2d,
            lights_2d,
            point_particles_2d,
            waters_2d,
            draws_3d,
            lighting_3d,
            point_particles_3d,
            waters_3d,
        ) = match &source {
            CameraStreamSourceState::TwoD(camera) => (
                self.collect_camera_stream_sprites_2d(camera.render_mask, stream_node),
                self.collect_camera_stream_lights_2d(camera.render_mask, stream_node),
                self.collect_camera_stream_point_particles_2d(camera.render_mask, stream_node),
                self.collect_camera_stream_waters_2d(camera.render_mask, stream_node),
                Arc::from([]),
                CameraStreamLighting3DState::default(),
                Arc::from([]),
                Arc::from([]),
            ),
            CameraStreamSourceState::ThreeD(camera) => (
                Arc::from([]),
                Arc::from([]),
                Arc::from([]),
                Arc::from([]),
                self.collect_camera_stream_draws_3d(camera.render_mask, stream_node),
                self.collect_camera_stream_lighting_3d(camera.render_mask, stream_node),
                self.collect_camera_stream_point_particles_3d(camera.render_mask, stream_node),
                self.collect_camera_stream_waters_3d(camera.render_mask, stream_node),
            ),
        };
        Some(CameraStreamState {
            source,
            resolution: [
                stream.resolution.x.clamp(1, 8192),
                stream.resolution.y.clamp(1, 8192),
            ],
            aspect_ratio: stream.aspect_ratio.max(0.0),
            post_processing: Arc::from(post_processing),
            output_texture: Self::camera_stream_texture_id(stream_node),
            sprites_2d,
            lights_2d,
            point_particles_2d,
            waters_2d,
            draws_3d,
            lighting_3d,
            point_particles_3d,
            waters_3d,
        })
    }

    fn collect_camera_stream_sprites_2d(
        &mut self,
        camera_mask: BitMask,
        stream_node: NodeID,
    ) -> Arc<[Sprite2DCommand]> {
        let mut out = Vec::new();
        for idx in 0..self.camera_stream_node_scratch.len() {
            let node = self.camera_stream_node_scratch[idx];
            if node == stream_node || !self.is_effectively_visible(node) {
                continue;
            }
            let Some((texture, region, transform, z_index)) =
                self.nodes
                    .get(node)
                    .and_then(|node_ref| match &node_ref.data {
                        SceneNodeData::Sprite2D(sprite)
                            if sprite.visible
                                && stream_render_mask_matches(
                                    camera_mask,
                                    sprite.render_layers,
                                ) =>
                        {
                            Some((
                                sprite.texture,
                                sprite.texture_region,
                                sprite.transform,
                                sprite.z_index,
                            ))
                        }
                        SceneNodeData::AnimatedSprite2D(sprite)
                            if sprite.visible
                                && stream_render_mask_matches(
                                    camera_mask,
                                    sprite.render_layers,
                                ) =>
                        {
                            Some((
                                sprite.texture,
                                sprite.current_texture_region(),
                                sprite.transform,
                                sprite.z_index,
                            ))
                        }
                        _ => None,
                    })
            else {
                let tilemap_data = self
                    .nodes
                    .get(node)
                    .and_then(|node_ref| match &node_ref.data {
                        SceneNodeData::TileMap2D(tilemap)
                            if tilemap.visible
                                && stream_render_mask_matches(
                                    camera_mask,
                                    tilemap.render_layers,
                                ) =>
                        {
                            Some((
                                tilemap.tileset.clone(),
                                tilemap.width,
                                tilemap.height,
                                tilemap.empty_tile,
                                tilemap.tiles.clone(),
                                tilemap.transform,
                                tilemap.z_index,
                            ))
                        }
                        _ => None,
                    });
                if let Some((
                    tileset_source,
                    width,
                    height,
                    empty_tile,
                    tiles,
                    local_transform,
                    z_index,
                )) = tilemap_data
                    && let Some(tileset) = resolve_tileset_2d(self, &tileset_source)
                    && let Some(texture) =
                        self.resolve_tilemap_texture(node, tileset.texture.as_ref())
                {
                    let base_model = self
                        .get_render_global_transform_2d(node)
                        .unwrap_or(local_transform)
                        .to_mat3()
                        .to_cols_array_2d();
                    out.extend(build_tilemap_sprites(TilemapSpriteBuild {
                        texture,
                        width,
                        height,
                        z_index,
                        empty_tile,
                        tint: self.effective_self_modulate(node),
                        base_model,
                        tiles: &tiles,
                        tileset: &tileset,
                    }));
                }
                continue;
            };
            let Some(texture) = self.resolve_sprite_texture(node, texture) else {
                continue;
            };
            let (uv_min, uv_max, size) = stream_sprite_region_uv(region);
            let model = self
                .get_render_global_transform_2d(node)
                .unwrap_or(transform)
                .to_mat3()
                .to_cols_array_2d();
            out.push(Sprite2DCommand {
                texture,
                model,
                tint: self.effective_self_modulate(node),
                uv_min,
                uv_max,
                size,
                z_index,
            });
        }
        Arc::from(out)
    }

    fn collect_camera_stream_lights_2d(
        &mut self,
        camera_mask: BitMask,
        stream_node: NodeID,
    ) -> Arc<[Light2DState]> {
        enum StreamLight2DData {
            Ambient {
                color: [f32; 3],
                intensity: f32,
            },
            Ray {
                transform: perro_structs::Transform2D,
                color: [f32; 3],
                intensity: f32,
                z_index: i32,
            },
            Point {
                transform: perro_structs::Transform2D,
                color: [f32; 3],
                intensity: f32,
                range: f32,
                z_index: i32,
            },
            Spot {
                transform: perro_structs::Transform2D,
                color: [f32; 3],
                intensity: f32,
                range: f32,
                inner_angle_radians: f32,
                outer_angle_radians: f32,
                z_index: i32,
            },
        }
        let mut out = Vec::new();
        for idx in 0..self.camera_stream_node_scratch.len() {
            let node = self.camera_stream_node_scratch[idx];
            if node == stream_node || !self.is_effectively_visible(node) {
                continue;
            }
            let data = self
                .nodes
                .get(node)
                .and_then(|node_ref| match &node_ref.data {
                    SceneNodeData::AmbientLight2D(light)
                        if light.visible
                            && light.active
                            && light.intensity > 0.0
                            && stream_render_mask_matches(camera_mask, light.render_layers) =>
                    {
                        Some(StreamLight2DData::Ambient {
                            color: light.color,
                            intensity: light.intensity,
                        })
                    }
                    SceneNodeData::RayLight2D(light)
                        if light.visible
                            && light.active
                            && light.intensity > 0.0
                            && stream_render_mask_matches(camera_mask, light.render_layers) =>
                    {
                        Some(StreamLight2DData::Ray {
                            transform: light.transform,
                            color: light.color,
                            intensity: light.intensity,
                            z_index: light.z_index,
                        })
                    }
                    SceneNodeData::PointLight2D(light)
                        if light.visible
                            && light.active
                            && light.intensity > 0.0
                            && light.range > 0.0
                            && stream_render_mask_matches(camera_mask, light.render_layers) =>
                    {
                        Some(StreamLight2DData::Point {
                            transform: light.transform,
                            color: light.color,
                            intensity: light.intensity,
                            range: light.range,
                            z_index: light.z_index,
                        })
                    }
                    SceneNodeData::SpotLight2D(light)
                        if light.visible
                            && light.active
                            && light.intensity > 0.0
                            && light.range > 0.0
                            && stream_render_mask_matches(camera_mask, light.render_layers) =>
                    {
                        Some(StreamLight2DData::Spot {
                            transform: light.transform,
                            color: light.color,
                            intensity: light.intensity,
                            range: light.range,
                            inner_angle_radians: light.inner_angle_radians,
                            outer_angle_radians: light.outer_angle_radians,
                            z_index: light.z_index,
                        })
                    }
                    _ => None,
                });
            match data {
                Some(StreamLight2DData::Ambient { color, intensity }) => {
                    out.push(Light2DState::Ambient(AmbientLight2DState {
                        color,
                        intensity: intensity.max(0.0),
                    }));
                }
                Some(StreamLight2DData::Ray {
                    transform,
                    color,
                    intensity,
                    z_index,
                }) => {
                    let global = self
                        .get_render_global_transform_2d(node)
                        .unwrap_or(transform);
                    out.push(Light2DState::Ray(RayLight2DState {
                        direction: direction_from_rotation_2d(global.rotation),
                        color,
                        intensity: intensity.max(0.0),
                        z_index,
                    }));
                }
                Some(StreamLight2DData::Point {
                    transform,
                    color,
                    intensity,
                    range,
                    z_index,
                }) => {
                    let global = self
                        .get_render_global_transform_2d(node)
                        .unwrap_or(transform);
                    out.push(Light2DState::Point(PointLight2DState {
                        position: [global.position.x, global.position.y],
                        color,
                        intensity: intensity.max(0.0),
                        range: range.max(0.001),
                        z_index,
                    }));
                }
                Some(StreamLight2DData::Spot {
                    transform,
                    color,
                    intensity,
                    range,
                    inner_angle_radians,
                    outer_angle_radians,
                    z_index,
                }) => {
                    let global = self
                        .get_render_global_transform_2d(node)
                        .unwrap_or(transform);
                    out.push(Light2DState::Spot(SpotLight2DState {
                        position: [global.position.x, global.position.y],
                        direction: direction_from_rotation_2d(global.rotation),
                        color,
                        intensity: intensity.max(0.0),
                        range: range.max(0.001),
                        inner_angle_radians: inner_angle_radians.max(0.0),
                        outer_angle_radians: outer_angle_radians.max(inner_angle_radians),
                        z_index,
                    }));
                }
                None => {}
            }
        }
        Arc::from(out)
    }

    fn collect_camera_stream_point_particles_2d(
        &mut self,
        camera_mask: BitMask,
        stream_node: NodeID,
    ) -> Arc<[(NodeID, PointParticles2DState)]> {
        let mut out = Vec::new();
        for idx in 0..self.camera_stream_node_scratch.len() {
            let node = self.camera_stream_node_scratch[idx];
            if node == stream_node || !self.is_effectively_visible(node) {
                continue;
            }
            let data = self
                .nodes
                .get(node)
                .and_then(|node_ref| match &node_ref.data {
                    SceneNodeData::ParticleEmitter2D(emitter)
                        if emitter.visible
                            && stream_render_mask_matches(camera_mask, emitter.render_layers) =>
                    {
                        Some((
                            emitter.profile.clone(),
                            emitter.sim_mode,
                            emitter.transform,
                            emitter.z_index,
                            emitter.active,
                            emitter.looping,
                            emitter.prewarm,
                            emitter.spawn_rate,
                            emitter.seed,
                            emitter.params.clone(),
                            emitter.internal_simulation_time,
                        ))
                    }
                    _ => None,
                });
            let Some((
                profile_source,
                sim_mode,
                transform,
                z_index,
                active,
                looping,
                prewarm,
                spawn_rate,
                seed,
                params,
                simulation_time,
            )) = data
            else {
                continue;
            };
            let profile = resolve_particle_profile_2d(self, &profile_source).unwrap_or_default();
            let lifetime_min = profile.lifetime_min.max(0.001);
            let lifetime_max = profile.lifetime_max.max(lifetime_min);
            let model = self
                .get_render_global_transform_2d(node)
                .unwrap_or(transform)
                .to_mat3()
                .to_cols_array_2d();
            out.push((
                node,
                PointParticles2DState {
                    model,
                    z_index,
                    active,
                    looping,
                    prewarm,
                    alive_budget: derived_particle_budget(spawn_rate.max(0.0), lifetime_max),
                    emission_rate: spawn_rate.max(0.0),
                    lifetime_min,
                    lifetime_max,
                    speed_min: profile.speed_min.max(0.0),
                    speed_max: profile.speed_max.max(profile.speed_min.max(0.0)),
                    spread_radians: profile.spread_radians.clamp(0.0, std::f32::consts::TAU),
                    size: profile.size.max(1.0),
                    size_min: profile.size_min.max(0.01),
                    size_max: profile.size_max.max(profile.size_min.max(0.01)),
                    force: profile.force,
                    color_start: profile.color_start,
                    color_end: profile.color_end,
                    seed,
                    params,
                    simulation_time,
                    simulation_delta: 0.0,
                    profile,
                    sim_mode: resolve_particle_sim_mode_2d(sim_mode),
                },
            ));
        }
        Arc::from(out)
    }

    fn collect_camera_stream_waters_2d(
        &mut self,
        camera_mask: BitMask,
        stream_node: NodeID,
    ) -> Arc<[(NodeID, Water2DState)]> {
        let mut out = Vec::new();
        for idx in 0..self.camera_stream_node_scratch.len() {
            let node = self.camera_stream_node_scratch[idx];
            if node == stream_node || !self.is_effectively_visible(node) {
                continue;
            }
            let data = self
                .nodes
                .get(node)
                .and_then(|node_ref| match &node_ref.data {
                    SceneNodeData::WaterBody2D(water)
                        if water.visible
                            && stream_render_mask_matches(camera_mask, water.render_layers) =>
                    {
                        Some((water.transform, water.z_index, water.water))
                    }
                    _ => None,
                });
            let Some((local_transform, z_index, water)) = data else {
                continue;
            };
            let model = self
                .get_render_global_transform_2d(node)
                .unwrap_or(local_transform)
                .to_mat3()
                .to_cols_array_2d();
            let coastline_shapes = self.collect_water_coastline_shapes_2d(node, &water);
            let queries = self.collect_water_queries_2d(node);
            let impacts = self.collect_water_impacts_2d(node, &water);
            let links = self.collect_water_links_2d(node, &water);
            out.push((
                node,
                Water2DState {
                    model,
                    z_index,
                    paused: false,
                    simulation_time: self.time.elapsed,
                    simulation_delta: self.time.delta.max(0.0),
                    size: water_render_size_2d(water),
                    shape: water_shape_state_2d(water.shape),
                    resolution: water.resolution,
                    render_resolution: water.render_resolution,
                    depth: water.shape.depth(water.depth),
                    flow: [water.flow.x, water.flow.y],
                    wind: [water.wind.x, water.wind.y],
                    idle_mode: water_idle_mode_state_2d(water.idle_mode),
                    wave_speed: water.wave.speed,
                    wave_scale: water.wave.scale,
                    wave_length: water.wave.length,
                    damping: water.wave.damping,
                    wake_strength: water.physics.wake_strength,
                    foam_strength: water.physics.foam_strength,
                    sample_readback_rate: water.physics.sample_readback_rate,
                    lod_near_distance: water.lod.near_distance,
                    lod_mid_distance: water.lod.mid_distance,
                    lod_far_distance: water.lod.far_distance,
                    lod_min_resolution: water.lod.min_resolution,
                    collision_layers: water.collision_layers,
                    collision_mask: water.collision_mask,
                    deep_color: water.optics.deep_color,
                    shallow_color: water.optics.shallow_color,
                    shallow_depth: water.optics.shallow_depth,
                    sky_bias_ratio: water.optics.sky_bias.ratio(),
                    transparency: water.visual.transparency,
                    reflectivity: water.visual.reflectivity,
                    roughness: water.visual.roughness,
                    fresnel_power: water.visual.fresnel_power,
                    normal_strength: water.visual.normal_strength,
                    ripple_scale: water.visual.ripple_scale,
                    foam_color: water.visual.foam_color,
                    foam_amount: water.visual.foam_amount,
                    crest_foam_threshold: water.visual.crest_foam_threshold,
                    caustic_strength: water.visual.caustic_strength,
                    refraction_strength: water.visual.refraction_strength,
                    scattering_strength: water.visual.scattering_strength,
                    distance_fog_strength: water.visual.distance_fog_strength,
                    coastline_foam_color: water.coastline.foam_color,
                    coastline_foam_strength: water.coastline.foam_strength,
                    coastline_foam_width: water.coastline.foam_width,
                    coastline_cutoff_softness: water.coastline.cutoff_softness,
                    coastline_wave_reflection: water.coastline.wave_reflection,
                    coastline_wave_damping: water.coastline.wave_damping,
                    coastline_edge_noise: water.coastline.edge_noise,
                    debug: water.debug,
                    links,
                    queries,
                    impacts,
                    coastline_shapes,
                },
            ));
        }
        Arc::from(out)
    }

    fn collect_camera_stream_draws_3d(
        &mut self,
        camera_mask: BitMask,
        stream_node: NodeID,
    ) -> Arc<[CameraStreamDraw3DState]> {
        let mut out = Vec::new();
        // Reuse the render-state scratch for skeleton palettes (see
        // `stream_skeleton_palette`) instead of allocating per skinned draw.
        let mut skeleton_global_scratch =
            std::mem::take(&mut self.render_3d.skeleton_global_scratch);
        let mut skeleton_palette_scratch =
            std::mem::take(&mut self.render_3d.skeleton_palette_scratch);
        for idx in 0..self.camera_stream_node_scratch.len() {
            let node = self.camera_stream_node_scratch[idx];
            if node == stream_node || !self.is_effectively_visible(node) {
                continue;
            }
            let Some((mesh, surfaces, _skeleton, meshlet_override, lod, blend, instance_kind)) =
                self.nodes
                    .get(node)
                    .and_then(|node_ref| match &node_ref.data {
                        SceneNodeData::MeshInstance3D(mesh)
                            if mesh.visible
                                && stream_render_mask_matches(camera_mask, mesh.render_layers) =>
                        {
                            Some((
                                mesh.mesh,
                                mesh.surfaces.clone(),
                                Some(mesh.skeleton),
                                mesh.meshlet_override,
                                LODOptions3D {
                                    min_lod: mesh.lod.min_lod,
                                    max_lod: mesh.lod.max_lod,
                                },
                                MeshBlendOptions3D {
                                    enabled: mesh.blend.enabled,
                                    screen_blending: mesh.blend.screen_blending,
                                    normal_blending: mesh.blend.normal_blending,
                                    blend_layers: mesh.blend.blend_layers,
                                    blend_mask: mesh.blend.blend_mask,
                                    distance: mesh.blend.distance,
                                    min_distance: mesh.blend.min_distance,
                                    noise_factor: mesh.blend.noise_factor,
                                    noise_scale: mesh.blend.noise_scale,
                                },
                                StreamMeshInstanceKind::Single,
                            ))
                        }
                        SceneNodeData::MultiMeshInstance3D(mesh)
                            if mesh.visible
                                && stream_render_mask_matches(camera_mask, mesh.render_layers) =>
                        {
                            Some((
                                mesh.mesh,
                                mesh.surfaces.clone(),
                                None,
                                mesh.meshlet_override,
                                LODOptions3D {
                                    min_lod: mesh.lod.min_lod,
                                    max_lod: mesh.lod.max_lod,
                                },
                                MeshBlendOptions3D {
                                    enabled: mesh.blend.enabled,
                                    screen_blending: mesh.blend.screen_blending,
                                    normal_blending: mesh.blend.normal_blending,
                                    blend_layers: mesh.blend.blend_layers,
                                    blend_mask: mesh.blend.blend_mask,
                                    distance: mesh.blend.distance,
                                    min_distance: mesh.blend.min_distance,
                                    noise_factor: mesh.blend.noise_factor,
                                    noise_scale: mesh.blend.noise_scale,
                                },
                                StreamMeshInstanceKind::Dense {
                                    instance_scale: mesh.instance_scale.max(0.0001),
                                    poses: Arc::from(
                                        mesh.instances
                                            .iter()
                                            .map(|instance| DenseInstancePose3D {
                                                position: [
                                                    instance.transform.position.x,
                                                    instance.transform.position.y,
                                                    instance.transform.position.z,
                                                ],
                                                scale: [
                                                    instance.transform.scale.x,
                                                    instance.transform.scale.y,
                                                    instance.transform.scale.z,
                                                ],
                                                rotation: [
                                                    instance.transform.rotation.x,
                                                    instance.transform.rotation.y,
                                                    instance.transform.rotation.z,
                                                    instance.transform.rotation.w,
                                                ],
                                                has_blend_shape_weight_override: instance
                                                    .blend_shape_weights
                                                    .is_some(),
                                                blend_shape_weights: instance
                                                    .blend_shape_weights
                                                    .clone()
                                                    .map(Arc::<[f32]>::from)
                                                    .unwrap_or_else(|| Arc::from([])),
                                            })
                                            .collect::<Vec<_>>(),
                                    ),
                                },
                            ))
                        }
                        _ => None,
                    })
            else {
                continue;
            };
            let Some((mesh, surfaces)) = self.resolve_render_mesh_assets(node, mesh, surfaces)
            else {
                continue;
            };
            let model = self
                .get_render_global_transform_3d(node)
                .unwrap_or(perro_structs::Transform3D::IDENTITY)
                .to_mat4()
                .to_cols_array_2d();
            let skeleton_palette = _skeleton.and_then(|skeleton| {
                (!skeleton.is_nil()).then(|| {
                    stream_skeleton_palette(
                        &self.nodes,
                        skeleton,
                        &mut skeleton_global_scratch,
                        &mut skeleton_palette_scratch,
                    )
                })?
            });
            match instance_kind {
                StreamMeshInstanceKind::Single => out.push(CameraStreamDraw3DState::Draw {
                    mesh,
                    surfaces,
                    node,
                    model,
                    skeleton: skeleton_palette,
                    meshlet_override,
                    lod,
                    blend,
                }),
                StreamMeshInstanceKind::Dense {
                    instance_scale,
                    poses,
                } => out.push(CameraStreamDraw3DState::DrawMultiDense {
                    mesh,
                    surfaces,
                    node,
                    node_model: model,
                    instance_scale,
                    instances: poses,
                    meshlet_override,
                    lod,
                    blend,
                }),
            }
        }
        self.render_3d.skeleton_global_scratch = skeleton_global_scratch;
        self.render_3d.skeleton_palette_scratch = skeleton_palette_scratch;
        Arc::from(out)
    }

    fn collect_camera_stream_lighting_3d(
        &mut self,
        camera_mask: BitMask,
        stream_node: NodeID,
    ) -> CameraStreamLighting3DState {
        enum StreamLight3DData {
            Ambient(AmbientLight3DState),
            Sky(Sky3DState),
            Ray {
                transform: perro_structs::Transform3D,
                color: [f32; 3],
                intensity: f32,
                cast_shadows: bool,
            },
            Point {
                transform: perro_structs::Transform3D,
                color: [f32; 3],
                intensity: f32,
                range: f32,
                cast_shadows: bool,
            },
            Spot {
                transform: perro_structs::Transform3D,
                color: [f32; 3],
                intensity: f32,
                range: f32,
                inner_angle_radians: f32,
                outer_angle_radians: f32,
                cast_shadows: bool,
            },
        }
        let mut lighting = CameraStreamLighting3DState::default();
        let mut ray_lights = Vec::new();
        let mut point_lights = Vec::new();
        let mut spot_lights = Vec::new();
        let mut ids = self.camera_stream_node_scratch.clone();
        ids.sort_unstable_by_key(|id| id.as_u64());
        for node in ids {
            if node == stream_node || !self.is_effectively_visible(node) {
                continue;
            }
            let data = self
                .nodes
                .get(node)
                .and_then(|node_ref| match &node_ref.data {
                    SceneNodeData::AmbientLight3D(light)
                        if lighting.ambient_light.is_none()
                            && light.visible
                            && light.active
                            && stream_render_mask_matches(camera_mask, light.render_layers) =>
                    {
                        Some(StreamLight3DData::Ambient(AmbientLight3DState {
                            color: light.color,
                            intensity: light.intensity.max(0.0),
                            cast_shadows: light.cast_shadows,
                        }))
                    }
                    SceneNodeData::Sky3D(sky)
                        if lighting.sky.is_none()
                            && sky.visible
                            && sky.active
                            && stream_render_mask_matches(camera_mask, sky.render_layers) =>
                    {
                        Some(StreamLight3DData::Sky(Sky3DState {
                            day_colors: Arc::from(sky.day_colors.as_ref()),
                            evening_colors: Arc::from(sky.evening_colors.as_ref()),
                            night_colors: Arc::from(sky.night_colors.as_ref()),
                            horizon_colors: Arc::from(sky.horizon_colors.as_ref()),
                            time: SkyTime3DState {
                                time_of_day: sky.time.time_of_day,
                                paused: sky.time.paused,
                                scale: sky.time.scale,
                            },
                            shaders: Arc::from(
                                sky.shaders
                                    .iter()
                                    .map(|shader| SkyShaderPass3DState {
                                        path: shader.path.clone(),
                                        params: Arc::from(shader.params.as_ref()),
                                    })
                                    .collect::<Vec<_>>(),
                            ),
                        }))
                    }
                    SceneNodeData::RayLight3D(light)
                        if light.visible
                            && light.active
                            && stream_render_mask_matches(camera_mask, light.render_layers) =>
                    {
                        Some(StreamLight3DData::Ray {
                            transform: light.transform,
                            color: light.color,
                            intensity: light.intensity,
                            cast_shadows: light.cast_shadows,
                        })
                    }
                    SceneNodeData::PointLight3D(light)
                        if light.visible
                            && light.active
                            && light.range > 0.0
                            && stream_render_mask_matches(camera_mask, light.render_layers) =>
                    {
                        Some(StreamLight3DData::Point {
                            transform: light.transform,
                            color: light.color,
                            intensity: light.intensity,
                            range: light.range,
                            cast_shadows: light.cast_shadows,
                        })
                    }
                    SceneNodeData::SpotLight3D(light)
                        if light.visible
                            && light.active
                            && light.range > 0.0
                            && stream_render_mask_matches(camera_mask, light.render_layers) =>
                    {
                        Some(StreamLight3DData::Spot {
                            transform: light.transform,
                            color: light.color,
                            intensity: light.intensity,
                            range: light.range,
                            inner_angle_radians: light.inner_angle_radians,
                            outer_angle_radians: light.outer_angle_radians,
                            cast_shadows: light.cast_shadows,
                        })
                    }
                    _ => None,
                });
            match data {
                Some(StreamLight3DData::Ambient(light)) => lighting.ambient_light = Some(light),
                Some(StreamLight3DData::Sky(sky)) => lighting.sky = Some(sky),
                Some(StreamLight3DData::Ray {
                    transform,
                    color,
                    intensity,
                    cast_shadows,
                }) => {
                    let global = self
                        .get_render_global_transform_3d(node)
                        .unwrap_or(transform);
                    ray_lights.push(RayLight3DState {
                        direction: stream_quaternion_forward(global.rotation),
                        color,
                        intensity: intensity.max(0.0),
                        cast_shadows,
                    });
                }
                Some(StreamLight3DData::Point {
                    transform,
                    color,
                    intensity,
                    range,
                    cast_shadows,
                }) => {
                    let global = self
                        .get_render_global_transform_3d(node)
                        .unwrap_or(transform);
                    point_lights.push(PointLight3DState {
                        position: [global.position.x, global.position.y, global.position.z],
                        color,
                        intensity: intensity.max(0.0),
                        range: range.max(0.001),
                        cast_shadows,
                    });
                }
                Some(StreamLight3DData::Spot {
                    transform,
                    color,
                    intensity,
                    range,
                    inner_angle_radians,
                    outer_angle_radians,
                    cast_shadows,
                }) => {
                    let global = self
                        .get_render_global_transform_3d(node)
                        .unwrap_or(transform);
                    spot_lights.push(SpotLight3DState {
                        position: [global.position.x, global.position.y, global.position.z],
                        direction: stream_quaternion_forward(global.rotation),
                        color,
                        intensity: intensity.max(0.0),
                        range: range.max(0.001),
                        inner_angle_radians: inner_angle_radians.max(0.0),
                        outer_angle_radians: outer_angle_radians.max(inner_angle_radians),
                        cast_shadows,
                    });
                }
                None => {}
            }
        }
        for (slot, light) in lighting.ray_lights.iter_mut().zip(ray_lights) {
            *slot = Some(light);
        }
        for (slot, light) in lighting.point_lights.iter_mut().zip(point_lights) {
            *slot = Some(light);
        }
        for (slot, light) in lighting.spot_lights.iter_mut().zip(spot_lights) {
            *slot = Some(light);
        }
        lighting
    }

    fn collect_camera_stream_point_particles_3d(
        &mut self,
        camera_mask: BitMask,
        stream_node: NodeID,
    ) -> Arc<[(NodeID, PointParticles3DState)]> {
        let mut out = Vec::new();
        for idx in 0..self.camera_stream_node_scratch.len() {
            let node = self.camera_stream_node_scratch[idx];
            if node == stream_node || !self.is_effectively_visible(node) {
                continue;
            }
            let data = self
                .nodes
                .get(node)
                .and_then(|node_ref| match &node_ref.data {
                    SceneNodeData::ParticleEmitter3D(emitter)
                        if emitter.visible
                            && stream_render_mask_matches(camera_mask, emitter.render_layers) =>
                    {
                        Some((
                            emitter.profile.clone(),
                            emitter.sim_mode,
                            emitter.render_mode,
                            emitter.transform,
                            emitter.active,
                            emitter.looping,
                            emitter.prewarm,
                            emitter.spawn_rate,
                            emitter.seed,
                            emitter.params.clone(),
                            emitter.internal_simulation_time,
                        ))
                    }
                    _ => None,
                });
            let Some((
                profile_source,
                sim_mode,
                render_mode,
                transform,
                active,
                looping,
                prewarm,
                spawn_rate,
                seed,
                params,
                simulation_time,
            )) = data
            else {
                continue;
            };
            let profile = resolve_particle_profile_3d(self, &profile_source).unwrap_or_default();
            let lifetime_min = profile.lifetime_min.max(0.001);
            let lifetime_max = profile.lifetime_max.max(lifetime_min);
            let default_sim_mode = self
                .project()
                .map(|project| project.config.particle_sim_default)
                .unwrap_or(perro_project::ParticleSimDefault::Cpu);
            let model = self
                .get_render_global_transform_3d(node)
                .unwrap_or(transform)
                .to_mat4()
                .to_cols_array_2d();
            out.push((
                node,
                PointParticles3DState {
                    model,
                    active,
                    looping,
                    prewarm,
                    lifetime_min,
                    lifetime_max,
                    alive_budget: derived_particle_budget_3d(spawn_rate.max(0.0), lifetime_max),
                    emission_rate: spawn_rate.max(0.0),
                    speed_min: profile.speed_min.max(0.0),
                    speed_max: profile.speed_max.max(profile.speed_min.max(0.0)),
                    spread_radians: profile.spread_radians.clamp(0.0, std::f32::consts::PI),
                    size: profile.size.max(1.0),
                    size_min: profile.size_min.max(0.01),
                    size_max: profile.size_max.max(profile.size_min.max(0.01)),
                    gravity: profile.force,
                    color_start: profile.color_start,
                    color_end: profile.color_end,
                    emissive: profile.emissive,
                    seed,
                    params,
                    simulation_time: simulation_time.max(0.0),
                    simulation_delta: self.time.delta.max(0.0),
                    profile,
                    sim_mode: resolve_particle_sim_mode_3d(sim_mode, default_sim_mode),
                    render_mode: resolve_particle_render_mode_3d(render_mode),
                },
            ));
        }
        Arc::from(out)
    }

    fn collect_camera_stream_waters_3d(
        &mut self,
        camera_mask: BitMask,
        stream_node: NodeID,
    ) -> Arc<[(NodeID, Water3DState)]> {
        let mut out = Vec::new();
        for idx in 0..self.camera_stream_node_scratch.len() {
            let node = self.camera_stream_node_scratch[idx];
            if node == stream_node || !self.is_effectively_visible(node) {
                continue;
            }
            let data = self
                .nodes
                .get(node)
                .and_then(|node_ref| match &node_ref.data {
                    SceneNodeData::WaterBody3D(water)
                        if water.visible
                            && stream_render_mask_matches(camera_mask, water.render_layers) =>
                    {
                        Some((water.transform, water.water))
                    }
                    _ => None,
                });
            let Some((local_transform, water)) = data else {
                continue;
            };
            let model = self
                .get_render_global_transform_3d(node)
                .unwrap_or(local_transform)
                .to_mat4()
                .to_cols_array_2d();
            let coastline_shapes = self.collect_water_coastline_shapes_3d(node, &water);
            let queries = self.collect_water_queries_3d(node);
            let impacts = self.collect_water_impacts_3d(node, &water);
            let links = self.collect_water_links_3d(node, &water);
            out.push((
                node,
                Water3DState {
                    model,
                    paused: false,
                    simulation_time: self.time.elapsed,
                    simulation_delta: self.time.delta.max(0.0),
                    size: water_render_size_3d(water),
                    shape: water_shape_state_3d(water.shape),
                    resolution: water.resolution,
                    render_resolution: water.render_resolution,
                    depth: water.shape.depth(water.depth),
                    flow: [water.flow.x, water.flow.y],
                    wind: [water.wind.x, water.wind.y],
                    idle_mode: water_idle_mode_state_3d(water.idle_mode),
                    wave_speed: water.wave.speed,
                    wave_scale: water.wave.scale,
                    wave_length: water.wave.length,
                    damping: water.wave.damping,
                    wake_strength: water.physics.wake_strength,
                    foam_strength: water.physics.foam_strength,
                    sample_readback_rate: water.physics.sample_readback_rate,
                    lod_near_distance: water.lod.near_distance,
                    lod_mid_distance: water.lod.mid_distance,
                    lod_far_distance: water.lod.far_distance,
                    lod_min_resolution: water.lod.min_resolution,
                    collision_layers: water.collision_layers,
                    collision_mask: water.collision_mask,
                    deep_color: water.optics.deep_color,
                    shallow_color: water.optics.shallow_color,
                    shallow_depth: water.optics.shallow_depth,
                    sky_bias_ratio: water.optics.sky_bias.ratio(),
                    transparency: water.visual.transparency,
                    reflectivity: water.visual.reflectivity,
                    roughness: water.visual.roughness,
                    fresnel_power: water.visual.fresnel_power,
                    normal_strength: water.visual.normal_strength,
                    ripple_scale: water.visual.ripple_scale,
                    foam_color: water.visual.foam_color,
                    foam_amount: water.visual.foam_amount,
                    crest_foam_threshold: water.visual.crest_foam_threshold,
                    caustic_strength: water.visual.caustic_strength,
                    refraction_strength: water.visual.refraction_strength,
                    scattering_strength: water.visual.scattering_strength,
                    distance_fog_strength: water.visual.distance_fog_strength,
                    coastline_foam_color: water.coastline.foam_color,
                    coastline_foam_strength: water.coastline.foam_strength,
                    coastline_foam_width: water.coastline.foam_width,
                    coastline_cutoff_softness: water.coastline.cutoff_softness,
                    coastline_wave_reflection: water.coastline.wave_reflection,
                    coastline_wave_damping: water.coastline.wave_damping,
                    coastline_edge_noise: water.coastline.edge_noise,
                    debug: water.debug,
                    links,
                    queries,
                    impacts,
                    coastline_shapes,
                },
            ));
        }
        Arc::from(out)
    }

    fn camera_stream_source_state(
        &mut self,
        camera_node: NodeID,
    ) -> Option<CameraStreamSourceState> {
        if !self.is_effectively_visible(camera_node) {
            return None;
        }
        let camera_data = self
            .nodes
            .get(camera_node)
            .and_then(|node| match &node.data {
                SceneNodeData::Camera2D(camera) => Some((
                    camera.transform,
                    camera.zoom,
                    camera.render_mask,
                    camera.post_processing.clone(),
                    camera.audio_options.clone(),
                )),
                _ => None,
            });
        if let Some((local_transform, zoom, render_mask, post_processing, audio_options)) =
            camera_data
        {
            let global = self
                .get_render_global_transform_2d(camera_node)
                .unwrap_or(local_transform);
            return Some(CameraStreamSourceState::TwoD(Camera2DState {
                position: [global.position.x, global.position.y],
                rotation_radians: global.rotation,
                zoom,
                render_mask,
                post_processing: Arc::from(post_processing.to_effects_vec()),
                audio_options,
            }));
        }

        let camera_data = self
            .nodes
            .get(camera_node)
            .and_then(|node| match &node.data {
                SceneNodeData::Camera3D(camera) => Some((
                    camera.transform,
                    camera.projection.clone(),
                    camera.render_mask,
                    camera.post_processing.clone(),
                    camera.audio_options.clone(),
                )),
                _ => None,
            });
        let (local_transform, projection, render_mask, post_processing, audio_options) =
            camera_data?;
        let global = self
            .get_render_global_transform_3d(camera_node)
            .unwrap_or(local_transform);
        Some(CameraStreamSourceState::ThreeD(Camera3DState {
            position: [global.position.x, global.position.y, global.position.z],
            rotation: [
                global.rotation.x,
                global.rotation.y,
                global.rotation.z,
                global.rotation.w,
            ],
            projection: camera_stream_projection_state(&projection),
            render_mask,
            post_processing: Arc::from(post_processing.to_effects_vec()),
            audio_options,
        }))
    }
}

/// Camera-stream skinning palette. Shares the retained builder
/// (`build_skeleton_palette`) so the inverse-bind lane and 3-row affine packing
/// stay in one place; scratch buffers are threaded in from the caller to avoid
/// a per-draw allocation.
fn stream_skeleton_palette(
    nodes: &crate::cns::NodeArena,
    skeleton_id: NodeID,
    global_scratch: &mut Vec<Mat4>,
    palette_scratch: &mut Vec<[[f32; 4]; 3]>,
) -> Option<perro_render_bridge::SkeletonPalette> {
    crate::runtime::render_3d::build_skeleton_palette(
        nodes,
        skeleton_id,
        global_scratch,
        palette_scratch,
    )?;
    Some(perro_render_bridge::SkeletonPalette {
        matrices: Arc::from(palette_scratch.as_slice()),
    })
}

enum StreamMeshInstanceKind {
    Single,
    Dense {
        instance_scale: f32,
        poses: Arc<[DenseInstancePose3D]>,
    },
}

#[inline]
fn stream_render_mask_matches(camera_mask: BitMask, render_layers: BitMask) -> bool {
    !camera_mask.intersects(render_layers)
}

fn stream_quaternion_forward(rotation: perro_structs::Quaternion) -> [f32; 3] {
    let q = glam::Quat::from_xyzw(rotation.x, rotation.y, rotation.z, rotation.w).normalize();
    let forward = q * glam::Vec3::NEG_Z;
    [forward.x, forward.y, forward.z]
}

fn stream_sprite_region_uv(region: Option<[f32; 4]>) -> ([f32; 2], [f32; 2], [f32; 2]) {
    let Some([x, y, w, h]) = region else {
        return ([0.0, 0.0], [1.0, 1.0], [0.0, 0.0]);
    };
    if !(x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
        return ([0.0, 0.0], [1.0, 1.0], [0.0, 0.0]);
    }
    ([x, y], [x + w, y + h], [w, h])
}

fn camera_stream_projection_state(projection: &CameraProjection) -> CameraProjectionState {
    match projection {
        CameraProjection::Perspective {
            fov_y_degrees,
            near,
            far,
        } => CameraProjectionState::Perspective {
            fov_y_degrees: *fov_y_degrees,
            near: *near,
            far: *far,
        },
        CameraProjection::Orthographic { size, near, far } => CameraProjectionState::Orthographic {
            size: *size,
            near: *near,
            far: *far,
        },
        CameraProjection::Frustum {
            left,
            right,
            bottom,
            top,
            near,
            far,
        } => CameraProjectionState::Frustum {
            left: *left,
            right: *right,
            bottom: *bottom,
            top: *top,
            near: *near,
            far: *far,
        },
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
