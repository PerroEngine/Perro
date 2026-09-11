use super::*;

#[cfg(test)]
#[path = "../../tests/unit/composite_gpu_tests.rs"]
mod tests;

#[inline]
fn post_effects_cache_safe(effects: &[PostProcessEffect]) -> bool {
    // Builtins read only their uniforms + input pixels. Custom shaders and
    // LUT-backed effects may read external state, so never freeze their output.
    effects.iter().all(|effect| {
        matches!(
            effect,
            PostProcessEffect::Blur { .. }
                | PostProcessEffect::Pixelate { .. }
                | PostProcessEffect::PixelArt { .. }
                | PostProcessEffect::Warp { .. }
                | PostProcessEffect::Vignette { .. }
                | PostProcessEffect::Crt { .. }
                | PostProcessEffect::ColorFilter { .. }
                | PostProcessEffect::ReverseFilter { .. }
                | PostProcessEffect::ChromaKey { .. }
                | PostProcessEffect::Bloom { .. }
                | PostProcessEffect::Saturate { .. }
                | PostProcessEffect::BlackWhite { .. }
                | PostProcessEffect::ColorGrade { .. }
        )
    })
}

/// Full-output-resolution, scene-linear frame. Never aliases the retained scene.
pub(super) struct FrameComposite {
    pub post: PostProcessor,
    accessibility: Option<VisualAccessibilityProcessor>,
    size: [u32; 2],
    generation: u64,
    present_input: PresentBindGroups,
    present_intermediate: Option<PresentBindGroups>,
    fallback_depth: wgpu::TextureView,
    effects: Vec<PostProcessEffect>,
    // Final post output stays valid while scene/UI content stays stable.
    // Caller passes None whenever any producer may write new pixels.
    post_cache_key: Option<u64>,
    post_cache_effects: Vec<PostProcessEffect>,
    camera_post: Option<PostProcessor>,
    camera_generation: u64,
    camera_cache_key: Option<u64>,
    camera_cache_effects: Vec<PostProcessEffect>,
}

