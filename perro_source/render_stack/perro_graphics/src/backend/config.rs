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
            smoothing_2d_samples: 1,
            fxaa_enabled: false,
            smaa_enabled: false,
            taa_enabled: false,
            static_texture_lookup: None,
            static_font_lookup: None,
            static_mesh_lookup: None,
            static_shader_lookup: None,
            pending_pipeline_warms: Vec::new(),
            last_pipeline_compiles_3d: 0,
            custom_shader_animated_cache: AHashMap::new(),
            retained_animated_material_memo: None,
            animated_stream_nodes_scratch: ahash::AHashSet::new(),
            animated_stream_memo: AHashMap::new(),
            nine_slice_sizes_memo: None,
            meshlets_enabled: false,
            dev_meshlets: false,
            meshlet_debug_view: false,
            occlusion_culling: OcclusionCullingMode::Gpu,
            ssao: SsaoQuality::Medium,
            shadow_quality: ShadowQuality::Medium,
            texture_filter: TextureFilterMode::LinearMipmap,
            hdr_mode: HdrMode::Auto,
            shader_variant_mode: ShaderVariantMode::Auto,
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
            camera_stream_states_changed: ahash::AHashSet::new(),
            frame_rects_cache: Vec::new(),
            late_overlay_sprites_cache: Vec::new(),
            late_overlay_sprites_cache_revision: u64::MAX,
            late_overlay_point_lights_cache: Vec::new(),
            late_overlay_point_lights_cache_revision: u64::MAX,
            late_overlay_shadow_casters_cache: Vec::new(),
            late_overlay_shadow_casters_cache_revision: u64::MAX,
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

    pub fn with_hdr_mode(mut self, mode: HdrMode) -> Self {
        self.hdr_mode = mode;
        self
    }

    pub fn with_shader_variant_mode(mut self, mode: ShaderVariantMode) -> Self {
        self.shader_variant_mode = mode;
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

    /// Project `graphics.anti_alias` mode: sets the 3D sample count
    /// (msaa2/msaa4 -> 2/4, off/fxaa/smaa/taa -> 1) and flags the matching
    /// present pass. FXAA/SMAA/TAA resources stay unallocated unless the
    /// mode requests them (at most one of the three is ever enabled).
    pub fn with_anti_alias(mut self, mode: AntiAliasMode) -> Self {
        self.fxaa_enabled = matches!(mode, AntiAliasMode::Fxaa);
        self.smaa_enabled = matches!(mode, AntiAliasMode::Smaa);
        self.taa_enabled = matches!(mode, AntiAliasMode::Taa);
        self.set_smoothing_samples(mode.sample_count());
        self
    }

    pub fn with_ssao(mut self, quality: SsaoQuality) -> Self {
        self.ssao = quality;
        self
    }

    pub fn with_shadow_quality(mut self, quality: ShadowQuality) -> Self {
        self.shadow_quality = quality;
        self
    }

    /// MSAA for sessions that never init the 3D pipeline. The first 3D frame
    /// switches to the `with_msaa` sample count.
    pub fn with_msaa_2d(mut self, enabled: bool) -> Self {
        self.smoothing_2d_samples = if enabled { 4 } else { 1 };
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

    /// Project virtual canvas (graphics.virtual_width/height). Drives the 2D
    /// aspect-fit world-to-pixel rule; defaults to 1920x1080 when not set.
    pub fn with_virtual_canvas(mut self, width: u32, height: u32) -> Self {
        let width = width.max(1) as f32;
        let height = height.max(1) as f32;
        self.renderer_2d.set_virtual_viewport(width, height);
        self.late_overlay_2d.set_virtual_viewport(width, height);
        self
    }
}
