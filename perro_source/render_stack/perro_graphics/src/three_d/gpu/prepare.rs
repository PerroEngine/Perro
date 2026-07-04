use super::*;
use rayon::slice::ParallelSliceMut;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

const PARALLEL_BATCH_SORT_MIN: usize = 10_000;

fn estimate_draw_instance_capacity(draws: &[Draw3DInstance]) -> (usize, usize) {
    let mut regular = 0usize;
    let mut multimesh = 0usize;
    for draw in draws {
        if let Some(dense) = draw.dense_multimesh.as_ref() {
            multimesh = multimesh.saturating_add(dense.instances.len());
        } else {
            regular = regular.saturating_add(draw.instance_mats.len());
        }
    }
    (regular, multimesh)
}

fn builtin_flat_mesh_double_sided(source: &str) -> bool {
    matches!(
        source,
        perro_builtin_meshes::PLANE_SOURCE | perro_builtin_meshes::QUAD_SOURCE
    )
}

impl Gpu3D {
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: Prepare3D<'_>) {
        let mut step_timing = Prepare3DStepTiming::default();
        if self.gpu_occlusion_enabled && HIZ_DEBUG_READBACK_ENABLED {
            if self.pending_hiz_debug_map_rx.is_some() {
                let _ = device.poll(wgpu::PollType::Poll);
            }
            if self.pending_hiz_debug_count > 0 && self.pending_hiz_debug_map_rx.is_none() {
                self.request_hiz_debug_map_async();
            }
            self.consume_hiz_debug_results();
        }
        if self.cpu_occlusion_enabled {
            if self.pending_occlusion_map_rx.is_some() {
                let _ = device.poll(wgpu::PollType::Poll);
            }
            if self.pending_occlusion_query_count > 0 && self.pending_occlusion_map_rx.is_none() {
                self.request_occlusion_map_async();
            }
            self.consume_occlusion_results();
            self.occlusion_frame = self.occlusion_frame.wrapping_add(1);
        }
        self.occlusion_query_keys_this_frame.clear();
        let occlusion_capture_this_frame = self.cpu_occlusion_enabled
            && self.pending_occlusion_query_count == 0
            && self.pending_occlusion_map_rx.is_none();

        let Prepare3D {
            resources,
            camera,
            lighting,
            draws,
            draws_revision,
            width,
            height,
            static_texture_lookup,
            static_mesh_lookup,
            static_shader_lookup,
        } = frame;
        self.custom_mesh_ranges
            .retain(|mesh_id, _| resources.has_mesh(*mesh_id));
        self.resize(device, width, height);
        self.ensure_material_fallback_texture(device, queue);
        self.frustum_cull_enabled = self.frustum_cull_supported;
        let (gpu_occlusion_enabled, cpu_occlusion_enabled) = occlusion_flags(self.occlusion_mode);
        self.gpu_occlusion_enabled = gpu_occlusion_enabled && self.frustum_cull_enabled;
        self.cpu_occlusion_enabled = cpu_occlusion_enabled;

        let uniform = build_scene_uniform(&camera, lighting, width, height);
        let sky_uniform = build_sky_uniform(&camera, lighting, width, height);
        self.sky_enabled = sky_uniform.is_some();
        if let Some(sky) = lighting.sky.as_ref() {
            self.ensure_sky_pipeline(device, sky, static_shader_lookup);
        } else {
            self.active_sky_pipeline_key = None;
        }
        match sky_uniform {
            Some(sky) => {
                let time_seconds = sky.params1[0];
                let mut static_sky = sky;
                static_sky.params1[0] = 0.0;
                if self.last_sky != Some(static_sky) {
                    queue.write_buffer(&self.sky_buffer, 0, bytemuck::bytes_of(&sky));
                    self.last_sky = Some(static_sky);
                    self.last_sky_time_seconds = time_seconds;
                } else if self.last_sky_time_seconds != time_seconds {
                    queue.write_buffer(
                        &self.sky_buffer,
                        SKY_PARAMS1_X_OFFSET,
                        bytemuck::bytes_of(&time_seconds),
                    );
                    self.last_sky_time_seconds = time_seconds;
                }
            }
            None => {
                self.last_sky = None;
                self.last_sky_time_seconds = -1.0;
            }
        }
        let draws_unchanged = self.last_draws_revision == draws_revision;
        // Classify each draw pair: single-instance regular draws (model-only)
        // and dense multimeshes whose poses are unchanged (node_model-only) both
        // stay on the transform-only fast path. A multimesh present but unchanged
        // no longer forces a full rebuild.
        let mut transform_only_kinds = std::mem::take(&mut self.transform_only_kinds_scratch);
        let transform_only_semantic = !draws_unchanged
            && classify_transform_only_scene(&self.last_draws, draws, &mut transform_only_kinds);
        // Multimesh patch needs the param-range bookkeeping to line up with the
        // current draw list; otherwise fall back to a full rebuild.
        let stable_multimesh_ranges = !transform_only_semantic
            || (self.last_draw_multimesh_param_ranges.len() == draws.len()
                && self.last_draw_multimesh_param_ranges.iter().all(|range| {
                    range.start <= range.end
                        && (range.end as usize) <= self.staged_multimesh_draw_params.len()
                }));
        let stable_instance_ranges = self.last_draw_instance_span_ranges.len() == draws.len()
            && self
                .last_draw_instance_span_ranges
                .iter()
                .all(|span_range| {
                    span_range.start <= span_range.end
                        && span_range.end <= self.last_draw_instance_spans.len()
                })
            && self.last_draw_instance_spans.iter().all(|range| {
                range.start <= range.end
                    && (range.end as usize) <= self.staged_instance_transforms.len()
            });
        let transform_only_changed = !draws_unchanged
            && transform_only_semantic
            && stable_instance_ranges
            && stable_multimesh_ranges;
        self.transform_only_kinds_scratch = transform_only_kinds;
        let scene_changed = self.last_scene != Some(uniform) || !draws_unchanged;
        if self.last_scene != Some(uniform) {
            queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
            self.last_scene = Some(uniform);
        }
        if self.cpu_occlusion_enabled && scene_changed {
            // Retained-mode correctness: when camera/transforms/resources update,
            // previous query visibility is stale and must not gate current frame.
            self.occlusion_state.clear();
        }
        let view_proj = compute_view_proj_mat(&camera, width, height);
        self.last_aspect = (width.max(1) as f32) / (height.max(1) as f32);
        self.last_proj_y_scale = projection_y_scale_from_projection(camera.projection);

