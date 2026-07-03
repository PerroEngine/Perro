use super::*;

impl Gpu3D {
    pub(super) fn should_run_frustum_cull(&self) -> bool {
        let mut min_batches = FRUSTUM_CULL_MIN_BATCHES;
        let mut min_instances = FRUSTUM_CULL_MIN_INSTANCES;
        if self.cpu_occlusion_enabled
            && self.last_occlusion_queried >= FRUSTUM_CULL_HIGH_VISIBLE_MIN_SAMPLES
        {
            let visible_ratio =
                self.last_occlusion_visible as f32 / self.last_occlusion_queried as f32;
            if visible_ratio >= FRUSTUM_CULL_HIGH_VISIBLE_RATIO {
                min_batches = FRUSTUM_CULL_HIGH_VISIBLE_MIN_BATCHES;
                min_instances = FRUSTUM_CULL_HIGH_VISIBLE_MIN_INSTANCES;
            }
        }
        self.frustum_cull_enabled
            && !self.draw_batches.is_empty()
            && (self.draw_batches.len() >= min_batches
                || self.staged_instance_transforms.len() >= min_instances)
    }

    #[inline]
    pub(super) fn should_run_hiz_occlusion(&self, frustum_cull_active: bool) -> bool {
        frustum_cull_active
            && self.gpu_occlusion_enabled
            && (self.draw_batches.len() >= HIZ_OCCLUSION_MIN_BATCHES
                || self.staged_instance_transforms.len() >= HIZ_OCCLUSION_MIN_INSTANCES)
    }

    #[inline]
    pub(super) fn should_run_depth_prepass(
        &self,
        depth_prepass_needed: bool,
        hiz_active: bool,
    ) -> bool {
        depth_prepass_needed
            || hiz_active
            || (self.draw_batches.len() >= DEPTH_PREPASS_MIN_BATCHES
                || self.staged_instance_transforms.len() >= DEPTH_PREPASS_MIN_INSTANCES)
    }

    // Rebuild both cull-item halves from the current batch topology and upload
    // both storage buffers. Used on any change that alters batch count/order.
    pub(super) fn rebuild_frustum_cull_items(&mut self, queue: &wgpu::Queue) {
        self.frustum_cull_static_staging.clear();
        self.frustum_cull_static_staging
            .reserve(self.draw_batches.len());
        self.frustum_cull_dynamic_staging.clear();
        self.frustum_cull_dynamic_staging
            .reserve(self.draw_batches.len());
        for batch in &self.draw_batches {
            let instance = &self.staged_instance_transforms[batch.instance_start as usize];
            let model_cols = model_cols_from_affine_rows(instance);
            self.frustum_cull_static_staging.push(FrustumCullStaticGpu {
                local_center_radius: [
                    batch.local_center[0],
                    batch.local_center[1],
                    batch.local_center[2],
                    batch.local_radius.max(0.0),
                ],
                cull_flags: [
                    if batch.disable_hiz_occlusion {
                        CULL_FLAG_DISABLE_HIZ_OCCLUSION
                    } else {
                        0
                    },
                    0,
                    0,
                    0,
                ],
            });
            self.frustum_cull_dynamic_staging
                .push(FrustumCullDynamicGpu {
                    model_0: model_cols[0],
                    model_1: model_cols[1],
                    model_2: model_cols[2],
                    model_3: model_cols[3],
                });
        }
        queue.write_buffer(
            &self.frustum_cull_static_buffer,
            0,
            bytemuck::cast_slice(&self.frustum_cull_static_staging),
        );
        queue.write_buffer(
            &self.frustum_cull_dynamic_buffer,
            0,
            bytemuck::cast_slice(&self.frustum_cull_dynamic_staging),
        );
    }

