use super::*;

impl<B: GraphicsBackend> RunnerState<B> {
    pub(super) fn startup_splash_overlay_commands(&mut self, alpha: f32) -> Vec<RenderCommand> {
        let alpha = alpha.clamp(0.0, 1.0);
        if let Some(result) = self
            .app
            .runtime
            .take_render_result(STARTUP_SPLASH_TEXTURE_REQUEST)
        {
            match result {
                perro_runtime::RuntimeRenderResult::Texture(id) => {
                    self.startup_splash.texture_id = Some(id);
                }
                perro_runtime::RuntimeRenderResult::Failed(_) => {
                    self.startup_splash.texture_requested = false;
                }
                perro_runtime::RuntimeRenderResult::Mesh(_)
                | perro_runtime::RuntimeRenderResult::Material(_) => {}
            }
        }
        let fallback_width = self
            .app
            .runtime
            .project()
            .map(|project| project.config.virtual_width.max(1))
            .unwrap_or(1920) as f32;
        let fallback_height = self
            .app
            .runtime
            .project()
            .map(|project| project.config.virtual_height.max(1))
            .unwrap_or(1080) as f32;
        let (window_width, window_height) = self
            .window
            .as_ref()
            .map(|window| window.inner_size())
            .map(|size| (size.width.max(1) as f32, size.height.max(1) as f32))
            .unwrap_or((fallback_width, fallback_height));

        let mut commands = Vec::with_capacity(3);
        commands.push(RenderCommand::TwoD(Command2D::SetCamera {
            camera: Camera2DState::default(),
        }));
        commands.push(RenderCommand::TwoD(Command2D::UpsertRect {
            node: STARTUP_SPLASH_BG_NODE,
            rect: Rect2DCommand {
                center: [0.0, 0.0],
                size: [window_width, window_height],
                color: [
                    STARTUP_SPLASH_BG_COLOR[0],
                    STARTUP_SPLASH_BG_COLOR[1],
                    STARTUP_SPLASH_BG_COLOR[2],
                    STARTUP_SPLASH_BG_COLOR[3] * alpha,
                ]
                .into(),
                z_index: STARTUP_SPLASH_BG_Z,
            },
        }));

        if !self.startup_splash.texture_requested
            && let Some(source) = self.startup_splash.source.clone()
        {
            self.startup_splash.texture_requested = true;
            commands.push(RenderCommand::Resource(ResourceCommand::CreateTexture {
                request: STARTUP_SPLASH_TEXTURE_REQUEST,
                id: TextureID::nil(),
                source: self
                    .startup_splash
                    .source_hash
                    .map(|v| v.to_string())
                    .unwrap_or(source),
                reserved: true,
            }));
        }

        let Some(texture_id) = self.startup_splash.texture_id else {
            return commands;
        };
        let (image_w, image_h) = self.startup_splash.image_size.unwrap_or((512, 512));
        let (texture_w, texture_h) = self
            .startup_splash
            .texture_size
            .unwrap_or((image_w, image_h));
        let max_w = window_width * STARTUP_SPLASH_MAX_WIDTH_FRAC;
        let max_h = window_height * STARTUP_SPLASH_MAX_HEIGHT_FRAC;
        let scale = (max_w / image_w as f32)
            .min(max_h / image_h as f32)
            .max(0.001);
        commands.push(RenderCommand::TwoD(Command2D::UpsertSprite {
            node: STARTUP_SPLASH_IMAGE_NODE,
            sprite: Sprite2DCommand {
                texture: texture_id,
                model: [[scale, 0.0, 0.0], [0.0, scale, 0.0], [0.0, 0.0, 1.0]],
                tint: [1.0, 1.0, 1.0, alpha].into(),
                z_index: STARTUP_SPLASH_IMAGE_Z,
                uv_min: [0.0, 0.0],
                uv_max: [texture_w as f32, texture_h as f32],
                uv_normalized: false,
                size: [image_w as f32, image_h as f32],
            },
        }));
        commands
    }

