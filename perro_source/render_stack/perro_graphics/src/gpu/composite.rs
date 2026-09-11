use super::*;

#[cfg(test)]
#[path = "../../tests/unit/composite_gpu_tests.rs"]
mod tests;

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

    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        present: &PresentProcessor,
        format: wgpu::TextureFormat,
        camera: &Camera3DState,
        camera_effects: &[PostProcessEffect],
        global_effects: &[PostProcessEffect],
        depth: Option<(&wgpu::TextureView, u64)>,
        accessibility: VisualAccessibilitySettings,
        static_shader_lookup: Option<StaticShaderLookup>,
        static_texture_lookup: Option<StaticTextureLookup>,
        hdr_output: bool,
    ) -> (bool, Duration, Duration) {
        // One chain owns its uniform/parameter ranges for the whole frame.
        // Separate apply_chain calls on one processor would overwrite the
        // camera uniforms with the later global queue writes before submission.
        self.effects.clear();
        self.effects.extend_from_slice(camera_effects);
        self.effects.extend_from_slice(global_effects);
        let post_enabled = PostProcessor::has_effects(&self.effects);
        let accessibility_enabled = accessibility.color_blind.is_some();
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
        let scene = self.post.scene_view().clone();
        let mut intermediate = false;
        let post_start = Instant::now();
        if post_enabled {
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
                    external_input_view_key: self.generation,
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
        } else {
            self.post.note_idle_frame(device);
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
