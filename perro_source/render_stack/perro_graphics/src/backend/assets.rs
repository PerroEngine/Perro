use super::*;

// load a custom shader source (static pack, else asset io) + probe 4 frame
// globals. `perro_time` prefix also covers `perro_time_phase`.
fn custom_shader_reads_frame_globals(
    shader_path: &str,
    static_shader_lookup: Option<StaticShaderLookup>,
) -> bool {
    let probe = |src: &str| {
        src.contains("perro_time")
            || src.contains("perro_delta_time")
            || src.contains("perro_frame_index")
    };
    if let Some(lookup) = static_shader_lookup {
        let hash = perro_ids::parse_hashed_source_uri(shader_path)
            .unwrap_or_else(|| perro_ids::string_to_u64(shader_path));
        let src = lookup(hash);
        if !src.is_empty() {
            return probe(src);
        }
    }
    match perro_io::load_asset(shader_path) {
        Ok(bytes) => match std::str::from_utf8(&bytes) {
            Ok(src) => probe(src),
            Err(_) => true,
        },
        // unreadable source: assume animated (= old always-redraw behavior).
        Err(_) => true,
    }
}

impl PerroGraphics {
    // true while pipeline warming still has work to drain: queued materials or
    // the shared registry's base families. both need drawn frames, so the frame
    // pump + the startup-splash exit gate must read the same predicate or one
    // stalls the other. gated on the lazy 3D world existing - warming is a
    // no-op w/o it + a 2D-only session must not spin.
    pub(super) fn pipeline_warm_pending(&self) -> bool {
        self.gpu.as_ref().is_some_and(|gpu| {
            let main_post_requested = crate::postprocess::PostProcessor::has_effects(
                self.renderer_3d.camera().post_processing.as_ref(),
            ) || crate::postprocess::PostProcessor::has_effects(
                self.renderer_2d.camera().post_processing.as_ref(),
            ) || crate::postprocess::PostProcessor::has_effects(
                self.global_post_processing_cache.as_ref(),
            ) || !self.retained_waters_3d_cache.is_empty();
            (gpu.has_three_d()
                && (!self.pending_pipeline_warms.is_empty() || gpu.base_families_pending()))
                || gpu.post_pipelines_pending(main_post_requested, &self.retained_camera_streams)
        })
    }

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

