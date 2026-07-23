use super::*;

impl Gpu {
    pub fn render(&mut self, frame: RenderFrame<'_>) -> RenderGpuTiming {
        let total_start = Instant::now();
        let mut timing = RenderGpuTiming::default();
        if let Some(timer) = self.gpu_timer.as_mut() {
            timer.poll(&self.device);
            timing.gpu_timestamp_main = timer.last_main;
            timing.gpu_timestamp_water = timer.last_water;
        }
        let RenderFrame {
            resources,
            camera_3d,
            lighting_3d,
            draws_3d,
            draws_3d_revision,
            point_particles_3d,
            point_particles_3d_revision,
            waters_3d,
            waters_3d_revision,
            decals_3d,
            decals_3d_revision,
            camera_streams,
            camera_2d,
            camera_2d_position,
            post_processing_2d,
            post_processing_global,
            accessibility,
            rects_2d,
            upload_2d,
            sprites_2d,
            sprites_2d_revision,
            point_lights_2d,
            point_lights_2d_revision,
            shadow_casters_2d,
            waters_2d,
            waters_2d_revision,
            late_overlay_camera_2d,
            late_overlay_rects_2d,
            late_overlay_upload_2d,
            late_overlay_sprites_2d,
            late_overlay_sprites_2d_revision,
            late_overlay_point_lights_2d,
            late_overlay_point_lights_2d_revision,
            late_overlay_shadow_casters_2d,
            redraw_requested,
            frame_time_seconds,
            frame_delta_seconds,
            frame_dirty_bits,
            static_texture_lookup,
            static_mesh_lookup,
            static_shader_lookup,
            ui_primitives,
            ui_primitive_depths,
            ui_textures_delta,
            ui_texture_size,
            ui_revision,
        } = frame;
        let rect_draw_count = upload_2d.draw_count as u32;
        // Keep window alive for the full surface lifetime.
        self.window_handle.id();

        let underwater_water = camera_underwater(&camera_3d, waters_3d);
        let post_requested = underwater_water.is_some()
            || PostProcessor::has_effects(camera_3d.post_processing.as_ref())
            || PostProcessor::has_effects(post_processing_2d.as_ref())
            || PostProcessor::has_effects(post_processing_global.as_ref());

        let has = |bit: u32| (frame_dirty_bits & bit) != 0;

        let has_2d_content = upload_2d.draw_count > 0
            || !sprites_2d.is_empty()
            || !point_lights_2d.is_empty()
            || !waters_2d.is_empty();
        let rect_upload_dirty = upload_2d.full_reupload || !upload_2d.dirty_ranges.is_empty();
        let needs_2d_prepare = has(DIRTY_2D)
            || has(DIRTY_CAMERA_2D)
            || rect_upload_dirty
            || (has(DIRTY_RESOURCES) && has_2d_content)
            || (redraw_requested && has_2d_content);

        // A decal whose texture is still decoding must be retried each frame
        // until it resolves; otherwise it stays hidden until the next dirty
        // frame forces a re-prepare (looked like "white until reload").
        let decals_texture_pending = self
            .three_d
            .as_ref()
            .is_some_and(|three_d| three_d.decals_pending());

        let three_d_content_changed = self.last_prepare_3d_camera.as_ref() != Some(&camera_3d)
            || self.last_prepare_3d_lighting.as_ref() != Some(lighting_3d)
            || self.last_prepare_3d_draws_revision != draws_3d_revision
            || self.last_prepare_3d_decals_revision != decals_3d_revision
            || decals_texture_pending
            || self.last_prepare_3d_width != self.render_width
            || self.last_prepare_3d_height != self.render_height;

        let needs_3d = !draws_3d.is_empty();
        let needs_particles_3d = !point_particles_3d.is_empty();
        let needs_water = !waters_2d.is_empty() || !waters_3d.is_empty();

        let needs_3d_pipeline = has(DIRTY_3D)
            || has(DIRTY_CAMERA_3D)
            || has(DIRTY_LIGHTS_3D)
            || has(DIRTY_RESOURCES)
            || needs_3d
            || needs_particles_3d
            || needs_water
            || post_requested
            || three_d_content_changed;

        let needs_3d_prepare = has(DIRTY_3D)
            || has(DIRTY_CAMERA_3D)
            || has(DIRTY_LIGHTS_3D)
            || has(DIRTY_RESOURCES)
            || three_d_content_changed;

        let needs_3d_particles_path = has(DIRTY_PARTICLES_3D) || needs_particles_3d;
        let needs_3d_particles_prepare = needs_3d_particles_path
            && (has(DIRTY_PARTICLES_3D)
                || self.last_prepare_particles_revision != point_particles_3d_revision
                || three_d_content_changed);
        let needs_water_prepare = needs_water;

        if !camera_streams.is_empty() && self.two_d.is_none() {
            self.two_d = Some(Gpu2D::new(
                &self.device,
                self.render_format,
                self.sample_count,
                self.texture_filter,
            ));
        }
        self.camera_stream_content_revisions
            .retain(|node, _| camera_streams.iter().any(|(active, _)| active == node));
        for (node, stream) in camera_streams {
            if !camera_stream_uses_render_target(stream) {
                continue;
            }
            let resolution = [stream.resolution[0].max(1), stream.resolution[1].max(1)];
            let needs_external_binding =
                self.camera_stream_external_bindings.get(node).copied() != Some(resolution);
            let Some(target) = self.ensure_camera_stream_target(*node, resolution) else {
                continue;
            };
            if needs_external_binding {
                let texture_id = stream.output_texture;
                let view_2d = target
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let view_ui = target
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                if let Some(two_d) = self.two_d.as_mut() {
                    two_d.upsert_external_texture(
                        &self.device,
                        texture_id,
                        view_2d,
                        resolution[0],
                        resolution[1],
                    );
                }
                if self.ui.is_none() {
                    self.ui = Some(GpuUi::new(
                        &self.device,
                        self.surface_view_format,
                        self.texture_filter,
                    ));
                }
                if let Some(ui) = self.ui.as_mut() {
                    ui.upsert_external_image_texture(&self.device, texture_id, view_ui, resolution);
                }
                self.camera_stream_external_bindings
                    .insert(*node, resolution);
            }
        }

        let prepare_2d_start = Instant::now();
        let mut did_prepare_2d = false;
        if needs_2d_prepare {
            if self.two_d.is_none() {
                self.two_d = Some(Gpu2D::new(
                    &self.device,
                    self.render_format,
                    self.sample_count,
                    self.texture_filter,
                ));
            }
            if let Some(two_d) = self.two_d.as_mut() {
                two_d.prepare(
                    &self.device,
                    &self.queue,
                    Prepare2D {
                        resources,
                        camera: camera_2d,
                        rects: rects_2d,
                        upload: upload_2d,
                        sprites: sprites_2d,
                        sprites_revision: sprites_2d_revision,
                        force_sprite_prepare: has(DIRTY_RESOURCES),
                        point_lights: point_lights_2d,
                        point_lights_revision: point_lights_2d_revision,
                        shadow_casters: shadow_casters_2d,
                        static_texture_lookup,
                    },
                );
                did_prepare_2d = true;
            }
        }
        if !did_prepare_2d {
            timing.skip_prepare_2d = 1;
        }
        if let Some(two_d) = self.two_d.as_ref() {
            timing.sprite_batches_2d = two_d.sprite_batch_count();
            timing.sprite_bind_group_switches_2d = two_d.sprite_bind_group_switch_count();
        }
        timing.prepare_2d = prepare_2d_start.elapsed();

        if needs_water_prepare {
            if self.three_d.is_none() {
                self.three_d = Some(Gpu3D::new(
                    &self.device,
                    &self.queue,
                    self.render_format,
                    Gpu3DConfig {
                        sample_count: self.sample_count,
                        width: self.render_width,
                        height: self.render_height,
                        meshlets_enabled: self.meshlets_enabled,
                        dev_meshlets: self.dev_meshlets,
                        meshlet_debug_view: self.meshlet_debug_view,
                        occlusion_culling: self.occlusion_culling,
                        ssao: self.ssao,
                        indirect_first_instance_enabled: self.indirect_first_instance_enabled,
                        multi_draw_indirect_enabled: self.multi_draw_indirect_enabled,
                        texture_filter: self.texture_filter,
                    },
                ));
            }
            if self.water.is_none() {
                let Some(two_d) = self.two_d.as_ref() else {
                    return timing;
                };
                let Some(three_d) = self.three_d.as_ref() else {
                    return timing;
                };
                self.water = Some(GpuWater::new(
                    &self.device,
                    self.render_format,
                    self.sample_count,
                    two_d.camera_bind_group_layout(),
                    three_d.water_camera_bind_group_layout(),
                    three_d.depth_prepass_view(),
                    self.render_width,
                    self.render_height,
                ));
            }
            if let Some(water) = self.water.as_mut() {
                let sky_color = sky_clear_color(lighting_3d)
                    .map(|color| [color.r as f32, color.g as f32, color.b as f32])
                    .unwrap_or([0.0, 0.0, 0.0]);
                let water_view_proj =
                    water_camera_view_proj(&camera_3d, self.render_width, self.render_height);
                water.prepare(
                    &self.device,
                    &self.queue,
                    waters_2d,
                    waters_3d,
                    WaterPrepareContext {
                        camera_2d_position,
                        camera_3d_position: camera_3d.position,
                        camera_3d_frustum_planes: water_extract_frustum_planes(water_view_proj),
                        sky_color,
                        time_seconds: frame_time_seconds,
                        delta_seconds: frame_delta_seconds,
                    },
                );
                self.last_prepare_water_2d_revision = waters_2d_revision;
                self.last_prepare_water_3d_revision = waters_3d_revision;
            }
        } else if !needs_water {
            if let Some(water) = self.water.as_mut() {
                water.clear_active();
            }
            self.last_prepare_water_2d_revision = u64::MAX;
            self.last_prepare_water_3d_revision = u64::MAX;
        }

        let prepare_3d_start = Instant::now();
        let mut did_prepare_3d = false;
        let mut prepare_3d_steps = Prepare3DStepTiming::default();
        if needs_3d_pipeline {
            if self.three_d.is_none() {
                self.three_d = Some(Gpu3D::new(
                    &self.device,
                    &self.queue,
                    self.render_format,
                    Gpu3DConfig {
                        sample_count: self.sample_count,
                        width: self.render_width,
                        height: self.render_height,
                        meshlets_enabled: self.meshlets_enabled,
                        dev_meshlets: self.dev_meshlets,
                        meshlet_debug_view: self.meshlet_debug_view,
                        occlusion_culling: self.occlusion_culling,
                        ssao: self.ssao,
                        indirect_first_instance_enabled: self.indirect_first_instance_enabled,
                        multi_draw_indirect_enabled: self.multi_draw_indirect_enabled,
                        texture_filter: self.texture_filter,
                    },
                ));
            }
            if needs_3d_particles_path && self.point_particles_3d.is_none() {
                self.point_particles_3d = Some(GpuPointParticles3D::new(
                    &self.device,
                    self.render_format,
                    self.sample_count,
                ));
            }
            if let Some(three_d) = self.three_d.as_mut()
                && needs_3d_prepare
            {
                for (node, stream) in camera_streams {
                    if !camera_stream_uses_render_target(stream) {
                        continue;
                    }
                    let resolution = [stream.resolution[0].max(1), stream.resolution[1].max(1)];
                    // Skip when the slot is already bound to the current target
                    // generation; `ensure_camera_stream_target` clears this entry
                    // whenever it recreates the target (resolution change).
                    if self.camera_stream_3d_bindings.get(node).copied() == Some(resolution) {
                        continue;
                    }
                    let Some(target) = self.camera_stream_targets.get(node) else {
                        continue;
                    };
                    let view = target
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    three_d.upsert_external_material_texture(
                        &self.device,
                        stream.output_texture.index(),
                        &view,
                        format!("__camera_stream__:{}", node.as_u64()),
                    );
                    self.camera_stream_3d_bindings.insert(*node, resolution);
                }
                three_d.prepare(
                    &self.device,
                    &self.queue,
                    Prepare3D {
                        resources,
                        camera: camera_3d.clone(),
                        lighting: lighting_3d,
                        draws: draws_3d,
                        draws_revision: draws_3d_revision,
                        force_full_rebuild: has(DIRTY_RESOURCES),
                        decals: decals_3d,
                        decals_revision: decals_3d_revision,
                        width: self.render_width,
                        height: self.render_height,
                        static_texture_lookup,
                        static_mesh_lookup,
                        static_shader_lookup,
                    },
                );
                did_prepare_3d = true;
                prepare_3d_steps = three_d.prepare_step_timing();
                self.last_prepare_3d_camera = Some(camera_3d.clone());
                self.last_prepare_3d_lighting = Some(lighting_3d.clone());
                self.last_prepare_3d_draws_revision = draws_3d_revision;
                self.last_prepare_3d_decals_revision = decals_3d_revision;
                self.last_prepare_3d_width = self.render_width;
                self.last_prepare_3d_height = self.render_height;
            }
            let prepare_particles_start = Instant::now();
            let mut did_prepare_particles_3d = false;
            if needs_3d_particles_prepare
                && let Some(point_particles_3d_gpu) = self.point_particles_3d.as_mut()
            {
                point_particles_3d_gpu.prepare(
                    &self.device,
                    &self.queue,
                    PreparePointParticles3D {
                        camera: camera_3d.clone(),
                        emitters: point_particles_3d,
                        width: self.render_width,
                        height: self.render_height,
                    },
                );
                self.last_prepare_particles_revision = point_particles_3d_revision;
                did_prepare_particles_3d = true;
            }
            timing.prepare_particles_3d = prepare_particles_start.elapsed();
            if !did_prepare_particles_3d {
                timing.skip_prepare_particles_3d = 1;
            }
        } else {
            timing.skip_prepare_particles_3d = 1;
        }
        if !did_prepare_3d {
            timing.skip_prepare_3d = 1;
            timing.skip_prepare_3d_frustum = 1;
            timing.skip_prepare_3d_hiz = 1;
            timing.skip_prepare_3d_indirect = 1;
            timing.skip_prepare_3d_cull_inputs = 1;
        } else {
            timing.prepare_3d_frustum = prepare_3d_steps.frustum_prep;
            timing.prepare_3d_hiz = prepare_3d_steps.hiz_prep;
            timing.prepare_3d_indirect = prepare_3d_steps.indirect_prep;
            timing.prepare_3d_cull_inputs = prepare_3d_steps.cull_input_prep;
            timing.skip_prepare_3d_frustum = prepare_3d_steps.frustum_skipped;
            timing.skip_prepare_3d_hiz = prepare_3d_steps.hiz_skipped;
            timing.skip_prepare_3d_indirect = prepare_3d_steps.indirect_skipped;
            timing.skip_prepare_3d_cull_inputs = prepare_3d_steps.cull_input_skipped;
        }
        if !needs_3d_particles_path {
            self.point_particles_3d = None;
            self.last_prepare_particles_revision = u64::MAX;
        }
        timing.prepare_3d = prepare_3d_start.elapsed();

        let (base_camera_post_chain, base_camera_post_enabled) =
            if PostProcessor::has_effects(camera_3d.post_processing.as_ref()) {
                (camera_3d.post_processing.as_ref(), true)
            } else if PostProcessor::has_effects(post_processing_2d.as_ref()) {
                (post_processing_2d.as_ref(), true)
            } else {
                (camera_3d.post_processing.as_ref(), false)
            };
        let mut underwater_post_chain = Vec::new();
        let (camera_post_chain, camera_post_enabled) = if let Some(water) = underwater_water {
            underwater_post_chain.reserve(base_camera_post_chain.len() + 3);
            underwater_post_chain.extend_from_slice(base_camera_post_chain);
            underwater_post_chain.extend(underwater_effects(water));
            (underwater_post_chain.as_slice(), true)
        } else {
            (base_camera_post_chain, base_camera_post_enabled)
        };
        let global_post_chain = post_processing_global.as_ref();
        let global_post_enabled = PostProcessor::has_effects(global_post_chain);
        let mut exposure_settings = PresentExposureSettings::default();
        exposure_settings.apply_effects(camera_post_chain);
        exposure_settings.apply_effects(global_post_chain);
        let accessibility_enabled = self.accessibility.has_settings(accessibility);
        // The seam pass needs a sampleable offscreen scene texture, so it
        // forces the non-direct path while active.
        let blend_screen_active = self
            .three_d
            .as_ref()
            .is_some_and(|three_d| three_d.screen_blend_active());
        // Final tonemap owns scene -> surface conversion.
        let msaa_direct_present = false;
        let direct_present = false;
        let depth_prepass_needed = !waters_3d.is_empty()
            || (camera_post_enabled && PostProcessor::uses_depth(camera_post_chain))
            || (global_post_enabled && PostProcessor::uses_depth(global_post_chain))
            || ui_primitive_depths.iter().any(Option::is_some);
        let mut frame = None;
        let mut swap_view = None;
        if direct_present || msaa_direct_present {
            let acquire_start = Instant::now();
            let acquire_surface_start = Instant::now();
            let acquired = match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    timing.acquire_surface = acquire_surface_start.elapsed();
                    self.surface.configure(&self.device, &self.config);
                    timing.acquire = acquire_start.elapsed();
                    timing.total = total_start.elapsed();
                    return timing;
                }
                wgpu::CurrentSurfaceTexture::Timeout
                | wgpu::CurrentSurfaceTexture::Occluded
                | wgpu::CurrentSurfaceTexture::Validation => {
                    timing.acquire_surface = acquire_surface_start.elapsed();
                    timing.acquire = acquire_start.elapsed();
                    timing.total = total_start.elapsed();
                    return timing;
                }
            };
            timing.acquire_surface = acquire_surface_start.elapsed();
            let acquire_view_start = Instant::now();
            let view = acquired.texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(self.surface_view_format),
                ..Default::default()
            });
            timing.acquire_view = acquire_view_start.elapsed();
            timing.acquire = acquire_start.elapsed();
            frame = Some(acquired);
            swap_view = Some(view);
        }
        let scene_view = self.post.scene_view().clone();
        let intermediate_view = self.accessibility.intermediate_view().clone();
        let color_view = if direct_present {
            let Some(view) = swap_view.as_ref() else {
                timing.total = total_start.elapsed();
                return timing;
            };
            view
        } else {
            self.msaa_color
                .as_ref()
                .map(|t| &t.view)
                .unwrap_or(&scene_view)
        };
        let resolve_view = if direct_present {
            None
        } else if msaa_direct_present {
            let Some(view) = swap_view.as_ref() else {
                timing.total = total_start.elapsed();
                return timing;
            };
            Some(view)
        } else if self.sample_count > 1 {
            Some(&scene_view)
        } else {
            None
        };

        let encode_start = Instant::now();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("perro_main_encoder"),
            });
        let gpu_timer_active = self
            .gpu_timer
            .as_ref()
            .is_some_and(GpuTimestampTimer::can_write);
        if gpu_timer_active && let Some(timer) = self.gpu_timer.as_ref() {
            timer.write_start(&mut encoder);
        }
        let clear_color = sky_clear_color(lighting_3d).unwrap_or(wgpu::Color {
            r: CLEAR_R,
            g: CLEAR_G,
            b: CLEAR_B,
            a: 1.0,
        });
        for (node, stream) in camera_streams {
            let (stream_draws_revision, stream_sprites_revision) =
                update_camera_stream_content_revisions(
                    &mut self.camera_stream_content_revisions,
                    *node,
                    &stream.draws_3d,
                    &stream.sprites_2d,
                );
            let has_stream_post = PostProcessor::has_effects(stream.post_processing.as_ref());
            // UI composites after the main present pass, so an engine-rendered
            // stream needs its own single scene-linear -> display conversion.
            let tone_map_stream = stream.tone_map_output
                && !matches!(stream.source, CameraStreamSourceState::Webcam { .. });
            let needs_intermediate = has_stream_post || tone_map_stream;
            let (
                target_view,
                post_input_view,
                tonemap_input_view,
                post_depth_view,
                post_view_key,
                render_texture,
            ) = {
                let Some(target) = self.camera_stream_targets.get(node) else {
                    continue;
                };
                (
                    target
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default()),
                    needs_intermediate.then(|| {
                        target
                            .post_input
                            .create_view(&wgpu::TextureViewDescriptor::default())
                    }),
                    tone_map_stream.then(|| {
                        target
                            .tonemap_input
                            .create_view(&wgpu::TextureViewDescriptor::default())
                    }),
                    has_stream_post.then(|| {
                        target
                            .depth
                            .create_view(&wgpu::TextureViewDescriptor::default())
                    }),
                    target.post_view_key,
                    if needs_intermediate {
                        target.post_input.clone()
                    } else {
                        target.texture.clone()
                    },
                )
            };
            let Some(render_view) = (if needs_intermediate {
                post_input_view.as_ref()
            } else {
                Some(&target_view)
            }) else {
                continue;
            };
            let mut stream_post_camera = None;
            let mut stream_post_depth_view = post_depth_view;
            if let CameraStreamSourceState::Webcam { texture, .. } = &stream.source {
                let source_view = self.camera_stream_2d.as_mut().and_then(|stream_2d| {
                    stream_2d.ensure_sampled_texture_view(
                        &self.device,
                        &self.queue,
                        resources,
                        *texture,
                        static_texture_lookup,
                    )
                });
                let (Some(source_view), Some(depth_view)) =
                    (source_view, stream_post_depth_view.as_ref())
                else {
                    continue;
                };
                let _clear_depth = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("perro_camera_stream_webcam_depth_clear"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                drop(_clear_depth);
                if self.camera_stream_post.is_none() {
                    self.camera_stream_post = Some(PostProcessor::new(
                        &self.device,
                        &self.queue,
                        self.render_format,
                        stream.resolution[0].max(1),
                        stream.resolution[1].max(1),
                    ));
                }
                if let Some(post) = self.camera_stream_post.as_mut() {
                    post.resize(
                        &self.device,
                        stream.resolution[0].max(1),
                        stream.resolution[1].max(1),
                    );
                    let camera = Camera3DState::default();
                    let post_context = PostProcessContext {
                        device: &self.device,
                        queue: &self.queue,
                        output_view: if tone_map_stream {
                            let Some(view) = tonemap_input_view.as_ref() else {
                                continue;
                            };
                            view
                        } else {
                            &target_view
                        },
                        camera: &camera,
                        external_input_view_key: post_view_key.wrapping_add(2),
                        depth_view_key: post_view_key.wrapping_add(1),
                        static_shader_lookup,
                        static_texture_lookup,
                    };
                    let post_chain_data = PostProcessChainData {
                        input_view: &source_view,
                        depth_view,
                        effects: stream.post_processing.as_ref(),
                    };
                    post.apply_chain(&post_context, &post_chain_data, &mut encoder);
                }
                continue;
            } else if let CameraStreamSourceState::TwoD(camera) = &stream.source {
                let stream_clear_color = stream
                    .clear_color
                    .map(premultiplied_clear_color)
                    .unwrap_or(if stream.transparent_background {
                        wgpu::Color::TRANSPARENT
                    } else {
                        wgpu::Color::BLACK
                    });
                let _clear_stream = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("perro_camera_stream_clear_2d"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: render_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(stream_clear_color),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                drop(_clear_stream);
                if has_stream_post {
                    let Some(depth_view) = stream_post_depth_view.as_ref() else {
                        continue;
                    };
                    let _clear_depth = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("perro_camera_stream_depth_clear_2d"),
                        color_attachments: &[],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: depth_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    drop(_clear_depth);
                }
                if self.camera_stream_2d.is_none() {
                    self.camera_stream_2d = Some(Gpu2D::new(
                        &self.device,
                        self.render_format,
                        1,
                        self.texture_filter,
                    ));
                }
                if let Some(stream_2d) = self.camera_stream_2d.as_mut() {
                    let camera_position = camera.position;
                    let camera = camera_2d_uniform_from_state(
                        camera,
                        stream.resolution[0],
                        stream.resolution[1],
                    );
                    let empty_upload = RectUploadPlan {
                        full_reupload: true,
                        dirty_ranges: Vec::new(),
                        draw_count: 0,
                    };
                    stream_2d.prepare(
                        &self.device,
                        &self.queue,
                        Prepare2D {
                            resources,
                            camera,
                            rects: &[],
                            upload: &empty_upload,
                            sprites: stream.sprites_2d.as_ref(),
                            sprites_revision: stream_sprites_revision,
                            force_sprite_prepare: has(DIRTY_RESOURCES),
                            point_lights: stream.lights_2d.as_ref(),
                            point_lights_revision: u64::MAX,
                            shadow_casters: &[],
                            static_texture_lookup,
                        },
                    );
                    let particle_rect_count = stream_2d.prepare_stream_point_particles(
                        &self.device,
                        &self.queue,
                        stream.point_particles_2d.as_ref(),
                    );
                    if !stream.waters_2d.is_empty() {
                        if self.camera_stream_3d.is_none() {
                            let mut stream_3d = Gpu3D::new(
                                &self.device,
                                &self.queue,
                                self.render_format,
                                Gpu3DConfig {
                                    sample_count: 1,
                                    width: stream.resolution[0].max(1),
                                    height: stream.resolution[1].max(1),
                                    meshlets_enabled: self.meshlets_enabled,
                                    dev_meshlets: self.dev_meshlets,
                                    meshlet_debug_view: self.meshlet_debug_view,
                                    occlusion_culling: self.occlusion_culling,
                                    ssao: self.ssao,
                                    indirect_first_instance_enabled: self
                                        .indirect_first_instance_enabled,
                                    multi_draw_indirect_enabled: self.multi_draw_indirect_enabled,
                                    texture_filter: self.texture_filter,
                                },
                            );
                            // Camera streams render into their own targets;
                            // the seam pass only wires up the main scene.
                            stream_3d.set_screen_blend_supported(false);
                            self.camera_stream_3d = Some(stream_3d);
                        }
                        if self.camera_stream_water.is_none()
                            && let Some(stream_3d_ref) = self.camera_stream_3d.as_ref()
                        {
                            self.camera_stream_water = Some(GpuWater::new(
                                &self.device,
                                self.render_format,
                                1,
                                stream_2d.camera_bind_group_layout(),
                                stream_3d_ref.water_camera_bind_group_layout(),
                                stream_3d_ref.depth_prepass_view(),
                                stream.resolution[0].max(1),
                                stream.resolution[1].max(1),
                            ));
                        }
                        if let Some(water) = self.camera_stream_water.as_mut() {
                            water.prepare(
                                &self.device,
                                &self.queue,
                                stream.waters_2d.as_ref(),
                                &[],
                                WaterPrepareContext {
                                    camera_2d_position: camera_position,
                                    camera_3d_position: [0.0, 0.0, 0.0],
                                    camera_3d_frustum_planes: [[0.0; 4]; 6],
                                    sky_color: [0.0, 0.0, 0.0],
                                    time_seconds: frame_time_seconds,
                                    delta_seconds: frame_delta_seconds,
                                },
                            );
                            water.encode(&mut encoder);
                            water.render_2d(
                                &mut encoder,
                                render_view,
                                None,
                                stream_2d.camera_bind_group(),
                                None,
                            );
                        }
                    }
                    stream_2d.render_pass(&mut encoder, render_view, None, particle_rect_count);
                }
            } else if let CameraStreamSourceState::ThreeD(camera) = &stream.source {
                stream_post_camera = Some(camera.clone());
                if self.camera_stream_3d.is_none() {
                    let mut stream_3d = Gpu3D::new(
                        &self.device,
                        &self.queue,
                        self.render_format,
                        Gpu3DConfig {
                            sample_count: 1,
                            width: stream.resolution[0].max(1),
                            height: stream.resolution[1].max(1),
                            meshlets_enabled: self.meshlets_enabled,
                            dev_meshlets: self.dev_meshlets,
                            meshlet_debug_view: self.meshlet_debug_view,
                            occlusion_culling: self.occlusion_culling,
                            ssao: self.ssao,
                            indirect_first_instance_enabled: self.indirect_first_instance_enabled,
                            multi_draw_indirect_enabled: self.multi_draw_indirect_enabled,
                            texture_filter: self.texture_filter,
                        },
                    );
                    // Camera streams render into their own targets; the seam
                    // pass only wires up the main scene.
                    stream_3d.set_screen_blend_supported(false);
                    self.camera_stream_3d = Some(stream_3d);
                }
                if let Some(stream_3d) = self.camera_stream_3d.as_mut() {
                    let width = stream.resolution[0].max(1);
                    let height = stream.resolution[1].max(1);
                    fill_camera_stream_draws_3d(
                        stream.draws_3d.as_ref(),
                        &mut self.camera_stream_draws_scratch,
                    );
                    let stream_lighting = camera_stream_lighting_3d(&stream.lighting_3d);
                    let stream_clear_color = if stream.transparent_background {
                        stream
                            .clear_color
                            .map(premultiplied_clear_color)
                            .unwrap_or(wgpu::Color::TRANSPARENT)
                    } else {
                        sky_clear_color(&stream_lighting)
                            .or_else(|| stream.clear_color.map(premultiplied_clear_color))
                            .unwrap_or(wgpu::Color {
                                r: CLEAR_R,
                                g: CLEAR_G,
                                b: CLEAR_B,
                                a: 1.0,
                            })
                    };
                    stream_3d.resize(&self.device, width, height);
                    stream_3d.prepare(
                        &self.device,
                        &self.queue,
                        Prepare3D {
                            resources,
                            camera: camera.clone(),
                            lighting: &stream_lighting,
                            draws: &self.camera_stream_draws_scratch,
                            draws_revision: stream_draws_revision,
                            force_full_rebuild: has(DIRTY_RESOURCES),
                            decals: &[],
                            decals_revision: 0,
                            width,
                            height,
                            static_texture_lookup,
                            static_mesh_lookup,
                            static_shader_lookup,
                        },
                    );
                    stream_3d.render_pass(
                        &self.queue,
                        &mut encoder,
                        render_view,
                        stream_clear_color,
                        false,
                        camera,
                        !stream.transparent_background,
                    );
                    if !stream.point_particles_3d.is_empty() {
                        if self.camera_stream_particles_3d.is_none() {
                            self.camera_stream_particles_3d = Some(GpuPointParticles3D::new(
                                &self.device,
                                self.render_format,
                                1,
                            ));
                        }
                        if let Some(particles) = self.camera_stream_particles_3d.as_mut() {
                            particles.prepare(
                                &self.device,
                                &self.queue,
                                PreparePointParticles3D {
                                    camera: camera.clone(),
                                    emitters: stream.point_particles_3d.as_ref(),
                                    width,
                                    height,
                                },
                            );
                            particles.render_pass(
                                &mut encoder,
                                render_view,
                                stream_3d.depth_view(),
                            );
                        }
                    }
                    if !stream.waters_3d.is_empty() {
                        if self.camera_stream_water.is_none()
                            && let Some(stream_2d_ref) = self.camera_stream_2d.as_ref()
                        {
                            self.camera_stream_water = Some(GpuWater::new(
                                &self.device,
                                self.render_format,
                                1,
                                stream_2d_ref.camera_bind_group_layout(),
                                stream_3d.water_camera_bind_group_layout(),
                                stream_3d.depth_prepass_view(),
                                width,
                                height,
                            ));
                        }
                        if let Some(water) = self.camera_stream_water.as_mut() {
                            water.set_scene_color_size(
                                &self.device,
                                stream_3d.depth_prepass_view(),
                                width,
                                height,
                            );
                            let water_view_proj = water_camera_view_proj(camera, width, height);
                            water.prepare(
                                &self.device,
                                &self.queue,
                                &[],
                                stream.waters_3d.as_ref(),
                                WaterPrepareContext {
                                    camera_2d_position: [0.0, 0.0],
                                    camera_3d_position: camera.position,
                                    camera_3d_frustum_planes: water_extract_frustum_planes(
                                        water_view_proj,
                                    ),
                                    sky_color: sky_clear_color(&stream_lighting)
                                        .map(|color| {
                                            [color.r as f32, color.g as f32, color.b as f32]
                                        })
                                        .unwrap_or([0.0, 0.0, 0.0]),
                                    time_seconds: frame_time_seconds,
                                    delta_seconds: frame_delta_seconds,
                                },
                            );
                            water.encode(&mut encoder);
                            water.capture_scene_color(&mut encoder, &render_texture, render_view);
                            water.render_3d(
                                &mut encoder,
                                render_view,
                                stream_3d.depth_view(),
                                stream_3d.water_camera_bind_group(),
                                false,
                            );
                        }
                    }
                    if has_stream_post {
                        stream_post_depth_view = Some(stream_3d.depth_prepass_view().clone());
                    }
                }
                if let Some(overlay_camera) = stream.overlay_camera_2d.as_ref()
                    && (!stream.sprites_2d.is_empty()
                        || !stream.lights_2d.is_empty()
                        || !stream.point_particles_2d.is_empty()
                        || !stream.waters_2d.is_empty())
                {
                    if self.camera_stream_2d.is_none() {
                        self.camera_stream_2d = Some(Gpu2D::new(
                            &self.device,
                            self.render_format,
                            1,
                            self.texture_filter,
                        ));
                    }
                    if let Some(stream_2d) = self.camera_stream_2d.as_mut() {
                        let camera_position = overlay_camera.position;
                        let camera = camera_2d_uniform_from_state(
                            overlay_camera,
                            stream.resolution[0],
                            stream.resolution[1],
                        );
                        let empty_upload = RectUploadPlan {
                            full_reupload: true,
                            dirty_ranges: Vec::new(),
                            draw_count: 0,
                        };
                        stream_2d.prepare(
                            &self.device,
                            &self.queue,
                            Prepare2D {
                                resources,
                                camera,
                                rects: &[],
                                upload: &empty_upload,
                                sprites: stream.sprites_2d.as_ref(),
                                sprites_revision: stream_sprites_revision,
                                force_sprite_prepare: has(DIRTY_RESOURCES),
                                point_lights: stream.lights_2d.as_ref(),
                                point_lights_revision: u64::MAX,
                                shadow_casters: &[],
                                static_texture_lookup,
                            },
                        );
                        let particle_rect_count = stream_2d.prepare_stream_point_particles(
                            &self.device,
                            &self.queue,
                            stream.point_particles_2d.as_ref(),
                        );
                        if !stream.waters_2d.is_empty()
                            && let Some(stream_3d) = self.camera_stream_3d.as_ref()
                        {
                            if self.camera_stream_water.is_none() {
                                self.camera_stream_water = Some(GpuWater::new(
                                    &self.device,
                                    self.render_format,
                                    1,
                                    stream_2d.camera_bind_group_layout(),
                                    stream_3d.water_camera_bind_group_layout(),
                                    stream_3d.depth_prepass_view(),
                                    stream.resolution[0].max(1),
                                    stream.resolution[1].max(1),
                                ));
                            }
                            if let Some(water) = self.camera_stream_water.as_mut() {
                                water.prepare(
                                    &self.device,
                                    &self.queue,
                                    stream.waters_2d.as_ref(),
                                    &[],
                                    WaterPrepareContext {
                                        camera_2d_position: camera_position,
                                        camera_3d_position: [0.0, 0.0, 0.0],
                                        camera_3d_frustum_planes: [[0.0; 4]; 6],
                                        sky_color: [0.0, 0.0, 0.0],
                                        time_seconds: frame_time_seconds,
                                        delta_seconds: frame_delta_seconds,
                                    },
                                );
                                water.encode(&mut encoder);
                                water.render_2d(
                                    &mut encoder,
                                    render_view,
                                    None,
                                    stream_2d.camera_bind_group(),
                                    None,
                                );
                            }
                        }
                        stream_2d.render_pass(&mut encoder, render_view, None, particle_rect_count);
                    }
                }
            }
            if has_stream_post {
                if self.camera_stream_post.is_none() {
                    self.camera_stream_post = Some(PostProcessor::new(
                        &self.device,
                        &self.queue,
                        self.render_format,
                        stream.resolution[0].max(1),
                        stream.resolution[1].max(1),
                    ));
                }
                let camera = stream_post_camera.unwrap_or_default();
                if let Some(post) = self.camera_stream_post.as_mut() {
                    let (Some(depth_view), Some(input_view)) =
                        (stream_post_depth_view.as_ref(), post_input_view.as_ref())
                    else {
                        continue;
                    };
                    post.resize(
                        &self.device,
                        stream.resolution[0].max(1),
                        stream.resolution[1].max(1),
                    );
                    let post_context = PostProcessContext {
                        device: &self.device,
                        queue: &self.queue,
                        output_view: &target_view,
                        camera: &camera,
                        external_input_view_key: post_view_key,
                        depth_view_key: post_view_key.wrapping_add(1),
                        static_shader_lookup,
                        static_texture_lookup,
                    };
                    let post_chain_data = PostProcessChainData {
                        input_view,
                        depth_view,
                        effects: stream.post_processing.as_ref(),
                    };
                    post.apply_chain(&post_context, &post_chain_data, &mut encoder);
                }
            }
            if tone_map_stream {
                let input_view = if has_stream_post {
                    tonemap_input_view.as_ref()
                } else {
                    post_input_view.as_ref()
                };
                if let Some(input_view) = input_view {
                    self.camera_stream_tonemap.apply(
                        &self.device,
                        &self.queue,
                        &mut encoder,
                        input_view,
                        &target_view,
                        CameraStreamTonemapSettings {
                            hdr_status: self.hdr_status,
                            exposure: exposure_settings.exposure,
                        },
                    );
                }
            }
        }
        if let Some(water) = self.water.as_ref() {
            if gpu_timer_active && let Some(timer) = self.gpu_timer.as_ref() {
                timer.write_water_start(&mut encoder);
            }
            water.encode(&mut encoder);
            if gpu_timer_active && let Some(timer) = self.gpu_timer.as_ref() {
                timer.write_water_end(&mut encoder);
            }
        } else if gpu_timer_active && let Some(timer) = self.gpu_timer.as_ref() {
            timer.write_water_start(&mut encoder);
            timer.write_water_end(&mut encoder);
        }
        let clear_in_water_pass =
            self.three_d.is_none() && self.two_d.is_some() && !waters_2d.is_empty();
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.render_pass(
                &self.queue,
                &mut encoder,
                color_view,
                clear_color,
                depth_prepass_needed,
                &camera_3d,
                true,
            );
            // Seam pass runs on the resolved offscreen scene texture, before
            // particles/water/2D draw on top.
            if blend_screen_active && !direct_present && self.sample_count == 1 {
                three_d.mesh_blend_screen_pass(
                    &self.device,
                    &mut encoder,
                    self.post.scene_texture(),
                    &scene_view,
                );
            }
            if let Some(point_particles_3d_gpu) = self.point_particles_3d.as_mut() {
                point_particles_3d_gpu.render_pass(&mut encoder, color_view, three_d.depth_view());
            }
            if let Some(water) = self.water.as_ref() {
                let clear_water_depth = draws_3d.is_empty()
                    && point_particles_3d.is_empty()
                    && lighting_3d.sky.is_none();
                water.capture_scene_color(&mut encoder, self.post.scene_texture(), color_view);
                water.render_3d(
                    &mut encoder,
                    color_view,
                    three_d.depth_view(),
                    three_d.water_camera_bind_group(),
                    clear_water_depth,
                );
            }
        } else if !clear_in_water_pass {
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_clear_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    resolve_target: resolve_view,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        if let Some(two_d) = self.two_d.as_ref() {
            let two_d_draws = two_d.draw_call_count(rect_draw_count) > 0;
            if let Some(water) = self.water.as_ref() {
                water.render_2d(
                    &mut encoder,
                    color_view,
                    (!two_d_draws).then_some(resolve_view).flatten(),
                    two_d.camera_bind_group(),
                    clear_in_water_pass.then_some(clear_color),
                );
            }
            if two_d_draws {
                two_d.render_pass(&mut encoder, color_view, resolve_view, rect_draw_count);
            } else if waters_2d.is_empty()
                && let Some(resolve_target) = resolve_view
            {
                let _resolve_only_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("perro_msaa_resolve_only_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: color_view,
                        resolve_target: Some(resolve_target),
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
            }
        } else if let Some(resolve_target) = resolve_view {
            // No 2D pass still needs one resolve pass on MSAA paths.
            let _resolve_only_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_msaa_resolve_only_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    resolve_target: Some(resolve_target),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        if blend_screen_active
            && !direct_present
            && !msaa_direct_present
            && self.sample_count > 1
            && let Some(three_d) = self.three_d.as_mut()
        {
            three_d.mesh_blend_screen_pass(
                &self.device,
                &mut encoder,
                self.post.scene_texture(),
                &scene_view,
            );
        }
        timing.encode_main = encode_start.elapsed();

        let post_start = Instant::now();
        #[derive(Clone, Copy)]
        enum FrameTex {
            Scene,
            Intermediate,
        }
        let mut current_tex = FrameTex::Scene;
        let post_view_generation = self.post_view_generation;
        let mut apply_post_chain = |effects: &[perro_structs::PostProcessEffect],
                                    current_tex: &mut FrameTex| {
            if effects.is_empty() {
                return;
            }
            let (input_view, output_view, next_tex, input_slot) = match *current_tex {
                FrameTex::Scene => (&scene_view, &intermediate_view, FrameTex::Intermediate, 1),
                FrameTex::Intermediate => (&intermediate_view, &scene_view, FrameTex::Scene, 2),
            };
            let view_key_base = post_view_generation.wrapping_mul(8);
            let post_context = PostProcessContext {
                device: &self.device,
                queue: &self.queue,
                output_view,
                camera: &camera_3d,
                external_input_view_key: view_key_base.wrapping_add(input_slot),
                depth_view_key: view_key_base.wrapping_add(3),
                static_shader_lookup,
                static_texture_lookup,
            };
            let Some(three_d) = self.three_d.as_ref() else {
                return;
            };
            let post_chain_data = PostProcessChainData {
                input_view,
                depth_view: three_d.depth_prepass_view(),
                effects,
            };
            self.post
                .apply_chain(&post_context, &post_chain_data, &mut encoder);
            *current_tex = next_tex;
        };
        if camera_post_enabled {
            apply_post_chain(camera_post_chain, &mut current_tex);
        }
        if global_post_enabled {
            apply_post_chain(global_post_chain, &mut current_tex);
        }
        timing.post_process = post_start.elapsed();

        let accessibility_start = Instant::now();
        if accessibility_enabled {
            let (accessibility_input_view, accessibility_output_view, next_tex) = match current_tex
            {
                FrameTex::Scene => (&scene_view, &intermediate_view, FrameTex::Intermediate),
                FrameTex::Intermediate => (&intermediate_view, &scene_view, FrameTex::Scene),
            };
            self.accessibility.apply(
                &self.device,
                &self.queue,
                &mut encoder,
                accessibility_input_view,
                accessibility_output_view,
                accessibility,
            );
            current_tex = next_tex;
        }
        timing.accessibility = accessibility_start.elapsed();

        if !direct_present && !msaa_direct_present {
            let final_bind_group = match current_tex {
                FrameTex::Scene => &self.present_scene_bind_group,
                FrameTex::Intermediate => &self.present_intermediate_bind_group,
            };
            let acquire_start = Instant::now();
            let acquire_surface_start = Instant::now();
            let acquired = match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    timing.acquire_surface = acquire_surface_start.elapsed();
                    self.surface.configure(&self.device, &self.config);
                    timing.acquire = acquire_start.elapsed();
                    timing.total = total_start.elapsed();
                    return timing;
                }
                wgpu::CurrentSurfaceTexture::Timeout
                | wgpu::CurrentSurfaceTexture::Occluded
                | wgpu::CurrentSurfaceTexture::Validation => {
                    timing.acquire_surface = acquire_surface_start.elapsed();
                    timing.acquire = acquire_start.elapsed();
                    timing.total = total_start.elapsed();
                    return timing;
                }
            };
            timing.acquire_surface = acquire_surface_start.elapsed();
            let acquire_view_start = Instant::now();
            let view = acquired.texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(self.surface_view_format),
                ..Default::default()
            });
            timing.acquire_view = acquire_view_start.elapsed();
            timing.acquire = acquire_start.elapsed();
            self.present.apply(
                &self.queue,
                &mut encoder,
                final_bind_group,
                &view,
                [self.render_width, self.render_height],
                frame_delta_seconds,
                exposure_settings,
                self.hdr_status,
            );
            swap_view = Some(view);
            frame = Some(acquired);
        }
        if ui_primitives.is_empty() {
            if let Some(ui) = self.ui.as_mut() {
                ui.clear();
            }
        } else {
            if self.ui.is_none() {
                self.ui = Some(GpuUi::new(
                    &self.device,
                    self.surface_view_format,
                    self.texture_filter,
                ));
            }
            if let (Some(ui), Some(output_view)) = (self.ui.as_mut(), swap_view.as_ref()) {
                let viewport = [self.config.width.max(1), self.config.height.max(1)];
                ui.prepare(
                    &self.device,
                    &self.queue,
                    UiPrepareInput {
                        resources,
                        viewport,
                        primitives: ui_primitives,
                        primitive_depths: ui_primitive_depths,
                        textures_delta: ui_textures_delta,
                        texture_size: ui_texture_size,
                        revision: ui_revision,
                        static_texture_lookup,
                    },
                );
                ui.render_pass(
                    &self.device,
                    &mut encoder,
                    output_view,
                    viewport,
                    self.three_d
                        .as_ref()
                        .map(|three_d| three_d.depth_prepass_view()),
                );
            }
        }
        if late_overlay_upload_2d.draw_count > 0
            || !late_overlay_sprites_2d.is_empty()
            || !late_overlay_point_lights_2d.is_empty()
        {
            if self.late_overlay_2d.is_none() {
                self.late_overlay_2d = Some(Gpu2D::new(
                    &self.device,
                    self.surface_view_format,
                    1,
                    self.texture_filter,
                ));
            }
            if let (Some(late_overlay_2d), Some(output_view)) =
                (self.late_overlay_2d.as_mut(), swap_view.as_ref())
            {
                late_overlay_2d.prepare(
                    &self.device,
                    &self.queue,
                    Prepare2D {
                        resources,
                        camera: late_overlay_camera_2d,
                        rects: late_overlay_rects_2d,
                        upload: late_overlay_upload_2d,
                        sprites: late_overlay_sprites_2d,
                        sprites_revision: late_overlay_sprites_2d_revision,
                        force_sprite_prepare: has(DIRTY_RESOURCES),
                        point_lights: late_overlay_point_lights_2d,
                        point_lights_revision: late_overlay_point_lights_2d_revision,
                        shadow_casters: late_overlay_shadow_casters_2d,
                        static_texture_lookup,
                    },
                );
                late_overlay_2d.render_pass(
                    &mut encoder,
                    output_view,
                    None,
                    late_overlay_upload_2d.draw_count as u32,
                );
            }
        }
        if gpu_timer_active && let Some(timer) = self.gpu_timer.as_ref() {
            timer.write_end_and_resolve(&mut encoder);
        }
        if let Some(water) = self.water.as_mut() {
            water.encode_readback(&mut encoder);
        }
        let submit_start = Instant::now();
        let submit_finish_start = Instant::now();
        let command_buffer = encoder.finish();
        timing.submit_finish_main = submit_finish_start.elapsed();
        let submit_queue_start = Instant::now();
        self.queue.submit(Some(command_buffer));
        if gpu_timer_active && let Some(timer) = self.gpu_timer.as_mut() {
            timer.request_readback();
        }
        if let Some(water) = self.water.as_mut() {
            water.finish_frame();
            water.request_readback();
        }
        timing.submit_queue_main = submit_queue_start.elapsed();
        timing.submit_main = submit_start.elapsed();
        timing.draw_calls_2d = self
            .two_d
            .as_ref()
            .map(|two_d| two_d.draw_call_count(rect_draw_count))
            .unwrap_or(0)
            + self.ui.as_ref().map(GpuUi::draw_call_count).unwrap_or(0);
        timing.draw_calls_3d = self
            .three_d
            .as_ref()
            .map(|three_d| three_d.draw_call_count())
            .unwrap_or(0);
        if let Some(three_d) = self.three_d.as_ref() {
            timing.draw_batches_3d = three_d.draw_batch_count();
            timing.pipeline_switches_3d = three_d.pipeline_switch_count();
            timing.texture_bind_group_switches_3d = three_d.texture_bind_group_switch_count();
        }
        let present_start = Instant::now();
        if let Some(frame) = frame {
            self.queue.present(frame);
            timing.present = present_start.elapsed();
            timing.presented = true;
        }
        timing.total = total_start.elapsed();
        timing
    }

    pub fn drain_water_samples(&mut self, out: &mut Vec<WaterSampleState>) {
        if let Some(water) = self.water.as_mut() {
            water.drain_samples(out);
        }
    }

    pub fn drain_water_body_samples(&mut self, out: &mut Vec<WaterBodySampleState>) {
        if let Some(water) = self.water.as_mut() {
            water.drain_body_samples(out);
        }
    }

    pub fn virtual_size() -> [f32; 2] {
        Gpu2D::virtual_size()
    }
}