        if draws_unchanged && !scene_changed {
            let frustum_cull_active = self.should_run_frustum_cull();
            let hiz_active = self.should_run_hiz_occlusion(frustum_cull_active);
            if frustum_cull_active {
                let frustum_inputs_invalid = !self.frustum_gpu_inputs_valid
                    || self.indirect_staging.len() != self.draw_batches.len()
                    || self.frustum_cull_dynamic_staging.len() != self.draw_batches.len()
                    || self.frustum_cull_static_staging.len() != self.draw_batches.len();
                if frustum_inputs_invalid {
                    let indirect_start = Instant::now();
                    self.ensure_frustum_cull_capacity(device, self.draw_batches.len());
                    self.indirect_staging.clear();
                    self.indirect_staging.reserve(self.draw_batches.len());
                    for batch in &self.draw_batches {
                        self.indirect_staging.push(DrawIndexedIndirectGpu {
                            index_count: batch.mesh.index_count,
                            instance_count: batch.instance_count,
                            first_index: batch.mesh.index_start,
                            base_vertex: batch.mesh.base_vertex,
                            first_instance: batch.instance_start,
                        });
                    }
                    queue.write_buffer(
                        &self.indirect_buffer,
                        0,
                        bytemuck::cast_slice(&self.indirect_staging),
                    );
                    step_timing.indirect_prep += indirect_start.elapsed();

                    let cull_start = Instant::now();
                    self.rebuild_frustum_cull_items(queue);
                    step_timing.cull_input_prep += cull_start.elapsed();
                    self.frustum_gpu_inputs_valid = true;
                } else {
                    step_timing.indirect_skipped = step_timing.indirect_skipped.saturating_add(1);
                    step_timing.cull_input_skipped =
                        step_timing.cull_input_skipped.saturating_add(1);
                }

                let frustum_start = Instant::now();
                let frustum = extract_frustum_planes(view_proj);
                let frustum_written = self.write_frustum_params_if_needed(queue, &frustum);
                step_timing.frustum_prep += frustum_start.elapsed();
                if !frustum_written {
                    step_timing.frustum_skipped = step_timing.frustum_skipped.saturating_add(1);
                }

                if hiz_active {
                    let hiz_start = Instant::now();
                    let hiz_written =
                        self.write_hiz_params_if_needed(queue, &uniform, self.draw_batches.len());
                    step_timing.hiz_prep += hiz_start.elapsed();
                    if !hiz_written {
                        step_timing.hiz_skipped = step_timing.hiz_skipped.saturating_add(1);
                    }
                } else {
                    step_timing.hiz_skipped = step_timing.hiz_skipped.saturating_add(1);
                }
            } else {
                step_timing.frustum_skipped = step_timing.frustum_skipped.saturating_add(1);
                step_timing.hiz_skipped = step_timing.hiz_skipped.saturating_add(1);
                step_timing.indirect_skipped = step_timing.indirect_skipped.saturating_add(1);
                step_timing.cull_input_skipped = step_timing.cull_input_skipped.saturating_add(1);
            }
            self.update_shadow_state(queue, &camera, lighting, self.has_shadow_casters);
            self.last_total_drawn =
                self.staged_instance_transforms.len() + self.staged_multimesh_instances.len();
            self.last_prepare_step_timing = step_timing;
            return;
        }
        if transform_only_changed {
            self.dirty_instance_spans_scratch.clear();
            for (draw, span_range) in draws.iter().zip(self.last_draw_instance_span_ranges.iter()) {
                let Some(model) = draw.instance_mats.first() else {
                    continue;
                };
                for range in self.last_draw_instance_spans[span_range.clone()].iter() {
                    if range.start >= range.end {
                        continue;
                    }
                    for instance in &mut self.staged_instance_transforms
                        [range.start as usize..range.end as usize]
                    {
                        instance.model_row_0 = [model[0][0], model[1][0], model[2][0], model[3][0]];
                        instance.model_row_1 = [model[0][1], model[1][1], model[2][1], model[3][1]];
                        instance.model_row_2 = [model[0][2], model[1][2], model[2][2], model[3][2]];
                    }
                    self.dirty_instance_spans_scratch.push(range.clone());
                }
            }
            self.dirty_instance_spans_scratch
                .sort_unstable_by_key(|span| span.start);
            self.merged_instance_spans_scratch.clear();
            for span in self.dirty_instance_spans_scratch.iter().cloned() {
                if let Some(last) = self.merged_instance_spans_scratch.last_mut()
                    && span.start <= last.end
                {
                    last.end = last.end.max(span.end);
                } else {
                    self.merged_instance_spans_scratch.push(span);
                }
            }
            for span in self.merged_instance_spans_scratch.iter() {
                let byte_start =
                    span.start as u64 * std::mem::size_of::<TransformInstanceGpu>() as u64;
                queue.write_buffer(
                    &self.instance_transform_buffer,
                    byte_start,
                    bytemuck::cast_slice(
                        &self.staged_instance_transforms[span.start as usize..span.end as usize],
                    ),
                );
            }
            // Dense multimeshes whose poses are unchanged: only the draw model
            // moved. Instances are relative to the draw model in the shader, so
            // patch just the MultiMeshDrawParamGpu rows and upload those slots.
            // No per-instance re-pack, no instance buffer re-upload.
            self.dirty_instance_spans_scratch.clear();
            for (draw, param_range) in draws
                .iter()
                .zip(self.last_draw_multimesh_param_ranges.iter())
            {
                let Some(dense) = draw.dense_multimesh.as_ref() else {
                    continue;
                };
                if param_range.start >= param_range.end {
                    continue;
                }
                let draw_model = Mat4::from_cols_array_2d(&dense.node_model);
                let row_0 = [
                    draw_model.x_axis.x,
                    draw_model.y_axis.x,
                    draw_model.z_axis.x,
                    draw_model.w_axis.x,
                ];
                let row_1 = [
                    draw_model.x_axis.y,
                    draw_model.y_axis.y,
                    draw_model.z_axis.y,
                    draw_model.w_axis.y,
                ];
                let row_2 = [
                    draw_model.x_axis.z,
                    draw_model.y_axis.z,
                    draw_model.z_axis.z,
                    draw_model.w_axis.z,
                ];
                for param in &mut self.staged_multimesh_draw_params
                    [param_range.start as usize..param_range.end as usize]
                {
                    param.model_row_0 = row_0;
                    param.model_row_1 = row_1;
                    param.model_row_2 = row_2;
                }
                self.dirty_instance_spans_scratch.push(param_range.clone());
            }
            if !self.dirty_instance_spans_scratch.is_empty() {
                self.dirty_instance_spans_scratch
                    .sort_unstable_by_key(|span| span.start);
                self.merged_instance_spans_scratch.clear();
                for span in self.dirty_instance_spans_scratch.iter().cloned() {
                    if let Some(last) = self.merged_instance_spans_scratch.last_mut()
                        && span.start <= last.end
                    {
                        last.end = last.end.max(span.end);
                    } else {
                        self.merged_instance_spans_scratch.push(span);
                    }
                }
                for span in self.merged_instance_spans_scratch.iter() {
                    let byte_start =
                        span.start as u64 * std::mem::size_of::<MultiMeshDrawParamGpu>() as u64;
                    queue.write_buffer(
                        &self.multimesh_draw_params_buffer,
                        byte_start,
                        bytemuck::cast_slice(
                            &self.staged_multimesh_draw_params
                                [span.start as usize..span.end as usize],
                        ),
                    );
                }
            }
            // Transforms shifted: overlap tests change, so refresh receiver lists.
            self.rebuild_mesh_blend_receivers();
            let frustum_cull_active = self.should_run_frustum_cull();
            let hiz_active = self.should_run_hiz_occlusion(frustum_cull_active);
            if frustum_cull_active {
                let frustum_inputs_invalid = !self.frustum_gpu_inputs_valid
                    || self.indirect_staging.len() != self.draw_batches.len()
                    || self.frustum_cull_dynamic_staging.len() != self.draw_batches.len()
                    || self.frustum_cull_static_staging.len() != self.draw_batches.len();
                if frustum_inputs_invalid {
                    let indirect_start = Instant::now();
                    self.ensure_frustum_cull_capacity(device, self.draw_batches.len());
                    self.indirect_staging.clear();
                    self.indirect_staging.reserve(self.draw_batches.len());
                    for batch in &self.draw_batches {
                        self.indirect_staging.push(DrawIndexedIndirectGpu {
                            index_count: batch.mesh.index_count,
                            instance_count: batch.instance_count,
                            first_index: batch.mesh.index_start,
                            base_vertex: batch.mesh.base_vertex,
                            first_instance: batch.instance_start,
                        });
                    }
                    queue.write_buffer(
                        &self.indirect_buffer,
                        0,
                        bytemuck::cast_slice(&self.indirect_staging),
                    );
                    step_timing.indirect_prep += indirect_start.elapsed();

                    let cull_start = Instant::now();
                    self.rebuild_frustum_cull_items(queue);
                    step_timing.cull_input_prep += cull_start.elapsed();
                    self.frustum_gpu_inputs_valid = true;
                } else {
                    step_timing.indirect_skipped = step_timing.indirect_skipped.saturating_add(1);

                    let cull_start = Instant::now();
                    self.dirty_cull_batch_spans_scratch.clear();
                    for (batch_idx, batch) in self.draw_batches.iter().enumerate() {
                        let batch_start = batch.instance_start;
                        let batch_end = batch.instance_start.saturating_add(batch.instance_count);
                        // Dirty spans are sorted + disjoint, but instance_start
                        // is NOT monotonic across batches (compaction can
                        // repoint a later batch at an earlier shared region), so
                        // search per batch instead of sweeping. A batch is dirty
                        // when ANY of its instances moved: multi-instance bounds
                        // depend on every instance, not just the first.
                        let candidate = self
                            .merged_instance_spans_scratch
                            .partition_point(|span| span.end <= batch_start);
                        let overlaps = self
                            .merged_instance_spans_scratch
                            .get(candidate)
                            .is_some_and(|span| span.start < batch_end);
                        if overlaps {
                            if let Some(last) = self.dirty_cull_batch_spans_scratch.last_mut()
                                && last.end == batch_idx
                            {
                                last.end = batch_idx + 1;
                            } else {
                                self.dirty_cull_batch_spans_scratch
                                    .push(batch_idx..(batch_idx + 1));
                            }
                        }
                    }
                    if self.dirty_cull_batch_spans_scratch.is_empty() {
                        step_timing.cull_input_skipped =
                            step_timing.cull_input_skipped.saturating_add(1);
                    } else {
                        // Transform-only path: topology is unchanged, so rewrite
                        // only the cull rows for the dirty batch spans. Single
                        // instance: refresh the dynamic model. Multi instance:
                        // the static half carries the merged world sphere, so it
                        // must be recomputed and re-uploaded as well.
                        for batch_span in self.dirty_cull_batch_spans_scratch.iter() {
                            let mut static_dirty = false;
                            for batch_idx in batch_span.clone() {
                                let batch = &self.draw_batches[batch_idx];
                                if batch.instance_count > 1 {
                                    let (static_row, dynamic_row) = multi_instance_cull_rows(
                                        batch,
                                        &self.staged_instance_transforms,
                                    );
                                    self.frustum_cull_static_staging[batch_idx] = static_row;
                                    self.frustum_cull_dynamic_staging[batch_idx] = dynamic_row;
                                    static_dirty = true;
                                    continue;
                                }
                                let instance =
                                    &self.staged_instance_transforms[batch.instance_start as usize];
                                let model_cols = model_cols_from_affine_rows(instance);
                                self.frustum_cull_dynamic_staging[batch_idx] =
                                    FrustumCullDynamicGpu {
                                        model_0: model_cols[0],
                                        model_1: model_cols[1],
                                        model_2: model_cols[2],
                                        model_3: model_cols[3],
                                    };
                            }
                            let byte_start = (batch_span.start
                                * std::mem::size_of::<FrustumCullDynamicGpu>())
                                as u64;
                            queue.write_buffer(
                                &self.frustum_cull_dynamic_buffer,
                                byte_start,
                                bytemuck::cast_slice(
                                    &self.frustum_cull_dynamic_staging
                                        [batch_span.start..batch_span.end],
                                ),
                            );
                            if static_dirty {
                                let byte_start = (batch_span.start
                                    * std::mem::size_of::<FrustumCullStaticGpu>())
                                    as u64;
                                queue.write_buffer(
                                    &self.frustum_cull_static_buffer,
                                    byte_start,
                                    bytemuck::cast_slice(
                                        &self.frustum_cull_static_staging
                                            [batch_span.start..batch_span.end],
                                    ),
                                );
                            }
                        }
                        step_timing.cull_input_prep += cull_start.elapsed();
                    }
                }

                let frustum_start = Instant::now();
                let frustum = extract_frustum_planes(view_proj);
                let frustum_written = self.write_frustum_params_if_needed(queue, &frustum);
                step_timing.frustum_prep += frustum_start.elapsed();
                if !frustum_written {
                    step_timing.frustum_skipped = step_timing.frustum_skipped.saturating_add(1);
                }

                if hiz_active {
                    let hiz_start = Instant::now();
                    let hiz_written =
                        self.write_hiz_params_if_needed(queue, &uniform, self.draw_batches.len());
                    step_timing.hiz_prep += hiz_start.elapsed();
                    if !hiz_written {
                        step_timing.hiz_skipped = step_timing.hiz_skipped.saturating_add(1);
                    }
                } else {
                    step_timing.hiz_skipped = step_timing.hiz_skipped.saturating_add(1);
                }
            } else {
                step_timing.frustum_skipped = step_timing.frustum_skipped.saturating_add(1);
                step_timing.hiz_skipped = step_timing.hiz_skipped.saturating_add(1);
                step_timing.indirect_skipped = step_timing.indirect_skipped.saturating_add(1);
                step_timing.cull_input_skipped = step_timing.cull_input_skipped.saturating_add(1);
                self.frustum_gpu_inputs_valid = false;
            }
            // Multimesh cull inputs are topology-only (unchanged here); just keep
            // the shared frustum planes current so the cull uses this camera.
            if self.should_run_multimesh_cull() {
                let frustum = extract_frustum_planes(view_proj);
                self.write_frustum_params_if_needed(queue, &frustum);
                self.write_multimesh_cull_params_if_needed(queue);
            }
            // Transform patch moved rigid + multimesh casters; drop the cache.
            self.shadow_casters_dirty = true;
            self.update_shadow_state(queue, &camera, lighting, self.has_shadow_casters);
            self.last_draws.clear();
            self.last_draws.extend_from_slice(draws);
            self.last_draws_revision = draws_revision;
            self.last_total_drawn =
                self.staged_instance_transforms.len() + self.staged_multimesh_instances.len();
            self.last_prepare_step_timing = step_timing;
            return;
        }

        self.frustum_gpu_inputs_valid = false;
        self.last_draws.clear();
        self.last_draws.extend_from_slice(draws);
        self.last_draws_revision = draws_revision;

