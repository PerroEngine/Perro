use super::*;

impl Gpu {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn wait_idle(&mut self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }

    #[cfg(target_arch = "wasm32")]
    pub fn wait_idle(&mut self) {}

    pub fn render_idle_clear(&mut self) -> bool {
        // Keep window alive for the full surface lifetime.
        self.window_handle.id();

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return false;
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return false,
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

        self.config.format = selection.format;
        self.config.color_space = selection.color_space;
        self.config.view_formats = (selection.view_format != selection.format)
            .then_some(selection.view_format)
            .into_iter()
            .collect();
        self.surface_view_format = selection.view_format;
        self.surface.configure(&self.device, &self.config);
        self.present = PresentProcessor::new(&self.device, self.surface_view_format);
        self.present_scene_bind_group = self
            .present
            .create_bind_group(&self.device, self.post.scene_view());
        self.present_intermediate_bind_group = self
            .present
            .create_bind_group(&self.device, self.accessibility.intermediate_view());
        self.ui = None;
        self.late_overlay_2d = None;
        self.hdr_status
    }

    pub async fn new_async(window: Arc<Window>, cfg: GpuConfig) -> Option<Self> {
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).ok()?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                apply_limit_buckets: false,
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok()?;
        let adapter_features = adapter.features();
        let mut required_features = wgpu::Features::empty();
        if adapter_features.contains(wgpu::Features::INDIRECT_FIRST_INSTANCE) {
            required_features |= wgpu::Features::INDIRECT_FIRST_INSTANCE;
        }
        #[cfg(not(target_arch = "wasm32"))]
        let enable_timestamp_queries = true;
        #[cfg(target_arch = "wasm32")]
        let enable_timestamp_queries = false;
        let timestamp_features =
            wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        if enable_timestamp_queries && adapter_features.contains(timestamp_features) {
            required_features |= timestamp_features;
        }

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("perro_device"),
                required_features,
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::default(),
            })
            .await
            .ok()?;
        let indirect_first_instance_enabled =
            required_features.contains(wgpu::Features::INDIRECT_FIRST_INSTANCE);
        // multi_draw_indexed_indirect (non-count) needs only INDIRECT_EXECUTION,
        // the same downlevel capability draw_indexed_indirect already relies on,
        // so it rides the existing indirect path with no extra feature request.
        let multi_draw_indirect_enabled = indirect_first_instance_enabled;
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
        let (render_width, render_height) =
            capped_render_size(width, height, device.limits().max_texture_dimension_2d);

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
        surface.configure(&device, &config);

        let max_supported_sample_count = max_supported_msaa_sample_count(&adapter, render_format);
        let sample_count = clamp_supported_sample_count(
            normalize_sample_count(cfg.smoothing_samples),
            max_supported_sample_count,
        );
        let two_d = Gpu2D::new(&device, render_format, sample_count, cfg.texture_filter);
        let late_overlay_2d = Gpu2D::new(&device, surface_view_format, 1, cfg.texture_filter);
        let ui = Some(GpuUi::new(&device, surface_view_format, cfg.texture_filter));
        let three_d = Gpu3D::new(
            &device,
            &queue,
            render_format,
            Gpu3DConfig {
                sample_count,
                width: render_width,
                height: render_height,
                meshlets_enabled: cfg.meshlets_enabled,
                dev_meshlets: cfg.dev_meshlets,
                meshlet_debug_view: cfg.meshlet_debug_view,
                occlusion_culling: cfg.occlusion_culling,
                ssao: cfg.ssao,
                indirect_first_instance_enabled,
                multi_draw_indirect_enabled,
                texture_filter: cfg.texture_filter,
            },
        );
        let point_particles_3d = GpuPointParticles3D::new(&device, render_format, sample_count);
        let camera_stream_2d = Gpu2D::new(&device, render_format, 1, cfg.texture_filter);
        let water = Some(GpuWater::new(
            &device,
            render_format,
            sample_count,
            two_d.camera_bind_group_layout(),
            three_d.water_camera_bind_group_layout(),
            three_d.depth_prepass_view(),
            render_width,
            render_height,
        ));
        let msaa_color = create_msaa_color_target(
            &device,
            render_format,
            render_width,
            render_height,
            sample_count,
        );
        let post = PostProcessor::new(&device, &queue, render_format, render_width, render_height);
        let accessibility =
            VisualAccessibilityProcessor::new(&device, render_format, render_width, render_height);
        let present = PresentProcessor::new(&device, surface_view_format);
        let camera_stream_tonemap = CameraStreamTonemap::new(&device, render_format);
        let present_scene_bind_group = present.create_bind_group(&device, post.scene_view());
        let present_intermediate_bind_group =
            present.create_bind_group(&device, accessibility.intermediate_view());
        let gpu_timer = timestamp_query_enabled.then(|| GpuTimestampTimer::new(&device, &queue));

        Some(Self {
            window_handle: window,
            surface,
            adapter,
            device,
            queue,
            config,
            surface_view_format,
            hdr_status: selection.status,
            render_width,
            render_height,
            render_format,
            sample_count,
            max_supported_sample_count,
            msaa_color,
            post,
            post_view_generation: 1,
            accessibility,
            present,
            present_scene_bind_group,
            present_intermediate_bind_group,
            two_d: Some(two_d),
            late_overlay_2d: Some(late_overlay_2d),
            ui,
            three_d: Some(three_d),
            point_particles_3d: Some(point_particles_3d),
            water,
            camera_stream_targets: AHashMap::new(),
            next_camera_stream_post_view_key: 0,
            camera_stream_external_bindings: AHashMap::new(),
            camera_stream_3d_bindings: AHashMap::new(),
            camera_stream_2d: Some(camera_stream_2d),
            camera_stream_3d: None,
            camera_stream_particles_3d: None,
            camera_stream_water: None,
            camera_stream_post: None,
            camera_stream_tonemap,
            camera_stream_draws_scratch: Vec::new(),
            last_prepare_particles_revision: u64::MAX,
            last_prepare_water_2d_revision: u64::MAX,
            last_prepare_water_3d_revision: u64::MAX,
            last_prepare_3d_camera: None,
            last_prepare_3d_lighting: None,
            last_prepare_3d_draws_revision: u64::MAX,
            last_prepare_3d_decals_revision: u64::MAX,
            last_prepare_3d_width: render_width,
            last_prepare_3d_height: render_height,
            meshlets_enabled: cfg.meshlets_enabled,
            dev_meshlets: cfg.dev_meshlets,
            meshlet_debug_view: cfg.meshlet_debug_view,
            occlusion_culling: cfg.occlusion_culling,
            ssao: cfg.ssao,
            texture_filter: cfg.texture_filter,
            indirect_first_instance_enabled,
            multi_draw_indirect_enabled,
            gpu_timer,
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(window: Arc<Window>, cfg: GpuConfig) -> Option<Self> {
        pollster::block_on(Self::new_async(window, cfg))
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        let mode = self.hdr_status.requested;
        self.set_hdr_mode(mode);
        if self.config.width == width && self.config.height == height {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        let (render_width, render_height) =
            capped_render_size(width, height, self.device.limits().max_texture_dimension_2d);
        let render_size_changed =
            self.render_width != render_width || self.render_height != render_height;
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
        self.accessibility
            .resize(&self.device, render_width, render_height);
        self.present_scene_bind_group = self
            .present
            .create_bind_group(&self.device, self.post.scene_view());
        self.present_intermediate_bind_group = self
            .present
            .create_bind_group(&self.device, self.accessibility.intermediate_view());
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
        if sample_count == self.sample_count {
            return;
        }
        self.sample_count = sample_count;
        self.post_view_generation = next_nonzero_generation(self.post_view_generation);
        if let Some(two_d) = self.two_d.as_mut() {
            two_d.set_sample_count(&self.device, self.render_format, sample_count);
        }
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.set_sample_count(
                &self.device,
                self.render_format,
                sample_count,
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
