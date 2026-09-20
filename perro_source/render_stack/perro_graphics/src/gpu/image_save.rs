use super::*;
use half::f16;
use perro_graphics_assets::save_rgba_image;

pub(super) struct CameraImageSaveRequest {
    node: NodeID,
    path: String,
    tone_mapped: bool,
}

pub(super) struct DisplayImageSaveRequest {
    path: String,
}

pub(super) struct PendingCameraImageSave {
    buffer: wgpu::Buffer,
    map_tx: Option<mpsc::Sender<Result<(), wgpu::BufferAsyncError>>>,
    rx: mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
    output: ImageSaveOutput,
    width: u32,
    height: u32,
    padded_bytes_per_row: u32,
    format: wgpu::TextureFormat,
    tone_mapped: bool,
}

enum ImageSaveOutput {
    File(String),
    Capture(CaptureFrameCallback),
}

impl ImageSaveOutput {
    fn label(&self) -> &str {
        match self {
            Self::File(path) => path,
            Self::Capture(_) => "<main-capture>",
        }
    }
}

// Keep GPU readback allocations bounded. The request queues retain small path
// records when the GPU is busy, while at most this many mapped buffers stay
// live at once. This avoids a 4K/8K capture turning every frame into another
// large allocation when disk or map callbacks lag.
const MAX_IN_FLIGHT_IMAGE_READBACKS: usize = 4;
const MAX_IMAGE_DIMENSION: u32 = 32_768;

impl Gpu {
    pub fn set_capture_callback(&mut self, callback: Option<CaptureFrameCallback>) {
        self.capture_callback = callback;
    }

    pub fn set_capture_source_node(&mut self, node: Option<NodeID>) {
        self.capture_source_node = node;
    }

    pub fn capture_pending(&self) -> bool {
        self.capture_target.is_some() && self.capture_callback.is_some()
    }

    pub fn take_capture_error(&mut self) -> Option<String> {
        self.capture_error.take()
    }

    pub fn drain_capture(&mut self) -> Result<(), String> {
        while self.pending_readback_buffers() {
            let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
            self.poll_camera_image_saves();
            if let Some(error) = self.capture_error.as_ref() {
                return Err(error.clone());
            }
        }
        Ok(())
    }

    fn pending_readback_buffers(&self) -> bool {
        !self.camera_image_save_pending.is_empty()
    }

    pub(super) fn wait_for_capture_slot(&mut self) {
        while self.camera_image_save_pending.len() >= MAX_IN_FLIGHT_IMAGE_READBACKS {
            let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
            self.poll_camera_image_saves();
        }
    }

    pub fn request_camera_image_save(&mut self, node: NodeID, path: String, tone_mapped: bool) {
        self.camera_image_save_requests
            .push(CameraImageSaveRequest {
                node,
                path,
                tone_mapped,
            });
    }

    pub fn request_display_image_save(&mut self, path: String) -> bool {
        if !self.config.usage.contains(wgpu::TextureUsages::COPY_SRC) {
            eprintln!(
                "[perro] display image save failed path={path} error=surface COPY_SRC unsupported"
            );
            return false;
        }
        self.display_image_save_requests
            .push(DisplayImageSaveRequest { path });
        true
    }

