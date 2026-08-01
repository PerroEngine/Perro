use super::*;

// Coalesces consecutive indirect-buffer indices into multi_draw_indexed_indirect
// runs. Push contiguous indices in draw order; call flush() on any pipeline/state
// break and at the end of the group. No-op (falls back per-call) when multi-draw
// is unavailable/disabled for the caller.
struct IndirectRunBuilder {
    enabled: bool,
    stride: u64,
    run: Option<(usize, u32)>,
}

impl IndirectRunBuilder {
    #[inline]
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            stride: std::mem::size_of::<DrawIndexedIndirectGpu>() as u64,
            run: None,
        }
    }

    // Returns true if `i` was absorbed into (or started) a run. Returns false
    // when coalescing is disabled; caller should issue a direct indirect draw.
    #[inline]
    fn push(&mut self, buffer: &wgpu::Buffer, pass: &mut wgpu::RenderPass<'_>, i: usize) -> bool {
        if !self.enabled {
            return false;
        }
        match &mut self.run {
            Some((run_start, run_len)) if *run_start + *run_len as usize == i => {
                *run_len += 1;
            }
            _ => {
                self.flush(buffer, pass);
                self.run = Some((i, 1));
            }
        }
        true
    }

    #[inline]
    fn flush(&mut self, buffer: &wgpu::Buffer, pass: &mut wgpu::RenderPass<'_>) {
        if let Some((run_start, run_len)) = self.run.take() {
            pass.multi_draw_indexed_indirect(buffer, run_start as u64 * self.stride, run_len);
        }
    }
}

// Issue one planned run out of the GPU-compacted buffer: `run.len` is the
// max_count, the survivor count sits in the count buffer at the run's
// destination slot, and the compacted commands start there too.
#[inline]
pub(super) fn draw_compacted_run(
    pass: &mut wgpu::RenderPass<'_>,
    compact_buffer: &wgpu::Buffer,
    count_buffer: &wgpu::Buffer,
    run: IndirectRunGpu,
) {
    let slot = u64::from(run.dst_base + run.start);
    pass.multi_draw_indexed_indirect_count(
        compact_buffer,
        slot * std::mem::size_of::<DrawIndexedIndirectGpu>() as u64,
        count_buffer,
        slot * std::mem::size_of::<u32>() as u64,
        run.len,
    );
}

// Sky as an in-pass draw: one fullscreen triangle on the far plane. The
// pipeline depth-tests LessEqual with writes off (see create_sky_pipeline), so
// it only shades pixels the opaque geometry left at depth 1.
fn draw_sky<'a>(gpu: &'a Gpu3D, pass: &mut wgpu::RenderPass<'a>) {
    let sky_pipeline = gpu
        .active_sky_pipeline_key
        .as_ref()
        .and_then(|key| gpu.custom_sky_pipelines.get(key))
        .unwrap_or_else(|| gpu.pipelines.sky());
    pass.set_pipeline(sky_pipeline);
    pass.set_bind_group(0, &gpu.sky_bind_group, &[]);
    pass.draw(0..3, 0..1);
}

