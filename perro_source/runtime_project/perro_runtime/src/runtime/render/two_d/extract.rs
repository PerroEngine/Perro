use super::*;

impl Runtime {
    pub fn extract_render_2d_commands(&mut self) {
        let bootstrap_scan = self.render_2d.prev_visible.is_empty()
            && self.render_2d.retained_sprites.is_empty()
            && self.render_2d.last_camera.is_none();
        let button_input_changed = self.button_2d_input_changed();
        let has_extraction_work = self.dirty.has_any_dirty()
            || self.dirty.has_pending_transform_roots()
            || !self.render_2d.removed_nodes.is_empty()
            || self.render_2d.force_full_scan_once
            || button_input_changed
            || bootstrap_scan;
        if !has_extraction_work {
            return;
        }

        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();

        let active_camera = self.active_render_camera_2d();
        let camera_changed = self.render_2d.last_camera.as_ref() != active_camera.as_ref();
        let previous_camera_render_mask = self
            .render_2d
            .last_camera
            .as_ref()
            .map(|camera| camera.render_mask)
            .unwrap_or(BitMask::NONE);
        let camera_render_mask = active_camera
            .as_ref()
            .map(|camera| camera.render_mask)
            .unwrap_or(BitMask::NONE);
        let camera_render_mask_changed = previous_camera_render_mask != camera_render_mask;
        let hovered_button_2d = self.hovered_button_2d(active_camera.as_ref(), camera_render_mask);
        self.refresh_button_2d_visual_states(hovered_button_2d);

        if camera_changed {
            if let Some(camera) = &active_camera {
                self.resource_api.set_audio_listener_2d(
                    camera.position,
                    camera.rotation_radians,
                    camera.audio_options.clone(),
                );
                self.queue_render_command(RenderCommand::TwoD(Command2D::SetCamera {
                    camera: camera.clone(),
                }));
            } else {
                let camera = Camera2DState::default();
                self.resource_api.set_audio_listener_2d(
                    camera.position,
                    camera.rotation_radians,
                    camera.audio_options.clone(),
                );
                self.queue_render_command(RenderCommand::TwoD(Command2D::SetCamera { camera }));
            }
            self.render_2d.last_camera = active_camera.clone();
        }

        let nodes = &self.nodes;
        let mut dirty_ids = self.render_2d.take_dirty_ids_scratch();
        dirty_ids.clear();
        dirty_ids.extend(
            self.dirty
                .dirty_indices()
                .iter()
                .filter_map(|&raw_index| nodes.slot_get(raw_index as usize).map(|(id, _)| id)),
        );
        let traversal_ids = self.render_2d.collect_traversal_with_scratch(
            &dirty_ids,
            nodes.iter().map(|(id, _)| id),
            bootstrap_scan || camera_render_mask_changed || button_input_changed,
            |node, out| {
                if let Some(node_ref) = nodes.get(node) {
                    out.extend(node_ref.get_children_ids().iter().copied());
                }
            },
        );
        self.render_2d.restore_dirty_ids_scratch(dirty_ids);

        let mut visible_now = self.render_2d.begin_visible_pass();

        for node in traversal_ids.iter().copied() {
            visible_now.remove(&node);
            let effective_visible =
                self.is_effectively_visible(node) && !self.is_under_sub_view(node);
            let sprite_data = self
                .nodes
                .get(node)
                .and_then(|scene_node| match &scene_node.data {
                    SceneNodeData::Sprite2D(sprite) => Some((
                        effective_visible
                            && sprite.visible
                            && render_mask_matches(camera_render_mask, sprite.render_layers),
                        sprite.texture,
                        sprite.texture_region,
                        sprite.flip_x,
                        sprite.flip_y,
                        sprite.transform,
                        sprite.z_index,
                        self.effective_self_modulate(node),
                        None,
                    )),
                    SceneNodeData::AnimatedSprite2D(sprite) => Some((
                        effective_visible
                            && sprite.visible
                            && render_mask_matches(camera_render_mask, sprite.render_layers),
                        sprite.texture,
                        sprite.current_texture_region(),
                        sprite.flip_x,
                        sprite.flip_y,
                        sprite.transform,
                        sprite.z_index,
                        self.effective_self_modulate(node),
                        None,
                    )),
                    SceneNodeData::VideoPlayer2D(video) => Some((
                        effective_visible
                            && video.visible
                            && render_mask_matches(camera_render_mask, video.render_layers),
                        video.video.texture,
                        None,
                        video.flip_x,
                        video.flip_y,
                        video.transform,
                        video.z_index,
                        Runtime::color_modulate(video.tint, self.effective_self_modulate(node)),
                        Some([video.size.x, video.size.y]),
                    )),
                    SceneNodeData::ImageButton2D(button) => Some((
                        effective_visible
                            && button.visible
                            && render_mask_matches(camera_render_mask, button.render_layers),
                        button.texture,
                        button.texture_region,
                        false,
                        false,
                        button.transform,
                        button.z_index,
                        Runtime::color_modulate(
                            image_button_2d_tint(
                                button,
                                self.render_ui
                                    .button_states
                                    .get(&node)
                                    .copied()
                                    .unwrap_or_default(),
                            ),
                            self.effective_self_modulate(node),
                        ),
                        Some([button.size.x, button.size.y]),
                    )),
                    _ => None,
                });
            if let Some((
                visible,
                texture,
                texture_region,
                flip_x,
                flip_y,
                local_transform,
                z_index,
                tint,
                size_override,
            )) = sprite_data
            {
                let model = self
                    .get_render_global_transform_2d(node)
                    .unwrap_or(local_transform)
                    .to_mat3()
                    .to_cols_array_2d();
                self.emit_sprite_2d(
                    node,
                    visible,
                    Sprite2DEmit {
                        texture,
                        texture_region,
                        flip_x,
                        flip_y,
                        model,
                        tint,
                        size_override,
                        z_index,
                    },
                    &mut visible_now,
                );
            }

            let label_data = self
                .nodes
                .get(node)
                .and_then(|scene_node| match &scene_node.data {
                    SceneNodeData::Label2D(label) => Some((
                        effective_visible
                            && label.visible
                            && render_mask_matches(camera_render_mask, label.render_layers),
                        label.transform,
                        label.size,
                        label.text.clone(),
                        label.color,
                        label.font_size,
                        label.font.clone(),
                        label.h_align,
                        label.v_align,
                        label.z_index,
                        self.effective_self_modulate(node),
                    )),
                    _ => None,
                });
            if let Some((
                visible,
                local_transform,
                size,
                text,
                color,
                font_size,
                font,
                h_align,
                v_align,
                z_index,
                modulate,
            )) = label_data
            {
                if visible {
                    let transform = self
                        .get_render_global_transform_2d(node)
                        .unwrap_or(local_transform);
                    let rect = label_2d_rect(
                        transform,
                        size,
                        active_camera.as_ref(),
                        self.input.viewport_size(),
                        z_index,
                    );
                    self.queue_render_command(RenderCommand::Ui(UiCommand::UpsertLabel {
                        node,
                        rect,
                        clip_rect: viewport_clip(self.input.viewport_size()),
                        text: std::sync::Arc::from(text.as_ref()),
                        color: Runtime::color_modulate(color, modulate),
                        font_size: (font_size * transform.scale.y.abs()).max(0.001),
                        font,
                        wrap_width: None,
                        h_align: text_align_state_2d(h_align),
                        v_align: text_align_state_2d(v_align),
                        backdrop_color: perro_structs::Color::TRANSPARENT,
                        corner_radii: Default::default(),
                        padding: [0.0; 4],
                        projected_quad: None,
                        depth_test: false,
                        fit_content: false,
                    }));
                    visible_now.insert(node);
                } else {
                    self.queue_render_command(RenderCommand::Ui(UiCommand::RemoveNode { node }));
                }
            }

            let button_2d_data =
                self.nodes
                    .get(node)
                    .and_then(|scene_node| match &scene_node.data {
                        SceneNodeData::Button2D(button) => Some((
                            effective_visible
                                && button.visible
                                && render_mask_matches(camera_render_mask, button.render_layers),
                            button.transform,
                            button.size,
                            button.z_index,
                            button_2d_style(
                                button,
                                self.render_ui
                                    .button_states
                                    .get(&node)
                                    .copied()
                                    .unwrap_or_default(),
                            )
                            .fill,
                        )),
                        _ => None,
                    });
            if let Some((visible, local_transform, size, z_index, color)) = button_2d_data
                && visible
            {
                let color = Runtime::color_modulate(color, self.effective_self_modulate(node));
                let transform = self
                    .get_render_global_transform_2d(node)
                    .unwrap_or(local_transform);
                self.queue_render_command(RenderCommand::TwoD(Command2D::UpsertRect {
                    node,
                    rect: Rect2DCommand {
                        center: [transform.position.x, transform.position.y],
                        size: [
                            size.x * transform.scale.x.abs(),
                            size.y * transform.scale.y.abs(),
                        ],
                        color,
                        z_index,
                    },
                }));
                visible_now.insert(node);
            }

            let stream_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::CameraStream2D(stream) => Some((
                    effective_visible
                        && stream.visible
                        && stream.stream.enabled
                        && render_mask_matches(camera_render_mask, stream.render_layers),
                    stream.stream.clone(),
                    stream.transform,
                    stream.z_index,
                    stream.tint,
                )),
                _ => None,
            });
            if let Some((visible, stream, local_transform, z_index, tint)) = stream_data {
                if visible {
                    if let Some(stream_state) = self.camera_stream_state(node, &stream) {
                        let tint =
                            Runtime::color_modulate(tint, self.effective_self_modulate(node));
                        let aspect =
                            camera_stream_aspect_ratio(stream.aspect_ratio, stream.resolution);
                        let texture_resolution = match &stream_state.source {
                            CameraStreamSourceState::Webcam { resolution, .. } => *resolution,
                            _ => stream_state.resolution,
                        };
                        let model = self
                            .get_render_global_transform_2d(node)
                            .unwrap_or(local_transform)
                            .to_mat3()
                            .to_cols_array_2d();
                        let sprite = Sprite2DCommand {
                            texture: stream_state.output_texture,
                            model,
                            tint,
                            uv_min: [0.0, 0.0],
                            uv_max: [texture_resolution[0] as f32, texture_resolution[1] as f32],
                            size: [aspect, 1.0],
                            z_index,
                        };
                        self.queue_render_command(RenderCommand::CameraStream(
                            CameraStreamCommand::Upsert {
                                node,
                                state: Box::new(stream_state.clone()),
                            },
                        ));
                        self.queue_render_command(RenderCommand::TwoD(
                            Command2D::UpsertCameraStream {
                                node,
                                stream: Box::new(stream_state),
                                sprite,
                            },
                        ));
                        self.render_2d.retained_sprites.insert(node, sprite);
                        visible_now.insert(node);
                    } else {
                        self.queue_render_command(RenderCommand::CameraStream(
                            CameraStreamCommand::RemoveNode { node },
                        ));
                        self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode {
                            node,
                        }));
                        self.render_2d.retained_sprites.remove(&node);
                    }
                } else {
                    self.queue_render_command(RenderCommand::CameraStream(
                        CameraStreamCommand::RemoveNode { node },
                    ));
                    self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
                    self.render_2d.retained_sprites.remove(&node);
                }
            }

            let nine_slice_data =
                self.nodes
                    .get(node)
                    .and_then(|scene_node| match &scene_node.data {
                        SceneNodeData::NineSlice2D(nine) => Some((
                            effective_visible
                                && nine.visible
                                && render_mask_matches(camera_render_mask, nine.render_layers),
                            nine.texture,
                            nine.texture_region,
                            nine.transform,
                            nine.size,
                            nine.margins,
                            nine.tint,
                            nine.z_index,
                        )),
                        SceneNodeData::NineSliceButton2D(button) => Some((
                            effective_visible
                                && button.visible
                                && render_mask_matches(camera_render_mask, button.render_layers),
                            button.texture,
                            button.texture_region,
                            button.transform,
                            button.size,
                            button.margins,
                            nine_slice_button_2d_tint(
                                button,
                                self.render_ui
                                    .button_states
                                    .get(&node)
                                    .copied()
                                    .unwrap_or_default(),
                            ),
                            button.z_index,
                        )),
                        _ => None,
                    });
            if let Some((visible, texture, region, local_transform, size, margins, tint, z_index)) =
                nine_slice_data
                && visible
                && let Some(texture) = self.resolve_sprite_texture(node, texture)
            {
                let tint = Runtime::color_modulate(tint, self.effective_self_modulate(node));
                let model = self
                    .get_render_global_transform_2d(node)
                    .unwrap_or(local_transform)
                    .to_mat3()
                    .to_cols_array_2d();
                let sprites =
                    build_nine_slice_sprites(texture, region, model, size, margins, tint, z_index);
                self.queue_render_command(RenderCommand::TwoD(Command2D::UpsertTileMap {
                    node,
                    tilemap: TileMap2DCommand {
                        texture,
                        sprites: Arc::from(sprites),
                        shadow_casters: Arc::from([]),
                    },
                }));
                visible_now.insert(node);
            }

            let point_emitter_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::ParticleEmitter2D(emitter) => Some((
                    effective_visible
                        && emitter.visible
                        && render_mask_matches(camera_render_mask, emitter.render_layers),
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
                )),
                _ => None,
            });
            if let Some((
                visible,
                emitter_profile,
                emitter_sim_mode,
                emitter_transform,
                emitter_z_index,
                emitter_active,
                emitter_looping,
                emitter_prewarm,
                emitter_spawn_rate,
                emitter_seed,
                emitter_params,
                emitter_simulation_time,
            )) = point_emitter_data
            {
                if visible {
                    let profile =
                        resolve_particle_profile_2d(self, &emitter_profile).unwrap_or_default();
                    let modulate = self.effective_self_modulate(node);
                    let lifetime_min = profile.lifetime_min.max(0.001);
                    let lifetime_max = profile.lifetime_max.max(lifetime_min);
                    if let Some(node_mut) = self.nodes.get_mut_untracked(node)
                        && let SceneNodeData::ParticleEmitter2D(emitter_mut) = &mut node_mut.data
                    {
                        emitter_mut.internal_lifetime_max = lifetime_max;
                    }
                    let model = self
                        .get_render_global_transform_2d(node)
                        .unwrap_or(emitter_transform)
                        .to_mat3()
                        .to_cols_array_2d();
                    self.queue_render_command(RenderCommand::TwoD(
                        Command2D::UpsertPointParticles {
                            node,
                            particles: Box::new(PointParticles2DState {
                                model,
                                z_index: emitter_z_index,
                                active: emitter_active,
                                looping: emitter_looping,
                                prewarm: emitter_prewarm,
                                alive_budget: derived_particle_budget(
                                    emitter_spawn_rate.max(0.0),
                                    lifetime_max,
                                ),
                                emission_rate: emitter_spawn_rate.max(0.0),
                                lifetime_min,
                                lifetime_max,
                                speed_min: profile.speed_min.max(0.0),
                                speed_max: profile.speed_max.max(profile.speed_min.max(0.0)),
                                spread_radians: profile
                                    .spread_radians
                                    .clamp(0.0, std::f32::consts::TAU),
                                size: profile.size.max(1.0),
                                size_min: profile.size_min.max(0.01),
                                size_max: profile.size_max.max(profile.size_min.max(0.01)),
                                force: profile.force,
                                color_start: Runtime::color_modulate(profile.color_start, modulate),
                                color_end: Runtime::color_modulate(profile.color_end, modulate),
                                seed: emitter_seed,
                                params: emitter_params,
                                simulation_time: emitter_simulation_time,
                                simulation_delta: 0.0,
                                profile,
                                sim_mode: resolve_particle_sim_mode_2d(emitter_sim_mode),
                            }),
                        },
                    ));
                    visible_now.insert(node);
                } else {
                    self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
                    self.render_2d.retained_sprites.remove(&node);
                }
            }

            let sub_view_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::SubView2D(view) => Some((
                    effective_visible
                        && view.visible
                        && view.sub_view.enabled
                        && render_mask_matches(camera_render_mask, view.render_layers),
                    view.sub_view.clone(),
                    view.transform,
                    view.size,
                    view.z_index,
                    view.tint,
                )),
                _ => None,
            });
            if let Some((visible, view, local_transform, size, z_index, tint)) = sub_view_data {
                if visible {
                    if let Some(stream_state) = self.sub_view_state(node, &view, None) {
                        let tint =
                            Runtime::color_modulate(tint, self.effective_self_modulate(node));
                        let texture_resolution = stream_state.resolution;
                        let model = self
                            .get_render_global_transform_2d(node)
                            .unwrap_or(local_transform)
                            .to_mat3()
                            .to_cols_array_2d();
                        let sprite = Sprite2DCommand {
                            texture: stream_state.output_texture,
                            model,
                            tint,
                            uv_min: [0.0, 0.0],
                            uv_max: [texture_resolution[0] as f32, texture_resolution[1] as f32],
                            size: [size.x.max(0.001), size.y.max(0.001)],
                            z_index,
                        };
                        self.queue_render_command(RenderCommand::CameraStream(
                            CameraStreamCommand::Upsert {
                                node,
                                state: Box::new(stream_state.clone()),
                            },
                        ));
                        self.queue_render_command(RenderCommand::TwoD(
                            Command2D::UpsertCameraStream {
                                node,
                                stream: Box::new(stream_state),
                                sprite,
                            },
                        ));
                        self.render_2d.retained_sprites.insert(node, sprite);
                        visible_now.insert(node);
                    } else {
                        self.queue_render_command(RenderCommand::CameraStream(
                            CameraStreamCommand::RemoveNode { node },
                        ));
                        self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode {
                            node,
                        }));
                        self.render_2d.retained_sprites.remove(&node);
                    }
                } else {
                    self.queue_render_command(RenderCommand::CameraStream(
                        CameraStreamCommand::RemoveNode { node },
                    ));
                    self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
                    self.render_2d.retained_sprites.remove(&node);
                }
            }

            let water_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::WaterBody2D(water) => Some((
                    effective_visible
                        && water.visible
                        && render_mask_matches(camera_render_mask, water.render_layers),
                    water.transform,
                    water.z_index,
                    water.water,
                )),
                _ => None,
            });
            if let Some((visible, local_transform, z_index, water)) = water_data {
                if visible {
                    // resolve water global transform once; reused for model +
                    // coastline + impacts (was recomputed 3x).
                    let water_global = self.get_render_global_transform_2d(node);
                    let model = water_global
                        .unwrap_or(local_transform)
                        .to_mat3()
                        .to_cols_array_2d();
                    let coastline_shapes =
                        self.collect_water_coastline_shapes_2d(&water, water_global);
                    let queries = self.collect_water_queries_2d(node);
                    let impacts = self.collect_water_impacts_2d(node, &water, water_global);
                    let links = self.collect_water_links_2d(node, &water);
                    let modulate = self.effective_self_modulate(node);
                    self.queue_render_command(RenderCommand::TwoD(Command2D::UpsertWater {
                        node,
                        water: Box::new(Water2DState {
                            model,
                            z_index,
                            paused: self.physics_paused(),
                            simulation_time: self.time.elapsed,
                            simulation_delta: self.time.delta.max(0.0),
                            size: water_render_size(water),
                            shape: water_shape_state(water.shape),
                            resolution: water.resolution,
                            render_resolution: water.render_resolution,
                            depth: water.depth,
                            flow: [water.flow.x, water.flow.y],
                            wind: [water.wind.x, water.wind.y],
                            idle_mode: water_idle_mode_state(water.idle_mode),
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
                            deep_color: Runtime::color_modulate(water.optics.deep_color, modulate),
                            shallow_color: Runtime::color_modulate(
                                water.optics.shallow_color,
                                modulate,
                            ),
                            shallow_depth: water.optics.shallow_depth,
                            sky_bias_ratio: water.optics.sky_bias.ratio(),
                            transparency: water.visual.transparency,
                            reflectivity: water.visual.reflectivity,
                            roughness: water.visual.roughness,
                            fresnel_power: water.visual.fresnel_power,
                            normal_strength: water.visual.normal_strength,
                            ripple_scale: water.visual.ripple_scale,
                            foam_color: Runtime::color_modulate(water.visual.foam_color, modulate),
                            foam_amount: water.visual.foam_amount,
                            crest_foam_threshold: water.visual.crest_foam_threshold,
                            caustic_strength: water.visual.caustic_strength,
                            refraction_strength: water.visual.refraction_strength,
                            scattering_strength: water.visual.scattering_strength,
                            distance_fog_strength: water.visual.distance_fog_strength,
                            coastline_foam_color: Runtime::color_modulate(
                                water.coastline.foam_color,
                                modulate,
                            ),
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
                        }),
                    }));
                    visible_now.insert(node);
                } else {
                    self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
                }
            }

            let ambient_light_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::AmbientLight2D(light)
                    if light.visible
                        && light.active
                        && effective_visible
                        && render_mask_matches(camera_render_mask, light.render_layers) =>
                {
                    Some((light.color, light.intensity))
                }
                _ => None,
            });
            if let Some((color, intensity)) = ambient_light_data {
                if intensity > 0.0 {
                    let color =
                        Runtime::color_modulate_rgb(color, self.effective_self_modulate(node));
                    self.queue_render_command(RenderCommand::TwoD(Command2D::SetAmbientLight {
                        node,
                        light: AmbientLight2DState {
                            color,
                            intensity: intensity.max(0.0),
                        },
                    }));
                    visible_now.insert(node);
                } else {
                    self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
                }
            }

            let ray_light_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::RayLight2D(light)
                    if effective_visible
                        && light.visible
                        && light.active
                        && render_mask_matches(camera_render_mask, light.render_layers) =>
                {
                    Some((
                        light.transform,
                        light.z_index,
                        light.color,
                        light.intensity,
                        light.cast_shadows,
                        light.shadow_softness,
                        light.shadow_samples,
                    ))
                }
                _ => None,
            });
            if let Some((
                local_transform,
                z_index,
                color,
                intensity,
                cast_shadows,
                shadow_softness,
                shadow_samples,
            )) = ray_light_data
            {
                if intensity > 0.0 {
                    let color =
                        Runtime::color_modulate_rgb(color, self.effective_self_modulate(node));
                    let global = self
                        .get_render_global_transform_2d(node)
                        .unwrap_or(local_transform);
                    self.queue_render_command(RenderCommand::TwoD(Command2D::SetRayLight {
                        node,
                        light: RayLight2DState {
                            direction: direction_from_rotation_2d(global.rotation),
                            color,
                            intensity: intensity.max(0.0),
                            z_index,
                            cast_shadows,
                            shadow_softness: shadow_softness_2d(shadow_softness),
                            shadow_samples: shadow_samples.clamp(1, 16),
                        },
                    }));
                    visible_now.insert(node);
                } else {
                    self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
                }
            }

            let point_light_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::PointLight2D(light) => Some((
                    effective_visible
                        && light.visible
                        && light.active
                        && render_mask_matches(camera_render_mask, light.render_layers),
                    light.transform,
                    light.z_index,
                    light.color,
                    light.intensity,
                    light.range,
                    light.cast_shadows,
                    light.shadow_softness,
                    light.shadow_samples,
                )),
                _ => None,
            });
            if let Some((
                visible,
                local_transform,
                z_index,
                color,
                intensity,
                range,
                cast_shadows,
                shadow_softness,
                shadow_samples,
            )) = point_light_data
            {
                if visible && intensity > 0.0 && range > 0.0 {
                    let color =
                        Runtime::color_modulate_rgb(color, self.effective_self_modulate(node));
                    let global = self
                        .get_render_global_transform_2d(node)
                        .unwrap_or(local_transform);
                    self.queue_render_command(RenderCommand::TwoD(Command2D::SetPointLight {
                        node,
                        light: PointLight2DState {
                            position: [global.position.x, global.position.y],
                            color,
                            intensity,
                            range,
                            z_index,
                            cast_shadows,
                            shadow_softness: shadow_softness_2d(shadow_softness),
                            shadow_samples: shadow_samples.clamp(1, 16),
                        },
                    }));
                    visible_now.insert(node);
                } else {
                    self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
                }
            }

            let spot_light_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::SpotLight2D(light)
                    if effective_visible
                        && light.visible
                        && light.active
                        && render_mask_matches(camera_render_mask, light.render_layers) =>
                {
                    Some((
                        light.transform,
                        light.z_index,
                        light.color,
                        light.intensity,
                        light.range,
                        light.inner_angle_radians,
                        light.outer_angle_radians,
                        light.cast_shadows,
                        light.shadow_softness,
                        light.shadow_samples,
                    ))
                }
                _ => None,
            });
            if let Some((
                local_transform,
                z_index,
                color,
                intensity,
                range,
                inner_angle_radians,
                outer_angle_radians,
                cast_shadows,
                shadow_softness,
                shadow_samples,
            )) = spot_light_data
            {
                if intensity > 0.0 && range > 0.0 {
                    let color =
                        Runtime::color_modulate_rgb(color, self.effective_self_modulate(node));
                    let global = self
                        .get_render_global_transform_2d(node)
                        .unwrap_or(local_transform);
                    self.queue_render_command(RenderCommand::TwoD(Command2D::SetSpotLight {
                        node,
                        light: SpotLight2DState {
                            position: [global.position.x, global.position.y],
                            direction: direction_from_rotation_2d(global.rotation),
                            color,
                            intensity: intensity.max(0.0),
                            range: range.max(0.001),
                            inner_angle_radians: inner_angle_radians.max(0.0),
                            outer_angle_radians: outer_angle_radians.max(inner_angle_radians),
                            z_index,
                            cast_shadows,
                            shadow_softness: shadow_softness_2d(shadow_softness),
                            shadow_samples: shadow_samples.clamp(1, 16),
                        },
                    }));
                    visible_now.insert(node);
                } else {
                    self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
                }
            }

            let shadow_caster_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::CollisionShape2D(shape) => Some((
                    effective_visible
                        && shape.visible
                        && render_mask_matches(camera_render_mask, shape.render_layers),
                    shape.transform,
                    shape.z_index,
                    shape.shape,
                )),
                _ => None,
            });
            if let Some((visible, local_transform, z_index, shape)) = shadow_caster_data {
                if visible {
                    let global = self
                        .get_render_global_transform_2d(node)
                        .unwrap_or(local_transform);
                    if let Some(caster) = shadow_caster_2d_state(global, z_index, shape) {
                        self.queue_render_command(RenderCommand::TwoD(
                            Command2D::UpsertShadowCaster { node, caster },
                        ));
                        visible_now.insert(node);
                    } else {
                        self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode {
                            node,
                        }));
                    }
                } else {
                    self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
                }
            }

            let tilemap_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::TileMap2D(tilemap) => Some((
                    effective_visible
                        && tilemap.visible
                        && render_mask_matches(camera_render_mask, tilemap.render_layers),
                    tilemap.clone(),
                )),
                _ => None,
            });
            if let Some((visible, tilemap)) = tilemap_data {
                if visible {
                    if let Some(tileset) = resolve_tileset_2d(self, &tilemap.tileset)
                        && let Some(texture) =
                            self.resolve_tilemap_texture(node, tileset.texture.as_ref())
                    {
                        let global_transform = self
                            .get_render_global_transform_2d(node)
                            .unwrap_or(tilemap.transform);
                        let global = global_transform.to_mat3().to_cols_array_2d();
                        let sprites = build_tilemap_sprites(TilemapSpriteBuild {
                            texture,
                            base_model: global,
                            z_index: tilemap.z_index,
                            width: tilemap.width,
                            height: tilemap.height,
                            empty_tile: tilemap.empty_tile,
                            tint: self.effective_self_modulate(node),
                            tiles: &tilemap.tiles,
                            tileset: &tileset,
                        });
                        let shadow_casters = if tilemap.collision_enabled {
                            build_tilemap_shadow_casters(&tilemap, global_transform, &tileset)
                        } else {
                            Vec::new()
                        };
                        self.queue_render_command(RenderCommand::TwoD(Command2D::UpsertTileMap {
                            node,
                            tilemap: TileMap2DCommand {
                                texture,
                                sprites: Arc::from(sprites),
                                shadow_casters: Arc::from(shadow_casters),
                            },
                        }));
                        visible_now.insert(node);
                    }
                } else {
                    self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
                    self.render_2d.retained_sprites.remove(&node);
                }
            }
        }
        for node in self.render_2d.collect_removed_visible_nodes(&visible_now) {
            if self.node_2d_has_pending_visual_asset(node) {
                visible_now.insert(node);
                continue;
            }
            self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
            self.queue_render_command(RenderCommand::Ui(UiCommand::RemoveNode { node }));
            self.render_2d.retained_sprites.remove(&node);
        }
        self.render_2d
            .finish_visible_pass(traversal_ids, visible_now);
    }

    pub(super) fn active_render_camera_2d(&mut self) -> Option<Camera2DState> {
        let mut found = None;
        for (node, scene_node) in self.nodes.iter() {
            let SceneNodeData::Camera2D(camera) = &scene_node.data else {
                continue;
            };
            if !camera.active || !self.is_effectively_visible(node) || self.is_under_sub_view(node)
            {
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
        let global = self
            .get_render_global_transform_2d(node)
            .unwrap_or(local_transform);
        Some(Camera2DState {
            position: [global.position.x, global.position.y],
            rotation_radians: global.rotation,
            zoom,
            render_mask,
            post_processing: Arc::from(post_processing.to_effects_vec()),
            audio_options,
        })
    }

    pub(super) fn emit_sprite_2d(
        &mut self,
        node: NodeID,
        visible: bool,
        emit: Sprite2DEmit,
        visible_now: &mut AHashSet<NodeID>,
    ) {
        if !visible {
            return;
        }

        let Some(resolved_texture) = self.resolve_sprite_texture(node, emit.texture) else {
            return;
        };

        let (mut uv_min, mut uv_max, region_size) = sprite_region_uv(emit.texture_region);
        if emit.flip_x {
            std::mem::swap(&mut uv_min[0], &mut uv_max[0]);
        }
        if emit.flip_y {
            std::mem::swap(&mut uv_min[1], &mut uv_max[1]);
        }
        let sprite = Sprite2DCommand {
            texture: resolved_texture,
            model: emit.model,
            tint: emit.tint,
            uv_min,
            uv_max,
            size: emit.size_override.unwrap_or(region_size),
            z_index: emit.z_index,
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
}