        let (regular_instance_hint, multimesh_instance_hint) =
            estimate_draw_instance_capacity(draws);
        self.staged_instance_transforms.clear();
        self.staged_instance_transforms
            .reserve(regular_instance_hint);
        self.staged_rigid_instance_meta.clear();
        self.staged_rigid_instance_meta
            .reserve(regular_instance_hint);
        self.staged_skinned_instance_meta.clear();
        self.staged_skinned_instance_meta
            .reserve(regular_instance_hint);
        self.staged_blend_shape_weights.clear();
        self.staged_blend_shape_instance_meta.clear();
        self.staged_blend_shape_instance_meta
            .reserve(regular_instance_hint);
        self.staged_skeletons.clear();
        self.staged_custom_params_meta.clear();
        self.staged_custom_params_values.clear();
        self.staged_custom_params_dedupe.clear();
        self.custom_params_meta_uploaded = 0;
        self.custom_params_values_uploaded = 0;
        self.staged_custom_params_meta_scratch.clear();
        self.staged_custom_params_values_scratch.clear();
        self.staged_custom_params_key_scratch.clear();
        self.draw_batches.clear();
        self.multimesh_batches.clear();
        self.staged_multimesh_instances.clear();
        self.staged_multimesh_instances
            .reserve(multimesh_instance_hint);
        self.staged_multimesh_draw_params.clear();
        self.multimesh_pose_pack_cache_seen.clear();
        self.draw_batches.reserve(draws.len());
        self.last_draw_instance_spans.clear();
        self.last_draw_instance_spans.reserve(draws.len());
        self.last_draw_instance_span_ranges.clear();
        self.last_draw_instance_span_ranges.reserve(draws.len());
        self.last_draw_multimesh_param_ranges.clear();
        self.last_draw_multimesh_param_ranges.reserve(draws.len());
        self.frustum_cull_static_staging.clear();
        self.frustum_cull_dynamic_staging.clear();
        self.indirect_staging.clear();
        let mut total_meshlets = 0usize;
        let frustum = extract_frustum_planes(view_proj);
        let default_mesh = self
            .resolve_builtin_mesh_asset("__cube__")
            .expect("cube mesh preset must exist");
        let mut debug_points_start: Option<u32> = None;
        let mut debug_points_count: u32 = 0;
        let mut debug_points_double_sided = false;
        let mut debug_points_local_center = [0.0f32; 3];
        let mut debug_point_instances = std::mem::take(&mut self.debug_point_instances_scratch);
        debug_point_instances.clear();
        let mut debug_edges_start: Option<u32> = None;
        let mut debug_edges_count: u32 = 0;
        let mut debug_edges_double_sided = false;
        let mut debug_edges_local_center = [0.0f32; 3];
        let mut debug_edge_instances = std::mem::take(&mut self.debug_edge_instances_scratch);
        debug_edge_instances.clear();
        let mut surface_entries = std::mem::take(&mut self.surface_entries_scratch);
        surface_entries.clear();
        let mut mesh_blends = std::mem::take(&mut self.mesh_blend_scratch);
        resolve_mesh_blends(draws, &mut mesh_blends);
        // Screen-space seam pass handles single-sample non-multimesh sources;
        // everything else keeps the in-material depth fade.
        if self.screen_blend_supported && self.sample_count == 1 {
            for (draw, blend) in draws.iter().zip(mesh_blends.iter_mut()) {
                if draw.dense_multimesh.is_none() {
                    promote_mesh_blend_screen_pass(blend);
                }
            }
        }

