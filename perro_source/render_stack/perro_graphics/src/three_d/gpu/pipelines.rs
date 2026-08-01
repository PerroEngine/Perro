use super::*;

fn next_custom_pipeline_token(
    tokens: &AHashMap<CustomPipelineKey, u32>,
    next_token: &mut u32,
) -> u32 {
    loop {
        let token = (*next_token).max(1);
        *next_token = token.wrapping_add(1).max(1);
        if !tokens.values().any(|used| *used == token) {
            return token;
        }
    }
}

fn clear_custom_pipeline_maps<T>(
    custom_pipelines: &mut AHashMap<u32, T>,
    custom_pipelines_rigid: &mut AHashMap<u32, T>,
    custom_pipelines_multimesh: &mut AHashMap<u32, T>,
    custom_pipeline_tokens: &mut AHashMap<CustomPipelineKey, u32>,
    custom_shader_sources: &mut AHashMap<Arc<str>, Arc<str>>,
    custom_pipeline_vertex_hooks: &mut AHashMap<u32, bool>,
) {
    custom_pipelines.clear();
    custom_pipelines_rigid.clear();
    custom_pipelines_multimesh.clear();
    custom_pipeline_tokens.clear();
    custom_shader_sources.clear();
    custom_pipeline_vertex_hooks.clear();
}

fn builtin_shader_features(material: &Material3D, receive_shadows: bool) -> MaterialShaderFeatures {
    let params = material.standard_params();
    MaterialShaderFeatures::new(
        params.base_color_texture != MATERIAL_TEXTURE_NONE,
        params.metallic_roughness_texture != MATERIAL_TEXTURE_NONE,
        params.normal_texture != MATERIAL_TEXTURE_NONE,
        params.occlusion_texture != MATERIAL_TEXTURE_NONE,
        params.emissive_texture != MATERIAL_TEXTURE_NONE,
        receive_shadows && !matches!(material, Material3D::Unlit(_)),
        params.alpha_mode,
        !material.vertex_modifiers().is_empty(),
    )
}

impl Gpu3D {
    fn custom_pipeline_token(&mut self, key: CustomPipelineKey) -> u32 {
        if let Some(&token) = self.custom_pipeline_tokens.get(&key) {
            return token;
        }
        let token = next_custom_pipeline_token(
            &self.custom_pipeline_tokens,
            &mut self.next_custom_pipeline_token,
        );
        self.custom_pipeline_tokens.insert(key, token);
        token
    }

    fn invalidate_custom_shader_path(&mut self, shader_path: &str) {
        let stale_tokens = self
            .custom_pipeline_tokens
            .iter()
            .filter_map(|(key, token)| (key.shader_path.as_ref() == shader_path).then_some(*token))
            .collect::<AHashSet<_>>();
        if stale_tokens.is_empty() {
            return;
        }
        self.custom_pipeline_tokens
            .retain(|_, token| !stale_tokens.contains(token));
        self.custom_pipeline_vertex_hooks
            .retain(|token, _| !stale_tokens.contains(token));
        self.custom_pipelines
            .retain(|token, _| !stale_tokens.contains(token));
        self.custom_pipelines_rigid
            .retain(|token, _| !stale_tokens.contains(token));
        self.custom_pipelines_multimesh
            .retain(|token, _| !stale_tokens.contains(token));
    }

    fn cache_custom_shader_source(
        &mut self,
        shader_path: &str,
        source: &str,
    ) -> (Arc<str>, Arc<str>) {
        if let Some((cached_path, cached_source)) =
            self.custom_shader_sources.get_key_value(shader_path)
            && cached_source.as_ref() == source
        {
            return (cached_path.clone(), cached_source.clone());
        }
        self.invalidate_custom_shader_path(shader_path);
        let path: Arc<str> = Arc::from(shader_path);
        let source: Arc<str> = Arc::from(source);
        self.custom_shader_sources
            .insert(path.clone(), source.clone());
        (path, source)
    }

    pub(crate) fn invalidate_custom_pipelines(&mut self) {
        clear_custom_pipeline_maps(
            &mut self.custom_pipelines,
            &mut self.custom_pipelines_rigid,
            &mut self.custom_pipelines_multimesh,
            &mut self.custom_pipeline_tokens,
            &mut self.custom_shader_sources,
            &mut self.custom_pipeline_vertex_hooks,
        );
        self.rebuild_batch_views();
    }

