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

fn vertex_modifier_bound(modifiers: &[VertexModifier3D], mesh_radius: f32) -> f32 {
    let radius = mesh_radius.max(0.0);
    modifiers.iter().fold(0.0, |bound, modifier| {
        bound
            + match *modifier {
                VertexModifier3D::Wind { strength, .. } => strength.abs(),
                VertexModifier3D::Wave { amplitude, .. } => amplitude.abs(),
                VertexModifier3D::Bend { angle_radians, .. }
                | VertexModifier3D::Twist { angle_radians, .. } => {
                    2.0 * radius * (angle_radians.abs() * 0.5).sin().abs().min(1.0)
                }
                VertexModifier3D::Inflate { amount, .. } => amount.abs(),
                VertexModifier3D::Jitter { amount, .. } => amount.abs() * 1.732_050_8,
                VertexModifier3D::PixelSnap { .. } => 1.0e9,
            }
    })
}

fn builtin_flat_mesh_double_sided(source: &str) -> bool {
    matches!(
        source,
        perro_builtin_meshes::PLANE_SOURCE | perro_builtin_meshes::QUAD_SOURCE
    )
}

#[inline]
fn prepare_fast_path_eligible(force_full_rebuild: bool, eligible: bool) -> bool {
    !force_full_rebuild && eligible
}

/// Field-wise adoption of `src` into `dst` that only touches the `Arc` lanes
/// whose allocation actually changed.
///
/// The retained producer hands out the same `Arc` when a lane's content is
/// unchanged, so a moving-but-not-animated draw differs in `instance_mats`
/// alone and an animated character in `skeleton` alone. A blanket
/// `dst.clone_from(src)` still pays inc+dec on all five shared lanes of every
/// draw in the scene; the pointer probes below cut that to the lanes that
/// moved (typically one) and never reallocate the destination.
#[inline]
fn adopt_draw_in_place(dst: &mut Draw3DInstance, src: &Draw3DInstance) {
    // Copy lanes: unconditional, no refcount traffic.
    dst.node = src.node;
    dst.kind.clone_from(&src.kind);
    dst.debug_color = src.debug_color;
    dst.meshlet_override = src.meshlet_override;
    dst.lod = src.lod;
    dst.blend = src.blend;
    dst.cast_shadows = src.cast_shadows;
    dst.receive_shadows = src.receive_shadows;
    if !Arc::ptr_eq(&dst.surfaces, &src.surfaces) {
        dst.surfaces = src.surfaces.clone();
    }
    if !Arc::ptr_eq(&dst.instance_mats, &src.instance_mats) {
        dst.instance_mats = src.instance_mats.clone();
    }
    if !Arc::ptr_eq(&dst.blend_shape_weights, &src.blend_shape_weights) {
        dst.blend_shape_weights = src.blend_shape_weights.clone();
    }
    match (dst.skeleton.as_mut(), src.skeleton.as_ref()) {
        (Some(dst_skeleton), Some(src_skeleton))
            if Arc::ptr_eq(&dst_skeleton.matrices, &src_skeleton.matrices) => {}
        (_, src_skeleton) => dst.skeleton = src_skeleton.cloned(),
    }
    match (dst.dense_multimesh.as_mut(), src.dense_multimesh.as_ref()) {
        (Some(dst_dense), Some(src_dense))
            if Arc::ptr_eq(&dst_dense.instances, &src_dense.instances) =>
        {
            dst_dense.node_model = src_dense.node_model;
            dst_dense.instance_scale = src_dense.instance_scale;
        }
        (_, src_dense) => dst.dense_multimesh = src_dense.cloned(),
    }
}

impl Gpu3D {
    /// Upload accounting for the last `prepare` (see [`FrameUploadStats`]).
    pub fn prepare_upload_stats(&self) -> FrameUploadStats {
        self.last_prepare_upload_stats
    }

    #[inline]
    pub(super) fn note_upload(&mut self, bytes: usize) {
        self.note_uploads(1, bytes);
    }

    #[inline]
    pub(super) fn note_uploads(&mut self, calls: u32, bytes: usize) {
        self.last_prepare_upload_stats.write_buffer_calls = self
            .last_prepare_upload_stats
            .write_buffer_calls
            .saturating_add(calls);
        self.last_prepare_upload_stats.write_buffer_bytes = self
            .last_prepare_upload_stats
            .write_buffer_bytes
            .saturating_add(bytes as u64);
    }

    /// Restage the per-batch indirect draw records and upload them.
    ///
    /// Deliberately NOT skip-gated on staged-byte equality, unlike every other
    /// staging upload here: `frustum_cull.wgsl` / `hiz_occlusion_cull.wgsl`
    /// bind this buffer `read_write` and zero `instance_count` for culled
    /// batches, then rebuild a survivor's count as `max(current, 1)`. The GPU
    /// copy therefore diverges from the staging vec after every cull dispatch,
    /// and a multi-instance batch that comes back into frustum would resurrect
    /// with one instance instead of its real count. Gating this needs the
    /// shader to source the count from the read-only static record (there are
    /// three spare `cull_flags` words) rather than from the command itself.
    pub(super) fn rebuild_and_upload_indirect(&mut self, queue: &wgpu::Queue) {
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
        let bytes = std::mem::size_of_val(self.indirect_staging.as_slice());
        queue.write_buffer(
            &self.indirect_buffer,
            0,
            bytemuck::cast_slice(&self.indirect_staging),
        );
        self.note_upload(bytes);
    }

    /// Refresh `last_draws` from this frame's draw list without rebuilding the
    /// vector. See [`adopt_draw_in_place`]: same observable state, but the
    /// per-frame refcount churn drops from every shared lane of every draw to
    /// only the lanes that changed.
    fn sync_last_draws(&mut self, draws: &[Draw3DInstance]) {
        if self.last_draws.len() != draws.len() {
            self.last_draws.clear();
            self.last_draws.extend_from_slice(draws);
            return;
        }
        for (dst, src) in self.last_draws.iter_mut().zip(draws.iter()) {
            adopt_draw_in_place(dst, src);
        }
    }

