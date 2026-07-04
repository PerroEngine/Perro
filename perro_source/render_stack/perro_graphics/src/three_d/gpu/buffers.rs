use super::*;

pub(super) struct ShadowMultimeshBgArgs<'a> {
    pub(super) device: &'a wgpu::Device,
    pub(super) multimesh_bgl: &'a wgpu::BindGroupLayout,
    pub(super) shadow_camera_buffers: &'a [wgpu::Buffer],
    pub(super) multimesh_draw_params_buffer: &'a wgpu::Buffer,
    pub(super) mesh_blend_depth_view: &'a wgpu::TextureView,
    pub(super) blend_shape_delta_buffer: &'a wgpu::Buffer,
    pub(super) blend_shape_weight_buffer: &'a wgpu::Buffer,
    pub(super) blend_shape_instance_meta_buffer: &'a wgpu::Buffer,
    pub(super) custom_params_meta_buffer: &'a wgpu::Buffer,
    pub(super) custom_params_values_buffer: &'a wgpu::Buffer,
    pub(super) shadow_identity_buffer: &'a wgpu::Buffer,
    pub(super) multimesh_instance_buffer: &'a wgpu::Buffer,
}

// One multimesh draw bind group per shadow layer: identical to multimesh_bgl
// except binding 0 = that layer's scene uniform (light view-proj) and binding 8
// = the dedicated identity index buffer, so vs_depth draws the full instance set
// projected into the light's view regardless of the camera cull output.
pub(super) fn build_shadow_multimesh_bind_groups(
    args: ShadowMultimeshBgArgs<'_>,
) -> Vec<wgpu::BindGroup> {
    args.shadow_camera_buffers
        .iter()
        .map(|scene_buffer| {
            args.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("perro_shadow_multimesh_bg"),
                layout: args.multimesh_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: scene_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: args.multimesh_draw_params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(args.mesh_blend_depth_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: args.blend_shape_delta_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: args.blend_shape_weight_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: args.blend_shape_instance_meta_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: args.custom_params_meta_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: args.custom_params_values_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 8,
                        resource: args.shadow_identity_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 9,
                        resource: args.multimesh_instance_buffer.as_entire_binding(),
                    },
                ],
            })
        })
        .collect()
}

struct AppendPackedLodDataArgs<'a> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    vertices: &'a [MeshVertex],
    mesh_indices: &'a [u32],
    base_vertex: u32,
    decoded_lods: &'a [DecodedLod],
    decoded_surfaces: &'a [MeshRange],
}

impl Gpu3D {
    pub fn draw_call_count(&self) -> u32 {
        (self.draw_batches.len() + self.multimesh_batches.len()) as u32
    }

    #[inline]
    pub fn prepare_step_timing(&self) -> Prepare3DStepTiming {
        self.last_prepare_step_timing
    }

    pub(super) fn fallback_material_texture_bind_group(&self) -> &wgpu::BindGroup {
        self.material_fallback_texture
            .as_ref()
            .map(|cached| &cached.bind_group)
            .expect("material fallback texture must be initialized in prepare")
    }

    pub(super) fn material_texture_bind_group(&self, slot: u32) -> &wgpu::BindGroup {
        self.material_textures
            .get(&slot)
            .map(|cached| &cached.bind_group)
            .unwrap_or_else(|| self.fallback_material_texture_bind_group())
    }

