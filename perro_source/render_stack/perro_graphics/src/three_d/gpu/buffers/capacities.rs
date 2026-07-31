use super::*;

impl Gpu3D {
    pub(in super::super) fn ensure_instance_transform_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
        self.shrink.instance_transforms.note_used(needed);
        if needed <= self.instance_transform_capacity {
            return;
        }
        let mut new_capacity = self.instance_transform_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.instance_transform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_mesh_instance_transforms"),
            size: (new_capacity * std::mem::size_of::<TransformInstanceGpu>()) as u64,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        self.instance_transform_capacity = new_capacity;
    }

    pub(in super::super) fn ensure_rigid_instance_meta_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
        self.shrink.rigid_instance_meta.note_used(needed);
        if needed <= self.rigid_instance_meta_capacity {
            return;
        }
        let mut new_capacity = self.rigid_instance_meta_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.rigid_instance_meta_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_mesh_instance_rigid_meta"),
            size: (new_capacity * std::mem::size_of::<RigidInstanceMetaGpu>()) as u64,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        self.rigid_instance_meta_capacity = new_capacity;
    }

    pub(in super::super) fn ensure_skinned_instance_meta_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
        self.shrink.skinned_instance_meta.note_used(needed);
        if needed <= self.skinned_instance_meta_capacity {
            return;
        }
        let mut new_capacity = self.skinned_instance_meta_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.skinned_instance_meta_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_mesh_instance_skinned_meta"),
            size: (new_capacity * std::mem::size_of::<SkinnedInstanceMetaGpu>()) as u64,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        self.skinned_instance_meta_capacity = new_capacity;
    }

    pub(in super::super) fn ensure_blend_shape_weight_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
        if needed <= self.blend_shape_weight_capacity {
            return;
        }
        let mut new_capacity = self.blend_shape_weight_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.blend_shape_weight_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_blend_shape_weights"),
            size: (new_capacity * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.blend_shape_weight_capacity = new_capacity;
        self.rebuild_camera_bind_groups(device);
    }

    pub(in super::super) fn ensure_blend_shape_instance_meta_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
        if needed <= self.blend_shape_instance_meta_capacity {
            return;
        }
        let mut new_capacity = self.blend_shape_instance_meta_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.blend_shape_instance_meta_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_blend_shape_instance_meta"),
            size: (new_capacity * std::mem::size_of::<BlendShapeInstanceMetaGpu>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.blend_shape_instance_meta_capacity = new_capacity;
        self.rebuild_camera_bind_groups(device);
    }

    pub(in super::super) fn ensure_packed_lod_param_capacity(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        needed: usize,
    ) {
        if needed <= self.packed_lod_param_capacity {
            return;
        }
        let mut new_capacity = self.packed_lod_param_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.packed_lod_param_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_packed_lod_params"),
            size: (new_capacity * std::mem::size_of::<PackedLodParamGpu>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if !self.packed_lod_params.is_empty() {
            queue.write_buffer(
                &self.packed_lod_param_buffer,
                0,
                bytemuck::cast_slice(&self.packed_lod_params),
            );
        }
        self.packed_lod_param_capacity = new_capacity;
        self.rebuild_camera_bind_groups(device);
    }

    pub(in super::super) fn ensure_multimesh_instance_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
        self.shrink.multimesh_instances.note_used(needed);
        if needed <= self.multimesh_instance_capacity {
            return;
        }
        let mut new_capacity = self.multimesh_instance_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.multimesh_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_multimesh_instances"),
            size: (new_capacity * std::mem::size_of::<MultiMeshInstanceGpu>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        self.multimesh_instance_capacity = new_capacity;
        // fresh buffer, undefined contents: the skip-identical gate must not fire.
        self.last_uploaded_multimesh_instances_hash = None;
        self.rebuild_camera_bind_groups(device);
    }

    pub(in super::super) fn ensure_multimesh_draw_params_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
        self.shrink.multimesh_draw_params.note_used(needed);
        if needed <= self.multimesh_draw_params_capacity {
            return;
        }
        let mut new_capacity = self.multimesh_draw_params_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.multimesh_draw_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_multimesh_draw_params"),
            size: (new_capacity * std::mem::size_of::<MultiMeshDrawParamGpu>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        self.multimesh_draw_params_capacity = new_capacity;
        // fresh buffer, undefined contents: the skip-identical gate must not fire.
        self.last_uploaded_multimesh_draw_params_hash = None;
        self.rebuild_camera_bind_groups(device);
    }

    pub(in super::super) fn rebuild_camera_bind_groups(&mut self, device: &wgpu::Device) {
        self.camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_camera3d_bg"),
            layout: &self.camera_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.skeleton_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.custom_params_meta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.custom_params_values_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.blend_shape_delta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.blend_shape_weight_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.blend_shape_instance_meta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.decal_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&self.decal_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(&self.decal_sampler),
                },
            ],
        });
        self.water_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_water_camera3d_bg"),
            layout: &self.water_camera_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.camera_buffer.as_entire_binding(),
            }],
        });
        self.shadow_camera_bind_groups = self
            .shadow_camera_buffers
            .iter()
            .map(|buffer| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("perro_shadow_camera3d_bg"),
                    layout: &self.camera_bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: self.skeleton_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: self.custom_params_meta_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: self.custom_params_values_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: self.blend_shape_delta_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: self.blend_shape_weight_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 6,
                            resource: self.blend_shape_instance_meta_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 7,
                            resource: self.decal_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 8,
                            resource: wgpu::BindingResource::TextureView(&self.decal_texture_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 9,
                            resource: wgpu::BindingResource::Sampler(&self.decal_sampler),
                        },
                    ],
                })
            })
            .collect();
        self.rigid_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_camera3d_rigid_bg"),
            layout: &self.rigid_camera_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.custom_params_meta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.custom_params_values_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.blend_shape_delta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.blend_shape_weight_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.blend_shape_instance_meta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.packed_lod_param_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.decal_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&self.decal_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(&self.decal_sampler),
                },
            ],
        });
        self.rigid_shadow_camera_bind_groups = self
            .shadow_camera_buffers
            .iter()
            .map(|buffer| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("perro_shadow_camera3d_rigid_bg"),
                    layout: &self.rigid_camera_bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: self.custom_params_meta_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: self.custom_params_values_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: self.blend_shape_delta_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: self.blend_shape_weight_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: self.blend_shape_instance_meta_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 6,
                            resource: self.packed_lod_param_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 7,
                            resource: self.decal_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 8,
                            resource: wgpu::BindingResource::TextureView(&self.decal_texture_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 9,
                            resource: wgpu::BindingResource::Sampler(&self.decal_sampler),
                        },
                    ],
                })
            })
            .collect();
        self.multimesh_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_multimesh_bg"),
            layout: &self.multimesh_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.multimesh_draw_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.mesh_blend_depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.blend_shape_delta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.blend_shape_weight_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.blend_shape_instance_meta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.custom_params_meta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.custom_params_values_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: self.multimesh_visible_index_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: self.multimesh_instance_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: self.decal_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(&self.decal_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: wgpu::BindingResource::Sampler(&self.decal_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: wgpu::BindingResource::TextureView(
                        self.ssao_pass
                            .as_ref()
                            .map(ssao::SsaoPass::view)
                            .unwrap_or(&self.ssao_fallback_view),
                    ),
                },
            ],
        });
        self.shadow_multimesh_bind_groups =
            build_shadow_multimesh_bind_groups(ShadowMultimeshBgArgs {
                device,
                multimesh_bgl: &self.multimesh_bgl,
                shadow_camera_buffers: &self.shadow_camera_buffers,
                multimesh_draw_params_buffer: &self.multimesh_draw_params_buffer,
                mesh_blend_depth_view: &self.mesh_blend_depth_view,
                blend_shape_delta_buffer: &self.blend_shape_delta_buffer,
                blend_shape_weight_buffer: &self.blend_shape_weight_buffer,
                blend_shape_instance_meta_buffer: &self.blend_shape_instance_meta_buffer,
                custom_params_meta_buffer: &self.custom_params_meta_buffer,
                custom_params_values_buffer: &self.custom_params_values_buffer,
                shadow_identity_buffer: &self.multimesh_shadow_identity_buffer,
                multimesh_instance_buffer: &self.multimesh_instance_buffer,
                decal_buffer: &self.decal_buffer,
                decal_texture_view: &self.decal_texture_view,
                decal_sampler: &self.decal_sampler,
                ssao_view: self
                    .ssao_pass
                    .as_ref()
                    .map(ssao::SsaoPass::view)
                    .unwrap_or(&self.ssao_fallback_view),
            });
        self.rebuild_multimesh_cull_bind_group(device);
        self.camera_bind_group_generation = self.camera_bind_group_generation.wrapping_add(1);
        if self.camera_bind_group_generation == 0 {
            self.camera_bind_group_generation = 1;
        }
        self.multimesh_bind_group_generation = self.multimesh_bind_group_generation.wrapping_add(1);
        if self.multimesh_bind_group_generation == 0 {
            self.multimesh_bind_group_generation = 1;
        }
    }

    // Grow (if needed) and fill the multimesh shadow identity index buffer so
    // visible_indices[i] == i for the full instance set. Grow rebuilds the
    // shadow multimesh bind groups (binding 8). Called on multimesh topology
    // change when any multimesh batch casts shadows.
    pub(in super::super) fn ensure_multimesh_shadow_identity(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        needed: usize,
    ) {
        if needed == 0 {
            return;
        }
        if needed > self.multimesh_shadow_identity_capacity {
            let mut new_capacity = self.multimesh_shadow_identity_capacity.max(1);
            while new_capacity < needed {
                new_capacity *= 2;
            }
            self.multimesh_shadow_identity_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_multimesh_shadow_identity"),
                size: (new_capacity * std::mem::size_of::<u32>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.multimesh_shadow_identity_capacity = new_capacity;
            self.shadow_multimesh_bind_groups =
                build_shadow_multimesh_bind_groups(ShadowMultimeshBgArgs {
                    device,
                    multimesh_bgl: &self.multimesh_bgl,
                    shadow_camera_buffers: &self.shadow_camera_buffers,
                    multimesh_draw_params_buffer: &self.multimesh_draw_params_buffer,
                    mesh_blend_depth_view: &self.mesh_blend_depth_view,
                    blend_shape_delta_buffer: &self.blend_shape_delta_buffer,
                    blend_shape_weight_buffer: &self.blend_shape_weight_buffer,
                    blend_shape_instance_meta_buffer: &self.blend_shape_instance_meta_buffer,
                    custom_params_meta_buffer: &self.custom_params_meta_buffer,
                    custom_params_values_buffer: &self.custom_params_values_buffer,
                    shadow_identity_buffer: &self.multimesh_shadow_identity_buffer,
                    multimesh_instance_buffer: &self.multimesh_instance_buffer,
                    decal_buffer: &self.decal_buffer,
                    decal_texture_view: &self.decal_texture_view,
                    decal_sampler: &self.decal_sampler,
                    ssao_view: self
                        .ssao_pass
                        .as_ref()
                        .map(ssao::SsaoPass::view)
                        .unwrap_or(&self.ssao_fallback_view),
                });
        }
        // Reuse the cull identity staging (identical values); rebuild if short.
        if self.staged_multimesh_visible_identity.len() < needed {
            self.staged_multimesh_visible_identity.clear();
            self.staged_multimesh_visible_identity
                .extend(0..needed as u32);
        }
        queue.write_buffer(
            &self.multimesh_shadow_identity_buffer,
            0,
            bytemuck::cast_slice(&self.staged_multimesh_visible_identity[..needed]),
        );
    }

    pub(in super::super) fn rebuild_multimesh_cull_bind_group(&mut self, device: &wgpu::Device) {
        self.multimesh_cull_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_multimesh_cull_bg"),
            layout: &self.multimesh_cull_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.frustum_cull_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.multimesh_cull_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.multimesh_draw_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.multimesh_instance_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.multimesh_instance_batch_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.multimesh_cull_batch_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.multimesh_visible_index_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.multimesh_indirect_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: self.multimesh_cull_counter_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: self.hiz_cull_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::TextureView(&self.hiz_sample_view),
                },
            ],
        });
    }

    // Grow the per-instance cull buffers (instance_batch + visible_indices).
    // Callers must rebuild bind groups after (done via rebuild_camera_bind_groups
    // for visible_indices in the multimesh bg, and here for the cull bg).
    pub(in super::super) fn ensure_multimesh_cull_instance_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
        self.shrink.multimesh_cull_instances.note_used(needed);
        if needed <= self.multimesh_cull_instance_capacity {
            return;
        }
        let mut new_capacity = self.multimesh_cull_instance_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.multimesh_instance_batch_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_multimesh_instance_batch"),
            size: (new_capacity * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        self.multimesh_visible_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_multimesh_visible_indices"),
            size: (new_capacity * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        self.multimesh_cull_instance_capacity = new_capacity;
        // fresh buffers, undefined contents: the skip-identical gates must not fire.
        self.multimesh_instance_batch_uploaded_len = 0;
        self.last_uploaded_multimesh_identity_len = 0;
        self.multimesh_identity_primed = false;
        // visible_indices feeds the multimesh draw bind group too; rebuild both.
        self.rebuild_camera_bind_groups(device);
    }

    pub(in super::super) fn ensure_multimesh_cull_batch_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
        if needed <= self.multimesh_cull_batch_capacity {
            return;
        }
        let mut new_capacity = self.multimesh_cull_batch_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.multimesh_cull_batch_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_multimesh_cull_batches"),
            size: (new_capacity * std::mem::size_of::<MultiMeshCullBatchGpu>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.multimesh_cull_counter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_multimesh_cull_counters"),
            size: (new_capacity * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.multimesh_indirect_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_multimesh_indirect"),
            size: (new_capacity * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64,
            usage: wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.multimesh_cull_batch_capacity = new_capacity;
        // fresh buffers, undefined contents: the skip-identical gates must not fire.
        self.last_uploaded_multimesh_cull_batches.clear();
        self.last_uploaded_multimesh_indirect.clear();
        self.rebuild_multimesh_cull_bind_group(device);
    }

    pub(in super::super) fn ensure_skeleton_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
        self.shrink.skeletons.note_used(needed);
        if needed <= self.skeleton_capacity {
            return;
        }
        let mut new_capacity = self.skeleton_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.skeleton_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_skeleton_palette_buffer"),
            size: (new_capacity * std::mem::size_of::<[[f32; 4]; 3]>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        self.rebuild_camera_bind_groups(device);
        self.skeleton_capacity = new_capacity;
    }

    pub(in super::super) fn ensure_custom_params_capacity(
        &mut self,
        device: &wgpu::Device,
        meta_needed: usize,
        values_needed: usize,
    ) {
        let mut rebuilt = false;
        if meta_needed > self.custom_params_meta_capacity {
            let mut new_capacity = self.custom_params_meta_capacity.max(1);
            while new_capacity < meta_needed {
                new_capacity *= 2;
            }
            self.custom_params_meta_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_custom_material_params_meta"),
                size: (new_capacity * std::mem::size_of::<u32>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.custom_params_meta_capacity = new_capacity;
            self.custom_params_meta_uploaded = 0;
            rebuilt = true;
        }
        if values_needed > self.custom_params_values_capacity {
            let mut new_capacity = self.custom_params_values_capacity.max(1);
            while new_capacity < values_needed {
                new_capacity *= 2;
            }
            self.custom_params_values_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_custom_material_params_values"),
                size: (new_capacity * std::mem::size_of::<f32>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.custom_params_values_capacity = new_capacity;
            self.custom_params_values_uploaded = 0;
            rebuilt = true;
        }
        if rebuilt {
            self.rebuild_camera_bind_groups(device);
        }
    }

    /// Periodic GC tick: shrink the biggest grow-only buffers once usage
    /// stays far below capacity (see `crate::gpu_shrink`). Every shrink
    /// preserves content with a GPU->GPU prefix copy, so retained draws and
    /// skip-upload gates stay valid even across skip-prepare frames; bind
    /// groups referencing a shrunk buffer are rebuilt below.
    pub fn shrink_tick(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        use crate::gpu_shrink::shrink_buffer_preserving;
        // Mesh arena lengths are authoritative retained content.
        self.shrink
            .mesh_vertices
            .note_used(self.mesh_vertex_len.max(self.rigid_vertex_len));
        self.shrink.mesh_indices.note_used(self.mesh_index_len);
        self.shrink
            .packed_lod_vertices
            .note_used(self.packed_lod_vertex_len);
        self.shrink
            .packed_lod_indices
            .note_used(self.packed_lod_index_len);
        self.shrink
            .blend_shape_deltas
            .note_used(self.blend_shape_delta_len);

        let vertex_usage = wgpu::BufferUsages::VERTEX
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;
        let storage_usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;

        if let Some(new_cap) = self.shrink.mesh_vertices.tick(self.vertex_capacity, 1024) {
            self.vertex_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.vertex_buffer,
                "perro_mesh_vertices",
                (new_cap * std::mem::size_of::<SkinnedMeshVertex>()) as u64,
                vertex_usage,
            );
            self.rigid_vertex_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.rigid_vertex_buffer,
                "perro_mesh_vertices_rigid",
                (new_cap * std::mem::size_of::<RigidMeshVertex>()) as u64,
                vertex_usage,
            );
            self.vertex_capacity = new_cap;
            self.rigid_vertex_capacity = new_cap;
        }
        if let Some(new_cap) = self.shrink.mesh_indices.tick(self.index_capacity, 1024) {
            self.index_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.index_buffer,
                "perro_mesh_indices",
                (new_cap * std::mem::size_of::<u32>()) as u64,
                wgpu::BufferUsages::INDEX
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
            );
            self.index_capacity = new_cap;
        }
        if let Some(new_cap) = self
            .shrink
            .packed_lod_vertices
            .tick(self.packed_lod_vertex_capacity, 256)
        {
            self.packed_lod_vertex_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.packed_lod_vertex_buffer,
                "perro_packed_lod_vertices_rigid",
                (new_cap * std::mem::size_of::<PackedRigidLodVertex>()) as u64,
                vertex_usage,
            );
            self.packed_lod_vertex_capacity = new_cap;
        }
        if let Some(new_cap) = self
            .shrink
            .packed_lod_indices
            .tick(self.packed_lod_index_capacity, 256)
        {
            self.packed_lod_index_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.packed_lod_index_buffer,
                "perro_packed_lod_indices",
                (new_cap * std::mem::size_of::<u32>()) as u64,
                wgpu::BufferUsages::INDEX
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
            );
            self.packed_lod_index_capacity = new_cap;
        }

        if let Some(new_cap) = self
            .shrink
            .instance_transforms
            .tick(self.instance_transform_capacity, 256)
        {
            self.instance_transform_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.instance_transform_buffer,
                "perro_mesh_instance_transforms",
                (new_cap * std::mem::size_of::<TransformInstanceGpu>()) as u64,
                vertex_usage,
            );
            self.instance_transform_capacity = new_cap;
        }
        if let Some(new_cap) = self
            .shrink
            .rigid_instance_meta
            .tick(self.rigid_instance_meta_capacity, 256)
        {
            self.rigid_instance_meta_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.rigid_instance_meta_buffer,
                "perro_mesh_instance_rigid_meta",
                (new_cap * std::mem::size_of::<RigidInstanceMetaGpu>()) as u64,
                vertex_usage,
            );
            self.rigid_instance_meta_capacity = new_cap;
        }
        if let Some(new_cap) = self
            .shrink
            .skinned_instance_meta
            .tick(self.skinned_instance_meta_capacity, 256)
        {
            self.skinned_instance_meta_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.skinned_instance_meta_buffer,
                "perro_mesh_instance_skinned_meta",
                (new_cap * std::mem::size_of::<SkinnedInstanceMetaGpu>()) as u64,
                vertex_usage,
            );
            self.skinned_instance_meta_capacity = new_cap;
        }

        // Buffers below feed bind groups; rebuild them once at the end.
        let mut rebuild_camera = false;
        if let Some(new_cap) = self
            .shrink
            .blend_shape_deltas
            .tick(self.blend_shape_delta_capacity, 64)
        {
            self.blend_shape_delta_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.blend_shape_delta_buffer,
                "perro_blend_shape_deltas",
                (new_cap * std::mem::size_of::<BlendShapeDeltaGpu>()) as u64,
                storage_usage,
            );
            self.blend_shape_delta_capacity = new_cap;
            rebuild_camera = true;
        }
        if let Some(new_cap) = self.shrink.skeletons.tick(self.skeleton_capacity, 64) {
            self.skeleton_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.skeleton_buffer,
                "perro_skeleton_palette_buffer",
                (new_cap * std::mem::size_of::<[[f32; 4]; 3]>()) as u64,
                storage_usage,
            );
            self.skeleton_capacity = new_cap;
            rebuild_camera = true;
        }
        if let Some(new_cap) = self
            .shrink
            .multimesh_instances
            .tick(self.multimesh_instance_capacity, 256)
        {
            self.multimesh_instance_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.multimesh_instance_buffer,
                "perro_multimesh_instances",
                (new_cap * std::mem::size_of::<MultiMeshInstanceGpu>()) as u64,
                storage_usage,
            );
            self.multimesh_instance_capacity = new_cap;
            rebuild_camera = true;
        }
        if let Some(new_cap) = self
            .shrink
            .multimesh_draw_params
            .tick(self.multimesh_draw_params_capacity, 256)
        {
            self.multimesh_draw_params_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.multimesh_draw_params_buffer,
                "perro_multimesh_draw_params",
                (new_cap * std::mem::size_of::<MultiMeshDrawParamGpu>()) as u64,
                storage_usage,
            );
            self.multimesh_draw_params_capacity = new_cap;
            rebuild_camera = true;
        }
        if let Some(new_cap) = self
            .shrink
            .multimesh_cull_instances
            .tick(self.multimesh_cull_instance_capacity, 256)
        {
            self.multimesh_instance_batch_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.multimesh_instance_batch_buffer,
                "perro_multimesh_instance_batch",
                (new_cap * std::mem::size_of::<u32>()) as u64,
                storage_usage,
            );
            self.multimesh_visible_index_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.multimesh_visible_index_buffer,
                "perro_multimesh_visible_indices",
                (new_cap * std::mem::size_of::<u32>()) as u64,
                storage_usage,
            );
            self.multimesh_cull_instance_capacity = new_cap;
            rebuild_camera = true;
        }
        if rebuild_camera {
            // Also rebuilds the multimesh cull + shadow multimesh bind groups.
            self.rebuild_camera_bind_groups(device);
        }
    }

    pub(in super::super) fn ensure_frustum_cull_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
        if needed == 0 || needed <= self.frustum_cull_items_capacity {
            return;
        }
        let mut new_capacity = self.frustum_cull_items_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.frustum_cull_static_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_frustum_cull_static"),
            size: (new_capacity * std::mem::size_of::<FrustumCullStaticGpu>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.frustum_cull_dynamic_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_frustum_cull_dynamic"),
            size: (new_capacity * std::mem::size_of::<FrustumCullDynamicGpu>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.indirect_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_draw_indirect"),
            size: (new_capacity * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64,
            usage: wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        self.hiz_debug_readback_buffer = self.hiz_debug_readback_buffer.is_some().then(|| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_hiz_indirect_readback"),
                size: (new_capacity * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
        });
        self.pending_hiz_debug_count = 0;
        self.pending_hiz_debug_map_rx = None;
        self.frustum_cull_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_frustum_cull_bg"),
            layout: &self.frustum_cull_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.frustum_cull_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.frustum_cull_static_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.frustum_cull_dynamic_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.indirect_buffer.as_entire_binding(),
                },
            ],
        });
        self.hiz_cull_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_hiz_cull_bg"),
            layout: &self.hiz_cull_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.hiz_cull_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.frustum_cull_static_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.frustum_cull_dynamic_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.indirect_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&self.hiz_sample_view),
                },
            ],
        });
        self.frustum_cull_items_capacity = new_capacity;
        self.indirect_capacity = new_capacity;
        self.frustum_gpu_inputs_valid = false;
    }
}