    #[inline]
    pub(super) fn write_frustum_params_if_needed(
        &mut self,
        queue: &wgpu::Queue,
        frustum: &[Vec4; 6],
    ) -> bool {
        let mut planes = [[0.0f32; 4]; 6];
        for (dst, plane) in planes.iter_mut().zip(frustum.iter()) {
            *dst = [plane.x, plane.y, plane.z, plane.w];
        }
        let params = FrustumCullParamsGpu {
            planes,
            draw_count: self.draw_batches.len() as u32,
            _pad: [0; 3],
        };
        if self.last_frustum_params == Some(params) {
            return false;
        }
        queue.write_buffer(
            &self.frustum_cull_params_buffer,
            0,
            bytemuck::bytes_of(&params),
        );
        self.last_frustum_params = Some(params);
        true
    }

    #[inline]
    pub(super) fn write_hiz_params_if_needed(
        &mut self,
        queue: &wgpu::Queue,
        uniform: &Scene3DUniform,
        draw_count: usize,
    ) -> bool {
        let params = HizCullParamsGpu {
            view_proj: uniform.view_proj,
            draw_count: draw_count as u32,
            hiz_mip_count: self.hiz_mip_count,
            hiz_width: self.hiz_size.0,
            hiz_height: self.hiz_size.1,
            aspect: self.last_aspect,
            proj_y_scale: self.last_proj_y_scale,
            depth_bias: HIZ_OCCLUSION_BIAS,
            _pad: 0,
        };
        if self.last_hiz_params == Some(params) {
            return false;
        }
        queue.write_buffer(&self.hiz_cull_params, 0, bytemuck::bytes_of(&params));
        self.last_hiz_params = Some(params);
        true
    }

