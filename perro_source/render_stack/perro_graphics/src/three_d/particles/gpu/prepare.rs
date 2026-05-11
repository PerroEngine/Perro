use super::*;

impl GpuPointParticles3D {
    pub fn set_sample_count(
        &mut self,
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
    ) {
        *self = Self::new(device, color_format, sample_count);
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: PreparePointParticles3D<'_>,
    ) {
        self.staged.clear();
        self.staged_billboards.clear();
        self.hybrid_emitters.clear();
        self.hybrid_particle_emitter_map.clear();
        self.hybrid_spawn_origin_dirty_slots.clear();
        self.hybrid_spawn_rotation_dirty_slots.clear();
        self.hybrid_particle_count = 0;
        self.hybrid_has_point = false;
        self.hybrid_has_billboard = false;
        self.hybrid_point_ranges.clear();
        self.hybrid_billboard_ranges.clear();
        self.compute_emitters.clear();
        self.compute_particle_emitter_map.clear();
        self.compute_spawn_origin_dirty_slots.clear();
        self.compute_spawn_rotation_dirty_slots.clear();
        self.compute_particle_count = 0;
        self.compute_has_point = false;
        self.compute_has_billboard = false;
        self.compute_point_ranges.clear();
        self.compute_billboard_ranges.clear();
        self.compute_expr_ops.clear();
        self.compute_custom_params.clear();
        self.hybrid_map_fingerprint = 0xcbf2_9ce4_8422_2325;
        self.compute_map_fingerprint = 0xcbf2_9ce4_8422_2325;
        self.spawn_origin_generation = self.spawn_origin_generation.wrapping_add(1);
        if self.spawn_origin_generation == 0 {
            self.spawn_origin_generation = 1;
        }
        self.emitter_order.clear();
        self.emitter_order.extend(0..frame.emitters.len());
        self.emitter_order
            .sort_unstable_by_key(|&i| frame.emitters[i].0.as_u64());
        for order_idx in 0..self.emitter_order.len() {
            let idx = self.emitter_order[order_idx];
            let (node, emitter) = &frame.emitters[idx];
            match emitter.sim_mode {
                ParticleSimulationMode3D::Cpu => self.push_emitter_particles(*node, emitter),
                ParticleSimulationMode3D::GpuVertex => {
                    if !self.push_hybrid_emitter_particles(*node, emitter) {
                        self.push_emitter_particles(*node, emitter);
                    }
                }
                ParticleSimulationMode3D::GpuCompute => {
                    if !gpu_compute_particles_enabled() {
                        if !self.push_hybrid_emitter_particles(*node, emitter) {
                            self.push_emitter_particles(*node, emitter);
                        }
                    } else if !self.push_compute_emitter_particles(*node, emitter) {
                        self.push_emitter_particles(*node, emitter);
                    }
                }
            }
        }
        let generation = self.spawn_origin_generation;
        self.spawn_origin_cache.retain(|_, per_particle| {
            per_particle.retain(|_, entry| entry.last_seen_generation == generation);
            !per_particle.is_empty()
        });
        if self.staged.is_empty()
            && self.staged_billboards.is_empty()
            && self.hybrid_emitters.is_empty()
            && self.compute_emitters.is_empty()
        {
            return;
        }
        if !self.staged.is_empty() {
            self.ensure_particle_capacity(device, self.staged.len());
            queue.write_buffer(&self.particle_buffer, 0, bytemuck::cast_slice(&self.staged));
        }
        if !self.staged_billboards.is_empty() {
            self.ensure_billboard_particle_capacity(device, self.staged_billboards.len());
            queue.write_buffer(
                &self.billboard_particle_buffer,
                0,
                bytemuck::cast_slice(&self.staged_billboards),
            );
        }
        if !self.hybrid_emitters.is_empty() {
            let hybrid_spawn_origin_recreated = self.ensure_hybrid_emitter_capacity(
                device,
                self.hybrid_emitters.len(),
                self.hybrid_particle_count as usize,
                self.hybrid_particle_spawn_origins.len(),
            );
            queue.write_buffer(
                &self.hybrid_emitter_buffer,
                0,
                bytemuck::cast_slice(&self.hybrid_emitters),
            );
            let hybrid_map_count = self.hybrid_particle_emitter_map.len();
            let hybrid_map_dirty = hybrid_spawn_origin_recreated
                || self.hybrid_map_uploaded_count != hybrid_map_count
                || self.hybrid_map_uploaded_fingerprint != self.hybrid_map_fingerprint;
            if hybrid_map_dirty {
                queue.write_buffer(
                    &self.hybrid_particle_emitter_buffer,
                    0,
                    bytemuck::cast_slice(&self.hybrid_particle_emitter_map),
                );
                self.hybrid_map_uploaded_count = hybrid_map_count;
                self.hybrid_map_uploaded_fingerprint = self.hybrid_map_fingerprint;
            }
            if hybrid_spawn_origin_recreated {
                queue.write_buffer(
                    &self.hybrid_particle_spawn_origin_buffer,
                    0,
                    bytemuck::cast_slice(&self.hybrid_particle_spawn_origins),
                );
                queue.write_buffer(
                    &self.hybrid_particle_spawn_rotation_buffer,
                    0,
                    bytemuck::cast_slice(&self.hybrid_particle_spawn_rotations),
                );
            } else if !self.hybrid_spawn_origin_dirty_slots.is_empty() {
                write_spawn_origin_dirty_ranges(
                    queue,
                    &self.hybrid_particle_spawn_origin_buffer,
                    &self.hybrid_particle_spawn_origins,
                    &mut self.hybrid_spawn_origin_dirty_slots,
                );
                write_spawn_origin_dirty_ranges(
                    queue,
                    &self.hybrid_particle_spawn_rotation_buffer,
                    &self.hybrid_particle_spawn_rotations,
                    &mut self.hybrid_spawn_rotation_dirty_slots,
                );
            }
            let params = GpuEmitterParams {
                emitter_count: self.hybrid_emitters.len() as u32,
                particle_count: self.hybrid_particle_count,
                _pad: [0; 2],
            };
            queue.write_buffer(&self.hybrid_params_buffer, 0, bytemuck::bytes_of(&params));
        }
        if !self.compute_emitters.is_empty() {
            let compute_spawn_origin_recreated = self.ensure_compute_capacity(
                device,
                self.compute_emitters.len(),
                self.compute_particle_count as usize,
                self.compute_particle_spawn_origins.len(),
                self.compute_expr_ops.len(),
                self.compute_custom_params.len(),
            );
            queue.write_buffer(
                &self.compute_emitter_buffer,
                0,
                bytemuck::cast_slice(&self.compute_emitters),
            );
            let compute_map_count = self.compute_particle_emitter_map.len();
            let compute_map_dirty = compute_spawn_origin_recreated
                || self.compute_map_uploaded_count != compute_map_count
                || self.compute_map_uploaded_fingerprint != self.compute_map_fingerprint;
            if compute_map_dirty {
                queue.write_buffer(
                    &self.compute_particle_emitter_buffer,
                    0,
                    bytemuck::cast_slice(&self.compute_particle_emitter_map),
                );
                self.compute_map_uploaded_count = compute_map_count;
                self.compute_map_uploaded_fingerprint = self.compute_map_fingerprint;
            }
            if compute_spawn_origin_recreated {
                queue.write_buffer(
                    &self.compute_particle_spawn_origin_buffer,
                    0,
                    bytemuck::cast_slice(&self.compute_particle_spawn_origins),
                );
                queue.write_buffer(
                    &self.compute_particle_spawn_rotation_buffer,
                    0,
                    bytemuck::cast_slice(&self.compute_particle_spawn_rotations),
                );
            } else if !self.compute_spawn_origin_dirty_slots.is_empty() {
                write_spawn_origin_dirty_ranges(
                    queue,
                    &self.compute_particle_spawn_origin_buffer,
                    &self.compute_particle_spawn_origins,
                    &mut self.compute_spawn_origin_dirty_slots,
                );
                write_spawn_origin_dirty_ranges(
                    queue,
                    &self.compute_particle_spawn_rotation_buffer,
                    &self.compute_particle_spawn_rotations,
                    &mut self.compute_spawn_rotation_dirty_slots,
                );
            }
            let params = GpuEmitterParams {
                emitter_count: self.compute_emitters.len() as u32,
                particle_count: self.compute_particle_count,
                _pad: [0; 2],
            };
            queue.write_buffer(&self.compute_params_buffer, 0, bytemuck::bytes_of(&params));
            if !self.compute_expr_ops.is_empty() {
                queue.write_buffer(
                    &self.compute_expr_op_buffer,
                    0,
                    bytemuck::cast_slice(&self.compute_expr_ops),
                );
            }
            if !self.compute_custom_params.is_empty() {
                queue.write_buffer(
                    &self.compute_custom_param_buffer,
                    0,
                    bytemuck::cast_slice(&self.compute_custom_params),
                );
            }
        }

        let uniform = CameraUniform {
            view_proj: compute_view_proj(&frame.camera, frame.width, frame.height)
                .to_cols_array_2d(),
            inv_view_size: [
                1.0 / (frame.width.max(1) as f32),
                1.0 / (frame.height.max(1) as f32),
            ],
            _pad: [0.0, 0.0],
        };
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
    }
}
