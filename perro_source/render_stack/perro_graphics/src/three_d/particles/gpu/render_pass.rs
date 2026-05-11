use super::*;

impl GpuPointParticles3D {
    pub fn render_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
    ) {
        if self.staged.is_empty()
            && self.staged_billboards.is_empty()
            && self.hybrid_particle_count == 0
            && self.compute_particle_count == 0
        {
            return;
        }
        if self.compute_particle_count > 0 {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perro_particles3d_compute_pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &self.camera_bg, &[]);
            compute_pass.set_bind_group(1, &self.compute_bg, &[]);
            let groups = self.compute_particle_count.div_ceil(64);
            compute_pass.dispatch_workgroups(groups, 1, 1);
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_particles3d_pass"),
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
                view: depth_view,
                depth_ops: None,
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if !self.staged.is_empty() {
            pass.set_pipeline(&self.cpu_pipeline);
            pass.set_bind_group(0, &self.camera_bg, &[]);
            pass.set_vertex_buffer(0, self.particle_buffer.slice(..));
            pass.draw(0..self.staged.len() as u32, 0..1);
        }
        if !self.staged_billboards.is_empty() {
            pass.set_pipeline(&self.cpu_billboard_pipeline);
            pass.set_bind_group(0, &self.camera_bg, &[]);
            pass.set_vertex_buffer(0, self.billboard_particle_buffer.slice(..));
            pass.draw(0..4, 0..self.staged_billboards.len() as u32);
        }
        if self.hybrid_particle_count > 0 {
            if self.hybrid_has_point {
                pass.set_pipeline(&self.hybrid_pipeline);
                pass.set_bind_group(0, &self.camera_bg, &[]);
                pass.set_bind_group(1, &self.hybrid_params_bg, &[]);
                for range in &self.hybrid_point_ranges {
                    pass.draw(0..1, range.start..(range.start + range.count));
                }
            }
            if self.hybrid_has_billboard {
                pass.set_pipeline(&self.hybrid_billboard_pipeline);
                pass.set_bind_group(0, &self.camera_bg, &[]);
                pass.set_bind_group(1, &self.hybrid_params_bg, &[]);
                for range in &self.hybrid_billboard_ranges {
                    pass.draw(0..4, range.start..(range.start + range.count));
                }
            }
        }
        if self.compute_particle_count > 0 {
            if self.compute_has_point {
                pass.set_pipeline(&self.compute_render_pipeline);
                pass.set_bind_group(0, &self.camera_bg, &[]);
                pass.set_bind_group(1, &self.compute_bg, &[]);
                for range in &self.compute_point_ranges {
                    pass.draw(0..1, range.start..(range.start + range.count));
                }
            }
            if self.compute_has_billboard {
                pass.set_pipeline(&self.compute_render_billboard_pipeline);
                pass.set_bind_group(0, &self.camera_bg, &[]);
                pass.set_bind_group(1, &self.compute_bg, &[]);
                for range in &self.compute_billboard_ranges {
                    pass.draw(0..4, range.start..(range.start + range.count));
                }
            }
        }
    }
}
