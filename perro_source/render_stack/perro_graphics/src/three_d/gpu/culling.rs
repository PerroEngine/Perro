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
            if batch.instance_count > 1 {
                let (static_row, dynamic_row) =
                    multi_instance_cull_rows(batch, &self.staged_instance_transforms);
                self.frustum_cull_static_staging.push(static_row);
                self.frustum_cull_dynamic_staging.push(dynamic_row);
                continue;
            }
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
        let mut src_region_dedup = std::mem::take(&mut self.compact_src_region_dedup_scratch);
        src_region_dedup.clear();

        compact_draw_batches_core(CompactDrawBatchesInputs {
            src_transforms: &src_transforms,
            src_rigid_meta: &src_rigid_meta,
            src_skinned_meta: &src_skinned_meta,
            src_batches: &src_batches,
            instance_owner: &instance_owner,
            dst_transforms: &mut dst_transforms,
            dst_rigid_meta: &mut dst_rigid_meta,
            dst_skinned_meta: &mut dst_skinned_meta,
            dst_batches: &mut dst_batches,
            spans_per_draw: &mut spans_per_draw,
            src_region_dedup: &mut src_region_dedup,
        });

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
        src_region_dedup.clear();
        self.compact_src_region_dedup_scratch = src_region_dedup;
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
            && base.mesh_blend_screen == next.mesh_blend_screen
            && base.mesh_blend_params == next.mesh_blend_params
            && base.mesh_blend_depth == next.mesh_blend_depth
            && base.blend_layers == next.blend_layers
            && base.blend_mask == next.blend_mask
            && base.casts_shadows == next.casts_shadows
            && base.material_kind == next.material_kind
            && base.material_texture_key == next.material_texture_key
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
            && base.material_texture_key == next.material_texture_key
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

pub(super) struct CompactDrawBatchesInputs<'a> {
    pub(super) src_transforms: &'a [TransformInstanceGpu],
    pub(super) src_rigid_meta: &'a [RigidInstanceMetaGpu],
    pub(super) src_skinned_meta: &'a [SkinnedInstanceMetaGpu],
    pub(super) src_batches: &'a [DrawBatch],
    pub(super) instance_owner: &'a [u32],
    pub(super) dst_transforms: &'a mut Vec<TransformInstanceGpu>,
    pub(super) dst_rigid_meta: &'a mut Vec<RigidInstanceMetaGpu>,
    pub(super) dst_skinned_meta: &'a mut Vec<SkinnedInstanceMetaGpu>,
    pub(super) dst_batches: &'a mut Vec<DrawBatch>,
    pub(super) spans_per_draw: &'a mut [Vec<Range<u32>>],
    pub(super) src_region_dedup: &'a mut AHashMap<(u32, u32), u32>,
}