    pub(super) fn end_startup_splash(&mut self) {
        self.app.graphics.submit_late_overlay_many([
            RenderCommand::TwoD(Command2D::RemoveNode {
                node: STARTUP_SPLASH_BG_NODE,
            }),
            RenderCommand::TwoD(Command2D::RemoveNode {
                node: STARTUP_SPLASH_IMAGE_NODE,
            }),
        ]);
        self.startup_splash.active = false;
        self.timing_warmup_frames_left = TIMING_WARMUP_FRAMES;
        self.batch_start = Instant::now();
        self.batch = BatchCoreStats::default();
    }

    pub(super) fn step_startup_frame(
        &mut self,
        event_loop: &ActiveEventLoop,
        frame_index: u64,
        frame_start: Instant,
        frame_delta: Duration,
        idle_duration: Duration,
    ) {
        let should_sample_timing = self.should_sample_timing();
        #[cfg(feature = "profile_heavy")]
        let work_start = Instant::now();
        #[cfg(not(feature = "profile_heavy"))]
        let work_start = should_sample_timing.then(Instant::now);
        let mut runtime_update_duration = Duration::ZERO;

        #[cfg(feature = "profile_heavy")]
        let simulation_start = Instant::now();
        #[cfg(not(feature = "profile_heavy"))]
        let simulation_start = should_sample_timing.then(Instant::now);
        #[cfg(feature = "profile_heavy")]
        let input_poll_start = Instant::now();
        #[cfg(not(feature = "profile_heavy"))]
        let input_poll_start = should_sample_timing.then(Instant::now);
        self.gamepad_input.begin_frame(&mut self.app);
        self.joycon_input.begin_frame(&mut self.app);
        #[cfg(feature = "profile_heavy")]
        let input_poll_duration = input_poll_start.elapsed();
        #[cfg(not(feature = "profile_heavy"))]
        let input_poll_duration = input_poll_start
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);
        #[cfg(feature = "profile_heavy")]
        let fixed_start = Instant::now();
        #[cfg(not(feature = "profile_heavy"))]
        let fixed_start = should_sample_timing.then(Instant::now);

