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
    path: String,
    width: u32,
    height: u32,
    padded_bytes_per_row: u32,
    format: wgpu::TextureFormat,
    tone_mapped: bool,
}

impl Gpu {
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
                    eprintln!(
                        "[perro] texture image readback failed path={} error=callback disconnected",
                        pending.path
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
                eprintln!(
                    "[perro] texture image readback failed path={} error={error}",
                    pending.path
                );
                pending.buffer.destroy();
                released_buffer = true;
                continue;
            }
            match read_camera_image_rgba(&pending).and_then(|rgba| {
                save_rgba_image(&rgba, pending.width, pending.height, &pending.path)
            }) {
                Ok(()) => {}
                Err(error) => eprintln!(
                    "[perro] texture image save failed path={} error={error}",
                    pending.path
                ),
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
        for request in self.camera_image_save_requests.drain(..) {
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
                request.path,
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
        for request in self.display_image_save_requests.drain(..) {
            if let Some(pending) = encode_image_save(
                &self.device,
                encoder,
                texture,
                request.path,
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
}

fn display_readback_format(
    _surface_format: wgpu::TextureFormat,
    surface_view_format: wgpu::TextureFormat,
) -> wgpu::TextureFormat {
    // Render-target conversion follows the view format. A Bgra8Unorm surface
    // viewed as Bgra8UnormSrgb therefore contains sRGB-encoded copied bytes.
    surface_view_format
}

#[allow(clippy::too_many_arguments)]
fn encode_image_save(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    texture: &wgpu::Texture,
    path: String,
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
                "[perro] image save failed path={path} error=unsupported GPU format {format:?}"
            );
            return None;
        }
    };
    let unpadded = width.saturating_mul(bytes_per_pixel);
    let padded_bytes_per_row = unpadded.div_ceil(256) * 256;
    let size = u64::from(padded_bytes_per_row) * u64::from(height);
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
        path,
        width,
        height,
        padded_bytes_per_row,
        format,
        tone_mapped,
    })
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
    let row_bytes = pending.width as usize * source_bytes_per_pixel;
    let mut rgba = Vec::with_capacity(pending.width as usize * pending.height as usize * 4);
    for row in 0..pending.height as usize {
        let start = row * pending.padded_bytes_per_row as usize;
        let source = &mapped[start..start + row_bytes];
        match pending.format {
            wgpu::TextureFormat::Rgba8Unorm => {
                for pixel in source.chunks_exact(4) {
                    rgba.extend_from_slice(&[
                        linear_u8_to_srgb(pixel[0]),
                        linear_u8_to_srgb(pixel[1]),
                        linear_u8_to_srgb(pixel[2]),
                        pixel[3],
                    ]);
                }
            }
            wgpu::TextureFormat::Rgba8UnormSrgb => {
                rgba.extend_from_slice(source);
            }
            wgpu::TextureFormat::Bgra8Unorm => {
                for pixel in source.chunks_exact(4) {
                    rgba.extend_from_slice(&[
                        linear_u8_to_srgb(pixel[2]),
                        linear_u8_to_srgb(pixel[1]),
                        linear_u8_to_srgb(pixel[0]),
                        pixel[3],
                    ]);
                }
            }
            wgpu::TextureFormat::Bgra8UnormSrgb => {
                for pixel in source.chunks_exact(4) {
                    rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
                }
            }
            wgpu::TextureFormat::Rgba16Float => {
                for pixel in source.chunks_exact(8) {
                    for channel in 0..4 {
                        let bits = u16::from_le_bytes([pixel[channel * 2], pixel[channel * 2 + 1]]);
                        let value = f16::from_bits(bits).to_f32();
                        let encoded = if channel == 3 {
                            value.clamp(0.0, 1.0)
                        } else {
                            let linear = if pending.tone_mapped {
                                value.max(0.0)
                            } else {
                                aces_tone_map(value.max(0.0))
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

fn linear_u8_to_srgb(value: u8) -> u8 {
    (linear_to_srgb(f32::from(value) / 255.0) * 255.0).round() as u8
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

#[cfg(test)]
mod tests {
    use super::display_readback_format;

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
}
