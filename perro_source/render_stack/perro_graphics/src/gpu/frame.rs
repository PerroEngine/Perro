use super::*;

impl Gpu {
    pub fn render(&mut self, frame: RenderFrame<'_>) -> RenderGpuTiming {
        let total_start = Instant::now();
        let mut timing = RenderGpuTiming::default();
        self.poll_camera_image_saves();
        if let Some(timer) = self.gpu_timer.as_mut() {
            timer.poll(&self.device);
            timing.gpu_timestamp_main = timer.last_main;
            timing.gpu_timestamp_water = timer.last_water;
            timing.gpu_timestamp_shadow = timer.last_shadow;
            // Slots 6/7 bracket shadow + mesh; shadow is timed separately.
            timing.gpu_timestamp_mesh = timer.last_mesh.saturating_sub(timer.last_shadow);
            timing.gpu_timestamp_post = timer.last_post;
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
            shadow_casters_2d_revision,
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
            late_overlay_shadow_casters_2d_revision,
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
            animated_stream_nodes,
            changed_stream_nodes,
            scene_continuous_updates,
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
            || (has(DIRTY_RESOURCES) && has_2d_content);

        // A decal whose texture is still decoding must be retried each frame
        // until it resolves; otherwise it stays hidden until the next dirty
        // frame forces a re-prepare (looked like "white until reload").
        let decals_texture_pending = self
            .three_d
            .as_ref()
            .is_some_and(|three_d| three_d.decals_pending());

        let three_d_content_changed = self.three_d.is_some()
            && (self.last_prepare_3d_camera.as_ref() != Some(&camera_3d)
                || !self
                    .last_prepare_3d_lighting
                    .as_ref()
                    .is_some_and(|prev| prev.content_eq(lighting_3d))
                || self.last_prepare_3d_draws_revision != draws_3d_revision
                || self.last_prepare_3d_decals_revision != decals_3d_revision
                || decals_texture_pending
                || self.last_prepare_3d_width != self.render_width
                || self.last_prepare_3d_height != self.render_height);

        let needs_3d = !draws_3d.is_empty();
        let needs_particles_3d = !point_particles_3d.is_empty();
        let needs_water = !waters_2d.is_empty() || !waters_3d.is_empty();
        let has_3d_content = needs_3d
            || needs_particles_3d
            || needs_water
            || !decals_3d.is_empty()
            || lighting_3d.sky.is_some()
            || ui_primitive_depths.iter().any(Option::is_some);
        let three_d_dirty =
            has(DIRTY_3D) || has(DIRTY_CAMERA_3D) || has(DIRTY_LIGHTS_3D) || has(DIRTY_RESOURCES);

        let needs_3d_pipeline = has_3d_content
            || post_requested
            || three_d_content_changed
            || (self.three_d.is_some() && three_d_dirty);

        // Prepare only on actual change: dirty bits or tracked content deltas.
        // `has_3d_content` alone must NOT force prepare — it is true every
        // frame in any 3D scene and re-running prepare (uniform rebuilds,
        // buffer writes, shadow state) on static frames costs real fps.
        let needs_3d_prepare = needs_3d_pipeline && (three_d_content_changed || three_d_dirty);

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
        // Output textures of every stream that (re)binds a target or
        // re-renders one this frame. Collected conservatively: a stream that
        // passes the idle gate counts as rendered even if a later early-out
        // skips it. The retained-scene fast path is only blocked when a
        // retained MAIN-scene draw / decal / sprite samples one of these (see
        // `collect_main_scene_sampled_texture_slots`); a UI-only consumer such
        // as a minimap composites after the scene texture and must not block.
        let mut rendered_stream_textures: Vec<perro_ids::TextureID> = Vec::new();
        for (node, stream) in camera_streams {
            if !camera_stream_uses_render_target(stream) {
                continue;
            }
            let resolution = [stream.resolution[0].max(1), stream.resolution[1].max(1)];
            let has_stream_post = PostProcessor::has_effects(stream.post_processing.as_ref());
            let tone_map_stream = stream.tone_map_output
                && !matches!(stream.source, CameraStreamSourceState::Webcam { .. });
            let needs_intermediate = has_stream_post || tone_map_stream;
            let needs_tonemap_input = has_stream_post && tone_map_stream;
            let needs_post_depth =
                has_stream_post && !matches!(stream.source, CameraStreamSourceState::ThreeD(_));
            let needs_external_binding =
                self.camera_stream_targets.get(node).is_none_or(|target| {
                    target.resolution != resolution
                        || target.post_input_view.is_some() != needs_intermediate
                        || target.tonemap_input_view.is_some() != needs_tonemap_input
                        || target.depth_view.is_some() != needs_post_depth
                }) || self.camera_stream_external_bindings.get(node).copied() != Some(resolution);
            let Some(target) = self.ensure_camera_stream_target(
                *node,
                resolution,
                needs_intermediate,
                needs_tonemap_input,
                needs_post_depth,
            ) else {
                continue;
            };
            if needs_external_binding {
                rendered_stream_textures.push(stream.output_texture);
                let texture_id = stream.output_texture;
                let view_2d = target.view.clone();
                let view_ui = target.view.clone();
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
        if needs_2d_prepare && has_2d_content {
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
                        shared_textures: &mut self.shared_textures,
                        camera: camera_2d,
                        rects: rects_2d,
                        upload: upload_2d,
                        sprites: sprites_2d,
                        sprites_revision: sprites_2d_revision,
                        force_sprite_prepare: has(DIRTY_RESOURCES),
                        point_lights: point_lights_2d,
                        point_lights_revision: point_lights_2d_revision,
                        shadow_casters: shadow_casters_2d,
                        shadow_casters_revision: shadow_casters_2d_revision,
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
            self.ensure_3d_sample_count();
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
                        multi_draw_indirect_count_enabled: self.multi_draw_indirect_count_enabled,
                        texture_filter: self.texture_filter,
                        shader_variant_mode: self.shader_variant_mode,
                        shadow_pcf_high: self.shadow_pcf_high,
                        shadow_scale_to_target: false,
                    },
                    self.pipeline_registries.get_or_create(
                        &self.device,
                        self.render_format,
                        self.sample_count,
                    ),
                    &self.mesh_arena,
                ));
            }
            if self.water.is_none() {
                if self.two_d.is_none() {
                    self.two_d = Some(Gpu2D::new(
                        &self.device,
                        self.render_format,
                        self.sample_count,
                        self.texture_filter,
                    ));
                }
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
                        camera_3d_position: camera_3d.position,
                        camera_3d_frustum_planes: water_extract_frustum_planes(water_view_proj),
                        camera_3d_lod_scale: water_camera_lod_scale(&camera_3d, self.render_height),
                        sky_color,
                        time_seconds: frame_time_seconds,
                        delta_seconds: frame_delta_seconds,
                        scene_geometry_present: !draws_3d.is_empty()
                            || !point_particles_3d.is_empty(),
                    },
                );
                self.last_prepare_water_2d_revision = waters_2d_revision;
                self.last_prepare_water_3d_revision = waters_3d_revision;
            }
        } else if !needs_water {
            if let Some(water) = self.water.as_mut() {
                water.clear_active();
                water.note_scene_color_idle(&self.device);
            }
            self.last_prepare_water_2d_revision = u64::MAX;
            self.last_prepare_water_3d_revision = u64::MAX;
        }

        let prepare_3d_start = Instant::now();
        let mut did_prepare_3d = false;
        let mut prepare_3d_steps = Prepare3DStepTiming::default();
        if needs_3d_pipeline {
            self.ensure_3d_sample_count();
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
                        multi_draw_indirect_count_enabled: self.multi_draw_indirect_count_enabled,
                        texture_filter: self.texture_filter,
                        shader_variant_mode: self.shader_variant_mode,
                        shadow_pcf_high: self.shadow_pcf_high,
                        shadow_scale_to_target: false,
                    },
                    self.pipeline_registries.get_or_create(
                        &self.device,
                        self.render_format,
                        self.sample_count,
                    ),
                    &self.mesh_arena,
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
                    three_d.upsert_external_material_texture(
                        &self.device,
                        stream.output_texture.index(),
                        &target.view,
                        format!("__camera_stream__:{}", node.as_u64()),
                    );
                    self.camera_stream_3d_bindings.insert(*node, resolution);
                }
                three_d.prepare(
                    &self.device,
                    &self.queue,
                    Prepare3D {
                        resources,
                        shared_textures: &mut self.shared_textures,
                        mesh_arena: &mut self.mesh_arena,
                        // The main view prepares before the camera-stream loop,
                        // so it is the only one allowed to consume the GC tick's
                        // arena compaction request.
                        mesh_arena_compact_allowed: true,
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
            // Prepare skipped: still advance shader frame globals so
            // perro_time()-driven materials animate on static frames.
            if needs_3d_pipeline && let Some(three_d) = self.three_d.as_ref() {
                three_d.patch_scene_globals(
                    &self.queue,
                    lighting_3d,
                    self.render_width,
                    self.render_height,
                );
            }
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

        // TAA sub-pixel camera jitter: advance the Halton(2,3) sequence every
        // frame (static frames skip prepare, but the jitter must still move
        // for the history to accumulate sub-pixel detail) and patch only the
        // GPU camera uniform. Main pass, depth prepass, water and decals all
        // read that uniform, so they jitter together; shadows, frustum/HiZ
        // culling, CPU occlusion, SSAO and the sky uniform keep unjittered
        // matrices (see apply_taa_jitter). Camera streams render through
        // their own Gpu3D instances and never jitter.
        let taa_run = self.present.taa_active() && self.three_d.is_some();
        if let Some(three_d) = self.three_d.as_mut() {
            let jitter_ndc = taa_run.then(|| {
                let jitter_px = taa_jitter_offset(self.taa_frame_index);
                self.taa_frame_index = self.taa_frame_index.wrapping_add(1);
                [
                    2.0 * jitter_px[0] / self.render_width.max(1) as f32,
                    2.0 * jitter_px[1] / self.render_height.max(1) as f32,
                ]
            });
            three_d.apply_taa_jitter(&self.queue, jitter_ndc);
        }

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
        let accessibility_enabled = accessibility.color_blind.is_some();
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
            let Some(acquired) = self.acquire_surface_texture() else {
                timing.acquire_surface = acquire_surface_start.elapsed();
                timing.acquire = acquire_start.elapsed();
                timing.total = total_start.elapsed();
                return timing;
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
        let intermediate_needed =
            camera_post_enabled || global_post_enabled || accessibility_enabled;
        if intermediate_needed {
            if self.accessibility.is_none() {
                let processor = VisualAccessibilityProcessor::new(
                    &self.device,
                    self.render_format,
                    self.render_width,
                    self.render_height,
                );
                self.present_intermediate_bind_group = Some(
                    self.present
                        .create_bind_group(&self.device, processor.intermediate_view()),
                );
                self.accessibility = Some(processor);
            } else if let Some(processor) = self.accessibility.as_mut() {
                // Re-promote an idle-released intermediate; the present bind
                // group built on the old view is stale after recreation.
                if processor.resize(&self.device, self.render_width, self.render_height) {
                    self.present_intermediate_bind_group = Some(
                        self.present
                            .create_bind_group(&self.device, processor.intermediate_view()),
                    );
                }
            }
        } else if let Some(processor) = self.accessibility.as_mut()
            && processor.note_idle_frame(&self.device)
        {
            self.present_intermediate_bind_group = None;
        }
        let intermediate_view = self
            .accessibility
            .as_ref()
            .map(|processor| processor.intermediate_view().clone())
            .unwrap_or_else(|| scene_view.clone());
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
        timing.stream_count = camera_streams.len().min(u32::MAX as usize) as u32;
        let stream_loop_start = Instant::now();
        for (node, stream) in camera_streams {
            let stream_reentered = !self.camera_stream_content_revisions.contains_key(node);
            // per-stream idle skip: unchanged state + nothing animating inside
            // => keep last rendered target texture, encode no passes. main
            // frame composites the retained texture as usual. change tracking
            // happens at Upsert apply, so no per-frame state compare here.
            let prev_state_matches = !changed_stream_nodes.contains(node);
            let stream_can_idle = prev_state_matches
                && !stream_reentered
                && !has(DIRTY_RESOURCES)
                && !animated_stream_nodes.contains(node)
                && stream.waters_2d.is_empty()
                && stream.waters_3d.is_empty()
                && stream.point_particles_2d.is_empty()
                && stream.point_particles_3d.is_empty()
                && !matches!(stream.source, CameraStreamSourceState::Webcam { .. })
                && stream.post_processing.is_empty()
                && stream
                    .lighting_3d
                    .sky
                    .as_ref()
                    .is_none_or(|sky| sky.time.paused && sky.shaders.is_empty());
            if stream_can_idle {
                continue;
            }
            rendered_stream_textures.push(stream.output_texture);
            timing.stream_renders = timing.stream_renders.saturating_add(1);
            timing.stream_pixels = timing.stream_pixels.saturating_add(
                u64::from(stream.resolution[0].max(1)) * u64::from(stream.resolution[1].max(1)),
            );
            // revision update only on render: idle streams skip the content
            // compare entirely; the compare against last-RENDERED content is
            // exactly what the prepare paths below need.
            let (stream_draws_revision, stream_sprites_revision) =
                update_camera_stream_content_revisions(
                    &mut self.camera_stream_content_revisions,
                    &mut self.next_camera_stream_content_revision,
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
            let (target_view, post_input_view, tonemap_input_view, post_depth_view, post_view_key) = {
                let Some(target) = self.camera_stream_targets.get(node) else {
                    continue;
                };
                (
                    target.view.clone(),
                    needs_intermediate.then(|| {
                        target
                            .post_input_view
                            .clone()
                            .expect("camera stream intermediate target")
                    }),
                    (has_stream_post && tone_map_stream).then(|| {
                        target
                            .tonemap_input_view
                            .clone()
                            .expect("camera stream tonemap input")
                    }),
                    target.depth_view.clone(),
                    target.post_view_key,
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
                let stream_2d =
                    camera_stream_cache_entry(&mut self.camera_stream_2d, *node, || {
                        Gpu2D::new(&self.device, self.render_format, 1, self.texture_filter)
                    });
                let source_view = stream_2d.ensure_sampled_texture_view(
                    &self.device,
                    &self.queue,
                    &mut self.shared_textures,
                    resources,
                    *texture,
                    static_texture_lookup,
                );
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
                let post = camera_stream_cache_entry(&mut self.camera_stream_post, *node, || {
                    PostProcessor::new(
                        &self.device,
                        &self.queue,
                        self.render_format,
                        stream.resolution[0].max(1),
                        stream.resolution[1].max(1),
                    )
                });
                {
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
                        hdr_output: self.hdr_status.active,
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
                if !stream.sprites_2d.is_empty()
                    || !stream.lights_2d.is_empty()
                    || !stream.point_particles_2d.is_empty()
                    || !stream.waters_2d.is_empty()
                {
                    let stream_2d =
                        camera_stream_cache_entry(&mut self.camera_stream_2d, *node, || {
                            Gpu2D::new(&self.device, self.render_format, 1, self.texture_filter)
                        });
                    let camera = camera_2d_uniform_from_state(
                        camera,
                        stream.resolution[0],
                        stream.resolution[1],
                        self.virtual_size_2d,
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
                            shared_textures: &mut self.shared_textures,
                            camera,
                            rects: &[],
                            upload: &empty_upload,
                            sprites: stream.sprites_2d.as_ref(),
                            sprites_revision: stream_sprites_revision,
                            force_sprite_prepare: has(DIRTY_RESOURCES),
                            point_lights: stream.lights_2d.as_ref(),
                            point_lights_revision: u64::MAX,
                            shadow_casters: &[],
                            shadow_casters_revision: u64::MAX,
                            static_texture_lookup,
                        },
                    );
                    let particle_rect_count = stream_2d.prepare_stream_point_particles(
                        &self.device,
                        &self.queue,
                        stream.point_particles_2d.as_ref(),
                    );
                    if !stream.waters_2d.is_empty() {
                        let stream_3d_ref =
                            camera_stream_cache_entry(&mut self.camera_stream_3d, *node, || {
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
                                        multi_draw_indirect_enabled: self
                                            .multi_draw_indirect_enabled,
                                        multi_draw_indirect_count_enabled: self
                                            .multi_draw_indirect_count_enabled,
                                        texture_filter: self.texture_filter,
                                        shader_variant_mode: self.shader_variant_mode,
                                        shadow_pcf_high: self.shadow_pcf_high,
                                        shadow_scale_to_target: true,
                                    },
                                    self.pipeline_registries.get_or_create(
                                        &self.device,
                                        self.render_format,
                                        1,
                                    ),
                                    &self.mesh_arena,
                                );
                                // Camera streams render into their own targets;
                                // the seam pass only wires up the main scene.
                                stream_3d.set_screen_blend_supported(false);
                                stream_3d
                            });
                        let water =
                            camera_stream_cache_entry(&mut self.camera_stream_water, *node, || {
                                GpuWater::new(
                                    &self.device,
                                    self.render_format,
                                    1,
                                    stream_2d.camera_bind_group_layout(),
                                    stream_3d_ref.water_camera_bind_group_layout(),
                                    stream_3d_ref.depth_prepass_view(),
                                    stream.resolution[0].max(1),
                                    stream.resolution[1].max(1),
                                )
                            });
                        water.prepare(
                            &self.device,
                            &self.queue,
                            stream.waters_2d.as_ref(),
                            &[],
                            WaterPrepareContext {
                                camera_3d_position: [0.0, 0.0, 0.0],
                                camera_3d_frustum_planes: [[0.0; 4]; 6],
                                camera_3d_lod_scale: [0.0; 2],
                                sky_color: [0.0, 0.0, 0.0],
                                time_seconds: frame_time_seconds,
                                delta_seconds: frame_delta_seconds,
                                scene_geometry_present: false,
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
                    stream_2d.render_pass(&mut encoder, render_view, None, particle_rect_count);
                }
            } else if let CameraStreamSourceState::ThreeD(camera) = &stream.source {
                stream_post_camera = Some(camera.clone());
                if camera_stream_needs_3d_world(stream) {
                    let stream_3d =
                        camera_stream_cache_entry(&mut self.camera_stream_3d, *node, || {
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
                                    multi_draw_indirect_count_enabled: self
                                        .multi_draw_indirect_count_enabled,
                                    texture_filter: self.texture_filter,
                                    shader_variant_mode: self.shader_variant_mode,
                                    shadow_pcf_high: self.shadow_pcf_high,
                                    shadow_scale_to_target: true,
                                },
                                self.pipeline_registries.get_or_create(
                                    &self.device,
                                    self.render_format,
                                    1,
                                ),
                                &self.mesh_arena,
                            );
                            // Camera streams render into their own targets; the seam
                            // pass only wires up the main scene.
                            stream_3d.set_screen_blend_supported(false);
                            stream_3d
                        });
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
                            shared_textures: &mut self.shared_textures,
                            mesh_arena: &mut self.mesh_arena,
                            mesh_arena_compact_allowed: false,
                            camera: camera.clone(),
                            lighting: &stream_lighting,
                            draws: &self.camera_stream_draws_scratch,
                            draws_revision: stream_draws_revision,
                            force_full_rebuild: has(DIRTY_RESOURCES) || stream_reentered,
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
                        // Streams share the main view's query slots; only the
                        // main view may write them.
                        None,
                    );
                    // Same harvest the main view does after submit, but per
                    // stream and summed: `render_pass` resets these at entry, so
                    // read them here while this stream's pass is the last one
                    // encoded on its own `Gpu3D`.
                    timing.stream_draw_calls_3d = timing
                        .stream_draw_calls_3d
                        .saturating_add(stream_3d.draw_call_count());
                    timing.stream_draw_batches_3d = timing
                        .stream_draw_batches_3d
                        .saturating_add(stream_3d.draw_batch_count());
                    timing.stream_draw_triangles_3d = timing
                        .stream_draw_triangles_3d
                        .saturating_add(stream_3d.triangle_count());
                    let stream_counters = stream_3d.pass_counters();
                    timing.stream_render_passes = timing
                        .stream_render_passes
                        .saturating_add(stream_counters.render_passes);
                    timing.stream_shadow_layer_renders = timing
                        .stream_shadow_layer_renders
                        .saturating_add(stream_counters.shadow_layer_renders);
                    if !stream.point_particles_3d.is_empty() {
                        let particles = camera_stream_cache_entry(
                            &mut self.camera_stream_particles_3d,
                            *node,
                            || GpuPointParticles3D::new(&self.device, self.render_format, 1),
                        );
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
                        particles.render_pass(&mut encoder, render_view, stream_3d.depth_view());
                    }
                    if !stream.waters_3d.is_empty()
                        && let Some(stream_2d_ref) = self.two_d.as_ref()
                    {
                        let water =
                            camera_stream_cache_entry(&mut self.camera_stream_water, *node, || {
                                GpuWater::new(
                                    &self.device,
                                    self.render_format,
                                    1,
                                    stream_2d_ref.camera_bind_group_layout(),
                                    stream_3d.water_camera_bind_group_layout(),
                                    stream_3d.depth_prepass_view(),
                                    width,
                                    height,
                                )
                            });
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
                                camera_3d_position: camera.position,
                                camera_3d_frustum_planes: water_extract_frustum_planes(
                                    water_view_proj,
                                ),
                                camera_3d_lod_scale: water_camera_lod_scale(camera, height),
                                sky_color: sky_clear_color(&stream_lighting)
                                    .map(|color| [color.r as f32, color.g as f32, color.b as f32])
                                    .unwrap_or([0.0, 0.0, 0.0]),
                                time_seconds: frame_time_seconds,
                                delta_seconds: frame_delta_seconds,
                                scene_geometry_present: !stream.draws_3d.is_empty()
                                    || !stream.point_particles_3d.is_empty(),
                            },
                        );
                        water.encode(&mut encoder);
                        water.capture_scene_color(&self.device, &mut encoder, render_view);
                        // Streams render at 1x: water depth-tests against a
                        // scene-depth copy (the scene depth target aliases the
                        // sampled prepass texture).
                        let water_depth_view =
                            stream_3d.water_depth_attachment(&self.device, &mut encoder);
                        water.render_3d(
                            &mut encoder,
                            render_view,
                            &water_depth_view,
                            stream_3d.water_camera_bind_group(),
                            false,
                        );
                    }
                    if stream.waters_3d.is_empty() {
                        stream_3d.release_water_depth();
                    }
                    if has_stream_post {
                        stream_post_depth_view = Some(stream_3d.depth_prepass_view().clone());
                    }
                } else {
                    let clear = stream.clear_color.map(premultiplied_clear_color).unwrap_or(
                        if stream.transparent_background {
                            wgpu::Color::TRANSPARENT
                        } else {
                            wgpu::Color::BLACK
                        },
                    );
                    let _clear_empty_3d = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("perro_camera_stream_clear_empty_3d"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: render_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(clear),
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: stream_post_depth_view.as_ref().map(|view| {
                            wgpu::RenderPassDepthStencilAttachment {
                                view,
                                depth_ops: Some(wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(1.0),
                                    store: wgpu::StoreOp::Store,
                                }),
                                stencil_ops: None,
                            }
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                }
                if let Some(overlay_camera) = stream.overlay_camera_2d.as_ref()
                    && (!stream.sprites_2d.is_empty()
                        || !stream.lights_2d.is_empty()
                        || !stream.point_particles_2d.is_empty()
                        || !stream.waters_2d.is_empty())
                {
                    let stream_2d =
                        camera_stream_cache_entry(&mut self.camera_stream_2d, *node, || {
                            Gpu2D::new(&self.device, self.render_format, 1, self.texture_filter)
                        });
                    {
                        let camera = camera_2d_uniform_from_state(
                            overlay_camera,
                            stream.resolution[0],
                            stream.resolution[1],
                            self.virtual_size_2d,
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
                                shared_textures: &mut self.shared_textures,
                                camera,
                                rects: &[],
                                upload: &empty_upload,
                                sprites: stream.sprites_2d.as_ref(),
                                sprites_revision: stream_sprites_revision,
                                force_sprite_prepare: has(DIRTY_RESOURCES),
                                point_lights: stream.lights_2d.as_ref(),
                                point_lights_revision: u64::MAX,
                                shadow_casters: &[],
                                shadow_casters_revision: u64::MAX,
                                static_texture_lookup,
                            },
                        );
                        let particle_rect_count = stream_2d.prepare_stream_point_particles(
                            &self.device,
                            &self.queue,
                            stream.point_particles_2d.as_ref(),
                        );
                        if !stream.waters_2d.is_empty()
                            && let Some(stream_3d) = self.camera_stream_3d.get(node)
                        {
                            let water = camera_stream_cache_entry(
                                &mut self.camera_stream_water,
                                *node,
                                || {
                                    GpuWater::new(
                                        &self.device,
                                        self.render_format,
                                        1,
                                        stream_2d.camera_bind_group_layout(),
                                        stream_3d.water_camera_bind_group_layout(),
                                        stream_3d.depth_prepass_view(),
                                        stream.resolution[0].max(1),
                                        stream.resolution[1].max(1),
                                    )
                                },
                            );
                            water.prepare(
                                &self.device,
                                &self.queue,
                                stream.waters_2d.as_ref(),
                                &[],
                                WaterPrepareContext {
                                    camera_3d_position: [0.0, 0.0, 0.0],
                                    camera_3d_frustum_planes: [[0.0; 4]; 6],
                                    camera_3d_lod_scale: [0.0; 2],
                                    sky_color: [0.0, 0.0, 0.0],
                                    time_seconds: frame_time_seconds,
                                    delta_seconds: frame_delta_seconds,
                                    scene_geometry_present: false,
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
                        stream_2d.render_pass(&mut encoder, render_view, None, particle_rect_count);
                    }
                }
            }
            if has_stream_post {
                let post = camera_stream_cache_entry(&mut self.camera_stream_post, *node, || {
                    PostProcessor::new(
                        &self.device,
                        &self.queue,
                        self.render_format,
                        stream.resolution[0].max(1),
                        stream.resolution[1].max(1),
                    )
                });
                let camera = stream_post_camera.unwrap_or_default();
                {
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
                        hdr_output: self.hdr_status.active,
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
        timing.gpu_stream_encode = stream_loop_start.elapsed();
        // Stable target view, new pixels. Idle streams skip this list and keep
        // the retained UI raster reusable.
        if let Some(ui) = self.ui.as_mut() {
            for &texture in &rendered_stream_textures {
                ui.note_live_texture_write(texture);
            }
        }
        // Per-stream processors idle-release like the main post. Their chains
        // sit behind the stream idle skip and several early-outs, so tick every
        // live entry once per frame instead of at each skip site; ping targets
        // and blur/bloom scratch otherwise latch at stream resolution forever
        // after a stream's effects are removed.
        for post in self.camera_stream_post.values_mut() {
            post.note_frame(&self.device);
        }

        // Retained-scene fast path. `post.scene_view()` (and the scene depth
        // target) survive across frames, so a frame that provably reproduces
        // the same scene image can leave the whole scene chain out of the
        // encoder and present the retained texture. Present/tonemap, the post
        // chain, UI and the late overlay always still run: the swapchain image
        // is new every frame (concern 1).
        //
        // Under MSAA the scene texture is the RESOLVE target of the mesh/2D
        // passes and `present_scene_bind_group` samples exactly it, so the
        // retained pixels are the resolved ones (concern 3).
        //
        // `post_stage_count` counts the stages that ping-pong scene <->
        // intermediate. One stage reads the scene and writes the intermediate;
        // two or more write BACK into the scene texture, destroying the very
        // pixels the fast path retains, so the gate rejects that case.
        let post_stage_count = u32::from(camera_post_enabled)
            + u32::from(global_post_enabled)
            + u32::from(accessibility_enabled);
        // Same rule the per-stream idle gate uses: an unpaused sky advances its
        // time uniform, and a custom sky shader may read the frame globals even
        // while the sky clock is paused. `scene_continuous_updates` (from the
        // backend) covers the unpaused case and animated draw materials; the
        // shader case is only visible here.
        let sky_animating = lighting_3d
            .sky
            .as_ref()
            .is_some_and(|sky| !sky.time.paused || !sky.shaders.is_empty());
        let retained_scene_key = RetainedSceneKey {
            render_width: self.render_width,
            render_height: self.render_height,
            sample_count: self.sample_count,
            post_view_generation: self.post_view_generation,
            clear_color: [clear_color.r, clear_color.g, clear_color.b, clear_color.a],
            depth_prepass_needed,
            blend_screen_active,
            post_stage_count,
        };
        let mut fast_path_signals = SceneFastPathSignals {
            retained_scene_valid: self.retained_scene_valid,
            retained_key_matches: self.retained_scene_key == retained_scene_key,
            did_prepare_3d,
            three_d_content_changed,
            three_d_dirty,
            did_prepare_2d,
            two_d_scene_changed: needs_2d_prepare && has_2d_content,
            taa_active: taa_run || self.present.taa_active(),
            needs_water,
            needs_particles: needs_3d_particles_path,
            scene_continuous_updates: scene_continuous_updates || sky_animating,
            streams_rendered: false,
            decals_texture_pending,
            post_stage_count,
            camera_image_saves_pending: !self.camera_image_save_requests.is_empty(),
        };
        // Consumer scan only when every other signal already allows the skip,
        // so scenes that re-encode anyway never pay for it. The set is keyed by
        // the three content revisions and additionally dropped on every
        // re-encode frame, so a consecutive run of fast-path frames rebuilds it
        // at most once.
        if !rendered_stream_textures.is_empty() && scene_fast_path_allowed(&fast_path_signals) {
            let sampled_key = (draws_3d_revision, sprites_2d_revision, decals_3d_revision);
            if self.main_scene_sampled_key != Some(sampled_key) {
                collect_main_scene_sampled_texture_slots(
                    &mut self.main_scene_sampled_texture_slots,
                    resources,
                    draws_3d,
                    decals_3d,
                    sprites_2d,
                );
                self.main_scene_sampled_key = Some(sampled_key);
            }
            fast_path_signals.streams_rendered = rendered_stream_textures.iter().any(|texture| {
                self.main_scene_sampled_texture_slots
                    .contains(&texture.index())
            });
        }
        let scene_fast_path = scene_fast_path_allowed(&fast_path_signals);
        // Cleared BEFORE the encode: a frame that renders the scene but then
        // fails to acquire a surface returns without submitting, so its passes
        // never execute. Leaving the flag set there would present the older
        // retained image forever (concern 2). Re-set only after submit.
        if scene_fast_path {
            timing.skip_render_3d = 1;
            timing.skip_render_2d = 1;
        } else {
            self.retained_scene_valid = false;
            // Revision-keyed above, but a material can swap texture slots
            // without moving a revision; that always lands on a re-encode
            // frame, so drop the set here too.
            self.main_scene_sampled_key = None;
        }

        // Water timestamps stay paired every frame so the GPU timer never
        // reads an unwritten query slot. `encode` is a no-op while the water
        // sim holds no active bodies, and the fast path requires no water.
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
        // Shadow timestamps follow the same paired-every-frame rule as water;
        // the 3D view writes them around its shadow block, and the fallback
        // below covers the frames it never runs.
        let shadow_slots = (gpu_timer_active)
            .then(|| self.gpu_timer.as_ref().map(GpuTimestampTimer::shadow_slots))
            .flatten();
        let mut shadow_timestamps_written = false;
        // Same paired-every-frame rule as water/shadow: slots 6/7 bracket the
        // whole 3D block, so `mesh = that - shadow` at harvest. A frame with no
        // 3D view (or a retained-scene frame) writes the pair back to back.
        let mut mesh_timestamps_written = false;
        // ---- scene chain: everything that writes the retained scene texture.
        // Skipped wholesale on a retained-scene frame; `scene_passes_encoded`
        // stays 0 there, which is what the fast-path tests assert on.
        if !scene_fast_path {
            let clear_in_water_pass =
                self.three_d.is_none() && self.two_d.is_some() && !waters_2d.is_empty();
            if let Some(three_d) = self.three_d.as_mut() {
                timing.scene_passes_encoded += 1;
                if gpu_timer_active && let Some(timer) = self.gpu_timer.as_ref() {
                    timer.write_mesh_start(&mut encoder);
                    mesh_timestamps_written = true;
                }
                three_d.render_pass(
                    &self.queue,
                    &mut encoder,
                    color_view,
                    clear_color,
                    depth_prepass_needed,
                    &camera_3d,
                    true,
                    shadow_slots,
                );
                shadow_timestamps_written = three_d.shadow_timestamps_written();
                if gpu_timer_active && let Some(timer) = self.gpu_timer.as_ref() {
                    timer.write_mesh_end(&mut encoder);
                }
                // Seam pass runs on the resolved offscreen scene texture, before
                // particles/water/2D draw on top.
                if blend_screen_active && !direct_present && self.sample_count == 1 {
                    timing.scene_passes_encoded += 1;
                    three_d.mesh_blend_screen_pass(
                        &self.device,
                        &mut encoder,
                        self.post.scene_texture(),
                        &scene_view,
                    );
                }
                if let Some(point_particles_3d_gpu) = self.point_particles_3d.as_mut() {
                    timing.scene_passes_encoded += 1;
                    point_particles_3d_gpu.render_pass(
                        &mut encoder,
                        color_view,
                        three_d.depth_view(),
                    );
                }
                if let Some(water) = self.water.as_mut() {
                    timing.scene_passes_encoded += 1;
                    let clear_water_depth = draws_3d.is_empty()
                        && point_particles_3d.is_empty()
                        && lighting_3d.sky.is_none();
                    let water_depth_view = if waters_3d.is_empty() {
                        // No 3D water: nothing attaches this view (chunk and flip
                        // passes are gated on 3D water counts).
                        three_d.release_water_depth();
                        three_d.depth_view().clone()
                    } else {
                        // Promote the lazily allocated refraction copy target
                        // before the capture that fills it.
                        water.set_scene_color_size(
                            &self.device,
                            three_d.depth_prepass_view(),
                            self.render_width,
                            self.render_height,
                        );
                        // At 1x this is a scene-depth copy: water samples the
                        // prepass view while depth-testing, and the scene depth
                        // target aliases the prepass texture.
                        three_d.water_depth_attachment(&self.device, &mut encoder)
                    };
                    water.capture_scene_color(&self.device, &mut encoder, color_view);
                    water.render_3d(
                        &mut encoder,
                        color_view,
                        &water_depth_view,
                        three_d.water_camera_bind_group(),
                        clear_water_depth,
                    );
                }
            } else if !clear_in_water_pass {
                timing.scene_passes_encoded += 1;
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
                    timing.scene_passes_encoded += 1;
                    water.render_2d(
                        &mut encoder,
                        color_view,
                        (!two_d_draws).then_some(resolve_view).flatten(),
                        two_d.camera_bind_group(),
                        clear_in_water_pass.then_some(clear_color),
                    );
                }
                if two_d_draws {
                    timing.scene_passes_encoded += 1;
                    two_d.render_pass(&mut encoder, color_view, resolve_view, rect_draw_count);
                } else if waters_2d.is_empty()
                    && let Some(resolve_target) = resolve_view
                {
                    timing.scene_passes_encoded += 1;
                    let _resolve_only_pass =
                        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
                timing.scene_passes_encoded += 1;
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
                timing.scene_passes_encoded += 1;
                three_d.mesh_blend_screen_pass(
                    &self.device,
                    &mut encoder,
                    self.post.scene_texture(),
                    &scene_view,
                );
            }
        }
        // ---- end scene chain.
        if !mesh_timestamps_written
            && gpu_timer_active
            && let Some(timer) = self.gpu_timer.as_ref()
        {
            timer.write_mesh_start(&mut encoder);
            timer.write_mesh_end(&mut encoder);
        }
        if !shadow_timestamps_written
            && gpu_timer_active
            && let Some(timer) = self.gpu_timer.as_ref()
        {
            timer.write_shadow_pair(&mut encoder);
        }
        timing.encode_main = encode_start.elapsed();

        let post_start = Instant::now();
        self.post
            .set_constrained(self.max_render_pixels < MAX_FRAME_RENDER_PIXELS);
        #[derive(Clone, Copy)]
        enum FrameTex {
            Scene,
            Intermediate,
        }
        let mut current_tex = FrameTex::Scene;
        let post_view_generation = self.post_view_generation;
        // Opened before the post closure borrows the encoder; closed just
        // before the frame end marker, so the pair spans post + UI + present.
        if gpu_timer_active && let Some(timer) = self.gpu_timer.as_ref() {
            timer.write_post_start(&mut encoder);
        }
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
                hdr_output: self.hdr_status.active,
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
        if !camera_post_enabled && !global_post_enabled {
            // Promoted ping / bloom scratch targets otherwise latch full-res
            // after the last effect is removed.
            self.post.note_idle_frame(&self.device);
        }
        timing.post_process = post_start.elapsed();

        let accessibility_start = Instant::now();
        if accessibility_enabled {
            let (accessibility_input_view, accessibility_output_view, next_tex) = match current_tex
            {
                FrameTex::Scene => (&scene_view, &intermediate_view, FrameTex::Intermediate),
                FrameTex::Intermediate => (&intermediate_view, &scene_view, FrameTex::Scene),
            };
            if let Some(processor) = self.accessibility.as_mut() {
                processor.apply(
                    &self.device,
                    &self.queue,
                    &mut encoder,
                    accessibility_input_view,
                    accessibility_output_view,
                    accessibility,
                );
                current_tex = next_tex;
            }
        }
        timing.accessibility = accessibility_start.elapsed();

        // TAA resolve inputs: UNJITTERED current/previous view-proj (same
        // rule the 3D prepare uses) + scene depth. At 1x — the only sample
        // count TAA runs at — the depth prepass texture IS the scene depth
        // target, so by this point it holds final opaque depth with no copy.
        let taa_frame = if taa_run {
            let current_view_proj = crate::three_d::gpu::compute_view_proj_mat(
                &camera_3d,
                self.render_width,
                self.render_height,
            );
            let prev_view_proj = self.taa_prev_view_proj.unwrap_or(current_view_proj);
            self.taa_prev_view_proj = Some(current_view_proj);
            let inv = current_view_proj.inverse();
            self.three_d.as_ref().map(|three_d| PresentTaaFrame {
                depth_view: three_d.depth_prepass_view().clone(),
                inv_view_proj: if inv.is_finite() { inv } else { Mat4::IDENTITY }
                    .to_cols_array_2d(),
                prev_view_proj: prev_view_proj.to_cols_array_2d(),
            })
        } else {
            self.taa_prev_view_proj = None;
            None
        };

        // ---- idle-frame skip. Everything below (acquire, encode, submit,
        // present) reproduces the image already on screen when these hold, so
        // the cheapest correct frame is no frame at all. See
        // `idle_frame_skip_allowed`.
        let ui_viewport = [self.config.width.max(1), self.config.height.max(1)];
        let ui_idle = ui_textures_delta.is_empty()
            && !ui_primitives.is_empty()
            && self
                .ui
                .as_ref()
                .is_some_and(|ui| ui.composite_is_idle(ui_viewport, ui_revision));
        let idle_signals = IdleFrameSignals {
            scene_fast_path,
            ui_idle,
            late_overlay_empty: late_overlay_upload_2d.draw_count == 0
                && late_overlay_sprites_2d.is_empty()
                && late_overlay_point_lights_2d.is_empty()
                && late_overlay_rects_2d.is_empty(),
            presented_once: self.presented_once,
            within_force_interval: self
                .last_present
                .is_some_and(|at| at.elapsed() < IDLE_FORCE_PRESENT_INTERVAL),
        };
        if idle_frame_skip_allowed(&idle_signals) {
            timing.idle_frame_skips = 1;
            timing.total = total_start.elapsed();
            return timing;
        }
        if !direct_present && !msaa_direct_present {
            let acquire_start = Instant::now();
            let acquire_surface_start = Instant::now();
            // Acquired before the bind-group borrow: the retry path re-configures
            // the surface and needs &mut self.
            let Some(acquired) = self.acquire_surface_texture() else {
                timing.acquire_surface = acquire_surface_start.elapsed();
                timing.acquire = acquire_start.elapsed();
                timing.total = total_start.elapsed();
                return timing;
            };
            timing.acquire_surface = acquire_surface_start.elapsed();
            let acquire_view_start = Instant::now();
            let view = acquired.texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(self.surface_view_format),
                ..Default::default()
            });
            timing.acquire_view = acquire_view_start.elapsed();
            timing.acquire = acquire_start.elapsed();
            let final_bind_group = match current_tex {
                FrameTex::Scene => &self.present_scene_bind_group,
                FrameTex::Intermediate => self
                    .present_intermediate_bind_group
                    .as_ref()
                    .expect("intermediate frame needs present bind group"),
            };
            self.present.apply(
                &self.queue,
                &mut encoder,
                final_bind_group,
                &view,
                [self.render_width, self.render_height],
                frame_delta_seconds,
                exposure_settings,
                self.hdr_status,
                taa_frame.as_ref(),
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
                ui.set_max_render_pixels(self.max_render_pixels);
                ui.prepare(
                    &self.device,
                    &self.queue,
                    UiPrepareInput {
                        resources,
                        shared_textures: &mut self.shared_textures,
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
                    self.three_d.as_ref().map(|three_d| {
                        (
                            three_d.depth_prepass_view(),
                            three_d.depth_prepass_view_generation(),
                        )
                    }),
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
                        shared_textures: &mut self.shared_textures,
                        camera: late_overlay_camera_2d,
                        rects: late_overlay_rects_2d,
                        upload: late_overlay_upload_2d,
                        sprites: late_overlay_sprites_2d,
                        sprites_revision: late_overlay_sprites_2d_revision,
                        force_sprite_prepare: has(DIRTY_RESOURCES),
                        point_lights: late_overlay_point_lights_2d,
                        point_lights_revision: late_overlay_point_lights_2d_revision,
                        shadow_casters: late_overlay_shadow_casters_2d,
                        shadow_casters_revision: late_overlay_shadow_casters_2d_revision,
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
            // Post pair closes immediately before the frame end marker, so it
            // spans the post chain + UI + tonemap/present tail.
            timer.write_post_end(&mut encoder);
            timer.write_end_and_resolve(&mut encoder);
        }
        if let Some(water) = self.water.as_mut() {
            water.encode_readback(&mut encoder);
        }
        self.encode_camera_image_saves(&mut encoder);
        let submit_start = Instant::now();
        let submit_finish_start = Instant::now();
        let command_buffer = encoder.finish();
        timing.submit_finish_main = submit_finish_start.elapsed();
        let submit_queue_start = Instant::now();
        self.queue.submit(Some(command_buffer));
        self.request_camera_image_save_maps();
        if gpu_timer_active && let Some(timer) = self.gpu_timer.as_mut() {
            timer.request_readback();
        }
        if let Some(water) = self.water.as_mut() {
            water.finish_frame();
            water.request_readback();
        }
        timing.submit_queue_main = submit_queue_start.elapsed();
        timing.submit_main = submit_start.elapsed();
        // The scene chain (or the retained texture it already produced) is now
        // on the queue: the retained image describes this key from here on.
        // Anchors the idle-skip safety valve. Set on the SUBMIT path only, so a
        // frame that failed to acquire never counts as presented.
        self.last_present = Some(Instant::now());
        self.presented_once = true;
        self.retained_scene_valid = true;
        self.retained_scene_key = retained_scene_key;
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
        // Pass/triangle counters only mean anything on a frame that re-encoded:
        // `render_pass` resets them at entry, so a fast-path frame still holds
        // the last encoded frame's numbers. Report zero there instead of a
        // stale repeat -- `skip_render_3d` already flags the frame.
        if !scene_fast_path && let Some(three_d) = self.three_d.as_ref() {
            timing.draw_triangles_3d = three_d.triangle_count();
            let counters = three_d.pass_counters();
            timing.scene_render_passes = counters.render_passes;
            timing.sky_draws = counters.sky_draws;
            timing.mesh_blend_seam_passes = counters.mesh_blend_seam_passes;
            timing.mesh_blend_scene_copies = counters.mesh_blend_scene_copies;
            timing.mesh_blend_copy_pixels = counters.mesh_blend_copy_pixels;
            timing.mesh_blend_source_depth_passes = counters.mesh_blend_source_depth_passes;
            timing.mesh_blend_source_depth_reuses = counters.mesh_blend_source_depth_reuses;
            timing.water_depth_copies = counters.water_depth_copies;
            timing.water_depth_clears = counters.water_depth_clears;
            timing.shadow_layer_renders = counters.shadow_layer_renders;
            timing.shadow_regular_batch_draws = counters.shadow_regular_batch_draws;
            timing.shadow_multimesh_batch_draws = counters.shadow_multimesh_batch_draws;
            timing.shadow_multimesh_instance_draws = counters.shadow_multimesh_instance_draws;
            timing.shadow_multimesh_culled_layers = counters.shadow_multimesh_culled_layers;
            timing.shadow_empty_layer_skips = counters.shadow_empty_layer_skips;
            timing.shadow_multiview_passes = counters.shadow_multiview_passes;
        }
        let present_start = Instant::now();
        if let Some(frame) = frame {
            self.queue.present(frame);
            timing.present = present_start.elapsed();
            timing.presented = true;
        }
        // Periodic refcount sweep: shared uploads whose consumer handles are
        // all gone (stream teardown, invalidation, filter-mode change) free
        // after a short grace instead of lingering for the session.
        self.shared_texture_frame_counter = self.shared_texture_frame_counter.wrapping_add(1);
        if self
            .shared_texture_frame_counter
            .is_multiple_of(SHARED_TEXTURE_SWEEP_INTERVAL_FRAMES)
        {
            self.shared_textures.sweep();
        }
        timing.total = total_start.elapsed();
        timing
    }

    /// Periodic GC tick (driven by the backend's GC interval): give every
    /// grow-only GPU buffer owner a chance to shrink back toward its current
    /// usage. One heavy scene otherwise pins the high-water mark for the whole
    /// session, which is brutal on shared-memory iGPUs.
    pub fn shrink_gpu_buffers_tick(&mut self) {
        let device = self.device.clone();
        let queue = self.queue.clone();
        // Shared by every Gpu3D, so it shrinks and raises its compaction
        // request exactly once; views adopt the result at their next prepare.
        self.mesh_arena.shrink_tick(&device, &queue);
        self.mesh_arena.reclaim_tick();
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.shrink_tick(&device, &queue);
            three_d.reclaim_memory_tick(&device);
        }
        if let Some(two_d) = self.two_d.as_mut() {
            two_d.shrink_tick(&device, &queue);
        }
        if let Some(late_overlay_2d) = self.late_overlay_2d.as_mut() {
            late_overlay_2d.shrink_tick(&device, &queue);
        }
        if let Some(particles) = self.point_particles_3d.as_mut() {
            particles.shrink_tick(&device, &queue);
        }
        if let Some(water) = self.water.as_mut() {
            water.shrink_tick(&device, &queue);
        }
        if let Some(ui) = self.ui.as_mut() {
            ui.shrink_tick(&device, &queue);
        }
        for three_d in self.camera_stream_3d.values_mut() {
            three_d.shrink_tick(&device, &queue);
            three_d.reclaim_memory_tick(&device);
            // Stream-only: the per-frame idle gate skips unchanged, non-animated
            // streams entirely, so their shadow atlases (and lazy mesh-blend
            // targets) sit fully grown behind a view that encodes nothing.
            // ~80MB per stream at 2 spot + 2 point casters. The main view above
            // renders every frame and is deliberately excluded.
            three_d.note_stream_gc_tick(&device);
        }
        for two_d in self.camera_stream_2d.values_mut() {
            two_d.shrink_tick(&device, &queue);
        }
        for particles in self.camera_stream_particles_3d.values_mut() {
            particles.shrink_tick(&device, &queue);
        }
        for water in self.camera_stream_water.values_mut() {
            water.shrink_tick(&device, &queue);
        }
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
}