    // true when a retained draw uses a custom shader that reads the frame
    // globals (perro_time/perro_time_phase/perro_delta_time/perro_frame_index)
    // and so needs continuous redraw. probe result cached per shader path;
    // unreadable sources count as animated (conservative = old behavior).
    // whole answer memoized on (draw revision, material revision): those 2 are
    // the only inputs, so idle frames never re-walk the draw list.
    fn has_retained_animated_custom_material(&mut self) -> bool {
        let draws_revision = self.renderer_3d.draw_revision();
        let material_revision = self.resources.material_revision();
        if let Some((draws, materials, animated)) = self.retained_animated_material_memo
            && draws == draws_revision
            && materials == material_revision
        {
            return animated;
        }
        let cache = &mut self.custom_shader_animated_cache;
        let lookup = self.static_shader_lookup;
        let animated =
            self.renderer_3d
                .any_retained_custom_material_where(&self.resources, |shader_path| {
                    let key = perro_ids::string_to_u64(shader_path);
                    *cache
                        .entry(key)
                        .or_insert_with(|| custom_shader_reads_frame_globals(shader_path, lookup))
                });
        self.retained_animated_material_memo = Some((draws_revision, material_revision, animated));
        animated
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
        if self.frame_index.is_multiple_of(60)
            && let Some(gpu) = self.gpu.as_mut()
        {
            let old_hdr = gpu.hdr_status();
            let new_hdr = gpu.set_hdr_mode(self.hdr_mode);
            if new_hdr != old_hdr {
                self.events.push(RenderEvent::HdrStatusChanged(new_hdr));
                self.redraw_requested = true;
            }
        }
        let total_start = Instant::now();
        // Tick even when this draw takes an idle early-out. Pipeline compiles
        // often finish on the last active frame; the following idle frames are
        // the settle window that makes one cache write cover the whole burst.
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.poll_idle_maintenance();
        }
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
        let has_late_overlay_commands = !late_overlay_pending.is_empty();
        let has_late_overlay = has_late_overlay_commands
            || self.late_overlay_2d.retained_sprite_count() > 0
            || !self.late_overlay_2d.retained_rects().is_empty();
        // A budgeted warm queue drains a few materials per frame, so the frame
        // pump has to stay awake until it empties or a scene that loads and
        // then sits still would leave its pipelines uncompiled - handing the
        // compile back to the first visible draw, which is the spike the budget
        // exists to spread. Gated on the 3D world existing: warming is a no-op
        // without it, and a 2D-only session that creates materials must not
        // spin forever.
        let has_pending_pipeline_warms = self.pipeline_warm_pending();
        let auto_exposure = |effects: &[perro_structs::PostProcessEffect]| {
            effects.iter().any(|effect| {
                matches!(
                    effect,
                    perro_structs::PostProcessEffect::Exposure {
                        auto_exposure: true,
                        ..
                    }
                )
            })
        };
        let material_revision = self.resources.material_revision();
        let stream_continuous = self.retained_camera_streams.iter().any(|(node, stream)| {
            let dynamic_source = matches!(stream.source, CameraStreamSourceState::Webcam { .. })
                || !stream.waters_2d.is_empty()
                || !stream.waters_3d.is_empty()
                || !stream.point_particles_2d.is_empty()
                || !stream.point_particles_3d.is_empty()
                || auto_exposure(stream.post_processing.as_ref())
                || stream
                    .lighting_3d
                    .sky
                    .as_ref()
                    .is_some_and(|sky| !sky.time.paused || !sky.shaders.is_empty());
            let animated_material = self
                .animated_stream_memo
                .get(node)
                .filter(|(key, revision, _)| {
                    *key == Arc::as_ptr(stream) as usize && *revision == material_revision
                })
                .is_none_or(|(_, _, animated)| *animated);
            dynamic_source || animated_material
        });
        let has_continuous_updates = self.renderer_3d.has_active_sky_animation()
            || has_pending_pipeline_warms
            || self.has_retained_animated_custom_material()
            || self.taa_enabled
            || self.renderer_2d.retained_water_count() > 0
            || self.late_overlay_2d.retained_water_count() > 0
            || self.renderer_2d.retained_point_particle_count() > 0
            || self.late_overlay_2d.retained_point_particle_count() > 0
            || self.particles_3d.retained_point_particle_count() > 0
            || auto_exposure(self.renderer_3d.camera().post_processing.as_ref())
            || auto_exposure(self.renderer_2d.camera().post_processing.as_ref())
            || self.global_post_processing.effects().any(|effect| {
                matches!(
                    effect,
                    perro_structs::PostProcessEffect::Exposure {
                        auto_exposure: true,
                        ..
                    }
                )
            })
            || stream_continuous;
        let has_retained_scene = self.renderer_2d.retained_sprite_count() > 0
            || !self.renderer_2d.retained_rects().is_empty()
            || has_late_overlay
            || self.renderer_2d.retained_water_count() > 0
            || self.renderer_ui.retained_count() > 0
            || self.renderer_3d.retained_draw_count() > 0
            || self.renderer_3d.has_retained_non_draw_state()
            || self.particles_3d.retained_point_particle_count() > 0;
        if !has_pending && !has_retained_scene && !has_pending_pipeline_warms {
            let mut presented = false;
            if self.redraw_requested
                && let Some(gpu) = &mut self.gpu
            {
                presented = gpu.render_idle_clear();
                self.redraw_requested = !presented;
            }
            self.frame.scratch_late_overlay_commands = late_overlay_pending;
            return Some(DrawFrameTiming {
                presented,
                total: total_start.elapsed(),
                idle_clear: true,
                ..DrawFrameTiming::default()
            });
        }
        if !has_pending
            && !has_late_overlay_commands
            && !has_continuous_updates
            && !self.redraw_requested
        {
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
        let mut frame_dirty_bits = command_summary.dirty_bits;
        let process_start = Instant::now();
        self.reserve_command_buckets(&command_summary);
        let camera_2d_before = self.renderer_2d.camera();
        let camera_3d_before = self.renderer_3d.camera();
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
        frame_dirty_bits = clear_unchanged_camera_dirty_bits(
            frame_dirty_bits,
            &camera_2d_before,
            &self.renderer_2d.camera(),
            &camera_3d_before,
            &self.renderer_3d.camera(),
        );
        let process_commands = process_start.elapsed();
        // Runtime camera extraction may resend byte-identical cameras every
        // tick. Once those bits clear, a static scene needs no GPU acquire,
        // submit, or present; keep CPU simulation alive and retain the last
        // swapchain image. Continuous effects above veto this path.
        if frame_dirty_bits == 0
            && !has_late_overlay_commands
            && !has_continuous_updates
            && !self.redraw_requested
        {
            pending.clear();
            self.frame.scratch_commands = pending;
            return Some(DrawFrameTiming {
                process_commands,
                total: total_start.elapsed(),
                idle_clear: true,
                ..DrawFrameTiming::default()
            });
        }
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
        let late_overlay_retained_rect_count = self.late_overlay_2d.retained_rects().len();
        let late_overlay_frame_shape_count = self.late_overlay_2d.frame_shapes().len();
        let late_overlay_total_rect_count =
            late_overlay_retained_rect_count + late_overlay_frame_shape_count;
        if late_overlay_frame_shape_count == 0
            && !late_overlay_upload.full_reupload
            && late_overlay_upload.dirty_ranges.is_empty()
            && self.late_overlay_rects_cache.len() == late_overlay_retained_rect_count
        {
            // Retained rect buffer already mirrors late-overlay renderer state.
        } else {
            self.late_overlay_rects_cache.clear();
            if self.late_overlay_rects_cache.capacity() < late_overlay_total_rect_count {
                self.late_overlay_rects_cache.reserve(
                    late_overlay_total_rect_count - self.late_overlay_rects_cache.capacity(),
                );
            }
            self.late_overlay_rects_cache
                .extend_from_slice(self.late_overlay_2d.retained_rects());
            self.late_overlay_rects_cache
                .extend_from_slice(self.late_overlay_2d.frame_shapes());
        }
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
        // Revision covers both caster maps behind `shadow_casters()`
        // (retained casters + tilemap casters): every mutation path bumps it,
        // so an unchanged revision means an unchanged snapshot.
        let late_overlay_shadow_casters_revision =
            self.late_overlay_2d.retained_shadow_casters_revision();
        if late_overlay_shadow_casters_revision != self.late_overlay_shadow_casters_cache_revision {
            self.late_overlay_shadow_casters_cache.clear();
            self.late_overlay_shadow_casters_cache
                .extend(self.late_overlay_2d.shadow_casters());
            self.late_overlay_shadow_casters_cache_revision = late_overlay_shadow_casters_revision;
        }
        // sizes depend only on the retained ui set + texture dims/sources, so
        // an unchanged pair means the last pass' sizes still hold. w/o this the
        // walk did a store lookup + '#' split + .svg scan per nine-slice per
        // frame.
        let nine_slice_key = (
            self.renderer_ui.revision(),
            self.resources.texture_dims_revision(),
        );
        if self.nine_slice_sizes_memo != Some(nine_slice_key) {
            let resources = &self.resources;
            self.renderer_ui.set_nine_slice_texture_sizes(|texture| {
                let Some(data) = resources.decoded_texture_data(texture) else {
                    return [0, 0];
                };
                let mut size = [data.width, data.height];
                let source = resources.texture_source(texture).unwrap_or_default();
                let path = source.split('#').next().unwrap_or_default();
                let svg = path
                    .get(path.len().saturating_sub(4)..)
                    .is_some_and(|ext| ext.eq_ignore_ascii_case(".svg"));
                if source.eq_ignore_ascii_case("__perro_builtin_logo_svg__") || svg {
                    size = [
                        (size[0] / SVG_RASTER_SCALE).max(1),
                        (size[1] / SVG_RASTER_SCALE).max(1),
                    ];
                }
                size
            });
            // a size change bumps the ui revision; re-read so the memo names
            // the state the sizes were written into.
            self.nine_slice_sizes_memo = Some((self.renderer_ui.revision(), nine_slice_key.1));
        }
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

        // stream upserts add/remove stream output textures referenced by ui
        // images + sprites, so DIRTY_STREAMS gates the recount too.
        if sprites_refs_changed
            || draws_refs_changed
            || (frame_dirty_bits & (DIRTY_RESOURCES | DIRTY_STREAMS)) != 0
        {
            self.resources.reset_ref_counts();
            for (texture, count) in &self.used_texture_refs_cache {
                self.resources.mark_texture_used_count(*texture, *count);
            }
            for (texture, nodes) in &self.scene_texture_refs_cache {
                self.resources
                    .mark_texture_used_count(*texture, nodes.len().min(u32::MAX as usize) as u32);
            }
            for texture in self.renderer_ui.image_textures() {
                self.resources.mark_texture_used(texture);
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
            // shrink grow-only GPU buffers back toward current usage.
            if let Some(gpu) = self.gpu.as_mut() {
                gpu.shrink_gpu_buffers_tick();
            }
            // reclaim idle CPU pixel copies (GPU copies stay); consumers
            // re-decode from source on the rare later re-upload.
            let _ = self
                .resources
                .evict_idle_decoded_textures(DECODED_TEXTURE_EVICT_TTL_TICKS);
            let drops = self.resources.gc_unused_after_frames(
                ResourceStore::DEFAULT_ZERO_REF_TTL_FRAMES,
                GC_INTERVAL_FRAMES,
                GC_MAX_DROPS_PER_KIND,
            );
            // Auto-dropped ids can never be invalidated later, so their GPU
            // uploads (3D material slots, UI images, decal layers, 2D sprites)
            // would pin the shared store forever. Sources are already gone from
            // the store; the id-keyed handles are what retain the upload, and
            // the shared store sweeps once its refcount falls back to 1.
            if !drops.textures.is_empty()
                && let Some(gpu) = self.gpu.as_mut()
            {
                for id in &drops.textures {
                    gpu.invalidate_texture(*id, None);
                }
            }
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
        let ui_paint = self
            .renderer_ui
            .prepare_paint([self.viewport.0 as f32, self.viewport.1 as f32]);
        let prepare_cpu = prepare_start.elapsed();

        if self.global_post_processing_cache_dirty {
            self.global_post_processing_cache =
                Arc::from(self.global_post_processing.to_effects_vec());
            self.global_post_processing_cache_dirty = false;
        }

        let mut gpu_timing = RenderGpuTiming::default();
        let mut pipeline_compiles = 0_u32;
        // streams whose 3D draws use time-reading custom shaders must
        // re-render every frame; the rest may idle-skip inside gpu.render.
        let mut animated_streams = std::mem::take(&mut self.animated_stream_nodes_scratch);
        animated_streams.clear();
        {
            let cache = &mut self.custom_shader_animated_cache;
            let memo = &mut self.animated_stream_memo;
            let lookup = self.static_shader_lookup;
            let material_revision = self.resources.material_revision();
            for (node, stream) in &self.retained_camera_streams {
                // Arc ptr identity: an unchanged retained state means unchanged
                // draws + surfaces, so the probe result cannot have moved.
                let stream_key = Arc::as_ptr(stream) as usize;
                if let Some((key, materials, animated)) = memo.get(node).copied()
                    && key == stream_key
                    && materials == material_revision
                {
                    if animated {
                        animated_streams.insert(*node);
                    }
                    continue;
                }
                let animated = stream.draws_3d.iter().any(|draw| {
                    let surfaces = match draw {
                        perro_render_bridge::CameraStreamDraw3DState::Draw { surfaces, .. }
                        | perro_render_bridge::CameraStreamDraw3DState::DrawMulti {
                            surfaces,
                            ..
                        }
                        | perro_render_bridge::CameraStreamDraw3DState::DrawMultiDense {
                            surfaces,
                            ..
                        } => surfaces,
                        perro_render_bridge::CameraStreamDraw3DState::CameraStreamQuad {
                            ..
                        } => return false,
                    };
                    surfaces.iter().any(|surface| {
                        surface
                            .material
                            .and_then(|material| self.resources.custom_shader_path(material))
                            .is_some_and(|path| {
                                let key = perro_ids::string_to_u64(path);
                                *cache.entry(key).or_insert_with(|| {
                                    custom_shader_reads_frame_globals(path, lookup)
                                })
                            })
                    })
                });
                memo.insert(*node, (stream_key, material_revision, animated));
                if animated {
                    animated_streams.insert(*node);
                }
            }
            // drop memo rows 4 streams that left; only runs when a stream is
            // actually removed.
            if memo.len() > self.retained_camera_streams.len() {
                let live = &self.retained_camera_streams;
                memo.retain(|node, _| live.iter().any(|(id, _)| id == node));
            }
        }
        if let Some(gpu) = &mut self.gpu {
            // compile pipelines 4 materials that arrived this frame while
            // their meshes/textures still load async => first draw skips
            // shader compile. no-op drain until the 3d pipeline exists.
            // budgeted: a scene switch queues every material at once + the
            // whole drain in one frame was the transition spike.
            // splash-covered frames afford a bigger burst (nothing visible
            // stalls); steady-state kp the small budget.
            let (warm_max, warm_budget) = if self.startup_warm_boost {
                (
                    PIPELINE_WARM_BOOST_MAX_COMPILES_PER_FRAME,
                    PIPELINE_WARM_BOOST_TIME_BUDGET,
                )
            } else {
                (
                    PIPELINE_WARM_MAX_COMPILES_PER_FRAME,
                    PIPELINE_WARM_TIME_BUDGET,
                )
            };
            let main_post_requested =
                crate::postprocess::PostProcessor::has_effects(camera_3d.post_processing.as_ref())
                    || crate::postprocess::PostProcessor::has_effects(
                        camera_2d_state.post_processing.as_ref(),
                    )
                    || crate::postprocess::PostProcessor::has_effects(
                        self.global_post_processing_cache.as_ref(),
                    )
                    || !self.retained_waters_3d_cache.is_empty();
            let warm_start = Instant::now();
            let post_warmed = gpu.warm_post_pipelines(
                main_post_requested,
                &self.retained_camera_streams,
                warm_max,
                Some(warm_budget),
            );
            gpu.warm_material_pipelines(
                &mut self.pending_pipeline_warms,
                self.static_shader_lookup,
                warm_max.saturating_sub(post_warmed),
                Some(warm_budget.saturating_sub(warm_start.elapsed())),
            );
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
                shadow_casters_2d_revision: self.retained_shadow_casters_cache_revision,
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
                late_overlay_shadow_casters_2d_revision: self
                    .late_overlay_shadow_casters_cache_revision,
                ui_primitives: ui_paint.primitives,
                ui_primitive_depths: ui_paint.primitive_depths,
                ui_textures_delta: ui_paint.textures_delta,
                ui_texture_size: ui_paint.texture_size,
                ui_revision: ui_paint.revision,
                frame_time_seconds: self.frame_time_seconds,
                frame_delta_seconds: self.frame_delta_seconds,
                frame_dirty_bits,
                static_texture_lookup: self.static_texture_lookup,
                static_mesh_lookup: self.static_mesh_lookup,
                static_shader_lookup: self.static_shader_lookup,
                animated_stream_nodes: &animated_streams,
                changed_stream_nodes: &self.camera_stream_states_changed,
                scene_continuous_updates: has_continuous_updates,
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
            // Read after render so lazy first-draw compiles land in the same
            // frame's count as the warm-queue ones.
            let compiles_now = gpu.pipeline_compiles_3d();
            pipeline_compiles = compiles_now
                .wrapping_sub(self.last_pipeline_compiles_3d)
                .min(u32::MAX as u64) as u32;
            self.last_pipeline_compiles_3d = compiles_now;
            self.redraw_requested = !gpu_timing.presented;
            if gpu_timing.presented {
                // changed streams re-rendered this frame; next frame they can
                // idle-skip again. kept on non-present so nothing is lost.
                self.camera_stream_states_changed.clear();
            }
        }
        self.animated_stream_nodes_scratch = animated_streams;
        let timing = DrawFrameTiming {
            presented: gpu_timing.presented,
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
            gpu_timestamp_shadow: gpu_timing.gpu_timestamp_shadow,
            gpu_timestamp_mesh: gpu_timing.gpu_timestamp_mesh,
            gpu_timestamp_post: gpu_timing.gpu_timestamp_post,
            draw_calls_2d: gpu_timing.draw_calls_2d,
            draw_calls_3d: gpu_timing.draw_calls_3d,
            sprite_batches_2d: gpu_timing.sprite_batches_2d,
            sprite_bind_group_switches_2d: gpu_timing.sprite_bind_group_switches_2d,
            draw_batches_3d: gpu_timing.draw_batches_3d,
            pipeline_compiles_3d: pipeline_compiles,
            pipeline_warms_pending_3d: self.pending_pipeline_warms.len().min(u32::MAX as usize)
                as u32,
            pipeline_switches_3d: gpu_timing.pipeline_switches_3d,
            texture_bind_group_switches_3d: gpu_timing.texture_bind_group_switches_3d,
            draw_instances_3d: self.retained_draw_instances_cache,
            draw_triangles_3d: gpu_timing.draw_triangles_3d,
            draw_material_refs_3d: self.used_material_refs_cache.len().min(u32::MAX as usize)
                as u32,
            skip_prepare_2d: gpu_timing.skip_prepare_2d,
            skip_prepare_3d: gpu_timing.skip_prepare_3d,
            skip_prepare_particles_3d: gpu_timing.skip_prepare_particles_3d,
            skip_prepare_3d_frustum: gpu_timing.skip_prepare_3d_frustum,
            skip_prepare_3d_hiz: gpu_timing.skip_prepare_3d_hiz,
            skip_prepare_3d_indirect: gpu_timing.skip_prepare_3d_indirect,
            skip_prepare_3d_cull_inputs: gpu_timing.skip_prepare_3d_cull_inputs,
            skip_render_3d: gpu_timing.skip_render_3d,
            skip_render_2d: gpu_timing.skip_render_2d,
            scene_passes_encoded: gpu_timing.scene_passes_encoded,
            scene_render_passes: gpu_timing.scene_render_passes,
            sky_draws: gpu_timing.sky_draws,
            mesh_blend_seam_passes: gpu_timing.mesh_blend_seam_passes,
            mesh_blend_scene_copies: gpu_timing.mesh_blend_scene_copies,
            mesh_blend_copy_pixels: gpu_timing.mesh_blend_copy_pixels,
            mesh_blend_source_depth_passes: gpu_timing.mesh_blend_source_depth_passes,
            mesh_blend_source_depth_reuses: gpu_timing.mesh_blend_source_depth_reuses,
            water_depth_copies: gpu_timing.water_depth_copies,
            water_depth_clears: gpu_timing.water_depth_clears,
            shadow_layer_renders: gpu_timing.shadow_layer_renders,
            shadow_regular_batch_draws: gpu_timing.shadow_regular_batch_draws,
            shadow_multimesh_batch_draws: gpu_timing.shadow_multimesh_batch_draws,
            shadow_multimesh_instance_draws: gpu_timing.shadow_multimesh_instance_draws,
            shadow_multimesh_culled_layers: gpu_timing.shadow_multimesh_culled_layers,
            shadow_empty_layer_skips: gpu_timing.shadow_empty_layer_skips,
            shadow_multiview_passes: gpu_timing.shadow_multiview_passes,
            stream_count: gpu_timing.stream_count,
            stream_renders: gpu_timing.stream_renders,
            gpu_stream_encode: gpu_timing.gpu_stream_encode,
            stream_pixels: gpu_timing.stream_pixels,
            stream_draw_calls_3d: gpu_timing.stream_draw_calls_3d,
            stream_draw_batches_3d: gpu_timing.stream_draw_batches_3d,
            stream_draw_triangles_3d: gpu_timing.stream_draw_triangles_3d,
            stream_render_passes: gpu_timing.stream_render_passes,
            stream_shadow_layer_renders: gpu_timing.stream_shadow_layer_renders,
            gpu_total: gpu_timing.total,
            total: total_start.elapsed(),
            idle_clear: false,
        };
        pending.clear();
        self.frame.scratch_commands = pending;
        Some(timing)
    }
}
