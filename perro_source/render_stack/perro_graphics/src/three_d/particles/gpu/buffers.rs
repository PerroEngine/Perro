use super::*;

impl GpuPointParticles3D {
    pub(super) fn ensure_particle_capacity(&mut self, device: &wgpu::Device, needed: usize) {
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
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.particle_capacity = new_capacity;
    }

    pub(super) fn ensure_billboard_particle_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
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
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
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
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
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
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
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
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            self.hybrid_particle_spawn_rotation_capacity = new_capacity;
            spawn_rotation_recreated = true;
        }
        if emitter_recreated || map_recreated || spawn_origin_recreated || spawn_rotation_recreated
        {
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
                usage: wgpu::BufferUsages::STORAGE,
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
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
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
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
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
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
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
        spawn_origin_recreated || spawn_rotation_recreated
    }
}
