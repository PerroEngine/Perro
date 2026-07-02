use super::*;

fn custom_pipeline_key(shader_path: &str, lighting: CustomMaterialLighting3D) -> String {
    let suffix = match lighting {
        CustomMaterialLighting3D::Standard => "#standard",
        CustomMaterialLighting3D::Raw => "#raw",
    };
    let mut key = String::with_capacity(shader_path.len() + suffix.len());
    key.push_str(shader_path);
    key.push_str(suffix);
    key
}

impl Gpu3D {
    pub(super) fn custom_pipeline_token(
        &mut self,
        shader_path: &str,
        lighting: CustomMaterialLighting3D,
    ) -> u32 {
        let key = custom_pipeline_key(shader_path, lighting);
        if let Some(&token) = self.custom_pipeline_tokens.get(&key) {
            return token;
        }
        let token = self.next_custom_pipeline_token;
        self.next_custom_pipeline_token = self.next_custom_pipeline_token.wrapping_add(1).max(1);
        self.custom_pipeline_tokens.insert(key, token);
        token
    }

    pub(super) fn ensure_custom_pipeline(
        &mut self,
        device: &wgpu::Device,
        path: RenderPath3D,
        shader_path: &str,
        lighting: CustomMaterialLighting3D,
        static_shader_lookup: Option<StaticShaderLookup>,
    ) -> Option<u32> {
        let token = self.custom_pipeline_token(shader_path, lighting);
        if path == RenderPath3D::Rigid && self.custom_pipelines_rigid.contains_key(&token) {
            return Some(token);
        }
        if path == RenderPath3D::Skinned && self.custom_pipelines.contains_key(&token) {
            return Some(token);
        }
        if path == RenderPath3D::MultiMesh && self.custom_pipelines_multimesh.contains_key(&token) {
            return Some(token);
        }
        let src = if let Some(lookup) = static_shader_lookup {
            let shader_hash = perro_ids::parse_hashed_source_uri(shader_path)
                .unwrap_or_else(|| perro_ids::string_to_u64(shader_path));
            let src = lookup(shader_hash);
            (!src.is_empty()).then_some(Cow::Borrowed(src))
        } else {
            None
        }
        .or_else(|| {
            let bytes = load_asset(shader_path).ok()?;
            let src = std::str::from_utf8(&bytes).ok()?;
            Some(Cow::Owned(src.to_string()))
        })?;
        let wgsl = if path == RenderPath3D::MultiMesh {
            build_custom_multimesh_material_shader(src.as_ref(), lighting)
        } else if path == RenderPath3D::Rigid {
            build_custom_material_shader_with_prelude(
                perro_macros::include_str_stripped!("shaders/prelude_rigid_3d.wgsl"),
                src.as_ref(),
                lighting,
            )
        } else {
            build_custom_material_shader_with_prelude(
                perro_macros::include_str_stripped!("shaders/prelude_skinned_3d.wgsl"),
                src.as_ref(),
                lighting,
            )
        };
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_mesh_custom"),
            source: wgpu::ShaderSource::Wgsl(wgsl.into()),
        });
        let pipeline_culled = if path == RenderPath3D::MultiMesh {
            create_multimesh_pipeline(
                device,
                &self.multimesh_pipeline_layout,
                &shader,
                self.color_format,
                self.sample_count,
                Some(wgpu::Face::Back),
            )
        } else if path == RenderPath3D::Rigid {
            create_pipeline_rigid(
                device,
                &self.rigid_material_pipeline_layout,
                &shader,
                self.color_format,
                self.sample_count,
                Some(wgpu::Face::Back),
            )
        } else {
            create_pipeline_skinned(
                device,
                &self.material_pipeline_layout,
                &shader,
                self.color_format,
                self.sample_count,
                Some(wgpu::Face::Back),
            )
        };
        let pipeline_double_sided = if path == RenderPath3D::MultiMesh {
            create_multimesh_pipeline(
                device,
                &self.multimesh_pipeline_layout,
                &shader,
                self.color_format,
                self.sample_count,
                None,
            )
        } else if path == RenderPath3D::Rigid {
            create_pipeline_rigid(
                device,
                &self.rigid_material_pipeline_layout,
                &shader,
                self.color_format,
                self.sample_count,
                None,
            )
        } else {
            create_pipeline_skinned(
                device,
                &self.material_pipeline_layout,
                &shader,
                self.color_format,
                self.sample_count,
                None,
            )
        };
        let pipeline_blend_culled = if path == RenderPath3D::MultiMesh {
            create_multimesh_blend_pipeline(
                device,
                &self.multimesh_pipeline_layout,
                &shader,
                self.color_format,
                self.sample_count,
                Some(wgpu::Face::Back),
            )
        } else if path == RenderPath3D::Rigid {
            create_pipeline_rigid_blend(
                device,
                &self.rigid_material_pipeline_layout,
                &shader,
                self.color_format,
                self.sample_count,
                Some(wgpu::Face::Back),
            )
        } else {
            create_pipeline_skinned_blend(
                device,
                &self.material_pipeline_layout,
                &shader,
                self.color_format,
                self.sample_count,
                Some(wgpu::Face::Back),
            )
        };
        let pipeline_blend_double_sided = if path == RenderPath3D::MultiMesh {
            create_multimesh_blend_pipeline(
                device,
                &self.multimesh_pipeline_layout,
                &shader,
                self.color_format,
                self.sample_count,
                None,
            )
        } else if path == RenderPath3D::Rigid {
            create_pipeline_rigid_blend(
                device,
                &self.rigid_material_pipeline_layout,
                &shader,
                self.color_format,
                self.sample_count,
                None,
            )
        } else {
            create_pipeline_skinned_blend(
                device,
                &self.material_pipeline_layout,
                &shader,
                self.color_format,
                self.sample_count,
                None,
            )
        };
        let map = if path == RenderPath3D::MultiMesh {
            &mut self.custom_pipelines_multimesh
        } else if path == RenderPath3D::Rigid {
            &mut self.custom_pipelines_rigid
        } else {
            &mut self.custom_pipelines
        };
        map.insert(
            token,
            CustomPipeline {
                pipeline_culled,
                pipeline_double_sided,
                pipeline_blend_culled,
                pipeline_blend_double_sided,
            },
        );
        Some(token)
    }

    pub(super) fn material_pipeline_kind(
        &mut self,
        device: &wgpu::Device,
        render_path: RenderPath3D,
        material: &Material3D,
        static_shader_lookup: Option<StaticShaderLookup>,
    ) -> MaterialPipelineKind {
        match material {
            Material3D::Standard(_) => MaterialPipelineKind::Standard,
            Material3D::Unlit(_) => MaterialPipelineKind::Unlit,
            Material3D::Toon(_) => MaterialPipelineKind::Toon,
            Material3D::Custom(custom) => {
                let shader_path = custom.shader_path.as_ref();
                if let Some(token) = self.ensure_custom_pipeline(
                    device,
                    render_path,
                    shader_path,
                    custom.lighting,
                    static_shader_lookup,
                ) {
                    MaterialPipelineKind::Custom(token)
                } else {
                    MaterialPipelineKind::Standard
                }
            }
        }
    }

    pub(super) fn pipeline_for_batch(&self, batch: &DrawBatch) -> &wgpu::RenderPipeline {
        let is_rigid = batch.path == RenderPath3D::Rigid;
        // Alpha-blended batches must not write depth, or transparents drawn
        // first occlude transparents behind them; the *_blend pipelines are
        // the same state with depth write off.
        let soft_depth = batch.mesh_blend || batch.alpha_mode == 2;
        if batch.draw_on_top {
            return if batch.double_sided && is_rigid {
                &self.pipeline_rigid_overlay_double_sided
            } else if is_rigid {
                &self.pipeline_rigid_overlay_culled
            } else if batch.double_sided {
                &self.pipeline_overlay_double_sided
            } else {
                &self.pipeline_overlay_culled
            };
        }
        match &batch.material_kind {
            MaterialPipelineKind::Standard => {
                if batch.packed_lod && soft_depth && batch.double_sided && is_rigid {
                    &self.pipeline_rigid_packed_lod_blend_double_sided
                } else if batch.packed_lod && soft_depth && is_rigid {
                    &self.pipeline_rigid_packed_lod_blend_culled
                } else if batch.packed_lod && batch.double_sided && is_rigid {
                    &self.pipeline_rigid_packed_lod_double_sided
                } else if batch.packed_lod && is_rigid {
                    &self.pipeline_rigid_packed_lod_culled
                } else if soft_depth && batch.double_sided && is_rigid {
                    &self.pipeline_rigid_blend_double_sided
                } else if soft_depth && is_rigid {
                    &self.pipeline_rigid_blend_culled
                } else if soft_depth && batch.double_sided {
                    &self.pipeline_blend_double_sided
                } else if soft_depth {
                    &self.pipeline_blend_culled
                } else if batch.double_sided && is_rigid {
                    &self.pipeline_rigid_double_sided
                } else if is_rigid {
                    &self.pipeline_rigid_culled
                } else if batch.double_sided {
                    &self.pipeline_double_sided
                } else {
                    &self.pipeline_culled
                }
            }
            MaterialPipelineKind::Unlit => {
                if soft_depth && batch.double_sided && is_rigid {
                    &self.pipeline_rigid_unlit_blend_double_sided
                } else if soft_depth && is_rigid {
                    &self.pipeline_rigid_unlit_blend_culled
                } else if soft_depth && batch.double_sided {
                    &self.pipeline_unlit_blend_double_sided
                } else if soft_depth {
                    &self.pipeline_unlit_blend_culled
                } else if batch.double_sided && is_rigid {
                    &self.pipeline_rigid_unlit_double_sided
                } else if is_rigid {
                    &self.pipeline_rigid_unlit_culled
                } else if batch.double_sided {
                    &self.pipeline_unlit_double_sided
                } else {
                    &self.pipeline_unlit_culled
                }
            }
            MaterialPipelineKind::Toon => {
                if soft_depth && batch.double_sided && is_rigid {
                    &self.pipeline_rigid_toon_blend_double_sided
                } else if soft_depth && is_rigid {
                    &self.pipeline_rigid_toon_blend_culled
                } else if soft_depth && batch.double_sided {
                    &self.pipeline_toon_blend_double_sided
                } else if soft_depth {
                    &self.pipeline_toon_blend_culled
                } else if batch.double_sided && is_rigid {
                    &self.pipeline_rigid_toon_double_sided
                } else if is_rigid {
                    &self.pipeline_rigid_toon_culled
                } else if batch.double_sided {
                    &self.pipeline_toon_double_sided
                } else {
                    &self.pipeline_toon_culled
                }
            }
            MaterialPipelineKind::Custom(token) => {
                let map = if is_rigid {
                    &self.custom_pipelines_rigid
                } else {
                    &self.custom_pipelines
                };
                map.get(token)
                    .map(|pipeline| {
                        if soft_depth && batch.double_sided {
                            &pipeline.pipeline_blend_double_sided
                        } else if soft_depth {
                            &pipeline.pipeline_blend_culled
                        } else if batch.double_sided {
                            &pipeline.pipeline_double_sided
                        } else {
                            &pipeline.pipeline_culled
                        }
                    })
                    .unwrap_or_else(|| {
                        if soft_depth && batch.double_sided && is_rigid {
                            &self.pipeline_rigid_blend_double_sided
                        } else if soft_depth && is_rigid {
                            &self.pipeline_rigid_blend_culled
                        } else if soft_depth && batch.double_sided {
                            &self.pipeline_blend_double_sided
                        } else if soft_depth {
                            &self.pipeline_blend_culled
                        } else if batch.double_sided && is_rigid {
                            &self.pipeline_rigid_double_sided
                        } else if is_rigid {
                            &self.pipeline_rigid_culled
                        } else if batch.double_sided {
                            &self.pipeline_double_sided
                        } else {
                            &self.pipeline_culled
                        }
                    })
            }
        }
    }

    pub(super) fn stage_custom_params(&mut self, material: &Material3D) -> (u32, u32) {
        match material {
            Material3D::Custom(custom) => {
                if custom.params.is_empty() {
                    return (0, 0);
                }
                self.staged_custom_params_key_scratch.clear();
                self.staged_custom_params_meta_scratch.clear();
                self.staged_custom_params_values_scratch.clear();
                self.staged_custom_params_meta_scratch
                    .reserve(custom.params.len());
                self.staged_custom_params_values_scratch
                    .reserve(custom.params.len() * 4);
                self.staged_custom_params_key_scratch
                    .reserve(custom.params.len() * 5);
                for param in custom.params.as_ref() {
                    let value_offset = self.staged_custom_params_values_scratch.len() as u32;
                    let kind = encode_custom_param_value_packed(
                        &param.value,
                        &mut self.staged_custom_params_values_scratch,
                    );
                    self.staged_custom_params_meta_scratch
                        .push((value_offset << 2) | kind);
                    self.staged_custom_params_key_scratch.push(kind);
                    match kind {
                        CUSTOM_PARAM_KIND_SCALAR => {
                            self.staged_custom_params_key_scratch.push(
                                self.staged_custom_params_values_scratch[value_offset as usize]
                                    .to_bits(),
                            );
                        }
                        CUSTOM_PARAM_KIND_VEC2 => {
                            self.staged_custom_params_key_scratch.push(
                                self.staged_custom_params_values_scratch[value_offset as usize]
                                    .to_bits(),
                            );
                            self.staged_custom_params_key_scratch.push(
                                self.staged_custom_params_values_scratch[value_offset as usize + 1]
                                    .to_bits(),
                            );
                        }
                        CUSTOM_PARAM_KIND_VEC3 => {
                            self.staged_custom_params_key_scratch.push(
                                self.staged_custom_params_values_scratch[value_offset as usize]
                                    .to_bits(),
                            );
                            self.staged_custom_params_key_scratch.push(
                                self.staged_custom_params_values_scratch[value_offset as usize + 1]
                                    .to_bits(),
                            );
                            self.staged_custom_params_key_scratch.push(
                                self.staged_custom_params_values_scratch[value_offset as usize + 2]
                                    .to_bits(),
                            );
                        }
                        _ => {
                            self.staged_custom_params_key_scratch.push(
                                self.staged_custom_params_values_scratch[value_offset as usize]
                                    .to_bits(),
                            );
                            self.staged_custom_params_key_scratch.push(
                                self.staged_custom_params_values_scratch[value_offset as usize + 1]
                                    .to_bits(),
                            );
                            self.staged_custom_params_key_scratch.push(
                                self.staged_custom_params_values_scratch[value_offset as usize + 2]
                                    .to_bits(),
                            );
                            self.staged_custom_params_key_scratch.push(
                                self.staged_custom_params_values_scratch[value_offset as usize + 3]
                                    .to_bits(),
                            );
                        }
                    }
                }
                if let Some(&cached) = self
                    .staged_custom_params_dedupe
                    .get(self.staged_custom_params_key_scratch.as_slice())
                {
                    return cached;
                }
                let offset = self.staged_custom_params_meta.len() as u32;
                let value_base = self.staged_custom_params_values.len() as u32;
                for meta in &self.staged_custom_params_meta_scratch {
                    let kind = *meta & 0x3;
                    let rel_offset = *meta >> 2;
                    self.staged_custom_params_meta
                        .push(((value_base + rel_offset) << 2) | kind);
                }
                self.staged_custom_params_values
                    .extend_from_slice(&self.staged_custom_params_values_scratch);
                let len = self.staged_custom_params_meta_scratch.len() as u32;
                self.staged_custom_params_dedupe
                    .insert(self.staged_custom_params_key_scratch.clone(), (offset, len));
                (offset, len)
            }
            _ => (0, 0),
        }
    }
}