    pub(super) fn compact_sorted_draw_batches(&mut self, draw_count: usize) {
        if draw_count == 0 {
            self.last_draw_instance_spans.clear();
            self.last_draw_instance_span_ranges.clear();
            return;
        }
        if self.draw_batches.is_empty() {
            self.last_draw_instance_spans.clear();
            self.last_draw_instance_span_ranges.clear();
            self.last_draw_instance_span_ranges.reserve(draw_count);
            for _ in 0..draw_count {
                self.last_draw_instance_span_ranges.push(0..0);
            }
            return;
        }
        if self.staged_instance_transforms.is_empty() {
            return;
        }

        let src_instance_count = self.staged_instance_transforms.len();
        let mut instance_owner = std::mem::take(&mut self.compact_instance_owner_scratch);
        instance_owner.clear();
        instance_owner.resize(src_instance_count, u32::MAX);
        if self.last_draw_instance_span_ranges.len() == draw_count {
            for (draw_index, span_range) in self.last_draw_instance_span_ranges.iter().enumerate() {
                if span_range.start > span_range.end
                    || span_range.end > self.last_draw_instance_spans.len()
                {
                    continue;
                }
                for span in self.last_draw_instance_spans[span_range.clone()].iter() {
                    let start = span.start as usize;
                    let end = span.end as usize;
                    if start >= end || end > src_instance_count {
                        continue;
                    }
                    for owner in &mut instance_owner[start..end] {
                        *owner = draw_index as u32;
                    }
                }
            }
        }

        let src_transforms = std::mem::take(&mut self.staged_instance_transforms);
        let src_rigid_meta = std::mem::take(&mut self.staged_rigid_instance_meta);
        let src_skinned_meta = std::mem::take(&mut self.staged_skinned_instance_meta);
        let src_batches = std::mem::take(&mut self.draw_batches);

        // Double-buffer: reuse the scratch vectors from the previous rebuild
        // (already at/near the needed capacity) instead of allocating fresh
        // ones. Cleared, then refilled via extend_from_slice/push below.
        let mut dst_transforms = std::mem::take(&mut self.compact_dst_transforms_scratch);
        dst_transforms.clear();
        dst_transforms.reserve(src_transforms.len());
        let mut dst_rigid_meta = std::mem::take(&mut self.compact_dst_rigid_meta_scratch);
        dst_rigid_meta.clear();
        dst_rigid_meta.reserve(src_rigid_meta.len());
        let mut dst_skinned_meta = std::mem::take(&mut self.compact_dst_skinned_meta_scratch);
        dst_skinned_meta.clear();
        dst_skinned_meta.reserve(src_skinned_meta.len());
        let mut dst_batches = std::mem::take(&mut self.compact_dst_batches_scratch);
        dst_batches.clear();
        dst_batches.reserve(src_batches.len());
        let mut spans_per_draw = std::mem::take(&mut self.compact_spans_per_draw_scratch);
        if spans_per_draw.len() < draw_count {
            spans_per_draw.resize_with(draw_count, Vec::new);
        } else {
            spans_per_draw.truncate(draw_count);
        }
        for spans in spans_per_draw.iter_mut() {
            spans.clear();
        }

        let mut batch_index = 0usize;
        while batch_index < src_batches.len() {
            let mut merged_batch = src_batches[batch_index].clone();
            let batch_group_start = &src_batches[batch_index];
            let dst_instance_start = dst_transforms.len() as u32;
            let mut merged_instance_count = 0u32;
            let mut merged_disable_hiz = false;
            let mut scan = batch_index;
            while scan < src_batches.len()
                && (scan == batch_index
                    || Self::can_compact_merge_batches(batch_group_start, &src_batches[scan]))
            {
                let batch = &src_batches[scan];
                let src_start = batch.instance_start as usize;
                let src_end = (batch.instance_start + batch.instance_count) as usize;
                if src_start < src_end
                    && src_end <= src_transforms.len()
                    && src_end <= src_rigid_meta.len()
                    && src_end <= src_skinned_meta.len()
                {
                    let dst_copy_start = dst_transforms.len() as u32;
                    dst_transforms.extend_from_slice(&src_transforms[src_start..src_end]);
                    dst_rigid_meta.extend_from_slice(&src_rigid_meta[src_start..src_end]);
                    dst_skinned_meta.extend_from_slice(&src_skinned_meta[src_start..src_end]);
                    let copied_count = (src_end - src_start) as u32;
                    merged_instance_count = merged_instance_count.saturating_add(copied_count);

                    let mut run_owner = u32::MAX;
                    let mut run_src_start = batch.instance_start;
                    let src_batch_end = batch.instance_start.saturating_add(batch.instance_count);
                    for src_instance in batch.instance_start..src_batch_end {
                        let owner = instance_owner[src_instance as usize];
                        if owner != run_owner {
                            if run_owner != u32::MAX {
                                let run_start =
                                    dst_copy_start + (run_src_start - batch.instance_start);
                                let run_end =
                                    dst_copy_start + (src_instance - batch.instance_start);
                                Self::push_compacted_draw_span(
                                    &mut spans_per_draw,
                                    run_owner as usize,
                                    run_start..run_end,
                                );
                            }
                            run_owner = owner;
                            run_src_start = src_instance;
                        }
                    }
                    if run_owner != u32::MAX {
                        let run_start = dst_copy_start + (run_src_start - batch.instance_start);
                        let run_end = dst_copy_start + (src_batch_end - batch.instance_start);
                        Self::push_compacted_draw_span(
                            &mut spans_per_draw,
                            run_owner as usize,
                            run_start..run_end,
                        );
                    }
                }
                merged_disable_hiz |= batch.disable_hiz_occlusion;
                scan += 1;
            }

            if merged_instance_count > 0 {
                merged_batch.instance_start = dst_instance_start;
                merged_batch.instance_count = merged_instance_count;
                merged_batch.disable_hiz_occlusion = merged_disable_hiz;
                if merged_instance_count > 1 {
                    merged_batch.local_center = [0.0, 0.0, 0.0];
                    merged_batch.local_radius = 1.0e9;
                    merged_batch.disable_hiz_occlusion = true;
                }
                dst_batches.push(merged_batch);
            }
            batch_index = scan;
        }

        // dst_* become the new live staged vectors; the vectors they replace
        // (now empty, taken via mem::take above) become next rebuild's scratch.
        self.compact_dst_transforms_scratch =
            std::mem::replace(&mut self.staged_instance_transforms, dst_transforms);
        self.compact_dst_rigid_meta_scratch =
            std::mem::replace(&mut self.staged_rigid_instance_meta, dst_rigid_meta);
        self.compact_dst_skinned_meta_scratch =
            std::mem::replace(&mut self.staged_skinned_instance_meta, dst_skinned_meta);
        self.compact_dst_batches_scratch = std::mem::replace(&mut self.draw_batches, dst_batches);
        instance_owner.clear();
        self.compact_instance_owner_scratch = instance_owner;

        self.last_draw_instance_spans.clear();
        self.last_draw_instance_span_ranges.clear();
        self.last_draw_instance_span_ranges.reserve(draw_count);
        for spans in spans_per_draw.iter_mut() {
            let start = self.last_draw_instance_spans.len();
            self.last_draw_instance_spans.append(spans);
            let end = self.last_draw_instance_spans.len();
            self.last_draw_instance_span_ranges.push(start..end);
        }
        self.compact_spans_per_draw_scratch = spans_per_draw;
    }

