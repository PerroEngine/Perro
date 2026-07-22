use super::*;

impl PerroGraphics {
    pub fn new() -> Self {
        #[cfg(all(not(target_arch = "wasm32"), not(test)))]
        let (async_mesh_load_tx, async_mesh_load_rx) = mpsc::channel();
        #[cfg(not(target_arch = "wasm32"))]
        let (async_texture_load_tx, async_texture_load_rx) = mpsc::channel();
        Self {
            frame: FrameState::default(),
            resources: ResourceStore::new(),
            renderer_2d: Renderer2D::new(),
            late_overlay_2d: Renderer2D::new(),
            renderer_3d: Renderer3D::new(),
            particles_3d: Particles3DRenderer::new(),
            renderer_ui: UiRenderer::new(),
            gpu: None,
            events: Vec::new(),
            #[cfg(all(not(target_arch = "wasm32"), not(test)))]
            async_mesh_load_tx,
            #[cfg(all(not(target_arch = "wasm32"), not(test)))]
            async_mesh_load_rx,
            #[cfg(all(not(target_arch = "wasm32"), not(test)))]
            pending_async_mesh_loads: AHashMap::new(),
            #[cfg(all(not(target_arch = "wasm32"), not(test)))]
            queued_async_mesh_loads: Vec::new(),
            #[cfg(not(target_arch = "wasm32"))]
            async_texture_load_tx,
            #[cfg(not(target_arch = "wasm32"))]
            async_texture_load_rx,
            #[cfg(not(target_arch = "wasm32"))]
            pending_async_texture_loads: AHashMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            queued_async_texture_loads: Vec::new(),
            viewport: (0, 0),
            vsync_enabled: true,
            smoothing_enabled: true,
            smoothing_samples: 4,
            smoothing_quality_samples: 4,
            static_texture_lookup: None,
            static_font_lookup: None,
            static_mesh_lookup: None,
            static_shader_lookup: None,
            meshlets_enabled: false,
            dev_meshlets: false,
            meshlet_debug_view: false,
            occlusion_culling: OcclusionCullingMode::Gpu,
            ssao: SsaoQuality::Medium,
            texture_filter: TextureFilterMode::LinearMipmap,
            hdr_mode: HdrMode::Auto,
            retained_draws_cache_revision: u64::MAX,
            retained_draw_instances_cache: 0,
            retained_point_particles_cache: Vec::new(),
            retained_point_particles_cache_revision: u64::MAX,
            retained_waters_2d_cache: Vec::new(),
            retained_waters_2d_cache_revision: u64::MAX,
            retained_waters_3d_cache: Vec::new(),
            retained_waters_3d_cache_revision: u64::MAX,
            retained_decals_3d_cache: Vec::new(),
            retained_decals_3d_cache_revision: u64::MAX,
            retained_sprites_cache: Vec::new(),
            retained_sprites_cache_revision: u64::MAX,
            retained_point_lights_cache: Vec::new(),
            retained_point_lights_cache_revision: u64::MAX,
            retained_shadow_casters_cache: Vec::new(),
            retained_shadow_casters_cache_revision: u64::MAX,
            camera_stream_targets: AHashMap::new(),
            stream_texture_dims: AHashMap::new(),
            retained_camera_streams: Vec::new(),
            frame_rects_cache: Vec::new(),
            late_overlay_sprites_cache: Vec::new(),
            late_overlay_sprites_cache_revision: u64::MAX,
            late_overlay_point_lights_cache: Vec::new(),
            late_overlay_point_lights_cache_revision: u64::MAX,
            late_overlay_shadow_casters_cache: Vec::new(),
            late_overlay_rects_cache: Vec::new(),
            used_texture_refs_cache: AHashMap::new(),
            used_mesh_refs_cache: AHashMap::new(),
            used_material_refs_cache: AHashMap::new(),
            scene_texture_refs_cache: AHashMap::new(),
            scene_mesh_refs_cache: AHashMap::new(),
            scene_material_refs_cache: AHashMap::new(),
            used_ref_draws_revision: u64::MAX,
            used_ref_sprites_revision: u64::MAX,
            global_post_processing: PostProcessSet::new(),
            global_post_processing_cache: Arc::from(Vec::new()),
            global_post_processing_cache_dirty: true,
            accessibility: VisualAccessibilitySettings::default(),
            frame_index: 0,
            redraw_requested: true,
            frame_time_seconds: 0.0,
            frame_delta_seconds: 0.0,
            last_frame_instant: None,
            #[cfg(target_arch = "wasm32")]
            pending_gpu: None,
        }
    }

    pub fn with_vsync(mut self, enabled: bool) -> Self {
        self.vsync_enabled = enabled;
        self
    }

    pub fn with_msaa(mut self, enabled: bool) -> Self {
        self.set_smoothing(enabled);
        self
    }

    pub fn with_msaa_samples(mut self, samples: u32) -> Self {
        self.set_smoothing_samples(samples);
        self
    }

    pub fn with_ssao(mut self, quality: SsaoQuality) -> Self {
        self.ssao = quality;
        self
    }

    pub fn with_static_texture_lookup(mut self, lookup: StaticTextureLookup) -> Self {
        self.static_texture_lookup = Some(lookup);
        self
    }

    pub fn with_static_font_lookup(mut self, lookup: StaticFontLookup) -> Self {
        self.static_font_lookup = Some(lookup);
        self.renderer_ui.set_static_font_lookup(lookup);
        self
    }

    pub fn with_ui_default_font(mut self, font: &str) -> Self {
        self.renderer_ui
            .set_default_font(perro_ui::UiFont::parse(font).unwrap_or_default());
        self
    }

    pub fn with_static_mesh_lookup(mut self, lookup: StaticMeshLookup) -> Self {
        self.static_mesh_lookup = Some(lookup);
        self
    }

    pub fn with_static_shader_lookup(mut self, lookup: StaticShaderLookup) -> Self {
        self.static_shader_lookup = Some(lookup);
        self
    }

    pub fn with_dev_meshlets(mut self, enabled: bool) -> Self {
        self.dev_meshlets = enabled;
        self
    }

    pub fn with_meshlets_enabled(mut self, enabled: bool) -> Self {
        self.meshlets_enabled = enabled;
        self
    }

    pub fn with_meshlet_debug_view(mut self, enabled: bool) -> Self {
        self.meshlet_debug_view = enabled;
        self
    }

    pub fn with_occlusion_culling(mut self, mode: OcclusionCullingMode) -> Self {
        self.occlusion_culling = mode;
        self
    }

    pub fn with_texture_filter(mut self, mode: TextureFilterMode) -> Self {
        self.texture_filter = mode;
        self
    }
}
