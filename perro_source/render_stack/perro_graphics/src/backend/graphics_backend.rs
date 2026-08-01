use super::*;

impl GraphicsBackend for PerroGraphics {
    fn attach_window(&mut self, window: Arc<Window>) {
        if self.gpu.is_none() {
            #[cfg(target_arch = "wasm32")]
            {
                if self.pending_gpu.is_some() {
                    return;
                }
                // wasm is single-threaded; Arc+Mutex only bridge the
                // spawn_local boundary, never cross threads.
                #[allow(clippy::arc_with_non_send_sync)]
                let slot = Arc::new(Mutex::new(None));
                let slot_clone = slot.clone();
                let cfg = GpuConfig {
                    smoothing_samples: self.smoothing_2d_samples,
                    smoothing_samples_3d: self.smoothing_samples,
                    fxaa: self.fxaa_enabled,
                    smaa: self.smaa_enabled,
                    taa: self.taa_enabled,
                    vsync_enabled: self.vsync_enabled,
                    meshlets_enabled: self.meshlets_enabled,
                    dev_meshlets: self.dev_meshlets,
                    meshlet_debug_view: self.meshlet_debug_view,
                    occlusion_culling: self.occlusion_culling,
                    ssao: self.ssao,
                    texture_filter: self.texture_filter,
                    hdr_mode: self.hdr_mode,
                    shader_variant_mode: self.shader_variant_mode,
                    shadow_quality: self.shadow_quality,
                };
                wasm_bindgen_futures::spawn_local(async move {
                    let gpu = Gpu::new_async(window, cfg)
                        .await
                        .unwrap_or_else(|err| panic!("GPU init fail: {err}"));
                    if let Ok(mut pending) = slot_clone.lock() {
                        *pending = Some(gpu);
                    }
                });
                self.pending_gpu = Some(slot);
                self.redraw_requested = true;
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let cfg = GpuConfig {
                    smoothing_samples: self.smoothing_2d_samples,
                    smoothing_samples_3d: self.smoothing_samples,
                    fxaa: self.fxaa_enabled,
                    smaa: self.smaa_enabled,
                    taa: self.taa_enabled,
                    vsync_enabled: self.vsync_enabled,
                    meshlets_enabled: self.meshlets_enabled,
                    dev_meshlets: self.dev_meshlets,
                    meshlet_debug_view: self.meshlet_debug_view,
                    occlusion_culling: self.occlusion_culling,
                    ssao: self.ssao,
                    texture_filter: self.texture_filter,
                    hdr_mode: self.hdr_mode,
                    shader_variant_mode: self.shader_variant_mode,
                    shadow_quality: self.shadow_quality,
                };
                let mut gpu =
                    Gpu::new(window, cfg).unwrap_or_else(|err| panic!("GPU init fail: {err}"));
                gpu.set_virtual_size_2d(self.renderer_2d.virtual_viewport());
                // A (0,0) viewport means no resize landed yet; Gpu::new already
                // configured from window.inner_size(), so resizing to 1x1 here
                // would only build the post/MSAA/present chain twice.
                if self.viewport.0 > 0 && self.viewport.1 > 0 {
                    gpu.resize(self.viewport.0, self.viewport.1);
                }
                self.events
                    .push(RenderEvent::HdrStatusChanged(gpu.hdr_status()));
                self.gpu = Some(gpu);
                self.redraw_requested = true;
            }
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.viewport = (width, height);
        self.renderer_2d.set_viewport(width, height);
        self.late_overlay_2d.set_viewport(width, height);
        if let Some(gpu) = &mut self.gpu {
            let old_hdr = gpu.hdr_status();
            gpu.resize(width.max(1), height.max(1));
            let new_hdr = gpu.hdr_status();
            if new_hdr != old_hdr {
                self.events.push(RenderEvent::HdrStatusChanged(new_hdr));
            }
        }
        self.redraw_requested = true;
    }

    fn take_surface_resync_request(&mut self) -> Option<(u32, u32)> {
        self.gpu
            .as_mut()
            .and_then(Gpu::take_surface_resync_request)
            .filter(|&(width, height)| (width, height) != self.viewport)
    }

    fn set_smoothing(&mut self, enabled: bool) {
        self.smoothing_enabled = enabled;
        self.smoothing_samples = if enabled {
            self.smoothing_quality_samples.max(2)
        } else {
            1
        };
        if let Some(gpu) = &mut self.gpu {
            gpu.set_smoothing_samples(self.smoothing_samples);
        }
        self.redraw_requested = true;
    }

    fn set_smoothing_samples(&mut self, samples: u32) {
        let normalized = normalize_aa_sample_count(samples);
        self.smoothing_samples = normalized;
        self.smoothing_enabled = normalized > 1;
        if normalized > 1 {
            self.smoothing_quality_samples = normalized;
        }
        if let Some(gpu) = &mut self.gpu {
            gpu.set_smoothing_samples(normalized);
        }
        self.redraw_requested = true;
    }

    fn profile_snapshot(&self) -> GraphicsProfileSnapshot {
        GraphicsProfileSnapshot {
            active_meshes: self.resources.active_mesh_count() as u32,
            active_materials: self.resources.active_material_count() as u32,
            active_textures: self.resources.active_texture_count() as u32,
        }
    }

    fn wait_idle(&mut self) {
        if let Some(gpu) = &mut self.gpu {
            gpu.wait_idle();
        }
    }

    fn set_startup_warm_boost(&mut self, enabled: bool) {
        self.startup_warm_boost = enabled;
    }

    fn pipeline_warm_idle(&self) -> bool {
        // Mirror the drain gate exactly (shared predicate): queued materials
        // *and* the base pipeline families, both only once the lazy 3D world
        // exists - b4 that nothing can drain + the queue must not hold the
        // splash open (a 2D-only game would sit at the hard timeout).
        !self.pipeline_warm_pending()
    }

    fn draw_frame(&mut self) {
        let _ = self.draw_frame_timed();
    }

    fn draw_frame_timed(&mut self) -> Option<DrawFrameTiming> {
        self.draw_frame_timed_internal(std::iter::empty::<RenderCommand>())
    }

    fn draw_frame_with_late_overlay<I>(&mut self, overlay_commands: I)
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        let _ = self.draw_frame_timed_internal(overlay_commands);
    }

    fn draw_frame_with_late_overlay_timed<I>(
        &mut self,
        overlay_commands: I,
    ) -> Option<DrawFrameTiming>
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        self.draw_frame_timed_internal(overlay_commands)
    }

    fn submit_late_overlay_many<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        self.process_late_overlay_commands(commands);
        self.redraw_requested = true;
    }
}
