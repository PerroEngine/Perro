//! Pixel checks for display-space overlays after scene presentation.
use super::*;
use crate::{resources::DecodedTextureRgba, two_d::gpu::Prepare2D};
use std::sync::Arc;

const SIZE: [u32; 2] = [64, 32];
const LINEAR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

async fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let adapter = wgpu::Instance::default()
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        })
        .await
        .ok()?;
    adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("display overlay test"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        })
        .await
        .ok()
}

fn texture(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("display overlay target"),
        size: wgpu::Extent3d {
            width: SIZE[0],
            height: SIZE[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

fn clear(encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, color: [f64; 3]) {
    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("display overlay scene clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: color[0],
                    g: color[1],
                    b: color[2],
                    a: 1.0,
                }),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
}

fn read(device: &wgpu::Device, queue: &wgpu::Queue, output: &wgpu::Texture) -> Vec<u8> {
    let pixel_bytes = output.format().block_copy_size(None).expect("copy size");
    let stride = (SIZE[0] * pixel_bytes).div_ceil(256) * 256;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("display overlay readback"),
        size: u64::from(stride * SIZE[1]),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        output.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(SIZE[1]),
            },
        },
        output.size(),
    );
    queue.submit([encoder.finish()]);
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).expect("map send");
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    rx.recv().expect("map callback").expect("map output");
    let bytes = slice.get_mapped_range().expect("mapped output");
    let result = bytes
        .chunks(stride as usize)
        .flat_map(|row| row[..(SIZE[0] * pixel_bytes) as usize].iter().copied())
        .collect();
    drop(bytes);
    staging.unmap();
    result
}

fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    output_format: wgpu::TextureFormat,
    hdr: HdrStatus,
) -> Vec<u8> {
    let scene = texture(
        device,
        LINEAR,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    );
    let output = texture(
        device,
        output_format,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let scene_view = scene.create_view(&Default::default());
    let output_view = output.create_view(&Default::default());
    let mut present = PresentProcessor::new(device, output_format);
    present.set_output_size(SIZE[0], SIZE[1]);
    let present_input = present.create_bind_group(device, &scene_view);

    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="3" height="1"><rect width="1" height="1" fill="#ff4000"/><rect x="1" width="1" height="1" fill="#ff4000" fill-opacity="0.5"/><rect x="2" width="1" height="1" fill="white"/></svg>"##;
    let (rgba, width, height) = perro_graphics_assets::decode_image_rgba(svg).expect("SVG");
    let mut resources = ResourceStore::new();
    let texture_id = resources.create_texture("res://display-overlay.svg", true);
    assert!(resources.set_decoded_texture_data(
        texture_id,
        DecodedTextureRgba {
            rgba: Arc::from(rgba),
            width,
            height,
        },
    ));
    let sprite = Sprite2DCommand {
        texture: texture_id,
        size: [48.0, 16.0],
        ..Sprite2DCommand::default()
    };
    let mut shared = SharedTextureStore::default();
    let mut overlay = Gpu2D::new(device, output_format, 1, TextureFilterMode::Nearest);
    overlay.prepare(
        device,
        queue,
        Prepare2D {
            resources: &resources,
            shared_textures: &mut shared,
            camera: Camera2DUniform {
                view: Mat4::IDENTITY.to_cols_array_2d(),
                ndc_scale: [2.0 / SIZE[0] as f32, 2.0 / SIZE[1] as f32],
                pad: [0.0; 2],
            },
            rects: &[],
            upload: &RectUploadPlan {
                full_reupload: true,
                dirty_ranges: Vec::new(),
                draw_count: 0,
            },
            sprites: &[sprite],
            sprites_revision: 1,
            force_sprite_prepare: true,
            point_lights: &[],
            point_lights_revision: 0,
            shadow_casters: &[],
            shadow_casters_revision: 0,
            static_texture_lookup: None,
        },
    );

    let mut encoder = device.create_command_encoder(&Default::default());
    clear(&mut encoder, &scene_view, [4.0, 2.0, 1.0]);
    present.apply(
        queue,
        &mut encoder,
        &present_input,
        &output_view,
        SIZE,
        1.0 / 60.0,
        PresentExposureSettings::default(),
        hdr,
        None,
    );
    overlay.render_pass(&mut encoder, &output_view, None, 0);
    queue.submit([encoder.finish()]);
    read(device, queue, &output)
}

fn rgba8(bytes: &[u8], x: usize, y: usize) -> [u8; 4] {
    bytes[(y * SIZE[0] as usize + x) * 4..][..4]
        .try_into()
        .expect("complete RGBA pixel")
}

fn rgba16f(bytes: &[u8], x: usize, y: usize) -> [f32; 4] {
    let at = (y * SIZE[0] as usize + x) * 8;
    std::array::from_fn(|channel| {
        let offset = at + channel * 2;
        half::f16::from_le_bytes([bytes[offset], bytes[offset + 1]]).to_f32()
    })
}

fn srgb_to_linear(value: u8) -> f32 {
    let value = f32::from(value) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(value: f32) -> u8 {
    let value = if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[test]
fn sdr_display_overlay_keeps_svg_color_and_straight_alpha() {
    pollster::block_on(async {
        let Some((device, queue)) = device().await else {
            eprintln!("skip display overlay test: no adapter");
            return;
        };
        let bytes = render(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            HdrStatus::default(),
        );
        let scene = rgba8(&bytes, 4, 4);
        let opaque = rgba8(&bytes, 16, 16);
        let translucent = rgba8(&bytes, 32, 16);
        assert!(
            opaque[0] >= 254 && opaque[1].abs_diff(64) <= 1 && opaque[2] <= 1,
            "{opaque:?}"
        );
        assert_eq!(opaque[3], 255);
        for channel in 0..3 {
            let expected = linear_to_srgb(
                (srgb_to_linear(opaque[channel]) + srgb_to_linear(scene[channel])) * 0.5,
            );
            assert!(
                translucent[channel].abs_diff(expected) <= 2,
                "scene={scene:?} opaque={opaque:?} translucent={translucent:?}"
            );
        }
        assert_eq!(translucent[3], 255);
    });
}

#[test]
fn hdr_display_overlay_uses_unit_white_and_keeps_scene_headroom() {
    pollster::block_on(async {
        let Some((device, queue)) = device().await else {
            eprintln!("skip HDR display overlay test: no adapter");
            return;
        };
        let bytes = render(
            &device,
            &queue,
            LINEAR,
            HdrStatus {
                active: true,
                headroom: 4.0,
                ..HdrStatus::default()
            },
        );
        let scene = rgba16f(&bytes, 4, 4);
        let opaque = rgba16f(&bytes, 16, 16);
        let translucent = rgba16f(&bytes, 32, 16);
        let white = rgba16f(&bytes, 48, 16);
        assert!(scene[0] > 1.0, "HDR scene lost headroom: {scene:?}");
        assert!(
            (opaque[0] - 1.0).abs() <= 0.002,
            "opaque red must use unit display white: {opaque:?}"
        );
        assert!(
            (opaque[1] - 0.0513).abs() <= 0.003 && opaque[2].abs() <= 0.002,
            "{opaque:?}"
        );
        assert!(
            white[..3]
                .iter()
                .all(|channel| (*channel - 1.0).abs() <= 0.002),
            "display white must stay 1.0 in extended-linear output: {white:?}"
        );
        for channel in 0..3 {
            let expected = (opaque[channel] + scene[channel]) * 0.5;
            assert!(
                (translucent[channel] - expected).abs() <= 0.01,
                "scene={scene:?} opaque={opaque:?} translucent={translucent:?}"
            );
        }
    });
}