    pub(super) fn poll_camera_image_saves(&mut self) {
        if self.camera_image_save_pending.is_empty() {
            return;
        }
        let _ = self.device.poll(wgpu::PollType::Poll);
        let mut index = 0;
        let mut released_buffer = false;
        while index < self.camera_image_save_pending.len() {
            let result = match self.camera_image_save_pending[index].rx.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    let pending = self.camera_image_save_pending.swap_remove(index);
                    if matches!(&pending.output, ImageSaveOutput::Capture(_)) {
                        self.capture_error
                            .get_or_insert("capture readback callback disconnected".to_owned());
                    }
                    eprintln!(
                        "[perro] texture image readback failed path={} error=callback disconnected",
                        pending.output.label()
                    );
                    pending.buffer.destroy();
                    released_buffer = true;
                    continue;
                }
            };
            let Some(result) = result else {
                index += 1;
                continue;
            };
            let pending = self.camera_image_save_pending.swap_remove(index);
            if let Err(error) = result {
                if matches!(&pending.output, ImageSaveOutput::Capture(_)) {
                    self.capture_error
                        .get_or_insert(format!("capture readback map failed: {error}"));
                }
                eprintln!(
                    "[perro] texture image readback failed path={} error={error}",
                    pending.output.label()
                );
                pending.buffer.destroy();
                released_buffer = true;
                continue;
            }
            let result = read_camera_image_rgba(&pending).and_then(|rgba| match &pending.output {
                ImageSaveOutput::File(path) => {
                    save_rgba_image(&rgba, pending.width, pending.height, path)
                }
                ImageSaveOutput::Capture(callback) => callback(CapturedRgbaFrame {
                    width: pending.width,
                    height: pending.height,
                    rgba,
                }),
            });
            match result {
                Ok(()) => {}
                Err(error) => {
                    if matches!(&pending.output, ImageSaveOutput::Capture(_)) {
                        self.capture_error.get_or_insert(error.clone());
                    }
                    eprintln!(
                        "[perro] texture image save failed path={} error={error}",
                        pending.output.label()
                    );
                }
            }
            pending.buffer.unmap();
            pending.buffer.destroy();
            released_buffer = true;
        }
        if released_buffer {
            let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        }
    }

    pub(super) fn request_camera_image_save_maps(&mut self) {
        for pending in &mut self.camera_image_save_pending {
            let Some(tx) = pending.map_tx.take() else {
                continue;
            };
            pending
                .buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = tx.send(result);
                });
        }
    }

    pub(super) fn encode_camera_image_saves(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let take = bounded_readback_take(
            self.camera_image_save_requests.len(),
            self.camera_image_save_pending.len(),
        );
        for request in self.camera_image_save_requests.drain(..take) {
            let Some(target) = self.camera_stream_targets.get(&request.node) else {
                eprintln!(
                    "[perro] texture image save failed path={} error=camera texture unavailable",
                    request.path
                );
                continue;
            };
            let [width, height] = target.resolution;
            if let Some(pending) = encode_image_save(
                &self.device,
                encoder,
                &target.texture,
                ImageSaveOutput::File(request.path),
                width,
                height,
                self.render_format,
                request.tone_mapped,
                "perro_camera_image_save_readback",
            ) {
                self.camera_image_save_pending.push(pending);
            }
        }
    }

    pub(super) fn encode_display_image_saves(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
    ) {
        let take = bounded_readback_take(
            self.display_image_save_requests.len(),
            self.camera_image_save_pending.len(),
        );
        for request in self.display_image_save_requests.drain(..take) {
            if let Some(pending) = encode_image_save(
                &self.device,
                encoder,
                texture,
                ImageSaveOutput::File(request.path),
                self.config.width,
                self.config.height,
                display_readback_format(self.config.format, self.surface_view_format),
                // HDR display output is linear extended sRGB (scRGB), not PQ;
                // down-map it to SDR for ordinary image formats. SDR surface
                // views have already encoded their display-ready bytes.
                !self.hdr_status.active,
                "perro_display_image_save_readback",
            ) {
                self.camera_image_save_pending.push(pending);
            }
        }
    }

    pub(super) fn encode_capture_frame(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let Some(callback) = self.capture_callback.clone() else {
            return;
        };
        let source_node = self.capture_source_node;
        let dimensions = match source_node {
            Some(node) => self
                .camera_stream_targets
                .get(&node)
                .map(|target| target.resolution),
            None => self.capture_target.as_ref().map(|target| target.size),
        };
        let Some([width, height]) = dimensions else {
            if source_node.is_some() {
                self.capture_error
                    .get_or_insert("capture source stream unavailable".to_owned());
            }
            return;
        };
        self.wait_for_capture_slot();
        let Some((texture, format, tone_mapped)) = (match source_node {
            Some(node) => self.camera_stream_targets.get(&node).map(|target| {
                (
                    target.texture.clone(),
                    capture_readback_format(true, self.render_format, self.surface_view_format),
                    target.tone_mapped,
                )
            }),
            None => self.capture_target.as_ref().map(|target| {
                (
                    target.texture.clone(),
                    capture_readback_format(false, self.render_format, self.surface_view_format),
                    false,
                )
            }),
        }) else {
            if source_node.is_some() {
                self.capture_error
                    .get_or_insert("capture source stream unavailable".to_owned());
            }
            return;
        };
        if let Some(pending) = encode_image_save(
            &self.device,
            encoder,
            &texture,
            ImageSaveOutput::Capture(callback),
            width,
            height,
            format,
            tone_mapped,
            "perro_main_capture_readback",
        ) {
            self.camera_image_save_pending.push(pending);
        }
    }
}