    // Gate the multimesh GPU cull: needs frustum-cull support (same gating as
    // rigid) and enough instances to beat direct draw. Blend batches are still
    // culled but never enter the prepass (handled by the caller).
    #[inline]
    pub(super) fn should_run_multimesh_cull(&self) -> bool {
        self.frustum_cull_supported
            && !self.multimesh_batches.is_empty()
            && self.staged_multimesh_instances.len() >= MULTIMESH_CULL_MIN_INSTANCES
    }

    // Build the GPU-cull side data for the current multimesh batches: per-batch
    // records, per-instance batch ids, identity visible indices (fallback +
    // pre-fill), and the indirect draw records. Uploads all buffers. Assumes
    // batches are already sorted/compacted so each batch owns a contiguous
    // region [instance_start, instance_start+instance_count).
    pub(super) fn rebuild_multimesh_cull_inputs(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        let batch_count = self.multimesh_batches.len();
        let instance_count = self.staged_multimesh_instances.len();
        self.ensure_multimesh_cull_batch_capacity(device, batch_count.max(1));
        self.ensure_multimesh_cull_instance_capacity(device, instance_count.max(1));

        build_multimesh_cull_staging(
            &self.multimesh_batches,
            instance_count,
            &mut self.staged_multimesh_cull_batches,
            &mut self.multimesh_indirect_staging,
            &mut self.staged_multimesh_instance_batch,
            &mut self.staged_multimesh_visible_identity,
        );

        if batch_count > 0 {
            queue.write_buffer(
                &self.multimesh_cull_batch_buffer,
                0,
                bytemuck::cast_slice(&self.staged_multimesh_cull_batches),
            );
            queue.write_buffer(
                &self.multimesh_indirect_buffer,
                0,
                bytemuck::cast_slice(&self.multimesh_indirect_staging),
            );
        }
        if instance_count > 0 {
            queue.write_buffer(
                &self.multimesh_instance_batch_buffer,
                0,
                bytemuck::cast_slice(&self.staged_multimesh_instance_batch),
            );
            // Prime visible_indices as identity so the direct-draw fallback (and
            // any batch the cull skips) reads correct source instances.
            queue.write_buffer(
                &self.multimesh_visible_index_buffer,
                0,
                bytemuck::cast_slice(&self.staged_multimesh_visible_identity),
            );
        }
    }

    #[inline]
    pub(super) fn write_multimesh_cull_params_if_needed(&mut self, queue: &wgpu::Queue) -> bool {
        let params = MultiMeshCullParamsGpu {
            instance_count: self.staged_multimesh_instances.len() as u32,
            batch_count: self.multimesh_batches.len() as u32,
            _pad1: 0,
            _pad2: 0,
        };
        if self.last_multimesh_cull_params == Some(params) {
            return false;
        }
        queue.write_buffer(
            &self.multimesh_cull_params_buffer,
            0,
            bytemuck::bytes_of(&params),
        );
        self.last_multimesh_cull_params = Some(params);
        true
    }

