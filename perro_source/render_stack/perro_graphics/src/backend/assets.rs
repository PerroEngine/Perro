use super::*;

impl PerroGraphics {
    pub(super) fn reserve_command_buckets(&mut self, summary: &CommandSummary) {
        if summary.rects_2d > 0 {
            self.renderer_2d.reserve_queued_rects(summary.rects_2d);
        }
        if summary.sprites_2d > 0 {
            self.renderer_2d.reserve_queued_sprites(summary.sprites_2d);
        }
        if summary.draws_3d > 0 {
            self.renderer_3d.reserve_queued_draws(summary.draws_3d);
        }
    }

    pub(super) fn draw_frame_timed_internal<I>(
        &mut self,
        late_overlay_commands: I,
    ) -> Option<DrawFrameTiming>
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        #[cfg(target_arch = "wasm32")]
        self.try_finish_gpu_init();
        let total_start = Instant::now();
        self.poll_async_mesh_loads();
        self.poll_async_texture_loads();
        let now = Instant::now();
        self.frame_delta_seconds = self
            .last_frame_instant
            .map(|prev| now.duration_since(prev).as_secs_f32())
            .unwrap_or(0.0)
            .max(0.0);
        self.last_frame_instant = Some(now);
        self.frame_time_seconds =
            (self.frame_time_seconds + self.frame_delta_seconds).rem_euclid(1.0e9);
        let mut late_overlay_pending =
            std::mem::take(&mut self.frame.scratch_late_overlay_commands);
        late_overlay_pending.clear();
        late_overlay_pending.extend(late_overlay_commands);
        let has_pending = !self.frame.pending_commands.is_empty();
        let has_late_overlay = !late_overlay_pending.is_empty()
            || self.late_overlay_2d.retained_sprite_count() > 0
            || !self.late_overlay_2d.retained_rects().is_empty();
        let has_continuous_updates = self.renderer_3d.has_active_sky_animation();
        let has_retained_scene = self.renderer_2d.retained_sprite_count() > 0
            || !self.renderer_2d.retained_rects().is_empty()
            || has_late_overlay
            || self.renderer_2d.retained_water_count() > 0
            || self.renderer_ui.retained_count() > 0
            || self.renderer_3d.retained_draw_count() > 0
            || self.renderer_3d.has_retained_non_draw_state()
            || self.particles_3d.retained_point_particle_count() > 0;
        if !has_pending && !has_retained_scene {
            if self.redraw_requested
                && let Some(gpu) = &mut self.gpu
            {
                self.redraw_requested = !gpu.render_idle_clear();
            }
            self.frame.scratch_late_overlay_commands = late_overlay_pending;
            return Some(DrawFrameTiming {
                total: total_start.elapsed(),
                idle_clear: true,
                ..DrawFrameTiming::default()
            });
        }
        if !has_pending && !has_continuous_updates && !self.redraw_requested {
            self.frame.scratch_late_overlay_commands = late_overlay_pending;
            return Some(DrawFrameTiming {
                total: total_start.elapsed(),
                idle_clear: true,
                ..DrawFrameTiming::default()
            });
        }
        let mut pending = std::mem::take(&mut self.frame.scratch_commands);
        pending.clear();
        std::mem::swap(&mut pending, &mut self.frame.pending_commands);
        let pending_command_count = pending.len();
        let command_summary = summarize_commands(&pending);
        let frame_dirty_bits = command_summary.dirty_bits;
        let process_start = Instant::now();
        self.reserve_command_buckets(&command_summary);
        let mut camera_commands = std::mem::take(&mut self.frame.scratch_camera_commands);
        camera_commands.clear();
        let mut write = 0usize;
        for read in 0..pending.len() {
            let is_camera_command = match &pending[read] {
                RenderCommand::TwoD(Command2D::SetCamera { .. }) => true,
                RenderCommand::ThreeD(cmd) => {
                    matches!(cmd.as_ref(), Command3D::SetCamera { .. })
                }
                _ => false,
            };
            if is_camera_command {
                camera_commands.push(pending[read].clone());
            } else {
                if read != write {
                    pending.swap(write, read);
                }
                write += 1;
            }
        }
        pending.truncate(write);
        self.process_commands(camera_commands.drain(..));
        self.process_commands(pending.drain(..));
        self.frame.scratch_camera_commands = camera_commands;
        self.process_late_overlay_commands(late_overlay_pending.drain(..));
        self.frame.scratch_late_overlay_commands = late_overlay_pending;
        let process_commands = process_start.elapsed();
        let prepare_start = Instant::now();
        let (
            (camera_2d, _stats, upload),
            (late_overlay_camera_2d, _late_overlay_stats, late_overlay_upload),
            (camera_3d, _stats_3d, lighting_3d),
        ) = if pending_command_count >= PARALLEL_RENDER_PREPARE_MIN {
            let resources = &self.resources;
            let renderer_2d = &mut self.renderer_2d;
            let late_overlay_2d = &mut self.late_overlay_2d;
            let renderer_3d = &mut self.renderer_3d;
            let particles_3d = &mut self.particles_3d;
            let ((main_2d, late_overlay), main_3d) = rayon::join(
                || {
                    let main_2d = renderer_2d.prepare_frame(resources);
                    let late_overlay = late_overlay_2d.prepare_frame(resources);
                    (main_2d, late_overlay)
                },
                || {
                    let main_3d = renderer_3d.prepare_frame(resources);
                    particles_3d.prepare_frame();
                    main_3d
                },
            );
            (main_2d, late_overlay, main_3d)
        } else {
            let main_2d = self.renderer_2d.prepare_frame(&self.resources);
            let late_overlay = self.late_overlay_2d.prepare_frame(&self.resources);
            let main_3d = self.renderer_3d.prepare_frame(&self.resources);
            self.particles_3d.prepare_frame();
            (main_2d, late_overlay, main_3d)
        };
        let camera_2d_state = self.renderer_2d.camera();
        let draws_revision = self.renderer_3d.draw_revision();
        let point_particles_revision = self.particles_3d.retained_point_particles_revision();
        if point_particles_revision != self.retained_point_particles_cache_revision {
            self.retained_point_particles_cache.clear();
            let point_particles_count = self.particles_3d.retained_point_particle_count();
            if self.retained_point_particles_cache.capacity() < point_particles_count {
                self.retained_point_particles_cache.reserve(
                    point_particles_count - self.retained_point_particles_cache.capacity(),
                );
            }
            self.retained_point_particles_cache
                .extend(self.particles_3d.retained_point_particles());
            self.retained_point_particles_cache
                .sort_unstable_by_key(|(node, _)| node.as_u64());
            self.retained_point_particles_cache_revision = point_particles_revision;
        }
        let waters_3d_revision = self.renderer_3d.retained_waters_revision();
        if waters_3d_revision != self.retained_waters_3d_cache_revision {
            self.retained_waters_3d_cache.clear();
            self.retained_waters_3d_cache
                .extend_from_slice(self.renderer_3d.retained_waters_sorted());
            self.retained_waters_3d_cache_revision = waters_3d_revision;
        }
        let decals_3d_revision = self.renderer_3d.retained_decals_revision();
        if decals_3d_revision != self.retained_decals_3d_cache_revision {
            self.retained_decals_3d_cache.clear();
            self.retained_decals_3d_cache
                .extend_from_slice(self.renderer_3d.retained_decals_sorted());
            self.retained_decals_3d_cache_revision = decals_3d_revision;
        }
        let retained_draws_3d = self.renderer_3d.retained_draws_sorted();
        if draws_revision != self.retained_draws_cache_revision {
            self.retained_draw_instances_cache =
                retained_draws_3d.iter().fold(0u32, |acc, draw| {
                    acc.saturating_add(draw_instance_count(draw))
                });
            self.retained_draws_cache_revision = draws_revision;
        }
        let waters_2d_revision = self.renderer_2d.retained_waters_revision();
        if waters_2d_revision != self.retained_waters_2d_cache_revision {
            self.retained_waters_2d_cache.clear();
            let water_count = self.renderer_2d.retained_water_count();
            if self.retained_waters_2d_cache.capacity() < water_count {
                self.retained_waters_2d_cache
                    .reserve(water_count - self.retained_waters_2d_cache.capacity());
            }
            self.retained_waters_2d_cache
                .extend(self.renderer_2d.retained_waters());
            self.retained_waters_2d_cache
                .sort_unstable_by_key(|(node, _)| node.as_u64());
            self.retained_waters_2d_cache_revision = waters_2d_revision;
        }
        let sprites_revision = self.renderer_2d.retained_sprites_revision();
        if sprites_revision != self.retained_sprites_cache_revision {
            self.retained_sprites_cache.clear();
            let sprite_count = self.renderer_2d.retained_sprite_count();
            if self.retained_sprites_cache.capacity() < sprite_count {
                self.retained_sprites_cache
                    .reserve(sprite_count - self.retained_sprites_cache.capacity());
            }
            self.retained_sprites_cache
                .extend(self.renderer_2d.retained_sprites());
            self.retained_sprites_cache_revision = sprites_revision;
        }
        let point_lights_revision = self.renderer_2d.retained_point_lights_revision();
        if point_lights_revision != self.retained_point_lights_cache_revision {
            self.retained_point_lights_cache.clear();
            let point_light_count = self.renderer_2d.light_count();
            if self.retained_point_lights_cache.capacity() < point_light_count {
                self.retained_point_lights_cache
                    .reserve(point_light_count - self.retained_point_lights_cache.capacity());
            }
            self.retained_point_lights_cache
                .extend(self.renderer_2d.lights());
            self.retained_point_lights_cache_revision = point_lights_revision;
        }
        let shadow_casters_revision = self.renderer_2d.retained_shadow_casters_revision();
        if shadow_casters_revision != self.retained_shadow_casters_cache_revision {
            self.retained_shadow_casters_cache.clear();
            self.retained_shadow_casters_cache
                .extend(self.renderer_2d.shadow_casters());
            self.retained_shadow_casters_cache_revision = shadow_casters_revision;
        }
        let retained_rect_count = self.renderer_2d.retained_rects().len();
        let frame_shape_count = self.renderer_2d.frame_shapes().len();
        let total_rect_count = retained_rect_count + frame_shape_count;
        if frame_shape_count == 0
            && !upload.full_reupload
            && upload.dirty_ranges.is_empty()
            && self.frame_rects_cache.len() == retained_rect_count
        {
            // Retained rect buffer already mirrors renderer state.
        } else {
            self.frame_rects_cache.clear();
            if self.frame_rects_cache.capacity() < total_rect_count {
                self.frame_rects_cache
                    .reserve(total_rect_count - self.frame_rects_cache.capacity());
            }
            self.frame_rects_cache
                .extend_from_slice(self.renderer_2d.retained_rects());
            self.frame_rects_cache
                .extend_from_slice(self.renderer_2d.frame_shapes());
        }
        self.late_overlay_rects_cache.clear();
        self.late_overlay_rects_cache
            .extend_from_slice(self.late_overlay_2d.retained_rects());
        self.late_overlay_rects_cache
            .extend_from_slice(self.late_overlay_2d.frame_shapes());
        let late_overlay_sprites_revision = self.late_overlay_2d.retained_sprites_revision();
        if late_overlay_sprites_revision != self.late_overlay_sprites_cache_revision {
            self.late_overlay_sprites_cache.clear();
            let sprite_count = self.late_overlay_2d.retained_sprite_count();
            if self.late_overlay_sprites_cache.capacity() < sprite_count {
                self.late_overlay_sprites_cache
                    .reserve(sprite_count - self.late_overlay_sprites_cache.capacity());
            }
            self.late_overlay_sprites_cache
                .extend(self.late_overlay_2d.retained_sprites());
            self.late_overlay_sprites_cache_revision = late_overlay_sprites_revision;
        }
        let late_overlay_point_lights_revision =
            self.late_overlay_2d.retained_point_lights_revision();
        if late_overlay_point_lights_revision != self.late_overlay_point_lights_cache_revision {
            self.late_overlay_point_lights_cache.clear();
            let point_light_count = self.late_overlay_2d.light_count();
            if self.late_overlay_point_lights_cache.capacity() < point_light_count {
                self.late_overlay_point_lights_cache
                    .reserve(point_light_count - self.late_overlay_point_lights_cache.capacity());
            }
            self.late_overlay_point_lights_cache
                .extend(self.late_overlay_2d.lights());
            self.late_overlay_point_lights_cache_revision = late_overlay_point_lights_revision;
        }
        self.late_overlay_shadow_casters_cache.clear();
        self.late_overlay_shadow_casters_cache
            .extend(self.late_overlay_2d.shadow_casters());
        let ui_image_textures: Vec<_> = self.renderer_ui.image_textures().collect();
        let ui_texture_sizes = ui_image_textures
            .iter()
            .filter_map(|texture| {
                self.resources.decoded_texture_data(*texture).map(|data| {
                    let mut size = [data.width, data.height];
                    let source = self.resources.texture_source(*texture).unwrap_or_default();
                    if source.eq_ignore_ascii_case("__perro_builtin_logo_svg__")
                        || source
                            .split('#')
                            .next()
                            .is_some_and(|path| path.to_ascii_lowercase().ends_with(".svg"))
                    {
                        size = [
                            (size[0] / SVG_RASTER_SCALE).max(1),
                            (size[1] / SVG_RASTER_SCALE).max(1),
                        ];
                    }
                    (*texture, size)
                })
            })
            .collect();
        self.renderer_ui
            .set_nine_slice_texture_sizes(&ui_texture_sizes);
        let ui_paint = self
            .renderer_ui
            .prepare_paint([self.viewport.0 as f32, self.viewport.1 as f32]);
        let sprites_refs_changed = self.used_ref_sprites_revision != sprites_revision;
        if sprites_refs_changed {
            self.used_texture_refs_cache.clear();
            self.used_texture_refs_cache
                .reserve(self.retained_sprites_cache.len());
            for sprite in &self.retained_sprites_cache {
                *self
                    .used_texture_refs_cache
                    .entry(sprite.texture)
                    .or_insert(0) += 1;
            }
            self.used_ref_sprites_revision = sprites_revision;
        }
        let draws_refs_changed = self.used_ref_draws_revision != draws_revision;
        if draws_refs_changed {
            self.used_mesh_refs_cache.clear();
            self.used_material_refs_cache.clear();
            self.used_mesh_refs_cache.reserve(retained_draws_3d.len());
            self.used_material_refs_cache
                .reserve(retained_draws_3d.len());
            for draw in retained_draws_3d {
                if let Draw3DKind::Mesh(mesh) = draw.kind {
                    *self.used_mesh_refs_cache.entry(mesh).or_insert(0) += 1;
                }
                for material in draw.surfaces.iter().filter_map(|surface| surface.material) {
                    *self.used_material_refs_cache.entry(material).or_insert(0) += 1;
                }
            }
            self.used_ref_draws_revision = draws_revision;
        }