fn bounded_readback_take(queued: usize, pending: usize) -> usize {
    queued.min(MAX_IN_FLIGHT_IMAGE_READBACKS.saturating_sub(pending))
}

fn display_readback_format(
    _surface_format: wgpu::TextureFormat,
    surface_view_format: wgpu::TextureFormat,
) -> wgpu::TextureFormat {
    // Render-target conversion follows the view format. A Bgra8Unorm surface
    // viewed as Bgra8UnormSrgb therefore contains sRGB-encoded copied bytes.
    surface_view_format
}

fn capture_readback_format(
    source_stream: bool,
    render_format: wgpu::TextureFormat,
    surface_view_format: wgpu::TextureFormat,
) -> wgpu::TextureFormat {
    if source_stream {
        render_format
    } else {
        surface_view_format
    }
}

#[allow(clippy::too_many_arguments)]
fn encode_image_save(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    texture: &wgpu::Texture,
    output: ImageSaveOutput,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    tone_mapped: bool,
    label: &'static str,
) -> Option<PendingCameraImageSave> {
    let bytes_per_pixel = match format {
        wgpu::TextureFormat::Rgba8Unorm
        | wgpu::TextureFormat::Rgba8UnormSrgb
        | wgpu::TextureFormat::Bgra8Unorm
        | wgpu::TextureFormat::Bgra8UnormSrgb => 4,
        wgpu::TextureFormat::Rgba16Float => 8,
        format => {
            eprintln!(
                "[perro] image save failed path={} error=unsupported GPU format {format:?}",
                output.label()
            );
            return None;
        }
    };
    let (padded_bytes_per_row, size) = match checked_readback_layout(width, height, bytes_per_pixel)
    {
        Ok(layout) => layout,
        Err(error) => {
            eprintln!(
                "[perro] image save failed path={} error={error}",
                output.label()
            );
            return None;
        }
    };
    if size > device.limits().max_buffer_size {
        eprintln!(
            "[perro] image save failed path={} error=readback buffer {size} exceeds device limit {}",
            output.label(),
            device.limits().max_buffer_size
        );
        return None;
    }
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let (tx, rx) = mpsc::channel();
    Some(PendingCameraImageSave {
        buffer,
        map_tx: Some(tx),
        rx,
        output,
        width,
        height,
        padded_bytes_per_row,
        format,
        tone_mapped,
    })
}

fn checked_readback_layout(
    width: u32,
    height: u32,
    bytes_per_pixel: u32,
) -> Result<(u32, u64), &'static str> {
    if width == 0 || height == 0 || width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
        return Err("invalid image dimensions");
    }
    let unpadded = width
        .checked_mul(bytes_per_pixel)
        .ok_or("image row size overflow")?;
    let padded = unpadded
        .div_ceil(256)
        .checked_mul(256)
        .ok_or("image row alignment overflow")?;
    let size = u64::from(padded)
        .checked_mul(u64::from(height))
        .ok_or("image buffer size overflow")?;
    Ok((padded, size))
}

