use super::*;

// Source string the 3D external material slot binds a stream target under;
// must match the one `frame.rs` upserts with.
pub(crate) fn camera_stream_texture_source(node: NodeID) -> String {
    format!("__camera_stream__:{}", node.as_u64())
}

impl Gpu {
    pub fn remove_camera_stream(&mut self, node: NodeID, output_texture: perro_ids::TextureID) {
        self.invalidate_retained_scene();
        // no target => the stream rendered straight into a source texture
        // (webcam passthrough) that no consumer bound externally.
        let had_target = self.camera_stream_targets.remove(&node).is_some();
        // Cached tonemap bind groups hold clones of this target's views.
        self.camera_stream_tonemap.invalidate_bind_cache();
        self.camera_stream_content_revisions.remove(&node);
        self.camera_stream_external_bindings.remove(&node);
        self.camera_stream_3d_bindings.remove(&node);
        self.camera_stream_3d.remove(&node);
        self.camera_stream_2d.remove(&node);
        self.camera_stream_particles_3d.remove(&node);
        self.camera_stream_water.remove(&node);
        self.camera_stream_post.remove(&node);
        // Consumer caches (2D sprite, UI image, 3D material slot) retain views
        // + bind groups built from the removed target. Without this unbind the
        // whole GpuCameraStreamTarget (color + post_input + tonemap_input +
        // depth) stays alive for the rest of the session.
        if had_target {
            self.invalidate_texture(
                output_texture,
                Some(camera_stream_texture_source(node).as_str()),
            );
        }
    }

    /// Whether the lazy main 3D world exists yet. Queued pipeline warms are a
    /// no-op until it does (a 2D-only session never builds it), so callers that
    /// keep the frame pump alive for a pending warm queue must check this or a
    /// 2D-only game with materials would redraw forever.
    #[inline]
    pub fn has_three_d(&self) -> bool {
        self.three_d.is_some()
    }

    /// Monotonic count of pipeline sets the main 3D world has compiled, from
    /// both the warm queue and lazy first-draw misses. Callers diff it across a
    /// frame; camera-stream worlds are deliberately excluded (they stay lazy by
    /// design and would blur the main view's reading).
    #[inline]
    pub fn pipeline_compiles_3d(&self) -> u64 {
        self.three_d
            .as_ref()
            .map_or(0, |three_d| three_d.pipeline_compiles())
    }

    /// True while the main 3D world still has base pipeline families to build
    /// (rigid + its depth/shadow, the multimesh trio, sky). False w/o a 3D
    /// world: a 2D-only session never builds them, so it must not pin the
    /// splash or the frame pump.
    #[inline]
    pub fn base_families_pending(&self) -> bool {
        self.three_d
            .as_ref()
            .is_some_and(|three_d| three_d.base_families_pending())
    }

    // Drain queued material warms into the main 3D pipeline caches, bounded by
    // a per-frame budget. Leaves the queue untouched until `three_d` exists (it
    // is created lazily on the first 3D frame); camera-stream worlds stay lazy
    // on purpose.
    //
    // An empty queue is NOT a bail: the same budget warms the base pipeline
    // families as leftover work, and those are exactly what a splash hold with
    // no pending materials must compile. Only a warm world (or no world) is a
    // cheap no-op.
    //
    // Draining the whole queue in one frame is the scene-transition spike: a
    // scene load queues every material at once and each new material shape
    // costs a WGSL compile plus four pipeline creations per render path. Cache
    // hits stay free and never count against the budget, so nothing slows down
    // for scenes whose pipelines are already built.
    //
    // Returns how many materials compiled this frame.
    pub fn warm_material_pipelines(
        &mut self,
        materials: &mut Vec<std::sync::Arc<perro_render_bridge::Material3D>>,
        static_shader_lookup: Option<crate::StaticShaderLookup>,
        max_compiles: usize,
        time_budget: Option<std::time::Duration>,
    ) -> usize {
        let Some(three_d) = self.three_d.as_mut() else {
            return 0;
        };
        if materials.is_empty() && !three_d.base_families_pending() {
            return 0;
        }
        three_d.warm_material_pipelines_budgeted(
            &self.device,
            materials,
            static_shader_lookup,
            max_compiles,
            time_budget,
        )
    }