        for (draw_index, draw) in draws.iter().enumerate() {
            let resolved_blend = mesh_blends[draw_index];
            let draw_instance_start = self.staged_instance_transforms.len() as u32;
            let draw_span_start = self.last_draw_instance_spans.len();
            let draw_multimesh_param_start = self.staged_multimesh_draw_params.len() as u32;
            let is_debug_point = matches!(draw.kind, Draw3DKind::DebugPointCube);
            let is_debug_edge = matches!(draw.kind, Draw3DKind::DebugEdgeCylinder);
            let is_camera_stream_quad = matches!(draw.kind, Draw3DKind::CameraStreamQuad { .. });
            let mesh_source = match draw.kind {
                Draw3DKind::Mesh(mesh) => resources.mesh_source(mesh).unwrap_or("__cube__"),
                Draw3DKind::CameraStreamQuad { .. } => "__quad__",
                Draw3DKind::DebugPointCube => "__cube__",
                Draw3DKind::DebugEdgeCylinder => "__cylinder__",
            };
            let flat_builtin_double_sided = builtin_flat_mesh_double_sided(mesh_source);
            let mesh_asset = match draw.kind {
                Draw3DKind::Mesh(mesh_id) => self
                    .resolve_mesh_range(
                        device,
                        queue,
                        resources,
                        mesh_id,
                        mesh_source,
                        static_mesh_lookup,
                    )
                    .unwrap_or_else(|| default_mesh.clone()),
                Draw3DKind::CameraStreamQuad { .. } => self
                    .resolve_builtin_mesh_asset("__quad__")
                    .unwrap_or_else(|| default_mesh.clone()),
                Draw3DKind::DebugPointCube => self
                    .resolve_builtin_mesh_asset("__cube__")
                    .unwrap_or_else(|| default_mesh.clone()),
                Draw3DKind::DebugEdgeCylinder => self
                    .resolve_builtin_mesh_asset("__cylinder__")
                    .unwrap_or_else(|| default_mesh.clone()),
            };
            let lod_model = draw
                .instance_mats
                .first()
                .or_else(|| draw.dense_multimesh.as_ref().map(|dense| &dense.node_model));
            let active_lod = select_mesh_lod(&mesh_asset, lod_model, camera.position, draw.lod);
            surface_entries.clear();
            match draw.kind {
                Draw3DKind::DebugPointCube => {
                    let color = draw.debug_color.unwrap_or([1.0, 0.92, 0.2, 1.0]);
                    surface_entries.push(SurfaceEntry3D {
                        range: active_lod.full,
                        packed_range: None,
                        packed_lod_param_id: 0,
                        material: Material3D::Standard(StandardMaterial3D {
                            base_color_factor: color,
                            roughness_factor: 0.35,
                            metallic_factor: 0.0,
                            emissive_factor: [
                                color[0] * color[3] * 0.55,
                                color[1] * color[3] * 0.55,
                                color[2] * color[3] * 0.55,
                            ],
                            ..StandardMaterial3D::default()
                        }),
                    });
                }
                Draw3DKind::DebugEdgeCylinder => {
                    let color = draw.debug_color.unwrap_or([0.15, 0.95, 0.95, 1.0]);
                    surface_entries.push(SurfaceEntry3D {
                        range: active_lod.full,
                        packed_range: None,
                        packed_lod_param_id: 0,
                        material: Material3D::Standard(StandardMaterial3D {
                            base_color_factor: color,
                            roughness_factor: 0.6,
                            metallic_factor: 0.0,
                            emissive_factor: [
                                color[0] * color[3] * 0.4,
                                color[1] * color[3] * 0.4,
                                color[2] * color[3] * 0.4,
                            ],
                            ..StandardMaterial3D::default()
                        }),
                    });
                }
                Draw3DKind::CameraStreamQuad { texture, tint } => {
                    surface_entries.push(SurfaceEntry3D {
                        range: active_lod.full,
                        packed_range: None,
                        packed_lod_param_id: 0,
                        material: Material3D::Unlit(perro_render_bridge::UnlitMaterial3D {
                            base_color_factor: tint,
                            alpha_mode: 2,
                            double_sided: true,
                            base_color_texture: texture.index(),
                            ..perro_render_bridge::UnlitMaterial3D::default()
                        }),
                    });
                }
                Draw3DKind::Mesh(_) => {
                    for (surface_index, surface) in draw.surfaces.iter().enumerate() {
                        let Some(range) = active_lod.surface_ranges.get(surface_index).copied()
                        else {
                            continue;
                        };
                        let packed = active_lod.packed.and_then(|packed| {
                            packed
                                .surface_ranges
                                .get(surface_index)
                                .copied()
                                .map(|range| (range, packed.param_index))
                        });
                        let base_material = surface
                            .material
                            .and_then(|id| resources.material(id))
                            .unwrap_or_default();
                        surface_entries.push(SurfaceEntry3D {
                            range,
                            packed_range: packed.map(|(range, _)| range),
                            packed_lod_param_id: packed.map(|(_, param)| param).unwrap_or(0),
                            material: apply_surface_binding(base_material, surface),
                        });
                    }
                    if surface_entries.is_empty() {
                        surface_entries.push(SurfaceEntry3D {
                            range: active_lod.full,
                            packed_range: active_lod.packed.map(|packed| packed.full),
                            packed_lod_param_id: active_lod
                                .packed
                                .map(|packed| packed.param_index)
                                .unwrap_or(0),
                            material: Material3D::default(),
                        });
                    }
                }
            }
            if surface_entries.is_empty() {
                self.last_draw_instance_spans
                    .push(draw_instance_start..(self.staged_instance_transforms.len() as u32));
                let draw_span_end = self.last_draw_instance_spans.len();
                self.last_draw_instance_span_ranges
                    .push(draw_span_start..draw_span_end);
                self.last_draw_multimesh_param_ranges.push(
                    draw_multimesh_param_start..(self.staged_multimesh_draw_params.len() as u32),
                );
                continue;
            }
            if let Some(dense) = &draw.dense_multimesh {
                let draw_model = Mat4::from_cols_array_2d(&dense.node_model);
                for entry in surface_entries.iter() {
                    let material = &entry.material;
                    let params = material.standard_params();
                    let material_kind = self.material_pipeline_kind(
                        device,
                        RenderPath3D::MultiMesh,
                        material,
                        static_shader_lookup,
                    );
                    let custom_params = self.stage_custom_params(material);
                    let base_linear = crate::srgb_to_linear_rgb([
                        params.base_color_factor[0],
                        params.base_color_factor[1],
                        params.base_color_factor[2],
                    ]);
                    let packed_color = pack_unorm4x8([
                        base_linear[0],
                        base_linear[1],
                        base_linear[2],
                        params.base_color_factor[3],
                    ]);
                    let packed_emissive = pack_emissive_hdr(params.emissive_factor);
                    let draw_param_index = self.staged_multimesh_draw_params.len() as u32;
                    self.staged_multimesh_draw_params
                        .push(MultiMeshDrawParamGpu {
                            model_row_0: [
                                draw_model.x_axis.x,
                                draw_model.y_axis.x,
                                draw_model.z_axis.x,
                                draw_model.w_axis.x,
                            ],
                            model_row_1: [
                                draw_model.x_axis.y,
                                draw_model.y_axis.y,
                                draw_model.z_axis.y,
                                draw_model.w_axis.y,
                            ],
                            model_row_2: [
                                draw_model.x_axis.z,
                                draw_model.y_axis.z,
                                draw_model.z_axis.z,
                                draw_model.w_axis.z,
                            ],
                            packed_color,
                            packed_emissive,
                            scale_bits: dense.instance_scale.max(0.0001).to_bits(),
                            packed_blend_params: resolved_blend.packed_params,
                            custom_params: [custom_params.0, custom_params.1],
                            packed_bleed: 0,
                            _pad: 0,
                        });
                    let mirrored_winding = draw_model.determinant() < 0.0;
                    let instance_start = self.staged_multimesh_instances.len() as u32;
                    // Item 3: reuse packed geometry lanes when this exact pose Arc
                    // was packed on a prior build. Only quaternion pack + lane copy
                    // are cached; draw_id/blend_meta_id are build-order specific
                    // and stay fresh, and blend metadata is still staged per
                    // instance below.
                    let pose_key = Arc::as_ptr(&dense.instances) as *const () as usize;
                    self.multimesh_pose_pack_cache_seen.insert(pose_key);
                    let cached_geom = self
                        .multimesh_pose_pack_cache
                        .get(&pose_key)
                        // Pinned source Arc guarantees pointer identity; still verify
                        // it is the same Arc (defensive) and the same length.
                        .filter(|(src, packed)| {
                            Arc::ptr_eq(src, &dense.instances)
                                && packed.len() == dense.instances.len()
                        })
                        .map(|(_, packed)| packed.clone());
                    for (index, pose) in dense.instances.iter().enumerate() {
                        let weights = if pose.has_blend_shape_weight_override {
                            pose.blend_shape_weights.as_ref()
                        } else {
                            draw.blend_shape_weights.as_ref()
                        };
                        let blend_meta_id = self.staged_blend_shape_instance_meta.len() as u32;
                        self.stage_blend_shape_instance(&mesh_asset, weights);
                        let (position, rotation, scale) = match &cached_geom {
                            Some(geom) => {
                                let g = geom[index];
                                (g.position, g.rotation, g.scale)
                            }
                            None => (
                                pose.position,
                                pack_quat_snorm16x4(pose.rotation),
                                pose.scale,
                            ),
                        };
                        self.staged_multimesh_instances.push(MultiMeshInstanceGpu {
                            position,
                            rotation,
                            scale,
                            draw_id: draw_param_index,
                            blend_meta_id,
                        });
                    }
                    if cached_geom.is_none() && !dense.instances.is_empty() {
                        let packed: Arc<[MultiMeshPosePacked]> = self.staged_multimesh_instances
                            [instance_start as usize..]
                            .iter()
                            .map(|inst| MultiMeshPosePacked {
                                position: inst.position,
                                rotation: inst.rotation,
                                scale: inst.scale,
                            })
                            .collect();
                        self.multimesh_pose_pack_cache
                            .insert(pose_key, (dense.instances.clone(), packed));
                    }
                    let instance_count = (self.staged_multimesh_instances.len() as u32)
                        .saturating_sub(instance_start);
                    if instance_count > 0 {
                        self.multimesh_batches.push(MultiMeshBatch {
                            mesh: entry.range,
                            instance_start,
                            instance_count,
                            draw_param_index,
                            mesh_local_radius: mesh_asset.bounds_radius.max(0.0),
                            double_sided: params.double_sided
                                || mirrored_winding
                                || flat_builtin_double_sided,
                            mesh_blend: resolved_mesh_blend_active(resolved_blend),
                            casts_shadows: draw.cast_shadows,
                            material_kind,
                        });
                    }
                }
                self.last_draw_instance_spans
                    .push(draw_instance_start..(self.staged_instance_transforms.len() as u32));
                let draw_span_end = self.last_draw_instance_spans.len();
                self.last_draw_instance_span_ranges
                    .push(draw_span_start..draw_span_end);
                self.last_draw_multimesh_param_ranges.push(
                    draw_multimesh_param_start..(self.staged_multimesh_draw_params.len() as u32),
                );
                continue;
            }
            // CPU occlusion query mode works at object granularity.
            // Force whole-mesh batching in that mode so each object can be queried.
            let prefer_packed_lod = active_lod.packed.is_some()
                && draw.skeleton.is_none()
                && mesh_asset.blend_shape_target_count == 0
                && !resolved_mesh_blend_active(resolved_blend)
                && surface_entries
                    .first()
                    .map(|entry| matches!(entry.material, Material3D::Standard(_)))
                    .unwrap_or(false);
            let builtin_primitive_source = is_builtin_primitive_mesh_source(mesh_source);
            let allow_meshlets = draw.meshlet_override.unwrap_or(!builtin_primitive_source);
            let use_meshlets = !is_debug_point
                && !is_debug_edge
                && !is_camera_stream_quad
                && !prefer_packed_lod
                && self.meshlets_enabled
                && allow_meshlets
                && !active_lod.meshlets.is_empty()
                && surface_entries.len() == 1
                && !self.cpu_occlusion_enabled;
            total_meshlets = total_meshlets.saturating_add(if use_meshlets {
                active_lod.meshlets.len()
            } else {
                1
            });

            // Keep casters available even when off-screen so directional shadow fitting
            // stays stable during camera orbit/rotation.

            if !use_meshlets {
                let occlusion_key = draw.node.as_u64();
                if self.cpu_occlusion_enabled
                    && !is_debug_point
                    && !is_debug_edge
                    && !self.should_probe_or_draw(occlusion_key)
                {
                    self.last_draw_instance_spans
                        .push(draw_instance_start..(self.staged_instance_transforms.len() as u32));
                    let draw_span_end = self.last_draw_instance_spans.len();
                    self.last_draw_instance_span_ranges
                        .push(draw_span_start..draw_span_end);
                    self.last_draw_multimesh_param_ranges.push(
                        draw_multimesh_param_start
                            ..(self.staged_multimesh_draw_params.len() as u32),
                    );
                    continue;
                }
                let occlusion_query =
                    if (is_debug_point || is_debug_edge) && self.cpu_occlusion_enabled {
                        // Debug primitives are batched into shared instanced draws, so per-object CPU
                        // occlusion queries are not meaningful for these draws.
                        None
                    } else if occlusion_capture_this_frame {
                        Some(self.push_occlusion_query_key(occlusion_key))
                    } else {
                        None
                    };
                let (skeleton_start, skeleton_count) = if let Some(skeleton) = &draw.skeleton {
                    let start = self.staged_skeletons.len() as u32;
                    let count = skeleton.matrices.len() as u32;
                    self.staged_skeletons
                        .extend(skeleton.matrices.iter().map(skeleton_bone_rows));
                    (start, count)
                } else {
                    (0, 0)
                };
                let instance_mats = draw.instance_mats.as_ref();
                if is_debug_point {
                    let material = &surface_entries[0].material;
                    let (custom_params_offset, custom_params_len) =
                        self.stage_custom_params(material);
                    let standard_params = material.standard_params();
                    self.ensure_material_texture_slot(
                        device,
                        queue,
                        resources,
                        standard_params.base_color_texture,
                        mesh_source,
                        static_texture_lookup,
                    );
                    if debug_point_instances.is_empty() {
                        debug_points_double_sided =
                            material.standard_params().double_sided || self.meshlet_debug_view;
                        debug_points_local_center = mesh_asset.bounds_center;
                    }
                    for model in instance_mats.iter().copied() {
                        debug_point_instances.push(build_instance(
                            model,
                            material,
                            BuildInstanceArgs {
                                debug_view: self.meshlet_debug_view,
                                debug_color: debug_color(draw.node.as_u64()),
                                mesh_blend: resolved_blend,
                                skeleton_start,
                                skeleton_count,
                                custom_params_offset,
                                custom_params_len,
                                packed_lod_param_id: 0,
                                receive_shadows: false,
                            },
                        ));
                        debug_points_count = debug_points_count.saturating_add(1);
                    }
                } else if is_debug_edge {
                    let material = &surface_entries[0].material;
                    let (custom_params_offset, custom_params_len) =
                        self.stage_custom_params(material);
                    let standard_params = material.standard_params();
                    self.ensure_material_texture_slot(
                        device,
                        queue,
                        resources,
                        standard_params.base_color_texture,
                        mesh_source,
                        static_texture_lookup,
                    );
                    if debug_edge_instances.is_empty() {
                        debug_edges_double_sided =
                            material.standard_params().double_sided || self.meshlet_debug_view;
                        debug_edges_local_center = mesh_asset.bounds_center;
                    }
                    for model in instance_mats.iter().copied() {
                        debug_edge_instances.push(build_instance(
                            model,
                            material,
                            BuildInstanceArgs {
                                debug_view: self.meshlet_debug_view,
                                debug_color: debug_color(draw.node.as_u64()),
                                mesh_blend: resolved_blend,
                                skeleton_start,
                                skeleton_count,
                                custom_params_offset,
                                custom_params_len,
                                packed_lod_param_id: 0,
                                receive_shadows: false,
                            },
                        ));
                        debug_edges_count = debug_edges_count.saturating_add(1);
                    }
                } else {
                    for entry in surface_entries.iter() {
                        let material = &entry.material;
                        let standard_params = material.standard_params();
                        self.ensure_material_texture_slot(
                            device,
                            queue,
                            resources,
                            standard_params.base_color_texture,
                            mesh_source,
                            static_texture_lookup,
                        );
                        let render_path = if skeleton_count > 0 {
                            RenderPath3D::Skinned
                        } else {
                            RenderPath3D::Rigid
                        };
                        let material_kind = self.material_pipeline_kind(
                            device,
                            render_path,
                            material,
                            static_shader_lookup,
                        );
                        let packed_lod = render_path == RenderPath3D::Rigid
                            && matches!(material_kind, MaterialPipelineKind::Standard)
                            && !resolved_mesh_blend_active(resolved_blend)
                            && mesh_asset.blend_shape_target_count == 0
                            && entry.packed_range.is_some();
                        let mesh_range = if packed_lod {
                            entry.packed_range.unwrap_or(entry.range)
                        } else {
                            entry.range
                        };
                        let packed_lod_param_id = if packed_lod {
                            entry.packed_lod_param_id
                        } else {
                            0
                        };
                        let (custom_params_offset, custom_params_len) =
                            self.stage_custom_params(material);
                        let instance_start = self.staged_instance_transforms.len() as u32;
                        let caster_debug = self.shadow_caster_debug_view
                            && draw.cast_shadows
                            && standard_params.alpha_mode == 0
                            && !is_camera_stream_quad;
                        for model in instance_mats.iter().copied() {
                            let instance = build_instance(
                                model,
                                material,
                                BuildInstanceArgs {
                                    debug_view: self.meshlet_debug_view,
                                    debug_color: if caster_debug {
                                        [1.0, 0.22, 0.82, 1.0]
                                    } else {
                                        debug_color(draw.node.as_u64())
                                    },
                                    mesh_blend: resolved_blend,
                                    skeleton_start,
                                    skeleton_count,
                                    custom_params_offset,
                                    custom_params_len,
                                    packed_lod_param_id,
                                    receive_shadows: draw.receive_shadows,
                                },
                            );
                            self.staged_instance_transforms.push(instance.transform);
                            self.staged_rigid_instance_meta.push(instance.rigid_meta);
                            self.staged_skinned_instance_meta
                                .push(instance.skinned_meta);
                            self.stage_blend_shape_instance(
                                &mesh_asset,
                                draw.blend_shape_weights.as_ref(),
                            );
                        }
                        let instance_count = (self.staged_instance_transforms.len() as u32)
                            .saturating_sub(instance_start);
                        if instance_count > 0 {
                            let uses_custom_shader = material_kind.uses_custom_shader();
                            let mirrored_winding = instance_mats
                                .iter()
                                .any(|model| Mat4::from_cols_array_2d(model).determinant() < 0.0);
                            // Multi-instance batches keep the tight local bound;
                            // the cull upload expands it into a merged
                            // per-instance world sphere.
                            let occlusion_bounds =
                                (mesh_asset.bounds_center, mesh_asset.bounds_radius);
                            push_draw_batch(
                                &mut self.draw_batches,
                                DrawBatchPush {
                                    render_path,
                                    mesh: mesh_range,
                                    instance_start,
                                    instance_count,
                                    double_sided: standard_params.double_sided
                                        || self.meshlet_debug_view
                                        || mirrored_winding
                                        || flat_builtin_double_sided,
                                    packed_lod,
                                    material_kind,
                                    alpha_mode: standard_params.alpha_mode,
                                    base_color_texture_slot: standard_params.base_color_texture,
                                    local_bounds: occlusion_bounds,
                                    occlusion_query,
                                    disable_hiz_occlusion: uses_custom_shader
                                        || standard_params.alpha_mode == 2
                                        || resolved_mesh_blend_active(resolved_blend),
                                    casts_shadows: draw.cast_shadows && !is_camera_stream_quad,
                                    receives_shadows: draw.receive_shadows,
                                    mesh_blend: resolved_mesh_blend_active(resolved_blend)
                                        && !resolved_mesh_blend_screen_pass(resolved_blend),
                                    mesh_blend_screen: resolved_mesh_blend_screen_pass(
                                        resolved_blend,
                                    ),
                                    mesh_blend_params: resolved_blend.packed_params,
                                    mesh_blend_depth: resolved_mesh_blend_depth_receiver(
                                        resolved_blend,
                                    ),
                                    blend_layers: draw.blend.blend_layers.bits(),
                                    blend_mask: draw.blend.blend_mask.bits(),
                                },
                            );
                        }
                    }
                }
            } else {
                let material = &surface_entries[0].material;
                let standard_params = material.standard_params();
                self.ensure_material_texture_slot(
                    device,
                    queue,
                    resources,
                    standard_params.base_color_texture,
                    mesh_source,
                    static_texture_lookup,
                );
                let material_kind = self.material_pipeline_kind(
                    device,
                    if draw.skeleton.is_some() {
                        RenderPath3D::Skinned
                    } else {
                        RenderPath3D::Rigid
                    },
                    material,
                    static_shader_lookup,
                );
                let (custom_params_offset, custom_params_len) = self.stage_custom_params(material);
                let (skeleton_start, skeleton_count) = if let Some(skeleton) = &draw.skeleton {
                    let start = self.staged_skeletons.len() as u32;
                    let count = skeleton.matrices.len() as u32;
                    self.staged_skeletons
                        .extend(skeleton.matrices.iter().map(skeleton_bone_rows));
                    (start, count)
                } else {
                    (0, 0)
                };
                let instance_mats = draw.instance_mats.as_ref();
                let caster_debug = self.shadow_caster_debug_view
                    && draw.cast_shadows
                    && standard_params.alpha_mode == 0
                    && !is_camera_stream_quad;
                let meshlet_casts_shadows = draw.cast_shadows && !self.disable_meshlet_shadows;
                let render_path = if skeleton_count > 0 {
                    RenderPath3D::Skinned
                } else {
                    RenderPath3D::Rigid
                };
                let uses_custom_shader = material_kind.uses_custom_shader();
                // Per-draw invariants: winding + custom-shader flag do not vary per
                // meshlet. Hoist the determinant any-scan out of the meshlet loop.
                let mirrored_winding = instance_mats
                    .iter()
                    .any(|model| Mat4::from_cols_array_2d(model).determinant() < 0.0);
                // Share one instance span across every meshlet batch of this draw:
                // meshlet batches differ only by index range, not per-instance data,
                // so stage the instances once and point all batches at the same span.
                // Debug view keeps per-meshlet staging: it bakes a distinct
                // debug_color per meshlet into the instance meta (debug-only, perf
                // irrelevant).
                let shared_instances = !self.meshlet_debug_view;
                let shared_instance_start = self.staged_instance_transforms.len() as u32;
                let mut shared_instance_count = 0u32;
                if shared_instances {
                    for model in instance_mats.iter().copied() {
                        let instance = build_instance(
                            model,
                            material,
                            BuildInstanceArgs {
                                debug_view: false,
                                debug_color: if caster_debug {
                                    if meshlet_casts_shadows {
                                        [0.05, 0.9, 1.0, 1.0]
                                    } else {
                                        [1.0, 0.85, 0.1, 1.0]
                                    }
                                } else {
                                    debug_color(draw.node.as_u64())
                                },
                                mesh_blend: resolved_blend,
                                skeleton_start,
                                skeleton_count,
                                custom_params_offset,
                                custom_params_len,
                                packed_lod_param_id: 0,
                                receive_shadows: draw.receive_shadows,
                            },
                        );
                        self.staged_instance_transforms.push(instance.transform);
                        self.staged_rigid_instance_meta.push(instance.rigid_meta);
                        self.staged_skinned_instance_meta
                            .push(instance.skinned_meta);
                        self.stage_blend_shape_instance(
                            &mesh_asset,
                            draw.blend_shape_weights.as_ref(),
                        );
                    }
                    shared_instance_count = (self.staged_instance_transforms.len() as u32)
                        .saturating_sub(shared_instance_start);
                    if shared_instance_count == 0 {
                        // No instances staged: nothing for any meshlet batch to draw.
                        self.last_draw_instance_spans.push(
                            draw_instance_start..(self.staged_instance_transforms.len() as u32),
                        );
                        let draw_span_end = self.last_draw_instance_spans.len();
                        self.last_draw_instance_span_ranges
                            .push(draw_span_start..draw_span_end);
                        self.last_draw_multimesh_param_ranges.push(
                            draw_multimesh_param_start
                                ..(self.staged_multimesh_draw_params.len() as u32),
                        );
                        continue;
                    }
                }
                for meshlet in active_lod.meshlets.iter().copied() {
                    // Keep meshlet casters for stable shadow fitting even when off-screen.
                    // CPU query occlusion at meshlet granularity self-occludes dynamic meshes.
                    // Keep meshlet occlusion GPU-driven only; CPU mode skips meshlet occlusion.
                    let occlusion_query = None;
                    let (instance_start, instance_count) = if shared_instances {
                        (shared_instance_start, shared_instance_count)
                    } else {
                        let instance_start = self.staged_instance_transforms.len() as u32;
                        for model in instance_mats.iter().copied() {
                            let instance = build_instance(
                                model,
                                material,
                                BuildInstanceArgs {
                                    debug_view: true,
                                    debug_color: if caster_debug {
                                        if meshlet_casts_shadows {
                                            [0.05, 0.9, 1.0, 1.0]
                                        } else {
                                            [1.0, 0.85, 0.1, 1.0]
                                        }
                                    } else {
                                        debug_color(
                                            (draw.node.as_u64() << 32) ^ meshlet.index_start as u64,
                                        )
                                    },
                                    mesh_blend: resolved_blend,
                                    skeleton_start,
                                    skeleton_count,
                                    custom_params_offset,
                                    custom_params_len,
                                    packed_lod_param_id: 0,
                                    receive_shadows: draw.receive_shadows,
                                },
                            );
                            self.staged_instance_transforms.push(instance.transform);
                            self.staged_rigid_instance_meta.push(instance.rigid_meta);
                            self.staged_skinned_instance_meta
                                .push(instance.skinned_meta);
                            self.stage_blend_shape_instance(
                                &mesh_asset,
                                draw.blend_shape_weights.as_ref(),
                            );
                        }
                        let instance_count = (self.staged_instance_transforms.len() as u32)
                            .saturating_sub(instance_start);
                        (instance_start, instance_count)
                    };
                    if instance_count == 0 {
                        continue;
                    }
                    // Per-meshlet local bounds give tighter frustum/occlusion
                    // rejection; multi-instance batches expand them into a
                    // merged per-instance world sphere at cull upload.
                    let (occlusion_center, occlusion_radius) =
                        (meshlet.center, meshlet.radius.max(0.001));
                    push_draw_batch(
                        &mut self.draw_batches,
                        DrawBatchPush {
                            render_path,
                            mesh: MeshRange {
                                index_start: meshlet.index_start,
                                index_count: meshlet.index_count,
                                base_vertex: mesh_asset.full.base_vertex,
                            },
                            instance_start,
                            instance_count,
                            packed_lod: false,
                            double_sided: standard_params.double_sided
                                || self.meshlet_debug_view
                                || mirrored_winding
                                || flat_builtin_double_sided,
                            material_kind: material_kind.clone(),
                            alpha_mode: standard_params.alpha_mode,
                            base_color_texture_slot: standard_params.base_color_texture,
                            local_bounds: (occlusion_center, occlusion_radius),
                            occlusion_query,
                            disable_hiz_occlusion: uses_custom_shader
                                || standard_params.alpha_mode == 2
                                || resolved_mesh_blend_active(resolved_blend),
                            casts_shadows: meshlet_casts_shadows,
                            receives_shadows: draw.receive_shadows,
                            mesh_blend: resolved_mesh_blend_active(resolved_blend)
                                && !resolved_mesh_blend_screen_pass(resolved_blend),
                            mesh_blend_screen: resolved_mesh_blend_screen_pass(resolved_blend),
                            mesh_blend_params: resolved_blend.packed_params,
                            mesh_blend_depth: resolved_mesh_blend_depth_receiver(resolved_blend),
                            blend_layers: draw.blend.blend_layers.bits(),
                            blend_mask: draw.blend.blend_mask.bits(),
                        },
                    );
                }
            }
            self.last_draw_instance_spans
                .push(draw_instance_start..(self.staged_instance_transforms.len() as u32));
            let draw_span_end = self.last_draw_instance_spans.len();
            self.last_draw_instance_span_ranges
                .push(draw_span_start..draw_span_end);
            self.last_draw_multimesh_param_ranges
                .push(draw_multimesh_param_start..(self.staged_multimesh_draw_params.len() as u32));
        }
        self.mesh_blend_scratch = mesh_blends;
        self.surface_entries_scratch = surface_entries;
        // Drop pose-pack cache entries for pose Arcs not present this build so the
        // cache (and the source Arcs it pins) does not grow unbounded.
        if self.multimesh_pose_pack_cache.len() > self.multimesh_pose_pack_cache_seen.len() {
            let seen = &self.multimesh_pose_pack_cache_seen;
            self.multimesh_pose_pack_cache
                .retain(|key, _| seen.contains(key));
        }
        if !debug_point_instances.is_empty() {
            debug_points_start = Some(self.staged_instance_transforms.len() as u32);
            for instance in debug_point_instances.drain(..) {
                self.staged_instance_transforms.push(instance.transform);
                self.staged_rigid_instance_meta.push(instance.rigid_meta);
                self.staged_skinned_instance_meta
                    .push(instance.skinned_meta);
                self.stage_blend_shape_instance(&default_mesh, &[]);
            }
        }
        if !debug_edge_instances.is_empty() {
            debug_edges_start = Some(self.staged_instance_transforms.len() as u32);
            for instance in debug_edge_instances.drain(..) {
                self.staged_instance_transforms.push(instance.transform);
                self.staged_rigid_instance_meta.push(instance.rigid_meta);
                self.staged_skinned_instance_meta
                    .push(instance.skinned_meta);
                self.stage_blend_shape_instance(&default_mesh, &[]);
            }
        }
        if let Some(instance_start) = debug_points_start
            && debug_points_count > 0
        {
            let material_kind = MaterialPipelineKind::Standard;
            let state_key = draw_batch_state_key(
                RenderPath3D::Rigid,
                true,
                debug_points_double_sided,
                0,
                false,
                &material_kind,
            );
            self.draw_batches.push(DrawBatch {
                state_key,
                render_state: render_state_key(
                    state_key,
                    MATERIAL_TEXTURE_NONE,
                    default_mesh.full.index_start,
                    default_mesh.full.base_vertex,
                    true,
                    0,
                    false,
                ),
                mesh: default_mesh.full,
                instance_start,
                instance_count: debug_points_count,
                path: RenderPath3D::Rigid,
                packed_lod: false,
                double_sided: debug_points_double_sided,
                material_kind,
                alpha_mode: 0,
                draw_on_top: true,
                base_color_texture_slot: MATERIAL_TEXTURE_NONE,
                local_center: debug_points_local_center,
                local_radius: 1.0e9,
                occlusion_query: None,
                disable_hiz_occlusion: true,
                casts_shadows: false,
                receives_shadows: false,
                mesh_blend: false,
                mesh_blend_screen: false,
                mesh_blend_params: 0,
                mesh_blend_depth: false,
                blend_layers: BitMask::ALL.bits(),
                blend_mask: BitMask::NONE.bits(),
                order_index: self.draw_batches.len() as u32,
            });
        }
        if let Some(instance_start) = debug_edges_start
            && debug_edges_count > 0
        {
            let debug_edge_mesh = self
                .resolve_builtin_mesh_asset("__cylinder__")
                .unwrap_or_else(|| default_mesh.clone());
            let material_kind = MaterialPipelineKind::Standard;
            let state_key = draw_batch_state_key(
                RenderPath3D::Rigid,
                true,
                debug_edges_double_sided,
                0,
                false,
                &material_kind,
            );
            self.draw_batches.push(DrawBatch {
                state_key,
                render_state: render_state_key(
                    state_key,
                    MATERIAL_TEXTURE_NONE,
                    debug_edge_mesh.full.index_start,
                    debug_edge_mesh.full.base_vertex,
                    true,
                    0,
                    false,
                ),
                mesh: debug_edge_mesh.full,
                instance_start,
                instance_count: debug_edges_count,
                path: RenderPath3D::Rigid,
                packed_lod: false,
                double_sided: debug_edges_double_sided,
                material_kind,
                alpha_mode: 0,
                draw_on_top: true,
                base_color_texture_slot: MATERIAL_TEXTURE_NONE,
                local_center: debug_edges_local_center,
                local_radius: 1.0e9,
                occlusion_query: None,
                disable_hiz_occlusion: true,
                casts_shadows: false,
                receives_shadows: false,
                mesh_blend: false,
                mesh_blend_screen: false,
                mesh_blend_params: 0,
                mesh_blend_depth: false,
                blend_layers: BitMask::ALL.bits(),
                blend_mask: BitMask::NONE.bits(),
                order_index: self.draw_batches.len() as u32,
            });
        }
        // Alpha batches must draw back-to-front by camera distance; their sort
        // key is order_index, so rewrite it from submission order to inverted
        // distance bits (monotonic for non-negative floats) before sorting.
        let cam_pos = Vec3::from(camera.position);
        for batch in self.draw_batches.iter_mut() {
            if batch.render_state.batch_kind != RenderBatchKind::Alpha {
                continue;
            }
            let Some(inst) = self
                .staged_instance_transforms
                .get(batch.instance_start as usize)
            else {
                continue;
            };
            let model = Mat4::from_cols_array_2d(&model_cols_from_affine_rows(inst));
            let center = (model * Vec3::from(batch.local_center).extend(1.0)).truncate();
            if !center.is_finite() {
                continue;
            }
            batch.order_index = u32::MAX - cam_pos.distance(center).to_bits();
        }
        if !draw_batches_sorted(&self.draw_batches) {
            if self.draw_batches.len() >= PARALLEL_BATCH_SORT_MIN {
                self.draw_batches
                    .par_sort_unstable_by(compare_draw_batch_keys);
            } else {
                self.draw_batches.sort_unstable_by(compare_draw_batch_keys);
            }
        }
        self.compact_sorted_draw_batches(draws.len());
        self.rebuild_batch_views();
        self.rebuild_mesh_blend_receivers();
        self.prepare_mesh_blend_screen(device, queue);
        self.apply_local_color_bleed();
        if !multimesh_batches_sorted(&self.multimesh_batches) {
            if self.multimesh_batches.len() >= PARALLEL_BATCH_SORT_MIN {
                self.multimesh_batches
                    .par_sort_unstable_by_key(multimesh_batch_sort_key);
            } else {
                self.multimesh_batches
                    .sort_unstable_by_key(multimesh_batch_sort_key);
            }
        }
        self.compact_sorted_multimesh_batches();
        if HIZ_DEBUG_READBACK_ENABLED {
            self.debug_frustum_visible_est = 0;
            for batch in &self.draw_batches {
                let model = model_cols_from_affine_rows(
                    &self.staged_instance_transforms[batch.instance_start as usize],
                );
                if bounds_in_frustum(model, batch.local_center, batch.local_radius, &frustum) {
                    self.debug_frustum_visible_est =
                        self.debug_frustum_visible_est.saturating_add(1);
                }
            }
        }
        // has_shadow_casters set in rebuild_batch_views above.
        if occlusion_capture_this_frame {
            self.ensure_occlusion_query_capacity(
                device,
                self.occlusion_query_keys_this_frame.len() as u32,
            );
        }
        self.ensure_instance_transform_capacity(device, self.staged_instance_transforms.len());
        self.ensure_rigid_instance_meta_capacity(device, self.staged_rigid_instance_meta.len());
        self.ensure_skinned_instance_meta_capacity(device, self.staged_skinned_instance_meta.len());
        self.ensure_blend_shape_weight_capacity(
            device,
            self.staged_blend_shape_weights.len().max(1),
        );
        self.ensure_blend_shape_instance_meta_capacity(
            device,
            self.staged_blend_shape_instance_meta.len().max(1),
        );
        if !self.staged_instance_transforms.is_empty() {
            queue.write_buffer(
                &self.instance_transform_buffer,
                0,
                bytemuck::cast_slice(&self.staged_instance_transforms),
            );
        }
        if !self.staged_rigid_instance_meta.is_empty() {
            queue.write_buffer(
                &self.rigid_instance_meta_buffer,
                0,
                bytemuck::cast_slice(&self.staged_rigid_instance_meta),
            );
        }
        if !self.staged_skinned_instance_meta.is_empty() {
            queue.write_buffer(
                &self.skinned_instance_meta_buffer,
                0,
                bytemuck::cast_slice(&self.staged_skinned_instance_meta),
            );
        }
        if !self.staged_blend_shape_weights.is_empty() {
            queue.write_buffer(
                &self.blend_shape_weight_buffer,
                0,
                bytemuck::cast_slice(&self.staged_blend_shape_weights),
            );
        }
        if !self.staged_blend_shape_instance_meta.is_empty() {
            queue.write_buffer(
                &self.blend_shape_instance_meta_buffer,
                0,
                bytemuck::cast_slice(&self.staged_blend_shape_instance_meta),
            );
        }
        self.ensure_skeleton_capacity(device, self.staged_skeletons.len().max(1));
        if !self.staged_skeletons.is_empty() {
            queue.write_buffer(
                &self.skeleton_buffer,
                0,
                bytemuck::cast_slice(&self.staged_skeletons),
            );
        }
        self.ensure_custom_params_capacity(
            device,
            self.staged_custom_params_meta.len().max(1),
            self.staged_custom_params_values.len().max(1),
        );
        if self.custom_params_meta_uploaded < self.staged_custom_params_meta.len() {
            let upload_start = self.custom_params_meta_uploaded;
            let byte_start = upload_start as u64 * std::mem::size_of::<u32>() as u64;
            queue.write_buffer(
                &self.custom_params_meta_buffer,
                byte_start,
                bytemuck::cast_slice(&self.staged_custom_params_meta[upload_start..]),
            );
            self.custom_params_meta_uploaded = self.staged_custom_params_meta.len();
        }
        if self.custom_params_values_uploaded < self.staged_custom_params_values.len() {
            let upload_start = self.custom_params_values_uploaded;
            let byte_start = upload_start as u64 * std::mem::size_of::<f32>() as u64;
            queue.write_buffer(
                &self.custom_params_values_buffer,
                byte_start,
                bytemuck::cast_slice(&self.staged_custom_params_values[upload_start..]),
            );
            self.custom_params_values_uploaded = self.staged_custom_params_values.len();
        }
        self.ensure_multimesh_draw_params_capacity(
            device,
            self.staged_multimesh_draw_params.len().max(1),
        );
        if !self.staged_multimesh_draw_params.is_empty() {
            queue.write_buffer(
                &self.multimesh_draw_params_buffer,
                0,
                bytemuck::cast_slice(&self.staged_multimesh_draw_params),
            );
        }
        self.ensure_multimesh_instance_capacity(
            device,
            self.staged_multimesh_instances.len().max(1),
        );
        if !self.staged_multimesh_instances.is_empty() {
            queue.write_buffer(
                &self.multimesh_instance_buffer,
                0,
                bytemuck::cast_slice(&self.staged_multimesh_instances),
            );
        }
        // Multimesh GPU cull inputs are topology-only, so they only need a
        // rebuild here on the full path; transform-only fast paths keep them
        // valid (they patch draw-param model rows, not batch topology).
        if self.should_run_multimesh_cull() {
            self.rebuild_multimesh_cull_inputs(device, queue);
            self.write_multimesh_cull_params_if_needed(queue);
            // The cull shader reads frustum planes from the shared rigid params
            // buffer; ensure they are current even if rigid cull is inactive.
            self.write_frustum_params_if_needed(queue, &frustum);
        }
        let frustum_cull_active = self.should_run_frustum_cull();
        let hiz_active = self.should_run_hiz_occlusion(frustum_cull_active);
        if frustum_cull_active {
            let indirect_start = Instant::now();
            self.ensure_frustum_cull_capacity(device, self.draw_batches.len());
            self.indirect_staging.clear();
            self.indirect_staging.reserve(self.draw_batches.len());
            for batch in &self.draw_batches {
                self.indirect_staging.push(DrawIndexedIndirectGpu {
                    index_count: batch.mesh.index_count,
                    instance_count: batch.instance_count,
                    first_index: batch.mesh.index_start,
                    base_vertex: batch.mesh.base_vertex,
                    first_instance: batch.instance_start,
                });
            }
            queue.write_buffer(
                &self.indirect_buffer,
                0,
                bytemuck::cast_slice(&self.indirect_staging),
            );
            step_timing.indirect_prep += indirect_start.elapsed();

            let cull_start = Instant::now();
            self.rebuild_frustum_cull_items(queue);
            step_timing.cull_input_prep += cull_start.elapsed();

            let frustum_start = Instant::now();
            let frustum_written = self.write_frustum_params_if_needed(queue, &frustum);
            step_timing.frustum_prep += frustum_start.elapsed();
            if !frustum_written {
                step_timing.frustum_skipped = step_timing.frustum_skipped.saturating_add(1);
            }
            if hiz_active {
                let hiz_start = Instant::now();
                let hiz_written =
                    self.write_hiz_params_if_needed(queue, &uniform, self.draw_batches.len());
                step_timing.hiz_prep += hiz_start.elapsed();
                if !hiz_written {
                    step_timing.hiz_skipped = step_timing.hiz_skipped.saturating_add(1);
                }
            } else {
                step_timing.hiz_skipped = step_timing.hiz_skipped.saturating_add(1);
            }
            self.frustum_gpu_inputs_valid = true;
        } else {
            step_timing.frustum_skipped = step_timing.frustum_skipped.saturating_add(1);
            step_timing.hiz_skipped = step_timing.hiz_skipped.saturating_add(1);
            step_timing.indirect_skipped = step_timing.indirect_skipped.saturating_add(1);
            step_timing.cull_input_skipped = step_timing.cull_input_skipped.saturating_add(1);
        }
        // Full rebuild re-staged every caster (rigid + multimesh); drop cache.
        self.shadow_casters_dirty = true;
        // Multimesh shadow path needs an identity index buffer covering the full
        // instance set. Only maintained when a multimesh batch casts shadows.
        if self
            .multimesh_batches
            .iter()
            .any(|batch| batch.casts_shadows && !batch.mesh_blend)
        {
            self.ensure_multimesh_shadow_identity(
                device,
                queue,
                self.staged_multimesh_instances.len(),
            );
        }
        self.update_shadow_state(queue, &camera, lighting, self.has_shadow_casters);
        self.last_total_meshlets = total_meshlets;
        self.last_total_drawn =
            self.staged_instance_transforms.len() + self.staged_multimesh_instances.len();
        self.debug_point_instances_scratch = debug_point_instances;
        self.debug_edge_instances_scratch = debug_edge_instances;
        self.last_prepare_step_timing = step_timing;
    }

    fn stage_blend_shape_instance(
        &mut self,
        mesh: &MeshAssetRange,
        weights: &[f32],
    ) -> BlendShapeInstanceMetaGpu {
        let meta = self.stage_blend_shape_weight_range(mesh, weights);
        self.staged_blend_shape_instance_meta.push(meta);
        meta
    }

    fn stage_blend_shape_weight_range(
        &mut self,
        mesh: &MeshAssetRange,
        weights: &[f32],
    ) -> BlendShapeInstanceMetaGpu {
        let target_count = mesh.blend_shape_target_count;
        let weight_count = weights.len().min(target_count as usize) as u32;
        let weight_start = self.staged_blend_shape_weights.len() as u32;
        self.staged_blend_shape_weights.extend(
            weights
                .iter()
                .take(weight_count as usize)
                .map(|weight| weight.clamp(0.0, 1.0)),
        );
        BlendShapeInstanceMetaGpu {
            weight_range: [weight_start, weight_count, 0, 0],
            shape_range: [
                mesh.blend_shape_delta_start,
                target_count,
                mesh.blend_shape_vertex_start,
                mesh.blend_shape_vertex_count,
            ],
        }
    }
}

