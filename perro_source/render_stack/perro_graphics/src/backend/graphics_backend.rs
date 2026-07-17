use super::*;

impl GraphicsBackend for PerroGraphics {
    fn attach_window(&mut self, window: Arc<Window>) {
        if self.gpu.is_none() {
            #[cfg(target_arch = "wasm32")]
            {
                if self.pending_gpu.is_some() {
                    return;
                }
                let slot = Arc::new(Mutex::new(None));
                let slot_clone = slot.clone();
                let cfg = GpuConfig {
                    smoothing_samples: self.smoothing_samples,
                    vsync_enabled: self.vsync_enabled,
                    meshlets_enabled: self.meshlets_enabled,
                    dev_meshlets: self.dev_meshlets,
                    meshlet_debug_view: self.meshlet_debug_view,
                    occlusion_culling: self.occlusion_culling,
                    ssao: self.ssao,
                    texture_filter: self.texture_filter,
                };
                wasm_bindgen_futures::spawn_local(async move {
                    let gpu = Gpu::new_async(window, cfg).await;
                    if let Ok(mut pending) = slot_clone.lock() {
                        *pending = gpu;
                    }
                });
                self.pending_gpu = Some(slot);
                self.redraw_requested = true;
                return;
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let cfg = GpuConfig {
                    smoothing_samples: self.smoothing_samples,
                    vsync_enabled: self.vsync_enabled,
                    meshlets_enabled: self.meshlets_enabled,
                    dev_meshlets: self.dev_meshlets,
                    meshlet_debug_view: self.meshlet_debug_view,
                    occlusion_culling: self.occlusion_culling,
                    ssao: self.ssao,
                    texture_filter: self.texture_filter,
                };
                let mut gpu = Gpu::new(window, cfg);
                if let Some(gpu_ref) = gpu.as_mut() {
                    let [vw, vh] = Gpu::virtual_size();
                    self.renderer_2d.set_virtual_viewport(vw, vh);
                    self.late_overlay_2d.set_virtual_viewport(vw, vh);
                    gpu_ref.resize(self.viewport.0.max(1), self.viewport.1.max(1));
                }
                self.gpu = gpu;
                self.redraw_requested = true;
            }
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.viewport = (width, height);
        self.renderer_2d.set_viewport(width, height);
        self.late_overlay_2d.set_viewport(width, height);
        if let Some(gpu) = &mut self.gpu {
            gpu.resize(width.max(1), height.max(1));
        }
        self.redraw_requested = true;
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