    pub(super) fn compact_sorted_multimesh_batches(&mut self) {
        if self.multimesh_batches.is_empty() || self.staged_multimesh_instances.is_empty() {
            return;
        }

        let src_instances = std::mem::take(&mut self.staged_multimesh_instances);
        let src_batches = std::mem::take(&mut self.multimesh_batches);
        let mut dst_instances = std::mem::take(&mut self.compact_multimesh_dst_instances_scratch);
        dst_instances.clear();
        dst_instances.reserve(src_instances.len());
        let mut dst_batches = std::mem::take(&mut self.compact_multimesh_dst_batches_scratch);
        dst_batches.clear();
        dst_batches.reserve(src_batches.len());

        let mut batch_index = 0usize;
        while batch_index < src_batches.len() {
            let mut merged_batch = src_batches[batch_index].clone();
            let batch_group_start = &src_batches[batch_index];
            let dst_instance_start = dst_instances.len() as u32;
            let mut merged_instance_count = 0u32;
            let mut scan = batch_index;
            while scan < src_batches.len()
                && (scan == batch_index
                    || Self::can_compact_merge_multimesh_batches(
                        batch_group_start,
                        &src_batches[scan],
                    ))
            {
                let batch = &src_batches[scan];
                let src_start = batch.instance_start as usize;
                let src_end = (batch.instance_start + batch.instance_count) as usize;
                if src_start < src_end && src_end <= src_instances.len() {
                    dst_instances.extend_from_slice(&src_instances[src_start..src_end]);
                    merged_instance_count =
                        merged_instance_count.saturating_add((src_end - src_start) as u32);
                }
                scan += 1;
            }

            if merged_instance_count > 0 {
                merged_batch.instance_start = dst_instance_start;
                merged_batch.instance_count = merged_instance_count;
                dst_batches.push(merged_batch);
            }
            batch_index = scan;
        }

        self.compact_multimesh_dst_instances_scratch =
            std::mem::replace(&mut self.staged_multimesh_instances, dst_instances);
        self.compact_multimesh_dst_batches_scratch =
            std::mem::replace(&mut self.multimesh_batches, dst_batches);
    }

    #[inline]
    pub(super) fn can_compact_merge_multimesh_batches(
        base: &MultiMeshBatch,
        next: &MultiMeshBatch,
    ) -> bool {
        base.mesh.index_start == next.mesh.index_start
            && base.mesh.index_count == next.mesh.index_count
            && base.mesh.base_vertex == next.mesh.base_vertex
            && base.draw_param_index == next.draw_param_index
            && base.double_sided == next.double_sided
            && base.mesh_blend == next.mesh_blend
            && base.material_kind == next.material_kind
    }

    #[inline]
    pub(super) fn can_compact_merge_batches(base: &DrawBatch, next: &DrawBatch) -> bool {
        base.state_key == next.state_key
            && base.mesh.index_start == next.mesh.index_start
            && base.mesh.index_count == next.mesh.index_count
            && base.mesh.base_vertex == next.mesh.base_vertex
            && base.path == next.path
            && base.packed_lod == next.packed_lod
            && base.double_sided == next.double_sided
            && base.material_kind == next.material_kind
            && base.alpha_mode == next.alpha_mode
            && base.draw_on_top == next.draw_on_top
            && base.base_color_texture_slot == next.base_color_texture_slot
            && base.occlusion_query.is_none()
            && next.occlusion_query.is_none()
            && base.casts_shadows == next.casts_shadows
            && base.receives_shadows == next.receives_shadows
            && base.mesh_blend == next.mesh_blend
            && base.mesh_blend_depth == next.mesh_blend_depth
            && base.blend_layers == next.blend_layers
            && base.blend_mask == next.blend_mask
    }

    #[inline]
    pub(super) fn push_compacted_draw_span(
        spans_per_draw: &mut [Vec<Range<u32>>],
        draw_index: usize,
        span: Range<u32>,
    ) {
        if span.start >= span.end || draw_index >= spans_per_draw.len() {
            return;
        }
        let spans = &mut spans_per_draw[draw_index];
        if let Some(last) = spans.last_mut()
            && span.start <= last.end
        {
            last.end = last.end.max(span.end);
        } else {
            spans.push(span);
        }
    }
}

