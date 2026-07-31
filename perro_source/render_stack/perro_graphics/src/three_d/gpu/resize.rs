use super::*;

impl Gpu3D {
    /// Upgrade the mesh-blend depth/mask targets from their 1x1 placeholders
    /// to the current scene resolution. Called from prepare the first frame a
    /// mesh blend is actually staged; scenes without blends keep the
    /// placeholders (~9MB per instance at 1080p).
    pub(super) fn ensure_mesh_blend_targets(&mut self, device: &wgpu::Device) {
        let (width, height) = self.depth_size;
        if self.mesh_blend_depth_texture.width() == width
            && self.mesh_blend_depth_texture.height() == height
        {
            return;
        }
        let (mesh_blend_depth_texture, mesh_blend_depth_view) =
            create_depth_prepass_texture(device, width, height);
        self.mesh_blend_depth_texture = mesh_blend_depth_texture;
        self.mesh_blend_depth_view = mesh_blend_depth_view;
        let (mesh_blend_mask_texture, mesh_blend_mask_view) =
            mesh_blend_screen::create_mesh_blend_mask_texture(device, width, height);
        self._mesh_blend_mask_texture = mesh_blend_mask_texture;
        self.mesh_blend_mask_view = mesh_blend_mask_view;
        self.mesh_blend_seam_bind_group = None;
        self.mesh_blend_scene_copy = None;
        // mesh_blend_depth_view is bound in the environment, multimesh and
        // shadow-multimesh bind groups; rebuild them all.
        self.rebuild_environment_bind_group(device);
        self.rebuild_camera_bind_groups(device);
        self.shadow_casters_dirty = true;
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.depth_size == (width, height) {
            return;
        }
        let (depth_prepass_texture, depth_prepass_view) =
            create_depth_prepass_texture(device, width, height);
        self.depth_prepass_texture = depth_prepass_texture;
        self.depth_prepass_view = depth_prepass_view;
        let (depth_texture, depth_view) = create_scene_depth_target(
            device,
            width,
            height,
            self.sample_count,
            &self.depth_prepass_texture,
            &self.depth_prepass_view,
        );
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
        self.water_scene_depth = None;
        if let Some(ssao_pass) = self.ssao_pass.as_mut() {
            ssao_pass.resize(
                device,
                width,
                height,
                &self.depth_prepass_view,
                self.ssao_quality,
            );
        }
        // Lazy targets: still-placeholder (1x1) targets stay placeholders;
        // already-upgraded ones follow the new resolution.
        let (blend_width, blend_height) = if self.mesh_blend_depth_texture.width() > 1 {
            (width, height)
        } else {
            (1, 1)
        };
        let (mesh_blend_depth_texture, mesh_blend_depth_view) =
            create_depth_prepass_texture(device, blend_width, blend_height);
        self.mesh_blend_depth_texture = mesh_blend_depth_texture;
        self.mesh_blend_depth_view = mesh_blend_depth_view;
        self.rebuild_environment_bind_group(device);
        let (mesh_blend_mask_texture, mesh_blend_mask_view) =
            mesh_blend_screen::create_mesh_blend_mask_texture(device, blend_width, blend_height);
        self._mesh_blend_mask_texture = mesh_blend_mask_texture;
        self.mesh_blend_mask_view = mesh_blend_mask_view;
        self.mesh_blend_seam_bind_group = None;
        self.mesh_blend_scene_copy = None;
        self.depth_size = (width, height);
        // Bind group pointers (mesh_blend_depth_view) changed; force a shadow
        // re-render so the cache does not keep stale layers.
        self.shadow_casters_dirty = true;
        let (hiz_width, hiz_height) = if occlusion_flags(self.occlusion_mode).0 {
            (width, height)
        } else {
            (1, 1)
        };
        let (hiz_texture, hiz_mip_views, hiz_sample_view, hiz_mip_count, hiz_size) =
            create_hiz_texture(device, hiz_width, hiz_height);
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
        registry: Arc<PipelineRegistry>,
        width: u32,
        height: u32,
    ) {
        let color_format = registry.color_format();
        let sample_count = registry.sample_count().max(1);
        if self.sample_count == sample_count && self.color_format == color_format {
            return;
        }
        // Swap in the lazily-built registry for the new (format, samples).
        // Bind group layouts are shared across registries, so every existing
        // bind group stays compatible with the freshly built pipelines.
        self.pipelines = registry;
        self.custom_sky_pipelines.clear();
        self.active_sky_pipeline_key = None;
        self.color_format = color_format;
        let (depth_prepass_texture, depth_prepass_view) =
            create_depth_prepass_texture(device, width, height);
        self.depth_prepass_texture = depth_prepass_texture;
        self.depth_prepass_view = depth_prepass_view;
        let (depth_texture, depth_view) = create_scene_depth_target(
            device,
            width,
            height,
            sample_count,
            &self.depth_prepass_texture,
            &self.depth_prepass_view,
        );
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
        self.water_scene_depth = None;
        if let Some(ssao_pass) = self.ssao_pass.as_mut() {
            ssao_pass.resize(
                device,
                width,
                height,
                &self.depth_prepass_view,
                self.ssao_quality,
            );
        }
        // Lazy targets: keep placeholders 1x1, track resolution otherwise
        // (see resize above).
        let (blend_width, blend_height) = if self.mesh_blend_depth_texture.width() > 1 {
            (width, height)
        } else {
            (1, 1)
        };
        let (mesh_blend_depth_texture, mesh_blend_depth_view) =
            create_depth_prepass_texture(device, blend_width, blend_height);
        self.mesh_blend_depth_texture = mesh_blend_depth_texture;
        self.mesh_blend_depth_view = mesh_blend_depth_view;
        self.rebuild_environment_bind_group(device);
        let (mesh_blend_mask_texture, mesh_blend_mask_view) =
            mesh_blend_screen::create_mesh_blend_mask_texture(device, blend_width, blend_height);
        self._mesh_blend_mask_texture = mesh_blend_mask_texture;
        self.mesh_blend_mask_view = mesh_blend_mask_view;
        self.mesh_blend_seam_bind_group = None;
        self.mesh_blend_scene_copy = None;
        self.rebuild_camera_bind_groups(device);
        // Shadow depth pipelines + bind group pointers were recreated; force a
        // full shadow re-render.
        self.shadow_casters_dirty = true;
        self.depth_size = (width.max(1), height.max(1));
        let (hiz_width, hiz_height) = if occlusion_flags(self.occlusion_mode).0 {
            (width, height)
        } else {
            (1, 1)
        };
        let (hiz_texture, hiz_mip_views, hiz_sample_view, hiz_mip_count, hiz_size) =
            create_hiz_texture(device, hiz_width, hiz_height);
        self.hiz_texture = hiz_texture;
        self.hiz_mip_views = hiz_mip_views;
        self.hiz_sample_view = hiz_sample_view;
        self.hiz_mip_count = hiz_mip_count;
        self.hiz_size = hiz_size;
        self.rebuild_hiz_bind_groups(device);
        self.sample_count = sample_count;
        self.invalidate_custom_pipelines();
        self.builtin_variant_pipelines.clear();
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