    pub(super) fn ensure_custom_pipeline(
        &mut self,
        device: &wgpu::Device,
        path: RenderPath3D,
        shader_path: &str,
        lighting: CustomMaterialLighting3D,
        alpha_mode: u8,
        static_shader_lookup: Option<StaticShaderLookup>,
    ) -> Option<u32> {
        let cached_source = self
            .custom_shader_sources
            .get_key_value(shader_path)
            .map(|(path, source)| (path.clone(), source.clone()));
        let (shader_path, shader_source) = if let Some(cached) = cached_source {
            cached
        } else {
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
            self.cache_custom_shader_source(shader_path, src.as_ref())
        };
        let has_vertex_hook = shader_source.contains("shade_vertex(");
        let token = self.custom_pipeline_token(CustomPipelineKey {
            shader_path,
            shader_source: shader_source.clone(),
            lighting,
            alpha_mode,
            vertex_hook: has_vertex_hook,
        });
        if path == RenderPath3D::Rigid && self.custom_pipelines_rigid.contains_key(&token) {
            return Some(token);
        }
        if path == RenderPath3D::Skinned && self.custom_pipelines.contains_key(&token) {
            return Some(token);
        }
        if path == RenderPath3D::MultiMesh && self.custom_pipelines_multimesh.contains_key(&token) {
            return Some(token);
        }
        // Record whether this shader defines a shade_vertex hook (same probe
        // as build_material_shader composition). Depth-only passes consult
        // this: a hook displaces geometry the shared depth shaders can't
        // replicate, so hooked customs stay out of shadow/prepass batches.
        self.custom_pipeline_vertex_hooks
            .insert(token, has_vertex_hook);
        let wgsl = if path == RenderPath3D::MultiMesh {
            build_custom_multimesh_material_shader(shader_source.as_ref(), lighting)
        } else if path == RenderPath3D::Rigid {
            build_custom_material_shader_with_prelude(
                prelude_rigid_wgsl(),
                shader_source.as_ref(),
                lighting,
            )
        } else {
            build_custom_material_shader_with_prelude(
                prelude_skinned_wgsl(),
                shader_source.as_ref(),
                lighting,
            )
        };
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_mesh_custom"),
            source: wgpu::ShaderSource::Wgsl(wgsl.into()),
        });
        let registry = self.pipelines.clone();
        let pipeline_culled = if path == RenderPath3D::MultiMesh {
            create_multimesh_pipeline(
                device,
                registry.multimesh_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                Some(wgpu::Face::Back),
            )
        } else if path == RenderPath3D::Rigid {
            create_pipeline_rigid(
                device,
                registry.rigid_material_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                Some(wgpu::Face::Back),
            )
        } else {
            create_pipeline_skinned(
                device,
                registry.material_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                Some(wgpu::Face::Back),
            )
        };
        let pipeline_double_sided = if path == RenderPath3D::MultiMesh {
            create_multimesh_pipeline(
                device,
                registry.multimesh_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                None,
            )
        } else if path == RenderPath3D::Rigid {
            create_pipeline_rigid(
                device,
                registry.rigid_material_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                None,
            )
        } else {
            create_pipeline_skinned(
                device,
                registry.material_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                None,
            )
        };
        let pipeline_blend_culled = if path == RenderPath3D::MultiMesh {
            create_multimesh_blend_pipeline(
                device,
                registry.multimesh_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                Some(wgpu::Face::Back),
            )
        } else if path == RenderPath3D::Rigid {
            create_pipeline_rigid_blend(
                device,
                registry.rigid_material_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                Some(wgpu::Face::Back),
            )
        } else {
            create_pipeline_skinned_blend(
                device,
                registry.material_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                Some(wgpu::Face::Back),
            )
        };
        let pipeline_blend_double_sided = if path == RenderPath3D::MultiMesh {
            create_multimesh_blend_pipeline(
                device,
                registry.multimesh_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                None,
            )
        } else if path == RenderPath3D::Rigid {
            create_pipeline_rigid_blend(
                device,
                registry.rigid_material_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                None,
            )
        } else {
            create_pipeline_skinned_blend(
                device,
                registry.material_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                None,
            )
        };
        let tick = self.pipeline_gc_tick;
        let map = if path == RenderPath3D::MultiMesh {
            &mut self.custom_pipelines_multimesh
        } else if path == RenderPath3D::Rigid {
            &mut self.custom_pipelines_rigid
        } else {
            &mut self.custom_pipelines
        };
        map.insert(
            token,
            TrackedPipelines {
                pipelines: CustomPipeline {
                    pipeline_culled,
                    pipeline_double_sided,
                    pipeline_blend_culled,
                    pipeline_blend_double_sided,
                },
                last_used_tick: tick,
            },
        );
        Some(token)
    }

    // Pre-compile every render-path pipeline this material can hit so its
    // first visible draw never pays shader-module + pipeline creation.
    // Reuses material_pipeline_kind, so cache hits make repeats free and
    // builtin non-variant materials cost nothing (prebuilt at init).
    pub(crate) fn warm_material_pipelines(
        &mut self,
        device: &wgpu::Device,
        material: &Material3D,
        static_shader_lookup: Option<StaticShaderLookup>,
    ) {
        for path in [
            RenderPath3D::Rigid,
            RenderPath3D::Skinned,
            RenderPath3D::MultiMesh,
        ] {
            let _ = self.material_pipeline_kind(device, path, material, true, static_shader_lookup);
        }
    }

    pub(super) fn material_pipeline_kind(
        &mut self,
        device: &wgpu::Device,
        render_path: RenderPath3D,
        material: &Material3D,
        receive_shadows: bool,
        static_shader_lookup: Option<StaticShaderLookup>,
    ) -> MaterialPipelineKind {
        let use_variants = self.shader_variant_mode == crate::ShaderVariantMode::Auto;
        match material {
            Material3D::Standard(_) => {
                let features = builtin_shader_features(material, receive_shadows);
                if use_variants
                    && self.ensure_builtin_variant_pipeline(
                        device,
                        render_path,
                        BuiltinShaderKind::Standard,
                        features,
                    )
                {
                    MaterialPipelineKind::StandardVariant(features)
                } else {
                    MaterialPipelineKind::Standard
                }
            }
            Material3D::Unlit(_) => {
                let features = builtin_shader_features(material, receive_shadows);
                if use_variants
                    && self.ensure_builtin_variant_pipeline(
                        device,
                        render_path,
                        BuiltinShaderKind::Unlit,
                        features,
                    )
                {
                    MaterialPipelineKind::UnlitVariant(features)
                } else {
                    MaterialPipelineKind::Unlit
                }
            }
            Material3D::Toon(_) | Material3D::HandDrawn(_) | Material3D::PixelSurface(_) => {
                let features = builtin_shader_features(material, receive_shadows);
                if use_variants
                    && self.ensure_builtin_variant_pipeline(
                        device,
                        render_path,
                        BuiltinShaderKind::Toon,
                        features,
                    )
                {
                    MaterialPipelineKind::ToonVariant(features)
                } else {
                    MaterialPipelineKind::Toon
                }
            }
            Material3D::Custom(custom) => {
                let shader_path = custom.shader_path.as_ref();
                if let Some(token) = self.ensure_custom_pipeline(
                    device,
                    render_path,
                    shader_path,
                    custom.lighting,
                    custom.surface.alpha_mode,
                    static_shader_lookup,
                ) {
                    MaterialPipelineKind::Custom(token)
                } else {
                    MaterialPipelineKind::Standard
                }
            }
        }
    }

    fn ensure_builtin_variant_pipeline(
        &mut self,
        device: &wgpu::Device,
        path: RenderPath3D,
        kind: BuiltinShaderKind,
        features: MaterialShaderFeatures,
    ) -> bool {
        if path == RenderPath3D::MultiMesh {
            return false;
        }
        let key = BuiltinPipelineKey {
            path,
            kind,
            features,
        };
        if self.builtin_variant_pipelines.contains_key(&key) {
            return true;
        }
        let shader = if path == RenderPath3D::Rigid {
            create_standard_shader_module_rigid_variant(device, kind, features)
        } else {
            create_standard_shader_module_skinned_variant(device, kind, features)
        };
        let registry = self.pipelines.clone();
        let pipeline_culled = if path == RenderPath3D::Rigid {
            create_pipeline_rigid(
                device,
                registry.rigid_material_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                Some(wgpu::Face::Back),
            )
        } else {
            create_pipeline_skinned(
                device,
                registry.material_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                Some(wgpu::Face::Back),
            )
        };
        let pipeline_double_sided = if path == RenderPath3D::Rigid {
            create_pipeline_rigid(
                device,
                registry.rigid_material_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                None,
            )
        } else {
            create_pipeline_skinned(
                device,
                registry.material_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                None,
            )
        };
        let pipeline_blend_culled = if path == RenderPath3D::Rigid {
            create_pipeline_rigid_blend(
                device,
                registry.rigid_material_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                Some(wgpu::Face::Back),
            )
        } else {
            create_pipeline_skinned_blend(
                device,
                registry.material_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                Some(wgpu::Face::Back),
            )
        };
        let pipeline_blend_double_sided = if path == RenderPath3D::Rigid {
            create_pipeline_rigid_blend(
                device,
                registry.rigid_material_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                None,
            )
        } else {
            create_pipeline_skinned_blend(
                device,
                registry.material_layout(),
                &shader,
                self.color_format,
                self.sample_count,
                None,
            )
        };
        self.builtin_variant_pipelines.insert(
            key,
            TrackedPipelines {
                pipelines: CustomPipeline {
                    pipeline_culled,
                    pipeline_double_sided,
                    pipeline_blend_culled,
                    pipeline_blend_double_sided,
                },
                last_used_tick: self.pipeline_gc_tick,
            },
        );
        true
    }

    pub(super) fn pipeline_for_batch(&self, batch: &DrawBatch) -> &wgpu::RenderPipeline {
        let is_rigid = batch.path == RenderPath3D::Rigid;
        // Unified depth: batches the prepass rasterized already have exact
        // depth in depth_view (copied there after the prepass), so they only
        // need a read-only LessEqual test. LessEqual + write-off is robust
        // against cross-pipeline position invariance, unlike Equal. The
        // predicate mirrors depth_prepass_batch_indices in rebuild_batch_views.
        let prepass_covered = self.unified_depth_active
            && !batch.draw_on_top
            && batch.alpha_mode != 2
            && !batch.mesh_blend
            && batch_depth_safe(batch, &self.custom_pipeline_vertex_hooks);
        // Alpha-blended batches must not write depth, or transparents drawn
        // first occlude transparents behind them; the *_blend pipelines are
        // the same state with depth write off.
        let soft_depth = batch.mesh_blend || batch.alpha_mode == 2 || prepass_covered;
        let reg = &*self.pipelines;
        if batch.draw_on_top {
            let pair = if is_rigid {
                reg.rigid_overlay()
            } else {
                reg.skinned_overlay()
            };
            return pair.select(batch.double_sided);
        }
        let builtin_variant = match &batch.material_kind {
            MaterialPipelineKind::StandardVariant(features) => {
                Some((BuiltinShaderKind::Standard, *features))
            }
            MaterialPipelineKind::UnlitVariant(features) => {
                Some((BuiltinShaderKind::Unlit, *features))
            }
            MaterialPipelineKind::ToonVariant(features) => {
                Some((BuiltinShaderKind::Toon, *features))
            }
            _ => None,
        };
        if let Some((kind, features)) = builtin_variant
            && !batch.packed_lod
            && let Some(entry) = self.builtin_variant_pipelines.get(&BuiltinPipelineKey {
                path: batch.path,
                kind,
                features,
            })
        {
            let pipeline = &entry.pipelines;
            return if soft_depth && batch.double_sided {
                &pipeline.pipeline_blend_double_sided
            } else if soft_depth {
                &pipeline.pipeline_blend_culled
            } else if batch.double_sided {
                &pipeline.pipeline_double_sided
            } else {
                &pipeline.pipeline_culled
            };
        }
        match &batch.material_kind {
            MaterialPipelineKind::Standard | MaterialPipelineKind::StandardVariant(_) => {
                let pair = if batch.packed_lod && is_rigid {
                    if soft_depth {
                        reg.rigid_packed_lod_blend()
                    } else {
                        reg.rigid_packed_lod()
                    }
                } else if is_rigid {
                    if soft_depth {
                        reg.rigid_standard_blend()
                    } else {
                        reg.rigid_standard()
                    }
                } else if soft_depth {
                    reg.skinned_standard_blend()
                } else {
                    reg.skinned_standard()
                };
                pair.select(batch.double_sided)
            }
            MaterialPipelineKind::Unlit | MaterialPipelineKind::UnlitVariant(_) => {
                let pair = if is_rigid {
                    if soft_depth {
                        reg.rigid_unlit_blend()
                    } else {
                        reg.rigid_unlit()
                    }
                } else if soft_depth {
                    reg.skinned_unlit_blend()
                } else {
                    reg.skinned_unlit()
                };
                pair.select(batch.double_sided)
            }
            MaterialPipelineKind::Toon | MaterialPipelineKind::ToonVariant(_) => {
                let pair = if is_rigid {
                    if soft_depth {
                        reg.rigid_toon_blend()
                    } else {
                        reg.rigid_toon()
                    }
                } else if soft_depth {
                    reg.skinned_toon_blend()
                } else {
                    reg.skinned_toon()
                };
                pair.select(batch.double_sided)
            }
            MaterialPipelineKind::Custom(token) => {
                let map = if is_rigid {
                    &self.custom_pipelines_rigid
                } else {
                    &self.custom_pipelines
                };
                if let Some(entry) = map.get(token) {
                    let pipeline = &entry.pipelines;
                    if soft_depth && batch.double_sided {
                        &pipeline.pipeline_blend_double_sided
                    } else if soft_depth {
                        &pipeline.pipeline_blend_culled
                    } else if batch.double_sided {
                        &pipeline.pipeline_double_sided
                    } else {
                        &pipeline.pipeline_culled
                    }
                } else {
                    // Custom pipeline missing (still compiling or evicted):
                    // fall back to the standard family.
                    let pair = if is_rigid {
                        if soft_depth {
                            reg.rigid_standard_blend()
                        } else {
                            reg.rigid_standard()
                        }
                    } else if soft_depth {
                        reg.skinned_standard_blend()
                    } else {
                        reg.skinned_standard()
                    };
                    pair.select(batch.double_sided)
                }
            }
        }
    }

    /// Periodic (GC-tick) LRU sweep over the content-keyed pipeline caches:
    /// `builtin_variant_pipelines` (keyed by (path, kind, features) - up to
    /// thousands of combos, each one shader module + 4 pipelines) and the
    /// three `custom_pipelines*` maps. An entry is live while any retained
    /// draw/multimesh batch references it; live entries are re-stamped every
    /// sweep, and entries unreferenced for [`PIPELINE_EVICT_GC_TICKS`] sweeps
    /// are dropped (lazily rebuilt by a later batch rebuild if the material
    /// returns). `builtin_variant_pipelines` is additionally capped at
    /// [`BUILTIN_VARIANT_PIPELINE_CAP`] entries, evicting the stalest
    /// unreferenced entries first. Only unreferenced entries are ever
    /// removed, so `pipeline_for_batch` cannot lose a pipeline out from under
    /// a live batch (its builtin fallback covers even that defensively).
    pub(super) fn evict_stale_pipelines(&mut self) {
        self.pipeline_gc_tick = self.pipeline_gc_tick.wrapping_add(1);
        let tick = self.pipeline_gc_tick;
        for batch in &self.draw_batches {
            match &batch.material_kind {
                MaterialPipelineKind::StandardVariant(features) => {
                    if let Some(entry) =
                        self.builtin_variant_pipelines.get_mut(&BuiltinPipelineKey {
                            path: batch.path,
                            kind: BuiltinShaderKind::Standard,
                            features: *features,
                        })
                    {
                        entry.last_used_tick = tick;
                    }
                }
                MaterialPipelineKind::UnlitVariant(features) => {
                    if let Some(entry) =
                        self.builtin_variant_pipelines.get_mut(&BuiltinPipelineKey {
                            path: batch.path,
                            kind: BuiltinShaderKind::Unlit,
                            features: *features,
                        })
                    {
                        entry.last_used_tick = tick;
                    }
                }
                MaterialPipelineKind::ToonVariant(features) => {
                    if let Some(entry) =
                        self.builtin_variant_pipelines.get_mut(&BuiltinPipelineKey {
                            path: batch.path,
                            kind: BuiltinShaderKind::Toon,
                            features: *features,
                        })
                    {
                        entry.last_used_tick = tick;
                    }
                }
                MaterialPipelineKind::Custom(token) => {
                    let map = if batch.path == RenderPath3D::Rigid {
                        &mut self.custom_pipelines_rigid
                    } else {
                        &mut self.custom_pipelines
                    };
                    if let Some(entry) = map.get_mut(token) {
                        entry.last_used_tick = tick;
                    }
                }
                _ => {}
            }
        }
        for batch in &self.multimesh_batches {
            // Builtin variants are stamped here too: multimesh batches never
            // appear in `draw_batches`, and a rebuild that reuses the staged
            // multimesh buffers never re-resolves their material kind, so this
            // sweep is the only thing keeping their pipelines live.
            let builtin_kind = match &batch.material_kind {
                MaterialPipelineKind::StandardVariant(features) => {
                    Some((BuiltinShaderKind::Standard, *features))
                }
                MaterialPipelineKind::UnlitVariant(features) => {
                    Some((BuiltinShaderKind::Unlit, *features))
                }
                MaterialPipelineKind::ToonVariant(features) => {
                    Some((BuiltinShaderKind::Toon, *features))
                }
                MaterialPipelineKind::Custom(token) => {
                    if let Some(entry) = self.custom_pipelines_multimesh.get_mut(token) {
                        entry.last_used_tick = tick;
                    }
                    None
                }
                _ => None,
            };
            if let Some((kind, features)) = builtin_kind
                && let Some(entry) = self.builtin_variant_pipelines.get_mut(&BuiltinPipelineKey {
                    path: RenderPath3D::MultiMesh,
                    kind,
                    features,
                })
            {
                entry.last_used_tick = tick;
            }
        }
        let live = |entry: &TrackedPipelines| {
            tick.wrapping_sub(entry.last_used_tick) <= PIPELINE_EVICT_GC_TICKS
        };
        self.builtin_variant_pipelines.retain(|_, entry| live(entry));
        self.custom_pipelines.retain(|_, entry| live(entry));
        self.custom_pipelines_rigid.retain(|_, entry| live(entry));
        self.custom_pipelines_multimesh
            .retain(|_, entry| live(entry));
        if self.builtin_variant_pipelines.len() > BUILTIN_VARIANT_PIPELINE_CAP {
            let excess = self.builtin_variant_pipelines.len() - BUILTIN_VARIANT_PIPELINE_CAP;
            let mut stale: Vec<(BuiltinPipelineKey, u64)> = self
                .builtin_variant_pipelines
                .iter()
                .filter(|(_, entry)| entry.last_used_tick != tick)
                .map(|(key, entry)| (*key, entry.last_used_tick))
                .collect();
            stale.sort_by_key(|&(_, last_used)| last_used);
            for (key, _) in stale.into_iter().take(excess) {
                self.builtin_variant_pipelines.remove(&key);
            }
        }
    }

    /// Stage `material`'s vertex modifiers + custom shader params into the
    /// shared arena the regular draws own. Returns `(header offset, param
    /// count)`; `(0, 0)` means the material carries neither.
    #[inline]
    pub(super) fn stage_custom_params(&mut self, material: &Material3D) -> (u32, u32) {
        self.stage_custom_params_into(material, false)
    }

    /// Same, but into the multimesh tail arena (tail-local offsets, rebased
    /// onto the regular arena lengths by `rebase_multimesh_custom_params_tail`).
    /// Keeping dense draws out of the shared arena is what lets a full rebuild
    /// reuse their staged rows verbatim.
    #[inline]
    pub(super) fn stage_multimesh_custom_params(&mut self, material: &Material3D) -> (u32, u32) {
        self.stage_custom_params_into(material, true)
    }

    fn stage_custom_params_into(&mut self, material: &Material3D, tail: bool) -> (u32, u32) {
        let modifiers = material.vertex_modifiers();
        let custom_params = match material {
            Material3D::Custom(custom) => custom.params.as_ref(),
            _ => &[],
        };
        if modifiers.is_empty() && custom_params.is_empty() {
            return (0, 0);
        }

        // Taken out so the commit below can borrow the destination arenas.
        let mut key = std::mem::take(&mut self.staged_custom_params_key_scratch);
        let mut meta_scratch = std::mem::take(&mut self.staged_custom_params_meta_scratch);
        let mut values_scratch = std::mem::take(&mut self.staged_custom_params_values_scratch);
        key.clear();
        meta_scratch.clear();
        values_scratch.clear();
        key.push(modifiers.len() as u32);
        for modifier in modifiers {
            encode_vertex_modifier(modifier, &mut values_scratch);
        }
        key.extend(values_scratch.iter().map(|value| value.to_bits()));

        for param in custom_params {
            let value_offset = values_scratch.len() as u32;
            let kind = encode_custom_param_value_packed(&param.value, &mut values_scratch);
            meta_scratch.push((value_offset << 2) | kind);
            key.push(kind);
            let value_len = match kind {
                CUSTOM_PARAM_KIND_SCALAR => 1,
                CUSTOM_PARAM_KIND_VEC2 => 2,
                CUSTOM_PARAM_KIND_VEC3 => 3,
                _ => 4,
            };
            key.extend(
                values_scratch[value_offset as usize..value_offset as usize + value_len]
                    .iter()
                    .map(|value| value.to_bits()),
            );
        }

        let staged = CustomParamsStaging {
            key: &key,
            meta: &meta_scratch,
            values: &values_scratch,
            modifier_count: modifiers.len() as u32,
        };
        let result = if tail {
            CustomParamsArena {
                meta: &mut self.staged_multimesh_custom_params_meta,
                values: &mut self.staged_multimesh_custom_params_values,
                dedupe: &mut self.staged_multimesh_custom_params_dedupe,
                entry_starts: Some(&mut self.staged_multimesh_custom_params_entry_starts),
            }
            .commit(staged)
        } else {
            CustomParamsArena {
                meta: &mut self.staged_custom_params_meta,
                values: &mut self.staged_custom_params_values,
                dedupe: &mut self.staged_custom_params_dedupe,
                entry_starts: None,
            }
            .commit(staged)
        };

        self.staged_custom_params_key_scratch = key;
        self.staged_custom_params_meta_scratch = meta_scratch;
        self.staged_custom_params_values_scratch = values_scratch;
        result
    }
}