        let fixed_accumulator_before = self.fixed_accumulator;
        let mut fixed_steps = 1u32;
        let mut fixed_step_seconds = frame_delta.as_secs_f32();
        let mut fixed_catchup_dropped = false;
        let simulated_delta_seconds = {
            if let Some(effective_fixed_step) = self.fixed_timestep {
                let plan = plan_fixed_steps(
                    frame_delta.as_secs_f32(),
                    effective_fixed_step,
                    self.fixed_accumulator,
                );
                fixed_steps = plan.steps;
                fixed_step_seconds = plan.step_seconds;
                fixed_catchup_dropped = plan.dropped_catchup;
                for _ in 0..plan.steps {
                    #[cfg(feature = "profile_heavy")]
                    {
                        let timing = self.app.fixed_update_runtime_timed(effective_fixed_step);
                        runtime_update_duration += timing.total;
                        self.batch_heavy.fixed_snapshot_update += timing.snapshot_update;
                        self.batch_heavy.fixed_script_update += timing.script_fixed_update;
                        self.batch_heavy.fixed_physics_update += timing.physics;
                        self.batch_heavy.fixed_internal_update += timing.internal_fixed_update;
                        self.batch_heavy.fixed_physics_pre_transforms +=
                            timing.physics_pre_transforms;
                        self.batch_heavy.fixed_physics_collect += timing.physics_collect;
                        self.batch_heavy.fixed_physics_sync_world += timing.physics_sync_world;
                        self.batch_heavy.fixed_physics_apply_forces_impulses +=
                            timing.physics_apply_forces_impulses;
                        self.batch_heavy.fixed_physics_step += timing.physics_step;
                        self.batch_heavy.fixed_physics_sync_nodes += timing.physics_sync_nodes;
                        self.batch_heavy.fixed_physics_post_transforms +=
                            timing.physics_post_transforms;
                        self.batch_heavy.fixed_physics_signals += timing.physics_signals;
                    }
                    #[cfg(not(feature = "profile_heavy"))]
                    {
                        let update_start = Instant::now();
                        self.app.fixed_update_runtime(effective_fixed_step);
                        runtime_update_duration += update_start.elapsed();
                    }
                }
                self.fixed_accumulator = plan.accumulator_after;
                self.app.set_physics_render_alpha(
                    (self.fixed_accumulator / effective_fixed_step).clamp(0.0, 1.0),
                );
                effective_fixed_step as f64 * plan.steps as f64
            } else {
                let variable_step = frame_delta.as_secs_f32();
                #[cfg(feature = "profile_heavy")]
                {
                    let timing = self.app.fixed_update_runtime_timed(variable_step);
                    runtime_update_duration += timing.total;
                    self.batch_heavy.fixed_snapshot_update += timing.snapshot_update;
                    self.batch_heavy.fixed_script_update += timing.script_fixed_update;
                    self.batch_heavy.fixed_physics_update += timing.physics;
                    self.batch_heavy.fixed_internal_update += timing.internal_fixed_update;
                    self.batch_heavy.fixed_physics_pre_transforms += timing.physics_pre_transforms;
                    self.batch_heavy.fixed_physics_collect += timing.physics_collect;
                    self.batch_heavy.fixed_physics_sync_world += timing.physics_sync_world;
                    self.batch_heavy.fixed_physics_apply_forces_impulses +=
                        timing.physics_apply_forces_impulses;
                    self.batch_heavy.fixed_physics_step += timing.physics_step;
                    self.batch_heavy.fixed_physics_sync_nodes += timing.physics_sync_nodes;
                    self.batch_heavy.fixed_physics_post_transforms +=
                        timing.physics_post_transforms;
                    self.batch_heavy.fixed_physics_signals += timing.physics_signals;
                }
                #[cfg(not(feature = "profile_heavy"))]
                {
                    let update_start = Instant::now();
                    self.app.fixed_update_runtime(variable_step);
                    runtime_update_duration += update_start.elapsed();
                }
                self.app.set_physics_render_alpha(1.0);
                variable_step as f64
            }
        };

