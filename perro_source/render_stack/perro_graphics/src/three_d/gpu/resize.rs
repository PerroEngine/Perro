use super::*;

impl Gpu3D {
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.depth_size == (width, height) {
            return;
        }
        let (depth_texture, depth_view) =
            create_depth_texture(device, width, height, self.sample_count);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
        let (depth_prepass_texture, depth_prepass_view) =
            create_depth_prepass_texture(device, width, height);
        self.depth_prepass_texture = depth_prepass_texture;
        self.depth_prepass_view = depth_prepass_view;
        if let Some(ssao_pass) = self.ssao_pass.as_mut() {
            ssao_pass.resize(
                device,
                width,
                height,
                &self.depth_prepass_view,
                self.ssao_quality,
            );
        }
        let (mesh_blend_depth_texture, mesh_blend_depth_view) =
            create_depth_prepass_texture(device, width, height);
        self.mesh_blend_depth_texture = mesh_blend_depth_texture;
        self.mesh_blend_depth_view = mesh_blend_depth_view;
        self.mesh_blend_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_mesh_blend_bg"),
            layout: &self.mesh_blend_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.mesh_blend_depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        self.ssao_pass
                            .as_ref()
                            .map(ssao::SsaoPass::view)
                            .unwrap_or(&self.ssao_fallback_view),
                    ),
                },
            ],
        });
        let (mesh_blend_mask_texture, mesh_blend_mask_view) =
            mesh_blend_screen::create_mesh_blend_mask_texture(device, width, height);
        self._mesh_blend_mask_texture = mesh_blend_mask_texture;
        self.mesh_blend_mask_view = mesh_blend_mask_view;
        self.mesh_blend_seam_bind_group = None;
        self.mesh_blend_scene_copy = None;
        self.depth_size = (width, height);
        // Bind group pointers (mesh_blend_depth_view) changed; force a shadow
        // re-render so the cache does not keep stale layers.
        self.shadow_casters_dirty = true;
        let (hiz_texture, hiz_mip_views, hiz_sample_view, hiz_mip_count, hiz_size) =
            create_hiz_texture(device, width, height);
        self.hiz_texture = hiz_texture;
        self.hiz_mip_views = hiz_mip_views;
        self.hiz_sample_view = hiz_sample_view;
        self.hiz_mip_count = hiz_mip_count;
        self.hiz_size = hiz_size;
        self.rebuild_camera_bind_groups(device);
        self.rebuild_hiz_bind_groups(device);
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
        // Multimesh cull bind group also references the hi-z pyramid view.
        self.rebuild_multimesh_cull_bind_group(device);
    }

    pub fn set_sample_count(
        &mut self,
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
        width: u32,
        height: u32,
    ) {
        let sample_count = sample_count.max(1);
        if self.sample_count == sample_count && self.color_format == color_format {
            return;
        }
        let shader = create_mesh_shader_module_skinned(device);
        let shader_unlit = create_unlit_shader_module_skinned(device);
        let shader_toon = create_toon_shader_module_skinned(device);
        let shader_rigid = create_mesh_shader_module_rigid(device);
        let shader_rigid_unlit = create_unlit_shader_module_rigid(device);
        let shader_rigid_toon = create_toon_shader_module_rigid(device);
        let shader_multimesh = create_multimesh_shader_module(device);
        let sky_shader = create_sky_shader_module(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_mesh_pipeline_layout"),
            bind_group_layouts: &[
                Some(&self.camera_bgl),
                Some(&self.material_texture_bgl),
                Some(&self.shadow_bgl),
                Some(&self.mesh_blend_bgl),
            ],
            immediate_size: 0,
        });
        let depth_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perro_depth_pipeline_layout"),
                bind_group_layouts: &[Some(&self.camera_bgl)],
                immediate_size: 0,
            });
        let rigid_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perro_mesh_pipeline_layout_rigid"),
                bind_group_layouts: &[
                    Some(&self.rigid_camera_bgl),
                    Some(&self.material_texture_bgl),
                    Some(&self.shadow_bgl),
                    Some(&self.mesh_blend_bgl),
                ],
                immediate_size: 0,
            });
        let rigid_depth_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perro_depth_pipeline_layout_rigid"),
                bind_group_layouts: &[Some(&self.rigid_camera_bgl)],
                immediate_size: 0,
            });
        let multimesh_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perro_multimesh_pipeline_layout"),
                bind_group_layouts: &[Some(&self.multimesh_bgl)],
                immediate_size: 0,
            });
        let multimesh_mask_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perro_mesh_blend_mask_layout_multimesh"),
                bind_group_layouts: &[
                    Some(&self.multimesh_bgl),
                    Some(&self.mesh_blend_mask_id_bgl),
                ],
                immediate_size: 0,
            });
        let sky_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_sky3d_pipeline_layout"),
            bind_group_layouts: &[Some(&self.sky_bgl)],
            immediate_size: 0,
        });
        self.pipeline_culled = create_pipeline_skinned(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_double_sided = create_pipeline_skinned(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
            None,
        );
        self.pipeline_blend_culled = create_pipeline_skinned_blend(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_blend_double_sided = create_pipeline_skinned_blend(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
            None,
        );
        self.pipeline_unlit_culled = create_pipeline_skinned(
            device,
            &pipeline_layout,
            &shader_unlit,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_unlit_double_sided = create_pipeline_skinned(
            device,
            &pipeline_layout,
            &shader_unlit,
            color_format,
            sample_count,
            None,
        );
        self.pipeline_unlit_blend_culled = create_pipeline_skinned_blend(
            device,
            &pipeline_layout,
            &shader_unlit,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_unlit_blend_double_sided = create_pipeline_skinned_blend(
            device,
            &pipeline_layout,
            &shader_unlit,
            color_format,
            sample_count,
            None,
        );
        self.pipeline_toon_culled = create_pipeline_skinned(
            device,
            &pipeline_layout,
            &shader_toon,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_toon_double_sided = create_pipeline_skinned(
            device,
            &pipeline_layout,
            &shader_toon,
            color_format,
            sample_count,
            None,
        );
        self.pipeline_toon_blend_culled = create_pipeline_skinned_blend(
            device,
            &pipeline_layout,
            &shader_toon,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_toon_blend_double_sided = create_pipeline_skinned_blend(
            device,
            &pipeline_layout,
            &shader_toon,
            color_format,
            sample_count,
            None,
        );
        self.pipeline_overlay_culled = create_pipeline_overlay_skinned(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_overlay_double_sided = create_pipeline_overlay_skinned(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
            None,
        );
        self.pipeline_rigid_culled = create_pipeline_rigid(
            device,
            &rigid_pipeline_layout,
            &shader_rigid,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_rigid_double_sided = create_pipeline_rigid(
            device,
            &rigid_pipeline_layout,
            &shader_rigid,
            color_format,
            sample_count,
            None,
        );
        self.pipeline_rigid_blend_culled = create_pipeline_rigid_blend(
            device,
            &rigid_pipeline_layout,
            &shader_rigid,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_rigid_blend_double_sided = create_pipeline_rigid_blend(
            device,
            &rigid_pipeline_layout,
            &shader_rigid,
            color_format,
            sample_count,
            None,
        );
        self.pipeline_rigid_unlit_culled = create_pipeline_rigid(
            device,
            &rigid_pipeline_layout,
            &shader_rigid_unlit,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_rigid_unlit_double_sided = create_pipeline_rigid(
            device,
            &rigid_pipeline_layout,
            &shader_rigid_unlit,
            color_format,
            sample_count,
            None,
        );
        self.pipeline_rigid_unlit_blend_culled = create_pipeline_rigid_blend(
            device,
            &rigid_pipeline_layout,
            &shader_rigid_unlit,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_rigid_unlit_blend_double_sided = create_pipeline_rigid_blend(
            device,
            &rigid_pipeline_layout,
            &shader_rigid_unlit,
            color_format,
            sample_count,
            None,
        );
        self.pipeline_rigid_toon_culled = create_pipeline_rigid(
            device,
            &rigid_pipeline_layout,
            &shader_rigid_toon,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_rigid_toon_double_sided = create_pipeline_rigid(
            device,
            &rigid_pipeline_layout,
            &shader_rigid_toon,
            color_format,
            sample_count,
            None,
        );
        self.pipeline_rigid_toon_blend_culled = create_pipeline_rigid_blend(
            device,
            &rigid_pipeline_layout,
            &shader_rigid_toon,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_rigid_toon_blend_double_sided = create_pipeline_rigid_blend(
            device,
            &rigid_pipeline_layout,
            &shader_rigid_toon,
            color_format,
            sample_count,
            None,
        );
        self.pipeline_rigid_overlay_culled = create_pipeline_overlay_rigid(
            device,
            &rigid_pipeline_layout,
            &shader_rigid,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_rigid_overlay_double_sided = create_pipeline_overlay_rigid(
            device,
            &rigid_pipeline_layout,
            &shader_rigid,
            color_format,
            sample_count,
            None,
        );
        self.pipeline_multimesh_culled = create_multimesh_pipeline(
            device,
            &multimesh_pipeline_layout,
            &shader_multimesh,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_multimesh_double_sided = create_multimesh_pipeline(
            device,
            &multimesh_pipeline_layout,
            &shader_multimesh,
            color_format,
            sample_count,
            None,
        );
        self.pipeline_multimesh_blend_culled = create_multimesh_blend_pipeline(
            device,
            &multimesh_pipeline_layout,
            &shader_multimesh,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_multimesh_blend_double_sided = create_multimesh_blend_pipeline(
            device,
            &multimesh_pipeline_layout,
            &shader_multimesh,
            color_format,
            sample_count,
            None,
        );
        self.pipeline_multimesh_mask_culled = create_multimesh_mask_pipeline(
            device,
            &multimesh_mask_pipeline_layout,
            &shader_multimesh,
            Some(wgpu::Face::Back),
        );
        self.pipeline_multimesh_mask_double_sided = create_multimesh_mask_pipeline(
            device,
            &multimesh_mask_pipeline_layout,
            &shader_multimesh,
            None,
        );
        let depth_prepass_shader = create_depth_prepass_shader_module_skinned(device);
        let depth_prepass_shader_rigid = create_depth_prepass_shader_module_rigid(device);
        self.pipeline_depth_prepass_culled = create_depth_prepass_pipeline_skinned(
            device,
            &depth_pipeline_layout,
            &depth_prepass_shader,
            Some(wgpu::Face::Back),
        );
        self.pipeline_depth_prepass_double_sided = create_depth_prepass_pipeline_skinned(
            device,
            &depth_pipeline_layout,
            &depth_prepass_shader,
            None,
        );
        self.pipeline_depth_prepass_rigid_culled = create_depth_prepass_pipeline_rigid(
            device,
            &rigid_depth_pipeline_layout,
            &depth_prepass_shader_rigid,
            Some(wgpu::Face::Back),
        );
        self.pipeline_depth_prepass_rigid_double_sided = create_depth_prepass_pipeline_rigid(
            device,
            &rigid_depth_pipeline_layout,
            &depth_prepass_shader_rigid,
            None,
        );
        self.pipeline_shadow_depth_culled = create_shadow_depth_pipeline_skinned(
            device,
            &depth_pipeline_layout,
            &depth_prepass_shader,
            Some(wgpu::Face::Back),
        );
        self.pipeline_shadow_depth_double_sided = create_shadow_depth_pipeline_skinned(
            device,
            &depth_pipeline_layout,
            &depth_prepass_shader,
            None,
        );
        self.pipeline_shadow_depth_rigid_culled = create_shadow_depth_pipeline_rigid(
            device,
            &rigid_depth_pipeline_layout,
            &depth_prepass_shader_rigid,
            Some(wgpu::Face::Back),
        );
        self.pipeline_shadow_depth_rigid_double_sided = create_shadow_depth_pipeline_rigid(
            device,
            &rigid_depth_pipeline_layout,
            &depth_prepass_shader_rigid,
            None,
        );
        self.sky_pipeline = create_sky_pipeline(
            device,
            &sky_pipeline_layout,
            &sky_shader,
            color_format,
            sample_count,
        );
        self.sky_pipeline_layout = sky_pipeline_layout;
        self.custom_sky_pipelines.clear();
        self.active_sky_pipeline_key = None;
        self.material_pipeline_layout = pipeline_layout;
        self.rigid_material_pipeline_layout = rigid_pipeline_layout;
        self.color_format = color_format;
        let (depth_texture, depth_view) = create_depth_texture(device, width, height, sample_count);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
        let (depth_prepass_texture, depth_prepass_view) =
            create_depth_prepass_texture(device, width, height);
        self.depth_prepass_texture = depth_prepass_texture;
        self.depth_prepass_view = depth_prepass_view;
        if let Some(ssao_pass) = self.ssao_pass.as_mut() {
            ssao_pass.resize(
                device,
                width,
                height,
                &self.depth_prepass_view,
                self.ssao_quality,
            );
        }
        let (mesh_blend_depth_texture, mesh_blend_depth_view) =
            create_depth_prepass_texture(device, width, height);
        self.mesh_blend_depth_texture = mesh_blend_depth_texture;
        self.mesh_blend_depth_view = mesh_blend_depth_view;
        self.mesh_blend_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_mesh_blend_bg"),
            layout: &self.mesh_blend_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.mesh_blend_depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        self.ssao_pass
                            .as_ref()
                            .map(ssao::SsaoPass::view)
                            .unwrap_or(&self.ssao_fallback_view),
                    ),
                },
            ],
        });
        let (mesh_blend_mask_texture, mesh_blend_mask_view) =
            mesh_blend_screen::create_mesh_blend_mask_texture(device, width, height);
        self._mesh_blend_mask_texture = mesh_blend_mask_texture;
        self.mesh_blend_mask_view = mesh_blend_mask_view;
        self.mesh_blend_seam_pipeline = mesh_blend_screen::create_mesh_blend_seam_pipeline(
            device,
            &self.mesh_blend_seam_bgl,
            color_format,
        );
        self.mesh_blend_seam_bind_group = None;
        self.mesh_blend_scene_copy = None;
        self.rebuild_camera_bind_groups(device);
        // Shadow depth pipelines + bind group pointers were recreated; force a
        // full shadow re-render.
        self.shadow_casters_dirty = true;
        self.depth_size = (width.max(1), height.max(1));
        let (hiz_texture, hiz_mip_views, hiz_sample_view, hiz_mip_count, hiz_size) =
            create_hiz_texture(device, width, height);
        self.hiz_texture = hiz_texture;
        self.hiz_mip_views = hiz_mip_views;
        self.hiz_sample_view = hiz_sample_view;
        self.hiz_mip_count = hiz_mip_count;
        self.hiz_size = hiz_size;
        self.rebuild_hiz_bind_groups(device);
        self.sample_count = sample_count;
        self.custom_pipelines.clear();
        self.custom_pipelines_rigid.clear();
        // Keyed by the same tokens: a stale entry under a re-minted token would
        // make ensure_custom_pipeline skip the rebuild and bind an old-sample-count
        // pipeline.
        self.custom_pipelines_multimesh.clear();
        self.custom_pipeline_tokens.clear();
        // Tokens restart from 1: drop the per-token vertex-hook flags so a
        // re-minted token can't inherit a stale hook flag (missing entries
        // classify as not depth-safe until the pipeline is ensured again).
        self.custom_pipeline_vertex_hooks.clear();
        self.next_custom_pipeline_token = 1;
        let (gpu_occlusion_enabled, cpu_occlusion_enabled) = occlusion_flags(self.occlusion_mode);
        self.gpu_occlusion_enabled = gpu_occlusion_enabled;
        self.cpu_occlusion_enabled = cpu_occlusion_enabled;
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
        // Multimesh cull bind group also references the hi-z pyramid view.
        self.rebuild_multimesh_cull_bind_group(device);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CUSTOM_SHADER: &str = "fn shade_material(in: FragmentInput) -> vec4<f32> {\n    return vec4<f32>(1.0, 1.0, 1.0, 1.0);\n}\n";

    fn test_shader_lookup(_path_hash: u64) -> &'static str {
        TEST_CUSTOM_SHADER
    }

    async fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .ok()?;
        adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("perro_resize_test_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::default(),
            })
            .await
            .ok()
    }

    #[test]
    fn sample_count_change_clears_all_custom_pipeline_maps() {
        pollster::block_on(async {
            let Some((device, queue)) = test_device().await else {
                eprintln!("skip resize custom pipeline test: no wgpu adapter");
                return;
            };
            let format = wgpu::TextureFormat::Rgba8Unorm;
            let mut gpu = Gpu3D::new(
                &device,
                &queue,
                format,
                Gpu3DConfig {
                    sample_count: 1,
                    width: 64,
                    height: 64,
                    meshlets_enabled: false,
                    dev_meshlets: false,
                    meshlet_debug_view: false,
                    occlusion_culling: OcclusionCullingMode::Off,
                    ssao: crate::SsaoQuality::Off,
                    indirect_first_instance_enabled: false,
                    multi_draw_indirect_enabled: false,
                    texture_filter: TextureFilterMode::Linear,
                },
            );

            let token = gpu
                .ensure_custom_pipeline(
                    &device,
                    RenderPath3D::MultiMesh,
                    "res://resize_test_custom.wgsl",
                    CustomMaterialLighting3D::Standard,
                    Some(test_shader_lookup),
                )
                .expect("multimesh custom pipeline must build");
            assert!(gpu.custom_pipelines_multimesh.contains_key(&token));
            assert!(gpu.custom_pipeline_vertex_hooks.contains_key(&token));

            gpu.set_sample_count(&device, format, 4, 64, 64);

            // Tokens re-mint from 1 after a sample-count change; every
            // token-keyed map must drop or a re-minted token can bind a
            // pipeline built for the old sample count.
            assert!(gpu.custom_pipelines_multimesh.is_empty());
            assert!(gpu.custom_pipelines.is_empty());
            assert!(gpu.custom_pipelines_rigid.is_empty());
            assert!(gpu.custom_pipeline_tokens.is_empty());
            assert!(gpu.custom_pipeline_vertex_hooks.is_empty());
            assert_eq!(gpu.next_custom_pipeline_token, 1);
        });
    }
}