// Local color bleed (one-bounce GI approximation) limits.
const BLEED_MAX_BATCHES: usize = 512;
const BLEED_MAX_EMITTERS: usize = 256;
const BLEED_MAX_OCCLUDERS: usize = 64;
const BLEED_RANGE: f32 = 14.0;
// Occluder spheres shrink a bit so shared floors don't block everything.
const BLEED_OCCLUDER_SHRINK: f32 = 0.8;
const BLEED_OCCLUDED_FACTOR: f32 = 0.2;

pub(super) struct BleedEmitter {
    batch_index: usize,
    center: Vec3,
    radius_sq: f32,
    color: Vec3,
}

pub(super) struct BleedOccluder {
    batch_index: usize,
    center: Vec3,
    radius: f32,
}

#[inline]
fn unpack_unorm8_lane(packed: u32, shift: u32) -> f32 {
    ((packed >> shift) & 0xff) as f32 / 255.0
}

// Bit layout matches decode_local_bleed in the prelude WGSL:
// r5 g5 b5 strength5 oct_x6 oct_y6.
#[inline]
fn pack_local_bleed(color: Vec3, strength: f32, dir: Vec3) -> u32 {
    #[inline]
    fn quant5(v: f32) -> u32 {
        (v.clamp(0.0, 1.0) * 31.0 + 0.5) as u32
    }
    #[inline]
    fn quant6(v: f32) -> u32 {
        (v.clamp(0.0, 1.0) * 63.0 + 0.5) as u32
    }
    let d = dir.normalize_or_zero();
    let sum = (d.x.abs() + d.y.abs() + d.z.abs()).max(1.0e-6);
    let mut ox = d.x / sum;
    let mut oy = d.y / sum;
    if d.z < 0.0 {
        let old_x = ox;
        ox = (1.0 - oy.abs()) * old_x.signum();
        oy = (1.0 - old_x.abs()) * oy.signum();
    }
    quant5(color.x)
        | (quant5(color.y) << 5)
        | (quant5(color.z) << 10)
        | (quant5(strength) << 15)
        | (quant6(ox * 0.5 + 0.5) << 20)
        | (quant6(oy * 0.5 + 0.5) << 26)
}

