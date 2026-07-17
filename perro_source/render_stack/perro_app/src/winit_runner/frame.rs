use super::*;

impl<B: GraphicsBackend> RunnerState<B> {
    pub(super) fn step_frame(&mut self, event_loop: &ActiveEventLoop, now: Instant) {
        if event_loop.exiting() || self.exit_result.is_some() {
            return;
        }
        if self.pacer.blocks_frame(now) {
            self.apply_frame_control_flow(event_loop, now);
            return;
        }
        self.frame_index = self.frame_index.saturating_add(1);
        let frame_index = self.frame_index;
        let frame_start = now;
        let frame_delta = frame_start.duration_since(self.last_frame_start);
        self.last_frame_start = frame_start;

        let fps_window_elapsed = frame_start.duration_since(self.fps_window_start);
        if fps_window_elapsed.as_secs_f32() >= FPS_WINDOW_SECONDS && self.fps_window_frames > 0 {
            self.app
                .set_fps(self.fps_window_frames as f32 / fps_window_elapsed.as_secs_f32());
            self.fps_window_start = frame_start;
            self.fps_window_frames = 0;
        }
        self.fps_window_frames = self.fps_window_frames.saturating_add(1);

        let elapsed_since_start = frame_start.duration_since(self.run_start);
        self.app.set_elapsed_time(elapsed_since_start.as_secs_f32());
        let simulated_delta_seconds;
        let should_sample_timing = self.should_sample_timing();

        let idle_duration = frame_start.saturating_duration_since(self.last_frame_end);

        if self.startup_splash.active {
            self.step_startup_frame(
                event_loop,
                frame_index,
                frame_start,
                frame_delta,
                idle_duration,
            );
            return;
        }

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
        // Poll device inputs before update so scripts see the latest state.
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
        {
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
                simulated_delta_seconds = effective_fixed_step as f64 * plan.steps as f64;
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
                simulated_delta_seconds = variable_step as f64;
            }
        }

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

