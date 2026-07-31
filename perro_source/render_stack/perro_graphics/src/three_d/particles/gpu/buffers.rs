use super::*;

const PARTICLE_STORAGE_USAGE: wgpu::BufferUsages = wgpu::BufferUsages::STORAGE
    .union(wgpu::BufferUsages::COPY_DST)
    .union(wgpu::BufferUsages::COPY_SRC);
const PARTICLE_VERTEX_USAGE: wgpu::BufferUsages = wgpu::BufferUsages::VERTEX
    .union(wgpu::BufferUsages::COPY_DST)
    .union(wgpu::BufferUsages::COPY_SRC);

impl GpuPointParticles3D {
    pub(super) fn ensure_particle_capacity(&mut self, device: &wgpu::Device, needed: usize) {
        self.shrink_points.note_used(needed);
        if needed <= self.particle_capacity {
            return;
        }
        let mut new_capacity = self.particle_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_points"),
            size: (new_capacity * std::mem::size_of::<PointParticleGpu>()) as u64,
            usage: PARTICLE_VERTEX_USAGE,
            mapped_at_creation: false,
        });
        self.particle_capacity = new_capacity;
    }

    pub(super) fn ensure_billboard_particle_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
        self.shrink_billboards.note_used(needed);
        if needed <= self.billboard_particle_capacity {
            return;
        }
        let mut new_capacity = self.billboard_particle_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.billboard_particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_billboards"),
            size: (new_capacity * std::mem::size_of::<PointParticleGpu>()) as u64,
            usage: PARTICLE_VERTEX_USAGE,
            mapped_at_creation: false,
        });
        self.billboard_particle_capacity = new_capacity;
    }

    pub(super) fn ensure_hybrid_emitter_capacity(
        &mut self,
        device: &wgpu::Device,
        needed_emitters: usize,
        needed_particles: usize,
        needed_spawn_slots: usize,
    ) -> bool {
        self.shrink_hybrid_map.note_used(needed_particles);
        self.shrink_hybrid_spawn_origins
            .note_used(needed_spawn_slots);
        self.shrink_hybrid_spawn_rotations
            .note_used(needed_spawn_slots);
        let mut emitter_recreated = false;
        if needed_emitters > self.hybrid_emitter_capacity {
            let mut new_capacity = self.hybrid_emitter_capacity.max(1);
            while new_capacity < needed_emitters {
                new_capacity *= 2;
            }
            self.hybrid_emitter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_particles3d_hybrid_emitters"),
                size: (new_capacity * std::mem::size_of::<GpuEmitterParticle>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.hybrid_emitter_capacity = new_capacity;
            emitter_recreated = true;
        }
        let mut map_recreated = false;
        if needed_particles > self.hybrid_particle_emitter_capacity {
            let mut new_capacity = self.hybrid_particle_emitter_capacity.max(1);
            while new_capacity < needed_particles {
                new_capacity *= 2;
            }
            self.hybrid_particle_emitter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_particles3d_hybrid_particle_emitters"),
                size: (new_capacity * std::mem::size_of::<u32>()) as u64,
                usage: PARTICLE_STORAGE_USAGE,
                mapped_at_creation: false,
            });
            self.hybrid_particle_emitter_capacity = new_capacity;
            map_recreated = true;
        }
        let mut spawn_origin_recreated = false;
        if needed_spawn_slots > self.hybrid_particle_spawn_origin_capacity {
            let mut new_capacity = self.hybrid_particle_spawn_origin_capacity.max(1);
            while new_capacity < needed_spawn_slots {
                new_capacity *= 2;
            }
            self.hybrid_particle_spawn_origin_buffer =
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("perro_particles3d_hybrid_particle_spawn_origins"),
                    size: (new_capacity * std::mem::size_of::<[f32; 4]>()) as u64,
                    usage: PARTICLE_STORAGE_USAGE,
                    mapped_at_creation: false,
                });
            self.hybrid_particle_spawn_origin_capacity = new_capacity;
            spawn_origin_recreated = true;
        }
        let mut spawn_rotation_recreated = false;
        if needed_spawn_slots > self.hybrid_particle_spawn_rotation_capacity {
            let mut new_capacity = self.hybrid_particle_spawn_rotation_capacity.max(1);
            while new_capacity < needed_spawn_slots {
                new_capacity *= 2;
            }
            self.hybrid_particle_spawn_rotation_buffer =
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("perro_particles3d_hybrid_particle_spawn_rotations"),
                    size: (new_capacity * std::mem::size_of::<[f32; 4]>()) as u64,
                    usage: PARTICLE_STORAGE_USAGE,
                    mapped_at_creation: false,
                });
            self.hybrid_particle_spawn_rotation_capacity = new_capacity;
            spawn_rotation_recreated = true;
        }
        if emitter_recreated || map_recreated || spawn_origin_recreated || spawn_rotation_recreated
        {
            self.rebuild_hybrid_params_bind_group(device);
        }
        spawn_origin_recreated || spawn_rotation_recreated
    }

    pub(super) fn ensure_compute_capacity(
        &mut self,
        device: &wgpu::Device,
        needed_emitters: usize,
        needed_particles: usize,
        needed_spawn_slots: usize,
        needed_expr_ops: usize,
        needed_custom_params: usize,
    ) -> bool {
        self.shrink_compute_particles.note_used(needed_particles);
        self.shrink_compute_map.note_used(needed_particles);
        self.shrink_compute_spawn_origins
            .note_used(needed_spawn_slots);
        self.shrink_compute_spawn_rotations
            .note_used(needed_spawn_slots);
        let mut emitter_recreated = false;
        if needed_emitters > self.compute_emitter_capacity {
            let mut new_capacity = self.compute_emitter_capacity.max(1);
            while new_capacity < needed_emitters {
                new_capacity *= 2;
            }
            self.compute_emitter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_particles3d_compute_emitters"),
                size: (new_capacity * std::mem::size_of::<GpuEmitterParticle>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.compute_emitter_capacity = new_capacity;
            emitter_recreated = true;
        }

        let mut particle_recreated = false;
        if needed_particles > self.compute_particle_capacity {
            let mut new_capacity = self.compute_particle_capacity.max(1);
            while new_capacity < needed_particles {
                new_capacity *= 2;
            }
            self.compute_particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_particles3d_compute_particles"),
                size: (new_capacity * std::mem::size_of::<GpuComputedParticle>()) as u64,
                usage: PARTICLE_STORAGE_USAGE,
                mapped_at_creation: false,
            });
            self.compute_particle_capacity = new_capacity;
            particle_recreated = true;
        }

        let mut expr_recreated = false;
        if needed_expr_ops > self.compute_expr_op_capacity {
            let mut new_capacity = self.compute_expr_op_capacity.max(1);
            while new_capacity < needed_expr_ops {
                new_capacity *= 2;
            }
            self.compute_expr_op_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_particles3d_compute_expr_ops"),
                size: (new_capacity * std::mem::size_of::<GpuExprOp>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.compute_expr_op_capacity = new_capacity;
            expr_recreated = true;
        }

        let mut params_recreated = false;
        if needed_custom_params > self.compute_custom_param_capacity {
            let mut new_capacity = self.compute_custom_param_capacity.max(1);
            while new_capacity < needed_custom_params {
                new_capacity *= 2;
            }
            self.compute_custom_param_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_particles3d_compute_custom_params"),
                size: (new_capacity * std::mem::size_of::<f32>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.compute_custom_param_capacity = new_capacity;
            params_recreated = true;
        }
        let mut map_recreated = false;
        if needed_particles > self.compute_particle_emitter_capacity {
            let mut new_capacity = self.compute_particle_emitter_capacity.max(1);
            while new_capacity < needed_particles {
                new_capacity *= 2;
            }
            self.compute_particle_emitter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_particles3d_compute_particle_emitters"),
                size: (new_capacity * std::mem::size_of::<u32>()) as u64,
                usage: PARTICLE_STORAGE_USAGE,
                mapped_at_creation: false,
            });
            self.compute_particle_emitter_capacity = new_capacity;
            map_recreated = true;
        }
        let mut spawn_origin_recreated = false;
        if needed_spawn_slots > self.compute_particle_spawn_origin_capacity {
            let mut new_capacity = self.compute_particle_spawn_origin_capacity.max(1);
            while new_capacity < needed_spawn_slots {
                new_capacity *= 2;
            }
            self.compute_particle_spawn_origin_buffer =
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("perro_particles3d_compute_particle_spawn_origins"),
                    size: (new_capacity * std::mem::size_of::<[f32; 4]>()) as u64,
                    usage: PARTICLE_STORAGE_USAGE,
                    mapped_at_creation: false,
                });
            self.compute_particle_spawn_origin_capacity = new_capacity;
            spawn_origin_recreated = true;
        }
        let mut spawn_rotation_recreated = false;
        if needed_spawn_slots > self.compute_particle_spawn_rotation_capacity {
            let mut new_capacity = self.compute_particle_spawn_rotation_capacity.max(1);
            while new_capacity < needed_spawn_slots {
                new_capacity *= 2;
            }
            self.compute_particle_spawn_rotation_buffer =
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("perro_particles3d_compute_particle_spawn_rotations"),
                    size: (new_capacity * std::mem::size_of::<[f32; 4]>()) as u64,
                    usage: PARTICLE_STORAGE_USAGE,
                    mapped_at_creation: false,
                });
            self.compute_particle_spawn_rotation_capacity = new_capacity;
            spawn_rotation_recreated = true;
        }

        if emitter_recreated
            || particle_recreated
            || expr_recreated
            || params_recreated
            || map_recreated
            || spawn_origin_recreated
            || spawn_rotation_recreated
        {
            self.rebuild_compute_bind_groups(device);
        }
        spawn_origin_recreated || spawn_rotation_recreated
    }

    fn rebuild_hybrid_params_bind_group(&mut self, device: &wgpu::Device) {
        self.hybrid_params_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_particles3d_hybrid_emitters_bg"),
            layout: &self.hybrid_emitters_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.hybrid_emitter_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.hybrid_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.hybrid_particle_emitter_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.hybrid_particle_spawn_origin_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self
                        .hybrid_particle_spawn_rotation_buffer
                        .as_entire_binding(),
                },
            ],
        });
    }

    fn rebuild_compute_bind_groups(&mut self, device: &wgpu::Device) {
        self.compute_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_particles3d_compute_bg"),
            layout: &self.compute_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.compute_emitter_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.compute_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.compute_particle_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.compute_expr_op_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.compute_custom_param_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.compute_particle_emitter_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self
                        .compute_particle_spawn_origin_buffer
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self
                        .compute_particle_spawn_rotation_buffer
                        .as_entire_binding(),
                },
            ],
        });
        self.compute_render_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_particles3d_compute_render_bg"),
            layout: &self.compute_render_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 8,
                resource: self.compute_particle_buffer.as_entire_binding(),
            }],
        });
    }

    /// Periodic GC tick: shrink the per-particle buffers once usage stays far
    /// below capacity. Content (including GPU-side sim state in the compute
    /// particle buffer) is preserved with a prefix copy, so live particles and
    /// the map/spawn upload gates stay valid; bind groups referencing shrunk
    /// buffers are rebuilt.
    pub fn shrink_tick(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if let Some(new_cap) = self.shrink_points.tick(self.particle_capacity, 1024) {
            self.particle_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.particle_buffer,
                "perro_particles3d_points",
                (new_cap * std::mem::size_of::<PointParticleGpu>()) as u64,
                PARTICLE_VERTEX_USAGE,
            );
            self.particle_capacity = new_cap;
        }
        if let Some(new_cap) = self
            .shrink_billboards
            .tick(self.billboard_particle_capacity, 1024)
        {
            self.billboard_particle_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.billboard_particle_buffer,
                "perro_particles3d_billboards",
                (new_cap * std::mem::size_of::<PointParticleGpu>()) as u64,
                PARTICLE_VERTEX_USAGE,
            );
            self.billboard_particle_capacity = new_cap;
        }

        let mut rebuild_compute = false;
        if let Some(new_cap) = self
            .shrink_compute_particles
            .tick(self.compute_particle_capacity, 1024)
        {
            self.compute_particle_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.compute_particle_buffer,
                "perro_particles3d_compute_particles",
                (new_cap * std::mem::size_of::<GpuComputedParticle>()) as u64,
                PARTICLE_STORAGE_USAGE,
            );
            self.compute_particle_capacity = new_cap;
            rebuild_compute = true;
        }
        if let Some(new_cap) = self
            .shrink_compute_map
            .tick(self.compute_particle_emitter_capacity, 1024)
        {
            self.compute_particle_emitter_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.compute_particle_emitter_buffer,
                "perro_particles3d_compute_particle_emitters",
                (new_cap * std::mem::size_of::<u32>()) as u64,
                PARTICLE_STORAGE_USAGE,
            );
            self.compute_particle_emitter_capacity = new_cap;
            rebuild_compute = true;
        }
        if let Some(new_cap) = self
            .shrink_compute_spawn_origins
            .tick(self.compute_particle_spawn_origin_capacity, 1024)
        {
            self.compute_particle_spawn_origin_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.compute_particle_spawn_origin_buffer,
                "perro_particles3d_compute_particle_spawn_origins",
                (new_cap * std::mem::size_of::<[f32; 4]>()) as u64,
                PARTICLE_STORAGE_USAGE,
            );
            self.compute_particle_spawn_origin_capacity = new_cap;
            rebuild_compute = true;
        }
        if let Some(new_cap) = self
            .shrink_compute_spawn_rotations
            .tick(self.compute_particle_spawn_rotation_capacity, 1024)
        {
            self.compute_particle_spawn_rotation_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.compute_particle_spawn_rotation_buffer,
                "perro_particles3d_compute_particle_spawn_rotations",
                (new_cap * std::mem::size_of::<[f32; 4]>()) as u64,
                PARTICLE_STORAGE_USAGE,
            );
            self.compute_particle_spawn_rotation_capacity = new_cap;
            rebuild_compute = true;
        }
        if rebuild_compute {
            self.rebuild_compute_bind_groups(device);
        }

        let mut rebuild_hybrid = false;
        if let Some(new_cap) = self
            .shrink_hybrid_map
            .tick(self.hybrid_particle_emitter_capacity, 1024)
        {
            self.hybrid_particle_emitter_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.hybrid_particle_emitter_buffer,
                "perro_particles3d_hybrid_particle_emitters",
                (new_cap * std::mem::size_of::<u32>()) as u64,
                PARTICLE_STORAGE_USAGE,
            );
            self.hybrid_particle_emitter_capacity = new_cap;
            rebuild_hybrid = true;
        }
        if let Some(new_cap) = self
            .shrink_hybrid_spawn_origins
            .tick(self.hybrid_particle_spawn_origin_capacity, 1024)
        {
            self.hybrid_particle_spawn_origin_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.hybrid_particle_spawn_origin_buffer,
                "perro_particles3d_hybrid_particle_spawn_origins",
                (new_cap * std::mem::size_of::<[f32; 4]>()) as u64,
                PARTICLE_STORAGE_USAGE,
            );
            self.hybrid_particle_spawn_origin_capacity = new_cap;
            rebuild_hybrid = true;
        }
        if let Some(new_cap) = self
            .shrink_hybrid_spawn_rotations
            .tick(self.hybrid_particle_spawn_rotation_capacity, 1024)
        {
            self.hybrid_particle_spawn_rotation_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.hybrid_particle_spawn_rotation_buffer,
                "perro_particles3d_hybrid_particle_spawn_rotations",
                (new_cap * std::mem::size_of::<[f32; 4]>()) as u64,
                PARTICLE_STORAGE_USAGE,
            );
            self.hybrid_particle_spawn_rotation_capacity = new_cap;
            rebuild_hybrid = true;
        }
        if rebuild_hybrid {
            self.rebuild_hybrid_params_bind_group(device);
        }
    }
}