// Multimesh emitters index past regular draw batches to stay unique.
const BLEED_MULTIMESH_INDEX_BASE: usize = 1 << 20;

// Shared gather: weighted sum of visible emitters around a receiver center.
// Returns the packed tint or None when nothing contributes.
fn gather_local_bleed(
    center: Vec3,
    self_index: usize,
    emitters: &[BleedEmitter],
    occluders: &[BleedOccluder],
) -> Option<u32> {
    let mut sum = Vec3::ZERO;
    let mut sum_dir = Vec3::ZERO;
    let mut total_w = 0.0f32;
    for emitter in emitters {
        if emitter.batch_index == self_index {
            continue;
        }
        let d_sq = emitter.center.distance_squared(center);
        let range_fade = (1.0 - d_sq / (BLEED_RANGE * BLEED_RANGE)).clamp(0.0, 1.0);
        if range_fade <= 0.0 {
            continue;
        }
        let mut w = (emitter.radius_sq / (d_sq + emitter.radius_sq)) * range_fade;
        if bleed_segment_occluded(
            center,
            emitter.center,
            occluders,
            self_index,
            emitter.batch_index,
        ) {
            w *= BLEED_OCCLUDED_FACTOR;
        }
        sum += emitter.color * w;
        sum_dir += (emitter.center - center).normalize_or_zero() * w;
        total_w += w;
    }
    if total_w <= 1.0e-3 {
        return None;
    }
    let tint = (sum / total_w).clamp(Vec3::ZERO, Vec3::ONE);
    Some(pack_local_bleed(
        tint,
        total_w.clamp(0.0, 1.0),
        sum_dir.normalize_or_zero(),
    ))
}