    pub(super) fn ensure_material_fallback_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        if self.material_fallback_texture.is_some() {
            return;
        }
        let cached = create_cached_material_texture(
            device,
            queue,
            &self.material_texture_bgl,
            CachedMaterialTextureInput {
                rgba: vec![255u8, 255, 255, 255],
                width: 1,
                height: 1,
                source: "__fallback__".to_string(),
                filter: self.texture_filter,
            },
        );
        self.material_fallback_texture = Some(cached);
    }

    pub(super) fn ensure_material_texture_slot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &ResourceStore,
        slot: u32,
        mesh_source: &str,
        _static_texture_lookup: Option<StaticTextureLookup>,
    ) {
        if slot == MATERIAL_TEXTURE_NONE {
            return;
        }
        self.ensure_material_fallback_texture(device, queue);

        // glTF material texture indices are model-local, not global texture IDs.
        // Prefer glTF-local texture source when mesh source is glTF/glb.
        let gltf_source = gltf_texture_source_from_mesh_source(mesh_source, slot);
        let global_source = resources.texture_source_by_index(slot).or_else(|| {
            slot.checked_add(1)
                .and_then(|next| resources.texture_source_by_index(next))
        });
        let source = if gltf_source.is_some() {
            gltf_source.or_else(|| global_source.map(ToString::to_string))
        } else {
            global_source.map(ToString::to_string).or(gltf_source)
        };
        let Some(source) = source else {
            self.material_textures.remove(&slot);
            return;
        };
        if self
            .material_textures
            .get(&slot)
            .is_some_and(|cached| cached.source == source)
        {
            return;
        }

        let (rgba, width, height) =
            if let Some(decoded) = resources.decoded_texture_data_by_source(source.as_str()) {
                (decoded.rgba.clone(), decoded.width, decoded.height)
            } else if resources.has_texture_source(source.as_str()) {
                self.material_textures.remove(&slot);
                return;
            } else if let Some(decoded) = load_texture_rgba(source.as_str()) {
                decoded
            } else {
                self.material_textures.remove(&slot);
                return;
            };
        let cached = create_cached_material_texture(
            device,
            queue,
            &self.material_texture_bgl,
            CachedMaterialTextureInput {
                rgba,
                width,
                height,
                source,
                filter: self.texture_filter,
            },
        );
        self.material_textures.insert(slot, cached);
    }

    pub fn upsert_external_material_texture(
        &mut self,
        device: &wgpu::Device,
        slot: u32,
        view: &wgpu::TextureView,
        source: String,
    ) {
        if slot == MATERIAL_TEXTURE_NONE {
            return;
        }
        let cached =
            create_external_material_texture(device, &self.material_texture_bgl, view, source);
        self.material_textures.insert(slot, cached);
    }

    pub(super) fn ensure_instance_transform_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
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
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.instance_transform_capacity = new_capacity;
    }

    pub(super) fn ensure_rigid_instance_meta_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
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
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.rigid_instance_meta_capacity = new_capacity;
    }

    pub(super) fn ensure_skinned_instance_meta_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
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
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.skinned_instance_meta_capacity = new_capacity;
    }

    pub(super) fn ensure_blend_shape_weight_capacity(
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

    pub(super) fn ensure_blend_shape_instance_meta_capacity(
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

    pub(super) fn ensure_packed_lod_param_capacity(
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

    pub(super) fn ensure_multimesh_instance_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
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
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.multimesh_instance_capacity = new_capacity;
        self.rebuild_camera_bind_groups(device);
    }

    pub(super) fn ensure_multimesh_draw_params_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
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
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.multimesh_draw_params_capacity = new_capacity;
        self.rebuild_camera_bind_groups(device);
    }

    pub(super) fn rebuild_camera_bind_groups(&mut self, device: &wgpu::Device) {
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
    pub(super) fn ensure_multimesh_shadow_identity(
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

    pub(super) fn rebuild_multimesh_cull_bind_group(&mut self, device: &wgpu::Device) {
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
    pub(super) fn ensure_multimesh_cull_instance_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
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
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.multimesh_visible_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_multimesh_visible_indices"),
            size: (new_capacity * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.multimesh_cull_instance_capacity = new_capacity;
        // visible_indices feeds the multimesh draw bind group too; rebuild both.
        self.rebuild_camera_bind_groups(device);
    }

    pub(super) fn ensure_multimesh_cull_batch_capacity(
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
        self.rebuild_multimesh_cull_bind_group(device);
    }

    pub(super) fn ensure_skeleton_capacity(&mut self, device: &wgpu::Device, needed: usize) {
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
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.rebuild_camera_bind_groups(device);
        self.skeleton_capacity = new_capacity;
    }

    pub(super) fn ensure_custom_params_capacity(
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

    pub(super) fn ensure_frustum_cull_capacity(&mut self, device: &wgpu::Device, needed: usize) {
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
        self.hiz_debug_readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_hiz_indirect_readback"),
            size: (new_capacity * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
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

    pub(super) fn build_hiz_from_depth(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some(copy_bg) = self.hiz_copy_bind_group.as_ref() else {
            return;
        };
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perro_hiz_copy_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.hiz_copy_pipeline);
            pass.set_bind_group(0, copy_bg, &[]);
            let groups_x = self.hiz_size.0.div_ceil(HIZ_WORKGROUP_SIZE_X);
            let groups_y = self.hiz_size.1.div_ceil(HIZ_WORKGROUP_SIZE_Y);
            pass.dispatch_workgroups(groups_x, groups_y, 1);
        }
        // SPD path: all downsample dispatches share ONE compute pass. Each dispatch
        // reads mip (HIZ_SPD_MIPS*d) and writes the next up-to-HIZ_SPD_MIPS mips
        // using workgroup shared memory, so the only serialization is between the
        // chunk dispatches, not per mip. Falls back to the per-mip path below when
        // the device lacks storage textures for the SPD bind group.
        if self.hiz_spd_supported && !self.hiz_spd_bind_groups.is_empty() {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perro_hiz_spd_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.hiz_spd_pipeline);
            // Source-mip index this dispatch reads (mip 0 is filled by the copy).
            let mut src_mip = 0u32;
            for spd_bg in &self.hiz_spd_bind_groups {
                // Base dst mip (src_mip+1) determines the workgroup grid: an 8x8
                // workgroup owns an 8x8 output region of that base mip.
                let base_dst = src_mip + 1;
                let dst_w = (self.hiz_size.0 >> base_dst).max(1);
                let dst_h = (self.hiz_size.1 >> base_dst).max(1);
                pass.set_bind_group(0, spd_bg, &[]);
                pass.dispatch_workgroups(
                    dst_w.div_ceil(HIZ_WORKGROUP_SIZE_X),
                    dst_h.div_ceil(HIZ_WORKGROUP_SIZE_Y),
                    1,
                );
                src_mip += HIZ_SPD_MIPS;
            }
            return;
        }
        let mut src_w = self.hiz_size.0;
        let mut src_h = self.hiz_size.1;
        for downsample_bg in &self.hiz_downsample_bind_groups {
            let dst_w = (src_w / 2).max(1);
            let dst_h = (src_h / 2).max(1);
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perro_hiz_downsample_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.hiz_downsample_pipeline);
            pass.set_bind_group(0, downsample_bg, &[]);
            pass.dispatch_workgroups(
                dst_w.div_ceil(HIZ_WORKGROUP_SIZE_X),
                dst_h.div_ceil(HIZ_WORKGROUP_SIZE_Y),
                1,
            );
            src_w = dst_w;
            src_h = dst_h;
        }
    }

    pub(super) fn rebuild_hiz_bind_groups(&mut self, device: &wgpu::Device) {
        self.hiz_spd_bind_groups.clear();
        self.hiz_spd_params_buffers.clear();
        if self.hiz_mip_views.is_empty() {
            self.hiz_copy_bind_group = None;
            self.hiz_downsample_bind_groups.clear();
            return;
        }

        self.hiz_copy_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_hiz_copy_bg"),
            layout: &self.hiz_copy_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.depth_prepass_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.hiz_mip_views[0]),
                },
            ],
        }));

        if self.hiz_spd_supported {
            self.rebuild_hiz_spd_bind_groups(device);
            // SPD path drives all downsampling; the per-mip groups stay empty.
            self.hiz_downsample_bind_groups.clear();
            return;
        }

        self.hiz_downsample_bind_groups.clear();
        self.hiz_downsample_bind_groups
            .reserve(self.hiz_mip_count.saturating_sub(1) as usize);
        for mip in 1..self.hiz_mip_count as usize {
            self.hiz_downsample_bind_groups
                .push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("perro_hiz_downsample_bg"),
                    layout: &self.hiz_downsample_bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(
                                &self.hiz_mip_views[mip - 1],
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&self.hiz_mip_views[mip]),
                        },
                    ],
                }));
        }
    }

    // Build the SPD downsample chain: one bind group + uniform per dispatch. Each
    // dispatch d reads mip (HIZ_SPD_MIPS*d) and writes up to HIZ_SPD_MIPS dst mips
    // above it. Unused dst slots (last chunk) are bound to a real mip view (mip 0)
    // as a dummy; the shader guards every store on `mip_count`, so nothing is
    // written there. Source dims feed NPOT edge clamping.
    fn rebuild_hiz_spd_bind_groups(&mut self, device: &wgpu::Device) {
        let total_mips = self.hiz_mip_count as usize;
        let spd = HIZ_SPD_MIPS as usize;
        // dst mips are 1..total_mips (mip 0 is the copy output / SPD source).
        let mut src_mip = 0usize;
        while src_mip + 1 < total_mips {
            let dst_count = (total_mips - (src_mip + 1)).min(spd);
            let src_w = (self.hiz_size.0 >> src_mip).max(1);
            let src_h = (self.hiz_size.1 >> src_mip).max(1);
            let params = HizSpdParamsGpu {
                mip_count: dst_count as u32,
                src_width: src_w,
                src_height: src_h,
                _pad: 0,
            };
            let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_hiz_spd_params"),
                size: std::mem::size_of::<HizSpdParamsGpu>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: true,
            });
            params_buffer
                .slice(..)
                .get_mapped_range_mut()
                .copy_from_slice(bytemuck::bytes_of(&params));
            params_buffer.unmap();

            let mut entries = Vec::with_capacity(spd + 2);
            entries.push(wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&self.hiz_mip_views[src_mip]),
            });
            for slot in 0..spd {
                // Real dst mip for this slot, or mip 0 as a bound-but-unwritten dummy.
                let dst_mip = if slot < dst_count {
                    src_mip + 1 + slot
                } else {
                    0
                };
                entries.push(wgpu::BindGroupEntry {
                    binding: 1 + slot as u32,
                    resource: wgpu::BindingResource::TextureView(&self.hiz_mip_views[dst_mip]),
                });
            }
            entries.push(wgpu::BindGroupEntry {
                binding: 1 + HIZ_SPD_MIPS,
                resource: params_buffer.as_entire_binding(),
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("perro_hiz_spd_bg"),
                layout: &self.hiz_spd_bgl,
                entries: &entries,
            });
            self.hiz_spd_bind_groups.push(bind_group);
            self.hiz_spd_params_buffers.push(params_buffer);
            src_mip += spd;
        }
    }

    pub(super) fn request_hiz_debug_map_async(&mut self) {
        if self.pending_hiz_debug_count == 0 || self.pending_hiz_debug_map_rx.is_some() {
            return;
        }
        let byte_len = u64::from(self.pending_hiz_debug_count)
            * std::mem::size_of::<DrawIndexedIndirectGpu>() as u64;
        let (tx, rx) = mpsc::channel();
        self.hiz_debug_readback_buffer.slice(0..byte_len).map_async(
            wgpu::MapMode::Read,
            move |result| {
                let _ = tx.send(result);
            },
        );
        self.pending_hiz_debug_map_rx = Some(rx);
    }

    pub(super) fn consume_hiz_debug_results(&mut self) {
        let count = self.pending_hiz_debug_count as usize;
        if count == 0 {
            return;
        }
        let Some(rx) = self.pending_hiz_debug_map_rx.as_ref() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(())) => {
                let byte_len = (count * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64;
                let data = self
                    .hiz_debug_readback_buffer
                    .slice(0..byte_len)
                    .get_mapped_range();
                let mut visible = 0u32;
                for bytes in data.chunks_exact(std::mem::size_of::<DrawIndexedIndirectGpu>()) {
                    let cmd = bytemuck::from_bytes::<DrawIndexedIndirectGpu>(bytes);
                    if cmd.instance_count > 0 {
                        visible = visible.saturating_add(1);
                    }
                }
                drop(data);
                self.hiz_debug_readback_buffer.unmap();

                let _total_batches = self.pending_hiz_debug_count;
                let _frustum_visible_est = self.pending_hiz_debug_frustum_visible_est;
                let _visible = visible;
                self.pending_hiz_debug_count = 0;
                self.pending_hiz_debug_frustum_visible_est = 0;
                self.pending_hiz_debug_map_rx = None;
            }
            Ok(Err(_)) | Err(TryRecvError::Disconnected) => {
                self.hiz_debug_readback_buffer.unmap();
                self.pending_hiz_debug_count = 0;
                self.pending_hiz_debug_frustum_visible_est = 0;
                self.pending_hiz_debug_map_rx = None;
            }
            Err(TryRecvError::Empty) => {}
        }
    }

    pub(super) fn should_probe_or_draw(&self, key: u64) -> bool {
        let Some(state) = self.occlusion_state.get(&key) else {
            return true;
        };
        state.visible_last_frame
            || self.occlusion_frame.saturating_sub(state.last_test_frame)
                >= OCCLUSION_PROBE_INTERVAL
    }

    pub(super) fn push_occlusion_query_key(&mut self, key: u64) -> u32 {
        let query = self.occlusion_query_keys_this_frame.len() as u32;
        self.occlusion_query_keys_this_frame.push(key);
        query
    }

    pub(super) fn ensure_occlusion_query_capacity(&mut self, device: &wgpu::Device, needed: u32) {
        if !self.cpu_occlusion_enabled {
            return;
        }
        if needed == 0 || needed <= self.occlusion_query_capacity {
            return;
        }
        let mut capacity = self.occlusion_query_capacity.max(64);
        while capacity < needed {
            capacity = capacity.saturating_mul(2);
        }
        self.occlusion_query_set = Some(device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("perro_occlusion_query_set"),
            ty: wgpu::QueryType::Occlusion,
            count: capacity,
        }));
        let byte_len = u64::from(capacity) * 8;
        self.occlusion_resolve_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_occlusion_resolve"),
            size: byte_len,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));
        self.occlusion_readback_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_occlusion_readback"),
            size: byte_len,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        }));
        self.occlusion_query_capacity = capacity;
    }

    pub(super) fn request_occlusion_map_async(&mut self) {
        if self.pending_occlusion_query_count == 0 || self.pending_occlusion_map_rx.is_some() {
            return;
        }
        let Some(readback) = self.occlusion_readback_buffer.as_ref() else {
            return;
        };
        let byte_len = u64::from(self.pending_occlusion_query_count) * 8;
        let (tx, rx) = mpsc::channel();
        readback
            .slice(0..byte_len)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        self.pending_occlusion_map_rx = Some(rx);
    }

    pub(super) fn consume_occlusion_results(&mut self) {
        if !self.cpu_occlusion_enabled {
            return;
        }
        let query_count = self.pending_occlusion_query_count as usize;
        if query_count == 0 {
            return;
        }
        let Some(rx) = self.pending_occlusion_map_rx.as_ref() else {
            return;
        };
        let Some(readback) = self.occlusion_readback_buffer.as_ref() else {
            self.pending_occlusion_query_count = 0;
            self.pending_occlusion_query_keys.clear();
            self.pending_occlusion_map_rx = None;
            return;
        };
        match rx.try_recv() {
            Ok(Ok(())) => {
                let byte_len = (query_count * 8) as u64;
                let data = readback.slice(0..byte_len).get_mapped_range();
                let mut visible = 0u32;
                for (i, bytes) in data.chunks_exact(8).enumerate() {
                    let samples =
                        u64::from_le_bytes(bytes.try_into().expect("8-byte occlusion sample"));
                    if samples > 0 {
                        visible = visible.saturating_add(1);
                    }
                    if let Some(key) = self.pending_occlusion_query_keys.get(i).copied() {
                        self.occlusion_state.insert(
                            key,
                            OcclusionState {
                                visible_last_frame: samples > 0,
                                last_test_frame: self.occlusion_frame,
                            },
                        );
                    }
                }
                drop(data);
                readback.unmap();
                self.last_occlusion_queried = query_count as u32;
                self.last_occlusion_visible = visible;
                self.last_occlusion_culled = (query_count as u32).saturating_sub(visible);
                self.pending_occlusion_query_count = 0;
                self.pending_occlusion_query_keys.clear();
                self.pending_occlusion_map_rx = None;
            }
            Ok(Err(_)) | Err(TryRecvError::Disconnected) => {
                readback.unmap();
                self.pending_occlusion_query_count = 0;
                self.pending_occlusion_query_keys.clear();
                self.pending_occlusion_map_rx = None;
            }
            Err(TryRecvError::Empty) => {}
        }
    }

    pub(super) fn resolve_mesh_range(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &ResourceStore,
        mesh_id: MeshID,
        source: &str,
        static_mesh_lookup: Option<StaticMeshLookup>,
    ) -> Option<MeshAssetRange> {
        if let Some(range) = self.builtin_mesh_ranges.get(source).copied() {
            let (bounds_center, bounds_radius) = self
                .builtin_mesh_bounds
                .get(source)
                .copied()
                .unwrap_or(([0.0, 0.0, 0.0], 1.0));
            return Some(MeshAssetRange {
                full: range,
                surface_ranges: Arc::from([range]),
                meshlets: self
                    .builtin_meshlets
                    .get(source)
                    .cloned()
                    .unwrap_or_else(|| Arc::from([])),
                lods: Arc::from([]),
                bounds_center,
                bounds_radius,
                blend_shape_delta_start: 0,
                blend_shape_target_count: 0,
                blend_shape_vertex_start: 0,
                blend_shape_vertex_count: 0,
            });
        }
        let revision = resources.mesh_revision(mesh_id);
        if let Some((cached_revision, range)) = self.custom_mesh_ranges.get(&mesh_id).cloned()
            && cached_revision == revision
        {
            return Some(range);
        }
        let decoded = if let Some(mesh) = resources.runtime_mesh_data_by_id(mesh_id) {
            load_mesh_from_source_no_dynamic_lods(source, static_mesh_lookup, Some(mesh))?
        } else {
            let runtime_mesh = resources.runtime_mesh_data(source);
            if let Some(mesh) = runtime_mesh {
                load_mesh_from_source_no_dynamic_lods(source, static_mesh_lookup, Some(mesh))?
            } else {
                load_mesh_from_source(
                    source,
                    static_mesh_lookup,
                    None,
                    self.meshlets_enabled && self.dev_meshlets,
                )?
            }
        };
        let range = self.append_mesh_data(device, queue, source, decoded)?;
        self.custom_mesh_ranges
            .insert(mesh_id, (revision, range.clone()));
        Some(range)
    }

    pub(super) fn resolve_builtin_mesh_asset(&self, source: &str) -> Option<MeshAssetRange> {
        let full = self.builtin_mesh_ranges.get(source).copied()?;
        let meshlets = self
            .builtin_meshlets
            .get(source)
            .cloned()
            .unwrap_or_else(|| Arc::from([]));
        let (bounds_center, bounds_radius) = self
            .builtin_mesh_bounds
            .get(source)
            .copied()
            .unwrap_or(([0.0, 0.0, 0.0], 1.0));
        Some(MeshAssetRange {
            full,
            surface_ranges: Arc::from([full]),
            meshlets,
            lods: Arc::from([]),
            bounds_center,
            bounds_radius,
            blend_shape_delta_start: 0,
            blend_shape_target_count: 0,
            blend_shape_vertex_start: 0,
            blend_shape_vertex_count: 0,
        })
    }

    pub(super) fn append_mesh_data(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _source: &str,
        decoded: DecodedMesh,
    ) -> Option<MeshAssetRange> {
        if decoded.vertices.is_empty() || decoded.indices.is_empty() {
            return None;
        }
        let DecodedMesh {
            vertices: decoded_vertices,
            indices: decoded_indices,
            surface_ranges: decoded_surface_ranges,
            blend_shapes: decoded_blend_shapes,
            meshlets: decoded_meshlets,
            lods: decoded_lods,
            has_skinning: _,
        } = decoded;
        let base_vertex = self.mesh_vertices.len() as u32;
        let index_start = self.mesh_indices.len() as u32;
        let index_count = decoded_indices.len() as u32;

        let (bounds_center, bounds_radius) = mesh_bounds_from_vertices(&decoded_vertices)?;
        let surface_ranges = if decoded_surface_ranges.is_empty() {
            vec![MeshRange {
                index_start,
                index_count,
                base_vertex: 0,
            }]
        } else {
            decoded_surface_ranges
                .iter()
                .copied()
                .map(|range| MeshRange {
                    index_start: index_start + range.index_start,
                    index_count: range.index_count,
                    base_vertex: 0,
                })
                .collect()
        };
        let added_vertices: Vec<SkinnedMeshVertex> = decoded_vertices
            .iter()
            .map(pack_skinned_mesh_vertex)
            .collect();
        let added_rigid_vertices: Vec<RigidMeshVertex> = decoded_vertices
            .iter()
            .map(pack_rigid_mesh_vertex)
            .collect();
        let mut added_indices = Vec::with_capacity(decoded_indices.len());
        for idx in decoded_indices {
            added_indices.push(idx + base_vertex);
        }

        let new_vertex_len = self.mesh_vertices.len() + added_vertices.len();
        let new_index_len = self.mesh_indices.len() + added_indices.len();
        self.ensure_mesh_buffer_capacity(device, queue, new_vertex_len, new_index_len);

        let vertex_offset =
            self.mesh_vertices.len() as u64 * std::mem::size_of::<SkinnedMeshVertex>() as u64;
        let rigid_vertex_offset =
            self.rigid_mesh_vertices.len() as u64 * std::mem::size_of::<RigidMeshVertex>() as u64;
        let index_offset = self.mesh_indices.len() as u64 * std::mem::size_of::<u32>() as u64;

        self.mesh_vertices.extend_from_slice(&added_vertices);
        self.rigid_mesh_vertices
            .extend_from_slice(&added_rigid_vertices);
        self.mesh_indices.extend_from_slice(&added_indices);

        queue.write_buffer(
            &self.vertex_buffer,
            vertex_offset,
            bytemuck::cast_slice(&added_vertices),
        );
        queue.write_buffer(
            &self.rigid_vertex_buffer,
            rigid_vertex_offset,
            bytemuck::cast_slice(&added_rigid_vertices),
        );
        queue.write_buffer(
            &self.index_buffer,
            index_offset,
            bytemuck::cast_slice(&added_indices),
        );

        let blend_shape_delta_start = self.blend_shape_deltas.len() as u32;
        let blend_shape_target_count = decoded_blend_shapes.len() as u32;
        let blend_shape_vertex_start = base_vertex;
        let blend_shape_vertex_count = decoded_vertices.len() as u32;
        if !decoded_blend_shapes.is_empty() {
            let added_delta_count = decoded_blend_shapes.len() * decoded_vertices.len();
            let old_delta_len = self.blend_shape_deltas.len();
            self.ensure_blend_shape_delta_capacity(
                device,
                queue,
                old_delta_len + added_delta_count,
            );
            self.blend_shape_deltas.reserve(added_delta_count);
            for shape in &decoded_blend_shapes {
                for vertex_index in 0..decoded_vertices.len() {
                    let vertex = shape.vertices.get(vertex_index).copied();
                    self.blend_shape_deltas.push(BlendShapeDeltaGpu {
                        position_delta: vertex
                            .map(|v| {
                                [
                                    v.position_delta[0],
                                    v.position_delta[1],
                                    v.position_delta[2],
                                    0.0,
                                ]
                            })
                            .unwrap_or([0.0; 4]),
                        normal_delta: vertex
                            .map(|v| [v.normal_delta[0], v.normal_delta[1], v.normal_delta[2], 0.0])
                            .unwrap_or([0.0; 4]),
                    });
                }
            }
            queue.write_buffer(
                &self.blend_shape_delta_buffer,
                old_delta_len as u64 * std::mem::size_of::<BlendShapeDeltaGpu>() as u64,
                bytemuck::cast_slice(&self.blend_shape_deltas[old_delta_len..]),
            );
        }

        let full = MeshRange {
            index_start,
            index_count,
            base_vertex: 0,
        };

        let meshlets: Vec<MeshletRange> = decoded_meshlets
            .iter()
            .copied()
            .filter_map(|meshlet| {
                if meshlet.index_count == 0 {
                    return None;
                }
                Some(MeshletRange {
                    index_start: index_start + meshlet.index_start,
                    index_count: meshlet.index_count,
                    center: meshlet.center,
                    radius: meshlet.radius.max(0.0),
                })
            })
            .collect();
        let meshlets_arc: Arc<[MeshletRange]> = Arc::from(meshlets);
        let surface_ranges_arc: Arc<[MeshRange]> = Arc::from(surface_ranges);
        let packed_lods = self.append_packed_lod_data(AppendPackedLodDataArgs {
            device,
            queue,
            vertices: &decoded_vertices,
            mesh_indices: &added_indices,
            base_vertex,
            decoded_lods: &decoded_lods,
            decoded_surfaces: &decoded_surface_ranges,
        });
        let lods = build_mesh_lod_ranges(BuildMeshLodRangesArgs {
            index_start,
            index_count,
            decoded_surfaces: &decoded_surface_ranges,
            uploaded_surfaces: &surface_ranges_arc,
            decoded_meshlets: &decoded_meshlets,
            uploaded_meshlets: &meshlets_arc,
            decoded_lods: &decoded_lods,
            packed_lods: &packed_lods,
        });

        Some(MeshAssetRange {
            full,
            surface_ranges: surface_ranges_arc,
            meshlets: meshlets_arc,
            lods: Arc::from(lods),
            bounds_center,
            bounds_radius,
            blend_shape_delta_start,
            blend_shape_target_count,
            blend_shape_vertex_start,
            blend_shape_vertex_count,
        })
    }

    fn append_packed_lod_data(
        &mut self,
        args: AppendPackedLodDataArgs<'_>,
    ) -> Vec<Option<PackedMeshLodRange>> {
        let AppendPackedLodDataArgs {
            device,
            queue,
            vertices,
            mesh_indices,
            base_vertex,
            decoded_lods,
            decoded_surfaces,
        } = args;
        if decoded_lods.len() <= 1 {
            return vec![None; decoded_lods.len()];
        }
        let param_upload_start = self.packed_lod_params.len();
        self.ensure_packed_lod_param_capacity(
            device,
            queue,
            param_upload_start + decoded_lods.len().saturating_sub(1),
        );
        let mut out = Vec::with_capacity(decoded_lods.len());
        for (lod_index, lod) in decoded_lods.iter().enumerate() {
            if lod_index == 0 || lod.index_count == 0 {
                out.push(None);
                continue;
            }
            let src_start = lod.index_start as usize;
            let src_end = src_start
                .saturating_add(lod.index_count as usize)
                .min(mesh_indices.len());
            if src_start >= src_end {
                out.push(None);
                continue;
            }
            let src_indices = &mesh_indices[src_start..src_end];
            let Some(param) = packed_lod_param(vertices, src_indices, base_vertex) else {
                out.push(None);
                continue;
            };
            let param_index = self.packed_lod_params.len() as u32;
            self.packed_lod_params.push(param);

            let packed_index_start = self.packed_lod_indices.len() as u32;
            let packed_vertex_start = self.packed_lod_vertices.len() as u32;
            let mut remap: AHashMap<u32, u32> = AHashMap::with_capacity(src_indices.len());
            let mut new_vertices = Vec::with_capacity(src_indices.len());
            let mut new_indices = Vec::with_capacity(src_indices.len());
            for &uploaded_index in src_indices {
                let local_index = uploaded_index.saturating_sub(base_vertex);
                let next_index = packed_vertex_start + new_vertices.len() as u32;
                let packed_index = *remap.entry(local_index).or_insert_with(|| {
                    if let Some(vertex) = vertices.get(local_index as usize) {
                        new_vertices.push(pack_packed_lod_vertex(vertex, &param));
                        next_index
                    } else {
                        0
                    }
                });
                new_indices.push(packed_index);
            }
            if new_vertices.is_empty() || new_indices.is_empty() {
                out.push(None);
                continue;
            }
            self.ensure_packed_lod_buffer_capacity(
                device,
                queue,
                self.packed_lod_vertices.len() + new_vertices.len(),
                self.packed_lod_indices.len() + new_indices.len(),
            );
            let vertex_offset = self.packed_lod_vertices.len() as u64
                * std::mem::size_of::<PackedRigidLodVertex>() as u64;
            let index_offset =
                self.packed_lod_indices.len() as u64 * std::mem::size_of::<u32>() as u64;
            self.packed_lod_vertices.extend_from_slice(&new_vertices);
            self.packed_lod_indices.extend_from_slice(&new_indices);
            queue.write_buffer(
                &self.packed_lod_vertex_buffer,
                vertex_offset,
                bytemuck::cast_slice(&new_vertices),
            );
            queue.write_buffer(
                &self.packed_lod_index_buffer,
                index_offset,
                bytemuck::cast_slice(&new_indices),
            );

            let mut packed_surfaces = Vec::new();
            let surface_start = lod.surface_start as usize;
            let surface_end = surface_start
                .saturating_add(lod.surface_count as usize)
                .min(decoded_surfaces.len());
            for surface in &decoded_surfaces[surface_start..surface_end] {
                let rel_start = surface.index_start.saturating_sub(lod.index_start);
                if rel_start >= lod.index_count {
                    continue;
                }
                packed_surfaces.push(MeshRange {
                    index_start: packed_index_start + rel_start,
                    index_count: surface.index_count.min(lod.index_count - rel_start),
                    base_vertex: 0,
                });
            }
            if packed_surfaces.is_empty() {
                packed_surfaces.push(MeshRange {
                    index_start: packed_index_start,
                    index_count: new_indices.len() as u32,
                    base_vertex: 0,
                });
            }
            out.push(Some(PackedMeshLodRange {
                full: MeshRange {
                    index_start: packed_index_start,
                    index_count: new_indices.len() as u32,
                    base_vertex: 0,
                },
                surface_ranges: Arc::from(packed_surfaces),
                param_index,
            }));
        }
        if self.packed_lod_params.len() > param_upload_start {
            let offset =
                param_upload_start as u64 * std::mem::size_of::<PackedLodParamGpu>() as u64;
            queue.write_buffer(
                &self.packed_lod_param_buffer,
                offset,
                bytemuck::cast_slice(&self.packed_lod_params[param_upload_start..]),
            );
        }
        out
    }

    pub(super) fn ensure_blend_shape_delta_capacity(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        needed: usize,
    ) {
        if needed <= self.blend_shape_delta_capacity {
            return;
        }
        let mut cap = self.blend_shape_delta_capacity.max(1);
        while cap < needed {
            cap *= 2;
        }
        let old_buffer = self.blend_shape_delta_buffer.clone();
        let old_size =
            self.blend_shape_deltas.len() as u64 * std::mem::size_of::<BlendShapeDeltaGpu>() as u64;
        self.blend_shape_delta_capacity = cap;
        self.blend_shape_delta_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_blend_shape_deltas"),
            size: (cap * std::mem::size_of::<BlendShapeDeltaGpu>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        if old_size > 0 {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("perro_blend_shape_delta_growth_copy"),
            });
            encoder.copy_buffer_to_buffer(
                &old_buffer,
                0,
                &self.blend_shape_delta_buffer,
                0,
                old_size,
            );
            queue.submit(Some(encoder.finish()));
        }
        self.rebuild_camera_bind_groups(device);
    }

    pub(super) fn ensure_mesh_buffer_capacity(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        needed_vertices: usize,
        needed_indices: usize,
    ) {
        let mut grew = false;

        if needed_vertices > self.vertex_capacity {
            let mut cap = self.vertex_capacity.max(1);
            while cap < needed_vertices {
                cap *= 2;
            }
            self.vertex_capacity = cap;
            self.rigid_vertex_capacity = cap;
            grew = true;
        }

        if needed_indices > self.index_capacity {
            let mut cap = self.index_capacity.max(1);
            while cap < needed_indices {
                cap *= 2;
            }
            self.index_capacity = cap;
            grew = true;
        }

        if grew {
            let old_vertex_buffer = self.vertex_buffer.clone();
            let old_rigid_vertex_buffer = self.rigid_vertex_buffer.clone();
            let old_index_buffer = self.index_buffer.clone();
            let old_vertex_size =
                self.mesh_vertices.len() as u64 * std::mem::size_of::<SkinnedMeshVertex>() as u64;
            let old_rigid_vertex_size = self.rigid_mesh_vertices.len() as u64
                * std::mem::size_of::<RigidMeshVertex>() as u64;
            let old_index_size = self.mesh_indices.len() as u64 * std::mem::size_of::<u32>() as u64;
            self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_mesh_vertices"),
                size: (self.vertex_capacity * std::mem::size_of::<SkinnedMeshVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            self.rigid_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_mesh_vertices_rigid"),
                size: (self.rigid_vertex_capacity * std::mem::size_of::<RigidMeshVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            self.index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_mesh_indices"),
                size: (self.index_capacity * std::mem::size_of::<u32>()) as u64,
                usage: wgpu::BufferUsages::INDEX
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            if old_vertex_size > 0 || old_rigid_vertex_size > 0 || old_index_size > 0 {
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("perro_mesh_buffer_growth_copy"),
                });
                if old_vertex_size > 0 {
                    encoder.copy_buffer_to_buffer(
                        &old_vertex_buffer,
                        0,
                        &self.vertex_buffer,
                        0,
                        old_vertex_size,
                    );
                }
                if old_rigid_vertex_size > 0 {
                    encoder.copy_buffer_to_buffer(
                        &old_rigid_vertex_buffer,
                        0,
                        &self.rigid_vertex_buffer,
                        0,
                        old_rigid_vertex_size,
                    );
                }
                if old_index_size > 0 {
                    encoder.copy_buffer_to_buffer(
                        &old_index_buffer,
                        0,
                        &self.index_buffer,
                        0,
                        old_index_size,
                    );
                }
                queue.submit([encoder.finish()]);
            }
        }
    }

    pub(super) fn ensure_packed_lod_buffer_capacity(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        needed_vertices: usize,
        needed_indices: usize,
    ) {
        let mut vertex_grew = false;
        let mut index_grew = false;
        if needed_vertices > self.packed_lod_vertex_capacity {
            while self.packed_lod_vertex_capacity < needed_vertices {
                self.packed_lod_vertex_capacity = self.packed_lod_vertex_capacity.max(1) * 2;
            }
            vertex_grew = true;
        }
        if needed_indices > self.packed_lod_index_capacity {
            while self.packed_lod_index_capacity < needed_indices {
                self.packed_lod_index_capacity = self.packed_lod_index_capacity.max(1) * 2;
            }
            index_grew = true;
        }
        if !vertex_grew && !index_grew {
            return;
        }
        let old_vertex_buffer = self.packed_lod_vertex_buffer.clone();
        let old_index_buffer = self.packed_lod_index_buffer.clone();
        let old_vertex_size = self.packed_lod_vertices.len() as u64
            * std::mem::size_of::<PackedRigidLodVertex>() as u64;
        let old_index_size =
            self.packed_lod_indices.len() as u64 * std::mem::size_of::<u32>() as u64;
        if vertex_grew {
            self.packed_lod_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_packed_lod_vertices_rigid"),
                size: (self.packed_lod_vertex_capacity
                    * std::mem::size_of::<PackedRigidLodVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
        }
        if index_grew {
            self.packed_lod_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_packed_lod_indices"),
                size: (self.packed_lod_index_capacity * std::mem::size_of::<u32>()) as u64,
                usage: wgpu::BufferUsages::INDEX
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perro_packed_lod_growth_copy"),
        });
        if vertex_grew && old_vertex_size > 0 {
            encoder.copy_buffer_to_buffer(
                &old_vertex_buffer,
                0,
                &self.packed_lod_vertex_buffer,
                0,
                old_vertex_size,
            );
        }
        if index_grew && old_index_size > 0 {
            encoder.copy_buffer_to_buffer(
                &old_index_buffer,
                0,
                &self.packed_lod_index_buffer,
                0,
                old_index_size,
            );
        }
        queue.submit([encoder.finish()]);
    }
}

