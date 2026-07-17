use super::*;

pub(super) fn shadow_layer_cull(
    shadow_batch_indices: &[usize],
    draw_batches: &[DrawBatch],
    transforms: &[TransformInstanceGpu],
    frustum: &[Vec4; 6],
    out: &mut Vec<usize>,
) {
    out.clear();
    for &batch_index in shadow_batch_indices {
        let Some(batch) = draw_batches.get(batch_index) else {
            continue;
        };
        match batch_world_sphere(batch, transforms) {
            Some((center, radius)) => {
                if sphere_in_frustum(center, radius, frustum) {
                    out.push(batch_index);
                }
            }
            // Conservative: no tight sphere (multi-instance / non-finite) => keep.
            None => out.push(batch_index),
        }
    }
}

#[inline]
pub(super) fn sphere_in_frustum(center: Vec3, radius: f32, planes: &[Vec4; 6]) -> bool {
    for plane in planes {
        if plane.truncate().dot(center) + plane.w < -radius {
            return false;
        }
    }
    true
}

impl Gpu3D {
    // Populate shadow_cull_scratch with the batches to draw for one shadow layer.
    pub(super) fn compute_shadow_cull(&mut self, camera_index: usize) {
        let mut scratch = std::mem::take(&mut self.shadow_cull_scratch);
        match self.shadow_camera_frustums.get(camera_index) {
            Some(frustum) => shadow_layer_cull(
                &self.shadow_batch_indices,
                &self.draw_batches,
                &self.staged_instance_transforms,
                frustum,
                &mut scratch,
            ),
            None => {
                scratch.clear();
                scratch.extend_from_slice(&self.shadow_batch_indices);
            }
        }
        self.shadow_cull_scratch = scratch;
    }
}