// Approximate world bounds of a multimesh draw from sampled instances.
pub(super) fn multimesh_world_bounds(
    batch: &MultiMeshBatch,
    draw_params: &[MultiMeshDrawParamGpu],
    instances: &[MultiMeshInstanceGpu],
) -> Option<(Vec3, f32)> {
    let param = draw_params.get(batch.draw_param_index as usize)?;
    let cols = [
        [
            param.model_row_0[0],
            param.model_row_1[0],
            param.model_row_2[0],
            0.0,
        ],
        [
            param.model_row_0[1],
            param.model_row_1[1],
            param.model_row_2[1],
            0.0,
        ],
        [
            param.model_row_0[2],
            param.model_row_1[2],
            param.model_row_2[2],
            0.0,
        ],
        [
            param.model_row_0[3],
            param.model_row_1[3],
            param.model_row_2[3],
            1.0,
        ],
    ];
    let model = Mat4::from_cols_array_2d(&cols);
    if !model.is_finite() {
        return None;
    }
    let start = batch.instance_start as usize;
    let end = (start + batch.instance_count as usize).min(instances.len());
    if end <= start {
        return None;
    }
    let count = end - start;
    let step = (count / 64).max(1);
    let mut sum = Vec3::ZERO;
    let mut samples: Vec<Vec3> = Vec::with_capacity(count.min(64) + 1);
    let mut i = start;
    while i < end {
        let world = (model * Vec3::from(instances[i].position).extend(1.0)).truncate();
        if world.is_finite() {
            sum += world;
            samples.push(world);
        }
        i += step;
    }
    if samples.is_empty() {
        return None;
    }
    let center = sum / samples.len() as f32;
    let radius = samples
        .iter()
        .map(|p| p.distance(center))
        .fold(0.0f32, f32::max)
        + 1.0;
    Some((center, radius.clamp(0.5, 100.0)))
}

