use super::*;
use rayon::slice::ParallelSliceMut;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

const PARALLEL_BATCH_SORT_MIN: usize = 10_000;

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
        let has_dense_multimesh =
            !draws_unchanged && draws.iter().any(|d| d.dense_multimesh.is_some());
        let transform_only_semantic = !draws_unchanged
            && !has_dense_multimesh
            && draws.len() == self.last_draws.len()
            && self
                .last_draws
                .iter()
                .zip(draws.iter())
                .all(|(prev, next)| {
                    prev.instance_mats.len() == 1
                        && next.instance_mats.len() == 1
                        && same_draw_except_model(prev, next)
                });
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
        let transform_only_changed =
            !draws_unchanged && transform_only_semantic && stable_instance_ranges;
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
                    || self.frustum_cull_staging.len() != self.draw_batches.len();
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
                    self.frustum_cull_staging.clear();
                    self.frustum_cull_staging.reserve(self.draw_batches.len());
                    for batch in &self.draw_batches {
                        let instance =
                            &self.staged_instance_transforms[batch.instance_start as usize];
                        let model_cols = model_cols_from_affine_rows(instance);
                        self.frustum_cull_staging.push(FrustumCullItemGpu {
                            model_0: model_cols[0],
                            model_1: model_cols[1],
                            model_2: model_cols[2],
                            model_3: model_cols[3],
                            local_center_radius: [
                                batch.local_center[0],
                                batch.local_center[1],
                                batch.local_center[2],
                                batch.local_radius.max(0.0),
                            ],
                            cull_flags: [
                                if batch.disable_hiz_occlusion {
                                    CULL_FLAG_DISABLE_HIZ_OCCLUSION
                                } else {
                                    0
                                },
                                0,
                                0,
                                0,
                            ],
                        });
                    }
                    queue.write_buffer(
                        &self.frustum_cull_items_buffer,
                        0,
                        bytemuck::cast_slice(&self.frustum_cull_staging),
                    );
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
            self.update_shadow_state(queue, &camera, lighting);
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
            let frustum_cull_active = self.should_run_frustum_cull();
            let hiz_active = self.should_run_hiz_occlusion(frustum_cull_active);
            if frustum_cull_active {
                let frustum_inputs_invalid = !self.frustum_gpu_inputs_valid
                    || self.indirect_staging.len() != self.draw_batches.len()
                    || self.frustum_cull_staging.len() != self.draw_batches.len();
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
                    self.frustum_cull_staging.clear();
                    self.frustum_cull_staging.reserve(self.draw_batches.len());
                    for batch in &self.draw_batches {
                        let instance =
                            &self.staged_instance_transforms[batch.instance_start as usize];
                        let model_cols = model_cols_from_affine_rows(instance);
                        self.frustum_cull_staging.push(FrustumCullItemGpu {
                            model_0: model_cols[0],
                            model_1: model_cols[1],
                            model_2: model_cols[2],
                            model_3: model_cols[3],
                            local_center_radius: [
                                batch.local_center[0],
                                batch.local_center[1],
                                batch.local_center[2],
                                batch.local_radius.max(0.0),
                            ],
                            cull_flags: [
                                if batch.disable_hiz_occlusion {
                                    CULL_FLAG_DISABLE_HIZ_OCCLUSION
                                } else {
                                    0
                                },
                                0,
                                0,
                                0,
                            ],
                        });
                    }
                    queue.write_buffer(
                        &self.frustum_cull_items_buffer,
                        0,
                        bytemuck::cast_slice(&self.frustum_cull_staging),
                    );
                    step_timing.cull_input_prep += cull_start.elapsed();
                    self.frustum_gpu_inputs_valid = true;
                } else {
                    step_timing.indirect_skipped = step_timing.indirect_skipped.saturating_add(1);

                    let cull_start = Instant::now();
                    self.dirty_cull_batch_spans_scratch.clear();
                    let mut dirty_span_idx = 0usize;
                    for (batch_idx, batch) in self.draw_batches.iter().enumerate() {
                        while dirty_span_idx < self.merged_instance_spans_scratch.len()
                            && self.merged_instance_spans_scratch[dirty_span_idx].end
                                <= batch.instance_start
                        {
                            dirty_span_idx += 1;
                        }
                        let Some(span) = self.merged_instance_spans_scratch.get(dirty_span_idx)
                        else {
                            break;
                        };
                        if batch.instance_start >= span.start && batch.instance_start < span.end {
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
                        for batch_span in self.dirty_cull_batch_spans_scratch.iter() {
                            for batch_idx in batch_span.clone() {
                                let batch = &self.draw_batches[batch_idx];
                                let instance =
                                    &self.staged_instance_transforms[batch.instance_start as usize];
                                let model_cols = model_cols_from_affine_rows(instance);
                                self.frustum_cull_staging[batch_idx] = FrustumCullItemGpu {
                                    model_0: model_cols[0],
                                    model_1: model_cols[1],
                                    model_2: model_cols[2],
                                    model_3: model_cols[3],
                                    local_center_radius: [
                                        batch.local_center[0],
                                        batch.local_center[1],
                                        batch.local_center[2],
                                        batch.local_radius.max(0.0),
                                    ],
                                    cull_flags: [
                                        if batch.disable_hiz_occlusion {
                                            CULL_FLAG_DISABLE_HIZ_OCCLUSION
                                        } else {
                                            0
                                        },
                                        0,
                                        0,
                                        0,
                                    ],
                                };
                            }
                            let byte_start = (batch_span.start
                                * std::mem::size_of::<FrustumCullItemGpu>())
                                as u64;
                            queue.write_buffer(
                                &self.frustum_cull_items_buffer,
                                byte_start,
                                bytemuck::cast_slice(
                                    &self.frustum_cull_staging[batch_span.start..batch_span.end],
                                ),
                            );
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
            self.update_shadow_state(queue, &camera, lighting);
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

        self.staged_instance_transforms.clear();
        self.staged_instance_transforms.reserve(draws.len());
        self.staged_instance_materials.clear();
        self.staged_instance_materials.reserve(draws.len());
        self.staged_rigid_instance_meta.clear();
        self.staged_rigid_instance_meta.reserve(draws.len());
        self.staged_skinned_instance_meta.clear();
        self.staged_skinned_instance_meta.reserve(draws.len());
        self.staged_blend_shape_weights.clear();
        self.staged_blend_shape_instance_meta.clear();
        self.staged_blend_shape_instance_meta.reserve(draws.len());
        self.staged_skeletons.clear();
        self.staged_custom_params_meta_scratch.clear();
        self.staged_custom_params_values_scratch.clear();
        self.staged_custom_params_key_scratch.clear();
        self.draw_batches.clear();
        self.multimesh_batches.clear();
        self.staged_multimesh_instances.clear();
        self.staged_multimesh_draw_params.clear();
        self.draw_batches.reserve(draws.len());
        self.last_draw_instance_spans.clear();
        self.last_draw_instance_spans.reserve(draws.len());
        self.last_draw_instance_span_ranges.clear();
        self.last_draw_instance_span_ranges.reserve(draws.len());
        self.frustum_cull_staging.clear();
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

        for (draw_index, draw) in draws.iter().enumerate() {
            let resolved_blend = mesh_blends[draw_index];
            let draw_instance_start = self.staged_instance_transforms.len() as u32;
            let draw_span_start = self.last_draw_instance_spans.len();
            let is_debug_point = matches!(draw.kind, Draw3DKind::DebugPointCube);
            let is_debug_edge = matches!(draw.kind, Draw3DKind::DebugEdgeCylinder);
            let is_camera_stream_quad = matches!(draw.kind, Draw3DKind::CameraStreamQuad { .. });
            let mesh_source = match draw.kind {
                Draw3DKind::Mesh(mesh) => resources.mesh_source(mesh).unwrap_or("__cube__"),
                Draw3DKind::CameraStreamQuad { .. } => "__quad__",
                Draw3DKind::DebugPointCube => "__cube__",
                Draw3DKind::DebugEdgeCylinder => "__cylinder__",
            };
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
                    surface_entries.push((
                        active_lod.full,
                        Material3D::Standard(StandardMaterial3D {
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
                    ));
                }
                Draw3DKind::DebugEdgeCylinder => {
                    let color = draw.debug_color.unwrap_or([0.15, 0.95, 0.95, 1.0]);
                    surface_entries.push((
                        active_lod.full,
                        Material3D::Standard(StandardMaterial3D {
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
                    ));
                }
                Draw3DKind::CameraStreamQuad { texture, tint } => {
                    surface_entries.push((
                        active_lod.full,
                        Material3D::Unlit(perro_render_bridge::UnlitMaterial3D {
                            base_color_factor: tint,
                            alpha_mode: 2,
                            double_sided: true,
                            base_color_texture: texture.index(),
                            ..perro_render_bridge::UnlitMaterial3D::default()
                        }),
                    ));
                }
                Draw3DKind::Mesh(_) => {
                    for (surface_index, surface) in draw.surfaces.iter().enumerate() {
                        let Some(range) = active_lod.surface_ranges.get(surface_index).copied()
                        else {
                            continue;
                        };
                        let base_material = surface
                            .material
                            .and_then(|id| resources.material(id))
                            .unwrap_or_default();
                        surface_entries
                            .push((range, apply_surface_binding(base_material, surface)));
                    }
                    if surface_entries.is_empty() {
                        surface_entries.push((active_lod.full, Material3D::default()));
                    }
                }
            }
            if surface_entries.is_empty() {
                self.last_draw_instance_spans
                    .push(draw_instance_start..(self.staged_instance_transforms.len() as u32));
                let draw_span_end = self.last_draw_instance_spans.len();
                self.last_draw_instance_span_ranges
                    .push(draw_span_start..draw_span_end);
                continue;
            }
            if let Some(dense) = &draw.dense_multimesh {
                let draw_model = Mat4::from_cols_array_2d(&dense.node_model);
                for (range, material) in surface_entries.iter() {
                    let params = material.standard_params();
                    let packed_color = pack_unorm4x8(params.base_color_factor);
                    let packed_emissive = pack_unorm4x8([
                        params.emissive_factor[0],
                        params.emissive_factor[1],
                        params.emissive_factor[2],
                        1.0,
                    ]);
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
                        });
                    let mirrored_winding = draw_model.determinant() < 0.0;
                    let instance_start = self.staged_multimesh_instances.len() as u32;
                    for pose in dense.instances.iter() {
                        let weights = if pose.has_blend_shape_weight_override {
                            pose.blend_shape_weights.as_ref()
                        } else {
                            draw.blend_shape_weights.as_ref()
                        };
                        let blend_shape_meta =
                            self.stage_blend_shape_weight_range(&mesh_asset, weights);
                        self.staged_multimesh_instances.push(MultiMeshInstanceGpu {
                            position: pose.position,
                            rotation: pose.rotation,
                            draw_id: draw_param_index,
                            weight_range: blend_shape_meta.weight_range,
                            shape_range: blend_shape_meta.shape_range,
                        });
                    }
                    let instance_count = (self.staged_multimesh_instances.len() as u32)
                        .saturating_sub(instance_start);
                    if instance_count > 0 {
                        self.multimesh_batches.push(MultiMeshBatch {
                            mesh: *range,
                            instance_start,
                            instance_count,
                            draw_param_index,
                            double_sided: params.double_sided || mirrored_winding,
                            mesh_blend: resolved_mesh_blend_active(resolved_blend),
                        });
                    }
                }
                self.last_draw_instance_spans
                    .push(draw_instance_start..(self.staged_instance_transforms.len() as u32));
                let draw_span_end = self.last_draw_instance_spans.len();
                self.last_draw_instance_span_ranges
                    .push(draw_span_start..draw_span_end);
                continue;
            }
            // CPU occlusion query mode works at object granularity.
            // Force whole-mesh batching in that mode so each object can be queried.
            let builtin_primitive_source = is_builtin_primitive_mesh_source(mesh_source);
            let allow_meshlets = draw.meshlet_override.unwrap_or(!builtin_primitive_source);
            let use_meshlets = !is_debug_point
                && !is_debug_edge
                && !is_camera_stream_quad
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
                        .extend_from_slice(skeleton.matrices.as_ref());
                    (start, count)
                } else {
                    (0, 0)
                };
                let instance_mats = draw.instance_mats.as_ref();
                if is_debug_point {
                    let material = &surface_entries[0].1;
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
                                receive_shadows: false,
                            },
                        ));
                        debug_points_count = debug_points_count.saturating_add(1);
                    }
                } else if is_debug_edge {
                    let material = &surface_entries[0].1;
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
                                receive_shadows: false,
                            },
                        ));
                        debug_edges_count = debug_edges_count.saturating_add(1);
                    }
                } else {
                    for (range, material) in surface_entries.iter() {
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
                            if skeleton_count > 0 {
                                RenderPath3D::Skinned
                            } else {
                                RenderPath3D::Rigid
                            },
                            material,
                            static_shader_lookup,
                        );
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
                                    receive_shadows: draw.receive_shadows,
                                },
                            );
                            self.staged_instance_transforms.push(instance.transform);
                            self.staged_instance_materials.push(instance.material);
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
                            let multi_instance = instance_count > 1;
                            let mirrored_winding = instance_mats
                                .iter()
                                .any(|model| Mat4::from_cols_array_2d(model).determinant() < 0.0);
                            let occlusion_bounds = if multi_instance {
                                ([0.0, 0.0, 0.0], 1.0e9)
                            } else {
                                (mesh_asset.bounds_center, mesh_asset.bounds_radius)
                            };
                            push_draw_batch(
                                &mut self.draw_batches,
                                DrawBatchPush {
                                    render_path: if skeleton_count > 0 {
                                        RenderPath3D::Skinned
                                    } else {
                                        RenderPath3D::Rigid
                                    },
                                    mesh: *range,
                                    instance_start,
                                    instance_count,
                                    double_sided: standard_params.double_sided
                                        || self.meshlet_debug_view
                                        || mirrored_winding,
                                    material_kind,
                                    alpha_mode: standard_params.alpha_mode,
                                    base_color_texture_slot: standard_params.base_color_texture,
                                    local_bounds: occlusion_bounds,
                                    occlusion_query,
                                    disable_hiz_occlusion: multi_instance
                                        || standard_params.alpha_mode == 2
                                        || resolved_mesh_blend_active(resolved_blend),
                                    casts_shadows: draw.cast_shadows && !is_camera_stream_quad,
                                    receives_shadows: draw.receive_shadows,
                                    mesh_blend: resolved_mesh_blend_active(resolved_blend),
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
                let (_, material) = &surface_entries[0];
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
                        .extend_from_slice(skeleton.matrices.as_ref());
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
                for meshlet in active_lod.meshlets.iter().copied() {
                    // Keep meshlet casters for stable shadow fitting even when off-screen.
                    // CPU query occlusion at meshlet granularity self-occludes dynamic meshes.
                    // Keep meshlet occlusion GPU-driven only; CPU mode skips meshlet occlusion.
                    let occlusion_query = None;
                    let instance_start = self.staged_instance_transforms.len() as u32;
                    for model in instance_mats.iter().copied() {
                        let instance = build_instance(
                            model,
                            material,
                            BuildInstanceArgs {
                                debug_view: self.meshlet_debug_view,
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
                                receive_shadows: draw.receive_shadows,
                            },
                        );
                        self.staged_instance_transforms.push(instance.transform);
                        self.staged_instance_materials.push(instance.material);
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
                    if instance_count == 0 {
                        continue;
                    }
                    let multi_instance = instance_count > 1;
                    let mirrored_winding = instance_mats
                        .iter()
                        .any(|model| Mat4::from_cols_array_2d(model).determinant() < 0.0);
                    // Use per-meshlet local bounds for tighter frustum/occlusion rejection.
                    let (occlusion_center, occlusion_radius) = if multi_instance {
                        ([0.0, 0.0, 0.0], 1.0e9)
                    } else {
                        (meshlet.center, meshlet.radius.max(0.001))
                    };
                    push_draw_batch(
                        &mut self.draw_batches,
                        DrawBatchPush {
                            render_path: if skeleton_count > 0 {
                                RenderPath3D::Skinned
                            } else {
                                RenderPath3D::Rigid
                            },
                            mesh: MeshRange {
                                index_start: meshlet.index_start,
                                index_count: meshlet.index_count,
                                base_vertex: mesh_asset.full.base_vertex,
                            },
                            instance_start,
                            instance_count,
                            double_sided: standard_params.double_sided
                                || self.meshlet_debug_view
                                || mirrored_winding,
                            material_kind: material_kind.clone(),
                            alpha_mode: standard_params.alpha_mode,
                            base_color_texture_slot: standard_params.base_color_texture,
                            local_bounds: (occlusion_center, occlusion_radius),
                            occlusion_query,
                            disable_hiz_occlusion: multi_instance
                                || standard_params.alpha_mode == 2
                                || resolved_mesh_blend_active(resolved_blend),
                            casts_shadows: meshlet_casts_shadows,
                            receives_shadows: draw.receive_shadows,
                            mesh_blend: resolved_mesh_blend_active(resolved_blend),
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
        }
        self.mesh_blend_scratch = mesh_blends;
        self.surface_entries_scratch = surface_entries;
        if !debug_point_instances.is_empty() {
            debug_points_start = Some(self.staged_instance_transforms.len() as u32);
            for instance in debug_point_instances.drain(..) {
                self.staged_instance_transforms.push(instance.transform);
                self.staged_instance_materials.push(instance.material);
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
                self.staged_instance_materials.push(instance.material);
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
                mesh_blend_depth: false,
                blend_layers: BitMask::ALL.bits(),
                blend_mask: BitMask::NONE.bits(),
                order_index: self.draw_batches.len() as u32,
            });
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
        if self.multimesh_batches.len() >= PARALLEL_BATCH_SORT_MIN {
            self.multimesh_batches.par_sort_unstable_by_key(|b| {
                (
                    b.mesh_blend,
                    b.double_sided,
                    b.mesh.index_start,
                    b.draw_param_index,
                )
            });
        } else {
            self.multimesh_batches.sort_unstable_by_key(|b| {
                (
                    b.mesh_blend,
                    b.double_sided,
                    b.mesh.index_start,
                    b.draw_param_index,
                )
            });
        }
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
        self.has_shadow_casters = self
            .draw_batches
            .iter()
            .any(|batch| !batch.draw_on_top && batch.casts_shadows && batch.alpha_mode == 0);
        if occlusion_capture_this_frame {
            self.ensure_occlusion_query_capacity(
                device,
                self.occlusion_query_keys_this_frame.len() as u32,
            );
        }
        self.ensure_instance_transform_capacity(device, self.staged_instance_transforms.len());
        self.ensure_instance_material_capacity(device, self.staged_instance_materials.len());
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
        if !self.staged_instance_materials.is_empty() {
            queue.write_buffer(
                &self.instance_material_buffer,
                0,
                bytemuck::cast_slice(&self.staged_instance_materials),
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
        let frustum_cull_active = self.should_run_frustum_cull();
        let hiz_active = self.should_run_hiz_occlusion(frustum_cull_active);
        if frustum_cull_active {
            let indirect_start = Instant::now();
            self.ensure_frustum_cull_capacity(device, self.draw_batches.len());
            self.indirect_staging.clear();
            self.indirect_staging.reserve(self.draw_batches.len());
            self.frustum_cull_staging.clear();
            self.frustum_cull_staging.reserve(self.draw_batches.len());
            for batch in &self.draw_batches {
                let model_cols = model_cols_from_affine_rows(
                    &self.staged_instance_transforms[batch.instance_start as usize],
                );
                self.indirect_staging.push(DrawIndexedIndirectGpu {
                    index_count: batch.mesh.index_count,
                    instance_count: batch.instance_count,
                    first_index: batch.mesh.index_start,
                    base_vertex: batch.mesh.base_vertex,
                    first_instance: batch.instance_start,
                });
                self.frustum_cull_staging.push(FrustumCullItemGpu {
                    model_0: model_cols[0],
                    model_1: model_cols[1],
                    model_2: model_cols[2],
                    model_3: model_cols[3],
                    local_center_radius: [
                        batch.local_center[0],
                        batch.local_center[1],
                        batch.local_center[2],
                        batch.local_radius.max(0.0),
                    ],
                    cull_flags: [
                        if batch.disable_hiz_occlusion {
                            CULL_FLAG_DISABLE_HIZ_OCCLUSION
                        } else {
                            0
                        },
                        0,
                        0,
                        0,
                    ],
                });
            }
            queue.write_buffer(
                &self.indirect_buffer,
                0,
                bytemuck::cast_slice(&self.indirect_staging),
            );
            step_timing.indirect_prep += indirect_start.elapsed();

            let cull_start = Instant::now();
            queue.write_buffer(
                &self.frustum_cull_items_buffer,
                0,
                bytemuck::cast_slice(&self.frustum_cull_staging),
            );
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
        self.update_shadow_state(queue, &camera, lighting);
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