        if sprites_refs_changed || draws_refs_changed || (frame_dirty_bits & DIRTY_RESOURCES) != 0 {
            self.resources.reset_ref_counts();
            for (texture, count) in &self.used_texture_refs_cache {
                self.resources.mark_texture_used_count(*texture, *count);
            }
            for (texture, nodes) in &self.scene_texture_refs_cache {
                self.resources
                    .mark_texture_used_count(*texture, nodes.len().min(u32::MAX as usize) as u32);
            }
            for texture in &ui_image_textures {
                self.resources.mark_texture_used(*texture);
            }
            for (mesh, count) in &self.used_mesh_refs_cache {
                self.resources.mark_mesh_used_count(*mesh, *count);
            }
            for (mesh, nodes) in &self.scene_mesh_refs_cache {
                self.resources
                    .mark_mesh_used_count(*mesh, nodes.len().min(u32::MAX as usize) as u32);
            }
            for (material, count) in &self.used_material_refs_cache {
                self.resources.mark_material_used_count(*material, *count);
            }
            for (material, nodes) in &self.scene_material_refs_cache {
                self.resources
                    .mark_material_used_count(*material, nodes.len().min(u32::MAX as usize) as u32);
            }
        }
        self.frame_index = self.frame_index.wrapping_add(1);
        if self.frame_index.is_multiple_of(GC_INTERVAL_FRAMES) {
            let drops = self.resources.gc_unused_after_frames(
                ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES,
                GC_INTERVAL_FRAMES,
                GC_MAX_DROPS_PER_KIND,
            );
            self.events.extend(
                drops
                    .textures
                    .into_iter()
                    .map(|id| RenderEvent::TextureDropped { id }),
            );
            self.events.extend(
                drops
                    .meshes
                    .into_iter()
                    .map(|id| RenderEvent::MeshDropped { id }),
            );
            self.events.extend(
                drops
                    .materials
                    .into_iter()
                    .map(|id| RenderEvent::MaterialDropped { id }),
            );
        }
        let prepare_cpu = prepare_start.elapsed();