    /// Writes the per-frame shader globals (time / resolution) into the scene
    /// uniform tail. Runs inside `prepare`, and standalone on frames where
    /// prepare is skipped so `perro_time()`-driven shaders keep animating.
    pub(crate) fn patch_scene_globals(
        &self,
        queue: &wgpu::Queue,
        lighting: &Lighting3DState,
        width: u32,
        height: u32,
    ) {
        let scene_globals = SceneGlobalsGpu {
            time_params: [
                lighting.frame_time_seconds,
                lighting.frame_delta_seconds,
                lighting.frame_index as f32,
                (lighting.frame_time_seconds / 60.0).fract(),
            ],
            resolution: [
                width.max(1) as f32,
                height.max(1) as f32,
                1.0 / width.max(1) as f32,
                1.0 / height.max(1) as f32,
            ],
        };
        queue.write_buffer(
            &self.camera_buffer,
            SCENE_GLOBALS_OFFSET,
            bytemuck::bytes_of(&scene_globals),
        );
    }

    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: Prepare3D<'_>) {
        let mut step_timing = Prepare3DStepTiming::default();
        self.last_prepare_upload_stats = FrameUploadStats::default();
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
            shared_textures,
            camera,
            lighting,
            draws,
            draws_revision,
            force_full_rebuild,
            decals,
            decals_revision,
            width,
            height,
            static_texture_lookup,
            static_mesh_lookup,
            static_shader_lookup,
        } = frame;
        self.custom_mesh_ranges
            .retain(|mesh_id, _| resources.has_mesh(*mesh_id));
        let compacted_mesh_storage = self.compact_custom_mesh_storage_if_needed(device);
        let force_full_rebuild = force_full_rebuild || compacted_mesh_storage;
        self.resize(device, width, height);
        self.ensure_material_fallback_texture(device, queue, shared_textures);
        self.ensure_environment_map(
            device,
            queue,
            lighting
                .sky
                .as_ref()
                .and_then(|sky| sky.environment.as_ref()),
            resources,
            static_texture_lookup,
        );
        self.prepare_decals(device, queue, resources, decals, decals_revision);
        self.frustum_cull_enabled = self.frustum_cull_supported;
        let (gpu_occlusion_enabled, cpu_occlusion_enabled) = occlusion_flags(self.occlusion_mode);
        self.gpu_occlusion_enabled = gpu_occlusion_enabled && self.frustum_cull_enabled;
        self.cpu_occlusion_enabled = cpu_occlusion_enabled;

        // Compute view_proj + its inverse once; scene/sky uniforms and frustum
        // extraction below all reuse them instead of recomputing per consumer.
        let view_proj = compute_view_proj_mat(&camera, width, height);
        let inv_view_proj = view_proj.inverse();
        // Everything below (uniform, sky, frustum extraction, HiZ, shadows,
        // occlusion) consumes the UNJITTERED matrices; when TAA runs, the
        // per-frame `apply_taa_jitter` call re-patches only the two matrix
        // slots of the GPU camera uniform with the sub-pixel jitter.
        self.taa_base_view_proj = view_proj;
        self.taa_base_inv_view_proj = if inv_view_proj.is_finite() {
            inv_view_proj
        } else {
            Mat4::IDENTITY
        };
        let mut uniform = build_scene_uniform(&camera, lighting, view_proj, inv_view_proj);
        if !self.ibl_maps.active() {
            uniform.ibl_params[0] = 0.0;
        }
        let sky_uniform = build_sky_uniform(&camera, lighting, inv_view_proj);
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
        let draws_unchanged = prepare_fast_path_eligible(
            force_full_rebuild,
            draws_semantically_unchanged(
                self.last_draws_revision,
                draws_revision,
                &self.last_draws,
                draws,
            ),
        );
        // Classify each draw pair: single-instance regular draws (model-only)
        // and dense multimeshes whose poses are unchanged (node_model-only) both
        // stay on the transform-only fast path. A multimesh present but unchanged
        // no longer forces a full rebuild.
        let mut transform_only_kinds = std::mem::take(&mut self.transform_only_kinds_scratch);
        let transform_only_semantic = prepare_fast_path_eligible(
            force_full_rebuild,
            !draws_unchanged
                && classify_transform_only_scene(
                    &self.last_draws,
                    draws,
                    &mut transform_only_kinds,
                ),
        );
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
        // Frame globals (time / resolution) live in the uniform tail and are
        // patched every frame so they never defeat the change gate above.
        self.patch_scene_globals(queue, lighting, width, height);
        if self.cpu_occlusion_enabled && scene_changed {
            // Retained-mode correctness: when camera/transforms/resources update,
            // previous query visibility is stale and must not gate current frame.
            self.occlusion_state.clear();
        }
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
                    self.rebuild_and_upload_indirect(queue);
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
            self.update_shadow_state(device, queue, &camera, lighting, self.has_shadow_casters);
            self.last_draws_revision = draws_revision;
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
            let mut patched_bytes = 0usize;
            for span in self.merged_instance_spans_scratch.iter() {
                let byte_start =
                    span.start as u64 * std::mem::size_of::<TransformInstanceGpu>() as u64;
                let slice = &self.staged_instance_transforms[span.start as usize..span.end as usize];
                patched_bytes += std::mem::size_of_val(slice);
                queue.write_buffer(
                    &self.instance_transform_buffer,
                    byte_start,
                    bytemuck::cast_slice(slice),
                );
            }
            if patched_bytes > 0 {
                let calls = self.merged_instance_spans_scratch.len() as u32;
                self.note_uploads(calls, patched_bytes);
                // Partial spans keep the GPU in sync with the staging vec, but
                // the whole-vec hash the full-rebuild gate compares against no
                // longer describes what was sent. Drop it rather than re-hash
                // the whole (potentially multi-MB) vec on the fast path; the
                // next full rebuild pays one upload to re-prime it.
                self.last_uploaded_instance_transforms_hash = None;
            }
            // The multimesh block below reuses the merged scratch; keep the
            // transform spans for the mesh-blend sphere refresh.
            self.transform_dirty_spans_snapshot.clear();
            self.transform_dirty_spans_snapshot
                .extend_from_slice(&self.merged_instance_spans_scratch);
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
                let mut patched_bytes = 0usize;
                for span in self.merged_instance_spans_scratch.iter() {
                    let byte_start =
                        span.start as u64 * std::mem::size_of::<MultiMeshDrawParamGpu>() as u64;
                    let slice = &self.staged_multimesh_draw_params
                        [span.start as usize..span.end as usize];
                    patched_bytes += std::mem::size_of_val(slice);
                    queue.write_buffer(
                        &self.multimesh_draw_params_buffer,
                        byte_start,
                        bytemuck::cast_slice(slice),
                    );
                }
                let calls = self.merged_instance_spans_scratch.len() as u32;
                self.note_uploads(calls, patched_bytes);
                // GPU buffer now equals the staged vec again; keep the
                // skip-identical hash in sync so the next full restage can
                // still gate its upload.
                self.last_uploaded_multimesh_draw_params_hash =
                    Some(super::pod_slice_len_hash(&self.staged_multimesh_draw_params));
            }
            // Transforms shifted: overlap tests may change, so refresh receiver
            // lists unless no blend-relevant batch actually moved this frame.
            let dirty_spans = std::mem::take(&mut self.transform_dirty_spans_snapshot);
            self.rebuild_mesh_blend_receivers_gated(Some(dirty_spans.as_slice()));
            self.transform_dirty_spans_snapshot = dirty_spans;
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
                    self.rebuild_and_upload_indirect(queue);
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
                        let mut patched_calls = 0u32;
                        let mut patched_bytes = 0usize;
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
                            let dynamic_slice = &self.frustum_cull_dynamic_staging
                                [batch_span.start..batch_span.end];
                            patched_calls += 1;
                            patched_bytes += std::mem::size_of_val(dynamic_slice);
                            queue.write_buffer(
                                &self.frustum_cull_dynamic_buffer,
                                byte_start,
                                bytemuck::cast_slice(dynamic_slice),
                            );
                            if static_dirty {
                                let byte_start = (batch_span.start
                                    * std::mem::size_of::<FrustumCullStaticGpu>())
                                    as u64;
                                let static_slice = &self.frustum_cull_static_staging
                                    [batch_span.start..batch_span.end];
                                patched_calls += 1;
                                patched_bytes += std::mem::size_of_val(static_slice);
                                queue.write_buffer(
                                    &self.frustum_cull_static_buffer,
                                    byte_start,
                                    bytemuck::cast_slice(static_slice),
                                );
                            }
                        }
                        // Span writes keep the GPU equal to the staging vecs but
                        // invalidate the whole-vec hashes the full-rebuild gate
                        // compares against (see the instance-transform note).
                        if patched_calls > 0 {
                            self.note_uploads(patched_calls, patched_bytes);
                            self.last_uploaded_frustum_cull_dynamic_hash = None;
                            self.last_uploaded_frustum_cull_static_hash = None;
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
            // The multimesh staging itself is untouched by a transform patch
            // (only the draw params' model rows moved, in place), so the reuse
            // snapshots stay valid -- refresh them with the draws that are now
            // reflected in the staged rows, keeping pose-Arc identity fresh.
            self.sync_multimesh_staging_cache_transforms(draws);
            // Transform patch moved rigid + multimesh casters; drop the cache.
            self.shadow_casters_dirty = true;
            self.update_shadow_state(device, queue, &camera, lighting, self.has_shadow_casters);
            self.sync_last_draws(draws);
            self.last_draws_revision = draws_revision;
            self.last_total_drawn =
                self.staged_instance_transforms.len() + self.staged_multimesh_instances.len();
            self.last_prepare_step_timing = step_timing;
            return;
        }

        self.frustum_gpu_inputs_valid = false;
        self.sync_last_draws(draws);
        self.last_draws_revision = draws_revision;

        // Scene-resolved mesh blend is an input to the multimesh staging (it is
        // baked into the draw params and batches), so it has to be resolved
        // before the reuse decision below -- and before the staging vectors are
        // cleared, since reuse keeps them.
        let mut mesh_blends = std::mem::take(&mut self.mesh_blend_scratch);
        resolve_mesh_blends(draws, &mut mesh_blends);
        // Screen-space seam pass handles single-mesh and multimesh sources.
        // MSAA resolves into the single-sample scene texture before the seam.
        if self.screen_blend_supported {
            for blend in mesh_blends.iter_mut() {
                promote_mesh_blend_screen_pass(blend);
            }
        }
        // A full rebuild triggered by an unrelated draw still repacks, hashes
        // and re-uploads every multimesh instance. When the dense draw sequence
        // is unchanged that work reproduces the exact same bytes, so skip it
        // whole: the staged multimesh vectors (instances, draw params, batches
        // and the blend-shape tails) are left standing and only the per-draw
        // range bookkeeping is replayed.
        let multimesh_reuse = self.can_reuse_multimesh_staging(
            force_full_rebuild,
            draws,
            &mesh_blends,
            camera.position,
        );
        if multimesh_reuse {
            self.multimesh_staging_reuse_count =
                self.multimesh_staging_reuse_count.saturating_add(1);
        }

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
        if !multimesh_reuse {
            self.multimesh_batches.clear();
            self.staged_multimesh_instances.clear();
            self.staged_multimesh_instances
                .reserve(multimesh_instance_hint);
            self.staged_multimesh_draw_params.clear();
            self.staged_multimesh_blend_meta.clear();
            self.staged_multimesh_blend_weights.clear();
            self.multimesh_blend_meta_base = 0;
            self.multimesh_blend_weight_base = 0;
            self.multimesh_blend_tail_revision = self.multimesh_blend_tail_revision.wrapping_add(1);
            self.staged_multimesh_custom_params_meta.clear();
            self.staged_multimesh_custom_params_values.clear();
            self.staged_multimesh_custom_params_dedupe.clear();
            self.staged_multimesh_custom_params_entry_starts.clear();
            self.multimesh_custom_params_meta_base = 0;
            self.multimesh_custom_params_values_base = 0;
            self.multimesh_custom_params_tail_revision =
                self.multimesh_custom_params_tail_revision.wrapping_add(1);
            self.multimesh_staging_cache.clear();
            self.multimesh_staging_cache_valid = false;
        }
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
        let Some(default_mesh) = self.resolve_builtin_mesh_asset("__cube__") else {
            return;
        };
        // Hoisted once per prepare: the per-draw arms below hand out
        // references instead of rebuilding these builtin assets per draw.
        let quad_mesh = self.resolve_builtin_mesh_asset("__quad__");
        let cylinder_mesh = self.resolve_builtin_mesh_asset("__cylinder__");
        // Cache moved out of self so resolve_mesh_range can return `&` into it
        // while the loop keeps calling `&mut self` methods; restored below.
        let mut custom_mesh_ranges = std::mem::take(&mut self.custom_mesh_ranges);
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

        let mut reused_dense_index = 0usize;
        for (draw_index, draw) in draws.iter().enumerate() {
            let resolved_blend = mesh_blends[draw_index];
            let draw_instance_start = self.staged_instance_transforms.len() as u32;
            let draw_span_start = self.last_draw_instance_spans.len();
            let draw_multimesh_param_start = self.staged_multimesh_draw_params.len() as u32;
            // Reused multimesh: the staged rows for this draw are already in
            // place from the previous build, so only the per-draw range
            // bookkeeping (and the pose-cache retention mark) is replayed.
            if multimesh_reuse && let Some(dense) = draw.dense_multimesh.as_ref() {
                let snapshot = &mut self.multimesh_staging_cache[reused_dense_index];
                let param_range = snapshot.param_range.clone();
                // Adopt this frame's draw so the next validity check keeps
                // hitting the pose-Arc pointer fast path even if the producer
                // handed out an equal-but-new Arc (the deep compare that just
                // accepted it is the slow path).
                adopt_draw_in_place(&mut snapshot.draw, draw);
                reused_dense_index += 1;
                self.last_draw_instance_spans
                    .push(draw_instance_start..draw_instance_start);
                let draw_span_end = self.last_draw_instance_spans.len();
                self.last_draw_instance_span_ranges
                    .push(draw_span_start..draw_span_end);
                self.last_draw_multimesh_param_ranges.push(param_range);
                self.multimesh_pose_pack_cache_seen
                    .insert(Arc::as_ptr(&dense.instances) as *const () as usize);
                continue;
            }
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
            let mesh_asset: &MeshAssetRange = match draw.kind {
                Draw3DKind::Mesh(mesh_id) => self
                    .resolve_mesh_range(
                        &mut custom_mesh_ranges,
                        device,
                        queue,
                        resources,
                        mesh_id,
                        mesh_source,
                        static_mesh_lookup,
                    )
                    .unwrap_or(&default_mesh),
                Draw3DKind::CameraStreamQuad { .. } => quad_mesh.as_ref().unwrap_or(&default_mesh),
                Draw3DKind::DebugPointCube => &default_mesh,
                Draw3DKind::DebugEdgeCylinder => {
                    cylinder_mesh.as_ref().unwrap_or(&default_mesh)
                }
            };
            let lod_model = draw
                .instance_mats
                .first()
                .or_else(|| draw.dense_multimesh.as_ref().map(|dense| &dense.node_model));
            let active_lod = select_mesh_lod(mesh_asset, lod_model, camera.position, draw.lod);
            surface_entries.clear();
            match draw.kind {
                Draw3DKind::DebugPointCube => {
                    let color = draw.debug_color.unwrap_or([1.0, 0.92, 0.2, 1.0]);
                    surface_entries.push(SurfaceEntry3D {
                        range: active_lod.full,
                        packed_range: None,
                        packed_lod_param_id: 0,
                        material: Arc::new(Material3D::Standard(StandardMaterial3D {
                            base_color_factor: color,
                            roughness_factor: 0.35,
                            metallic_factor: 0.0,
                            emissive_factor: [
                                color[0] * color[3] * 0.55,
                                color[1] * color[3] * 0.55,
                                color[2] * color[3] * 0.55,
                            ],
                            ..StandardMaterial3D::default()
                        })),
                        modulate_bias: false,
                    });
                }
                Draw3DKind::DebugEdgeCylinder => {
                    let color = draw.debug_color.unwrap_or([0.15, 0.95, 0.95, 1.0]);
                    surface_entries.push(SurfaceEntry3D {
                        range: active_lod.full,
                        packed_range: None,
                        packed_lod_param_id: 0,
                        material: Arc::new(Material3D::Standard(StandardMaterial3D {
                            base_color_factor: color,
                            roughness_factor: 0.6,
                            metallic_factor: 0.0,
                            emissive_factor: [
                                color[0] * color[3] * 0.4,
                                color[1] * color[3] * 0.4,
                                color[2] * color[3] * 0.4,
                            ],
                            ..StandardMaterial3D::default()
                        })),
                        modulate_bias: false,
                    });
                }
                Draw3DKind::CameraStreamQuad { texture, tint } => {
                    surface_entries.push(SurfaceEntry3D {
                        range: active_lod.full,
                        packed_range: None,
                        packed_lod_param_id: 0,
                        material: Arc::new(Material3D::Unlit(
                            perro_render_bridge::UnlitMaterial3D {
                                base_color_factor: tint,
                                alpha_mode: 2,
                                double_sided: true,
                                base_color_texture: texture.index(),
                                ..perro_render_bridge::UnlitMaterial3D::default()
                            },
                        )),
                        modulate_bias: false,
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
                            .unwrap_or_else(default_material3d_arc);
                        let (material, modulate_bias) =
                            apply_surface_binding(base_material, surface);
                        surface_entries.push(SurfaceEntry3D {
                            range,
                            packed_range: packed.map(|(range, _)| range),
                            packed_lod_param_id: packed.map(|(_, param)| param).unwrap_or(0),
                            material,
                            modulate_bias,
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
                            material: default_material3d_arc(),
                            modulate_bias: false,
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
                let lod_sensitive = mesh_asset.lods.len() > 1;
                for entry in surface_entries.iter() {
                    let material: &Material3D = &entry.material;
                    self.ensure_standard_material_texture_slots(
                        device,
                        queue,
                        shared_textures,
                        resources,
                        material.texture_slots(),
                        mesh_source,
                        static_texture_lookup,
                    );
                    let material_texture_key = self.custom_material_image_key(
                        device,
                        queue,
                        shared_textures,
                        resources,
                        material,
                        static_texture_lookup,
                    );
                    self.ensure_material_texture_bind_group(device, material_texture_key);
                    let material_kind = self.material_pipeline_kind(
                        device,
                        RenderPath3D::MultiMesh,
                        material,
                        draw.receive_shadows,
                        static_shader_lookup,
                    );
                    // Tail-local header offset; rebased onto the regular arena
                    // lengths by `rebase_multimesh_custom_params_tail` once the
                    // build is done, so this row never depends on what the
                    // regular draws staged.
                    let custom_params = self.stage_multimesh_custom_params(material);
                    let mirrored_winding = draw_model.determinant() < 0.0;
                    let packed_material = build_instance(
                        dense.node_model,
                        material,
                        BuildInstanceArgs {
                            debug_view: false,
                            debug_color: [1.0; 4],
                            mesh_blend: resolved_blend,
                            skeleton_start: 0,
                            skeleton_count: 0,
                            custom_params_offset: custom_params.0,
                            custom_params_len: custom_params.1,
                            packed_lod_param_id: 0,
                            receive_shadows: draw.receive_shadows,
                            modulate_bias: entry.modulate_bias,
                        },
                    )
                    .rigid_meta
                    .material;
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
                            custom_params: [custom_params.0, custom_params.1],
                            packed_color: packed_material.packed_color,
                            packed_pbr_params_0: packed_material.packed_pbr_params_0,
                            packed_emissive: packed_material.packed_emissive,
                            packed_material_params: packed_material.packed_material_params,
                            scale_bits: dense.instance_scale.max(0.0001).to_bits(),
                            packed_blend_params: resolved_blend.packed_params,
                            packed_bleed: 0,
                            _pad: [0; 3],
                        });
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
                        // Tail-local id / weight start; both are rebased onto
                        // the rigid staging lengths once the loop is done.
                        let blend_meta_id = self.staged_multimesh_blend_meta.len() as u32;
                        self.stage_multimesh_blend_shape_instance(mesh_asset, weights);
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
                            mesh_local_radius: mesh_asset.bounds_radius.max(0.0)
                                + vertex_modifier_bound(
                                    material.vertex_modifiers(),
                                    mesh_asset.bounds_radius,
                                ),
                            double_sided: material.double_sided()
                                || mirrored_winding
                                || flat_builtin_double_sided,
                            mesh_blend: resolved_mesh_blend_active(resolved_blend)
                                && !resolved_mesh_blend_screen_pass(resolved_blend),
                            mesh_blend_screen: resolved_mesh_blend_screen_pass(resolved_blend),
                            mesh_blend_params: resolved_blend.packed_params,
                            mesh_blend_params_ext: resolved_blend.packed_params_ext,
                            mesh_blend_depth: resolved_mesh_blend_depth_receiver(resolved_blend),
                            blend_layers: draw.blend.blend_layers.bits(),
                            blend_mask: draw.blend.blend_mask.bits(),
                            casts_shadows: draw.cast_shadows,
                            material_kind,
                            material_texture_key,
                        });
                    }
                }
                self.last_draw_instance_spans
                    .push(draw_instance_start..(self.staged_instance_transforms.len() as u32));
                let draw_span_end = self.last_draw_instance_spans.len();
                self.last_draw_instance_span_ranges
                    .push(draw_span_start..draw_span_end);
                let param_range =
                    draw_multimesh_param_start..(self.staged_multimesh_draw_params.len() as u32);
                self.last_draw_multimesh_param_ranges
                    .push(param_range.clone());
                self.multimesh_staging_cache.push(MultiMeshDrawSnapshot {
                    draw: draw.clone(),
                    node_model: dense.node_model,
                    resolved_blend,
                    lod_sensitive,
                    param_range,
                });
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
                    .map(|entry| matches!(*entry.material, Material3D::Standard(_)))
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
                    // Palette matrices are already packed as 3 affine rows.
                    self.staged_skeletons.extend_from_slice(&skeleton.matrices);
                    (start, count)
                } else {
                    (0, 0)
                };
                let instance_mats = draw.instance_mats.as_ref();
                if is_debug_point {
                    let material: &Material3D = &surface_entries[0].material;
                    let (custom_params_offset, custom_params_len) =
                        self.stage_custom_params(material);
                    self.ensure_material_texture_slot(
                        device,
                        queue,
                        shared_textures,
                        resources,
                        material.base_color_texture_slot(),
                        mesh_source,
                        static_texture_lookup,
                    );
                    if debug_point_instances.is_empty() {
                        debug_points_double_sided =
                            material.double_sided() || self.meshlet_debug_view;
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
                                modulate_bias: false,
                            },
                        ));
                        debug_points_count = debug_points_count.saturating_add(1);
                    }
                } else if is_debug_edge {
                    let material: &Material3D = &surface_entries[0].material;
                    let (custom_params_offset, custom_params_len) =
                        self.stage_custom_params(material);
                    self.ensure_material_texture_slot(
                        device,
                        queue,
                        shared_textures,
                        resources,
                        material.base_color_texture_slot(),
                        mesh_source,
                        static_texture_lookup,
                    );
                    if debug_edge_instances.is_empty() {
                        debug_edges_double_sided =
                            material.double_sided() || self.meshlet_debug_view;
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
                                modulate_bias: false,
                            },
                        ));
                        debug_edges_count = debug_edges_count.saturating_add(1);
                    }
                } else {
                    for entry in surface_entries.iter() {
                        let material: &Material3D = &entry.material;
                        // Per-surface flag/slot reads off the variant; building
                        // standard_params() here cloned the vertex_modifiers
                        // Cow per surface per frame.
                        let material_alpha_mode = material.alpha_mode();
                        let material_double_sided = material.double_sided();
                        let material_texture_slots = material.texture_slots();
                        self.ensure_standard_material_texture_slots(
                            device,
                            queue,
                            shared_textures,
                            resources,
                            material_texture_slots,
                            mesh_source,
                            static_texture_lookup,
                        );
                        let material_texture_key = self.custom_material_image_key(
                            device,
                            queue,
                            shared_textures,
                            resources,
                            material,
                            static_texture_lookup,
                        );
                        self.ensure_material_texture_bind_group(device, material_texture_key);
                        let render_path = if skeleton_count > 0 {
                            RenderPath3D::Skinned
                        } else {
                            RenderPath3D::Rigid
                        };
                        let material_kind = self.material_pipeline_kind(
                            device,
                            render_path,
                            material,
                            draw.receive_shadows,
                            static_shader_lookup,
                        );
                        let packed_lod = render_path == RenderPath3D::Rigid
                            && matches!(
                                material_kind,
                                MaterialPipelineKind::Standard
                                    | MaterialPipelineKind::StandardVariant(_)
                            )
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
                            && material_alpha_mode == 0
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
                                    modulate_bias: entry.modulate_bias,
                                },
                            );
                            self.staged_instance_transforms.push(instance.transform);
                            self.staged_rigid_instance_meta.push(instance.rigid_meta);
                            self.staged_skinned_instance_meta
                                .push(instance.skinned_meta);
                            self.stage_blend_shape_instance(
                                mesh_asset,
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
                            let modifier_bound = vertex_modifier_bound(
                                material.vertex_modifiers(),
                                mesh_asset.bounds_radius,
                            );
                            let occlusion_bounds = (
                                mesh_asset.bounds_center,
                                mesh_asset.bounds_radius + modifier_bound,
                            );
                            push_draw_batch(
                                &mut self.draw_batches,
                                DrawBatchPush {
                                    render_path,
                                    mesh: mesh_range,
                                    instance_start,
                                    instance_count,
                                    double_sided: material_double_sided
                                        || self.meshlet_debug_view
                                        || mirrored_winding
                                        || flat_builtin_double_sided,
                                    packed_lod,
                                    material_kind,
                                    alpha_mode: material_alpha_mode,
                                    base_color_texture_slot: material_texture_slots[0],
                                    material_texture_key,
                                    local_bounds: occlusion_bounds,
                                    occlusion_query,
                                    disable_hiz_occlusion: uses_custom_shader
                                        || !material.vertex_modifiers().is_empty()
                                        || material_alpha_mode == 2
                                        || resolved_mesh_blend_active(resolved_blend),
                                    casts_shadows: draw.cast_shadows && !is_camera_stream_quad,
                                    receives_shadows: draw.receive_shadows,
                                    mesh_blend: resolved_mesh_blend_active(resolved_blend)
                                        && !resolved_mesh_blend_screen_pass(resolved_blend),
                                    mesh_blend_screen: resolved_mesh_blend_screen_pass(
                                        resolved_blend,
                                    ),
                                    mesh_blend_params: resolved_blend.packed_params,
                                    mesh_blend_params_ext: resolved_blend.packed_params_ext,
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
                let material: &Material3D = &surface_entries[0].material;
                // Flag/slot reads off the variant; building standard_params()
                // here cloned the vertex_modifiers Cow per draw per frame.
                let material_alpha_mode = material.alpha_mode();
                let material_double_sided = material.double_sided();
                let material_texture_slots = material.texture_slots();
                self.ensure_standard_material_texture_slots(
                    device,
                    queue,
                    shared_textures,
                    resources,
                    material_texture_slots,
                    mesh_source,
                    static_texture_lookup,
                );
                let material_texture_key = self.custom_material_image_key(
                    device,
                    queue,
                    shared_textures,
                    resources,
                    material,
                    static_texture_lookup,
                );
                self.ensure_material_texture_bind_group(device, material_texture_key);
                let material_kind = self.material_pipeline_kind(
                    device,
                    if draw.skeleton.is_some() {
                        RenderPath3D::Skinned
                    } else {
                        RenderPath3D::Rigid
                    },
                    material,
                    draw.receive_shadows,
                    static_shader_lookup,
                );
                let (custom_params_offset, custom_params_len) = self.stage_custom_params(material);
                let (skeleton_start, skeleton_count) = if let Some(skeleton) = &draw.skeleton {
                    let start = self.staged_skeletons.len() as u32;
                    let count = skeleton.matrices.len() as u32;
                    // Palette matrices are already packed as 3 affine rows.
                    self.staged_skeletons.extend_from_slice(&skeleton.matrices);
                    (start, count)
                } else {
                    (0, 0)
                };
                let instance_mats = draw.instance_mats.as_ref();
                let caster_debug = self.shadow_caster_debug_view
                    && draw.cast_shadows
                    && material_alpha_mode == 0
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
                                modulate_bias: surface_entries[0].modulate_bias,
                            },
                        );
                        self.staged_instance_transforms.push(instance.transform);
                        self.staged_rigid_instance_meta.push(instance.rigid_meta);
                        self.staged_skinned_instance_meta
                            .push(instance.skinned_meta);
                        self.stage_blend_shape_instance(
                            mesh_asset,
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
                                    modulate_bias: surface_entries[0].modulate_bias,
                                },
                            );
                            self.staged_instance_transforms.push(instance.transform);
                            self.staged_rigid_instance_meta.push(instance.rigid_meta);
                            self.staged_skinned_instance_meta
                                .push(instance.skinned_meta);
                            self.stage_blend_shape_instance(
                                mesh_asset,
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
                    let modifier_bound = vertex_modifier_bound(
                        material.vertex_modifiers(),
                        mesh_asset.bounds_radius,
                    );
                    let (occlusion_center, occlusion_radius) =
                        (meshlet.center, meshlet.radius.max(0.001) + modifier_bound);
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
                            double_sided: material_double_sided
                                || self.meshlet_debug_view
                                || mirrored_winding
                                || flat_builtin_double_sided,
                            material_kind: material_kind.clone(),
                            alpha_mode: material_alpha_mode,
                            base_color_texture_slot: material_texture_slots[0],
                            material_texture_key,
                            local_bounds: (occlusion_center, occlusion_radius),
                            occlusion_query,
                            disable_hiz_occlusion: uses_custom_shader
                                || !material.vertex_modifiers().is_empty()
                                || material_alpha_mode == 2
                                || resolved_mesh_blend_active(resolved_blend),
                            casts_shadows: meshlet_casts_shadows,
                            receives_shadows: draw.receive_shadows,
                            mesh_blend: resolved_mesh_blend_active(resolved_blend)
                                && !resolved_mesh_blend_screen_pass(resolved_blend),
                            mesh_blend_screen: resolved_mesh_blend_screen_pass(resolved_blend),
                            mesh_blend_params: resolved_blend.packed_params,
                            mesh_blend_params_ext: resolved_blend.packed_params_ext,
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
        self.custom_mesh_ranges = custom_mesh_ranges;
        self.mesh_blend_scratch = mesh_blends;
        self.surface_entries_scratch = surface_entries;
        if !multimesh_reuse {
            self.multimesh_staging_cache_valid = true;
            self.multimesh_staging_cache_camera = camera.position;
        }
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
            let material_texture_key = MaterialTextureKey::empty();
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
                    material_texture_key.state_hash(),
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
                material_texture_key,
                local_center: debug_points_local_center,
                local_radius: 1.0e9,
                occlusion_query: None,
                disable_hiz_occlusion: true,
                casts_shadows: false,
                receives_shadows: false,
                mesh_blend: false,
                mesh_blend_screen: false,
                mesh_blend_params: 0,
                mesh_blend_params_ext: 0,
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
            let material_texture_key = MaterialTextureKey::empty();
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
                    material_texture_key.state_hash(),
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
                material_texture_key,
                local_center: debug_edges_local_center,
                local_radius: 1.0e9,
                occlusion_query: None,
                disable_hiz_occlusion: true,
                casts_shadows: false,
                receives_shadows: false,
                mesh_blend: false,
                mesh_blend_screen: false,
                mesh_blend_params: 0,
                mesh_blend_params_ext: 0,
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
        // First frame with any staged mesh blend: swap the 1x1 placeholder
        // blend depth/mask targets for full-resolution ones before encoding.
        if self.mesh_blend_depth_active
            || self
                .draw_batches
                .iter()
                .any(|batch| batch.mesh_blend || batch.mesh_blend_screen || batch.mesh_blend_depth)
            || self
                .multimesh_batches
                .iter()
                .any(|batch| batch.mesh_blend || batch.mesh_blend_screen || batch.mesh_blend_depth)
        {
            self.ensure_mesh_blend_targets(device);
        }
        if multimesh_reuse {
            // Bleed writes only the params it finds a tint for, and a fresh
            // pack starts them at zero; reused params still carry last build's
            // tint, so reset before the emitters (which come from the regular
            // draws that just changed) are gathered.
            for param in self.staged_multimesh_draw_params.iter_mut() {
                param.packed_bleed = 0;
            }
        }
        self.apply_local_color_bleed();
        // Reused batches are already sorted AND compacted from the build that
        // packed them, and the instance rows they point at were left untouched;
        // re-running compaction would rewrite the same instances into a new
        // layout for nothing.
        if !multimesh_reuse {
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
        }
        self.prepare_mesh_blend_screen(device, queue);
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
        // Only batches on the skinned path sample `skinned_instances` (the
        // rigid / depth-prepass / shadow-rigid shaders read the parallel rigid
        // lane instead), so this is exactly "does the skinned meta buffer have
        // a reader this frame".
        self.staged_has_skinned = self
            .draw_batches
            .iter()
            .any(|batch| matches!(batch.path, RenderPath3D::Skinned));
        self.ensure_instance_transform_capacity(device, self.staged_instance_transforms.len());
        self.ensure_rigid_instance_meta_capacity(device, self.staged_rigid_instance_meta.len());
        self.ensure_skinned_instance_meta_capacity(device, self.staged_skinned_instance_meta.len());
        // Multimesh blend-shape rows live in the tail of the same two buffers,
        // right after the rigid rows (which the rigid/skinned shaders index
        // positionally, so they must own the head).
        let multimesh_rows_rebased = self.rebase_multimesh_blend_tails();
        // Same treatment for the custom-material params: the tail sits after
        // the rows the regular draws just staged. Must run before the draw
        // params are hashed/uploaded below (it rewrites their header offsets)
        // and after `apply_local_color_bleed`, which owns a different lane.
        self.rebase_multimesh_custom_params_tail();
        self.ensure_blend_shape_weight_capacity(
            device,
            (self.staged_blend_shape_weights.len() + self.staged_multimesh_blend_weights.len())
                .max(1),
        );
        self.ensure_blend_shape_instance_meta_capacity(
            device,
            (self.staged_blend_shape_instance_meta.len() + self.staged_multimesh_blend_meta.len())
                .max(1),
        );
        // Skip-identical gates on the rigid-side staging. Skinned / blend-shape
        // animation changes the draw list every frame, so those scenes never
        // reach the transform-only fast path and restage all five lanes below
        // even though only one of them (usually the bone palettes) actually
        // moved. Hashing what a `write_buffer` would send is far cheaper than
        // sending it, and unlike a CPU mirror it costs no extra memory.
        if !self.staged_instance_transforms.is_empty() {
            let hash = super::pod_slice_len_hash(&self.staged_instance_transforms);
            if self.last_uploaded_instance_transforms_hash != Some(hash) {
                let bytes = std::mem::size_of_val(self.staged_instance_transforms.as_slice());
                queue.write_buffer(
                    &self.instance_transform_buffer,
                    0,
                    bytemuck::cast_slice(&self.staged_instance_transforms),
                );
                self.last_uploaded_instance_transforms_hash = Some(hash);
                self.note_upload(bytes);
            }
        }
        if !self.staged_rigid_instance_meta.is_empty() {
            let hash = super::pod_slice_len_hash(&self.staged_rigid_instance_meta);
            if self.last_uploaded_rigid_instance_meta_hash != Some(hash) {
                let bytes = std::mem::size_of_val(self.staged_rigid_instance_meta.as_slice());
                queue.write_buffer(
                    &self.rigid_instance_meta_buffer,
                    0,
                    bytemuck::cast_slice(&self.staged_rigid_instance_meta),
                );
                self.last_uploaded_rigid_instance_meta_hash = Some(hash);
                self.note_upload(bytes);
            }
        }
        // Only the skinned pipelines sample `skinned_instances`; a scene with no
        // bone palettes has no reader at all, so the rows stay unsent.
        if !self.staged_skinned_instance_meta.is_empty() && self.staged_has_skinned {
            let hash = super::pod_slice_len_hash(&self.staged_skinned_instance_meta);
            if self.last_uploaded_skinned_instance_meta_hash != Some(hash) {
                let bytes = std::mem::size_of_val(self.staged_skinned_instance_meta.as_slice());
                queue.write_buffer(
                    &self.skinned_instance_meta_buffer,
                    0,
                    bytemuck::cast_slice(&self.staged_skinned_instance_meta),
                );
                self.last_uploaded_skinned_instance_meta_hash = Some(hash);
                self.note_upload(bytes);
            }
        }
        if !self.staged_blend_shape_weights.is_empty() {
            let hash = super::pod_slice_len_hash(&self.staged_blend_shape_weights);
            if self.last_uploaded_blend_shape_weights_hash != Some(hash) {
                let bytes = std::mem::size_of_val(self.staged_blend_shape_weights.as_slice());
                queue.write_buffer(
                    &self.blend_shape_weight_buffer,
                    0,
                    bytemuck::cast_slice(&self.staged_blend_shape_weights),
                );
                self.last_uploaded_blend_shape_weights_hash = Some(hash);
                self.note_upload(bytes);
            }
        }
        if !self.staged_blend_shape_instance_meta.is_empty() {
            let hash = super::pod_slice_len_hash(&self.staged_blend_shape_instance_meta);
            if self.last_uploaded_blend_shape_meta_hash != Some(hash) {
                let bytes = std::mem::size_of_val(self.staged_blend_shape_instance_meta.as_slice());
                queue.write_buffer(
                    &self.blend_shape_instance_meta_buffer,
                    0,
                    bytemuck::cast_slice(&self.staged_blend_shape_instance_meta),
                );
                self.last_uploaded_blend_shape_meta_hash = Some(hash);
                self.note_upload(bytes);
            }
        }
        self.upload_multimesh_blend_tails(queue);
        self.ensure_skeleton_capacity(device, self.staged_skeletons.len().max(1));
        if !self.staged_skeletons.is_empty() {
            let hash = super::pod_slice_len_hash(&self.staged_skeletons);
            if self.last_uploaded_skeletons_hash != Some(hash) {
                let bytes = std::mem::size_of_val(self.staged_skeletons.as_slice());
                queue.write_buffer(
                    &self.skeleton_buffer,
                    0,
                    bytemuck::cast_slice(&self.staged_skeletons),
                );
                self.last_uploaded_skeletons_hash = Some(hash);
                self.note_upload(bytes);
            }
        }
        // Both arenas carry the multimesh tail after the regular rows.
        self.ensure_custom_params_capacity(
            device,
            (self.staged_custom_params_meta.len()
                + self.staged_multimesh_custom_params_meta.len())
            .max(1),
            (self.staged_custom_params_values.len()
                + self.staged_multimesh_custom_params_values.len())
            .max(1),
        );
        if self.custom_params_meta_uploaded < self.staged_custom_params_meta.len() {
            let upload_start = self.custom_params_meta_uploaded;
            let byte_start = upload_start as u64 * std::mem::size_of::<u32>() as u64;
            let bytes =
                std::mem::size_of_val(&self.staged_custom_params_meta[upload_start..]);
            queue.write_buffer(
                &self.custom_params_meta_buffer,
                byte_start,
                bytemuck::cast_slice(&self.staged_custom_params_meta[upload_start..]),
            );
            self.custom_params_meta_uploaded = self.staged_custom_params_meta.len();
            self.note_upload(bytes);
        }
        if self.custom_params_values_uploaded < self.staged_custom_params_values.len() {
            let upload_start = self.custom_params_values_uploaded;
            let byte_start = upload_start as u64 * std::mem::size_of::<f32>() as u64;
            let bytes =
                std::mem::size_of_val(&self.staged_custom_params_values[upload_start..]);
            queue.write_buffer(
                &self.custom_params_values_buffer,
                byte_start,
                bytemuck::cast_slice(&self.staged_custom_params_values[upload_start..]),
            );
            self.custom_params_values_uploaded = self.staged_custom_params_values.len();
            self.note_upload(bytes);
        }
        self.upload_multimesh_custom_params_tail(queue);
        self.ensure_multimesh_draw_params_capacity(
            device,
            self.staged_multimesh_draw_params.len().max(1),
        );
        // A restage regularly reproduces byte-identical multimesh staging
        // (an unrelated draw changed); hashing the staged bytes and comparing
        // against the last-uploaded hash is far cheaper than re-uploading
        // multiple MB of instances every frame, and unlike a mirror copy it
        // costs no extra CPU memory.
        if !self.staged_multimesh_draw_params.is_empty() {
            let hash = super::pod_slice_len_hash(&self.staged_multimesh_draw_params);
            if self.last_uploaded_multimesh_draw_params_hash != Some(hash) {
                let bytes = std::mem::size_of_val(self.staged_multimesh_draw_params.as_slice());
                queue.write_buffer(
                    &self.multimesh_draw_params_buffer,
                    0,
                    bytemuck::cast_slice(&self.staged_multimesh_draw_params),
                );
                self.last_uploaded_multimesh_draw_params_hash = Some(hash);
                self.note_upload(bytes);
            }
        }
        self.ensure_multimesh_instance_capacity(
            device,
            self.staged_multimesh_instances.len().max(1),
        );
        // Reuse already proved the instance rows are the ones sitting in the
        // GPU buffer, so the hash (a full read of every staged instance byte)
        // is skipped too; only a blend-tail rebase can have touched them.
        if !self.staged_multimesh_instances.is_empty()
            && (!multimesh_reuse || multimesh_rows_rebased)
        {
            let hash = super::pod_slice_len_hash(&self.staged_multimesh_instances);
            if self.last_uploaded_multimesh_instances_hash != Some(hash) {
                let bytes = std::mem::size_of_val(self.staged_multimesh_instances.as_slice());
                queue.write_buffer(
                    &self.multimesh_instance_buffer,
                    0,
                    bytemuck::cast_slice(&self.staged_multimesh_instances),
                );
                self.last_uploaded_multimesh_instances_hash = Some(hash);
                self.note_upload(bytes);
            }
        }
        // Multimesh inputs are topology-only, so rebuild them on the full path.
        // Direct draws also read the identity visible-index buffer, including
        // multimeshes below the GPU-cull threshold.
        // Reused staging means identical topology, so the per-instance cull
        // staging (batch ids + identity indices) would come out byte-identical;
        // skip the O(instances) rebuild when the standing one already matches.
        let cull_inputs_current =
            multimesh_reuse && self.multimesh_cull_staging_matches_current_topology();
        if !self.multimesh_batches.is_empty() && !cull_inputs_current {
            self.rebuild_multimesh_cull_inputs(device, queue);
        }
        if self.should_run_multimesh_cull() {
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
            self.rebuild_and_upload_indirect(queue);
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
        self.update_shadow_state(device, queue, &camera, lighting, self.has_shadow_casters);
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

    /// Same as `stage_blend_shape_instance` but for a multimesh instance: the
    /// row goes into the multimesh tail with a tail-local weight start, both
    /// rebased onto the rigid lengths by `rebase_multimesh_blend_tails` once
    /// the build is done.
    fn stage_multimesh_blend_shape_instance(&mut self, mesh: &MeshAssetRange, weights: &[f32]) {
        let target_count = mesh.blend_shape_target_count;
        let weight_count = weights.len().min(target_count as usize) as u32;
        let weight_start = self.staged_multimesh_blend_weights.len() as u32;
        self.staged_multimesh_blend_weights.extend(
            weights
                .iter()
                .take(weight_count as usize)
                .map(|weight| weight.clamp(0.0, 1.0)),
        );
        self.staged_multimesh_blend_meta
            .push(BlendShapeInstanceMetaGpu {
                weight_range: [weight_start, weight_count, 0, 0],
                shape_range: [
                    mesh.blend_shape_delta_start,
                    target_count,
                    mesh.blend_shape_vertex_start,
                    mesh.blend_shape_vertex_count,
                ],
            });
    }

    /// Point the staged multimesh rows at the tail region they will be uploaded
    /// to. The tail starts where the rigid staging ends, which moves whenever a
    /// regular draw changes its instance count; the rows are shifted by the
    /// delta instead of repacked. Returns `true` when instance rows changed
    /// (i.e. their hash/upload gate has to run again).
    fn rebase_multimesh_blend_tails(&mut self) -> bool {
        if self.staged_multimesh_blend_meta.is_empty() {
            self.multimesh_blend_meta_base = 0;
            self.multimesh_blend_weight_base = 0;
            return false;
        }
        let meta_base = self.staged_blend_shape_instance_meta.len() as u32;
        let weight_base = self.staged_blend_shape_weights.len() as u32;
        let weight_delta = weight_base.wrapping_sub(self.multimesh_blend_weight_base);
        if weight_delta != 0 {
            for meta in self.staged_multimesh_blend_meta.iter_mut() {
                meta.weight_range[0] = meta.weight_range[0].wrapping_add(weight_delta);
            }
            self.multimesh_blend_weight_base = weight_base;
            self.multimesh_blend_tail_revision = self.multimesh_blend_tail_revision.wrapping_add(1);
        }
        let meta_delta = meta_base.wrapping_sub(self.multimesh_blend_meta_base);
        if meta_delta == 0 {
            return false;
        }
        for instance in self.staged_multimesh_instances.iter_mut() {
            instance.blend_meta_id = instance.blend_meta_id.wrapping_add(meta_delta);
        }
        self.multimesh_blend_meta_base = meta_base;
        true
    }

    /// Upload the multimesh blend tails into the region right after the rigid
    /// rows. Gated on (content revision, base, len, buffer capacity) so a
    /// rebuild that only touched the regular draws re-uploads nothing.
    fn upload_multimesh_blend_tails(&mut self, queue: &wgpu::Queue) {
        if !self.staged_multimesh_blend_meta.is_empty() {
            let key = (
                self.multimesh_blend_tail_revision,
                self.multimesh_blend_meta_base,
                self.staged_multimesh_blend_meta.len(),
                self.blend_shape_instance_meta_capacity,
            );
            if self.uploaded_multimesh_blend_meta != Some(key) {
                let offset = self.multimesh_blend_meta_base as u64
                    * std::mem::size_of::<BlendShapeInstanceMetaGpu>() as u64;
                queue.write_buffer(
                    &self.blend_shape_instance_meta_buffer,
                    offset,
                    bytemuck::cast_slice(&self.staged_multimesh_blend_meta),
                );
                self.uploaded_multimesh_blend_meta = Some(key);
            }
        }
        if !self.staged_multimesh_blend_weights.is_empty() {
            let key = (
                self.multimesh_blend_tail_revision,
                self.multimesh_blend_weight_base,
                self.staged_multimesh_blend_weights.len(),
                self.blend_shape_weight_capacity,
            );
            if self.uploaded_multimesh_blend_weights != Some(key) {
                let offset =
                    self.multimesh_blend_weight_base as u64 * std::mem::size_of::<f32>() as u64;
                queue.write_buffer(
                    &self.blend_shape_weight_buffer,
                    offset,
                    bytemuck::cast_slice(&self.staged_multimesh_blend_weights),
                );
                self.uploaded_multimesh_blend_weights = Some(key);
            }
        }
    }

    /// Point the staged multimesh custom-param rows at the arena tail they will
    /// be uploaded to, which starts where the regular draws' rows end. Both
    /// levels of indirection are explicit offsets, so a moved tail is a delta
    /// rewrite rather than a repack:
    ///
    /// - `MultiMeshDrawParamGpu::custom_params[0]` -> header index in the meta
    ///   arena (`0` means the material has no params; a real entry is >= 2),
    /// - meta word 0 of each header -> value index of its modifier records,
    ///   and each following param word packs `(value index << 2) | kind`.
    ///
    /// The values themselves are position-independent, so only the offsets
    /// move.
    fn rebase_multimesh_custom_params_tail(&mut self) {
        if self.staged_multimesh_custom_params_meta.is_empty() {
            self.multimesh_custom_params_meta_base = 0;
            self.multimesh_custom_params_values_base = 0;
            return;
        }
        let values_base = self.staged_custom_params_values.len() as u32;
        let values_delta = values_base.wrapping_sub(self.multimesh_custom_params_values_base);
        if values_delta != 0 {
            let starts = &self.staged_multimesh_custom_params_entry_starts;
            let meta = &mut self.staged_multimesh_custom_params_meta;
            for (entry, &start) in starts.iter().enumerate() {
                let end = starts
                    .get(entry + 1)
                    .copied()
                    .unwrap_or(meta.len() as u32) as usize;
                let start = start as usize;
                meta[start] = meta[start].wrapping_add(values_delta);
                // Word 1 is the modifier count; the rest are param words.
                for word in &mut meta[(start + 2).min(end)..end] {
                    let kind = *word & 0x3;
                    let offset = (*word >> 2).wrapping_add(values_delta);
                    *word = (offset << 2) | kind;
                }
            }
            self.multimesh_custom_params_values_base = values_base;
            self.multimesh_custom_params_tail_revision =
                self.multimesh_custom_params_tail_revision.wrapping_add(1);
        }
        let meta_base = self.staged_custom_params_meta.len() as u32;
        let meta_delta = meta_base.wrapping_sub(self.multimesh_custom_params_meta_base);
        if meta_delta == 0 {
            return;
        }
        for param in self.staged_multimesh_draw_params.iter_mut() {
            if param.custom_params[0] != 0 {
                param.custom_params[0] = param.custom_params[0].wrapping_add(meta_delta);
            }
        }
        self.multimesh_custom_params_meta_base = meta_base;
    }

    /// Upload the multimesh custom-param tails into the region right after the
    /// regular rows. Gated the same way as the blend tails: a rebuild that only
    /// touched the regular draws re-uploads nothing.
    fn upload_multimesh_custom_params_tail(&mut self, queue: &wgpu::Queue) {
        if !self.staged_multimesh_custom_params_meta.is_empty() {
            let key = (
                self.multimesh_custom_params_tail_revision,
                self.multimesh_custom_params_meta_base,
                self.staged_multimesh_custom_params_meta.len(),
                self.custom_params_meta_capacity,
            );
            if self.uploaded_multimesh_custom_params_meta != Some(key) {
                let offset = self.multimesh_custom_params_meta_base as u64
                    * std::mem::size_of::<u32>() as u64;
                queue.write_buffer(
                    &self.custom_params_meta_buffer,
                    offset,
                    bytemuck::cast_slice(&self.staged_multimesh_custom_params_meta),
                );
                self.uploaded_multimesh_custom_params_meta = Some(key);
            }
        }
        if !self.staged_multimesh_custom_params_values.is_empty() {
            let key = (
                self.multimesh_custom_params_tail_revision,
                self.multimesh_custom_params_values_base,
                self.staged_multimesh_custom_params_values.len(),
                self.custom_params_values_capacity,
            );
            if self.uploaded_multimesh_custom_params_values != Some(key) {
                let offset = self.multimesh_custom_params_values_base as u64
                    * std::mem::size_of::<f32>() as u64;
                queue.write_buffer(
                    &self.custom_params_values_buffer,
                    offset,
                    bytemuck::cast_slice(&self.staged_multimesh_custom_params_values),
                );
                self.uploaded_multimesh_custom_params_values = Some(key);
            }
        }
    }

    /// Whether the staged multimesh buffers can be carried into this rebuild
    /// unchanged.
    ///
    /// Everything the staged rows bake in has to be unchanged: the dense draw
    /// SEQUENCE (order decides `instance_start` / `draw_param_index`, so a
    /// reorder or an insert invalidates), each draw's poses (pose-`Arc`
    /// identity, pinned in the snapshot so the pointer cannot be recycled),
    /// world transform, material/surface bindings and scene-resolved blend, and
    /// the camera when a mesh has baked LODs. `force_full_rebuild` (raised on
    /// any resource dirty) covers mesh/material/texture edits.
    ///
    /// Custom-material params ride on the same key: they are a pure function of
    /// the surface bindings (material id + per-surface overrides) plus the
    /// material asset, and their staged rows live in the multimesh tail arena,
    /// so nothing a regular draw stages can move them.
    fn can_reuse_multimesh_staging(
        &self,
        force_full_rebuild: bool,
        draws: &[Draw3DInstance],
        mesh_blends: &[ResolvedMeshBlend],
        camera_position: [f32; 3],
    ) -> bool {
        if force_full_rebuild
            || !self.multimesh_staging_cache_valid
            || self.multimesh_staging_cache.is_empty()
        {
            return false;
        }
        let camera_moved = camera_position != self.multimesh_staging_cache_camera;
        let mut cached = self.multimesh_staging_cache.iter();
        for (draw_index, draw) in draws.iter().enumerate() {
            let Some(dense) = draw.dense_multimesh.as_ref() else {
                continue;
            };
            let Some(snapshot) = cached.next() else {
                return false;
            };
            if snapshot.node_model != dense.node_model
                || snapshot.resolved_blend != mesh_blends[draw_index]
                || (snapshot.lod_sensitive && camera_moved)
                || !same_multimesh_except_node_model(&snapshot.draw, draw)
            {
                return false;
            }
        }
        cached.next().is_none()
    }

    /// Keep the reuse snapshots in step with a transform-only frame: the staged
    /// draw params were patched in place with the new world transforms, so the
    /// staging still matches these draws. Refreshing the stored draw also keeps
    /// pose-`Arc` pointer identity current for the next full rebuild.
    fn sync_multimesh_staging_cache_transforms(&mut self, draws: &[Draw3DInstance]) {
        if !self.multimesh_staging_cache_valid {
            return;
        }
        let mut index = 0usize;
        for draw in draws.iter() {
            let Some(dense) = draw.dense_multimesh.as_ref() else {
                continue;
            };
            let Some(snapshot) = self.multimesh_staging_cache.get_mut(index) else {
                self.multimesh_staging_cache_valid = false;
                return;
            };
            snapshot.node_model = dense.node_model;
            adopt_draw_in_place(&mut snapshot.draw, draw);
            index += 1;
        }
        if index != self.multimesh_staging_cache.len() {
            self.multimesh_staging_cache_valid = false;
        }
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

impl Gpu3D {
    /// Per-frame TAA camera jitter. `Some(offset)` patches the GPU camera
    /// uniform's `view_proj`/`inv_view_proj` slots with a sub-pixel NDC
    /// translation of the unjittered matrices cached by `prepare` (the main
    /// pass, depth prepass, water pass and decals all read that uniform, so
    /// they jitter together); `None` restores the unjittered matrices once.
    ///
    /// Runs outside `prepare` on purpose: static frames skip prepare, but the
    /// jitter must advance every frame for TAA to accumulate sub-pixel detail.
    /// Nothing CPU-side reads the patched values — shadow fitting, frustum
    /// culling, HiZ and CPU occlusion all consume the unjittered locals.
    pub(crate) fn apply_taa_jitter(&mut self, queue: &wgpu::Queue, jitter_ndc: Option<[f32; 2]>) {
        match jitter_ndc {
            Some([jx, jy]) => {
                let jittered =
                    Mat4::from_translation(Vec3::new(jx, jy, 0.0)) * self.taa_base_view_proj;
                let inv = jittered.inverse();
                let inv = if inv.is_finite() {
                    inv
                } else {
                    self.taa_base_inv_view_proj
                };
                queue.write_buffer(
                    &self.camera_buffer,
                    SCENE_VIEW_PROJ_OFFSET,
                    bytemuck::bytes_of(&jittered.to_cols_array_2d()),
                );
                queue.write_buffer(
                    &self.camera_buffer,
                    SCENE_INV_VIEW_PROJ_OFFSET,
                    bytemuck::bytes_of(&inv.to_cols_array_2d()),
                );
                self.taa_jitter_applied = true;
            }
            None => {
                if !self.taa_jitter_applied {
                    return;
                }
                queue.write_buffer(
                    &self.camera_buffer,
                    SCENE_VIEW_PROJ_OFFSET,
                    bytemuck::bytes_of(&self.taa_base_view_proj.to_cols_array_2d()),
                );
                queue.write_buffer(
                    &self.camera_buffer,
                    SCENE_INV_VIEW_PROJ_OFFSET,
                    bytemuck::bytes_of(&self.taa_base_inv_view_proj.to_cols_array_2d()),
                );
                self.taa_jitter_applied = false;
            }
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

#[path = "prepare/bleed.rs"]
mod bleed;
pub(super) use bleed::{BleedEmitter, BleedOccluder, multimesh_world_bounds};