        #[cfg(feature = "profile_heavy")]
        let fixed_duration = fixed_start.elapsed();
        #[cfg(not(feature = "profile_heavy"))]
        let fixed_duration = fixed_start
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);
        let runtime_timing = self.app.update_runtime(frame_delta.as_secs_f32());
        runtime_update_duration += runtime_timing.total;
        self.apply_mouse_mode_request();
        self.apply_cursor_icon_request();
        self.apply_window_requests(event_loop);
        #[cfg(feature = "profile_heavy")]
        let simulation_duration = simulation_start.elapsed();
        #[cfg(not(feature = "profile_heavy"))]
        let simulation_duration = simulation_start
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);
        #[cfg(not(feature = "profile_heavy"))]
        let _ = (
            runtime_update_duration,
            input_poll_duration,
            fixed_duration,
            runtime_timing,
            simulated_delta_seconds,
        );

        let alpha = self.startup_splash.alpha(frame_start);
        let splash_overlay = self.startup_splash_overlay_commands(alpha);
        #[cfg(feature = "profile_heavy")]
        let present_timing = self.app.present_with_overlay_timed_no_ui(splash_overlay);
        #[cfg(not(feature = "profile_heavy"))]
        let present_timing = if should_sample_timing {
            Some(self.app.present_with_overlay_timed_no_ui(splash_overlay))
        } else {
            self.app.present_with_overlay_no_ui(splash_overlay);
            None
        };
        self.apply_cursor_icon_request();
        let mut inflight_now = Vec::<RenderRequestID>::new();
        self.app
            .runtime
            .copy_inflight_render_requests(&mut inflight_now);
        if !self.startup_splash.first_frame_captured {
            self.startup_splash
                .first_frame_inflight
                .extend(inflight_now.iter().copied());
            self.startup_splash.first_frame_captured = true;
        }
        #[cfg(feature = "profile_heavy")]
        let work_duration = work_start.elapsed();
        #[cfg(not(feature = "profile_heavy"))]
        let work_duration = work_start
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);
        #[cfg(feature = "profile_heavy")]
        let present_wait_duration = present_timing.gpu_present;
        #[cfg(not(feature = "profile_heavy"))]
        let present_wait_duration = present_timing
            .as_ref()
            .map(|timing| timing.gpu_present)
            .unwrap_or(Duration::ZERO);
        #[cfg(feature = "profile_heavy")]
        let present_active_duration = present_timing.active;
        #[cfg(not(feature = "profile_heavy"))]
        let present_active_duration = present_timing
            .as_ref()
            .map(|timing| timing.active)
            .unwrap_or(Duration::ZERO);
        let active_work_duration = work_duration.saturating_sub(present_wait_duration);
        let measured_frame_duration = active_work_duration
            .saturating_add(idle_duration)
            .saturating_add(present_wait_duration);
        let frame_end = Instant::now();
        self.last_frame_end = frame_end;
        // Splash frames pace at refresh (or a slower explicit cap): no reason
        // to burn a core rendering an uncapped static image.
        self.pacer.update_deadline(frame_start, frame_end, true);
        if should_sample_timing {
            self.app.set_frame_timing(
                simulation_duration,
                present_active_duration,
                measured_frame_duration,
            );
            #[cfg(feature = "profile_heavy")]
            self.app.set_present_timing_profile(&present_timing);
        }

        let warmup_frame = self.timing_warmup_frames_left > 0;
        if !warmup_frame {
            self.batch.frames = self.batch.frames.saturating_add(1);
            if should_sample_timing {
                self.batch.timing_samples = self.batch.timing_samples.saturating_add(1);
                self.batch.work += active_work_duration;
                self.batch.simulation += simulation_duration;
                self.batch.present += present_active_duration;
                self.batch.idle_before_frame += idle_duration;
                self.batch.present_wait += present_wait_duration;
                self.batch.idle += idle_duration + present_wait_duration;
            }
        }
        #[cfg(all(feature = "ui_profile", not(feature = "profile_heavy")))]
        if !warmup_frame && let Some(timing) = present_timing.as_ref() {
            self.batch_ui.extract_ui += timing.extract_ui;
            self.batch_ui.layout += timing.ui_layout;
            self.batch_ui.commands += timing.ui_commands;
            self.batch_ui.dirty_nodes += timing.ui_dirty_nodes as u64;
            self.batch_ui.affected_nodes += timing.ui_affected_nodes as u64;
            self.batch_ui.recalculated_rects += timing.ui_recalculated_rects as u64;
            self.batch_ui.cached_rects += timing.ui_cached_rects as u64;
            self.batch_ui.auto_layout_batches += timing.ui_auto_layout_batches as u64;
            self.batch_ui.command_nodes += timing.ui_command_nodes as u64;
            self.batch_ui.command_emitted += timing.ui_command_emitted as u64;
            self.batch_ui.command_skipped += timing.ui_command_skipped as u64;
            self.batch_ui.removed_nodes += timing.ui_removed_nodes as u64;
        }
        if frame_index != 1
            && let Some(csv) = &mut self.timing_csv
        {
            csv.write(CsvFrameSample {
                frame_index,
                phase: "startup",
                warmup: true,
                sampled: should_sample_timing,
                frame_delta_us: frame_delta.as_micros(),
                idle_before_frame_us: idle_duration.as_micros(),
                simulation_us: simulation_duration.as_micros(),
                render_active_us: present_active_duration.as_micros(),
                work_active_us: active_work_duration.as_micros(),
                present_wait_us: present_wait_duration.as_micros(),
                fixed_steps,
                fixed_step_us: Duration::from_secs_f32(fixed_step_seconds).as_micros(),
                fixed_accum_before_us: Duration::from_secs_f32(fixed_accumulator_before)
                    .as_micros(),
                fixed_accum_after_us: Duration::from_secs_f32(self.fixed_accumulator).as_micros(),
                fixed_catchup_dropped,
                timestamp_ms: unix_timestamp_ms(),
            });
        }
        #[cfg(feature = "profile_heavy")]
        if !warmup_frame {
            self.batch_heavy.runtime_update += runtime_update_duration;
            self.batch_heavy.input_poll += input_poll_duration;
            self.batch_heavy.fixed_update += fixed_duration;
            self.batch_heavy.runtime_start_schedule += runtime_timing.start_schedule;
            self.batch_heavy.runtime_snapshot_update += runtime_timing.snapshot_update;
            self.batch_heavy.runtime_script_update += runtime_timing.update_schedule.scripts_total;
            self.batch_heavy.runtime_internal_update += runtime_timing.internal_update;
            self.batch_heavy.runtime_script_count +=
                runtime_timing.update_schedule.script_count as u64;
            if runtime_timing.update_schedule.slowest_script
                > self.batch_heavy.runtime_slowest_script
            {
                self.batch_heavy.runtime_slowest_script =
                    runtime_timing.update_schedule.slowest_script;
            }
            self.batch_heavy.present_extract_2d += present_timing.extract_2d;
            self.batch_heavy.present_extract_3d += present_timing.extract_3d;
            self.batch_ui.extract_ui += present_timing.extract_ui;
            self.batch_ui.layout += present_timing.ui_layout;
            self.batch_ui.commands += present_timing.ui_commands;
            self.batch_ui.dirty_nodes += present_timing.ui_dirty_nodes as u64;
            self.batch_ui.affected_nodes += present_timing.ui_affected_nodes as u64;
            self.batch_ui.recalculated_rects += present_timing.ui_recalculated_rects as u64;
            self.batch_ui.cached_rects += present_timing.ui_cached_rects as u64;
            self.batch_ui.auto_layout_batches += present_timing.ui_auto_layout_batches as u64;
            self.batch_ui.command_nodes += present_timing.ui_command_nodes as u64;
            self.batch_ui.command_emitted += present_timing.ui_command_emitted as u64;
            self.batch_ui.command_skipped += present_timing.ui_command_skipped as u64;
            self.batch_ui.removed_nodes += present_timing.ui_removed_nodes as u64;
            self.batch_heavy.present_drain_commands += present_timing.drain_commands;
            self.batch_heavy.present_submit_commands += present_timing.submit_commands;
            self.batch_heavy.present_draw_frame += present_timing.gpu_present;
            self.batch_heavy.draw_process_commands += present_timing.draw_process_commands;
            self.batch_heavy.draw_prepare_cpu += present_timing.draw_prepare_cpu;
            self.batch_heavy.draw_gpu_prepare_2d += present_timing.draw_gpu_prepare_2d;
            self.batch_heavy.draw_gpu_prepare_3d += present_timing.draw_gpu_prepare_3d;
            self.batch_heavy.draw_gpu_prepare_particles_3d +=
                present_timing.draw_gpu_prepare_particles_3d;
            self.batch_heavy.draw_gpu_prepare_3d_frustum +=
                present_timing.draw_gpu_prepare_3d_frustum;
            self.batch_heavy.draw_gpu_prepare_3d_hiz += present_timing.draw_gpu_prepare_3d_hiz;
            self.batch_heavy.draw_gpu_prepare_3d_indirect +=
                present_timing.draw_gpu_prepare_3d_indirect;
            self.batch_heavy.draw_gpu_prepare_3d_cull_inputs +=
                present_timing.draw_gpu_prepare_3d_cull_inputs;
            self.batch_heavy.draw_gpu_acquire += present_timing.draw_gpu_acquire;
            self.batch_heavy.draw_gpu_acquire_surface += present_timing.draw_gpu_acquire_surface;
            self.batch_heavy.draw_gpu_acquire_view += present_timing.draw_gpu_acquire_view;
            self.batch_heavy.draw_gpu_encode_main += present_timing.draw_gpu_encode_main;
            self.batch_heavy.draw_gpu_submit_main += present_timing.draw_gpu_submit_main;
            self.batch_heavy.draw_gpu_submit_finish_main +=
                present_timing.draw_gpu_submit_finish_main;
            self.batch_heavy.draw_gpu_submit_queue_main +=
                present_timing.draw_gpu_submit_queue_main;
            self.batch_heavy.draw_gpu_post_process += present_timing.draw_gpu_post_process;
            self.batch_heavy.draw_gpu_accessibility += present_timing.draw_gpu_accessibility;
            self.batch_heavy.draw_gpu_present += present_timing.draw_gpu_present;
            self.batch_heavy.draw_calls_2d += present_timing.draw_calls_2d as u64;
            self.batch_heavy.draw_calls_3d += present_timing.draw_calls_3d as u64;
            self.batch_heavy.draw_calls_total += present_timing.draw_calls_total as u64;
            self.batch_heavy.sprite_batches_2d += present_timing.sprite_batches_2d as u64;
            self.batch_heavy.sprite_bind_group_switches_2d +=
                present_timing.sprite_bind_group_switches_2d as u64;
            self.batch_heavy.draw_batches_3d += present_timing.draw_batches_3d as u64;
            self.batch_heavy.pipeline_switches_3d += present_timing.pipeline_switches_3d as u64;
            self.batch_heavy.texture_bind_group_switches_3d +=
                present_timing.texture_bind_group_switches_3d as u64;
            self.batch_heavy.draw_instances_3d += present_timing.draw_instances_3d as u64;
            self.batch_heavy.draw_material_refs_3d += present_timing.draw_material_refs_3d as u64;
            self.batch_heavy.render_command_count += present_timing.render_command_count as u64;
            self.batch_heavy.dirty_node_count += present_timing.dirty_node_count as u64;
            self.batch_heavy.active_meshes += present_timing.active_meshes as u64;
            self.batch_heavy.active_materials += present_timing.active_materials as u64;
            self.batch_heavy.active_textures += present_timing.active_textures as u64;
            self.batch_heavy.skip_prepare_2d += present_timing.skip_prepare_2d as u64;
            self.batch_heavy.skip_prepare_3d += present_timing.skip_prepare_3d as u64;
            self.batch_heavy.skip_prepare_particles_3d +=
                present_timing.skip_prepare_particles_3d as u64;
            self.batch_heavy.skip_prepare_3d_frustum +=
                present_timing.skip_prepare_3d_frustum as u64;
            self.batch_heavy.skip_prepare_3d_hiz += present_timing.skip_prepare_3d_hiz as u64;
            self.batch_heavy.skip_prepare_3d_indirect +=
                present_timing.skip_prepare_3d_indirect as u64;
            self.batch_heavy.skip_prepare_3d_cull_inputs +=
                present_timing.skip_prepare_3d_cull_inputs as u64;
            self.batch_heavy.present_drain_events += present_timing.drain_events;
            self.batch_heavy.present_apply_events += present_timing.apply_events;
            self.batch_heavy.sim_delta_seconds += simulated_delta_seconds;
        }

        let shown_for = frame_start.saturating_duration_since(self.startup_splash.shown_at);
        let hard_timeout_hit = shown_for >= STARTUP_SPLASH_HARD_TIMEOUT;
        if self.startup_splash.fade_started_at.is_none()
            && (shown_for >= STARTUP_SPLASH_HOLD_DURATION || hard_timeout_hit)
        {
            self.startup_splash.fade_started_at = Some(frame_start);
        }
        if self.startup_splash.should_finish(frame_start) {
            self.end_startup_splash();
        }

        self.app.begin_input_frame();
    }
}
