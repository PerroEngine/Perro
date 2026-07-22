use super::*;

impl Runtime {
    pub fn queue_render_command(&mut self, command: RenderCommand) {
        self.render.queue_command(command);
    }

    pub(super) fn count_scene_resource_refs_into(
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
                SceneNodeData::Sprite3D(sprite)
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
                SceneNodeData::VideoPlayer2D(video)
                    if !self.render_2d.retained_sprites.contains_key(&node_id) =>
                {
                    if !video.video.texture.is_nil()
                        && !self.resource_api.is_texture_id_pending(video.video.texture)
                    {
                        add_ref(textures, video.video.texture, node_id);
                    }
                }
                SceneNodeData::VideoPlayer3D(video) => {
                    if !video.video.texture.is_nil()
                        && !self.resource_api.is_texture_id_pending(video.video.texture)
                    {
                        add_ref(textures, video.video.texture, node_id);
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
                SceneNodeData::NineSliceButton2D(button)
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
                SceneNodeData::UiVideoPlayer(video)
                    if !self.render_ui.retained_commands.contains_key(&node_id) =>
                {
                    if !video.video.texture.is_nil()
                        && !self.resource_api.is_texture_id_pending(video.video.texture)
                    {
                        add_ref(textures, video.video.texture, node_id);
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
                SceneNodeData::UiNineSliceButton(button)
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
                | RenderEvent::TextureCreated { .. }
                | RenderEvent::TextureLoaded { .. }
        ) {
            self.request_full_2d_scan_once();
            self.request_full_3d_scan_once();
            // SubView owns a retained camera-stream snapshot. Resource
            // completion must rebuild that snapshot too; a global 3D scan does
            // not revisit UI-local mesh descendants.
            let viewport_nodes = self
                .nodes
                .iter()
                .filter_map(|(node, node_ref)| {
                    matches!(
                        node_ref.data,
                        SceneNodeData::UiSubView(_)
                            | SceneNodeData::SubView2D(_)
                            | SceneNodeData::SubView3D(_)
                    )
                    .then_some(node)
                })
                .collect::<Vec<_>>();
            for node in viewport_nodes {
                self.mark_ui_dirty(node, Self::UI_DIRTY_COMMANDS);
            }
        }
        // resource lifecycle events resolve pending resources / invalidate
        // retained draws, which arena mutation_revision can't see. force
        // resource-ref re-scan. water sample telemetry arrives every physics
        // tick + never touches resource pending/loaded/retained state (its
        // handlers only write water sample caches), so exclude it: else the
        // O(all nodes) ref scan reruns every tick in any scene w/ water.
        // stream texel updates (webcam/video, ~30/s) only change pixels of an
        // already-referenced texture; refs never change, so exclude them too:
        // else every frame reruns the O(all nodes) ref scan + full 2d/3d scan.
        if !matches!(
            event,
            RenderEvent::HdrStatusChanged(_)
                | RenderEvent::WaterSamples { .. }
                | RenderEvent::WaterBodySamples { .. }
                | RenderEvent::TextureTexelsUpdated { .. }
        ) {
            self.scene_resource_refs_dirty = true;
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
        if let Some(view) = self.sub_view_ancestor(id) {
            self.dirty.mark_rerender(view);
            if self
                .nodes
                .get(view)
                .is_some_and(|node| matches!(node.data, SceneNodeData::UiSubView(_)))
            {
                self.mark_ui_dirty(view, Self::UI_DIRTY_COMMANDS);
            }
        }
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
        let mut seen = AHashSet::default();
        stack.clear();
        stack.push(root_id);
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            let ui_dirty = is_ui_node_data(&node.data);
            // direct field-path split: &self.nodes borrow (via `node`) stays
            // live across the &mut self.dirty calls below (disjoint fields).
            if let Some(children) = self.nodes.children(id) {
                stack.extend_from_slice(children);
            }
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
        if let Some(view) = self.sub_view_ancestor(root_id) {
            self.dirty.mark_rerender(view);
            if self
                .nodes
                .get(view)
                .is_some_and(|node| matches!(node.data, SceneNodeData::UiSubView(_)))
            {
                self.mark_ui_dirty(view, Self::UI_DIRTY_COMMANDS);
            }
        }
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
        if self.nodes.children(root).is_none_or(<[NodeID]>::is_empty) {
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

    pub(crate) fn note_removed_render_node(&mut self, node: NodeID, ty: NodeType) {
        if matches!(ty, NodeType::Webcam) {
            let _ = self.resource_api.release_webcam_node_slot(node);
        }
        if matches!(
            ty,
            NodeType::VideoPlayer2D | NodeType::VideoPlayer3D | NodeType::UiVideoPlayer
        ) {
            let _ = perro_resource_api::sub_apis::VideoAPI::video_release_node(
                self.resource_api.as_ref(),
                node,
            );
        }
        self.render_2d.note_removed_node(node);
        self.render_3d.note_removed_node(node);
        self.render_ui.note_removed_node(node);
        self.locale_text.remove_node_bindings(node);
        if matches!(
            ty,
            NodeType::CameraStream2D
                | NodeType::CameraStream3D
                | NodeType::UiCameraStream
                | NodeType::SubView2D
                | NodeType::SubView3D
                | NodeType::UiSubView
                | NodeType::Webcam
        ) {
            self.queue_render_command(RenderCommand::CameraStream(
                CameraStreamCommand::RemoveNode { node },
            ));
        }
        if matches!(ty.get_renderable(), Renderable::True) {
            match ty.get_spatial() {
                Spatial::TwoD => {
                    self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
                    self.queue_render_command(RenderCommand::Ui(
                        perro_render_bridge::UiCommand::RemoveNode { node },
                    ));
                }
                Spatial::ThreeD => {
                    self.queue_render_command(RenderCommand::ThreeD(Box::new(
                        Command3D::RemoveNode { node },
                    )));
                    self.queue_render_command(RenderCommand::Ui(
                        perro_render_bridge::UiCommand::RemoveNode { node },
                    ));
                }
                Spatial::None => {
                    if matches!(
                        ty,
                        NodeType::UiCameraStream
                            | NodeType::UiSubView
                            | NodeType::UiPanel
                            | NodeType::UiProgressBar
                            | NodeType::UiButton
                            | NodeType::UiDropdown
                            | NodeType::UiColorPicker
                            | NodeType::UiShape
                            | NodeType::UiCheckbox
                            | NodeType::UiImage
                            | NodeType::UiImageButton
                            | NodeType::UiNineSliceButton
                            | NodeType::UiNineSlice
                            | NodeType::UiAnimatedImage
                            | NodeType::UiLabel
                            | NodeType::UiTextBox
                            | NodeType::UiTextBlock
                    ) {
                        self.queue_render_command(RenderCommand::Ui(
                            perro_render_bridge::UiCommand::RemoveNode { node },
                        ));
                    }
                }
            }
        }
    }
}