// True when a third batch sphere blocks the segment between two centers.
#[inline]
fn bleed_segment_occluded(
    from: Vec3,
    to: Vec3,
    occluders: &[BleedOccluder],
    skip_a: usize,
    skip_b: usize,
) -> bool {
    let seg = to - from;
    let len_sq = seg.length_squared();
    if len_sq <= 1.0e-6 {
        return false;
    }
    for occ in occluders {
        if occ.batch_index == skip_a || occ.batch_index == skip_b {
            continue;
        }
        let t = ((occ.center - from).dot(seg) / len_sq).clamp(0.0, 1.0);
        // Endpoints touching the occluder are contact, not blockage.
        if t <= 0.05 || t >= 0.95 {
            continue;
        }
        let closest = from + seg * t;
        let r = occ.radius * BLEED_OCCLUDER_SHRINK;
        if occ.center.distance_squared(closest) < r * r {
            return true;
        }
    }
    false
}

impl Gpu3D {
    // Approximate one-bounce GI: tint each standard-material batch with the
    // distance-weighted albedo/emissive of nearby batches. The tint rides in
    // packed_pbr_params_1 (free unless mesh blend owns it) and the
    // MATERIAL_FLAG_LOCAL_BLEED bit tells the shader the lane is valid.
    pub(super) fn apply_local_color_bleed(&mut self) {
        if self.draw_batches.len() > BLEED_MAX_BATCHES {
            return;
        }
        let mut emitters = std::mem::take(&mut self.bleed_emitters_scratch);
        emitters.clear();
        let mut occluders = std::mem::take(&mut self.bleed_occluders_scratch);
        occluders.clear();
        for (batch_index, batch) in self.draw_batches.iter().enumerate() {
            if emitters.len() >= BLEED_MAX_EMITTERS {
                break;
            }
            // Multi-instance batches stay out of the bleed emitter/occluder set:
            // their first-instance transform does not stand in for the whole
            // batch, and pre-merge behavior excluded them via the 1e9 sentinel.
            if batch.draw_on_top || batch.instance_count != 1 || batch.local_radius >= 1.0e8 {
                continue;
            }
            let Some(inst) = self
                .staged_instance_transforms
                .get(batch.instance_start as usize)
            else {
                continue;
            };
            let model = Mat4::from_cols_array_2d(&model_cols_from_affine_rows(inst));
            if !model.is_finite() {
                continue;
            }
            let center = (model * Vec3::from(batch.local_center).extend(1.0)).truncate();
            if !center.is_finite() {
                continue;
            }
            let sx = model.x_axis.truncate().length();
            let sy = model.y_axis.truncate().length();
            let sz = model.z_axis.truncate().length();
            let radius = (batch.local_radius.max(0.0) * sx.max(sy).max(sz)).clamp(0.05, 50.0);
            if occluders.len() < BLEED_MAX_OCCLUDERS && radius >= 0.75 && batch.alpha_mode != 2 {
                occluders.push(BleedOccluder {
                    batch_index,
                    center,
                    radius,
                });
            }
            let Some(meta) = self
                .staged_rigid_instance_meta
                .get(batch.instance_start as usize)
            else {
                continue;
            };
            let packed = meta.material.packed_color;
            let albedo = Vec3::new(
                unpack_unorm8_lane(packed, 0),
                unpack_unorm8_lane(packed, 8),
                unpack_unorm8_lane(packed, 16),
            );
            let em = meta.material.packed_emissive;
            let em_scale = unpack_unorm8_lane(em, 24) * 16.0;
            let emissive = Vec3::new(
                unpack_unorm8_lane(em, 0),
                unpack_unorm8_lane(em, 8),
                unpack_unorm8_lane(em, 16),
            ) * em_scale;
            let color = albedo * 0.8 + emissive;
            if color.max_element() <= 1.0e-3 {
                continue;
            }
            emitters.push(BleedEmitter {
                batch_index,
                center,
                radius_sq: radius * radius,
                color,
            });
        }
        // Multimesh draws join as emitters too (grass fields tint neighbors).
        let mut multimesh_bounds = std::mem::take(&mut self.bleed_multimesh_bounds_scratch);
        multimesh_bounds.clear();
        multimesh_bounds.reserve(self.multimesh_batches.len());
        for (mm_index, batch) in self.multimesh_batches.iter().enumerate() {
            let bounds = multimesh_world_bounds(
                batch,
                &self.staged_multimesh_draw_params,
                &self.staged_multimesh_instances,
            );
            if let Some((center, radius)) = bounds
                && emitters.len() < BLEED_MAX_EMITTERS
                && let Some(param) = self
                    .staged_multimesh_draw_params
                    .get(batch.draw_param_index as usize)
            {
                let albedo = Vec3::new(
                    unpack_unorm8_lane(param.packed_color, 0),
                    unpack_unorm8_lane(param.packed_color, 8),
                    unpack_unorm8_lane(param.packed_color, 16),
                );
                let em_scale = unpack_unorm8_lane(param.packed_emissive, 24) * 16.0;
                let emissive = Vec3::new(
                    unpack_unorm8_lane(param.packed_emissive, 0),
                    unpack_unorm8_lane(param.packed_emissive, 8),
                    unpack_unorm8_lane(param.packed_emissive, 16),
                ) * em_scale;
                let color = albedo * 0.8 + emissive;
                if color.max_element() > 1.0e-3 {
                    emitters.push(BleedEmitter {
                        batch_index: BLEED_MULTIMESH_INDEX_BASE + mm_index,
                        center,
                        radius_sq: radius * radius,
                        color,
                    });
                }
            }
            multimesh_bounds.push(bounds);
        }
        if emitters.is_empty() {
            self.bleed_emitters_scratch = emitters;
            self.bleed_occluders_scratch = occluders;
            self.bleed_multimesh_bounds_scratch = multimesh_bounds;
            return;
        }
        for batch_index in 0..self.draw_batches.len() {
            let (instance_start, instance_count, local_center) = {
                let batch = &self.draw_batches[batch_index];
                if batch.draw_on_top
                    || batch.mesh_blend
                    || batch.instance_count == 0
                    || matches!(batch.material_kind, MaterialPipelineKind::Unlit)
                {
                    continue;
                }
                (
                    batch.instance_start as usize,
                    batch.instance_count as usize,
                    batch.local_center,
                )
            };
            let Some(inst) = self.staged_instance_transforms.get(instance_start) else {
                continue;
            };
            let model = Mat4::from_cols_array_2d(&model_cols_from_affine_rows(inst));
            if !model.is_finite() {
                continue;
            }
            let center = (model * Vec3::from(local_center).extend(1.0)).truncate();
            if !center.is_finite() {
                continue;
            }
            let Some(packed) = gather_local_bleed(center, batch_index, &emitters, &occluders)
            else {
                continue;
            };
            let end = instance_start + instance_count;
            for meta in self
                .staged_rigid_instance_meta
                .get_mut(instance_start..end)
                .unwrap_or(&mut [])
            {
                meta.material.packed_pbr_params_1 = packed;
                meta.material.packed_material_params |= MATERIAL_FLAG_LOCAL_BLEED << 3;
            }
            for meta in self
                .staged_skinned_instance_meta
                .get_mut(instance_start..end)
                .unwrap_or(&mut [])
            {
                meta.material.packed_pbr_params_1 = packed;
                meta.material.packed_material_params |= MATERIAL_FLAG_LOCAL_BLEED << 3;
            }
        }
        // Multimesh receivers: one tint per draw param, read in the vertex
        // stage since instances share the draw's material.
        for (mm_index, bounds) in multimesh_bounds.iter().enumerate() {
            let Some((center, _)) = *bounds else {
                continue;
            };
            let (draw_param_index, unlit) = {
                let batch = &self.multimesh_batches[mm_index];
                (
                    batch.draw_param_index as usize,
                    matches!(batch.material_kind, MaterialPipelineKind::Unlit),
                )
            };
            if unlit {
                continue;
            }
            let self_index = BLEED_MULTIMESH_INDEX_BASE + mm_index;
            let Some(packed) = gather_local_bleed(center, self_index, &emitters, &occluders) else {
                continue;
            };
            if let Some(param) = self.staged_multimesh_draw_params.get_mut(draw_param_index) {
                param.packed_bleed = packed;
            }
        }
        self.bleed_emitters_scratch = emitters;
        self.bleed_occluders_scratch = occluders;
        self.bleed_multimesh_bounds_scratch = multimesh_bounds;
    }
}

#[cfg(test)]
mod tests {
    use super::builtin_flat_mesh_double_sided;
    use super::{BleedOccluder, Vec3, bleed_segment_occluded, pack_local_bleed};

    #[test]
    fn local_bleed_pack_lanes_match_shader_layout() {
        let packed = pack_local_bleed(Vec3::new(1.0, 0.5, 0.0), 0.5, Vec3::Y);
        assert_eq!(packed & 0x1f, 31, "r lane");
        assert_eq!((packed >> 5) & 0x1f, 16, "g lane");
        assert_eq!((packed >> 10) & 0x1f, 0, "b lane");
        assert_eq!((packed >> 15) & 0x1f, 16, "strength lane");
        // +Y maps to octahedral (0, 1) -> quantized (32, 63).
        assert_eq!((packed >> 20) & 0x3f, 32, "oct x");
        assert_eq!((packed >> 26) & 0x3f, 63, "oct y");
    }

    #[test]
    fn bleed_occlusion_blocks_midpoint_sphere_only() {
        let occ = [BleedOccluder {
            batch_index: 7,
            center: Vec3::new(0.0, 0.0, 5.0),
            radius: 1.5,
        }];
        let a = Vec3::ZERO;
        let b = Vec3::new(0.0, 0.0, 10.0);
        assert!(bleed_segment_occluded(a, b, &occ, 0, 1));
        // Skipped when the occluder is one of the endpoints' batches.
        assert!(!bleed_segment_occluded(a, b, &occ, 7, 1));
        // Off-axis sphere does not block.
        let off = [BleedOccluder {
            batch_index: 7,
            center: Vec3::new(4.0, 0.0, 5.0),
            radius: 1.5,
        }];
        assert!(!bleed_segment_occluded(a, b, &off, 0, 1));
    }

    #[test]
    fn flat_builtin_meshes_default_double_sided() {
        assert!(builtin_flat_mesh_double_sided(
            perro_builtin_meshes::PLANE_SOURCE
        ));
        assert!(builtin_flat_mesh_double_sided(
            perro_builtin_meshes::QUAD_SOURCE
        ));
        assert!(!builtin_flat_mesh_double_sided(
            perro_builtin_meshes::CUBE_SOURCE
        ));
    }
}