impl Gpu3D {
    #[allow(clippy::too_many_arguments)]
    pub fn render_pass(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        clear_color: wgpu::Color,
        depth_prepass_needed: bool,
        camera: &Camera3DState,
        render_sky: bool,
    ) {
        self.perf_counters.pipeline_switches = 0;
        self.perf_counters.texture_bind_group_switches = 0;
        self.perf_counters.camera_bind_group_switches = 0;
        self.pass_counters = PassCounters::default();
        let frustum_cull_active = self.should_run_frustum_cull();
        let hiz_active = self.should_run_hiz_occlusion(frustum_cull_active);
        let multimesh_cull_active = self.should_run_multimesh_cull();
        self.multimesh_cull_active = multimesh_cull_active;
        if multimesh_cull_active {
            // The cull compute overwrites the visible-index buffer this frame;
            // the identity prime is gone until the next topology build.
            self.multimesh_identity_primed = false;
        }
        let mesh_blend_depth_active = self.mesh_blend_depth_active;
        // Mesh blending forces the depth prepass: the mask pass depth-tests
        // against it and the seam pass reads it for world reconstruction.
        let depth_prepass_active = self.should_run_depth_prepass(
            depth_prepass_needed
                || self.ssao_pass.is_some()
                || mesh_blend_depth_active
                || self.mesh_blend_screen_active,
            hiz_active,
        );
        // Unified depth: at 1 sample the prepass and main depth share the
        // Depth32Float format, so the prepass result is copied into depth_view
        // and the main pass loads it instead of re-rasterizing occluders.
        // pipeline_for_batch reads this to drop depth writes on covered batches.
        self.unified_depth_active = self.sample_count == 1 && depth_prepass_active;
        let query_count = if self.cpu_occlusion_enabled
            && self.pending_occlusion_query_count == 0
            && self.pending_occlusion_map_rx.is_none()
        {
            self.occlusion_query_keys_this_frame.len() as u32
        } else {
            0
        };
        let render_sky = render_sky && self.sky_enabled;
        let has_any_work = !self.draw_batches.is_empty()
            || !self.multimesh_batches.is_empty()
            || render_sky
            || depth_prepass_active
            || mesh_blend_depth_active
            || self.mesh_blend_screen_active
            || hiz_active
            || (self.shadow_pass_enabled && self.has_shadow_casters);
        if !has_any_work {
            self.pass_counters.render_passes += 1;
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_mesh_clear_only_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    resolve_target: None,
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
            return;
        }
        if self.shadow_pass_enabled && self.has_shadow_casters {
            if self.ray_shadow_enabled {
                for cascade in 0..MAX_SHADOW_RAY_CASCADES.min(self.shadow_layer_views.len()) {
                    // Cached-valid layer: depth retained, skip the pass entirely.
                    if self
                        .shadow_layer_valid
                        .get(cascade)
                        .copied()
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    self.compute_shadow_cull(cascade);
                    self.pass_counters.render_passes += 1;
                    let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("perro_ray_shadow3d_pass"),
                        color_attachments: &[],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &self.shadow_layer_views[cascade],
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
                    draw_shadow_batches(self, &mut shadow_pass, cascade);
                    drop(shadow_pass);
                    if let Some(valid) = self.shadow_layer_valid.get_mut(cascade) {
                        *valid = true;
                    }
                }
            }
            for spot in 0..self
                .spot_shadow_count
                .min(self.spot_shadow_layer_views.len())
            {
                let flat = MAX_SHADOW_RAY_LIGHTS * MAX_SHADOW_RAY_CASCADES + spot;
                if self.shadow_layer_valid.get(flat).copied().unwrap_or(false) {
                    continue;
                }
                self.compute_shadow_cull(flat);
                self.pass_counters.render_passes += 1;
                let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("perro_spot_shadow3d_pass"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.spot_shadow_layer_views[spot],
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
                draw_shadow_batches(self, &mut shadow_pass, flat);
                drop(shadow_pass);
                if let Some(valid) = self.shadow_layer_valid.get_mut(flat) {
                    *valid = true;
                }
            }
            let point_layers = self
                .point_shadow_count
                .saturating_mul(POINT_SHADOW_FACE_COUNT)
                .min(self.point_shadow_layer_views.len());
            for layer in 0..point_layers {
                let flat = MAX_SHADOW_RAY_LIGHTS * MAX_SHADOW_RAY_CASCADES
                    + MAX_SHADOW_SPOT_LIGHTS
                    + layer;
                if self.shadow_layer_valid.get(flat).copied().unwrap_or(false) {
                    continue;
                }
                self.compute_shadow_cull(flat);
                self.pass_counters.render_passes += 1;
                let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("perro_point_shadow3d_pass"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.point_shadow_layer_views[layer],
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
                draw_shadow_batches(self, &mut shadow_pass, flat);
                drop(shadow_pass);
                if let Some(valid) = self.shadow_layer_valid.get_mut(flat) {
                    *valid = true;
                }
            }
        }
        if frustum_cull_active {
            let mut cull_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perro_frustum_cull_pass"),
                timestamp_writes: None,
            });
            cull_pass.set_pipeline(&self.frustum_cull_pipeline);
            cull_pass.set_bind_group(0, &self.frustum_cull_bind_group, &[]);
            let groups = (self.draw_batches.len() as u32).div_ceil(FRUSTUM_CULL_WORKGROUP_SIZE);
            cull_pass.dispatch_workgroups(groups, 1, 1);
        }
        // Multimesh per-instance cull. Must run before the prepass so the prepass
        // and main pass draw the same visible set. Counters cleared each frame;
        // cs_finalize writes the per-batch instance_count from the counter.
        if multimesh_cull_active {
            let counter_bytes = (self.multimesh_batches.len() * std::mem::size_of::<u32>()) as u64;
            encoder.clear_buffer(&self.multimesh_cull_counter_buffer, 0, Some(counter_bytes));
            let mut cull_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perro_multimesh_cull_pass"),
                timestamp_writes: None,
            });
            cull_pass.set_bind_group(0, &self.multimesh_cull_bind_group, &[]);
            cull_pass.set_pipeline(&self.multimesh_cull_pipeline);
            let instance_groups = (self.staged_multimesh_instances.len() as u32)
                .div_ceil(FRUSTUM_CULL_WORKGROUP_SIZE);
            cull_pass.dispatch_workgroups(instance_groups, 1, 1);
            cull_pass.set_pipeline(&self.multimesh_cull_finalize_pipeline);
            let batch_groups =
                (self.multimesh_batches.len() as u32).div_ceil(FRUSTUM_CULL_WORKGROUP_SIZE);
            cull_pass.dispatch_workgroups(batch_groups, 1, 1);
        }
        // Indirect-count path: plan every consuming pass's state runs (pure CPU
        // segmentation over draw_batches) and upload the whole plan once, then
        // compact the culled command buffer per run so each draw submits only
        // the survivors.
        self.build_indirect_run_plans(
            frustum_cull_active
                && self.multi_draw_indirect_enabled
                && self.multi_draw_indirect_count_enabled
                && !self.draw_batches.is_empty(),
            depth_prepass_active,
            mesh_blend_depth_active,
        );
        self.upload_indirect_run_plan(queue);
        // The depth prepass and the mesh-blend depth pass read the frustum-only
        // cull result, so their compaction is encoded here. Hi-z rewrites the
        // indirect buffer between them and the main pass; when it is off nothing
        // does, so the main pass's runs ride along in this same dispatch and the
        // second one is skipped entirely.
        let early_runs = 0..if hiz_active {
            self.indirect_run_plan_main.start
        } else {
            self.indirect_run_plan.len()
        };
        self.encode_indirect_compaction(
            queue,
            encoder,
            INDIRECT_COMPACT_DISPATCH_EARLY,
            early_runs,
        );
        if depth_prepass_active {
            self.pass_counters.render_passes += 1;
            let mut prepass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_depth_prepass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_prepass_view,
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
            let mut current_state: Option<(RenderPath3D, bool, bool)> = None;
            prepass.set_vertex_buffer(1, self.instance_transform_buffer.slice(..));
            // Count mode replaces the coalescing builder: the runs were planned
            // (and GPU-compacted) up front, so each run is issued at its first
            // batch and the builder stays disabled.
            let count_mode = self.multi_draw_indirect_count_prepass_active;
            let mut prepass_run = IndirectRunBuilder::new(
                frustum_cull_active && self.multi_draw_indirect_enabled && !count_mode,
            );
            let plan_range = self.indirect_run_plan_prepass.clone();
            let mut plan_cursor = plan_range.start;
            for &i in &self.depth_prepass_batch_indices {
                let batch = &self.draw_batches[i];
                let state = (batch.path, batch.double_sided, batch.packed_lod);
                if current_state != Some(state) {
                    prepass_run.flush(&self.indirect_buffer, &mut prepass);
                    let (camera_bg, vertex_buf, pipeline) = if batch.path == RenderPath3D::Rigid {
                        let p = if batch.double_sided {
                            if batch.packed_lod {
                                &self.pipelines.depth_prepass_rigid_packed_lod().double_sided
                            } else {
                                &self.pipelines.depth_prepass_rigid().double_sided
                            }
                        } else {
                            if batch.packed_lod {
                                &self.pipelines.depth_prepass_rigid_packed_lod().culled
                            } else {
                                &self.pipelines.depth_prepass_rigid().culled
                            }
                        };
                        let v = if batch.packed_lod {
                            &self.packed_lod_vertex_buffer
                        } else {
                            &self.rigid_vertex_buffer
                        };
                        (&self.rigid_camera_bind_group, v, p)
                    } else {
                        let p = if batch.double_sided {
                            &self.pipelines.depth_prepass_skinned().double_sided
                        } else {
                            &self.pipelines.depth_prepass_skinned().culled
                        };
                        (&self.camera_bind_group, &self.vertex_buffer, p)
                    };
                    prepass.set_bind_group(0, camera_bg, &[]);
                    if batch.packed_lod {
                        prepass.set_index_buffer(
                            self.packed_lod_index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                    } else {
                        prepass.set_index_buffer(
                            self.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                    }
                    prepass.set_vertex_buffer(0, vertex_buf.slice(..));
                    if batch.path == RenderPath3D::Skinned {
                        prepass.set_vertex_buffer(2, self.skinned_instance_meta_buffer.slice(..));
                    } else {
                        prepass.set_vertex_buffer(2, self.rigid_instance_meta_buffer.slice(..));
                    }
                    prepass.set_pipeline(pipeline);
                    current_state = Some(state);
                }
                if count_mode {
                    // Only the run's first batch issues; the rest are already
                    // covered by that call's compacted command range.
                    if plan_cursor < plan_range.end
                        && self.indirect_run_plan[plan_cursor].start as usize == i
                    {
                        draw_compacted_run(
                            &mut prepass,
                            &self.indirect_compact_buffer,
                            &self.indirect_count_buffer,
                            self.indirect_run_plan[plan_cursor],
                        );
                        plan_cursor += 1;
                    }
                } else if prepass_run.push(&self.indirect_buffer, &mut prepass, i) {
                    // absorbed into (or started) a run
                } else if frustum_cull_active {
                    let offset = (i * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64;
                    prepass.draw_indexed_indirect(&self.indirect_buffer, offset);
                } else {
                    let start = batch.mesh.index_start;
                    let end = start + batch.mesh.index_count;
                    let instances =
                        batch.instance_start..batch.instance_start + batch.instance_count;
                    prepass.draw_indexed(start..end, batch.mesh.base_vertex, instances);
                }
            }
            prepass_run.flush(&self.indirect_buffer, &mut prepass);
            draw_multimesh_depth_prepass(self, &mut prepass, multimesh_cull_active);
            drop(prepass);
            // Unified depth (sample_count == 1): depth_view aliases the
            // prepass texture, so the main pass loads the prepass result
            // directly; no copy is needed.
        }
        if let Some(ssao_pass) = self.ssao_pass.as_ref() {
            let view_proj = compute_view_proj_mat(camera, self.depth_size.0, self.depth_size.1);
            let (sample_count, radius_px, strength, depth_sigma) = match self.ssao_quality {
                crate::SsaoQuality::Low => (4, 8.0, 1.4, 160.0),
                crate::SsaoQuality::Medium => (8, 12.0, 2.0, 200.0),
                crate::SsaoQuality::High => (12, 16.0, 2.3, 240.0),
                crate::SsaoQuality::Ultra => (16, 20.0, 2.6, 280.0),
                crate::SsaoQuality::Off => (0, 0.0, 0.0, 0.0),
            };
            ssao_pass.encode(
                queue,
                encoder,
                ssao::SsaoUniform {
                    inv_view_proj: view_proj.inverse().to_cols_array_2d(),
                    full_size: [self.depth_size.0 as f32, self.depth_size.1 as f32],
                    radius_px,
                    strength,
                    depth_sigma,
                    sample_count,
                    target_divisor: ssao_pass.target_divisor(),
                    _pad: 0.0,
                },
            );
        }
        // Decide the seam stage's screen region before the mask pass: both the
        // mask pass and the later seam pass (+ its full-res scene copy) are
        // pure work for the seam shader, so an all-off-screen source set drops
        // every one of them.
        self.update_mesh_blend_seam_region(camera);
        if depth_prepass_active {
            self.encode_mesh_blend_mask_pass(encoder, frustum_cull_active);
        }
        if mesh_blend_depth_active {
            self.pass_counters.render_passes += 1;
            let mut blend_prepass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_mesh_blend_depth_pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.mesh_blend_depth_view,
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
            let mut current_state: Option<(RenderPath3D, bool, bool)> = None;
            blend_prepass.set_vertex_buffer(1, self.instance_transform_buffer.slice(..));
            // Same count-mode split as the depth prepass (own run plan, own
            // compacted-buffer region).
            let count_mode = self.multi_draw_indirect_count_blend_depth_active;
            let mut blend_prepass_run = IndirectRunBuilder::new(
                frustum_cull_active && self.multi_draw_indirect_enabled && !count_mode,
            );
            let plan_range = self.indirect_run_plan_blend_depth.clone();
            let mut plan_cursor = plan_range.start;
            for &i in &self.mesh_blend_depth_batch_indices {
                let batch = &self.draw_batches[i];
                let state = (batch.path, batch.double_sided, batch.packed_lod);
                if current_state != Some(state) {
                    blend_prepass_run.flush(&self.indirect_buffer, &mut blend_prepass);
                    let (camera_bg, vertex_buf, pipeline) = if batch.path == RenderPath3D::Rigid {
                        let p = if batch.double_sided {
                            if batch.packed_lod {
                                &self.pipelines.depth_prepass_rigid_packed_lod().double_sided
                            } else {
                                &self.pipelines.depth_prepass_rigid().double_sided
                            }
                        } else {
                            if batch.packed_lod {
                                &self.pipelines.depth_prepass_rigid_packed_lod().culled
                            } else {
                                &self.pipelines.depth_prepass_rigid().culled
                            }
                        };
                        let v = if batch.packed_lod {
                            &self.packed_lod_vertex_buffer
                        } else {
                            &self.rigid_vertex_buffer
                        };
                        (&self.rigid_camera_bind_group, v, p)
                    } else {
                        let p = if batch.double_sided {
                            &self.pipelines.depth_prepass_skinned().double_sided
                        } else {
                            &self.pipelines.depth_prepass_skinned().culled
                        };
                        (&self.camera_bind_group, &self.vertex_buffer, p)
                    };
                    blend_prepass.set_bind_group(0, camera_bg, &[]);
                    if batch.packed_lod {
                        blend_prepass.set_index_buffer(
                            self.packed_lod_index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                    } else {
                        blend_prepass.set_index_buffer(
                            self.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                    }
                    blend_prepass.set_vertex_buffer(0, vertex_buf.slice(..));
                    if batch.path == RenderPath3D::Skinned {
                        blend_prepass
                            .set_vertex_buffer(2, self.skinned_instance_meta_buffer.slice(..));
                    } else {
                        blend_prepass
                            .set_vertex_buffer(2, self.rigid_instance_meta_buffer.slice(..));
                    }
                    blend_prepass.set_pipeline(pipeline);
                    current_state = Some(state);
                }
                if count_mode {
                    if plan_cursor < plan_range.end
                        && self.indirect_run_plan[plan_cursor].start as usize == i
                    {
                        draw_compacted_run(
                            &mut blend_prepass,
                            &self.indirect_compact_buffer,
                            &self.indirect_count_buffer,
                            self.indirect_run_plan[plan_cursor],
                        );
                        plan_cursor += 1;
                    }
                } else if blend_prepass_run.push(&self.indirect_buffer, &mut blend_prepass, i) {
                    // absorbed into (or started) a run
                } else if frustum_cull_active {
                    let offset = (i * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64;
                    blend_prepass.draw_indexed_indirect(&self.indirect_buffer, offset);
                } else {
                    let start = batch.mesh.index_start;
                    let end = start + batch.mesh.index_count;
                    let instances =
                        batch.instance_start..batch.instance_start + batch.instance_count;
                    blend_prepass.draw_indexed(start..end, batch.mesh.base_vertex, instances);
                }
            }
            blend_prepass_run.flush(&self.indirect_buffer, &mut blend_prepass);
            drop(blend_prepass);
        }
        if hiz_active {
            self.build_hiz_from_depth(encoder);

            let mut cull_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perro_hiz_occlusion_cull_pass"),
                timestamp_writes: None,
            });
            cull_pass.set_pipeline(&self.hiz_cull_pipeline);
            cull_pass.set_bind_group(0, &self.hiz_cull_bind_group, &[]);
            let groups = (self.draw_batches.len() as u32).div_ceil(FRUSTUM_CULL_WORKGROUP_SIZE);
            cull_pass.dispatch_workgroups(groups, 1, 1);
            drop(cull_pass);

            // Multimesh hi-z phase: the prepass drew the frustum-only survivors
            // (feeding the pyramid); recompact the SAME visible-index/indirect
            // buffers with frustum + hi-z so the main pass skips occluded
            // instances. Safe to overwrite: the prepass already executed.
            if multimesh_cull_active {
                let counter_bytes =
                    (self.multimesh_batches.len() * std::mem::size_of::<u32>()) as u64;
                encoder.clear_buffer(&self.multimesh_cull_counter_buffer, 0, Some(counter_bytes));
                let mut cull_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("perro_multimesh_cull_hiz_pass"),
                    timestamp_writes: None,
                });
                cull_pass.set_bind_group(0, &self.multimesh_cull_bind_group, &[]);
                cull_pass.set_pipeline(&self.multimesh_cull_hiz_pipeline);
                let instance_groups = (self.staged_multimesh_instances.len() as u32)
                    .div_ceil(FRUSTUM_CULL_WORKGROUP_SIZE);
                cull_pass.dispatch_workgroups(instance_groups, 1, 1);
                cull_pass.set_pipeline(&self.multimesh_cull_finalize_pipeline);
                let batch_groups =
                    (self.multimesh_batches.len() as u32).div_ceil(FRUSTUM_CULL_WORKGROUP_SIZE);
                cull_pass.dispatch_workgroups(batch_groups, 1, 1);
            }

            if HIZ_DEBUG_READBACK_ENABLED
                && self.pending_hiz_debug_count == 0
                && self.pending_hiz_debug_map_rx.is_none()
                && let Some(readback_buffer) = self.hiz_debug_readback_buffer.as_ref()
            {
                let count = self.draw_batches.len() as u32;
                if count > 0 {
                    let byte_len =
                        u64::from(count) * std::mem::size_of::<DrawIndexedIndirectGpu>() as u64;
                    encoder.copy_buffer_to_buffer(
                        &self.indirect_buffer,
                        0,
                        readback_buffer,
                        0,
                        byte_len,
                    );
                    self.pending_hiz_debug_count = count;
                    self.pending_hiz_debug_frustum_visible_est = self.debug_frustum_visible_est;
                }
            }
        }
        // Hi-z rewrote the indirect buffer after the early compaction, so the
        // main pass's runs are compacted again here - after every cull pass has
        // written instance_count, before the main pass reads the result.
        if hiz_active {
            let main_runs = self.indirect_run_plan_main.clone();
            self.encode_indirect_compaction(
                queue,
                encoder,
                INDIRECT_COMPACT_DISPATCH_MAIN,
                main_runs,
            );
        }
        // Sky used to own a fullscreen pass of its own that cleared the color
        // target; the mesh pass then reloaded it. It now draws as the last
        // opaque draw *inside* the mesh pass (far plane, LessEqual, no depth
        // write), so the mesh pass owns the Clear: one pass less per frame, and
        // early-z kills every sky fragment sitting behind opaque geometry
        // instead of shading it and overdrawing it. Nothing else can leave the
        // color target unwritten - `has_any_work` is true whenever `render_sky`
        // is, so the mesh pass below always runs and always draws the sky.
        self.pass_counters.sky_draws = u32::from(render_sky);
        // Bound here (not earlier) so the shadow section can still take &mut self
        // for the per-layer cull; the query set is only read by the main pass.
        let query_set = if query_count > 0 {
            self.occlusion_query_set.as_ref()
        } else {
            None
        };
        self.pass_counters.render_passes += 1;
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_mesh_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: if self.unified_depth_active {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(1.0)
                    },
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: query_set,
            multiview_mask: None,
        });
        if self.draw_batches.is_empty() && self.multimesh_batches.is_empty() {
            if render_sky {
                draw_sky(self, &mut pass);
            }
            drop(pass);
        } else {
            if !self.draw_batches.is_empty() {
                let Some(fallback_material_bind_group) =
                    self.fallback_material_texture_bind_group()
                else {
                    // No fallback material: nothing can draw, but the sky still
                    // has to fill the frame (it used to have its own pass).
                    if render_sky {
                        draw_sky(self, &mut pass);
                    }
                    return;
                };
                pass.set_bind_group(1, fallback_material_bind_group, &[]);
                pass.set_bind_group(2, &self.shadow_bind_group, &[]);
                pass.set_bind_group(3, &self.ibl_bind_group, &[]);
            }
            let mut current_state_key = None;
            let mut current_texture_key = MaterialTextureKey::empty();
            // Local counters: `pass` holds a shared borrow of self for the
            // multimesh draw, so self.perf_counters can't be written here.
            let mut pipeline_switches: u32 = 0;
            let mut camera_bind_group_switches: u32 = 0;
            let mut texture_bind_group_switches: u32 = 0;
            // Vertex buffer 1 is the same instance-transform buffer for every
            // batch; set once here and re-set only after the multimesh draw
            // (which binds its own instance buffer to slot 1).
            pass.set_vertex_buffer(1, self.instance_transform_buffer.slice(..));
            for (group_index, batch_indices) in [
                &self.opaque_batch_indices,
                &self.alpha_batch_indices,
                &self.overlay_batch_indices,
            ]
            .into_iter()
            .enumerate()
            {
                // Pending multi_draw run. Only used when frustum cull writes the
                // indirect buffer and the MULTI_DRAW_INDIRECT feature is
                // available. Consecutive batches sharing pipeline/index/vertex/
                // texture state (guaranteed contiguous in draw_batches by the
                // sort) coalesce into one call.
                // Count mode replaces the coalescing builder entirely: the runs
                // were planned (and GPU-compacted) up front, so the builder is
                // left disabled and each run is issued at its first batch.
                let count_mode = self.multi_draw_indirect_count_active;
                let multi_draw =
                    frustum_cull_active && self.multi_draw_indirect_enabled && !count_mode;
                let mut run = IndirectRunBuilder::new(multi_draw);
                let plan_range = self.indirect_run_plan_groups[group_index].clone();
                let mut plan_cursor = plan_range.start;
                for &i in batch_indices.iter() {
                    let batch = &self.draw_batches[i];
                    let state_change = current_state_key != Some(batch.state_key);
                    let texture_change = current_texture_key != batch.material_texture_key;
                    // Any state/texture switch or query batch ends the current run.
                    if state_change || texture_change || batch.occlusion_query.is_some() {
                        run.flush(&self.indirect_buffer, &mut pass);
                    }
                    if state_change {
                        let pipeline = self.pipeline_for_batch(batch);
                        pass.set_pipeline(pipeline);
                        pipeline_switches = pipeline_switches.saturating_add(1);
                        if batch.path == RenderPath3D::Rigid {
                            pass.set_bind_group(0, &self.rigid_camera_bind_group, &[]);
                            if batch.packed_lod {
                                pass.set_index_buffer(
                                    self.packed_lod_index_buffer.slice(..),
                                    wgpu::IndexFormat::Uint32,
                                );
                                pass.set_vertex_buffer(0, self.packed_lod_vertex_buffer.slice(..));
                            } else {
                                pass.set_index_buffer(
                                    self.index_buffer.slice(..),
                                    wgpu::IndexFormat::Uint32,
                                );
                                pass.set_vertex_buffer(0, self.rigid_vertex_buffer.slice(..));
                            }
                            pass.set_vertex_buffer(2, self.rigid_instance_meta_buffer.slice(..));
                        } else {
                            pass.set_bind_group(0, &self.camera_bind_group, &[]);
                            pass.set_index_buffer(
                                self.index_buffer.slice(..),
                                wgpu::IndexFormat::Uint32,
                            );
                            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                            pass.set_vertex_buffer(2, self.skinned_instance_meta_buffer.slice(..));
                        }
                        camera_bind_group_switches = camera_bind_group_switches.saturating_add(1);
                        current_state_key = Some(batch.state_key);
                    }
                    if texture_change {
                        let Some(material_bind_group) =
                            self.material_texture_set_bind_group(batch.material_texture_key)
                        else {
                            continue;
                        };
                        pass.set_bind_group(1, material_bind_group, &[]);
                        current_texture_key = batch.material_texture_key;
                        texture_bind_group_switches = texture_bind_group_switches.saturating_add(1);
                    }
                    if let Some(query_index) = batch.occlusion_query {
                        pass.begin_occlusion_query(query_index);
                        if frustum_cull_active {
                            let offset = (i * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64;
                            pass.draw_indexed_indirect(&self.indirect_buffer, offset);
                        } else {
                            let start = batch.mesh.index_start;
                            let end = start + batch.mesh.index_count;
                            let instances =
                                batch.instance_start..batch.instance_start + batch.instance_count;
                            pass.draw_indexed(start..end, batch.mesh.base_vertex, instances);
                        }
                        pass.end_occlusion_query();
                    } else if count_mode {
                        // Only the run's first batch issues; the rest are already
                        // covered by that call's compacted command range.
                        if plan_cursor < plan_range.end
                            && self.indirect_run_plan[plan_cursor].start as usize == i
                        {
                            draw_compacted_run(
                                &mut pass,
                                &self.indirect_compact_buffer,
                                &self.indirect_count_buffer,
                                self.indirect_run_plan[plan_cursor],
                            );
                            plan_cursor += 1;
                        }
                    } else if run.push(&self.indirect_buffer, &mut pass, i) {
                        // absorbed into (or started) a run
                    } else if frustum_cull_active {
                        let offset = (i * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64;
                        pass.draw_indexed_indirect(&self.indirect_buffer, offset);
                    } else {
                        let start = batch.mesh.index_start;
                        let end = start + batch.mesh.index_count;
                        let instances =
                            batch.instance_start..batch.instance_start + batch.instance_count;
                        pass.draw_indexed(start..end, batch.mesh.base_vertex, instances);
                    }
                }
                run.flush(&self.indirect_buffer, &mut pass);
                current_state_key = None;
                current_texture_key = MaterialTextureKey::empty();
                if group_index == 0 {
                    draw_multimesh_batches(self, &mut pass);
                    // Sky goes between the opaque groups and the alpha /
                    // overlay groups: after it, depth-covered pixels are gone
                    // and transparents still composite over it exactly as they
                    // did when sky owned the clear.
                    if render_sky {
                        draw_sky(self, &mut pass);
                        // The sky layout only declares group 0, so groups 1..3
                        // are dropped when the next mesh pipeline binds. Group 0
                        // comes back with the next batch's state change; 1..3
                        // are restored here. Group 1 is restored to the fallback
                        // rather than left to the next batch's texture change,
                        // which does not fire for a batch whose texture key
                        // already equals the reset key.
                        if !self.draw_batches.is_empty()
                            && let Some(fallback) = self.fallback_material_texture_bind_group()
                        {
                            pass.set_bind_group(1, fallback, &[]);
                            pass.set_bind_group(2, &self.shadow_bind_group, &[]);
                            pass.set_bind_group(3, &self.ibl_bind_group, &[]);
                        }
                    }
                    // Restore slot 1 after the multimesh draw rebinds it.
                    pass.set_vertex_buffer(1, self.instance_transform_buffer.slice(..));
                }
            }
            drop(pass);
            self.perf_counters.pipeline_switches = self
                .perf_counters
                .pipeline_switches
                .saturating_add(pipeline_switches);
            self.perf_counters.camera_bind_group_switches = self
                .perf_counters
                .camera_bind_group_switches
                .saturating_add(camera_bind_group_switches);
            self.perf_counters.texture_bind_group_switches = self
                .perf_counters
                .texture_bind_group_switches
                .saturating_add(texture_bind_group_switches);
        }

        // Indexed (not iterated by reference) so the per-source pass counters
        // can be written between the two passes without holding a borrow of
        // `mesh_blend_source_receivers` across them.
        for source_slot in 0..self.mesh_blend_source_receivers.len() {
            let (source_i, receiver_range) = self.mesh_blend_source_receivers[source_slot].clone();
            self.pass_counters.render_passes += 2;
            let source_batch = &self.draw_batches[source_i];
            let mut blend_prepass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_mesh_blend_source_depth_pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.mesh_blend_depth_view,
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
            let mut current_state: Option<(RenderPath3D, bool, bool)> = None;
            blend_prepass.set_vertex_buffer(1, self.instance_transform_buffer.slice(..));
            for &target_i in &self.mesh_blend_receiver_indices[receiver_range] {
                let target_batch = &self.draw_batches[target_i];
                let state = (
                    target_batch.path,
                    target_batch.double_sided,
                    target_batch.packed_lod,
                );
                if current_state != Some(state) {
                    let (camera_bg, vertex_buf, pipeline) =
                        if target_batch.path == RenderPath3D::Rigid {
                            let p = if target_batch.double_sided {
                                if target_batch.packed_lod {
                                    &self.pipelines.depth_prepass_rigid_packed_lod().double_sided
                                } else {
                                    &self.pipelines.depth_prepass_rigid().double_sided
                                }
                            } else {
                                if target_batch.packed_lod {
                                    &self.pipelines.depth_prepass_rigid_packed_lod().culled
                                } else {
                                    &self.pipelines.depth_prepass_rigid().culled
                                }
                            };
                            let v = if target_batch.packed_lod {
                                &self.packed_lod_vertex_buffer
                            } else {
                                &self.rigid_vertex_buffer
                            };
                            (&self.rigid_camera_bind_group, v, p)
                        } else {
                            let p = if target_batch.double_sided {
                                &self.pipelines.depth_prepass_skinned().double_sided
                            } else {
                                &self.pipelines.depth_prepass_skinned().culled
                            };
                            (&self.camera_bind_group, &self.vertex_buffer, p)
                        };
                    blend_prepass.set_bind_group(0, camera_bg, &[]);
                    if target_batch.packed_lod {
                        blend_prepass.set_index_buffer(
                            self.packed_lod_index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                    } else {
                        blend_prepass.set_index_buffer(
                            self.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                    }
                    blend_prepass.set_vertex_buffer(0, vertex_buf.slice(..));
                    if target_batch.path == RenderPath3D::Skinned {
                        blend_prepass
                            .set_vertex_buffer(2, self.skinned_instance_meta_buffer.slice(..));
                    } else {
                        blend_prepass
                            .set_vertex_buffer(2, self.rigid_instance_meta_buffer.slice(..));
                    }
                    blend_prepass.set_pipeline(pipeline);
                    current_state = Some(state);
                }
                if frustum_cull_active {
                    let offset = (target_i * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64;
                    blend_prepass.draw_indexed_indirect(&self.indirect_buffer, offset);
                } else {
                    let start = target_batch.mesh.index_start;
                    let end = start + target_batch.mesh.index_count;
                    let instances = target_batch.instance_start
                        ..target_batch.instance_start + target_batch.instance_count;
                    blend_prepass.draw_indexed(
                        start..end,
                        target_batch.mesh.base_vertex,
                        instances,
                    );
                }
            }
            drop(blend_prepass);

            let mut blend_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_mesh_blend_source_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let Some(material_bind_group) =
                self.material_texture_set_bind_group(source_batch.material_texture_key)
            else {
                return;
            };
            blend_pass.set_bind_group(1, material_bind_group, &[]);
            blend_pass.set_bind_group(2, &self.shadow_bind_group, &[]);
            blend_pass.set_bind_group(3, &self.ibl_bind_group, &[]);
            blend_pass.set_pipeline(self.pipeline_for_batch(source_batch));
            if source_batch.path == RenderPath3D::Rigid {
                blend_pass.set_bind_group(0, &self.rigid_camera_bind_group, &[]);
                if source_batch.packed_lod {
                    blend_pass.set_index_buffer(
                        self.packed_lod_index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                    blend_pass.set_vertex_buffer(0, self.packed_lod_vertex_buffer.slice(..));
                } else {
                    blend_pass
                        .set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    blend_pass.set_vertex_buffer(0, self.rigid_vertex_buffer.slice(..));
                }
                blend_pass.set_vertex_buffer(2, self.rigid_instance_meta_buffer.slice(..));
            } else {
                blend_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                blend_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                blend_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                blend_pass.set_vertex_buffer(2, self.skinned_instance_meta_buffer.slice(..));
            }
            blend_pass.set_vertex_buffer(1, self.instance_transform_buffer.slice(..));
            if frustum_cull_active {
                let offset = (source_i * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64;
                blend_pass.draw_indexed_indirect(&self.indirect_buffer, offset);
            } else {
                let start = source_batch.mesh.index_start;
                let end = start + source_batch.mesh.index_count;
                let instances = source_batch.instance_start
                    ..source_batch.instance_start + source_batch.instance_count;
                blend_pass.draw_indexed(start..end, source_batch.mesh.base_vertex, instances);
            }
            drop(blend_pass);
        }

        if query_count > 0
            && let (Some(query_set), Some(resolve), Some(readback)) = (
                self.occlusion_query_set.as_ref(),
                self.occlusion_resolve_buffer.as_ref(),
                self.occlusion_readback_buffer.as_ref(),
            )
        {
            let byte_len = u64::from(query_count) * 8;
            encoder.resolve_query_set(query_set, 0..query_count, resolve, 0);
            encoder.copy_buffer_to_buffer(resolve, 0, readback, 0, byte_len);

            self.pending_occlusion_query_count = query_count;
            self.pending_occlusion_query_keys.clear();
            self.pending_occlusion_query_keys
                .extend(self.occlusion_query_keys_this_frame.iter().copied());
        }
    }

    pub fn depth_view(&self) -> &wgpu::TextureView {
        &self.depth_view
    }

    pub fn depth_prepass_view(&self) -> &wgpu::TextureView {
        &self.depth_prepass_view
    }

    /// Identity of the current depth-prepass view. Changes only when the view
    /// is recreated; never 0, so callers can use 0 for "no 3D depth".
    pub fn depth_prepass_view_generation(&self) -> u64 {
        self.depth_prepass_view_generation
    }

    /// Depth attachment for the 3D water pass. Water samples scene depth (the
    /// prepass view) while depth-testing; at sample_count == 1 the scene depth
    /// target aliases the prepass texture, and one pass cannot both sample and
    /// attach the same texture. Water therefore depth-tests against a lazily
    /// allocated copy of the scene depth, refreshed here each frame it is
    /// requested. MSAA returns the separate main depth target directly.
    ///
    /// Attaching the prepass view read-only instead (WebGPU allows sampling a
    /// depth texture bound with `depth_ops: None` in the same pass) would drop
    /// the copy, but it also forces `depth_write_enabled: false` on everything
    /// in the pass - and the water surface relies on its writes: the 3D water
    /// pipeline is alpha-blended with a top-surface alpha that reaches 1.0, and
    /// the wave-displaced chunk grid is drawn unsorted, so without a depth
    /// buffer a far crest overwrites a near one at grazing angles. The
    /// flip-splash pass that follows also depth-tests (LessEqual, writes off)
    /// against those writes to sit behind the surface. The copy stays until the
    /// surface gets a private depth target plus a shader-side scene-depth
    /// reject; `PassCounters::water_depth_copies` tracks the cost.
    pub fn water_depth_attachment(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
    ) -> wgpu::TextureView {
        if self.sample_count > 1 {
            return self.depth_view.clone();
        }
        let (width, height) = self.depth_size;
        let (width, height) = (width.max(1), height.max(1));
        let stale = self
            .water_scene_depth
            .as_ref()
            .map(|(texture, _)| texture.width() != width || texture.height() != height)
            .unwrap_or(true);
        if stale {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("perro_water_scene_depth"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_PREPASS_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.water_scene_depth = Some((texture, view));
        }
        let (texture, view) = self
            .water_scene_depth
            .as_ref()
            .expect("water scene depth allocated above");
        encoder.copy_texture_to_texture(
            self.depth_prepass_texture.as_image_copy(),
            texture.as_image_copy(),
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = view.clone();
        self.pass_counters.water_depth_copies += 1;
        view
    }

    /// Drop the lazily allocated water scene-depth copy once no 3D water
    /// renders anymore.
    pub fn release_water_depth(&mut self) {
        self.water_scene_depth = None;
    }

    pub fn camera_bind_group(&self) -> &wgpu::BindGroup {
        &self.camera_bind_group
    }

    pub fn camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.camera_bgl
    }

    pub fn water_camera_bind_group(&self) -> &wgpu::BindGroup {
        &self.water_camera_bind_group
    }

    pub fn water_camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.water_camera_bgl
    }

    // Precompute source->receiver batch lists so the per-source depth passes
    // skip the O(N) scan over all batches in render_pass.
    pub(super) fn rebuild_mesh_blend_receivers(&mut self) {
        self.rebuild_mesh_blend_receivers_gated(None);
    }

    // `dirty_spans` = Some on transform-only frames: the merged instance spans
    // prepare actually patched. Reuse the existing receiver lists when no
    // blend-relevant batch sphere moved since the last build, and recompute
    // spheres only for batches overlapping a dirty span. Conservative: any
    // structural change or any doubt falls through to a full rebuild.
    pub(super) fn rebuild_mesh_blend_receivers_gated(&mut self, dirty_spans: Option<&[Range<u32>]>) {
        // No sources => no receivers; keep everything cleared.
        if self.mesh_blend_batch_indices.is_empty() {
            self.mesh_blend_source_receivers.clear();
            self.mesh_blend_receiver_indices.clear();
            self.mesh_blend_prev_spheres.clear();
            return;
        }

        // Each batch's merged world sphere is O(instance_count); cache it once so
        // the source x target loop never recomputes a target's sphere per source.
        let mut spheres = std::mem::take(&mut self.mesh_blend_batch_spheres_scratch);
        spheres.clear();
        spheres.reserve(self.draw_batches.len());
        // A sphere reads only its own batch's instance window, so on a
        // transform-only frame every batch outside the patched spans keeps last
        // frame's value: O(dirty instances) instead of O(all instances).
        let reuse =
            dirty_spans.filter(|_| self.mesh_blend_prev_spheres.len() == self.draw_batches.len());
        match reuse {
            Some(spans) => {
                for (index, batch) in self.draw_batches.iter().enumerate() {
                    if batch_overlaps_dirty_spans(batch, spans) {
                        spheres.push(batch_world_sphere(batch, &self.staged_instance_transforms));
                    } else {
                        spheres.push(self.mesh_blend_prev_spheres[index]);
                    }
                }
            }
            None => {
                for batch in &self.draw_batches {
                    spheres.push(batch_world_sphere(batch, &self.staged_instance_transforms));
                }
            }
        }

        // Reuse the current lists when the previous snapshot is structurally
        // comparable and no source / potential-target sphere moved.
        let can_skip = dirty_spans.is_some()
            && self.mesh_blend_source_receivers.len() == self.mesh_blend_batch_indices.len()
            && self.mesh_blend_prev_spheres.len() == spheres.len()
            && !mesh_blend_relevant_sphere_changed(
                &self.draw_batches,
                &self.mesh_blend_batch_indices,
                &self.mesh_blend_prev_spheres,
                &spheres,
            );

        if !can_skip {
            let mut spans = std::mem::take(&mut self.mesh_blend_source_receivers);
            let mut receivers = std::mem::take(&mut self.mesh_blend_receiver_indices);
            spans.clear();
            receivers.clear();
            for &source_i in &self.mesh_blend_batch_indices {
                let source_batch = &self.draw_batches[source_i];
                let source_sphere = spheres[source_i];
                let start = receivers.len();
                for (target_i, target_batch) in self.draw_batches.iter().enumerate() {
                    if mesh_blend_receiver_matches(
                        source_i,
                        source_batch,
                        source_sphere,
                        target_i,
                        target_batch,
                        spheres[target_i],
                    ) {
                        receivers.push(target_i);
                    }
                }
                spans.push((source_i, start..receivers.len()));
            }
            self.mesh_blend_source_receivers = spans;
            self.mesh_blend_receiver_indices = receivers;
        }

        // Snapshot spheres for next-frame comparison, recycle the scratch.
        self.mesh_blend_prev_spheres.clear();
        self.mesh_blend_prev_spheres.extend_from_slice(&spheres);
        self.mesh_blend_batch_spheres_scratch = spheres;
    }

    // Screen region the mesh-blend seam stage has to touch this frame.
    //
    // The seam shader only rewrites a pixel when one side of the mask boundary
    // it finds is a *source* id (receiver-receiver boundaries are explicitly
    // skipped), so when every blend source is off screen the mask pass, the
    // full-res scene copy and the fullscreen seam pass all produce an identical
    // image and are dropped. When sources are visible, the union of their
    // projected rects (grown by the seam tap reach) bounds every pixel the pass
    // can change, and the copy only has to cover what those pixels sample.
    //
    // Conservative by construction: anything without a usable bound - a
    // participant crossing the camera plane, a sentinel-radius batch, an
    // instance count too dense to bound cheaply - forces the full-screen path,
    // which is exactly the old behavior.
    pub(super) fn update_mesh_blend_seam_region(&mut self, camera: &Camera3DState) {
        if !self.mesh_blend_screen_active || self.mesh_blend_mask_batch_entries.is_empty() {
            self.mesh_blend_seam_region = SeamRegion::Skip;
            return;
        }
        let (width, height) = self.depth_size;
        if width == 0 || height == 0 {
            self.mesh_blend_seam_region = SeamRegion::Full;
            return;
        }
        let view_proj = compute_view_proj_mat(camera, width, height);
        let frustum = extract_frustum_planes(view_proj);
        let mut any_visible = false;
        let mut unbounded = false;
        let mut bounds: Option<[f32; 4]> = None;
        for entry in &self.mesh_blend_mask_batch_entries {
            let sphere = match *entry {
                MeshBlendMaskEntry::Draw { batch_index, id } => {
                    if id >= super::mesh_blend_screen::RECEIVER_ID_BASE {
                        continue;
                    }
                    let batch = &self.draw_batches[batch_index];
                    if batch.instance_count > SEAM_GATE_MAX_INSTANCES {
                        None
                    } else {
                        batch_merged_world_sphere(batch, &self.staged_instance_transforms)
                    }
                }
                MeshBlendMaskEntry::MultiMesh { batch_index, id } => {
                    if id >= super::mesh_blend_screen::RECEIVER_ID_BASE {
                        continue;
                    }
                    multimesh_batch_world_sphere(
                        &self.multimesh_batches[batch_index],
                        &self.staged_multimesh_instances,
                        &self.staged_multimesh_draw_params,
                    )
                }
            };
            let Some((center, radius)) = sphere else {
                any_visible = true;
                unbounded = true;
                continue;
            };
            if !sphere_in_frustum(center, radius, &frustum) {
                continue;
            }
            any_visible = true;
            match sphere_screen_bounds(view_proj, center, radius, width, height) {
                Some(rect) => {
                    bounds = Some(match bounds {
                        Some(current) => union_bounds(current, rect),
                        None => rect,
                    });
                }
                None => unbounded = true,
            }
        }
        self.mesh_blend_seam_region = if !any_visible {
            SeamRegion::Skip
        } else if unbounded {
            SeamRegion::Full
        } else {
            match bounds {
                Some(rect) => seam_region_from_bounds(rect, width, height),
                None => SeamRegion::Full,
            }
        };
    }
}

// Instance ceiling for the CPU seam gate. Bounding a batch is O(instances), so
// anything denser skips the gate and takes the unconditional full-screen path
// rather than paying a per-frame scan.
const SEAM_GATE_MAX_INSTANCES: u32 = 4096;

// Merged world sphere over a multimesh batch's instances. Mirrors
// instance_world_sphere in multimesh_cull.wgsl (instance-local center pushed
// through the draw model row; radius scaled by the max instance axis, the draw
// scale and the model's max column scale). None when the batch is denser than
// the gate ceiling, the instance/draw ranges do not line up, or any value is
// non-finite - the caller then treats it as unbounded.
fn multimesh_batch_world_sphere(
    batch: &MultiMeshBatch,
    instances: &[MultiMeshInstanceGpu],
    draw_params: &[MultiMeshDrawParamGpu],
) -> Option<(Vec3, f32)> {
    if batch.instance_count == 0 || batch.instance_count > SEAM_GATE_MAX_INSTANCES {
        return None;
    }
    if !batch.mesh_local_radius.is_finite() || batch.mesh_local_radius >= 1.0e8 {
        return None;
    }
    let start = batch.instance_start as usize;
    let end = start.checked_add(batch.instance_count as usize)?;
    if end > instances.len() {
        return None;
    }
    let mut merged: Option<(Vec3, f32)> = None;
    for inst in &instances[start..end] {
        let draw = draw_params.get(inst.draw_id as usize)?;
        let row_0 = Vec4::from_array(draw.model_row_0);
        let row_1 = Vec4::from_array(draw.model_row_1);
        let row_2 = Vec4::from_array(draw.model_row_2);
        let local = Vec4::new(inst.position[0], inst.position[1], inst.position[2], 1.0);
        let center = Vec3::new(row_0.dot(local), row_1.dot(local), row_2.dot(local));
        let draw_scale = f32::from_bits(draw.scale_bits);
        let inst_scale = inst.scale[0].max(inst.scale[1]).max(inst.scale[2]) * draw_scale;
        let model_scale = row_0
            .truncate()
            .length_squared()
            .max(row_1.truncate().length_squared())
            .max(row_2.truncate().length_squared())
            .max(1.0e-12)
            .sqrt();
        let radius = batch.mesh_local_radius * inst_scale.max(0.0) * model_scale;
        if !center.is_finite() || !radius.is_finite() {
            return None;
        }
        merged = Some(match merged {
            Some(current) => enclose_spheres(current, (center, radius)),
            None => (center, radius),
        });
    }
    merged.filter(|(center, radius)| center.is_finite() && radius.is_finite() && *radius < 1.0e8)
}

// Pixel-space bounds [min_x, min_y, max_x, max_y] of a world sphere, from the
// eight corners of its AABB. None when any corner sits on or behind the camera
// plane: the perspective divide folds there and a 2D rect stops bounding the
// projection.
fn sphere_screen_bounds(
    view_proj: Mat4,
    center: Vec3,
    radius: f32,
    width: u32,
    height: u32,
) -> Option<[f32; 4]> {
    let r = radius.max(0.0);
    let (w, h) = (width as f32, height as f32);
    let mut out = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for corner in 0..8u32 {
        let offset = Vec3::new(
            if corner & 1 == 0 { -r } else { r },
            if corner & 2 == 0 { -r } else { r },
            if corner & 4 == 0 { -r } else { r },
        );
        let clip = view_proj * (center + offset).extend(1.0);
        if !clip.is_finite() || clip.w <= 1.0e-4 {
            return None;
        }
        let ndc_x = clip.x / clip.w;
        let ndc_y = clip.y / clip.w;
        // Screen y runs down; the seam shader uses the same flip when it turns
        // a pixel coord back into NDC.
        let px = (ndc_x * 0.5 + 0.5) * w;
        let py = (0.5 - ndc_y * 0.5) * h;
        if !px.is_finite() || !py.is_finite() {
            return None;
        }
        out[0] = out[0].min(px);
        out[1] = out[1].min(py);
        out[2] = out[2].max(px);
        out[3] = out[3].max(py);
    }
    Some(out)
}

#[inline]
fn union_bounds(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].max(b[2]),
        a[3].max(b[3]),
    ]
}

// Grow the source bounds by the seam tap reach (every pixel the seam pass can
// rewrite lies within one tap ring of a source pixel), snap outwards to whole
// pixels and clamp to the target.
fn seam_region_from_bounds(bounds: [f32; 4], width: u32, height: u32) -> SeamRegion {
    let reach = super::mesh_blend_screen::SEAM_TAP_REACH_PX as f32;
    let min_x = (bounds[0] - reach).floor();
    let min_y = (bounds[1] - reach).floor();
    let max_x = (bounds[2] + reach).ceil();
    let max_y = (bounds[3] + reach).ceil();
    if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
        return SeamRegion::Full;
    }
    let x0 = min_x.clamp(0.0, width as f32) as u32;
    let y0 = min_y.clamp(0.0, height as f32) as u32;
    let x1 = max_x.clamp(0.0, width as f32) as u32;
    let y1 = max_y.clamp(0.0, height as f32) as u32;
    if x1 <= x0 || y1 <= y0 {
        // Sphere passed the frustum test but bounds nothing on screen; take the
        // safe path rather than trusting the degenerate rect.
        return SeamRegion::Full;
    }
    if x0 == 0 && y0 == 0 && x1 == width && y1 == height {
        return SeamRegion::Full;
    }
    SeamRegion::Rect(x0, y0, x1 - x0, y1 - y0)
}

// A batch is blend-relevant if it can be a mesh-blend source or a valid receiver
// target; only such a batch's movement can change any receiver list. Anything
// uncertain is treated as relevant (=> rebuild). draw_on_top / alpha batches that
// are not sources never participate, so their movement can be ignored.

#[path = "render_pass/mesh_blend.rs"]
mod mesh_blend;
use mesh_blend::*;
#[path = "render_pass/shadows.rs"]
mod shadows;
use shadows::*;

#[cfg(test)]
#[path = "../../../tests/unit/three_d_render_pass_gpu_tests.rs"]
mod render_pass_gpu_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use perro_graphics_assets::MeshRange;
    use perro_structs::BitMask;

    fn frustum_at(center: [f32; 3]) -> [Vec4; 6] {
        let eye = Vec3::new(center[0], center[1], center[2] + 20.0);
        let view = Mat4::look_at_rh(eye, Vec3::from(center), Vec3::Y);
        let proj = Mat4::perspective_rh(0.6, 1.0, 0.5, 100.0);
        extract_frustum_planes(proj * view)
    }

    fn translated_instance(pos: [f32; 3]) -> TransformInstanceGpu {
        TransformInstanceGpu {
            model_row_0: [1.0, 0.0, 0.0, pos[0]],
            model_row_1: [0.0, 1.0, 0.0, pos[1]],
            model_row_2: [0.0, 0.0, 1.0, pos[2]],
        }
    }

    pub(super) fn test_batch(
        instance_start: u32,
        instance_count: u32,
        local_radius: f32,
    ) -> DrawBatch {
        let material_kind = MaterialPipelineKind::Standard;
        let state_key =
            draw_batch_state_key(RenderPath3D::Rigid, false, false, 0, false, &material_kind);
        let material_texture_key = MaterialTextureKey::from_base(0);
        DrawBatch {
            state_key,
            render_state: render_state_key(
                state_key,
                material_texture_key.state_hash(),
                0,
                0,
                false,
                0,
                false,
            ),
            mesh: MeshRange {
                index_start: 0,
                index_count: 3,
                base_vertex: 0,
            },
            instance_start,
            instance_count,
            path: RenderPath3D::Rigid,
            packed_lod: false,
            double_sided: false,
            material_kind,
            alpha_mode: 0,
            draw_on_top: false,
            base_color_texture_slot: 0,
            material_texture_key,
            local_center: [0.0, 0.0, 0.0],
            local_radius,
            occlusion_query: None,
            disable_hiz_occlusion: false,
            casts_shadows: true,
            receives_shadows: true,
            mesh_blend: false,
            mesh_blend_screen: false,
            mesh_blend_params: 0,
            mesh_blend_params_ext: 0,
            mesh_blend_depth: false,
            blend_layers: BitMask::ALL.bits(),
            blend_mask: BitMask::NONE.bits(),
            order_index: 0,
        }
    }

    #[test]
    fn shadow_layer_cull_drops_off_view_single_instance_batches() {
        // In-view caster at origin, far off-view caster at x=+80.
        let batches = [test_batch(0, 1, 1.0), test_batch(1, 1, 1.0)];
        let transforms = [
            translated_instance([0.0, 0.0, 0.0]),
            translated_instance([80.0, 0.0, 0.0]),
        ];
        let frustum = frustum_at([0.0, 0.0, 0.0]);
        let indices = [0usize, 1usize];
        let mut out = Vec::new();
        shadow_layer_cull(&indices, &batches, &transforms, &frustum, &mut out);
        assert_eq!(out, vec![0], "off-view caster culled, order preserved");
    }

    #[test]
    fn shadow_layer_cull_keeps_multi_instance_batches_conservatively() {
        // Sentinel local_radius (debug batches) has no tight sphere -> keep.
        let batches = [test_batch(0, 4, 1.0e9)];
        let transforms = [translated_instance([80.0, 0.0, 0.0])];
        let frustum = frustum_at([0.0, 0.0, 0.0]);
        let indices = [0usize];
        let mut out = Vec::new();
        shadow_layer_cull(&indices, &batches, &transforms, &frustum, &mut out);
        assert_eq!(out, vec![0], "unbounded multi-instance batch survives");
    }

    #[test]
    fn shadow_layer_cull_uses_merged_sphere_for_multi_instance_batches() {
        // Batch 0: both instances far off-view -> merged sphere culled.
        // Batch 1: one instance in view -> merged sphere survives.
        let batches = [test_batch(0, 2, 1.0), test_batch(2, 2, 1.0)];
        let transforms = [
            translated_instance([80.0, 0.0, 0.0]),
            translated_instance([90.0, 0.0, 0.0]),
            translated_instance([0.0, 0.0, 0.0]),
            translated_instance([85.0, 0.0, 0.0]),
        ];
        let frustum = frustum_at([0.0, 0.0, 0.0]);
        let indices = [0usize, 1usize];
        let mut out = Vec::new();
        shadow_layer_cull(&indices, &batches, &transforms, &frustum, &mut out);
        assert_eq!(out, vec![1], "fully off-view multi-instance batch culled");
    }

    #[test]
    fn shadow_layer_cull_emits_no_wasted_commands_to_compact() {
        // Why the indirect-count path does not extend to the shadow passes: the
        // per-layer cull runs on the CPU and draw_shadow_batches issues one
        // direct draw_indexed per surviving batch, so a 70%-culled layer already
        // submits exactly the survivors. There is no culled-to-zero slot for a
        // compaction pass to remove - max_count == count by construction.
        let total = 512usize;
        let visible = |i: usize| i % 10 >= 7;
        let batches: Vec<DrawBatch> = (0..total).map(|i| test_batch(i as u32, 1, 1.0)).collect();
        let off_view = [80.0, 0.0, 0.0];
        let transforms: Vec<TransformInstanceGpu> = (0..total)
            .map(|i| translated_instance(if visible(i) { [0.0; 3] } else { off_view }))
            .collect();
        let indices: Vec<usize> = (0..total).collect();
        let mut out = Vec::new();
        shadow_layer_cull(
            &indices,
            &batches,
            &transforms,
            &frustum_at([0.0, 0.0, 0.0]),
            &mut out,
        );
        let expected: Vec<usize> = indices.iter().copied().filter(|&i| visible(i)).collect();
        assert_eq!(out, expected, "survivors only, in draw order");
        assert_eq!(out.len(), 153, "70% culled");
    }

    #[test]
    fn batch_overlaps_dirty_spans_matches_only_touched_instance_windows() {
        // Sorted + disjoint patched spans; batch windows are not monotonic.
        let spans = [4u32..6, 10..12];
        assert!(!batch_overlaps_dirty_spans(&test_batch(0, 4, 1.0), &spans));
        assert!(batch_overlaps_dirty_spans(&test_batch(3, 2, 1.0), &spans));
        assert!(batch_overlaps_dirty_spans(&test_batch(5, 1, 1.0), &spans));
        assert!(!batch_overlaps_dirty_spans(&test_batch(6, 4, 1.0), &spans));
        assert!(batch_overlaps_dirty_spans(&test_batch(6, 5, 1.0), &spans));
        assert!(!batch_overlaps_dirty_spans(&test_batch(12, 8, 1.0), &spans));
        // Empty windows / no patched spans never match.
        assert!(!batch_overlaps_dirty_spans(&test_batch(4, 0, 1.0), &spans));
        assert!(!batch_overlaps_dirty_spans(&test_batch(0, 99, 1.0), &[]));
    }

    #[test]
    fn sphere_in_frustum_accepts_center_and_rejects_far() {
        let frustum = frustum_at([0.0, 0.0, 0.0]);
        assert!(sphere_in_frustum(Vec3::ZERO, 1.0, &frustum));
        assert!(!sphere_in_frustum(
            Vec3::new(200.0, 0.0, 0.0),
            1.0,
            &frustum
        ));
        // Large radius rescues a center just outside the planes.
        assert!(sphere_in_frustum(Vec3::new(30.0, 0.0, 0.0), 40.0, &frustum));
    }
}