impl FrameComposite {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        size: [u32; 2],
        present: &PresentProcessor,
    ) -> Self {
        let post = PostProcessor::new(device, queue, format, size[0], size[1]);
        let present_input = present.create_bind_group(device, post.scene_view());
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("perro_composite_fallback_depth"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        Self {
            post,
            accessibility: None,
            size,
            generation: 1,
            present_input,
            present_intermediate: None,
            fallback_depth: depth.create_view(&Default::default()),
            effects: Vec::new(),
            post_cache_key: None,
            post_cache_effects: Vec::new(),
            camera_post: None,
            camera_generation: 1,
            camera_cache_key: None,
            camera_cache_effects: Vec::new(),
        }
    }

    pub fn rebind_present(&mut self, device: &wgpu::Device, present: &PresentProcessor) {
        self.present_input = present.create_bind_group(device, self.post.scene_view());
        self.present_intermediate = self
            .accessibility
            .as_ref()
            .map(|a| present.create_bind_group(device, a.intermediate_view()));
    }

    pub fn resize(&mut self, device: &wgpu::Device, size: [u32; 2], present: &PresentProcessor) {
        if self.size == size {
            return;
        }
        self.size = size;
        self.generation = next_nonzero_generation(self.generation);
        self.post.resize(device, size[0], size[1]);
        self.post_cache_key = None;
        self.post_cache_effects.clear();
        if let Some(camera_post) = self.camera_post.as_mut() {
            camera_post.resize(device, size[0], size[1]);
            self.camera_generation = next_nonzero_generation(self.camera_generation);
            self.camera_cache_key = None;
            self.camera_cache_effects.clear();
        }
        if let Some(a) = self.accessibility.as_mut() {
            a.resize(device, size[0], size[1]);
        }
        self.rebind_present(device, present);
    }

    pub fn present_bind_group(&self, intermediate: bool) -> &PresentBindGroups {
        if intermediate {
            self.present_intermediate
                .as_ref()
                .expect("composite intermediate")
        } else {
            &self.present_input
        }
    }

    pub fn camera_scene_view(&self) -> Option<&wgpu::TextureView> {
        self.camera_post.as_ref().map(PostProcessor::scene_view)
    }

    pub fn camera_generation(&self) -> u64 {
        self.camera_generation
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn set_constrained(&mut self, constrained: bool) {
        self.post.set_constrained(constrained);
        if let Some(camera_post) = self.camera_post.as_mut() {
            camera_post.set_constrained(constrained);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn apply_camera(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        format: wgpu::TextureFormat,
        input_view: &wgpu::TextureView,
        input_generation: u64,
        camera: &Camera3DState,
        camera_effects: &[PostProcessEffect],
        depth: Option<(&wgpu::TextureView, u64)>,
        static_shader_lookup: Option<StaticShaderLookup>,
        static_texture_lookup: Option<StaticTextureLookup>,
        hdr_output: bool,
        cache_key: Option<u64>,
    ) -> (bool, Duration) {
        let camera_enabled = PostProcessor::has_effects(camera_effects);
        if !camera_enabled {
            if self.camera_cache_key.is_some() || !self.camera_cache_effects.is_empty() {
                self.camera_generation = next_nonzero_generation(self.camera_generation);
            }
            if let Some(camera_post) = self.camera_post.as_mut() {
                camera_post.note_idle_frame(device);
            }
            self.camera_cache_key = None;
            self.camera_cache_effects.clear();
            return (false, Duration::ZERO);
        }

        let camera_post = self.camera_post.get_or_insert_with(|| {
            PostProcessor::new(device, queue, format, self.size[0], self.size[1])
        });
        camera_post.resize(device, self.size[0], self.size[1]);
        let camera_output_view = camera_post.scene_view().clone();
        if self.camera_cache_effects != camera_effects {
            self.camera_generation = next_nonzero_generation(self.camera_generation);
        }
        let cache_hit = post_effects_cache_safe(camera_effects)
            && cache_key.is_some()
            && self.camera_cache_key == cache_key
            && self.camera_cache_effects == camera_effects;
        let start = Instant::now();
        if !cache_hit {
            let (depth_view, depth_key) = depth
                .map(|(view, generation)| (view, generation.wrapping_mul(2).wrapping_add(1)))
                .unwrap_or((&self.fallback_depth, 0));
            camera_post.apply_chain(
                &PostProcessContext {
                    device,
                    queue,
                    output_view: &camera_output_view,
                    camera,
                    external_input_view_key: input_generation,
                    depth_view_key: depth_key,
                    static_shader_lookup,
                    static_texture_lookup,
                    hdr_output,
                },
                &PostProcessChainData {
                    input_view,
                    depth_view,
                    effects: camera_effects,
                },
                encoder,
            );
            self.camera_cache_key = cache_key;
            self.camera_cache_effects.clear();
            self.camera_cache_effects.extend_from_slice(camera_effects);
        }
        (true, start.elapsed())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn apply_global(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        present: &PresentProcessor,
        format: wgpu::TextureFormat,
        input_view: &wgpu::TextureView,
        input_generation: u64,
        camera: &Camera3DState,
        global_effects: &[PostProcessEffect],
        depth: Option<(&wgpu::TextureView, u64)>,
        accessibility: VisualAccessibilitySettings,
        static_shader_lookup: Option<StaticShaderLookup>,
        static_texture_lookup: Option<StaticTextureLookup>,
        hdr_output: bool,
        post_cache_key: Option<u64>,
    ) -> (bool, Duration, Duration) {
        self.effects.clear();
        self.effects.extend_from_slice(global_effects);
        let post_enabled = PostProcessor::has_effects(&self.effects);
        let accessibility_enabled = accessibility.color_blind.is_some();
        let post_cache_hit = post_enabled
            && !accessibility_enabled
            && post_effects_cache_safe(&self.effects)
            && post_cache_key.is_some()
            && self.post_cache_key == post_cache_key
            && self.post_cache_effects == self.effects;
        if post_enabled || accessibility_enabled {
            let a = self.accessibility.get_or_insert_with(|| {
                VisualAccessibilityProcessor::new(device, format, self.size[0], self.size[1])
            });
            if a.resize(device, self.size[0], self.size[1]) || self.present_intermediate.is_none() {
                self.present_intermediate =
                    Some(present.create_bind_group(device, a.intermediate_view()));
            }
        } else if let Some(a) = self.accessibility.as_mut()
            && a.note_idle_frame(device)
        {
            self.present_intermediate = None;
        }
        let scene = input_view.clone();
        let mut intermediate = false;
        let post_start = Instant::now();
        if post_enabled && !post_cache_hit {
            if depth.is_none() {
                let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("perro_composite_clear_depth"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.fallback_depth,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
            }
            let (depth_view, depth_key) = depth
                .map(|(view, generation)| (view, generation.wrapping_mul(2).wrapping_add(1)))
                .unwrap_or((&self.fallback_depth, 0));
            self.post.apply_chain(
                &PostProcessContext {
                    device,
                    queue,
                    output_view: self
                        .accessibility
                        .as_ref()
                        .expect("post-process target initialized")
                        .intermediate_view(),
                    camera,
                    external_input_view_key: input_generation,
                    depth_view_key: depth_key,
                    static_shader_lookup,
                    static_texture_lookup,
                    hdr_output,
                },
                &PostProcessChainData {
                    input_view: &scene,
                    depth_view,
                    effects: &self.effects,
                },
                encoder,
            );
            intermediate = true;
            if !accessibility_enabled {
                self.post_cache_key = post_cache_key;
                self.post_cache_effects.clone_from(&self.effects);
            }
        } else if post_cache_hit {
            // `accessibility` stays off on cache hits, so the retained post
            // target remains the present input without a new GPU pass.
            intermediate = true;
        } else {
            self.post.note_idle_frame(device);
            self.post_cache_key = None;
            self.post_cache_effects.clear();
        }
        let post_time = post_start.elapsed();
        let accessibility_start = Instant::now();
        if accessibility_enabled {
            let a = self
                .accessibility
                .as_mut()
                .expect("accessibility target initialized");
            let scratch = a.intermediate_view().clone();
            let (input, output) = if intermediate {
                (&scratch, &scene)
            } else {
                (&scene, &scratch)
            };
            a.apply(device, queue, encoder, input, output, accessibility);
            intermediate = !intermediate;
        }
        (intermediate, post_time, accessibility_start.elapsed())
    }
}