pub(super) fn draw_shadow_batches<'a>(
    gpu: &'a Gpu3D,
    shadow_pass: &mut wgpu::RenderPass<'a>,
    camera_index: usize,
) {
    let Some(shadow_camera_bg) = gpu.shadow_camera_bind_groups.get(camera_index) else {
        return;
    };
    let Some(rigid_shadow_camera_bg) = gpu.rigid_shadow_camera_bind_groups.get(camera_index) else {
        return;
    };
    let mut current_state: Option<(RenderPath3D, bool, bool)> = None;
    shadow_pass.set_vertex_buffer(1, gpu.instance_transform_buffer.slice(..));
    // shadow_cull_scratch was filled by compute_shadow_cull for this layer.
    for &batch_index in &gpu.shadow_cull_scratch {
        let batch = &gpu.draw_batches[batch_index];
        let state = (batch.path, batch.double_sided, batch.packed_lod);
        if current_state != Some(state) {
            let (camera_bg, vertex_buf, pipeline) = if batch.path == RenderPath3D::Rigid {
                (
                    rigid_shadow_camera_bg,
                    if batch.packed_lod {
                        &gpu.packed_lod_vertex_buffer
                    } else {
                        &gpu.rigid_vertex_buffer
                    },
                    if batch.double_sided {
                        if batch.packed_lod {
                            &gpu.pipeline_shadow_depth_rigid_packed_lod_double_sided
                        } else {
                            &gpu.pipeline_shadow_depth_rigid_double_sided
                        }
                    } else {
                        if batch.packed_lod {
                            &gpu.pipeline_shadow_depth_rigid_packed_lod_culled
                        } else {
                            &gpu.pipeline_shadow_depth_rigid_culled
                        }
                    },
                )
            } else {
                (
                    shadow_camera_bg,
                    &gpu.vertex_buffer,
                    if batch.double_sided {
                        &gpu.pipeline_shadow_depth_double_sided
                    } else {
                        &gpu.pipeline_shadow_depth_culled
                    },
                )
            };
            shadow_pass.set_bind_group(0, camera_bg, &[]);
            if batch.packed_lod {
                shadow_pass.set_index_buffer(
                    gpu.packed_lod_index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
            } else {
                shadow_pass.set_index_buffer(gpu.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            }
            shadow_pass.set_vertex_buffer(0, vertex_buf.slice(..));
            if batch.path == RenderPath3D::Skinned {
                shadow_pass.set_vertex_buffer(2, gpu.skinned_instance_meta_buffer.slice(..));
            } else {
                shadow_pass.set_vertex_buffer(2, gpu.rigid_instance_meta_buffer.slice(..));
            }
            shadow_pass.set_pipeline(pipeline);
            current_state = Some(state);
        }
        let start = batch.mesh.index_start;
        let end = start + batch.mesh.index_count;
        let instances = batch.instance_start..batch.instance_start + batch.instance_count;
        shadow_pass.draw_indexed(start..end, batch.mesh.base_vertex, instances);
    }
    draw_multimesh_shadow_casters(gpu, shadow_pass, camera_index);
}

// True when a multimesh batch uses a custom material whose shader defines a
// shade_vertex hook (or whose hook flag is unknown — conservative). The shared
// depth-only pipelines can't run the hook, so such batches must not feed the
// shadow map or the depth prepass.
pub(super) fn multimesh_batch_vertex_hooked(gpu: &Gpu3D, batch: &MultiMeshBatch) -> bool {
    match &batch.material_kind {
        MaterialPipelineKind::Custom(token) => {
            gpu.custom_pipeline_vertex_hooks.get(token).copied() != Some(false)
        }
        _ => false,
    }
}

// Draw shadow-casting multimesh batches into the current shadow layer. Uses the
// per-layer shadow bind group (light scene uniform + identity index buffer), so
// direct draws over the full instance set — the camera cull output is invalid
// for a light's view. Mesh-blend batches are excluded (alpha, like rigid mode 2).
pub(super) fn draw_multimesh_shadow_casters<'a>(
    gpu: &'a Gpu3D,
    pass: &mut wgpu::RenderPass<'a>,
    camera_index: usize,
) {
    if gpu.multimesh_batches.is_empty() {
        return;
    }
    let Some(shadow_bg) = gpu.shadow_multimesh_bind_groups.get(camera_index) else {
        return;
    };
    let frustum = gpu.shadow_camera_frustums.get(camera_index);
    let mut bound = false;
    let mut current_double_sided: Option<bool> = None;
    for batch in gpu.multimesh_batches.iter() {
        if !batch.casts_shadows || batch.mesh_blend {
            continue;
        }
        // Same rule as rebuild_batch_views: a shade_vertex custom would cast
        // an undisplaced (wrong) shadow through the shared depth-only
        // pipeline, so it stays out; hook-free custom casts like standard.
        if multimesh_batch_vertex_hooked(gpu, batch) {
            continue;
        }
        // Cull whole grass/prop fields outside the light view when bounds exist.
        if let Some(frustum) = frustum
            && let Some((center, radius)) = super::prepare::multimesh_world_bounds(
                batch,
                &gpu.staged_multimesh_draw_params,
                &gpu.staged_multimesh_instances,
            )
            && !sphere_in_frustum(center, radius, frustum)
        {
            continue;
        }
        if !bound {
            pass.set_bind_group(0, shadow_bg, &[]);
            pass.set_vertex_buffer(0, gpu.rigid_vertex_buffer.slice(..));
            pass.set_index_buffer(gpu.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            bound = true;
        }
        if current_double_sided != Some(batch.double_sided) {
            let pipeline = if batch.double_sided {
                &gpu.pipeline_multimesh_shadow_depth_double_sided
            } else {
                &gpu.pipeline_multimesh_shadow_depth_culled
            };
            pass.set_pipeline(pipeline);
            current_double_sided = Some(batch.double_sided);
        }
        let start = batch.mesh.index_start;
        let end = start + batch.mesh.index_count;
        let instances = batch.instance_start..batch.instance_start + batch.instance_count;
        pass.draw_indexed(start..end, batch.mesh.base_vertex, instances);
    }
}

pub(super) fn draw_multimesh_batches<'a>(gpu: &'a Gpu3D, pass: &mut wgpu::RenderPass<'a>) {
    if gpu.multimesh_batches.is_empty() {
        return;
    }
    // Prepass-covered variants apply only to non-blend batches when unified
    // depth is active (the prepass primed their depth). Blend batches keep
    // depth-write-off blend pipelines regardless.
    let covered = gpu.unified_depth_active;
    let cull = gpu.multimesh_cull_active;
    pass.set_bind_group(0, &gpu.multimesh_bind_group, &[]);
    let Some(fallback_material) = gpu.fallback_material_texture_bind_group() else {
        return;
    };
    pass.set_bind_group(1, fallback_material, &[]);
    pass.set_bind_group(3, &gpu.ibl_bind_group, &[]);
    pass.set_vertex_buffer(0, gpu.rigid_vertex_buffer.slice(..));
    pass.set_index_buffer(gpu.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
    let mut current_state: Option<(bool, bool, &MaterialPipelineKind)> = None;
    let mut current_texture_key: Option<MaterialTextureKey> = None;
    // Multimesh indirect records are laid out contiguously in batch order
    // (rebuild_multimesh_cull_inputs / compact_sorted_multimesh_batches), so
    // consecutive same-pipeline batches coalesce into one multi-draw call.
    let mut run = IndirectRunBuilder::new(cull && gpu.multi_draw_indirect_enabled);
    for (batch_index, batch) in gpu.multimesh_batches.iter().enumerate() {
        let state = (batch.double_sided, batch.mesh_blend, &batch.material_kind);
        let state_change = current_state != Some(state);
        let texture_change = current_texture_key != Some(batch.material_texture_key);
        if state_change || texture_change {
            run.flush(&gpu.multimesh_indirect_buffer, pass);
        }
        if state_change {
            let pipeline = match &batch.material_kind {
                MaterialPipelineKind::Custom(token) => {
                    gpu.custom_pipelines_multimesh.get(token).map(|pipeline| {
                        if batch.mesh_blend && batch.double_sided {
                            &pipeline.pipeline_blend_double_sided
                        } else if batch.mesh_blend {
                            &pipeline.pipeline_blend_culled
                        } else if batch.double_sided {
                            &pipeline.pipeline_double_sided
                        } else {
                            &pipeline.pipeline_culled
                        }
                    })
                }
                _ => None,
            }
            .unwrap_or({
                if batch.mesh_blend && batch.double_sided {
                    &gpu.pipeline_multimesh_blend_double_sided
                } else if batch.mesh_blend {
                    &gpu.pipeline_multimesh_blend_culled
                } else if covered && batch.double_sided {
                    &gpu.pipeline_multimesh_covered_double_sided
                } else if covered {
                    &gpu.pipeline_multimesh_covered
                } else if batch.double_sided {
                    &gpu.pipeline_multimesh_double_sided
                } else {
                    &gpu.pipeline_multimesh_culled
                }
            });
            pass.set_pipeline(pipeline);
            current_state = Some(state);
        }
        if texture_change {
            let Some(material_bind_group) =
                gpu.material_texture_set_bind_group(batch.material_texture_key)
            else {
                continue;
            };
            pass.set_bind_group(1, material_bind_group, &[]);
            current_texture_key = Some(batch.material_texture_key);
        }
        if run.push(&gpu.multimesh_indirect_buffer, pass, batch_index) {
            // absorbed into (or started) a run
        } else if cull {
            let offset = (batch_index * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64;
            pass.draw_indexed_indirect(&gpu.multimesh_indirect_buffer, offset);
        } else {
            let start = batch.mesh.index_start;
            let end = start + batch.mesh.index_count;
            let instances = batch.instance_start..batch.instance_start + batch.instance_count;
            pass.draw_indexed(start..end, batch.mesh.base_vertex, instances);
        }
    }
    run.flush(&gpu.multimesh_indirect_buffer, pass);
}

// Draw non-blend multimesh batches into the depth prepass (post-cull, same
// indirect args). Mesh-blend batches are excluded, mirroring how mesh_blend
// rigid batches are excluded from the prepass.
pub(super) fn draw_multimesh_depth_prepass<'a>(
    gpu: &'a Gpu3D,
    pass: &mut wgpu::RenderPass<'a>,
    cull: bool,
) {
    if gpu.multimesh_batches.is_empty() {
        return;
    }
    pass.set_bind_group(0, &gpu.multimesh_bind_group, &[]);
    pass.set_vertex_buffer(0, gpu.rigid_vertex_buffer.slice(..));
    pass.set_index_buffer(gpu.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
    let mut current_double_sided: Option<bool> = None;
    for (batch_index, batch) in gpu.multimesh_batches.iter().enumerate() {
        if batch.mesh_blend {
            continue;
        }
        // A shade_vertex custom would prime the unified depth buffer with
        // undisplaced positions and hole out its own displaced main draw;
        // its main pipeline (LessEqual + depth write) self-primes instead.
        if multimesh_batch_vertex_hooked(gpu, batch) {
            continue;
        }
        if current_double_sided != Some(batch.double_sided) {
            let pipeline = if batch.double_sided {
                &gpu.pipeline_multimesh_depth_prepass_double_sided
            } else {
                &gpu.pipeline_multimesh_depth_prepass_culled
            };
            pass.set_pipeline(pipeline);
            current_double_sided = Some(batch.double_sided);
        }
        if cull {
            let offset = (batch_index * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64;
            pass.draw_indexed_indirect(&gpu.multimesh_indirect_buffer, offset);
        } else {
            let start = batch.mesh.index_start;
            let end = start + batch.mesh.index_count;
            let instances = batch.instance_start..batch.instance_start + batch.instance_count;
            pass.draw_indexed(start..end, batch.mesh.base_vertex, instances);
        }
    }
}

// Spheres are precomputed per batch by the caller; None (non-finite / sentinel
// radius / out-of-range) means no usable bound, so the pair conservatively
// overlaps.
pub(super) fn mesh_blend_batches_overlap(
    source_sphere: Option<(Vec3, f32)>,
    target_sphere: Option<(Vec3, f32)>,
) -> bool {
    let Some((source_center, source_radius)) = source_sphere else {
        return true;
    };
    let Some((target_center, target_radius)) = target_sphere else {
        return true;
    };
    source_center.distance_squared(target_center)
        <= (source_radius + target_radius).max(0.0).powi(2)
}

pub(super) fn batch_world_sphere(
    batch: &DrawBatch,
    transforms: &[TransformInstanceGpu],
) -> Option<(Vec3, f32)> {
    // Multi-instance batches merge every instance's world sphere; batches with
    // no usable bound (non-finite / sentinel radius) return None and survive.
    batch_merged_world_sphere(batch, transforms)
}
