use super::*;

impl Gpu3D {
    pub fn render_pass(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        clear_color: wgpu::Color,
        depth_prepass_needed: bool,
    ) {
        let frustum_cull_active = self.should_run_frustum_cull();
        let hiz_active = self.should_run_hiz_occlusion(frustum_cull_active);
        let depth_prepass_active = self.should_run_depth_prepass(depth_prepass_needed, hiz_active);
        let query_count = if self.cpu_occlusion_enabled
            && self.pending_occlusion_query_count == 0
            && self.pending_occlusion_map_rx.is_none()
        {
            self.occlusion_query_keys_this_frame.len() as u32
        } else {
            0
        };
        let query_set = if query_count > 0 {
            self.occlusion_query_set.as_ref()
        } else {
            None
        };
        let has_any_work = !self.draw_batches.is_empty()
            || !self.multimesh_batches.is_empty()
            || self.sky_enabled
            || depth_prepass_active
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
            let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_shadow3d_pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_map_view,
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
            shadow_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            let mut current_state: Option<(RenderPath3D, bool)> = None;
            for batch in &self.draw_batches {
                if batch.draw_on_top || !batch.casts_shadows || batch.alpha_mode != 0 {
                    continue;
                }
                let state = (batch.path, batch.double_sided);
                if current_state != Some(state) {
                    let (camera_bg, vertex_buf, pipeline) = if batch.path == RenderPath3D::Rigid {
                        let p = if batch.double_sided {
                            &self.pipeline_shadow_depth_rigid_double_sided
                        } else {
                            &self.pipeline_shadow_depth_rigid_culled
                        };
                        (
                            &self.rigid_shadow_camera_bind_group,
                            &self.rigid_vertex_buffer,
                            p,
                        )
                    } else {
                        let p = if batch.double_sided {
                            &self.pipeline_shadow_depth_double_sided
                        } else {
                            &self.pipeline_shadow_depth_culled
                        };
                        (&self.shadow_camera_bind_group, &self.vertex_buffer, p)
                    };
                    shadow_pass.set_bind_group(0, camera_bg, &[]);
                    shadow_pass.set_vertex_buffer(0, vertex_buf.slice(..));
                    shadow_pass.set_vertex_buffer(1, self.instance_transform_buffer.slice(..));
                    if batch.path == RenderPath3D::Skinned {
                        shadow_pass
                            .set_vertex_buffer(2, self.skinned_instance_meta_buffer.slice(..));
                    }
                    shadow_pass.set_pipeline(pipeline);
                    current_state = Some(state);
                }
                let start = batch.mesh.index_start;
                let end = start + batch.mesh.index_count;
                let instances = batch.instance_start..batch.instance_start + batch.instance_count;
                shadow_pass.draw_indexed(start..end, batch.mesh.base_vertex, instances);
            }
            drop(shadow_pass);
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
            prepass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            let mut current_state: Option<(RenderPath3D, bool)> = None;
            for (i, batch) in self.draw_batches.iter().enumerate() {
                if batch.draw_on_top || batch.alpha_mode != 0 || batch.mesh_blend {
                    continue;
                }
                let state = (batch.path, batch.double_sided);
                if current_state != Some(state) {
                    let (camera_bg, vertex_buf, pipeline) = if batch.path == RenderPath3D::Rigid {
                        let p = if batch.double_sided {
                            &self.pipeline_depth_prepass_rigid_double_sided
                        } else {
                            &self.pipeline_depth_prepass_rigid_culled
                        };
                        (&self.rigid_camera_bind_group, &self.rigid_vertex_buffer, p)
                    } else {
                        let p = if batch.double_sided {
                            &self.pipeline_depth_prepass_double_sided
                        } else {
                            &self.pipeline_depth_prepass_culled
                        };
                        (&self.camera_bind_group, &self.vertex_buffer, p)
                    };
                    prepass.set_bind_group(0, camera_bg, &[]);
                    prepass.set_vertex_buffer(0, vertex_buf.slice(..));
                    prepass.set_vertex_buffer(1, self.instance_transform_buffer.slice(..));
                    if batch.path == RenderPath3D::Skinned {
                        prepass.set_vertex_buffer(2, self.skinned_instance_meta_buffer.slice(..));
                    }
                    prepass.set_pipeline(pipeline);
                    current_state = Some(state);
                }
                if frustum_cull_active {
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
            drop(prepass);
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
        if self.sky_enabled {
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
            sky_pass.set_pipeline(&self.sky_pipeline);
            sky_pass.set_bind_group(0, &self.sky_bind_group, &[]);
            sky_pass.draw(0..3, 0..1);
            drop(sky_pass);
        }
        let color_load = if self.sky_enabled {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(clear_color)
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
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: query_set,
            multiview_mask: None,
        });
        if self.draw_batches.is_empty() {
            drop(pass);
        } else {
            pass.set_bind_group(1, self.fallback_material_texture_bind_group(), &[]);
            pass.set_bind_group(2, &self.shadow_bind_group, &[]);
            pass.set_bind_group(3, &self.mesh_blend_bind_group, &[]);
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            let mut current_state_key = None;
            let mut current_texture_slot = MATERIAL_TEXTURE_NONE;
            for (i, batch) in self.draw_batches.iter().enumerate() {
                if current_state_key != Some(batch.state_key) {
                    let pipeline = self.pipeline_for_batch(batch);
                    pass.set_pipeline(pipeline);
                    if batch.path == RenderPath3D::Rigid {
                        pass.set_bind_group(0, &self.rigid_camera_bind_group, &[]);
                        pass.set_vertex_buffer(0, self.rigid_vertex_buffer.slice(..));
                        pass.set_vertex_buffer(3, self.rigid_instance_meta_buffer.slice(..));
                    } else {
                        pass.set_bind_group(0, &self.camera_bind_group, &[]);
                        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                        pass.set_vertex_buffer(3, self.skinned_instance_meta_buffer.slice(..));
                    }
                    pass.set_vertex_buffer(1, self.instance_transform_buffer.slice(..));
                    pass.set_vertex_buffer(2, self.instance_material_buffer.slice(..));
                    current_state_key = Some(batch.state_key);
                }
                if current_texture_slot != batch.base_color_texture_slot {
                    pass.set_bind_group(
                        1,
                        self.material_texture_bind_group(batch.base_color_texture_slot),
                        &[],
                    );
                    current_texture_slot = batch.base_color_texture_slot;
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
            drop(pass);
        }

        if !self.multimesh_batches.is_empty() {
            let mut mm_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_multimesh_pass"),
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
            mm_pass.set_bind_group(0, &self.multimesh_bind_group, &[]);
            mm_pass.set_vertex_buffer(0, self.rigid_vertex_buffer.slice(..));
            mm_pass.set_vertex_buffer(1, self.multimesh_instance_buffer.slice(..));
            mm_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            let mut current_state: Option<(bool, bool)> = None;
            for batch in &self.multimesh_batches {
                let state = (batch.double_sided, batch.mesh_blend);
                if current_state != Some(state) {
                    mm_pass.set_pipeline(if batch.mesh_blend && batch.double_sided {
                        &self.pipeline_multimesh_blend_double_sided
                    } else if batch.mesh_blend {
                        &self.pipeline_multimesh_blend_culled
                    } else if batch.double_sided {
                        &self.pipeline_multimesh_double_sided
                    } else {
                        &self.pipeline_multimesh_culled
                    });
                    current_state = Some(state);
                }
                let start = batch.mesh.index_start;
                let end = start + batch.mesh.index_count;
                let instances = batch.instance_start..batch.instance_start + batch.instance_count;
                mm_pass.draw_indexed(start..end, batch.mesh.base_vertex, instances);
            }
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
}
