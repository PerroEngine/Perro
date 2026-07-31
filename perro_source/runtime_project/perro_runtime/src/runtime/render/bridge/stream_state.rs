use super::*;

impl Runtime {
    /// Scratch maps come from the caller (cleared here) so per-node calls in
    /// hot rebuild loops reuse capacity instead of allocating fresh containers.
    pub(super) fn nested_ui_sub_view_rect(
        &self,
        owner: NodeID,
        node: NodeID,
        owner_resolution: [u32; 2],
        computed: &mut AHashMap<NodeID, ComputedUiRect>,
        scales: &mut AHashMap<NodeID, Vector2>,
        auto: &mut AHashSet<NodeID>,
    ) -> Option<ComputedUiRect> {
        computed.clear();
        scales.clear();
        auto.clear();
        let root = ComputedUiRect::new(
            Vector2::ZERO,
            Vector2::new(owner_resolution[0] as f32, owner_resolution[1] as f32),
        );
        computed.insert(owner, root);
        scales.insert(owner, Vector2::ONE);
        self.compute_ui_rect(node, root, computed, scales, auto)
    }

    fn prepare_nested_sub_views(&mut self, owner: NodeID, owner_resolution: [u32; 2]) {
        let mut members = self.render_ui.acquire_node_vec();
        self.fill_world_members(owner, &mut members);
        // take-pattern scratch: recursion (views nested in views) takes empty
        // maps, the common depth-1 case reuses persisted capacity.
        let mut computed = std::mem::take(&mut self.render_ui.nested_rect_computed_scratch);
        let mut scales = std::mem::take(&mut self.render_ui.nested_rect_scales_scratch);
        let mut auto = std::mem::take(&mut self.render_ui.nested_rect_auto_scratch);
        for &node in members.iter() {
            let Some(scene_node) = self.nodes.get(node) else {
                continue;
            };
            let (view, auto_size) = match &scene_node.data {
                SceneNodeData::UiSubView(view) => {
                    let view = SubView::from(view.as_ref());
                    let auto_size = self
                        .nested_ui_sub_view_rect(
                            owner,
                            node,
                            owner_resolution,
                            &mut computed,
                            &mut scales,
                            &mut auto,
                        )
                        .map(|rect| [rect.size.x, rect.size.y])
                        .or_else(|| {
                            self.render_ui
                                .retained_rects
                                .get(&node)
                                .map(|rect| rect.size)
                        });
                    (view, auto_size)
                }
                SceneNodeData::SubView2D(view) => (view.sub_view.clone(), None),
                SceneNodeData::SubView3D(view) => (view.sub_view.clone(), None),
                _ => continue,
            };
            if !self.is_effectively_visible(node) {
                self.queue_camera_stream_remove(node);
                continue;
            }
            if let Some(state) = self.sub_view_state(node, &view, auto_size) {
                self.queue_camera_stream_upsert(node, Arc::new(state));
            } else {
                self.queue_camera_stream_remove(node);
            }
        }
        computed.clear();
        scales.clear();
        auto.clear();
        self.render_ui.nested_rect_computed_scratch = computed;
        self.render_ui.nested_rect_scales_scratch = scales;
        self.render_ui.nested_rect_auto_scratch = auto;
        self.render_ui.release_node_vec(members);
    }

    fn sub_view_camera_2d(&mut self, view_node: NodeID) -> Option<Camera2DState> {
        let mut found = None;
        for idx in 0..self.camera_stream_node_scratch.len() {
            let node = self.camera_stream_node_scratch[idx];
            let Some(scene_node) = self.nodes.get(node) else {
                continue;
            };
            let SceneNodeData::Camera2D(camera) = &scene_node.data else {
                continue;
            };
            if !camera.active || !self.is_effectively_visible(node) {
                continue;
            }
            found = Some((
                node,
                camera.transform,
                camera.zoom,
                camera.render_mask,
                camera.post_processing.clone(),
                camera.audio_options.clone(),
            ));
        }
        let (node, local_transform, zoom, render_mask, post_processing, audio_options) = found?;
        let transform = self
            .stream_render_transform_2d(node, view_node)
            .unwrap_or(local_transform);
        Some(Camera2DState {
            position: [transform.position.x, transform.position.y],
            rotation_radians: transform.rotation,
            zoom,
            render_mask,
            post_processing: self.camera_postfx_arc(node, &post_processing),
            audio_options,
        })
    }

