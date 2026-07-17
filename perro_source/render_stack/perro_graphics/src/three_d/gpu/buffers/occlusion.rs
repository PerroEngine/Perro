use super::*;

impl Gpu3D {
    pub(in super::super) fn build_hiz_from_depth(&self, encoder: &mut wgpu::CommandEncoder) {
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

    pub(in super::super) fn rebuild_hiz_bind_groups(&mut self, device: &wgpu::Device) {
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
    pub(in super::super) fn rebuild_hiz_spd_bind_groups(&mut self, device: &wgpu::Device) {
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
            let Ok(mut data) = params_buffer.slice(..).get_mapped_range_mut() else {
                params_buffer.unmap();
                return;
            };
            data.copy_from_slice(bytemuck::bytes_of(&params));
            drop(data);
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

    pub(in super::super) fn request_hiz_debug_map_async(&mut self) {
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

    pub(in super::super) fn consume_hiz_debug_results(&mut self) {
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
                let Ok(data) = self
                    .hiz_debug_readback_buffer
                    .slice(0..byte_len)
                    .get_mapped_range()
                else {
                    self.hiz_debug_readback_buffer.unmap();
                    self.pending_hiz_debug_count = 0;
                    self.pending_hiz_debug_frustum_visible_est = 0;
                    self.pending_hiz_debug_map_rx = None;
                    return;
                };
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

    pub(in super::super) fn should_probe_or_draw(&self, key: u64) -> bool {
        let Some(state) = self.occlusion_state.get(&key) else {
            return true;
        };
        state.visible_last_frame
            || self.occlusion_frame.saturating_sub(state.last_test_frame)
                >= OCCLUSION_PROBE_INTERVAL
    }

    pub(in super::super) fn push_occlusion_query_key(&mut self, key: u64) -> u32 {
        let query = self.occlusion_query_keys_this_frame.len() as u32;
        self.occlusion_query_keys_this_frame.push(key);
        query
    }

    pub(in super::super) fn ensure_occlusion_query_capacity(
        &mut self,
        device: &wgpu::Device,
        needed: u32,
    ) {
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

    pub(in super::super) fn request_occlusion_map_async(&mut self) {
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

    pub(in super::super) fn consume_occlusion_results(&mut self) {
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
                let Ok(data) = readback.slice(0..byte_len).get_mapped_range() else {
                    readback.unmap();
                    self.pending_occlusion_query_count = 0;
                    self.pending_occlusion_query_keys.clear();
                    self.pending_occlusion_map_rx = None;
                    return;
                };
                let mut visible = 0u32;
                for (i, bytes) in data.chunks_exact(8).enumerate() {
                    let mut sample_bytes = [0u8; 8];
                    sample_bytes.copy_from_slice(bytes);
                    let samples = u64::from_le_bytes(sample_bytes);
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
}
