use super::*;
use super::stream_3d::finish_stream_lighting_3d;

/// Output of one fused member collection: every lane a stream state carries.
/// Dimensions the stream did not request stay shared-empty / default.
struct StreamCollectedLanes {
    sprites_2d: Arc<[Sprite2DCommand]>,
    lights_2d: Arc<[Light2DState]>,
    point_particles_2d: Arc<[(NodeID, PointParticles2DState)]>,
    waters_2d: Arc<[(NodeID, Water2DState)]>,
    draws_3d: Arc<[CameraStreamDraw3DState]>,
    lighting_3d: CameraStreamLighting3DState,
    point_particles_3d: Arc<[(NodeID, PointParticles3DState)]>,
    waters_3d: Arc<[(NodeID, Water3DState)]>,
}

impl Default for StreamCollectedLanes {
    fn default() -> Self {
        Self {
            sprites_2d: empty_arc_slice(),
            lights_2d: empty_arc_slice(),
            point_particles_2d: empty_arc_slice(),
            waters_2d: empty_arc_slice(),
            draws_3d: empty_arc_slice(),
            lighting_3d: CameraStreamLighting3DState::default(),
            point_particles_3d: empty_arc_slice(),
            waters_3d: empty_arc_slice(),
        }
    }
}

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

    /// One member walk picks both explicit sub-view cameras (was 2 walks):
    /// last active `Camera2D` wins, highest activation-order `Camera3D` wins.
    fn sub_view_cameras(
        &mut self,
        localize_2d: &StreamLocalize2D,
        localize_3d: &StreamLocalize3D,
    ) -> (Option<Camera2DState>, Option<Camera3DState>) {
        let mut found_2d = None;
        let mut found_3d_priority: Option<(u64, u32, u32)> = None;
        let mut found_3d = None;
        for idx in 0..self.camera_stream_node_scratch.len() {
            let node = self.camera_stream_node_scratch[idx];
            let Some(scene_node) = self.nodes.get(node) else {
                continue;
            };
            match &scene_node.data {
                SceneNodeData::Camera2D(camera) => {
                    if !camera.active || !self.is_effectively_visible(node) {
                        continue;
                    }
                    found_2d = Some((
                        node,
                        camera.transform,
                        camera.zoom,
                        camera.render_mask,
                        camera.post_processing.clone(),
                        camera.audio_options.clone(),
                    ));
                }
                SceneNodeData::Camera3D(camera) => {
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
                    let replace = found_3d_priority
                        .map(|current| priority > current)
                        .unwrap_or(true);
                    if replace {
                        found_3d_priority = Some(priority);
                        found_3d = Some((
                            node,
                            camera.transform,
                            camera.projection.clone(),
                            camera.render_mask,
                            camera.post_processing.clone(),
                            camera.audio_options.clone(),
                        ));
                    }
                }
                _ => {}
            }
        }
        let camera_2d = found_2d.map(
            |(node, local_transform, zoom, render_mask, post_processing, audio_options)| {
                let transform = self
                    .stream_localized_transform_2d(node, localize_2d)
                    .unwrap_or(local_transform);
                Camera2DState {
                    position: [transform.position.x, transform.position.y],
                    rotation_radians: transform.rotation,
                    zoom,
                    render_mask,
                    post_processing: self.camera_postfx_arc(node, &post_processing),
                    audio_options,
                }
            },
        );
        let camera_3d = found_3d.map(
            |(node, local_transform, projection, render_mask, post_processing, audio_options)| {
                let transform = self
                    .stream_localized_transform_3d(node, localize_3d)
                    .unwrap_or(local_transform);
                Camera3DState {
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
                }
            },
        );
        (camera_2d, camera_3d)
    }

    pub(crate) fn camera_stream_texture_id(node: NodeID) -> TextureID {
        TextureID::from_parts(node.index(), node.generation())
    }

    /// Retained post-processing lane 4 a stream node (see `retained_arc_lane`).
    fn retained_post_fx(
        &mut self,
        stream_node: NodeID,
        built: &[perro_structs::PostProcessEffect],
    ) -> Arc<[perro_structs::PostProcessEffect]> {
        let lanes = self.stream_retention.lanes.entry(stream_node).or_default();
        retained_arc_lane(Some(&mut lanes.post_processing), built)
    }

    /// Fused member collection: ONE walk over the watched world fills every
    /// requested lane (was 4 walks per dimension, 8 for sub-views). Lane
    /// `Arc`s are retained per stream node: a refresh whose lane contents are
    /// unchanged hands back the previously emitted `Arc`, so graphics-side
    /// compares collapse to `Arc::ptr_eq`. `retain: false` = one-shot camera
    /// captures (no retention entry).
    fn collect_camera_stream_lanes(
        &mut self,
        stream_node: NodeID,
        two_d: Option<(BitMask, Option<[u32; 2]>)>,
        three_d: Option<BitMask>,
        retain: bool,
    ) -> StreamCollectedLanes {
        let localize_2d = two_d
            .is_some()
            .then(|| self.stream_localizer_2d(stream_node));
        let localize_3d = three_d
            .is_some()
            .then(|| self.stream_localizer_3d(stream_node));
        let mut out_2d = two_d.is_some().then(|| self.take_stream_2d_collect());
        let mut out_3d = three_d.is_some().then(|| self.take_stream_3d_collect());
        for idx in 0..self.camera_stream_node_scratch.len() {
            let node = self.camera_stream_node_scratch[idx];
            if node == stream_node
                || !self.is_effectively_visible(node)
                || self.stream_skips_isolated_child(node, stream_node)
            {
                continue;
            }
            if let (Some((mask, resolution)), Some(out), Some(localize)) =
                (two_d, out_2d.as_mut(), localize_2d.as_ref())
            {
                self.stream_member_2d(node, stream_node, mask, resolution, localize, out);
            }
            if let (Some(mask), Some(out), Some(localize)) =
                (three_d, out_3d.as_mut(), localize_3d.as_ref())
            {
                self.stream_member_3d(node, mask, localize, out);
            }
        }
        let mut lanes = StreamCollectedLanes::default();
        if let Some(out) = out_3d.as_mut() {
            lanes.lighting_3d = finish_stream_lighting_3d(out);
        }
        {
            let mut retained = retain.then(|| {
                self.stream_retention
                    .lanes
                    .entry(stream_node)
                    .or_default()
            });
            if let Some(out) = out_2d.as_ref() {
                lanes.sprites_2d = retained_arc_lane(
                    retained.as_deref_mut().map(|lanes| &mut lanes.sprites_2d),
                    &out.sprites,
                );
                lanes.lights_2d = retained_arc_lane(
                    retained.as_deref_mut().map(|lanes| &mut lanes.lights_2d),
                    &out.lights,
                );
                lanes.point_particles_2d = retained_arc_lane(
                    retained
                        .as_deref_mut()
                        .map(|lanes| &mut lanes.point_particles_2d),
                    &out.particles,
                );
                lanes.waters_2d = retained_arc_lane(
                    retained.as_deref_mut().map(|lanes| &mut lanes.waters_2d),
                    &out.waters,
                );
            }
            if let Some(out) = out_3d.as_ref() {
                lanes.draws_3d = retained_arc_lane(
                    retained.as_deref_mut().map(|lanes| &mut lanes.draws_3d),
                    &out.draws,
                );
                lanes.point_particles_3d = retained_arc_lane(
                    retained
                        .as_deref_mut()
                        .map(|lanes| &mut lanes.point_particles_3d),
                    &out.particles,
                );
                lanes.waters_3d = retained_arc_lane(
                    retained.map(|lanes| &mut lanes.waters_3d),
                    &out.waters,
                );
            }
        }
        if let Some(out) = out_2d.take() {
            self.restore_stream_2d_collect(out);
        }
        if let Some(out) = out_3d.take() {
            self.restore_stream_3d_collect(out);
        }
        lanes
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
            let post_processing = self.retained_post_fx(stream_node, &post_processing);
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
                post_processing,
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
        let post_processing = self.retained_post_fx(stream_node, &post_processing);
        let (two_d, three_d) = match &source {
            CameraStreamSourceState::TwoD(camera) => (Some((camera.render_mask, None)), None),
            CameraStreamSourceState::ThreeD(camera) => (None, Some(camera.render_mask)),
            CameraStreamSourceState::Webcam { .. } => (None, None),
        };
        let lanes = self.collect_camera_stream_lanes(stream_node, two_d, three_d, true);
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
            post_processing,
            output_texture,
            sprites_2d: lanes.sprites_2d,
            lights_2d: lanes.lights_2d,
            point_particles_2d: lanes.point_particles_2d,
            waters_2d: lanes.waters_2d,
            draws_3d: lanes.draws_3d,
            lighting_3d: lanes.lighting_3d,
            point_particles_3d: lanes.point_particles_3d,
            waters_3d: lanes.waters_3d,
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
        let (two_d, three_d) = match &source {
            CameraStreamSourceState::TwoD(camera) => {
                (Some((camera.render_mask, Some(resolution))), None)
            }
            CameraStreamSourceState::ThreeD(camera) => (None, Some(camera.render_mask)),
            CameraStreamSourceState::Webcam { .. } => unreachable!(),
        };
        // one-shot capture: no retention entry (always paired upsert/remove).
        let lanes = self.collect_camera_stream_lanes(camera_node, two_d, three_d, false);
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
            sprites_2d: lanes.sprites_2d,
            lights_2d: lanes.lights_2d,
            point_particles_2d: lanes.point_particles_2d,
            waters_2d: lanes.waters_2d,
            draws_3d: lanes.draws_3d,
            lighting_3d: lanes.lighting_3d,
            point_particles_3d: lanes.point_particles_3d,
            waters_3d: lanes.waters_3d,
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
        // root inverses once per refresh; shared by the camera pick + the
        // fused member walk (was 1 inverse per member per collector).
        let localize_2d = self.stream_localizer_2d(view_node);
        let localize_3d = self.stream_localizer_3d(view_node);

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
        let (camera_2d, camera_3d) = self.sub_view_cameras(&localize_2d, &localize_3d);
        let camera_3d = camera_3d.unwrap_or(implicit_camera_3d);
        let camera_2d = camera_2d.unwrap_or(implicit_camera_2d);
        let render_mask_3d = camera_3d.render_mask;
        let render_mask_2d = camera_2d.render_mask;
        let mut post_processing = camera_3d.post_processing.to_vec();
        post_processing.extend(camera_2d.post_processing.iter().cloned());
        post_processing.extend(view.post_processing.to_effects_vec());
        let post_processing = self.retained_post_fx(view_node, &post_processing);
        let source = CameraStreamSourceState::ThreeD(camera_3d);
        let overlay_camera_2d = Some(camera_2d);
        let lanes = self.collect_camera_stream_lanes(
            view_node,
            Some((render_mask_2d, Some(resolution))),
            Some(render_mask_3d),
            true,
        );

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
            post_processing,
            output_texture: Self::camera_stream_texture_id(view_node),
            sprites_2d: lanes.sprites_2d,
            lights_2d: lanes.lights_2d,
            point_particles_2d: lanes.point_particles_2d,
            waters_2d: lanes.waters_2d,
            draws_3d: lanes.draws_3d,
            lighting_3d: lanes.lighting_3d,
            point_particles_3d: lanes.point_particles_3d,
            waters_3d: lanes.waters_3d,
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

#[cfg(test)]
mod stream_retention_tests {
    use super::*;
    use perro_nodes::{AmbientLight2D, Node3D, SubView3D as SubView3DNode};
    use perro_runtime_api::sub_apis::NodeAPI;

    fn sub_view_of(runtime: &Runtime, view: NodeID) -> SubView {
        match &runtime
            .nodes
            .get(view)
            .expect("test or bench setup must succeed")
            .data
        {
            SceneNodeData::SubView3D(node) => node.sub_view.clone(),
            _ => panic!("expected SubView3D"),
        }
    }

    fn retention_scene(runtime: &mut Runtime) -> (NodeID, NodeID, NodeID) {
        let view = NodeAPI::create::<SubView3DNode>(runtime);
        let light = NodeAPI::create::<AmbientLight2D>(runtime);
        let mover = NodeAPI::create::<Node3D>(runtime);
        assert!(runtime.reparent(view, light));
        assert!(runtime.reparent(view, mover));
        if let Some(mut node) = runtime.nodes.get_mut(view)
            && let SceneNodeData::SubView3D(sub_view) = &mut node.data
        {
            sub_view.sub_view.enabled = true;
        }
        if let Some(mut node) = runtime.nodes.get_mut(light)
            && let SceneNodeData::AmbientLight2D(ambient) = &mut node.data
        {
            ambient.active = true;
            ambient.intensity = 1.0;
        }
        (view, light, mover)
    }

    #[test]
    fn unchanged_refresh_reuses_lane_arcs() {
        let mut runtime = Runtime::new();
        let (view, light, mover) = retention_scene(&mut runtime);
        let sub_view = sub_view_of(&runtime, view);

        let first = runtime
            .sub_view_state(view, &sub_view, None)
            .expect("test or bench setup must succeed");
        assert!(!first.lights_2d.is_empty());
        let second = runtime
            .sub_view_state(view, &sub_view, None)
            .expect("test or bench setup must succeed");
        assert!(Arc::ptr_eq(&first.lights_2d, &second.lights_2d));
        assert!(Arc::ptr_eq(&first.post_processing, &second.post_processing));

        // unrelated node change: untouched lane keeps its Arc.
        if let Some(mut node) = runtime.nodes.get_mut(mover)
            && let SceneNodeData::Node3D(node_3d) = &mut node.data
        {
            node_3d.transform.position.x += 1.0;
        }
        let third = runtime
            .sub_view_state(view, &sub_view, None)
            .expect("test or bench setup must succeed");
        assert!(Arc::ptr_eq(&second.lights_2d, &third.lights_2d));

        // lane content change: fresh Arc.
        if let Some(mut node) = runtime.nodes.get_mut(light)
            && let SceneNodeData::AmbientLight2D(ambient) = &mut node.data
        {
            ambient.intensity = 0.5;
        }
        let fourth = runtime
            .sub_view_state(view, &sub_view, None)
            .expect("test or bench setup must succeed");
        assert!(!Arc::ptr_eq(&third.lights_2d, &fourth.lights_2d));
    }

    #[test]
    fn equal_state_upsert_resends_prior_whole_state_arc() {
        let mut runtime = Runtime::new();
        let (view, _light, _mover) = retention_scene(&mut runtime);
        let sub_view = sub_view_of(&runtime, view);

        let state = runtime
            .sub_view_state(view, &sub_view, None)
            .expect("test or bench setup must succeed");
        let first = Arc::new(state.clone());
        runtime.queue_camera_stream_upsert(view, first.clone());
        let retained = runtime
            .stream_retention
            .states
            .get(&view)
            .expect("retained whole state")
            .clone();
        assert!(Arc::ptr_eq(&retained, &first));

        // equal rebuild in a fresh Arc: retention keeps the FIRST Arc so the
        // gpu-side upsert compare hits Arc::ptr_eq.
        runtime.queue_camera_stream_upsert(view, Arc::new(state));
        let still_retained = runtime
            .stream_retention
            .states
            .get(&view)
            .expect("retained whole state")
            .clone();
        assert!(Arc::ptr_eq(&still_retained, &first));
    }
}
