use super::*;

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
            vec![255u8, 255, 255, 255],
            1,
            1,
            "__fallback__".to_string(),
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
        static_texture_lookup: Option<StaticTextureLookup>,
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

        let decoded = if source == "__default__" {
            Some((vec![255u8, 255, 255, 255], 1, 1))
        } else if let Some(lookup) = static_texture_lookup {
            let source_hash = perro_ids::parse_hashed_source_uri(source.as_str())
                .unwrap_or_else(|| perro_ids::string_to_u64(source.as_str()));
            let bytes = lookup(source_hash);
            if !bytes.is_empty() {
                decode_ptex(bytes)
            } else {
                load_texture_rgba(source.as_str())
            }
        } else {
            load_texture_rgba(source.as_str())
        };
        let Some((rgba, width, height)) = decoded else {
            self.material_textures.remove(&slot);
            return;
        };
        let cached = create_cached_material_texture(
            device,
            queue,
            &self.material_texture_bgl,
            rgba,
            width,
            height,
            source,
        );
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

    pub(super) fn ensure_instance_material_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: usize,
    ) {
        if needed <= self.instance_material_capacity {
            return;
        }
        let mut new_capacity = self.instance_material_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.instance_material_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_mesh_instance_materials"),
            size: (new_capacity * std::mem::size_of::<MaterialInstanceGpu>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.instance_material_capacity = new_capacity;
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
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.multimesh_instance_capacity = new_capacity;
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
            ],
        });
        self.shadow_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_shadow_camera3d_bg"),
            layout: &self.camera_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.shadow_camera_buffer.as_entire_binding(),
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
            ],
        });
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
            ],
        });
        self.rigid_shadow_camera_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("perro_shadow_camera3d_rigid_bg"),
                layout: &self.rigid_camera_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.shadow_camera_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.custom_params_meta_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.custom_params_values_buffer.as_entire_binding(),
                    },
                ],
            });
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
            ],
        });
        self.camera_bind_group_generation = self.camera_bind_group_generation.wrapping_add(1);
        if self.camera_bind_group_generation == 0 {
            self.camera_bind_group_generation = 1;
        }
        self.multimesh_bind_group_generation = self.multimesh_bind_group_generation.wrapping_add(1);
        if self.multimesh_bind_group_generation == 0 {
            self.multimesh_bind_group_generation = 1;
        }
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
            size: (new_capacity * std::mem::size_of::<[[f32; 4]; 4]>()) as u64,
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
        self.frustum_cull_items_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_frustum_cull_items"),
            size: (new_capacity * std::mem::size_of::<FrustumCullItemGpu>()) as u64,
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
                    resource: self.frustum_cull_items_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
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
                    resource: self.frustum_cull_items_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.indirect_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
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
            });
        }
        let revision = resources.mesh_revision(mesh_id);
        if let Some((cached_revision, range)) = self.custom_mesh_ranges.get(&mesh_id).cloned()
            && cached_revision == revision
        {
            return Some(range);
        }
        let decoded = if let Some(mesh) = resources.runtime_mesh_data_by_id(mesh_id) {
            load_mesh_from_source(
                source,
                static_mesh_lookup,
                Some(mesh),
                self.meshlets_enabled && self.dev_meshlets,
            )?
        } else {
            load_mesh_from_source(
                source,
                static_mesh_lookup,
                resources.runtime_mesh_data(source),
                self.meshlets_enabled && self.dev_meshlets,
            )?
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
        let base_vertex = self.mesh_vertices.len() as u32;
        let index_start = self.mesh_indices.len() as u32;
        let index_count = decoded.indices.len() as u32;

        let (bounds_center, bounds_radius) = mesh_bounds_from_vertices(&decoded.vertices)?;
        let decoded_surface_ranges = decoded.surface_ranges.clone();
        let decoded_meshlets = decoded.meshlets.clone();
        let decoded_lods = decoded.lods.clone();
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
        let added_vertices = decoded.vertices;
        let added_rigid_vertices: Vec<RigidMeshVertex> = added_vertices
            .iter()
            .map(|v| RigidMeshVertex {
                pos: v.pos,
                normal: v.normal,
                uv: v.uv,
            })
            .collect();
        let mut added_indices = Vec::with_capacity(decoded.indices.len());
        for idx in decoded.indices {
            added_indices.push(idx + base_vertex);
        }

        let new_vertex_len = self.mesh_vertices.len() + added_vertices.len();
        let new_index_len = self.mesh_indices.len() + added_indices.len();
        self.ensure_mesh_buffer_capacity(device, queue, new_vertex_len, new_index_len);

        let vertex_offset =
            self.mesh_vertices.len() as u64 * std::mem::size_of::<MeshVertex>() as u64;
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
        let lods = build_mesh_lod_ranges(
            index_start,
            index_count,
            &decoded_surface_ranges,
            &surface_ranges_arc,
            &decoded_meshlets,
            &meshlets_arc,
            &decoded_lods,
        );

        Some(MeshAssetRange {
            full,
            surface_ranges: surface_ranges_arc,
            meshlets: meshlets_arc,
            lods: Arc::from(lods),
            bounds_center,
            bounds_radius,
        })
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
                self.mesh_vertices.len() as u64 * std::mem::size_of::<MeshVertex>() as u64;
            let old_rigid_vertex_size = self.rigid_mesh_vertices.len() as u64
                * std::mem::size_of::<RigidMeshVertex>() as u64;
            let old_index_size = self.mesh_indices.len() as u64 * std::mem::size_of::<u32>() as u64;
            self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_mesh_vertices"),
                size: (self.vertex_capacity * std::mem::size_of::<MeshVertex>()) as u64,
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
}