/// One encoded material's params, ready to be appended to an arena.
struct CustomParamsStaging<'a> {
    /// Dedupe key: modifier count + every value bit pattern + param kinds.
    key: &'a [u32],
    /// Param words with arena-relative value offsets: `(offset << 2) | kind`.
    meta: &'a [u32],
    values: &'a [f32],
    modifier_count: u32,
}

/// Destination arena for `stage_custom_params_into`: either the shared one the
/// regular draws fill or the multimesh tail.
struct CustomParamsArena<'a> {
    meta: &'a mut Vec<u32>,
    values: &'a mut Vec<f32>,
    dedupe: &'a mut AHashMap<Vec<u32>, (u32, u32)>,
    /// Tail only: header start of each entry, so the rebase can walk the arena.
    entry_starts: Option<&'a mut Vec<u32>>,
}

impl CustomParamsArena<'_> {
    fn commit(self, staged: CustomParamsStaging<'_>) -> (u32, u32) {
        if let Some(&cached) = self.dedupe.get(staged.key) {
            return cached;
        }
        let header_offset = self.meta.len() as u32;
        let value_base = self.values.len() as u32;
        if let Some(starts) = self.entry_starts {
            starts.push(header_offset);
        }
        self.meta.push(value_base);
        self.meta.push(staged.modifier_count);
        for meta in staged.meta {
            let kind = *meta & 0x3;
            let rel_offset = *meta >> 2;
            self.meta.push(((value_base + rel_offset) << 2) | kind);
        }
        self.values.extend_from_slice(staged.values);
        // `header_offset + 2` skips the two header words; a real entry is
        // therefore always >= 2, which is what makes 0 a usable "none" marker.
        let result = (header_offset + 2, staged.meta.len() as u32);
        self.dedupe.insert(staged.key.to_vec(), result);
        result
    }
}