    pub fn invalidate_custom_material_pipelines(&mut self) {
        self.invalidate_retained_scene();
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.invalidate_custom_pipelines();
        }
        for camera_stream_3d in self.camera_stream_3d.values_mut() {
            camera_stream_3d.invalidate_custom_pipelines();
        }
    }

    pub fn invalidate_texture(&mut self, texture: perro_ids::TextureID, source: Option<&str>) {
        self.invalidate_retained_scene();
        // Drop the shared upload first; the per-consumer fan-out below drops
        // the handles + bind groups, so the next demand re-uploads once.
        if let Some(source) = source {
            self.shared_textures.invalidate_source(source);
        }
        if let Some(two_d) = self.two_d.as_mut() {
            two_d.invalidate_texture(texture);
        }
        if let Some(late_overlay_2d) = self.late_overlay_2d.as_mut() {
            late_overlay_2d.invalidate_texture(texture);
        }
        for camera_stream_2d in self.camera_stream_2d.values_mut() {
            camera_stream_2d.invalidate_texture(texture);
        }
        if let Some(ui) = self.ui.as_mut() {
            ui.invalidate_image_texture(texture);
        }
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.invalidate_material_texture(texture.index());
            three_d.invalidate_material_texture_source(source);
            three_d.invalidate_decal_texture(texture);
        }
        for camera_stream_3d in self.camera_stream_3d.values_mut() {
            camera_stream_3d.invalidate_material_texture(texture.index());
            camera_stream_3d.invalidate_material_texture_source(source);
            camera_stream_3d.invalidate_decal_texture(texture);
        }
    }

    // mark/unmark a texture id as a stream (webcam/video) across every consumer
    // cache, so rebuilds use a single-level (no-mip) texture that supports the
    // per-frame in-place base upload.
    pub fn set_stream_texture(&mut self, texture: perro_ids::TextureID, is_stream: bool) {
        self.invalidate_retained_scene();
        if let Some(two_d) = self.two_d.as_mut() {
            two_d.set_stream_texture(texture, is_stream);
        }
        if let Some(late_overlay_2d) = self.late_overlay_2d.as_mut() {
            late_overlay_2d.set_stream_texture(texture, is_stream);
        }
        for camera_stream_2d in self.camera_stream_2d.values_mut() {
            camera_stream_2d.set_stream_texture(texture, is_stream);
        }
        if let Some(ui) = self.ui.as_mut() {
            ui.set_stream_texture(texture, is_stream);
        }
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.set_stream_texture(texture.index(), is_stream);
        }
        for camera_stream_3d in self.camera_stream_3d.values_mut() {
            camera_stream_3d.set_stream_texture(texture.index(), is_stream);
        }
    }

    // in-place base-level upload of a stream frame into every resident cache; no
    // texture/sampler/bind-group recreation, no mip regen. missing/resized caches
    // no-op (they rebuild from decoded data on the next prepare). `source` keeps
    // 3D custom-source material slots fresh (in-place write or invalidate).
    pub fn write_stream_texture(
        &mut self,
        texture: perro_ids::TextureID,
        source: Option<&str>,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) {
        // Webcam / video frames land straight in the texels of a texture the
        // scene may sample, with no dirty bit and no revision bump. The
        // retained scene can no longer be trusted to match.
        self.invalidate_retained_scene();
        let queue = &self.queue;
        // Shared fast path: resident stream uploads are single shared textures,
        // so one base-level write refreshes every consumer's bind group at
        // once. The per-consumer fan-out below only remains for the miss case
        // (nothing resident yet, or stale mismatched entries to invalidate).
        if let Some(source) = source
            && self
                .shared_textures
                .write_stream_base_level(queue, source, width, height, rgba)
        {
            return;
        }
        // Miss path fan-out: several consumer caches usually resolve to the
        // same SharedGpuTexture, so collapse repeat base-level writes of this
        // one frame to a single upload per distinct GPU texture. Each consumer
        // still runs its residency check and its own invalidation bookkeeping.
        let dedupe = crate::texture_mips::StreamWriteDedupe::begin();
        if let Some(two_d) = self.two_d.as_mut() {
            two_d.write_stream_texture(queue, texture, width, height, rgba);
        }
        if let Some(late_overlay_2d) = self.late_overlay_2d.as_mut() {
            late_overlay_2d.write_stream_texture(queue, texture, width, height, rgba);
        }
        for camera_stream_2d in self.camera_stream_2d.values_mut() {
            camera_stream_2d.write_stream_texture(queue, texture, width, height, rgba);
        }
        if let Some(ui) = self.ui.as_mut() {
            ui.write_stream_texture(queue, texture, width, height, rgba);
        }
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.write_stream_material_texture(queue, texture.index(), width, height, rgba);
            three_d.write_stream_material_texture_source(queue, source, width, height, rgba);
        }
        for camera_stream_3d in self.camera_stream_3d.values_mut() {
            camera_stream_3d.write_stream_material_texture(
                queue,
                texture.index(),
                width,
                height,
                rgba,
            );
            camera_stream_3d
                .write_stream_material_texture_source(queue, source, width, height, rgba);
        }
        drop(dedupe);
    }

    /// `(distinct base-level uploads issued, redundant uploads elided)` across
    /// every `write_stream_texture` fan-out since startup. Profiling hook; the
    /// dedupe itself is covered by `texture_mips`' own tests.
    #[allow(dead_code)]
    pub fn stream_texture_upload_counts(&self) -> (u64, u64) {
        crate::texture_mips::stream_write_totals()
    }

    pub(super) fn ensure_camera_stream_target(
        &mut self,
        node: NodeID,
        resolution: [u32; 2],
        needs_intermediate: bool,
        needs_tonemap_input: bool,
        needs_post_depth: bool,
    ) -> Option<&GpuCameraStreamTarget> {
        let resolution = [resolution[0].max(1), resolution[1].max(1)];
        let recreate = self.camera_stream_targets.get(&node).is_none_or(|target| {
            target.resolution != resolution
                || target.post_input_view.is_some() != needs_intermediate
                || target.tonemap_input_view.is_some() != needs_tonemap_input
                || target.depth_view.is_some() != needs_post_depth
        });
        if recreate {
            self.next_camera_stream_post_view_key =
                next_nonzero_generation(self.next_camera_stream_post_view_key);
            let post_view_key = self
                .next_camera_stream_post_view_key
                .wrapping_mul(8)
                .wrapping_add(4);
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("perro_camera_stream_target"),
                size: wgpu::Extent3d {
                    width: resolution[0],
                    height: resolution[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.render_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let post_input = needs_intermediate.then(|| {
                self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("perro_camera_stream_post_input"),
                    size: wgpu::Extent3d {
                        width: resolution[0],
                        height: resolution[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: self.render_format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                })
            });
            let tonemap_input = needs_tonemap_input.then(|| {
                self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("perro_camera_stream_tonemap_input"),
                    size: wgpu::Extent3d {
                        width: resolution[0],
                        height: resolution[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: self.render_format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
            });
            let depth = needs_post_depth.then(|| {
                self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("perro_camera_stream_post_depth"),
                    size: wgpu::Extent3d {
                        width: resolution[0],
                        height: resolution[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Depth24Plus,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let post_input_view = post_input
                .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()));
            let tonemap_input_view = tonemap_input
                .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()));
            let depth_view =
                depth.map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()));
            self.camera_stream_targets.insert(
                node,
                GpuCameraStreamTarget {
                    texture,
                    view,
                    post_input_view,
                    tonemap_input_view,
                    depth_view,
                    resolution,
                    post_view_key,
                },
            );
            self.camera_stream_external_bindings.remove(&node);
            self.camera_stream_3d_bindings.remove(&node);
            // The old views are retired; release the bind groups built on them.
            self.camera_stream_tonemap.invalidate_bind_cache();
        }
        self.camera_stream_targets.get(&node)
    }
}
