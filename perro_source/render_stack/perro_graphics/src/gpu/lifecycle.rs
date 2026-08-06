use super::*;

impl Gpu {
    fn note_surface_acquire_success(&mut self) {
        if let Some(status) = self.surface_acquire_failure.take() {
            eprintln!("[perro][gfx] surface acquire recovered after ({status})");
            perro_structs::structs::boot_log::mark("surface acquire recovered");
        }
    }

    fn note_surface_acquire_failure(&mut self, status: &'static str) {
        if self.surface_acquire_failure == Some(status) {
            return;
        }
        self.surface_acquire_failure = Some(status);
        eprintln!("[perro][gfx] surface acquire fail=({status})");
        perro_structs::structs::boot_log::mark(&format!("surface acquire fail: {status}"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn wait_idle(&mut self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }

    #[cfg(target_arch = "wasm32")]
    pub fn wait_idle(&mut self) {}

    /// Live window size when it differs from the configured swapchain extent.
    /// `self.config` only tracks the resize events the runtime forwarded, so
    /// after a drifted acquire the window is the only reliable source.
    fn drifted_window_size(&self) -> Option<(u32, u32)> {
        let size = self.window_handle.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);
        (width != self.config.width || height != self.config.height).then_some((width, height))
    }

    /// Acquire the next swapchain image. Suboptimal still returns the image
    /// (dropping it would stall on drivers that raise it every frame) and only
    /// records the resync. Outdated/Lost cannot present at all, so those
    /// reconfigure at the live size and retry the acquire once.
    pub(crate) fn acquire_surface_texture(&mut self) -> Option<wgpu::SurfaceTexture> {
        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => {
                self.note_surface_acquire_success();
                Some(frame)
            }
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                self.note_surface_acquire_success();
                self.surface_resync_request = self.drifted_window_size();
                Some(frame)
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                let drift = self.drifted_window_size();
                if let Some((width, height)) = drift {
                    // Config only: the render targets stay put so this frame
                    // presents its already-encoded scene (scaled) instead of an
                    // empty one. The runner's resize rebuilds them next frame.
                    self.config.width = width;
                    self.config.height = height;
                }
                self.surface.configure(&self.device, &self.config);
                self.surface_resync_request = drift;
                match self.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(frame)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                        self.note_surface_acquire_success();
                        Some(frame)
                    }
                    wgpu::CurrentSurfaceTexture::Timeout => {
                        self.note_surface_acquire_failure("timeout after reconfigure");
                        None
                    }
                    wgpu::CurrentSurfaceTexture::Occluded => {
                        self.note_surface_acquire_failure("occluded after reconfigure");
                        None
                    }
                    wgpu::CurrentSurfaceTexture::Validation => {
                        self.note_surface_acquire_failure("validation after reconfigure");
                        None
                    }
                    wgpu::CurrentSurfaceTexture::Outdated => {
                        self.note_surface_acquire_failure("outdated after reconfigure");
                        None
                    }
                    wgpu::CurrentSurfaceTexture::Lost => {
                        self.note_surface_acquire_failure("lost after reconfigure");
                        None
                    }
                }
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                self.note_surface_acquire_failure("timeout");
                None
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                self.note_surface_acquire_failure("occluded");
                None
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                self.note_surface_acquire_failure("validation");
                None
            }
        }
    }

    /// Drained after present: the size the swapchain must be re-synced to.
    pub fn take_surface_resync_request(&mut self) -> Option<(u32, u32)> {
        self.surface_resync_request.take()
    }

    pub fn render_idle_clear(&mut self) -> bool {
        // Keep window alive for the full surface lifetime.
        self.window_handle.id();

        let Some(frame) = self.acquire_surface_texture() else {
            return false;
        };

        let swap_view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.surface_view_format),
            ..Default::default()
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("perro_idle_clear_encoder"),
            });
        {
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_idle_clear_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &swap_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: CLEAR_R,
                            g: CLEAR_G,
                            b: CLEAR_B,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        true
    }

    pub fn hdr_status(&self) -> HdrStatus {
        self.hdr_status
    }

    pub fn set_hdr_mode(&mut self, mode: HdrMode) -> HdrStatus {
        let caps = self.surface.get_capabilities(&self.adapter);
        let display = self.surface.display_hdr_info(&self.adapter);
        let selection = choose_surface_selection(
            &caps,
            &display,
            mode,
            self.render_format == wgpu::TextureFormat::Rgba16Float,
        );
        let output_changed = self.config.format != selection.format
            || self.config.color_space != selection.color_space
            || self.surface_view_format != selection.view_format;
        self.hdr_status = selection.status;
        if !output_changed {
            return self.hdr_status;
        }
        self.invalidate_retained_scene();

        self.config.format = selection.format;
        self.config.color_space = selection.color_space;
        self.config.view_formats = (selection.view_format != selection.format)
            .then_some(selection.view_format)
            .into_iter()
            .collect();
        self.surface_view_format = selection.view_format;
        self.surface.configure(&self.device, &self.config);
        self.present = PresentProcessor::new(&self.device, self.surface_view_format);
        self.present
            .set_output_size(self.config.width, self.config.height);
        // The rebuilt processor starts with every AA stage off; re-apply the
        // config + sample-count gates so FXAA/SMAA/TAA survive output changes.
        self.present
            .set_fxaa_active(self.fxaa_requested && self.sample_count == 1);
        self.present
            .set_smaa_active(self.smaa_requested && self.sample_count == 1);
        self.present
            .set_taa_active(self.taa_requested && self.sample_count == 1);
        self.present_scene_bind_group = self
            .present
            .create_bind_group(&self.device, self.post.scene_view());
        self.present_intermediate_bind_group = self.accessibility.as_ref().map(|accessibility| {
            self.present
                .create_bind_group(&self.device, accessibility.intermediate_view())
        });
        self.ui = None;
        self.late_overlay_2d = None;
        self.hdr_status
    }

    pub async fn new_async(window: Arc<Window>, cfg: GpuConfig) -> Result<Self, String> {
        perro_structs::structs::boot_log::mark("gpu: enter");
        // Keep DX12/Vulkan selectable in shipped builds. Instance::default()
        // enables both compiled backends but does not apply WGPU_BACKEND.
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle().with_env());
        perro_structs::structs::boot_log::mark("gpu: wgpu instance");
        let surface = instance
            .create_surface(window.clone())
            .map_err(|err| format!("surface create fail: {err}"))?;

        // `PERRO_SIM` dev perf simulation. Reuses the constrained-adapter
        // policy below instead of adding a second low-end path, so a simulated
        // run exercises the exact code a real iGPU ships through.
        let sim = perro_structs::structs::devsim::profile();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                apply_limit_buckets: false,
                power_preference: match cfg.power_preference {
                    _ if sim.prefer_low_power => wgpu::PowerPreference::LowPower,
                    crate::PowerPreference::HighPerformance => {
                        wgpu::PowerPreference::HighPerformance
                    }
                    crate::PowerPreference::LowPower => wgpu::PowerPreference::LowPower,
                },
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|err| format!("GPU adapter request fail: {err}"))?;
        perro_structs::structs::boot_log::mark("gpu: adapter");
        let adapter_info = adapter.get_info();
        // A simulated low-end run takes the memory-hint path too: the hint is
        // part of what makes an iGPU behave like an iGPU.
        let low_memory_adapter = sim.gpu_constrained
            || matches!(
                adapter_info.device_type,
                wgpu::DeviceType::IntegratedGpu
                    | wgpu::DeviceType::VirtualGpu
                    | wgpu::DeviceType::Cpu
            );
        // An explicit sim request wins over the full-quality escape hatch:
        // asking for the low-end tier is the whole point of the run.
        let constrained_adapter = low_memory_adapter
            && (sim.gpu_constrained || std::env::var_os("PERRO_FULL_GPU_QUALITY").is_none());
        let max_render_pixels = if constrained_adapter {
            1920 * 1080
        } else {
            MAX_FRAME_RENDER_PIXELS
        }
        .min(sim.max_render_pixels.unwrap_or(u64::MAX));
        let effective_ssao = if constrained_adapter {
            match cfg.ssao {
                crate::SsaoQuality::Off => crate::SsaoQuality::Off,
                _ => crate::SsaoQuality::Low,
            }
        } else {
            cfg.ssao
        };
        eprintln!(
            "[perro][gfx] adapter=({}) type=({:?}) backend=({:?})",
            adapter_info.name, adapter_info.device_type, adapter_info.backend
        );
        // Shadow atlas resolutions follow shadow_quality (Low halves them);
        // the constrained-adapter probe still wins via min. Every Gpu3D built
        // on this adapter (main view + camera streams) inherits the result.
        let (mut shadow_ray, mut shadow_spot, mut shadow_point) =
            crate::three_d::gpu::shadow_map_sizes_for_quality(cfg.shadow_quality);
        if constrained_adapter {
            shadow_ray = shadow_ray.min(1024);
            shadow_spot = shadow_spot.min(1024);
            shadow_point = shadow_point.min(512);
            eprintln!(
                "[perro][gfx] low-memory policy=on max_scene=1080p msaa=off ssao=low shadow_atlas={shadow_ray}/{shadow_spot}/{shadow_point}"
            );
        }
        crate::three_d::gpu::set_default_shadow_map_sizes(shadow_ray, shadow_spot, shadow_point);
        let adapter_features = adapter.features();
        let mut required_features = wgpu::Features::empty();
        if adapter_features.contains(wgpu::Features::INDIRECT_FIRST_INSTANCE) {
            required_features |= wgpu::Features::INDIRECT_FIRST_INSTANCE;
        }
        #[cfg(not(target_arch = "wasm32"))]
        let enable_timestamp_queries = gpu_timestamp_queries_requested();
        #[cfg(target_arch = "wasm32")]
        let enable_timestamp_queries = false;
        let timestamp_features =
            wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        if enable_timestamp_queries && adapter_features.contains(timestamp_features) {
            required_features |= timestamp_features;
        }
        // Optional: lets the 3D pass issue multi_draw_indexed_indirect_count over
        // a GPU-compacted command buffer so culled-to-zero draws never reach the
        // GPU frontend. Absent (metal / wasm / older drivers) the full existing
        // multi_draw path stays in charge, unchanged.
        if adapter_features.contains(wgpu::Features::MULTI_DRAW_INDIRECT_COUNT) {
            required_features |= wgpu::Features::MULTI_DRAW_INDIRECT_COUNT;
        }
        // Optional: one shadow depth pass per LAYER SET instead of per layer
        // (`@builtin(view_index)` picks the light matrix). A shadow layer costs
        // ~7-10us flat in atlas resolution -- the cost is the pass -- so folding
        // a point light's 6 faces or the 4 cascades into one pass is the lever.
        // Absent (metal / wasm / older drivers) the per-layer path stays in
        // charge, unchanged. `max_multiview_view_count` defaults to 0, so the
        // limit has to be raised with it or every multiview pass fails.
        let shadow_multiview_views = crate::three_d::shaders::MAX_MULTIVIEW_SHADOW_VIEWS as u32;
        let multiview_views = adapter
            .limits()
            .max_multiview_view_count
            .min(shadow_multiview_views);
        let mut required_limits = wgpu::Limits::default();
        if cfg!(not(target_arch = "wasm32"))
            && adapter_features.contains(wgpu::Features::MULTIVIEW)
            && multiview_views >= shadow_multiview_views
        {
            required_features |= wgpu::Features::MULTIVIEW;
            required_limits.max_multiview_view_count = multiview_views;
        }
        // Optional: let the driver reuse machine code it already generated on a
        // prior launch (blob on disk). wgpu implement it on Vulkan only, so on
        // DX12/Metal/GL/wasm this stay off + every pipeline build is unchanged.

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("perro_device"),
                required_features,
                required_limits,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: if low_memory_adapter {
                    wgpu::MemoryHints::MemoryUsage
                } else {
                    wgpu::MemoryHints::Performance
                },
                trace: wgpu::Trace::default(),
            })
            .await
            .map_err(|err| format!("GPU device request fail: {err}"))?;
        // B4 any pipeline build: post/present/2d/ui all compile below, and a
        // cache installed later would miss them.
        let indirect_first_instance_enabled =
            required_features.contains(wgpu::Features::INDIRECT_FIRST_INSTANCE);
        // multi_draw_indexed_indirect (non-count) needs only INDIRECT_EXECUTION,
        // the same downlevel capability draw_indexed_indirect already relies on,
        // so it rides the existing indirect path with no extra feature request.
        let multi_draw_indirect_enabled = indirect_first_instance_enabled;
        // Count-driven multi-draw rides on top of the multi-draw path: without
        // the indirect path there is nothing to compact.
        let multi_draw_indirect_count_enabled = multi_draw_indirect_enabled
            && required_features.contains(wgpu::Features::MULTI_DRAW_INDIRECT_COUNT);
        let timestamp_query_enabled = required_features.contains(timestamp_features);
        if !indirect_first_instance_enabled {
            eprintln!(
                "[perro][3d] INDIRECT_FIRST_INSTANCE not supported by adapter; falling back to CPU frustum path"
            );
        }
        let caps = surface.get_capabilities(&adapter);
        let display_hdr = surface.display_hdr_info(&adapter);
        let scene_hdr = scene_hdr_supported(&adapter);
        let selection = choose_surface_selection(&caps, &display_hdr, cfg.hdr_mode, scene_hdr);
        let surface_format = selection.format;
        let surface_view_format = selection.view_format;
        let render_format = supported_linear_render_format(&adapter, surface_view_format);
        let present_mode = choose_present_mode(&caps.present_modes, cfg.vsync_enabled);
        let max_frame_latency = choose_max_frame_latency(cfg.vsync_enabled);
        let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            caps.alpha_modes[0]
        };
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);
        // Author render scale first, hardware caps second: the caps only ever
        // shrink further, so a scaled scene never grows back past a budget.
        let render_scale = if cfg.render_scale.is_finite() {
            cfg.render_scale.clamp(MIN_RENDER_SCALE, MAX_RENDER_SCALE)
        } else {
            MAX_RENDER_SCALE
        };
        let (scaled_width, scaled_height) = scaled_render_size(width, height, render_scale);
        let (render_width, render_height) = capped_render_size_with_pixel_limit(
            scaled_width,
            scaled_height,
            device.limits().max_texture_dimension_2d,
            max_render_pixels,
        );
        if (render_width, render_height) != (width, height) {
            eprintln!(
                "[perro][gfx] scene size=({render_width}x{render_height}) surface=({width}x{height})"
            );
        }

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode,
            alpha_mode,
            view_formats: (surface_view_format != surface_format)
                .then_some(surface_view_format)
                .into_iter()
                .collect(),
            desired_maximum_frame_latency: max_frame_latency,
            color_space: selection.color_space,
        };
        eprintln!(
            "[perro][gfx] vsync=({}) present_mode=({present_mode:?}) max_frame_latency=({max_frame_latency}) present_caps=({:?})",
            cfg.vsync_enabled, caps.present_modes
        );
        perro_structs::structs::boot_log::mark("gpu: device + surface config");
        surface.configure(&device, &config);
        perro_structs::structs::boot_log::mark("gpu: surface.configure");

        let mut max_supported_sample_count =
            max_supported_msaa_sample_count(&adapter, render_format);
        // Integrated/virtual adapters are bandwidth-bound, and MSAA is the
        // worst thing to spend that bandwidth on: it multiplies color + depth
        // traffic in every pass, pays a resolve, AND locks out the cheap post
        // AA (FXAA/SMAA/TAA only run at sample_count 1). Drop it to 1 there and
        // substitute FXAA, which costs a fraction of a ms at 1080p. Only when
        // no other post AA was requested, so the frame never runs two.
        let msaa_traded_for_fxaa = constrained_adapter
            && normalize_sample_count(cfg.smoothing_samples) > 1
            && !cfg.fxaa
            && !cfg.smaa
            && !cfg.taa;
        if constrained_adapter {
            max_supported_sample_count = 1;
        }
        let sample_count = clamp_supported_sample_count(
            normalize_sample_count(cfg.smoothing_samples),
            max_supported_sample_count,
        );
        let fxaa_requested = cfg.fxaa || msaa_traded_for_fxaa;
        perro_structs::structs::boot_log::mark("gpu: msaa caps query");
        let msaa_color = create_msaa_color_target(
            &device,
            render_format,
            render_width,
            render_height,
            sample_count,
        );
        perro_structs::structs::boot_log::mark("gpu: msaa target");
        let post = PostProcessor::new(&device, &queue, render_format, render_width, render_height);
        perro_structs::structs::boot_log::mark("gpu: post processor");
        let mut present = PresentProcessor::new(&device, surface_view_format);
        perro_structs::structs::boot_log::mark("gpu: present processor");
        present.set_output_size(width, height);
        present.set_fxaa_active(fxaa_requested && sample_count == 1);
        present.set_smaa_active(cfg.smaa && sample_count == 1);
        present.set_taa_active(cfg.taa && sample_count == 1);
        let camera_stream_tonemap = CameraStreamTonemap::new(&device, render_format);
        let present_scene_bind_group = present.create_bind_group(&device, post.scene_view());
        let gpu_timer = timestamp_query_enabled.then(|| GpuTimestampTimer::new(&device, &queue));
        // Built once here (the builtin mesh prefix is uploaded exactly once for
        // the whole process); every Gpu3D mirrors its handles.
        let mesh_arena = SharedMeshArena::new(&device, cfg.meshlets_enabled, cfg.dev_meshlets);
        perro_structs::structs::boot_log::mark("gpu: tonemap + timer + mesh arena");

        Ok(Self {
            window_handle: window,
            surface,
            adapter,
            device,
            queue,
            config,
            surface_resync_request: None,
            surface_acquire_failure: None,
            surface_view_format,
            hdr_status: selection.status,
            render_width,
            render_height,
            render_scale,
            max_render_pixels,
            render_format,
            sample_count,
            max_supported_sample_count,
            sample_count_3d_target: normalize_sample_count(cfg.smoothing_samples_3d),
            sample_count_3d_applied: cfg.smoothing_samples_3d == cfg.smoothing_samples,
            fxaa_requested,
            smaa_requested: cfg.smaa,
            taa_requested: cfg.taa,
            taa_frame_index: 0,
            taa_prev_view_proj: None,
            shadow_pcf_high: cfg.shadow_quality == crate::ShadowQuality::High,
            msaa_color,
            post,
            post_view_generation: 1,
            accessibility: None,
            present,
            present_scene_bind_group,
            present_intermediate_bind_group: None,
            shared_textures: SharedTextureStore::default(),
            mesh_arena,
            shared_texture_frame_counter: 0,
            pipeline_registries: PipelineRegistryCache::new(),
            two_d: None,
            late_overlay_2d: None,
            ui: None,
            three_d: None,
            point_particles_3d: None,
            water: None,
            camera_stream_targets: AHashMap::new(),
            camera_stream_content_revisions: AHashMap::new(),
            next_camera_stream_content_revision: 0,
            next_camera_stream_post_view_key: 0,
            camera_stream_external_bindings: AHashMap::new(),
            camera_stream_3d_bindings: AHashMap::new(),
            camera_stream_2d: AHashMap::new(),
            camera_stream_3d: AHashMap::new(),
            camera_stream_particles_3d: AHashMap::new(),
            camera_stream_water: AHashMap::new(),
            camera_stream_post: AHashMap::new(),
            camera_stream_tonemap,
            camera_stream_draws_scratch: Vec::new(),
            camera_image_save_requests: Vec::new(),
            camera_image_save_pending: Vec::new(),
            last_prepare_particles_revision: u64::MAX,
            last_prepare_water_2d_revision: u64::MAX,
            last_prepare_water_3d_revision: u64::MAX,
            last_prepare_3d_camera: None,
            last_prepare_3d_lighting: None,
            last_prepare_3d_draws_revision: u64::MAX,
            last_prepare_3d_decals_revision: u64::MAX,
            last_prepare_3d_width: render_width,
            last_prepare_3d_height: render_height,
            retained_scene_valid: false,
            retained_scene_key: RetainedSceneKey::default(),
            main_scene_sampled_texture_slots: ahash::AHashSet::new(),
            main_scene_sampled_key: None,
            meshlets_enabled: cfg.meshlets_enabled,
            dev_meshlets: cfg.dev_meshlets,
            meshlet_debug_view: cfg.meshlet_debug_view,
            occlusion_culling: cfg.occlusion_culling,
            ssao: effective_ssao,
            texture_filter: cfg.texture_filter,
            shader_variant_mode: cfg.shader_variant_mode,
            indirect_first_instance_enabled,
            multi_draw_indirect_enabled,
            multi_draw_indirect_count_enabled,
            gpu_timer,
            last_present: None,
            presented_once: false,
            virtual_size_2d: [1920.0, 1080.0],
        })
    }

    /// Drop the retained-scene fast path: the next frame re-encodes the whole
    /// scene chain. Called from every mutation that can change the rendered
    /// image without going through a per-frame dirty bit or content revision
    /// (texture invalidation / stream writes / target teardown / resize).
    pub(crate) fn invalidate_retained_scene(&mut self) {
        self.retained_scene_valid = false;
    }

    /// Project virtual canvas used for the 2D aspect-fit world-to-pixel rule
    /// in offscreen (camera stream / sub view) passes.
    pub fn set_virtual_size_2d(&mut self, size: [f32; 2]) {
        if size[0].is_finite() && size[1].is_finite() && size[0] > 0.0 && size[1] > 0.0 {
            self.virtual_size_2d = size;
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(window: Arc<Window>, cfg: GpuConfig) -> Result<Self, String> {
        pollster::block_on(Self::new_async(window, cfg))
    }

    /// One-way latch: the first frame that needs the 3D pipeline switches from
    /// the 2D sample count (graphics.msaa_2d) to the 3D one (graphics.msaa).
    pub(crate) fn ensure_3d_sample_count(&mut self) {
        if self.sample_count_3d_applied {
            return;
        }
        let target = self.sample_count_3d_target;
        self.sample_count_3d_applied = true;
        self.set_smoothing_samples(target);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        // Same order as new_async: author render scale, then hardware caps.
        let (scaled_width, scaled_height) = scaled_render_size(width, height, self.render_scale);
        let (render_width, render_height) = capped_render_size_with_pixel_limit(
            scaled_width,
            scaled_height,
            self.device.limits().max_texture_dimension_2d,
            self.max_render_pixels,
        );
        let render_size_changed =
            self.render_width != render_width || self.render_height != render_height;
        // Size early-out first: a window drag fires WM_SIZE per pixel step and
        // set_hdr_mode costs surface caps + display HDR driver queries. Explicit
        // mode changes go through set_hdr_mode directly, so nothing is lost.
        // The render size is part of the check: a failed-acquire reconfigure
        // moves the config on its own and the rebuild still has to land.
        if self.config.width == width && self.config.height == height && !render_size_changed {
            return;
        }
        self.invalidate_retained_scene();
        let mode = self.hdr_status.requested;
        self.set_hdr_mode(mode);
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        // Push before the render-size early-out: the swapchain can change size
        // while the capped render size stays put, and the TAA merged-resolve
        // gate compares the two.
        self.present.set_output_size(width, height);
        self.render_width = render_width;
        self.render_height = render_height;
        if !render_size_changed {
            return;
        }
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.resize(&self.device, render_width, render_height);
        }
        if let (Some(water), Some(three_d)) = (self.water.as_mut(), self.three_d.as_ref()) {
            water.set_scene_color_size(
                &self.device,
                three_d.depth_prepass_view(),
                render_width,
                render_height,
            );
        }
        self.post.resize(&self.device, render_width, render_height);
        self.post_view_generation = next_nonzero_generation(self.post_view_generation);
        if let Some(accessibility) = self.accessibility.as_mut() {
            accessibility.resize(&self.device, render_width, render_height);
        }
        self.present_scene_bind_group = self
            .present
            .create_bind_group(&self.device, self.post.scene_view());
        self.present_intermediate_bind_group = self.accessibility.as_ref().map(|accessibility| {
            self.present
                .create_bind_group(&self.device, accessibility.intermediate_view())
        });
        self.msaa_color = create_msaa_color_target(
            &self.device,
            self.render_format,
            render_width,
            render_height,
            self.sample_count,
        );
        // Force next 3D prepare to refresh viewport-dependent GPU state.
        self.last_prepare_3d_width = 0;
        self.last_prepare_3d_height = 0;
    }

    pub fn set_smoothing_samples(&mut self, samples: u32) {
        let sample_count = clamp_supported_sample_count(
            normalize_sample_count(samples),
            self.max_supported_sample_count,
        );
        // Explicit calls pin the sample count; the 3D latch must not override.
        self.sample_count_3d_target = sample_count;
        self.sample_count_3d_applied = true;
        if sample_count == self.sample_count {
            return;
        }
        self.invalidate_retained_scene();
        self.sample_count = sample_count;
        // FXAA/SMAA/TAA never stack on MSAA: the passes follow the live sample
        // count (e.g. the 2D->3D latch turning MSAA on drops the resources).
        self.present
            .set_fxaa_active(self.fxaa_requested && sample_count == 1);
        self.present
            .set_smaa_active(self.smaa_requested && sample_count == 1);
        self.present
            .set_taa_active(self.taa_requested && sample_count == 1);
        self.post_view_generation = next_nonzero_generation(self.post_view_generation);
        if let Some(two_d) = self.two_d.as_mut() {
            two_d.set_sample_count(&self.device, self.render_format, sample_count);
        }
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.set_sample_count(
                &self.device,
                self.pipeline_registries.get_or_create(
                    &self.device,
                    self.render_format,
                    sample_count,
                ),
                self.render_width,
                self.render_height,
            );
        }
        if let Some(point_particles_3d) = self.point_particles_3d.as_mut() {
            point_particles_3d.set_sample_count(&self.device, self.render_format, sample_count);
        }
        if self.water.is_some() {
            let rebuilt = if let (Some(water), Some(two_d), Some(three_d)) = (
                self.water.as_mut(),
                self.two_d.as_ref(),
                self.three_d.as_ref(),
            ) {
                water.set_sample_count(
                    &self.device,
                    self.render_format,
                    sample_count,
                    two_d.camera_bind_group_layout(),
                    three_d.water_camera_bind_group_layout(),
                );
                true
            } else {
                false
            };
            if !rebuilt {
                // Camera layouts unavailable: drop the water GPU state so it
                // is lazily recreated at the new sample count.
                self.water = None;
            }
        }
        self.msaa_color = create_msaa_color_target(
            &self.device,
            self.render_format,
            self.render_width,
            self.render_height,
            sample_count,
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn gpu_timestamp_queries_requested() -> bool {
    let explicit = std::env::var("PERRO_GPU_TIMESTAMPS").ok();
    let profiling = std::env::var_os("PERRO_TIMING_CSV").is_some()
        || std::env::var_os("PERRO_PROFILE_CSV").is_some()
        || std::env::var_os("PERRO_GPU_BENCH").is_some();
    gpu_timestamp_query_policy(explicit.as_deref(), cfg!(debug_assertions), profiling)
}

#[cfg(not(target_arch = "wasm32"))]
fn gpu_timestamp_query_policy(explicit: Option<&str>, debug: bool, profiling: bool) -> bool {
    explicit.map_or(debug || profiling, |value| {
        !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "off" | "false" | "no"
        )
    })
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod timestamp_query_tests {
    use super::gpu_timestamp_query_policy;

    #[test]
    fn release_default_skips_timestamp_readback() {
        assert!(!gpu_timestamp_query_policy(None, false, false));
    }

    #[test]
    fn debug_and_profile_keep_timestamps() {
        assert!(gpu_timestamp_query_policy(None, true, false));
        assert!(gpu_timestamp_query_policy(None, false, true));
    }

    #[test]
    fn explicit_timestamp_setting_wins() {
        assert!(gpu_timestamp_query_policy(Some("1"), false, false));
        assert!(gpu_timestamp_query_policy(Some("yes"), false, false));
        assert!(!gpu_timestamp_query_policy(Some("0"), true, true));
        assert!(!gpu_timestamp_query_policy(Some("OFF"), true, true));
    }
}