fn encode_vertex_modifier(modifier: &VertexModifier3D, out: &mut Vec<f32>) {
    let mut record = [0.0f32; 16];
    match *modifier {
        VertexModifier3D::Wind {
            direction,
            strength,
            speed,
            frequency,
            mask,
        } => {
            record[0] = 0.0;
            record[4..7].copy_from_slice(&direction);
            record[7] = strength;
            record[8] = speed;
            record[9] = frequency;
            encode_vertex_mask(Some(mask), &mut record);
        }
        VertexModifier3D::Wave {
            axis,
            direction,
            amplitude,
            speed,
            frequency,
            phase,
            mask,
        } => {
            record[0] = 1.0;
            record[1] = vertex_axis_code(axis);
            record[4..7].copy_from_slice(&direction);
            record[7] = amplitude;
            record[8] = speed;
            record[9] = frequency;
            record[10] = phase;
            encode_vertex_mask(mask, &mut record);
        }
        VertexModifier3D::Bend {
            along_axis,
            bend_axis,
            angle_radians,
            start,
            end,
        } => {
            record[0] = 2.0;
            record[1] = vertex_axis_code(along_axis);
            record[2] = vertex_axis_code(bend_axis);
            record[4] = angle_radians;
            record[5] = start;
            record[6] = end;
        }
        VertexModifier3D::Twist {
            axis,
            angle_radians,
            start,
            end,
        } => {
            record[0] = 3.0;
            record[1] = vertex_axis_code(axis);
            record[4] = angle_radians;
            record[5] = start;
            record[6] = end;
        }
        VertexModifier3D::Inflate { amount, mask } => {
            record[0] = 4.0;
            record[4] = amount;
            encode_vertex_mask(mask, &mut record);
        }
        VertexModifier3D::Jitter {
            amount,
            scale,
            rate,
            seed,
            mask,
        } => {
            record[0] = 5.0;
            record[4] = amount;
            record[5] = scale;
            record[6] = rate;
            record[7] = seed as f32;
            encode_vertex_mask(mask, &mut record);
        }
        VertexModifier3D::PixelSnap {
            virtual_height,
            strength,
        } => {
            record[0] = 6.0;
            record[4] = virtual_height as f32;
            record[5] = strength;
        }
    }
    out.extend_from_slice(&record);
}

