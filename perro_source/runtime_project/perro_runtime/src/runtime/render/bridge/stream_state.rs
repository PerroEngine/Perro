use super::*;

impl Runtime {
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
            let output_texture = *texture;
            return Some(CameraStreamState {
                source,
                overlay_camera_2d: None,
                clear_color: None,
                resolution: [
                    stream.resolution.x.clamp(1, 8192),
                    stream.resolution.y.clamp(1, 8192),
                ],
                aspect_ratio: stream.aspect_ratio.max(0.0),
                post_processing: Arc::from(stream.post_processing.to_effects_vec()),
                output_texture,
                sprites_2d: Arc::from([]),
                lights_2d: Arc::from([]),
                point_particles_2d: Arc::from([]),
                waters_2d: Arc::from([]),
                draws_3d: Arc::from([]),
                lighting_3d: CameraStreamLighting3DState::default(),
                point_particles_3d: Arc::from([]),
                waters_3d: Arc::from([]),
            });
        }
        // build node-id list once; collectors below share it via index access
        // instead of each re-collecting the whole arena.
        self.camera_stream_node_scratch.clear();
        self.camera_stream_node_scratch
            .extend(self.nodes.iter().map(|(id, _)| id));
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
            CameraStreamSourceState::Webcam { .. } => (
                Arc::from([]),
                Arc::from([]),
                Arc::from([]),
                Arc::from([]),
                Arc::from([]),
                CameraStreamLighting3DState::default(),
                Arc::from([]),
                Arc::from([]),
            ),
        };
        let output_texture = match &source {
            CameraStreamSourceState::Webcam { texture, .. } => *texture,
            _ => Self::camera_stream_texture_id(stream_node),
        };
        Some(CameraStreamState {
            source,
            overlay_camera_2d: None,
            clear_color: None,
            resolution: [
                stream.resolution.x.clamp(1, 8192),
                stream.resolution.y.clamp(1, 8192),
            ],
            aspect_ratio: stream.aspect_ratio.max(0.0),
            post_processing: Arc::from(post_processing),
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

    pub(crate) fn ui_viewport_state(
        &mut self,
        viewport_node: NodeID,
        viewport: &UiViewport,
        ui_size: [f32; 2],
    ) -> Option<CameraStreamState> {
        if !viewport.enabled {
            return None;
        }

        self.camera_stream_node_scratch.clear();
        if let Some(root) = self.nodes.get(viewport_node) {
            self.camera_stream_node_scratch
                .extend(root.children.iter().copied());
        }
        let mut cursor = 0usize;
        while cursor < self.camera_stream_node_scratch.len() {
            let node = self.camera_stream_node_scratch[cursor];
            cursor += 1;
            let Some(scene_node) = self.nodes.get(node) else {
                continue;
            };
            if matches!(scene_node.data, SceneNodeData::UiViewport(_)) {
                continue;
            }
            self.camera_stream_node_scratch
                .extend(scene_node.children.iter().copied());
        }

        let render_mask = BitMask::NONE;
        let source = CameraStreamSourceState::ThreeD(Camera3DState {
            position: [
                viewport.view_position.x,
                viewport.view_position.y,
                viewport.view_position.z,
            ],
            rotation: [
                viewport.view_rotation.x,
                viewport.view_rotation.y,
                viewport.view_rotation.z,
                viewport.view_rotation.w,
            ],
            projection: camera_stream_projection_state(&viewport.projection),
            render_mask,
            post_processing: Arc::from([]),
            audio_options: perro_structs::AudioListenerOptions::new(),
        });
        let overlay_camera_2d = Some(Camera2DState {
            position: [viewport.view_2d_position.x, viewport.view_2d_position.y],
            rotation_radians: viewport.view_2d_rotation,
            zoom: viewport.view_2d_zoom.max(0.001),
            render_mask,
            post_processing: Arc::from([]),
            audio_options: perro_structs::AudioListenerOptions::new(),
        });

        let resolution = [
            if viewport.resolution.x == 0 {
                ui_size[0].round().clamp(1.0, 8192.0) as u32
            } else {
                viewport.resolution.x.clamp(1, 8192)
            },
            if viewport.resolution.y == 0 {
                ui_size[1].round().clamp(1.0, 8192.0) as u32
            } else {
                viewport.resolution.y.clamp(1, 8192)
            },
        ];

        Some(CameraStreamState {
            source,
            overlay_camera_2d,
            clear_color: Some(viewport.background),
            resolution,
            aspect_ratio: viewport.aspect_ratio.max(0.0),
            post_processing: Arc::from(viewport.post_processing.to_effects_vec()),
            output_texture: Self::camera_stream_texture_id(viewport_node),
            sprites_2d: self.collect_camera_stream_sprites_2d(render_mask, viewport_node),
            lights_2d: self.collect_camera_stream_lights_2d(render_mask, viewport_node),
            point_particles_2d: self
                .collect_camera_stream_point_particles_2d(render_mask, viewport_node),
            waters_2d: self.collect_camera_stream_waters_2d(render_mask, viewport_node),
            draws_3d: self.collect_camera_stream_draws_3d(render_mask, viewport_node),
            lighting_3d: self.collect_camera_stream_lighting_3d(render_mask, viewport_node),
            point_particles_3d: self
                .collect_camera_stream_point_particles_3d(render_mask, viewport_node),
            waters_3d: self.collect_camera_stream_waters_3d(render_mask, viewport_node),
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