fn read_camera_image_rgba(pending: &PendingCameraImageSave) -> Result<Vec<u8>, String> {
    let mapped = pending
        .buffer
        .slice(..)
        .get_mapped_range()
        .map_err(|error| format!("map readback failed: {error}"))?;
    let source_bytes_per_pixel = if pending.format == wgpu::TextureFormat::Rgba16Float {
        8
    } else {
        4
    };
    let row_bytes = (pending.width as usize)
        .checked_mul(source_bytes_per_pixel)
        .ok_or_else(|| "readback row size overflow".to_string())?;
    let output_len = (pending.width as usize)
        .checked_mul(pending.height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "readback output size overflow".to_string())?;
    let mut rgba = Vec::with_capacity(output_len);
    let capture_alpha = matches!(&pending.output, ImageSaveOutput::Capture(_));
    for row in 0..pending.height as usize {
        let start = row
            .checked_mul(pending.padded_bytes_per_row as usize)
            .ok_or_else(|| "readback row offset overflow".to_string())?;
        let end = start
            .checked_add(row_bytes)
            .ok_or_else(|| "readback row end overflow".to_string())?;
        let source = mapped
            .get(start..end)
            .ok_or_else(|| "GPU readback returned a short mapped buffer".to_string())?;
        match pending.format {
            wgpu::TextureFormat::Rgba8Unorm => {
                for pixel in source.chunks_exact(4) {
                    rgba.extend_from_slice(&decode_rgba8_pixel(
                        pending.format,
                        pixel,
                        capture_alpha,
                    )?);
                }
            }
            wgpu::TextureFormat::Rgba8UnormSrgb => {
                for pixel in source.chunks_exact(4) {
                    rgba.extend_from_slice(&decode_rgba8_pixel(
                        pending.format,
                        pixel,
                        capture_alpha,
                    )?);
                }
            }
            wgpu::TextureFormat::Bgra8Unorm => {
                for pixel in source.chunks_exact(4) {
                    rgba.extend_from_slice(&decode_rgba8_pixel(
                        pending.format,
                        pixel,
                        capture_alpha,
                    )?);
                }
            }
            wgpu::TextureFormat::Bgra8UnormSrgb => {
                for pixel in source.chunks_exact(4) {
                    rgba.extend_from_slice(&decode_rgba8_pixel(
                        pending.format,
                        pixel,
                        capture_alpha,
                    )?);
                }
            }
            wgpu::TextureFormat::Rgba16Float => {
                for pixel in source.chunks_exact(8) {
                    let alpha = f16::from_bits(u16::from_le_bytes([pixel[6], pixel[7]]))
                        .to_f32()
                        .clamp(0.0, 1.0);
                    for channel in 0..4 {
                        let bits = u16::from_le_bytes([pixel[channel * 2], pixel[channel * 2 + 1]]);
                        let value = f16::from_bits(bits).to_f32();
                        let encoded = if channel == 3 {
                            alpha
                        } else {
                            let straight = if capture_alpha && alpha > 1.0e-6 {
                                value / alpha
                            } else {
                                value
                            };
                            let linear = if pending.tone_mapped {
                                straight.max(0.0)
                            } else {
                                aces_tone_map(straight.max(0.0))
                            };
                            linear_to_srgb(linear)
                        };
                        rgba.push((encoded * 255.0).round() as u8);
                    }
                }
            }
            format => return Err(format!("unsupported GPU format {format:?}")),
        }
    }
    drop(mapped);
    Ok(rgba)
}

fn decode_rgba8_pixel(
    format: wgpu::TextureFormat,
    pixel: &[u8],
    capture_alpha: bool,
) -> Result<[u8; 4], String> {
    let Some(&alpha) = pixel.get(3) else {
        return Err("GPU readback returned a short RGBA8 pixel".to_owned());
    };
    let (r, g, b, srgb) = match format {
        wgpu::TextureFormat::Rgba8Unorm => (pixel[0], pixel[1], pixel[2], false),
        wgpu::TextureFormat::Rgba8UnormSrgb => (pixel[0], pixel[1], pixel[2], true),
        wgpu::TextureFormat::Bgra8Unorm => (pixel[2], pixel[1], pixel[0], false),
        wgpu::TextureFormat::Bgra8UnormSrgb => (pixel[2], pixel[1], pixel[0], true),
        format => return Err(format!("unsupported GPU format {format:?}")),
    };
    Ok([
        capture_unpremultiply_u8(r, alpha, capture_alpha, srgb),
        capture_unpremultiply_u8(g, alpha, capture_alpha, srgb),
        capture_unpremultiply_u8(b, alpha, capture_alpha, srgb),
        alpha,
    ])
}

fn linear_u8_to_srgb(value: u8) -> u8 {
    (linear_to_srgb(f32::from(value) / 255.0) * 255.0).round() as u8
}