fn encode_vertex_mask(mask: Option<VertexModifierMask3D>, record: &mut [f32; 16]) {
    if let Some(mask) = mask {
        record[12] = 1.0;
        record[13] = vertex_axis_code(mask.axis);
        record[14] = mask.start;
        record[15] = mask.end;
    }
}

fn vertex_axis_code(axis: VertexAxis3D) -> f32 {
    match axis {
        VertexAxis3D::X => 0.0,
        VertexAxis3D::Y => 1.0,
        VertexAxis3D::Z => 2.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pipeline_key(
        shader_path: &str,
        shader_source: &str,
        lighting: CustomMaterialLighting3D,
        alpha_mode: u8,
        vertex_hook: bool,
    ) -> CustomPipelineKey {
        CustomPipelineKey {
            shader_path: Arc::from(shader_path),
            shader_source: Arc::from(shader_source),
            lighting,
            alpha_mode,
            vertex_hook,
        }
    }

    fn token_for(
        tokens: &mut AHashMap<CustomPipelineKey, u32>,
        next_token: &mut u32,
        key: CustomPipelineKey,
    ) -> u32 {
        if let Some(token) = tokens.get(&key) {
            return *token;
        }
        let token = next_custom_pipeline_token(tokens, next_token);
        tokens.insert(key, token);
        token
    }

    #[test]
    fn builtin_features_derive_from_material_without_dev_flags() {
        let plain = Material3D::Standard(StandardMaterial3D::default());
        assert_eq!(
            builtin_shader_features(&plain, false),
            MaterialShaderFeatures::new(false, false, false, false, false, false, 0, false)
        );

        let textured = Material3D::Standard(StandardMaterial3D {
            base_color_texture: 1,
            metallic_roughness_texture: 2,
            normal_texture: 3,
            occlusion_texture: 4,
            emissive_texture: 5,
            alpha_mode: 1,
            ..StandardMaterial3D::default()
        });
        assert_eq!(
            builtin_shader_features(&textured, true),
            MaterialShaderFeatures::new(true, true, true, true, true, true, 1, false)
        );

        let unlit = Material3D::Unlit(perro_render_bridge::UnlitMaterial3D::default());
        assert_eq!(
            builtin_shader_features(&unlit, true),
            MaterialShaderFeatures::new(false, false, false, false, false, false, 0, false)
        );
    }

    #[test]
    fn ui_subview_custom_pipelines_stay_exact_across_reentry_and_reload() {
        let raw_bg = pipeline_key(
            "res://cloud_stitched.wgsl",
            "fn shade() {}",
            CustomMaterialLighting3D::Raw,
            0,
            false,
        );
        let standard_logo = pipeline_key(
            "res://logo_circus.wgsl",
            "fn shade_vertex() {}",
            CustomMaterialLighting3D::Standard,
            0,
            true,
        );
        let mut tokens = AHashMap::new();
        let mut hooks = AHashMap::new();
        let mut next_token = 1;
        let bg_token = token_for(&mut tokens, &mut next_token, raw_bg.clone());
        let logo_token = token_for(&mut tokens, &mut next_token, standard_logo.clone());
        hooks.insert(bg_token, raw_bg.vertex_hook);
        hooks.insert(logo_token, standard_logo.vertex_hook);

        assert_ne!(bg_token, logo_token);
        assert_eq!(
            token_for(&mut tokens, &mut next_token, raw_bg.clone()),
            bg_token
        );
        assert_eq!(
            token_for(&mut tokens, &mut next_token, standard_logo),
            logo_token
        );
        assert_eq!(hooks.get(&bg_token), Some(&false));
        assert_eq!(hooks.get(&logo_token), Some(&true));

        let reloaded_bg = pipeline_key(
            "res://cloud_stitched.wgsl",
            "fn shade_vertex() {}",
            CustomMaterialLighting3D::Raw,
            0,
            true,
        );
        let reloaded_token = token_for(&mut tokens, &mut next_token, reloaded_bg.clone());
        hooks.insert(reloaded_token, reloaded_bg.vertex_hook);
        let blend_token = token_for(
            &mut tokens,
            &mut next_token,
            pipeline_key(
                "res://cloud_stitched.wgsl",
                "fn shade_vertex() {}",
                CustomMaterialLighting3D::Raw,
                2,
                true,
            ),
        );

        assert_ne!(reloaded_token, bg_token);
        assert_ne!(blend_token, reloaded_token);
        assert_eq!(hooks.get(&bg_token), Some(&false));
        assert_eq!(hooks.get(&reloaded_token), Some(&true));
    }

    #[test]
    fn custom_pipeline_invalidation_clears_all_token_scoped_state() {
        let key = pipeline_key(
            "res://bg.wgsl",
            "fn shade() {}",
            CustomMaterialLighting3D::Raw,
            0,
            false,
        );
        let mut pipelines = AHashMap::from_iter([(7, ())]);
        let mut rigid = AHashMap::from_iter([(7, ())]);
        let mut multimesh = AHashMap::from_iter([(7, ())]);
        let mut tokens = AHashMap::from_iter([(key, 7)]);
        let mut sources =
            AHashMap::from_iter([(Arc::from("res://bg.wgsl"), Arc::from("fn shade() {}"))]);
        let mut hooks = AHashMap::from_iter([(7, false)]);

        clear_custom_pipeline_maps(
            &mut pipelines,
            &mut rigid,
            &mut multimesh,
            &mut tokens,
            &mut sources,
            &mut hooks,
        );

        assert!(pipelines.is_empty());
        assert!(rigid.is_empty());
        assert!(multimesh.is_empty());
        assert!(tokens.is_empty());
        assert!(sources.is_empty());
        assert!(hooks.is_empty());
    }

    #[test]
    fn vertex_modifier_gpu_record_keeps_kind_params_and_mask() {
        let modifier = VertexModifier3D::Wave {
            axis: VertexAxis3D::Z,
            direction: [1.0, 2.0, 3.0],
            amplitude: 0.25,
            speed: 1.5,
            frequency: 2.5,
            phase: 0.75,
            mask: Some(VertexModifierMask3D {
                axis: VertexAxis3D::Y,
                start: -1.0,
                end: 4.0,
            }),
        };
        let mut packed = Vec::new();
        encode_vertex_modifier(&modifier, &mut packed);

        assert_eq!(packed.len(), 16);
        assert_eq!(&packed[0..3], &[1.0, 2.0, 0.0]);
        assert_eq!(&packed[4..11], &[1.0, 2.0, 3.0, 0.25, 1.5, 2.5, 0.75]);
        assert_eq!(&packed[12..16], &[1.0, 1.0, -1.0, 4.0]);
    }
}