fn packed_lod_param(
    vertices: &[MeshVertex],
    uploaded_indices: &[u32],
    base_vertex: u32,
) -> Option<PackedLodParamGpu> {
    let mut pos_min = [f32::INFINITY; 3];
    let mut pos_max = [f32::NEG_INFINITY; 3];
    let mut uv_min = [f32::INFINITY; 2];
    let mut uv_max = [f32::NEG_INFINITY; 2];
    let mut any = false;
    for &uploaded_index in uploaded_indices {
        let local_index = uploaded_index.saturating_sub(base_vertex);
        let Some(vertex) = vertices.get(local_index as usize) else {
            continue;
        };
        any = true;
        for axis in 0..3 {
            pos_min[axis] = pos_min[axis].min(vertex.pos[axis]);
            pos_max[axis] = pos_max[axis].max(vertex.pos[axis]);
        }
        for axis in 0..2 {
            uv_min[axis] = uv_min[axis].min(vertex.uv[axis]);
            uv_max[axis] = uv_max[axis].max(vertex.uv[axis]);
        }
    }
    if !any {
        return None;
    }
    let pos_extent = [
        (pos_max[0] - pos_min[0]).max(1.0e-9),
        (pos_max[1] - pos_min[1]).max(1.0e-9),
        (pos_max[2] - pos_min[2]).max(1.0e-9),
        0.0,
    ];
    Some(PackedLodParamGpu {
        pos_min: [pos_min[0], pos_min[1], pos_min[2], 0.0],
        pos_extent,
        uv_min_extent: [
            uv_min[0],
            uv_min[1],
            (uv_max[0] - uv_min[0]).max(1.0e-9),
            (uv_max[1] - uv_min[1]).max(1.0e-9),
        ],
    })
}

fn pack_packed_lod_vertex(vertex: &MeshVertex, param: &PackedLodParamGpu) -> PackedRigidLodVertex {
    PackedRigidLodVertex {
        pos: [
            pack_unorm16_local(vertex.pos[0], param.pos_min[0], param.pos_extent[0]),
            pack_unorm16_local(vertex.pos[1], param.pos_min[1], param.pos_extent[1]),
            pack_unorm16_local(vertex.pos[2], param.pos_min[2], param.pos_extent[2]),
            0,
        ],
        normal: pack_normal_snorm8x4(vertex.normal),
        uv: [
            pack_unorm16_local(vertex.uv[0], param.uv_min_extent[0], param.uv_min_extent[2]),
            pack_unorm16_local(vertex.uv[1], param.uv_min_extent[1], param.uv_min_extent[3]),
        ],
    }
}