        #[cfg(feature = "profile_heavy")]
        let present_timing = self.app.present_timed();
        #[cfg(not(feature = "profile_heavy"))]
        let present_timing = if should_sample_timing {
            Some(self.app.present_timed())
        } else {
            self.app.present();
            None
        };
        self.apply_cursor_icon_request();
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
        self.pacer.update_deadline(frame_start, frame_end, false);
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
        if let Some(csv) = &mut self.timing_csv {
            csv.write(CsvFrameSample {
                frame_index,
                phase: "steady",
                warmup: warmup_frame,
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
            });
        }
        if warmup_frame {
            self.timing_warmup_frames_left = self.timing_warmup_frames_left.saturating_sub(1);
            if self.timing_warmup_frames_left == 0 {
                self.batch_start = frame_end;
            }
            self.app.begin_input_frame();
            return;
        }
        #[cfg(feature = "profile_heavy")]
        {
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

        let batch_elapsed_secs = frame_end.duration_since(self.batch_start).as_secs_f32();
        if batch_elapsed_secs >= LOG_INTERVAL_SECONDS && self.batch.timing_samples > 0 {
            let avg_work_us = avg_micros(self.batch.work, self.batch.timing_samples);
            let avg_simulation_us = avg_micros(self.batch.simulation, self.batch.timing_samples);
            let avg_present_us = avg_micros(self.batch.present, self.batch.timing_samples);
            let avg_idle_before_frame_us =
                avg_micros(self.batch.idle_before_frame, self.batch.timing_samples);
            let avg_present_wait_us =
                avg_micros(self.batch.present_wait, self.batch.timing_samples);
            log_avg_sampled(
                avg_simulation_us,
                avg_present_us,
                avg_work_us,
                avg_idle_before_frame_us,
                avg_present_wait_us,
            );
            #[cfg(all(
                feature = "ui_profile",
                not(feature = "profile_heavy"),
                not(perro_no_console)
            ))]
            {
                let avg_present_extract_ui_us =
                    self.batch_ui.extract_ui.as_micros() as f64 / self.batch.frames as f64;
                let avg_ui_layout_us =
                    self.batch_ui.layout.as_micros() as f64 / self.batch.frames as f64;
                let avg_ui_commands_us =
                    self.batch_ui.commands.as_micros() as f64 / self.batch.frames as f64;
                let avg_ui_dirty = self.batch_ui.dirty_nodes as f64 / self.batch.frames as f64;
                let avg_ui_affected =
                    self.batch_ui.affected_nodes as f64 / self.batch.frames as f64;
                let avg_ui_recalc =
                    self.batch_ui.recalculated_rects as f64 / self.batch.frames as f64;
                let avg_ui_cached = self.batch_ui.cached_rects as f64 / self.batch.frames as f64;
                let avg_ui_batches =
                    self.batch_ui.auto_layout_batches as f64 / self.batch.frames as f64;
                let avg_ui_cmd_nodes =
                    self.batch_ui.command_nodes as f64 / self.batch.frames as f64;
                let avg_ui_cmd_emit =
                    self.batch_ui.command_emitted as f64 / self.batch.frames as f64;
                let avg_ui_cmd_skip =
                    self.batch_ui.command_skipped as f64 / self.batch.frames as f64;
                let avg_ui_removed = self.batch_ui.removed_nodes as f64 / self.batch.frames as f64;
                println!(
                    "ui profile: total=({avg_present_extract_ui_us:.3}us) layout=({avg_ui_layout_us:.3}us) commands=({avg_ui_commands_us:.3}us) dirty=({avg_ui_dirty:.2}) affected=({avg_ui_affected:.2}) rect_recalc=({avg_ui_recalc:.2}) rect_cache=({avg_ui_cached:.2}) auto_batches=({avg_ui_batches:.2}) cmd_nodes=({avg_ui_cmd_nodes:.2}) cmd_emit=({avg_ui_cmd_emit:.2}) cmd_skip=({avg_ui_cmd_skip:.2}) rm=({avg_ui_removed:.2})"
                );
            }
            #[cfg(all(
                any(feature = "profile_heavy", feature = "mem_profile"),
                not(perro_no_console)
            ))]
            if self.mem_profile_enabled
                && let Some(sample) = process_memory_sample()
            {
                let avg_frame_us = avg_work_us
                    .saturating_add(avg_idle_before_frame_us)
                    .saturating_add(avg_present_wait_us);
                let avg_fps = if avg_frame_us > 0 {
                    1_000_000.0 / avg_frame_us as f64
                } else {
                    0.0
                };
                if let Some(csv) = &mut self.mem_profile_csv {
                    csv.write(MemProfileCsvSample {
                        batch_end_frame: self.frame_index,
                        sample,
                        avg_update_us: avg_simulation_us,
                        avg_render_us: avg_present_us,
                        avg_idle_us: avg_idle_before_frame_us,
                        avg_present_wait_us,
                        avg_frame_us,
                        avg_fps,
                    });
                }
            }
            #[cfg(all(feature = "profile_heavy", not(perro_no_console)))]
            {
                let avg_runtime_update_us =
                    self.batch_heavy.runtime_update.as_micros() as f64 / self.batch.frames as f64;
                let avg_input_poll_us =
                    self.batch_heavy.input_poll.as_micros() as f64 / self.batch.frames as f64;
                let avg_fixed_update_us =
                    self.batch_heavy.fixed_update.as_micros() as f64 / self.batch.frames as f64;
                let avg_fixed_snapshot_update_us =
                    self.batch_heavy.fixed_snapshot_update.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_fixed_script_update_us = self.batch_heavy.fixed_script_update.as_micros()
                    as f64
                    / self.batch.frames as f64;
                let avg_fixed_physics_update_us = self.batch_heavy.fixed_physics_update.as_micros()
                    as f64
                    / self.batch.frames as f64;
                let avg_fixed_internal_update_us =
                    self.batch_heavy.fixed_internal_update.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_fixed_physics_pre_transforms_us =
                    self.batch_heavy.fixed_physics_pre_transforms.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_fixed_physics_collect_us =
                    self.batch_heavy.fixed_physics_collect.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_fixed_physics_sync_world_us =
                    self.batch_heavy.fixed_physics_sync_world.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_fixed_physics_apply_forces_impulses_us =
                    self.batch_heavy
                        .fixed_physics_apply_forces_impulses
                        .as_micros() as f64
                        / self.batch.frames as f64;
                let avg_fixed_physics_step_us = self.batch_heavy.fixed_physics_step.as_micros()
                    as f64
                    / self.batch.frames as f64;
                let avg_fixed_physics_sync_nodes_us =
                    self.batch_heavy.fixed_physics_sync_nodes.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_fixed_physics_post_transforms_us =
                    self.batch_heavy.fixed_physics_post_transforms.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_fixed_physics_signals_us =
                    self.batch_heavy.fixed_physics_signals.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_runtime_script_update_us =
                    self.batch_heavy.runtime_script_update.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_runtime_script_count =
                    self.batch_heavy.runtime_script_count as f64 / self.batch.frames as f64;

                let avg_present_extract_2d_us = self.batch_heavy.present_extract_2d.as_micros()
                    as f64
                    / self.batch.frames as f64;
                let avg_present_extract_3d_us = self.batch_heavy.present_extract_3d.as_micros()
                    as f64
                    / self.batch.frames as f64;
                let avg_present_extract_ui_us =
                    self.batch_ui.extract_ui.as_micros() as f64 / self.batch.frames as f64;
                let avg_ui_layout_us =
                    self.batch_ui.layout.as_micros() as f64 / self.batch.frames as f64;
                let avg_ui_commands_us =
                    self.batch_ui.commands.as_micros() as f64 / self.batch.frames as f64;
                let avg_ui_dirty = self.batch_ui.dirty_nodes as f64 / self.batch.frames as f64;
                let avg_ui_affected =
                    self.batch_ui.affected_nodes as f64 / self.batch.frames as f64;
                let avg_ui_recalc =
                    self.batch_ui.recalculated_rects as f64 / self.batch.frames as f64;
                let avg_ui_cached = self.batch_ui.cached_rects as f64 / self.batch.frames as f64;
                let avg_ui_batches =
                    self.batch_ui.auto_layout_batches as f64 / self.batch.frames as f64;
                let avg_ui_cmd_nodes =
                    self.batch_ui.command_nodes as f64 / self.batch.frames as f64;
                let avg_ui_cmd_emit =
                    self.batch_ui.command_emitted as f64 / self.batch.frames as f64;
                let avg_ui_cmd_skip =
                    self.batch_ui.command_skipped as f64 / self.batch.frames as f64;
                let avg_ui_removed = self.batch_ui.removed_nodes as f64 / self.batch.frames as f64;
                let avg_present_drain_commands_us =
                    self.batch_heavy.present_drain_commands.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_present_submit_commands_us =
                    self.batch_heavy.present_submit_commands.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_present_draw_frame_us = self.batch_heavy.present_draw_frame.as_micros()
                    as f64
                    / self.batch.frames as f64;
                let avg_draw_process_commands_us =
                    self.batch_heavy.draw_process_commands.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_draw_prepare_cpu_us =
                    self.batch_heavy.draw_prepare_cpu.as_micros() as f64 / self.batch.frames as f64;
                let avg_draw_gpu_prepare_2d_us = self.batch_heavy.draw_gpu_prepare_2d.as_micros()
                    as f64
                    / self.batch.frames as f64;
                let avg_draw_gpu_prepare_3d_us = self.batch_heavy.draw_gpu_prepare_3d.as_micros()
                    as f64
                    / self.batch.frames as f64;
                let avg_draw_gpu_prepare_particles_3d_us =
                    self.batch_heavy.draw_gpu_prepare_particles_3d.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_draw_gpu_prepare_3d_frustum_us =
                    self.batch_heavy.draw_gpu_prepare_3d_frustum.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_draw_gpu_prepare_3d_hiz_us =
                    self.batch_heavy.draw_gpu_prepare_3d_hiz.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_draw_gpu_prepare_3d_indirect_us =
                    self.batch_heavy.draw_gpu_prepare_3d_indirect.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_draw_gpu_prepare_3d_cull_inputs_us =
                    self.batch_heavy.draw_gpu_prepare_3d_cull_inputs.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_draw_gpu_acquire_us =
                    self.batch_heavy.draw_gpu_acquire.as_micros() as f64 / self.batch.frames as f64;
                let avg_draw_gpu_acquire_surface_us =
                    self.batch_heavy.draw_gpu_acquire_surface.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_draw_gpu_acquire_view_us =
                    self.batch_heavy.draw_gpu_acquire_view.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_draw_gpu_encode_main_us = self.batch_heavy.draw_gpu_encode_main.as_micros()
                    as f64
                    / self.batch.frames as f64;
                let avg_draw_gpu_submit_main_us = self.batch_heavy.draw_gpu_submit_main.as_micros()
                    as f64
                    / self.batch.frames as f64;
                let avg_draw_gpu_submit_finish_main_us =
                    self.batch_heavy.draw_gpu_submit_finish_main.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_draw_gpu_submit_queue_main_us =
                    self.batch_heavy.draw_gpu_submit_queue_main.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_draw_gpu_post_process_us =
                    self.batch_heavy.draw_gpu_post_process.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_draw_gpu_accessibility_us =
                    self.batch_heavy.draw_gpu_accessibility.as_micros() as f64
                        / self.batch.frames as f64;
                let avg_draw_gpu_present_us =
                    self.batch_heavy.draw_gpu_present.as_micros() as f64 / self.batch.frames as f64;
                let avg_draw_calls_2d =
                    self.batch_heavy.draw_calls_2d as f64 / self.batch.frames as f64;
                let avg_draw_calls_3d =
                    self.batch_heavy.draw_calls_3d as f64 / self.batch.frames as f64;
                let avg_draw_calls_total =
                    self.batch_heavy.draw_calls_total as f64 / self.batch.frames as f64;
                let avg_sprite_batches_2d =
                    self.batch_heavy.sprite_batches_2d as f64 / self.batch.frames as f64;
                let avg_sprite_bind_group_switches_2d =
                    self.batch_heavy.sprite_bind_group_switches_2d as f64
                        / self.batch.frames as f64;
                let avg_draw_batches_3d =
                    self.batch_heavy.draw_batches_3d as f64 / self.batch.frames as f64;
                let avg_pipeline_switches_3d =
                    self.batch_heavy.pipeline_switches_3d as f64 / self.batch.frames as f64;
                let avg_texture_bind_group_switches_3d =
                    self.batch_heavy.texture_bind_group_switches_3d as f64
                        / self.batch.frames as f64;
                let avg_draw_instances_3d =
                    self.batch_heavy.draw_instances_3d as f64 / self.batch.frames as f64;
                let avg_instances_per_draw_3d = if self.batch_heavy.draw_calls_3d > 0 {
                    self.batch_heavy.draw_instances_3d as f64
                        / self.batch_heavy.draw_calls_3d as f64
                } else {
                    0.0
                };
                let avg_draw_material_refs_3d =
                    self.batch_heavy.draw_material_refs_3d as f64 / self.batch.frames as f64;
                let avg_render_commands =
                    self.batch_heavy.render_command_count as f64 / self.batch.frames as f64;
                let avg_dirty_nodes =
                    self.batch_heavy.dirty_node_count as f64 / self.batch.frames as f64;
                let avg_active_meshes =
                    self.batch_heavy.active_meshes as f64 / self.batch.frames as f64;
                let avg_active_materials =
                    self.batch_heavy.active_materials as f64 / self.batch.frames as f64;
                let avg_active_textures =
                    self.batch_heavy.active_textures as f64 / self.batch.frames as f64;
                let avg_present_drain_events_us = self.batch_heavy.present_drain_events.as_micros()
                    as f64
                    / self.batch.frames as f64;
                let avg_present_apply_events_us = self.batch_heavy.present_apply_events.as_micros()
                    as f64
                    / self.batch.frames as f64;
                let avg_frame_us = (self.batch.work.as_micros() as f64
                    + self.batch.idle_before_frame.as_micros() as f64
                    + self.batch.present_wait.as_micros() as f64)
                    / self.batch.frames as f64;
                let pct_skip_prepare_2d =
                    (self.batch_heavy.skip_prepare_2d as f64 * 100.0) / self.batch.frames as f64;
                let pct_skip_prepare_3d =
                    (self.batch_heavy.skip_prepare_3d as f64 * 100.0) / self.batch.frames as f64;
                let pct_skip_prepare_particles_3d =
                    (self.batch_heavy.skip_prepare_particles_3d as f64 * 100.0)
                        / self.batch.frames as f64;
                let pct_skip_prepare_3d_frustum = (self.batch_heavy.skip_prepare_3d_frustum as f64
                    * 100.0)
                    / self.batch.frames as f64;
                let pct_skip_prepare_3d_hiz = (self.batch_heavy.skip_prepare_3d_hiz as f64 * 100.0)
                    / self.batch.frames as f64;
                let pct_skip_prepare_3d_indirect =
                    (self.batch_heavy.skip_prepare_3d_indirect as f64 * 100.0)
                        / self.batch.frames as f64;
                let pct_skip_prepare_3d_cull_inputs =
                    (self.batch_heavy.skip_prepare_3d_cull_inputs as f64 * 100.0)
                        / self.batch.frames as f64;
                println!(
                    "simulation breakdown: input=({:.3}us) fixed=({:.3}us) runtime=({:.3}us)",
                    avg_input_poll_us, avg_fixed_update_us, avg_runtime_update_us
                );
                println!(
                    "fixed breakdown: snapshot=({:.3}us) scripts=({:.3}us) physics=({:.3}us) internal=({:.3}us)",
                    avg_fixed_snapshot_update_us,
                    avg_fixed_script_update_us,
                    avg_fixed_physics_update_us,
                    avg_fixed_internal_update_us
                );
                println!(
                    "physics breakdown: pre_xform=({:.3}us) collect=({:.3}us) sync_world=({:.3}us) apply=({:.3}us) step=({:.3}us) sync_nodes=({:.3}us) post_xform=({:.3}us) signals=({:.3}us)",
                    avg_fixed_physics_pre_transforms_us,
                    avg_fixed_physics_collect_us,
                    avg_fixed_physics_sync_world_us,
                    avg_fixed_physics_apply_forces_impulses_us,
                    avg_fixed_physics_step_us,
                    avg_fixed_physics_sync_nodes_us,
                    avg_fixed_physics_post_transforms_us,
                    avg_fixed_physics_signals_us
                );
                println!(
                    "user scripts: ({:.3}us avg) | script calls/frame: ({:.2}) | slowest script: ({:.3}us)",
                    avg_runtime_script_update_us,
                    avg_runtime_script_count,
                    self.batch_heavy.runtime_slowest_script.as_micros() as f64
                );
                println!(
                    "present breakdown: extract2d=({:.3}us) extract3d=({:.3}us) extract_ui=({:.3}us) ui_layout=({:.3}us) ui_commands=({:.3}us) drain=({:.3}us) submit=({:.3}us) draw=({:.3}us) events_drain=({:.3}us) events_apply=({:.3}us)",
                    avg_present_extract_2d_us,
                    avg_present_extract_3d_us,
                    avg_present_extract_ui_us,
                    avg_ui_layout_us,
                    avg_ui_commands_us,
                    avg_present_drain_commands_us,
                    avg_present_submit_commands_us,
                    avg_present_draw_frame_us,
                    avg_present_drain_events_us,
                    avg_present_apply_events_us
                );
                println!(
                    "ui nodes: dirty=({avg_ui_dirty:.2}) affected=({avg_ui_affected:.2}) rect_recalc=({avg_ui_recalc:.2}) rect_cache=({avg_ui_cached:.2}) auto_batches=({avg_ui_batches:.2}) cmd_nodes=({avg_ui_cmd_nodes:.2}) cmd_emit=({avg_ui_cmd_emit:.2}) cmd_skip=({avg_ui_cmd_skip:.2}) rm=({avg_ui_removed:.2})"
                );
                println!(
                    "draw breakdown: process=({:.3}us) prep=({:.3}us) gpu_prepare2d=({:.3}us) gpu_prepare3d=({:.3}us) acquire=({:.3}us) encode=({:.3}us) gpu_submit=({:.3}us) post=({:.3}us) access=({:.3}us) present=({:.3}us) calls2d=({:.2}) calls3d=({:.2}) calls=({:.2})",
                    avg_draw_process_commands_us,
                    avg_draw_prepare_cpu_us,
                    avg_draw_gpu_prepare_2d_us,
                    avg_draw_gpu_prepare_3d_us,
                    avg_draw_gpu_acquire_us,
                    avg_draw_gpu_encode_main_us,
                    avg_draw_gpu_submit_main_us,
                    avg_draw_gpu_post_process_us,
                    avg_draw_gpu_accessibility_us,
                    avg_draw_gpu_present_us,
                    avg_draw_calls_2d,
                    avg_draw_calls_3d,
                    avg_draw_calls_total
                );
                println!(
                    "draw substeps: prep_particles3d=({:.3}us) prep_frustum=({:.3}us) prep_hiz=({:.3}us) prep_indirect=({:.3}us) prep_cull_inputs=({:.3}us) acquire_surface=({:.3}us) acquire_view=({:.3}us) submit_finish=({:.3}us) submit_queue=({:.3}us)",
                    avg_draw_gpu_prepare_particles_3d_us,
                    avg_draw_gpu_prepare_3d_frustum_us,
                    avg_draw_gpu_prepare_3d_hiz_us,
                    avg_draw_gpu_prepare_3d_indirect_us,
                    avg_draw_gpu_prepare_3d_cull_inputs_us,
                    avg_draw_gpu_acquire_surface_us,
                    avg_draw_gpu_acquire_view_us,
                    avg_draw_gpu_submit_finish_main_us,
                    avg_draw_gpu_submit_queue_main_us
                );
                println!(
                    "draw skips: prep2d=({:.1}%) prep3d=({:.1}%) prep_particles3d=({:.1}%) frustum=({:.1}%) hiz=({:.1}%) indirect=({:.1}%) cull_inputs=({:.1}%)",
                    pct_skip_prepare_2d,
                    pct_skip_prepare_3d,
                    pct_skip_prepare_particles_3d,
                    pct_skip_prepare_3d_frustum,
                    pct_skip_prepare_3d_hiz,
                    pct_skip_prepare_3d_indirect,
                    pct_skip_prepare_3d_cull_inputs
                );
                println!(
                    "renderer counters: sprite_batches=({avg_sprite_batches_2d:.2}) sprite_binds=({avg_sprite_bind_group_switches_2d:.2}) draw_batches3d=({avg_draw_batches_3d:.2}) pipe_sw3d=({avg_pipeline_switches_3d:.2}) tex_sw3d=({avg_texture_bind_group_switches_3d:.2})"
                );
                if let Some(csv) = &mut self.profile_csv {
                    let row = ProfileCsvRow {
                        batch_end_frame: self.frame_index,
                        frames: self.batch.frames,
                        sampled_frames: self.batch.timing_samples,
                        avg_draw_calls_2d,
                        avg_draw_calls_3d,
                        avg_draw_calls_total,
                        avg_sprite_batches_2d,
                        avg_sprite_bind_group_switches_2d,
                        avg_draw_batches_3d,
                        avg_pipeline_switches_3d,
                        avg_texture_bind_group_switches_3d,
                        avg_draw_instances_3d,
                        avg_instances_per_draw_3d,
                        avg_draw_material_refs_3d,
                        avg_render_commands,
                        avg_dirty_nodes,
                        avg_extract2d_us: avg_present_extract_2d_us,
                        avg_extract3d_us: avg_present_extract_3d_us,
                        avg_extract_ui_us: avg_present_extract_ui_us,
                        avg_drain_commands_us: avg_present_drain_commands_us,
                        avg_submit_commands_us: avg_present_submit_commands_us,
                        avg_draw_process_us: avg_draw_process_commands_us,
                        avg_draw_prep_us: avg_draw_prepare_cpu_us,
                        avg_active_meshes,
                        avg_active_materials,
                        avg_active_textures,
                        avg_present_wait_us: self.batch.present_wait.as_micros() as f64
                            / self.batch.frames as f64,
                        avg_frame_us,
                    };
                    csv.write(&row);
                }
            }
            self.batch = BatchCoreStats::default();
            #[cfg(all(feature = "ui_profile", not(feature = "profile_heavy")))]
            {
                self.batch_ui = BatchUiStats::default();
            }
            #[cfg(feature = "profile_heavy")]
            {
                self.batch_ui = BatchUiStats::default();
                self.batch_heavy = BatchHeavyStats::default();
            }
            self.batch_start = frame_end;
        }

        // Clear per-frame pressed/released flags after update to preserve
        // window events that arrived since the last frame.
        self.app.begin_input_frame();
    }
}
