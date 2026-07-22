use super::*;

impl Gpu {
    pub fn invalidate_texture(&mut self, texture: perro_ids::TextureID, source: Option<&str>) {
        if let Some(two_d) = self.two_d.as_mut() {
            two_d.invalidate_texture(texture);
        }
        if let Some(late_overlay_2d) = self.late_overlay_2d.as_mut() {
            late_overlay_2d.invalidate_texture(texture);
        }
        if let Some(camera_stream_2d) = self.camera_stream_2d.as_mut() {
            camera_stream_2d.invalidate_texture(texture);
        }
        if let Some(ui) = self.ui.as_mut() {
            ui.invalidate_image_texture(texture);
        }
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.invalidate_material_texture(texture.index());
            three_d.invalidate_material_texture_source(source);
        }
        if let Some(camera_stream_3d) = self.camera_stream_3d.as_mut() {
            camera_stream_3d.invalidate_material_texture(texture.index());
            camera_stream_3d.invalidate_material_texture_source(source);
        }
    }

    // mark/unmark a texture id as a stream (webcam/video) across every consumer
    // cache, so rebuilds use a single-level (no-mip) texture that supports the
    // per-frame in-place base upload.
    pub fn set_stream_texture(&mut self, texture: perro_ids::TextureID, is_stream: bool) {
        if let Some(two_d) = self.two_d.as_mut() {
            two_d.set_stream_texture(texture, is_stream);
        }
        if let Some(late_overlay_2d) = self.late_overlay_2d.as_mut() {
            late_overlay_2d.set_stream_texture(texture, is_stream);
        }
        if let Some(camera_stream_2d) = self.camera_stream_2d.as_mut() {
            camera_stream_2d.set_stream_texture(texture, is_stream);
        }
        if let Some(ui) = self.ui.as_mut() {
            ui.set_stream_texture(texture, is_stream);
        }
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.set_stream_texture(texture.index(), is_stream);
        }
        if let Some(camera_stream_3d) = self.camera_stream_3d.as_mut() {
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
        let queue = &self.queue;
        if let Some(two_d) = self.two_d.as_mut() {
            two_d.write_stream_texture(queue, texture, width, height, rgba);
        }
        if let Some(late_overlay_2d) = self.late_overlay_2d.as_mut() {
            late_overlay_2d.write_stream_texture(queue, texture, width, height, rgba);
        }
        if let Some(camera_stream_2d) = self.camera_stream_2d.as_mut() {
            camera_stream_2d.write_stream_texture(queue, texture, width, height, rgba);
        }
        if let Some(ui) = self.ui.as_mut() {
            ui.write_stream_texture(queue, texture, width, height, rgba);
        }
        if let Some(three_d) = self.three_d.as_mut() {
            three_d.write_stream_material_texture(queue, texture.index(), width, height, rgba);
            three_d.write_stream_material_texture_source(queue, source, width, height, rgba);
        }
        if let Some(camera_stream_3d) = self.camera_stream_3d.as_mut() {
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
    }

    pub(super) fn ensure_camera_stream_target(
        &mut self,
        node: NodeID,
        resolution: [u32; 2],
    ) -> Option<&GpuCameraStreamTarget> {
        let resolution = [resolution[0].max(1), resolution[1].max(1)];
        let recreate = self
            .camera_stream_targets
            .get(&node)
            .is_none_or(|target| target.resolution != resolution);
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
            let post_input = self.device.create_texture(&wgpu::TextureDescriptor {
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
            });
            let depth = self.device.create_texture(&wgpu::TextureDescriptor {
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
            });
            self.camera_stream_targets.insert(
                node,
                GpuCameraStreamTarget {
                    texture,
                    post_input,
                    depth,
                    resolution,
                    post_view_key,
                },
            );
            self.camera_stream_external_bindings.remove(&node);
            self.camera_stream_3d_bindings.remove(&node);
        }
        self.camera_stream_targets.get(&node)
    }
}
