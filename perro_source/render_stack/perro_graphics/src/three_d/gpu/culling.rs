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
        let mut instance_owner = vec![u32::MAX; src_instance_count];
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
        let src_materials = std::mem::take(&mut self.staged_instance_materials);
        let src_rigid_meta = std::mem::take(&mut self.staged_rigid_instance_meta);
        let src_skinned_meta = std::mem::take(&mut self.staged_skinned_instance_meta);
        let src_batches = std::mem::take(&mut self.draw_batches);

        let mut dst_transforms = Vec::with_capacity(src_transforms.len());
        let mut dst_materials = Vec::with_capacity(src_materials.len());
        let mut dst_rigid_meta = Vec::with_capacity(src_rigid_meta.len());
        let mut dst_skinned_meta = Vec::with_capacity(src_skinned_meta.len());
        let mut dst_batches = Vec::with_capacity(src_batches.len());
        let mut spans_per_draw: Vec<Vec<Range<u32>>> = vec![Vec::new(); draw_count];

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
                    && src_end <= src_materials.len()
                    && src_end <= src_rigid_meta.len()
                    && src_end <= src_skinned_meta.len()
                {
                    let dst_copy_start = dst_transforms.len() as u32;
                    dst_transforms.extend_from_slice(&src_transforms[src_start..src_end]);
                    dst_materials.extend_from_slice(&src_materials[src_start..src_end]);
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

        self.staged_instance_transforms = dst_transforms;
        self.staged_instance_materials = dst_materials;
        self.staged_rigid_instance_meta = dst_rigid_meta;
        self.staged_skinned_instance_meta = dst_skinned_meta;
        self.draw_batches = dst_batches;

        self.last_draw_instance_spans.clear();
        self.last_draw_instance_span_ranges.clear();
        self.last_draw_instance_span_ranges.reserve(draw_count);
        for spans in spans_per_draw.iter_mut() {
            let start = self.last_draw_instance_spans.len();
            self.last_draw_instance_spans.append(spans);
            let end = self.last_draw_instance_spans.len();
            self.last_draw_instance_span_ranges.push(start..end);
        }
    }

    #[inline]
    pub(super) fn can_compact_merge_batches(base: &DrawBatch, next: &DrawBatch) -> bool {
        base.state_key == next.state_key
            && base.mesh.index_start == next.mesh.index_start
            && base.mesh.index_count == next.mesh.index_count
            && base.mesh.base_vertex == next.mesh.base_vertex
            && base.path == next.path
            && base.double_sided == next.double_sided
            && base.material_kind == next.material_kind
            && base.alpha_mode == next.alpha_mode
            && base.draw_on_top == next.draw_on_top
            && base.base_color_texture_slot == next.base_color_texture_slot
            && base.occlusion_query.is_none()
            && next.occlusion_query.is_none()
            && base.casts_shadows == next.casts_shadows
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