    fn sub_view_camera_3d(&mut self, view_node: NodeID) -> Option<Camera3DState> {
        let mut found_priority: Option<(u64, u32, u32)> = None;
        let mut found = None;
        for idx in 0..self.camera_stream_node_scratch.len() {
            let node = self.camera_stream_node_scratch[idx];
            let Some(scene_node) = self.nodes.get(node) else {
                continue;
            };
            let SceneNodeData::Camera3D(camera) = &scene_node.data else {
                continue;
            };
            if !camera.active || !self.is_effectively_visible(node) {
                continue;
            }
            let order = self
                .render_3d
                .camera_activation_order
                .get(&node)
                .copied()
                .unwrap_or(0);
            let priority = (order, node.generation(), node.index());
            let replace = found_priority
                .map(|current| priority > current)
                .unwrap_or(true);
            if replace {
                found_priority = Some(priority);
                found = Some((
                    node,
                    camera.transform,
                    camera.projection.clone(),
                    camera.render_mask,
                    camera.post_processing.clone(),
                    camera.audio_options.clone(),
                ));
            }
        }
        let (node, local_transform, projection, render_mask, post_processing, audio_options) =
            found?;
        let transform = self
            .stream_render_transform_3d(node, view_node)
            .unwrap_or(local_transform);
        Some(Camera3DState {
            position: [
                transform.position.x,
                transform.position.y,
                transform.position.z,
            ],
            rotation: [
                transform.rotation.x,
                transform.rotation.y,
                transform.rotation.z,
                transform.rotation.w,
            ],
            projection: camera_stream_projection_state(&projection),
            render_mask,
            post_processing: self.camera_postfx_arc(node, &post_processing),
            audio_options,
        })
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
        // webcam sources render nothing from the scene: only source/output/
        // resolution/post-processing matter. short-circuit before the O(all
        // nodes) scratch fill + collectors (which return empty for webcam).
        if let CameraStreamSourceState::Webcam { texture, .. } = &source {
            let post_processing = stream.post_processing.to_effects_vec();
            let has_image_effect = post_processing
                .iter()
                .any(|effect| !matches!(effect, perro_structs::PostProcessEffect::Exposure { .. }));
            let output_texture = if has_image_effect {
                Self::camera_stream_texture_id(stream_node)
            } else {
                *texture
            };
            return Some(CameraStreamState {
                source,
                tone_map_output: matches!(
                    self.nodes.get(stream_node).map(|node| &node.data),
                    Some(SceneNodeData::UiCameraStream(_))
                ),
                overlay_camera_2d: None,
                transparent_background: true,
                clear_color: None,
                resolution: [
                    stream.resolution.x.clamp(1, 8192),
                    stream.resolution.y.clamp(1, 8192),
                ],
                aspect_ratio: stream.aspect_ratio.max(0.0),
                post_processing: arc_slice_from_vec(post_processing),
                output_texture,
                sprites_2d: empty_arc_slice(),
                lights_2d: empty_arc_slice(),
                point_particles_2d: empty_arc_slice(),
                waters_2d: empty_arc_slice(),
                draws_3d: empty_arc_slice(),
                lighting_3d: CameraStreamLighting3DState::default(),
                point_particles_3d: empty_arc_slice(),
                waters_3d: empty_arc_slice(),
            });
        }
        // Share the owning world's direct member list once (refcount clone of
        // the membership cache; no per-stream copy). Nested sub-view
        // descendants never enter any collector for this stream.
        let owner = self.node_world(stream_node)?;
        self.camera_stream_node_scratch = self.world_members_arc(owner);
        let mut post_processing = match &source {
            CameraStreamSourceState::TwoD(camera) => camera.post_processing.to_vec(),
            CameraStreamSourceState::ThreeD(camera) => camera.post_processing.to_vec(),
            CameraStreamSourceState::Webcam { .. } => Vec::new(),
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
                self.collect_camera_stream_sprites_2d(camera.render_mask, stream_node, None),
                self.collect_camera_stream_lights_2d(camera.render_mask, stream_node),
                self.collect_camera_stream_point_particles_2d(camera.render_mask, stream_node),
                self.collect_camera_stream_waters_2d(camera.render_mask, stream_node),
                empty_arc_slice(),
                CameraStreamLighting3DState::default(),
                empty_arc_slice(),
                empty_arc_slice(),
            ),
            CameraStreamSourceState::ThreeD(camera) => (
                empty_arc_slice(),
                empty_arc_slice(),
                empty_arc_slice(),
                empty_arc_slice(),
                self.collect_camera_stream_draws_3d(camera.render_mask, stream_node),
                self.collect_camera_stream_lighting_3d(camera.render_mask, stream_node),
                self.collect_camera_stream_point_particles_3d(camera.render_mask, stream_node),
                self.collect_camera_stream_waters_3d(camera.render_mask, stream_node),
            ),
            CameraStreamSourceState::Webcam { .. } => (
                empty_arc_slice(),
                empty_arc_slice(),
                empty_arc_slice(),
                empty_arc_slice(),
                empty_arc_slice(),
                CameraStreamLighting3DState::default(),
                empty_arc_slice(),
                empty_arc_slice(),
            ),
        };
        let output_texture = match &source {
            CameraStreamSourceState::Webcam { texture, .. } => *texture,
            _ => Self::camera_stream_texture_id(stream_node),
        };
        Some(CameraStreamState {
            source,
            tone_map_output: matches!(
                self.nodes.get(stream_node).map(|node| &node.data),
                Some(SceneNodeData::UiCameraStream(_))
            ),
            overlay_camera_2d: None,
            transparent_background: true,
            clear_color: None,
            resolution: [
                stream.resolution.x.clamp(1, 8192),
                stream.resolution.y.clamp(1, 8192),
            ],
            aspect_ratio: stream.aspect_ratio.max(0.0),
            post_processing: arc_slice_from_vec(post_processing),
            output_texture,
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

    pub(crate) fn camera_capture_state(
        &mut self,
        camera_node: NodeID,
        output_texture: TextureID,
        resolution: [u32; 2],
    ) -> Option<CameraStreamState> {
        let source = self.camera_stream_source_state(camera_node)?;
        if matches!(source, CameraStreamSourceState::Webcam { .. }) {
            return None;
        }
        let owner = self.node_world(camera_node)?;
        self.camera_stream_node_scratch = self.world_members_arc(owner);
        let post_processing = match &source {
            CameraStreamSourceState::TwoD(camera) => camera.post_processing.clone(),
            CameraStreamSourceState::ThreeD(camera) => camera.post_processing.clone(),
            CameraStreamSourceState::Webcam { .. } => empty_arc_slice(),
        };
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
                self.collect_camera_stream_sprites_2d(
                    camera.render_mask,
                    camera_node,
                    Some(resolution),
                ),
                self.collect_camera_stream_lights_2d(camera.render_mask, camera_node),
                self.collect_camera_stream_point_particles_2d(camera.render_mask, camera_node),
                self.collect_camera_stream_waters_2d(camera.render_mask, camera_node),
                empty_arc_slice(),
                CameraStreamLighting3DState::default(),
                empty_arc_slice(),
                empty_arc_slice(),
            ),
            CameraStreamSourceState::ThreeD(camera) => (
                empty_arc_slice(),
                empty_arc_slice(),
                empty_arc_slice(),
                empty_arc_slice(),
                self.collect_camera_stream_draws_3d(camera.render_mask, camera_node),
                self.collect_camera_stream_lighting_3d(camera.render_mask, camera_node),
                self.collect_camera_stream_point_particles_3d(camera.render_mask, camera_node),
                self.collect_camera_stream_waters_3d(camera.render_mask, camera_node),
            ),
            CameraStreamSourceState::Webcam { .. } => unreachable!(),
        };
        Some(CameraStreamState {
            source,
            tone_map_output: true,
            overlay_camera_2d: None,
            transparent_background: false,
            clear_color: None,
            resolution: [resolution[0].clamp(1, 8192), resolution[1].clamp(1, 8192)],
            aspect_ratio: 0.0,
            post_processing,
            output_texture,
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

    pub(crate) fn sub_view_state(
        &mut self,
        view_node: NodeID,
        view: &SubView,
        auto_size: Option<[f32; 2]>,
    ) -> Option<CameraStreamState> {
        if !view.enabled {
            return None;
        }

        const AUTO_RESOLUTION_SCALE: f32 = 2.0;
        let resolution = [
            if view.resolution.x == 0 {
                (auto_size.unwrap_or([1.0, 1.0])[0] * AUTO_RESOLUTION_SCALE)
                    .round()
                    .clamp(1.0, 8192.0) as u32
            } else {
                view.resolution.x.clamp(1, 8192)
            },
            if view.resolution.y == 0 {
                (auto_size.unwrap_or([1.0, 1.0])[1] * AUTO_RESOLUTION_SCALE)
                    .round()
                    .clamp(1.0, 8192.0) as u32
            } else {
                view.resolution.y.clamp(1, 8192)
            },
        ];

        self.prepare_nested_sub_views(view_node, resolution);
        self.camera_stream_node_scratch = self.world_members_arc(view_node);

        let implicit_camera_3d = Camera3DState {
            position: [
                view.view_position.x,
                view.view_position.y,
                view.view_position.z,
            ],
            rotation: [
                view.view_rotation.x,
                view.view_rotation.y,
                view.view_rotation.z,
                view.view_rotation.w,
            ],
            projection: camera_stream_projection_state(&view.projection),
            render_mask: BitMask::NONE,
            post_processing: empty_arc_slice(),
            audio_options: perro_structs::AudioListenerOptions::new(),
        };
        let implicit_camera_2d = Camera2DState {
            position: [view.view_2d_position.x, view.view_2d_position.y],
            rotation_radians: view.view_2d_rotation,
            zoom: view.view_2d_zoom.max(0.001),
            render_mask: BitMask::NONE,
            post_processing: empty_arc_slice(),
            audio_options: perro_structs::AudioListenerOptions::new(),
        };
        let camera_3d = self
            .sub_view_camera_3d(view_node)
            .unwrap_or(implicit_camera_3d);
        let camera_2d = self
            .sub_view_camera_2d(view_node)
            .unwrap_or(implicit_camera_2d);
        let render_mask_3d = camera_3d.render_mask;
        let render_mask_2d = camera_2d.render_mask;
        let mut post_processing = camera_3d.post_processing.to_vec();
        post_processing.extend(camera_2d.post_processing.iter().cloned());
        post_processing.extend(view.post_processing.to_effects_vec());
        let source = CameraStreamSourceState::ThreeD(camera_3d);
        let overlay_camera_2d = Some(camera_2d);

        Some(CameraStreamState {
            source,
            // Keep owned sub-view output scene-linear. The UI composite writes
            // it through the surface view, which performs the display transfer.
            // Applying ACES here compresses saturated art before composition.
            tone_map_output: false,
            overlay_camera_2d,
            transparent_background: view.background.a() < 1.0,
            clear_color: Some(view.background),
            resolution,
            aspect_ratio: view.aspect_ratio.max(0.0),
            post_processing: arc_slice_from_vec(post_processing),
            output_texture: Self::camera_stream_texture_id(view_node),
            sprites_2d: self.collect_camera_stream_sprites_2d(
                render_mask_2d,
                view_node,
                Some(resolution),
            ),
            lights_2d: self.collect_camera_stream_lights_2d(render_mask_2d, view_node),
            point_particles_2d: self
                .collect_camera_stream_point_particles_2d(render_mask_2d, view_node),
            waters_2d: self.collect_camera_stream_waters_2d(render_mask_2d, view_node),
            draws_3d: self.collect_camera_stream_draws_3d(render_mask_3d, view_node),
            lighting_3d: self.collect_camera_stream_lighting_3d(render_mask_3d, view_node),
            point_particles_3d: self
                .collect_camera_stream_point_particles_3d(render_mask_3d, view_node),
            waters_3d: self.collect_camera_stream_waters_3d(render_mask_3d, view_node),
        })
    }

    pub(super) fn camera_stream_source_state(
        &mut self,
        camera_node: NodeID,
    ) -> Option<CameraStreamSourceState> {
        if !self.is_effectively_visible(camera_node) {
            let _ = self.resource_api.release_webcam_node_slot(camera_node);
            return None;
        }
        if self.nodes.get(camera_node).is_some_and(
            |node| matches!(&node.data, SceneNodeData::Webcam(webcam) if !webcam.enabled),
        ) {
            let _ = self.resource_api.release_webcam_node_slot(camera_node);
            return None;
        }
        let webcam_data = self
            .nodes
            .get(camera_node)
            .and_then(|node| match &node.data {
                SceneNodeData::Webcam(webcam) if webcam.enabled => Some(webcam.config.clone()),
                _ => None,
            });
        if let Some(config) = webcam_data {
            let webcam = self
                .resource_api
                .ensure_webcam_node_slot(camera_node, config);
            let texture = perro_resource_api::sub_apis::WebcamAPI::webcam_texture(
                self.resource_api.as_ref(),
                webcam,
            );
            let resolution = perro_resource_api::sub_apis::WebcamAPI::webcam_resolution(
                self.resource_api.as_ref(),
                webcam,
            )
            .or_else(|| {
                self.nodes
                    .get(camera_node)
                    .and_then(|node| match &node.data {
                        SceneNodeData::Webcam(webcam) => {
                            Some([webcam.config.width.max(1), webcam.config.height.max(1)])
                        }
                        _ => None,
                    })
            })
            .unwrap_or([1, 1]);
            return Some(CameraStreamSourceState::Webcam {
                texture,
                resolution,
            });
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
                post_processing: self.camera_postfx_arc(camera_node, &post_processing),
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
            post_processing: self.camera_postfx_arc(camera_node, &post_processing),
            audio_options,
        }))
    }
}