        if self.global_post_processing_cache_dirty {
            self.global_post_processing_cache =
                Arc::from(self.global_post_processing.to_effects_vec());
            self.global_post_processing_cache_dirty = false;
        }

        let mut gpu_timing = RenderGpuTiming::default();
        if let Some(gpu) = &mut self.gpu {
            gpu_timing = gpu.render(RenderFrame {
                resources: &self.resources,
                camera_3d,
                lighting_3d: &lighting_3d,
                draws_3d: retained_draws_3d,
                draws_3d_revision: draws_revision,
                point_particles_3d: &self.retained_point_particles_cache,
                point_particles_3d_revision: self.retained_point_particles_cache_revision,
                waters_3d: &self.retained_waters_3d_cache,
                waters_3d_revision: self.retained_waters_3d_cache_revision,
                decals_3d: &self.retained_decals_3d_cache,
                decals_3d_revision: self.retained_decals_3d_cache_revision,
                camera_streams: &self.retained_camera_streams,
                camera_2d,
                camera_2d_position: camera_2d_state.position,
                post_processing_2d: camera_2d_state.post_processing,
                post_processing_global: self.global_post_processing_cache.clone(),
                accessibility: self.accessibility,
                rects_2d: &self.frame_rects_cache,
                upload_2d: &upload,
                sprites_2d: &self.retained_sprites_cache,
                sprites_2d_revision: self.retained_sprites_cache_revision,
                point_lights_2d: &self.retained_point_lights_cache,
                point_lights_2d_revision: self.retained_point_lights_cache_revision,
                shadow_casters_2d: &self.retained_shadow_casters_cache,
                waters_2d: &self.retained_waters_2d_cache,
                waters_2d_revision: self.retained_waters_2d_cache_revision,
                late_overlay_camera_2d,
                late_overlay_rects_2d: &self.late_overlay_rects_cache,
                late_overlay_upload_2d: &late_overlay_upload,
                late_overlay_sprites_2d: &self.late_overlay_sprites_cache,
                late_overlay_sprites_2d_revision: self.late_overlay_sprites_cache_revision,
                late_overlay_point_lights_2d: &self.late_overlay_point_lights_cache,
                late_overlay_point_lights_2d_revision: self
                    .late_overlay_point_lights_cache_revision,
                late_overlay_shadow_casters_2d: &self.late_overlay_shadow_casters_cache,
                ui_primitives: ui_paint.primitives,
                ui_textures_delta: ui_paint.textures_delta,
                ui_texture_size: ui_paint.texture_size,
                ui_revision: ui_paint.revision,
                redraw_requested: self.redraw_requested,
                frame_time_seconds: self.frame_time_seconds,
                frame_delta_seconds: self.frame_delta_seconds,
                frame_dirty_bits,
                static_texture_lookup: self.static_texture_lookup,
                static_mesh_lookup: self.static_mesh_lookup,
                static_shader_lookup: self.static_shader_lookup,
            });
            let mut water_samples = Vec::new();
            gpu.drain_water_samples(&mut water_samples);
            if !water_samples.is_empty() {
                self.events.push(RenderEvent::WaterSamples {
                    samples: Arc::from(water_samples.into_boxed_slice()),
                });
            }
            let mut water_body_samples = Vec::new();
            gpu.drain_water_body_samples(&mut water_body_samples);
            if !water_body_samples.is_empty() {
                self.events.push(RenderEvent::WaterBodySamples {
                    samples: Arc::from(water_body_samples.into_boxed_slice()),
                });
            }
            self.redraw_requested = !gpu_timing.presented;
        }
        let timing = DrawFrameTiming {
            process_commands,
            prepare_cpu,
            gpu_prepare_2d: gpu_timing.prepare_2d,
            gpu_prepare_3d: gpu_timing.prepare_3d,
            gpu_prepare_particles_3d: gpu_timing.prepare_particles_3d,
            gpu_prepare_3d_frustum: gpu_timing.prepare_3d_frustum,
            gpu_prepare_3d_hiz: gpu_timing.prepare_3d_hiz,
            gpu_prepare_3d_indirect: gpu_timing.prepare_3d_indirect,
            gpu_prepare_3d_cull_inputs: gpu_timing.prepare_3d_cull_inputs,
            gpu_acquire: gpu_timing.acquire,
            gpu_acquire_surface: gpu_timing.acquire_surface,
            gpu_acquire_view: gpu_timing.acquire_view,
            gpu_encode_main: gpu_timing.encode_main,
            gpu_submit_main: gpu_timing.submit_main,
            gpu_submit_finish_main: gpu_timing.submit_finish_main,
            gpu_submit_queue_main: gpu_timing.submit_queue_main,
            gpu_post_process: gpu_timing.post_process,
            gpu_accessibility: gpu_timing.accessibility,
            gpu_present: gpu_timing.present,
            gpu_timestamp_main: gpu_timing.gpu_timestamp_main,
            gpu_timestamp_water: gpu_timing.gpu_timestamp_water,
            draw_calls_2d: gpu_timing.draw_calls_2d,
            draw_calls_3d: gpu_timing.draw_calls_3d,
            sprite_batches_2d: gpu_timing.sprite_batches_2d,
            sprite_bind_group_switches_2d: gpu_timing.sprite_bind_group_switches_2d,
            draw_batches_3d: gpu_timing.draw_batches_3d,
            pipeline_switches_3d: gpu_timing.pipeline_switches_3d,
            texture_bind_group_switches_3d: gpu_timing.texture_bind_group_switches_3d,
            draw_instances_3d: self.retained_draw_instances_cache,
            draw_material_refs_3d: self.used_material_refs_cache.len().min(u32::MAX as usize)
                as u32,
            skip_prepare_2d: gpu_timing.skip_prepare_2d,
            skip_prepare_3d: gpu_timing.skip_prepare_3d,
            skip_prepare_particles_3d: gpu_timing.skip_prepare_particles_3d,
            skip_prepare_3d_frustum: gpu_timing.skip_prepare_3d_frustum,
            skip_prepare_3d_hiz: gpu_timing.skip_prepare_3d_hiz,
            skip_prepare_3d_indirect: gpu_timing.skip_prepare_3d_indirect,
            skip_prepare_3d_cull_inputs: gpu_timing.skip_prepare_3d_cull_inputs,
            gpu_total: gpu_timing.total,
            total: total_start.elapsed(),
            idle_clear: false,
        };
        pending.clear();
        self.frame.scratch_commands = pending;
        Some(timing)
    }
}