// Build the multimesh cull staging vectors from sorted/compacted batches.
// Each batch owns region [instance_start, instance_start+instance_count).
// Pure (no GPU) so it can be unit-tested.
fn build_multimesh_cull_staging(
    batches: &[MultiMeshBatch],
    instance_count: usize,
    cull_batches: &mut Vec<MultiMeshCullBatchGpu>,
    indirect: &mut Vec<DrawIndexedIndirectGpu>,
    instance_batch: &mut Vec<u32>,
    visible_identity: &mut Vec<u32>,
) {
    cull_batches.clear();
    cull_batches.reserve(batches.len());
    indirect.clear();
    indirect.reserve(batches.len());
    instance_batch.clear();
    instance_batch.resize(instance_count, 0u32);
    visible_identity.clear();
    visible_identity.reserve(instance_count);
    for i in 0..instance_count as u32 {
        visible_identity.push(i);
    }
    for (batch_index, batch) in batches.iter().enumerate() {
        cull_batches.push(MultiMeshCullBatchGpu {
            instance_start: batch.instance_start,
            instance_cap: batch.instance_count,
            indirect_index: batch_index as u32,
            mesh_radius_bits: batch.mesh_local_radius.max(0.0).to_bits(),
        });
        indirect.push(DrawIndexedIndirectGpu {
            index_count: batch.mesh.index_count,
            instance_count: batch.instance_count,
            first_index: batch.mesh.index_start,
            base_vertex: batch.mesh.base_vertex,
            first_instance: batch.instance_start,
        });
        let start = batch.instance_start as usize;
        let end = (start + batch.instance_count as usize).min(instance_count);
        for owner in &mut instance_batch[start..end] {
            *owner = batch_index as u32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch(instance_start: u32, instance_count: u32, radius: f32) -> MultiMeshBatch {
        MultiMeshBatch {
            mesh: MeshRange {
                index_start: instance_start * 3,
                index_count: 6,
                base_vertex: 2,
            },
            instance_start,
            instance_count,
            draw_param_index: 0,
            mesh_local_radius: radius,
            double_sided: false,
            mesh_blend: false,
            material_kind: MaterialPipelineKind::Standard,
        }
    }

    #[test]
    fn multimesh_cull_staging_maps_instances_and_regions() {
        let batches = [batch(0, 3, 1.5), batch(3, 2, 2.0)];
        let mut cull_batches = Vec::new();
        let mut indirect = Vec::new();
        let mut instance_batch = Vec::new();
        let mut identity = Vec::new();
        build_multimesh_cull_staging(
            &batches,
            5,
            &mut cull_batches,
            &mut indirect,
            &mut instance_batch,
            &mut identity,
        );

        // Per-instance batch owner matches the batch region.
        assert_eq!(instance_batch, vec![0, 0, 0, 1, 1]);
        // Identity fallback covers every instance.
        assert_eq!(identity, vec![0, 1, 2, 3, 4]);
        // Cull records preserve region + firstInstance + radius.
        assert_eq!(cull_batches[0].instance_start, 0);
        assert_eq!(cull_batches[0].instance_cap, 3);
        assert_eq!(cull_batches[0].indirect_index, 0);
        assert_eq!(cull_batches[0].mesh_radius_bits, 1.5f32.to_bits());
        assert_eq!(cull_batches[1].instance_start, 3);
        assert_eq!(cull_batches[1].instance_cap, 2);
        // Indirect records seed instance_count = source count, firstInstance =
        // region base so the compacted region maps back correctly.
        assert_eq!(indirect[0].instance_count, 3);
        assert_eq!(indirect[0].first_instance, 0);
        assert_eq!(indirect[0].first_index, 0);
        assert_eq!(indirect[1].instance_count, 2);
        assert_eq!(indirect[1].first_instance, 3);
        assert_eq!(indirect[1].base_vertex, 2);
    }
}