fn capture_unpremultiply_u8(value: u8, alpha: u8, capture: bool, srgb: bool) -> u8 {
    if !capture {
        return if srgb {
            value
        } else {
            linear_u8_to_srgb(value)
        };
    }
    if alpha == 0 {
        return 0;
    }
    let a = f32::from(alpha) / 255.0;
    let premultiplied = f32::from(value) / 255.0;
    let linear = if srgb {
        srgb_to_linear(premultiplied)
    } else {
        premultiplied
    };
    (linear_to_srgb((linear / a).clamp(0.0, 1.0)) * 255.0).round() as u8
}

fn aces_tone_map(value: f32) -> f32 {
    ((value * (2.51 * value + 0.03)) / (value * (2.43 * value + 0.59) + 0.14)).clamp(0.0, 1.0)
}

fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        bounded_readback_take, capture_readback_format, capture_unpremultiply_u8,
        checked_readback_layout, decode_rgba8_pixel, display_readback_format,
    };

    #[test]
    fn readback_queue_never_drains_past_queue_or_cap() {
        assert_eq!(bounded_readback_take(0, 0), 0);
        assert_eq!(bounded_readback_take(1, 0), 1);
        assert_eq!(bounded_readback_take(3, 0), 3);
        assert_eq!(bounded_readback_take(9, 0), 4);
        assert_eq!(bounded_readback_take(9, 3), 1);
        assert_eq!(bounded_readback_take(9, 4), 0);
    }

    #[test]
    fn readback_layout_aligns_rows_and_checks_dims() {
        assert_eq!(checked_readback_layout(1, 1, 4), Ok((256, 256)));
        assert_eq!(checked_readback_layout(256, 2, 4), Ok((1024, 2048)));
        assert!(checked_readback_layout(0, 1, 4).is_err());
        assert!(checked_readback_layout(32_769, 1, 4).is_err());
    }

    #[test]
    fn display_readback_uses_srgb_view_encoding() {
        assert_eq!(
            display_readback_format(
                wgpu::TextureFormat::Bgra8Unorm,
                wgpu::TextureFormat::Bgra8UnormSrgb,
            ),
            wgpu::TextureFormat::Bgra8UnormSrgb
        );
    }

    #[test]
    fn display_readback_keeps_linear_hdr_view() {
        assert_eq!(
            display_readback_format(
                wgpu::TextureFormat::Rgba16Float,
                wgpu::TextureFormat::Rgba16Float,
            ),
            wgpu::TextureFormat::Rgba16Float
        );
    }

    #[test]
    fn capture_readback_uses_source_texture_format() {
        assert_eq!(
            capture_readback_format(
                true,
                wgpu::TextureFormat::Rgba16Float,
                wgpu::TextureFormat::Bgra8UnormSrgb,
            ),
            wgpu::TextureFormat::Rgba16Float
        );
        assert_eq!(
            capture_readback_format(
                false,
                wgpu::TextureFormat::Rgba16Float,
                wgpu::TextureFormat::Bgra8UnormSrgb,
            ),
            wgpu::TextureFormat::Bgra8UnormSrgb
        );
    }

    #[test]
    fn capture_alpha_unpremultiplies_linear_rgba() {
        // Linear 0.25 premul / alpha 0.5 -> straight 0.5 -> sRGB 188.
        assert!((i16::from(capture_unpremultiply_u8(64, 128, true, false)) - 188).abs() <= 1);
    }

    #[test]
    fn capture_alpha_unpremultiplies_srgb_bgra() {
        // sRGB 128 premul at alpha 128 encodes near 93; return RGBA order.
        let decoded =
            decode_rgba8_pixel(wgpu::TextureFormat::Bgra8UnormSrgb, &[0, 0, 93, 128], true)
                .expect("valid BGRA8 pixel");
        assert!((i16::from(decoded[0]) - 128).abs() <= 1);
        assert_eq!(&decoded[1..], &[0, 0, 128]);
    }

    #[test]
    fn capture_alpha_zero_cuts_rgb_fringe() {
        let decoded = decode_rgba8_pixel(wgpu::TextureFormat::Bgra8Unorm, &[255, 127, 1, 0], true)
            .expect("valid BGRA8 pixel");
        assert_eq!(decoded, [0, 0, 0, 0]);
    }
}
