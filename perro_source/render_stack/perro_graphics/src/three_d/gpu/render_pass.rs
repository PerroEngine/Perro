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
        let frustum_cull_active = self.should_run_frustum_cull();
        let hiz_active = self.should_run_hiz_occlusion(frustum_cull_active);
        let multimesh_cull_active = self.should_run_multimesh_cull();
        self.multimesh_cull_active = multimesh_cull_active;
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
        if depth_prepass_active {
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
            let mut prepass_run =
                IndirectRunBuilder::new(frustum_cull_active && self.multi_draw_indirect_enabled);
            for &i in &self.depth_prepass_batch_indices {
                let batch = &self.draw_batches[i];
                let state = (batch.path, batch.double_sided, batch.packed_lod);
                if current_state != Some(state) {
                    prepass_run.flush(&self.indirect_buffer, &mut prepass);
                    let (camera_bg, vertex_buf, pipeline) = if batch.path == RenderPath3D::Rigid {
                        let p = if batch.double_sided {
                            if batch.packed_lod {
                                &self.pipeline_depth_prepass_rigid_packed_lod_double_sided
                            } else {
                                &self.pipeline_depth_prepass_rigid_double_sided
                            }
                        } else {
                            if batch.packed_lod {
                                &self.pipeline_depth_prepass_rigid_packed_lod_culled
                            } else {
                                &self.pipeline_depth_prepass_rigid_culled
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
                            &self.pipeline_depth_prepass_double_sided
                        } else {
                            &self.pipeline_depth_prepass_culled
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
                if prepass_run.push(&self.indirect_buffer, &mut prepass, i) {
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
            if self.unified_depth_active {
                // Depth32Float allows texture-to-texture copies (Depth24Plus
                // does not); this primes depth_view so the main pass loads it.
                encoder.copy_texture_to_texture(
                    self.depth_prepass_texture.as_image_copy(),
                    self.depth_texture.as_image_copy(),
                    wgpu::Extent3d {
                        width: self.depth_size.0,
                        height: self.depth_size.1,
                        depth_or_array_layers: 1,
                    },
                );
            }
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
        if depth_prepass_active {
            self.encode_mesh_blend_mask_pass(encoder, frustum_cull_active);
        }
        if mesh_blend_depth_active {
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
            let mut blend_prepass_run =
                IndirectRunBuilder::new(frustum_cull_active && self.multi_draw_indirect_enabled);
            for &i in &self.mesh_blend_depth_batch_indices {
                let batch = &self.draw_batches[i];
                let state = (batch.path, batch.double_sided, batch.packed_lod);
                if current_state != Some(state) {
                    blend_prepass_run.flush(&self.indirect_buffer, &mut blend_prepass);
                    let (camera_bg, vertex_buf, pipeline) = if batch.path == RenderPath3D::Rigid {
                        let p = if batch.double_sided {
                            if batch.packed_lod {
                                &self.pipeline_depth_prepass_rigid_packed_lod_double_sided
                            } else {
                                &self.pipeline_depth_prepass_rigid_double_sided
                            }
                        } else {
                            if batch.packed_lod {
                                &self.pipeline_depth_prepass_rigid_packed_lod_culled
                            } else {
                                &self.pipeline_depth_prepass_rigid_culled
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
                            &self.pipeline_depth_prepass_double_sided
                        } else {
                            &self.pipeline_depth_prepass_culled
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
                if blend_prepass_run.push(&self.indirect_buffer, &mut blend_prepass, i) {
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
            {
                let count = self.draw_batches.len() as u32;
                if count > 0 {
                    let byte_len =
                        u64::from(count) * std::mem::size_of::<DrawIndexedIndirectGpu>() as u64;
                    encoder.copy_buffer_to_buffer(
                        &self.indirect_buffer,
                        0,
                        &self.hiz_debug_readback_buffer,
                        0,
                        byte_len,
                    );
                    self.pending_hiz_debug_count = count;
                    self.pending_hiz_debug_frustum_visible_est = self.debug_frustum_visible_est;
                }
            }
        }
        if render_sky {
            let mut sky_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_sky3d_pass"),
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
            let sky_pipeline = self
                .active_sky_pipeline_key
                .as_ref()
                .and_then(|key| self.custom_sky_pipelines.get(key))
                .unwrap_or(&self.sky_pipeline);
            sky_pass.set_pipeline(sky_pipeline);
            sky_pass.set_bind_group(0, &self.sky_bind_group, &[]);
            sky_pass.draw(0..3, 0..1);
            drop(sky_pass);
        }
        let color_load = if render_sky {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(clear_color)
        };
        // Bound here (not earlier) so the shadow section can still take &mut self
        // for the per-layer cull; the query set is only read by the main pass.
        let query_set = if query_count > 0 {
            self.occlusion_query_set.as_ref()
        } else {
            None
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_mesh_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: color_load,
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
            drop(pass);
        } else {
            if !self.draw_batches.is_empty() {
                let Some(fallback_material_bind_group) =
                    self.fallback_material_texture_bind_group()
                else {
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
                let multi_draw = frustum_cull_active && self.multi_draw_indirect_enabled;
                let mut run = IndirectRunBuilder::new(multi_draw);
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

        for &(source_i, ref receiver_range) in &self.mesh_blend_source_receivers {
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
            for &target_i in &self.mesh_blend_receiver_indices[receiver_range.clone()] {
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
                                    &self.pipeline_depth_prepass_rigid_packed_lod_double_sided
                                } else {
                                    &self.pipeline_depth_prepass_rigid_double_sided
                                }
                            } else {
                                if target_batch.packed_lod {
                                    &self.pipeline_depth_prepass_rigid_packed_lod_culled
                                } else {
                                    &self.pipeline_depth_prepass_rigid_culled
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
                                &self.pipeline_depth_prepass_double_sided
                            } else {
                                &self.pipeline_depth_prepass_culled
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
        self.rebuild_mesh_blend_receivers_gated(false);
    }

    // `allow_skip` (transform-only frames): reuse the existing receiver lists when
    // no blend-relevant batch sphere moved since the last build. Conservative:
    // any structural change or any doubt falls through to a full rebuild.
    pub(super) fn rebuild_mesh_blend_receivers_gated(&mut self, allow_skip: bool) {
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
        for batch in &self.draw_batches {
            spheres.push(batch_world_sphere(batch, &self.staged_instance_transforms));
        }

        // Reuse the current lists when the previous snapshot is structurally
        // comparable and no source / potential-target sphere moved.
        let can_skip = allow_skip
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

    fn test_batch(instance_start: u32, instance_count: u32, local_radius: f32) -> DrawBatch {
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