// Compact sorted draw batches: copy each merge group's instances into dst and
// repoint. Meshlet batches of one draw share one instance span; that span's src
// region is copied once and later batches point at the existing dst region
// (src_region_dedup) so the shared span is not re-duplicated per meshlet. Pure
// (no GPU/self) so it can be unit-tested.
pub(super) fn compact_draw_batches_core(inputs: CompactDrawBatchesInputs) {
    let CompactDrawBatchesInputs {
        src_transforms,
        src_rigid_meta,
        src_skinned_meta,
        src_batches,
        instance_owner,
        dst_transforms,
        dst_rigid_meta,
        dst_skinned_meta,
        dst_batches,
        spans_per_draw,
        src_region_dedup,
    } = inputs;

    let mut batch_index = 0usize;
    while batch_index < src_batches.len() {
        let mut merged_batch = src_batches[batch_index].clone();
        let batch_group_start = &src_batches[batch_index];
        let dst_instance_start = dst_transforms.len() as u32;
        let mut merged_instance_count = 0u32;
        let mut merged_disable_hiz = false;
        let mut merged_local = (
            batch_group_start.local_center,
            batch_group_start.local_radius,
        );
        // Track the dst region the first batch of this merge group mapped to,
        // so a shared-span repoint places the merged batch at that region
        // instead of the (skipped) append point.
        let mut group_dst_start: Option<u32> = None;
        let mut scan = batch_index;
        while scan < src_batches.len()
            && (scan == batch_index
                || Gpu3D::can_compact_merge_batches(batch_group_start, &src_batches[scan]))
        {
            let batch = &src_batches[scan];
            // Merge predicate guarantees one mesh per group, so local bounds
            // share one space; keep the tight enclosing sphere.
            merged_local =
                enclose_local_spheres(merged_local, (batch.local_center, batch.local_radius));
            let src_start = batch.instance_start as usize;
            let src_end = (batch.instance_start + batch.instance_count) as usize;
            if src_start < src_end
                && src_end <= src_transforms.len()
                && src_end <= src_rigid_meta.len()
                && src_end <= src_skinned_meta.len()
            {
                // Shared spans (meshlet batches of one draw) resolve to the same
                // (src_start, src_end). Copy such a region once; later batches
                // point at the existing dst region and skip both the re-copy and
                // the span re-emission (the span was already emitted on first copy).
                if let Some(&existing_dst) =
                    src_region_dedup.get(&(src_start as u32, src_end as u32))
                {
                    merged_instance_count =
                        merged_instance_count.saturating_add((src_end - src_start) as u32);
                    if group_dst_start.is_none() {
                        group_dst_start = Some(existing_dst);
                    }
                    merged_disable_hiz |= batch.disable_hiz_occlusion;
                    scan += 1;
                    continue;
                }
                let dst_copy_start = dst_transforms.len() as u32;
                if group_dst_start.is_none() {
                    group_dst_start = Some(dst_copy_start);
                }
                src_region_dedup.insert((src_start as u32, src_end as u32), dst_copy_start);
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
                            let run_start = dst_copy_start + (run_src_start - batch.instance_start);
                            let run_end = dst_copy_start + (src_instance - batch.instance_start);
                            Gpu3D::push_compacted_draw_span(
                                spans_per_draw,
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
                    Gpu3D::push_compacted_draw_span(
                        spans_per_draw,
                        run_owner as usize,
                        run_start..run_end,
                    );
                }
            }
            merged_disable_hiz |= batch.disable_hiz_occlusion;
            scan += 1;
        }

        if merged_instance_count > 0 {
            // group_dst_start is the region the group actually landed in; for a
            // shared-span repoint it is the earlier (deduped) dst region, not the
            // append point at dst_instance_start.
            merged_batch.instance_start = group_dst_start.unwrap_or(dst_instance_start);
            merged_batch.instance_count = merged_instance_count;
            merged_batch.disable_hiz_occlusion = merged_disable_hiz;
            // Multi-instance batches keep the tight local bound; the cull upload
            // expands it into a merged per-instance world sphere.
            merged_batch.local_center = merged_local.0;
            merged_batch.local_radius = merged_local.1;
            dst_batches.push(merged_batch);
        }
        batch_index = scan;
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
            mesh_blend_screen: false,
            mesh_blend_params: 0,
            mesh_blend_depth: false,
            blend_layers: BitMask::ALL.bits(),
            blend_mask: BitMask::NONE.bits(),
            casts_shadows: true,
            material_kind: MaterialPipelineKind::Standard,
            material_texture_key: MaterialTextureKey::empty(),
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

    #[test]
    fn multimesh_compaction_keeps_screen_blend_participants_separate() {
        let base = batch(0, 2, 1.0);
        let mut screen = batch(2, 2, 1.0);
        screen.mesh.index_start = base.mesh.index_start;
        screen.draw_param_index = base.draw_param_index;
        screen.mesh_blend_screen = true;
        screen.mesh_blend_params = 0x0102_0304;
        screen.mesh_blend_depth = true;

        assert!(!Gpu3D::can_compact_merge_multimesh_batches(&base, &screen));
    }

    fn transform_marked(marker: f32) -> TransformInstanceGpu {
        let mut t = TransformInstanceGpu::zeroed();
        t.model_row_0[0] = marker;
        t
    }

    // A meshlet-style draw batch: shares an instance span, differs only by mesh
    // index range. index_start feeds render_state/state so batches don't merge.
    fn meshlet_batch(index_start: u32, instance_start: u32, instance_count: u32) -> DrawBatch {
        let state_key = draw_batch_state_key(
            RenderPath3D::Rigid,
            false,
            false,
            0,
            false,
            &MaterialPipelineKind::Standard,
        );
        let material_texture_key = MaterialTextureKey::from_base(0);
        DrawBatch {
            state_key,
            render_state: render_state_key(
                state_key,
                material_texture_key.state_hash(),
                index_start,
                0,
                false,
                0,
                false,
            ),
            mesh: MeshRange {
                index_start,
                index_count: 12,
                base_vertex: 0,
            },
            instance_start,
            instance_count,
            path: RenderPath3D::Rigid,
            packed_lod: false,
            double_sided: false,
            material_kind: MaterialPipelineKind::Standard,
            alpha_mode: 0,
            draw_on_top: false,
            base_color_texture_slot: 0,
            material_texture_key,
            local_center: [index_start as f32, 0.0, 0.0],
            local_radius: 1.0,
            occlusion_query: None,
            disable_hiz_occlusion: false,
            casts_shadows: true,
            receives_shadows: true,
            mesh_blend: false,
            mesh_blend_screen: false,
            mesh_blend_params: 0,
            mesh_blend_depth: false,
            blend_layers: 0,
            blend_mask: 0,
            order_index: index_start,
        }
    }

    fn run_compact(
        src_transforms: &[TransformInstanceGpu],
        src_batches: &[DrawBatch],
        instance_owner: &[u32],
        draw_count: usize,
    ) -> (
        Vec<TransformInstanceGpu>,
        Vec<DrawBatch>,
        Vec<Vec<Range<u32>>>,
    ) {
        let n = src_transforms.len();
        let src_rigid: Vec<RigidInstanceMetaGpu> = vec![RigidInstanceMetaGpu::zeroed(); n];
        let src_skinned: Vec<SkinnedInstanceMetaGpu> = vec![SkinnedInstanceMetaGpu::zeroed(); n];
        let mut dst_transforms = Vec::new();
        let mut dst_rigid = Vec::new();
        let mut dst_skinned = Vec::new();
        let mut dst_batches = Vec::new();
        let mut spans_per_draw: Vec<Vec<Range<u32>>> = vec![Vec::new(); draw_count];
        let mut dedup = AHashMap::default();
        compact_draw_batches_core(CompactDrawBatchesInputs {
            src_transforms,
            src_rigid_meta: &src_rigid,
            src_skinned_meta: &src_skinned,
            src_batches,
            instance_owner,
            dst_transforms: &mut dst_transforms,
            dst_rigid_meta: &mut dst_rigid,
            dst_skinned_meta: &mut dst_skinned,
            dst_batches: &mut dst_batches,
            spans_per_draw: &mut spans_per_draw,
            src_region_dedup: &mut dedup,
        });
        (dst_transforms, dst_batches, spans_per_draw)
    }

    #[test]
    fn compact_shares_one_span_across_meshlet_batches() {
        // One draw, one instance, 3 meshlet batches all pointing at the same
        // single-instance span [0,1). Expect dst to hold ONE copy, all 3 batches
        // repointed to it, and the draw's span list to have ONE range.
        let src_transforms = vec![transform_marked(7.0)];
        let src_batches = [
            meshlet_batch(0, 0, 1),
            meshlet_batch(30, 0, 1),
            meshlet_batch(60, 0, 1),
        ];
        let instance_owner = [0u32]; // instance owned by draw 0
        let (dst_transforms, dst_batches, spans_per_draw) =
            run_compact(&src_transforms, &src_batches, &instance_owner, 1);

        // Exactly one instance copied, not three.
        assert_eq!(dst_transforms.len(), 1);
        assert_eq!(dst_transforms[0].model_row_0[0], 7.0);
        // All three meshlet batches survive and point at the shared dst region.
        assert_eq!(dst_batches.len(), 3);
        for batch in &dst_batches {
            assert_eq!(batch.instance_start, 0);
            assert_eq!(batch.instance_count, 1);
        }
        // Per-meshlet mesh index ranges are preserved (distinct batches).
        assert_eq!(dst_batches[0].mesh.index_start, 0);
        assert_eq!(dst_batches[1].mesh.index_start, 30);
        assert_eq!(dst_batches[2].mesh.index_start, 60);
        // The draw's span list has ONE range covering the single instance, so the
        // transform-only patch path uploads the shared region once.
        assert_eq!(spans_per_draw.len(), 1);
        assert_eq!(spans_per_draw[0], vec![0..1]);
    }

    #[test]
    fn compact_dedups_shared_span_even_when_interleaved() {
        // Two draws each with 2 meshlet batches, interleaved after sort:
        //   draw0[a], draw1[a], draw0[b], draw1[b]
        // draw0 shares span [0,1), draw1 shares span [1,2). Dedup is keyed by src
        // region (not position) so the later batches repoint regardless of order.
        let src_transforms = vec![transform_marked(1.0), transform_marked(2.0)];
        let src_batches = [
            meshlet_batch(0, 0, 1),  // draw0 meshlet a
            meshlet_batch(10, 1, 1), // draw1 meshlet a
            meshlet_batch(20, 0, 1), // draw0 meshlet b (shares [0,1))
            meshlet_batch(30, 1, 1), // draw1 meshlet b (shares [1,2))
        ];
        let instance_owner = [0u32, 1u32];
        let (dst_transforms, dst_batches, spans_per_draw) =
            run_compact(&src_transforms, &src_batches, &instance_owner, 2);

        // Only two instances copied total (one per draw), not four.
        assert_eq!(dst_transforms.len(), 2);
        assert_eq!(dst_batches.len(), 4);
        // draw0's two batches share one dst region; draw1's two share another.
        assert_eq!(dst_batches[0].instance_start, dst_batches[2].instance_start);
        assert_eq!(dst_batches[1].instance_start, dst_batches[3].instance_start);
        assert_ne!(dst_batches[0].instance_start, dst_batches[1].instance_start);
        // Each draw contributes exactly one span (all its meshlet batches share it).
        assert_eq!(spans_per_draw[0].len(), 1);
        assert_eq!(spans_per_draw[1].len(), 1);
    }

    #[test]
    fn compact_multi_instance_shared_span_keeps_tight_bounds() {
        // 3-instance draw, 2 meshlet batches sharing span [0,3). Multi-instance
        // batches keep their tight local bound + hi-z; the cull upload expands
        // the bound into a merged per-instance world sphere.
        let src_transforms = vec![
            transform_marked(1.0),
            transform_marked(2.0),
            transform_marked(3.0),
        ];
        let src_batches = [meshlet_batch(0, 0, 3), meshlet_batch(40, 0, 3)];
        let instance_owner = [0u32, 0u32, 0u32];
        let (dst_transforms, dst_batches, spans_per_draw) =
            run_compact(&src_transforms, &src_batches, &instance_owner, 1);

        assert_eq!(dst_transforms.len(), 3);
        assert_eq!(dst_batches.len(), 2);
        for batch in &dst_batches {
            assert_eq!(batch.instance_start, 0);
            assert_eq!(batch.instance_count, 3);
            assert!(!batch.disable_hiz_occlusion);
            assert!(batch.local_radius < 1.0e8);
        }
        // meshlet_batch centers are [index_start, 0, 0] with radius 1; batches
        // here don't merge (different mesh ranges) so each keeps its own bound.
        assert_eq!(dst_batches[0].local_center, [0.0, 0.0, 0.0]);
        assert_eq!(dst_batches[0].local_radius, 1.0);
        assert_eq!(dst_batches[1].local_center, [40.0, 0.0, 0.0]);
        assert_eq!(dst_batches[1].local_radius, 1.0);
        assert_eq!(spans_per_draw[0], vec![0..3]);
    }

    #[test]
    fn compact_regular_adjacent_batches_still_copy_each_region() {
        // Non-shared (regular) batches with adjacent exclusive regions must each
        // copy their own instances; dedup only fires on identical src regions.
        let src_transforms = vec![transform_marked(1.0), transform_marked(2.0)];
        let src_batches = [meshlet_batch(0, 0, 1), meshlet_batch(30, 1, 1)];
        let instance_owner = [0u32, 1u32];
        let (dst_transforms, dst_batches, _spans) =
            run_compact(&src_transforms, &src_batches, &instance_owner, 2);
        assert_eq!(dst_transforms.len(), 2);
        assert_eq!(dst_batches[0].instance_start, 0);
        assert_eq!(dst_batches[1].instance_start, 1);
    }
}
