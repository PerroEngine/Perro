//! Retained UI supersample raster + UI GC decay.
//!
//! Runs the real `GpuUi::prepare` / `GpuUi::render_pass` against a headless
//! wgpu device; skipped with a note when no adapter is available.
use super::*;
use crate::gpu_shrink::SHRINK_LOW_TICKS;
use epaint::{Color32, Mesh, Rect, Vertex, pos2};

const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const VIEWPORT: [u32; 2] = [128, 96];

async fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::default();
    let adapter = instance
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
            label: Some("perro_ui_test_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        })
        .await
        .ok()
}

fn output_view(device: &wgpu::Device) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_ui_test_output"),
        size: wgpu::Extent3d {
            width: VIEWPORT[0],
            height: VIEWPORT[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OUTPUT_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn test_vertex(x: f32, y: f32) -> Vertex {
    Vertex {
        pos: pos2(x, y),
        uv: pos2(0.0, 0.0),
        color: Color32::WHITE,
    }
}

fn quad(x: f32) -> Arc<ClippedPrimitive> {
    quad_with_texture(x, TextureId::default())
}

fn quad_with_texture(x: f32, texture: TextureId) -> Arc<ClippedPrimitive> {
    let mut mesh = Mesh::with_texture(texture);
    mesh.vertices = vec![
        test_vertex(x, 0.0),
        test_vertex(x + 8.0, 0.0),
        test_vertex(x + 8.0, 8.0),
    ];
    mesh.indices = vec![0, 1, 2];
    Arc::new(ClippedPrimitive {
        clip_rect: Rect::from_min_max(pos2(0.0, 0.0), pos2(64.0, 64.0)),
        primitive: Primitive::Mesh(mesh),
    })
}

fn external_view(device: &wgpu::Device) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_ui_test_external"),
        size: wgpu::Extent3d {
            width: 8,
            height: 8,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OUTPUT_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn rgba_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    rgba: [u8; 4],
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ui alpha test user texture"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&Default::default());
    (texture, view)
}

fn colored_rect(
    min_x: f32,
    max_x: f32,
    texture: TextureId,
    color: Color32,
) -> Arc<ClippedPrimitive> {
    let mut mesh = Mesh::with_texture(texture);
    mesh.vertices = vec![
        Vertex {
            pos: pos2(min_x, 8.0),
            uv: pos2(0.0, 0.0),
            color,
        },
        Vertex {
            pos: pos2(max_x, 8.0),
            uv: pos2(1.0, 0.0),
            color,
        },
        Vertex {
            pos: pos2(max_x, 40.0),
            uv: pos2(1.0, 1.0),
            color,
        },
        Vertex {
            pos: pos2(min_x, 40.0),
            uv: pos2(0.0, 1.0),
            color,
        },
    ];
    mesh.indices = vec![0, 1, 2, 0, 2, 3];
    Arc::new(ClippedPrimitive {
        clip_rect: Rect::EVERYTHING,
        primitive: Primitive::Mesh(mesh),
    })
}

fn render_ui_pixels(
    ui: &mut GpuUi,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    primitives: &[Arc<ClippedPrimitive>],
    textures_delta: &TexturesDelta,
) -> Vec<u8> {
    render_ui_pixels_at(ui, device, queue, primitives, textures_delta, 1, VIEWPORT)
}

fn render_ui_pixels_at(
    ui: &mut GpuUi,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    primitives: &[Arc<ClippedPrimitive>],
    textures_delta: &TexturesDelta,
    revision: u64,
    viewport: [u32; 2],
) -> Vec<u8> {
    let output = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ui alpha test output"),
        size: wgpu::Extent3d {
            width: viewport[0],
            height: viewport[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OUTPUT_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let output_view = output.create_view(&Default::default());
    let mut shared_textures = SharedTextureStore::default();
    ui.prepare(
        device,
        queue,
        UiPrepareInput {
            resources: &ResourceStore::new(),
            shared_textures: &mut shared_textures,
            viewport,
            primitives,
            world_projections: &[],
            textures_delta,
            texture_size: [1, 1],
            revision,
            static_texture_lookup: None,
        },
    );
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear ui alpha test output"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &output_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
    }
    ui.render_pass(device, &mut encoder, &output_view, viewport, None);
    let bytes_per_row = viewport[0] * 4;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ui alpha test readback"),
        size: u64::from(bytes_per_row * viewport[1]),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        output.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(viewport[1]),
            },
        },
        wgpu::Extent3d {
            width: viewport[0],
            height: viewport[1],
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).expect("GPU test map channel");
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("GPU test poll");
    rx.recv().expect("GPU test map result").expect("GPU map");
    slice.get_mapped_range().expect("GPU mapped bytes").to_vec()
}

fn pixel(bytes: &[u8], x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * VIEWPORT[0] + x) * 4) as usize;
    bytes[offset..offset + 4].try_into().expect("RGBA pixel")
}

#[test]
fn user_and_managed_translucency_match_without_double_alpha() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip UI alpha pixel test: no wgpu adapter");
            return;
        };
        let mut ui = GpuUi::new(&device, OUTPUT_FORMAT, TextureFilterMode::Linear);
        let user_key = TextureID::from_u64(912);
        let (_user_texture, user_view) = rgba_texture(&device, &queue, [128, 0, 0, 128]);
        ui.upsert_external_image_texture(&device, user_key, user_view, [1, 1]);

        let tint = Color32::from_rgba_unmultiplied(255, 255, 255, 128);
        let primitives = [
            colored_rect(8.0, 56.0, TextureId::default(), tint),
            colored_rect(72.0, 120.0, TextureId::User(user_key.as_u64()), tint),
        ];
        let mut delta = TexturesDelta::default();
        delta.set.push((
            TextureId::default(),
            epaint::ImageDelta::full(
                epaint::ColorImage::new(
                    [1, 1],
                    vec![Color32::from_rgba_unmultiplied(128, 0, 0, 128)],
                ),
                epaint::textures::TextureOptions::LINEAR,
            ),
        ));

        let bytes = render_ui_pixels(&mut ui, &device, &queue, &primitives, &delta);
        let managed = pixel(&bytes, 32, 24);
        let user = pixel(&bytes, 96, 24);
        assert_eq!(pixel(&bytes, 0, 0), [0, 0, 0, 0]);
        for channel in 0..4 {
            assert!(
                managed[channel].abs_diff(user[channel]) <= 2,
                "managed {managed:?}, user {user:?}"
            );
        }
        assert!(
            (62..=68).contains(&managed[0]) && (62..=68).contains(&managed[3]),
            "expected two half-alpha factors once each: {managed:?}"
        );
        assert_eq!([managed[1], managed[2]], [0, 0]);
    });
}

#[test]
fn opaque_gray_matches_source_for_user_and_managed_textures() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip UI opaque color pixel test: no wgpu adapter");
            return;
        };
        let mut ui = GpuUi::new(&device, OUTPUT_FORMAT, TextureFilterMode::Linear);
        let user_key = TextureID::from_u64(913);
        let (_user_texture, user_view) = rgba_texture(&device, &queue, [128, 128, 128, 255]);
        ui.upsert_external_image_texture(&device, user_key, user_view, [1, 1]);
        let primitives = [
            colored_rect(8.0, 56.0, TextureId::default(), Color32::WHITE),
            colored_rect(
                72.0,
                120.0,
                TextureId::User(user_key.as_u64()),
                Color32::WHITE,
            ),
        ];
        let mut delta = TexturesDelta::default();
        delta.set.push((
            TextureId::default(),
            epaint::ImageDelta::full(
                epaint::ColorImage::new([1, 1], vec![Color32::from_gray(128)]),
                epaint::textures::TextureOptions::LINEAR,
            ),
        ));

        let bytes = render_ui_pixels(&mut ui, &device, &queue, &primitives, &delta);
        let managed = pixel(&bytes, 32, 24);
        let user = pixel(&bytes, 96, 24);
        assert_eq!(managed, [128, 128, 128, 255]);
        assert_eq!(user, [128, 128, 128, 255]);
    });
}

fn cycle(
    ui: &mut GpuUi,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    view: &wgpu::TextureView,
    primitives: &[Arc<ClippedPrimitive>],
    revision: u64,
) {
    cycle_at(ui, device, queue, view, primitives, revision, VIEWPORT);
}

fn cycle_at(
    ui: &mut GpuUi,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    view: &wgpu::TextureView,
    primitives: &[Arc<ClippedPrimitive>],
    revision: u64,
    viewport: [u32; 2],
) {
    let resources = ResourceStore::new();
    let mut shared_textures = SharedTextureStore::default();
    let textures_delta = TexturesDelta::default();
    ui.prepare(
        device,
        queue,
        UiPrepareInput {
            resources: &resources,
            shared_textures: &mut shared_textures,
            viewport,
            primitives,
            world_projections: &[],
            textures_delta: &textures_delta,
            texture_size: [0, 0],
            revision,
            static_texture_lookup: None,
        },
    );
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("perro_ui_test_encoder"),
    });
    ui.render_pass(device, &mut encoder, view, viewport, None);
    queue.submit(Some(encoder.finish()));
}

#[test]
fn unchanged_ui_skips_the_supersample_raster() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip ui retained raster test: no wgpu adapter");
            return;
        };
        let mut ui = GpuUi::new(&device, OUTPUT_FORMAT, TextureFilterMode::Linear);
        let view = output_view(&device);
        let primitives = [quad(0.0), quad(16.0)];

        cycle(&mut ui, &device, &queue, &view, &primitives, 1);
        assert_eq!(ui.ui_supersample_redraws(), 1);
        assert_eq!(ui.ui_supersample_composites(), 1);

        // Same primitives, new revision: the geometry rebuild is already
        // skipped by the signature; the raster must be skipped too.
        cycle(&mut ui, &device, &queue, &view, &primitives, 2);
        assert_eq!(ui.ui_supersample_redraws(), 1);
        assert_eq!(ui.ui_supersample_composites(), 2);
    });
}

#[test]
fn world_glyph_gpu_preserves_perspective_clipping_and_scene_depth() {
    check_world_glyph_target(OUTPUT_FORMAT);
}

#[test]
fn world_glyph_gpu_renders_into_linear_sub_view_target() {
    check_world_glyph_target(wgpu::TextureFormat::Rgba16Float);
}

fn check_world_glyph_target(output_format: wgpu::TextureFormat) {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip world glyph raster test: no wgpu adapter");
            return;
        };
        let mut ui = GpuUi::new(&device, output_format, TextureFilterMode::Linear);
        let size = wgpu::Extent3d {
            width: VIEWPORT[0],
            height: VIEWPORT[1],
            depth_or_array_layers: 1,
        };
        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("world glyph test output"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: output_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = output.create_view(&Default::default());
        let scene_depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("world glyph scene depth"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth_view = scene_depth.create_view(&Default::default());
        let mut primitive = (*quad(0.0)).clone();
        primitive.clip_rect = Rect::EVERYTHING;
        if let Primitive::Mesh(mesh) = &mut primitive.primitive {
            mesh.vertices[0].uv = pos2(0.1, 0.5);
            mesh.vertices[1].uv = pos2(0.9, 0.5);
            mesh.vertices[2].uv = pos2(0.1, 0.5);
        }
        let primitives = [Arc::new(primitive)];
        let mut delta = TexturesDelta::default();
        delta.set.push((
            TextureId::default(),
            epaint::ImageDelta::full(
                epaint::ColorImage::new(
                    [64, 1],
                    (0..64)
                        .map(|x| if x < 32 { Color32::RED } else { Color32::GREEN })
                        .collect(),
                ),
                epaint::textures::TextureOptions::NEAREST,
            ),
        ));
        let mut shared_textures = SharedTextureStore::default();
        // At pixel (80,20), affine UV interpolation selects green; true
        // perspective interpolation selects red. Reuse mesh Arcs to also
        // test projection-only changes with depth testing disabled.
        let cases = [
            (0.5, false, true),   // visible through the surface at depth 0.4
            (-0.1, false, false), // before near
            (1.1, false, false),  // beyond far
            (0.5, true, false),   // behind scene surface
            (0.3, true, true),    // in front of scene surface
        ];
        for (revision, (depth, depth_test, visible)) in cases.into_iter().enumerate() {
            let clips = [
                [-0.8, 0.8, depth, 1.0],
                [3.2, 3.2, depth * 4.0, 4.0],
                [-0.8, -0.8, depth, 1.0],
            ];
            let projections = [Some(crate::ui::painter::UiWorldProjection {
                clip_positions: Arc::from(clips),
                depth_test,
            })];
            ui.prepare(
                &device,
                &queue,
                UiPrepareInput {
                    resources: &ResourceStore::new(),
                    shared_textures: &mut shared_textures,
                    viewport: VIEWPORT,
                    primitives: &primitives,
                    world_projections: &projections,
                    textures_delta: &delta,
                    texture_size: [64, 1],
                    revision: revision as u64,
                    static_texture_lookup: None,
                },
            );
            delta.clear();
            assert_eq!(ui.vertices[1].pos, clips[1]);
            let mut encoder = device.create_command_encoder(&Default::default());
            {
                let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("clear world glyph test"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(0.4),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    ..Default::default()
                });
            }
            ui.render_pass(
                &device,
                &mut encoder,
                &view,
                VIEWPORT,
                Some((&depth_view, 1)),
            );
            let bytes_per_pixel = if output_format == wgpu::TextureFormat::Rgba16Float {
                8
            } else {
                4
            };
            let bytes_per_row = VIEWPORT[0] * bytes_per_pixel;
            let staging = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("world glyph readback"),
                size: u64::from(bytes_per_row * VIEWPORT[1]),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &output,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &staging,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_row),
                        rows_per_image: Some(VIEWPORT[1]),
                    },
                },
                size,
            );
            queue.submit([encoder.finish()]);
            let slice = staging.slice(..);
            let (tx, rx) = std::sync::mpsc::channel();
            slice.map_async(wgpu::MapMode::Read, move |result| {
                tx.send(result).expect("GPU test setup must succeed");
            });
            device
                .poll(wgpu::PollType::wait_indefinitely())
                .expect("GPU test setup must succeed");
            rx.recv()
                .expect("GPU test setup must succeed")
                .expect("GPU test setup must succeed");
            let bytes = slice
                .get_mapped_range()
                .expect("GPU test setup must succeed");
            let offset = ((20 * VIEWPORT[0] + 80) * bytes_per_pixel) as usize;
            let raw = &bytes[offset..offset + bytes_per_pixel as usize];
            let pixel: Vec<u16> = if bytes_per_pixel == 8 {
                raw.chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect()
            } else {
                raw.iter().map(|v| u16::from(*v)).collect()
            };
            if visible {
                assert!(
                    pixel[0] > 200 && pixel[1] < 40 && pixel[3] > 200,
                    "perspective pixel: {pixel:?}"
                );
            } else {
                assert_eq!(pixel, [0, 0, 0, 0], "clip depth {depth}");
            }
            drop(bytes);
            staging.unmap();
        }
    });
}

#[test]
fn sparse_ui_patch_matches_full_geometry_and_falls_back_on_layout_changes() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip sparse UI patch test: no wgpu adapter");
            return;
        };
        let mut ui = GpuUi::new(&device, OUTPUT_FORMAT, TextureFilterMode::Linear);
        let view = output_view(&device);
        let mut primitives: Vec<_> = (0..64).map(|i| quad((i % 8) as f32 * 8.0)).collect();
        cycle(&mut ui, &device, &queue, &view, &primitives, 1);
        let full_bytes = ui.perf_counters.mesh_upload_bytes;
        primitives[17] = quad(3.0);
        cycle(&mut ui, &device, &queue, &view, &primitives, 2);
        assert_eq!(ui.perf_counters.patched_primitives, 1);
        assert_eq!(ui.perf_counters.mesh_upload_calls, 2);
        assert_eq!(ui.perf_counters.mesh_upload_bytes * 64, full_bytes);
        assert_eq!(ui.ui_supersample_redraws(), 2);
        let mut reference = GpuUi::new(&device, OUTPUT_FORMAT, TextureFilterMode::Linear);
        cycle(&mut reference, &device, &queue, &view, &primitives, 2);
        assert_eq!(
            bytemuck::cast_slice::<_, u8>(&ui.vertices),
            bytemuck::cast_slice::<_, u8>(&reference.vertices)
        );
        assert_eq!(ui.indices, reference.indices);
        assert_eq!(ui.meshes, reference.meshes);

        // Adjacent changed primitives coalesce into one pair of queue writes.
        primitives[17] = quad(4.0);
        primitives[18] = quad(5.0);
        cycle(&mut ui, &device, &queue, &view, &primitives, 3);
        assert_eq!(ui.perf_counters.patched_primitives, 2);
        assert_eq!(ui.perf_counters.mesh_upload_calls, 2);

        let mut changed_clip = (*quad(6.0)).clone();
        changed_clip.clip_rect.max.x = 32.0;
        primitives[17] = Arc::new(changed_clip);
        cycle(&mut ui, &device, &queue, &view, &primitives, 4);
        assert_eq!(ui.perf_counters.patched_primitives, 0);
        assert_eq!(ui.perf_counters.mesh_upload_bytes, full_bytes);
        primitives.push(quad(7.0));
        cycle(&mut ui, &device, &queue, &view, &primitives, 5);
        assert_eq!(ui.perf_counters.patched_primitives, 0);
        assert!(ui.perf_counters.mesh_upload_bytes > full_bytes);

        // A source Arc change plus projected depth must use the full packer.
        primitives[17] = quad(7.5);
        let mut depths = vec![None; primitives.len()];
        depths[17] = Some(crate::ui::painter::UiWorldProjection {
            clip_positions: Arc::from([[0.0, 0.0, 0.25, 1.0]; 3]),
            depth_test: true,
        });
        let resources = ResourceStore::new();
        let mut shared_textures = SharedTextureStore::default();
        ui.prepare(
            &device,
            &queue,
            UiPrepareInput {
                resources: &resources,
                shared_textures: &mut shared_textures,
                viewport: VIEWPORT,
                primitives: &primitives,
                world_projections: &depths,
                textures_delta: &TexturesDelta::default(),
                texture_size: [0, 0],
                revision: 6,
                static_texture_lookup: None,
            },
        );
        assert_eq!(ui.perf_counters.patched_primitives, 0);
        assert!(ui.prepared_uses_depth_test);
        assert_eq!(ui.vertices[17 * 3].depth_test, [1.0, 1.0]);
        assert_eq!(ui.vertices[17 * 3].pos, [0.0, 0.0, 0.25, 1.0]);

        // Camera motion keeps the retained UI revision, mesh Arcs and topology
        // stable: patch projected verts only and skip the index upload.
        let mut moved_depths = vec![None; primitives.len()];
        moved_depths[17] = Some(crate::ui::painter::UiWorldProjection {
            clip_positions: Arc::from([[0.1, 0.0, 0.5, 1.0]; 3]),
            depth_test: true,
        });
        ui.prepare(
            &device,
            &queue,
            UiPrepareInput {
                resources: &resources,
                shared_textures: &mut shared_textures,
                viewport: VIEWPORT,
                primitives: &primitives,
                world_projections: &moved_depths,
                textures_delta: &TexturesDelta::default(),
                texture_size: [0, 0],
                revision: 6,
                static_texture_lookup: None,
            },
        );
        assert_eq!(ui.perf_counters.patched_primitives, 1);
        assert_eq!(ui.perf_counters.mesh_upload_calls, 1);
        assert_eq!(
            ui.perf_counters.mesh_upload_bytes,
            3 * std::mem::size_of::<UiVertexGpu>()
        );
        assert_eq!(ui.vertices[17 * 3].pos, [0.1, 0.0, 0.5, 1.0]);
    });
}

#[test]
fn sparse_ui_resize_under_pixel_cap_rebuilds_all_scaled_vertices() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip capped UI resize test: no wgpu adapter");
            return;
        };
        let mut ui = GpuUi::new(&device, OUTPUT_FORMAT, TextureFilterMode::Linear);
        ui.set_max_render_pixels(u64::from(VIEWPORT[0] * VIEWPORT[1]));
        let view = output_view(&device);
        let mut primitives: Vec<_> = (0..64).map(|i| quad((i % 8) as f32 * 8.0)).collect();
        cycle(&mut ui, &device, &queue, &view, &primitives, 1);
        let old_render_size = ui.prepared_render_viewport;
        let old_position = ui.vertices[6].pos;
        primitives[17] = quad(3.0);
        let resources = ResourceStore::new();
        let mut shared_textures = SharedTextureStore::default();
        ui.prepare(
            &device,
            &queue,
            UiPrepareInput {
                resources: &resources,
                shared_textures: &mut shared_textures,
                viewport: [VIEWPORT[0] * 2, VIEWPORT[1] * 2],
                primitives: &primitives,
                world_projections: &[],
                textures_delta: &TexturesDelta::default(),
                texture_size: [0, 0],
                revision: 2,
                static_texture_lookup: None,
            },
        );
        assert_eq!(
            ui.prepared_render_viewport, old_render_size,
            "test must keep same capped render target"
        );
        assert_eq!(ui.perf_counters.patched_primitives, 0);
        assert_eq!(
            ui.vertices[6].pos,
            [old_position[0] * 0.5, old_position[1] * 0.5, 0.0, 1.0]
        );
        assert_eq!(
            ui.perf_counters.mesh_upload_bytes,
            64 * (3 * std::mem::size_of::<UiVertexGpu>() + 3 * std::mem::size_of::<u32>())
        );
    });
}

#[test]
fn idle_external_camera_target_skips_raster_until_written() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip ui external target retention test: no wgpu adapter");
            return;
        };
        let mut ui = GpuUi::new(&device, OUTPUT_FORMAT, TextureFilterMode::Linear);
        let view = output_view(&device);
        let texture = perro_ids::TextureID::from_u64(91);
        ui.upsert_external_image_texture(&device, texture, external_view(&device), [8, 8]);
        let primitives = [quad_with_texture(0.0, TextureId::User(texture.as_u64()))];

        cycle(&mut ui, &device, &queue, &view, &primitives, 1);
        assert_eq!(ui.ui_supersample_redraws(), 1);

        cycle(&mut ui, &device, &queue, &view, &primitives, 2);
        assert_eq!(ui.ui_supersample_redraws(), 1);

        ui.note_live_texture_write(texture);
        cycle(&mut ui, &device, &queue, &view, &primitives, 3);
        assert_eq!(ui.ui_supersample_redraws(), 2);
    });
}

#[test]
fn changed_ui_redraws_the_supersample_raster() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip ui raster invalidation test: no wgpu adapter");
            return;
        };
        let mut ui = GpuUi::new(&device, OUTPUT_FORMAT, TextureFilterMode::Linear);
        let view = output_view(&device);

        cycle(&mut ui, &device, &queue, &view, &[quad(0.0)], 1);
        assert_eq!(ui.ui_supersample_redraws(), 1);

        // Re-tessellated content: a fresh primitive Arc moves the signature.
        cycle(&mut ui, &device, &queue, &view, &[quad(24.0)], 2);
        assert_eq!(ui.ui_supersample_redraws(), 2);

        // A texture delta mutates atlas pixels under an unchanged signature.
        let unchanged = quad(24.0);
        let unchanged = std::slice::from_ref(&unchanged);
        cycle(&mut ui, &device, &queue, &view, unchanged, 3);
        let retained = ui.ui_supersample_redraws();
        ui.invalidate_image_texture(perro_ids::TextureID::from_u64(7));
        cycle(&mut ui, &device, &queue, &view, unchanged, 4);
        assert_eq!(ui.ui_supersample_redraws(), retained + 1);
    });
}

#[test]
fn one_mesh_topology_change_redraws_dirty_tiles_only() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip UI dirty tile test: no wgpu adapter");
            return;
        };
        let viewport = [1024, 768];
        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("perro_ui_dirty_tile_output"),
            size: wgpu::Extent3d {
                width: viewport[0],
                height: viewport[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OUTPUT_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = output.create_view(&Default::default());
        let mut ui = GpuUi::new(&device, OUTPUT_FORMAT, TextureFilterMode::Linear);
        let mut primitives: Vec<_> = (0..64)
            .map(|index| quad((index % 8) as f32 * 12.0))
            .collect();

        cycle_at(&mut ui, &device, &queue, &view, &primitives, 1, viewport);
        assert_eq!(ui.ui_partial_redraws(), 0);

        // Model a label whose glyph count changes: primitive count stays one,
        // but vertex/index topology grows and sparse buffer patching falls back.
        primitives[17] = colored_rect(12.0, 36.0, TextureId::default(), Color32::WHITE);
        cycle_at(&mut ui, &device, &queue, &view, &primitives, 2, viewport);

        assert_eq!(ui.perf_counters.patched_primitives, 0);
        assert_eq!(ui.ui_partial_redraws(), 1);
        assert!(ui.dirty_tiles.is_empty(), "render must consume dirty tiles");
    });
}

#[test]
fn dirty_tile_pixels_match_full_raster_with_interleaved_z() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip UI dirty tile pixel test: no wgpu adapter");
            return;
        };
        let viewport = [1024, 768];
        let mut atlas = TexturesDelta::default();
        atlas.set.push((
            TextureId::default(),
            epaint::ImageDelta::full(
                epaint::ColorImage::new([1, 1], vec![Color32::WHITE]),
                epaint::textures::TextureOptions::NEAREST,
            ),
        ));
        let initial = vec![
            colored_rect(
                0.0,
                700.0,
                TextureId::default(),
                Color32::from_rgb(20, 30, 80),
            ),
            colored_rect(12.0, 44.0, TextureId::default(), Color32::RED),
            colored_rect(32.0, 64.0, TextureId::default(), Color32::GREEN),
        ];
        let changed = vec![
            initial[0].clone(),
            colored_rect(20.0, 52.0, TextureId::default(), Color32::BLUE),
            initial[2].clone(),
        ];

        let mut partial = GpuUi::new(&device, OUTPUT_FORMAT, TextureFilterMode::Linear);
        render_ui_pixels_at(&mut partial, &device, &queue, &initial, &atlas, 1, viewport);
        let partial_pixels = render_ui_pixels_at(
            &mut partial,
            &device,
            &queue,
            &changed,
            &TexturesDelta::default(),
            2,
            viewport,
        );
        assert_eq!(partial.ui_partial_redraws(), 1);

        let mut full = GpuUi::new(&device, OUTPUT_FORMAT, TextureFilterMode::Linear);
        let full_pixels =
            render_ui_pixels_at(&mut full, &device, &queue, &changed, &atlas, 2, viewport);

        assert_eq!(partial_pixels, full_pixels);
    });
}

#[test]
fn shrink_tick_decays_buffers_and_releases_the_target() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip ui shrink tick test: no wgpu adapter");
            return;
        };
        let mut ui = GpuUi::new(&device, OUTPUT_FORMAT, TextureFilterMode::Linear);
        let view = output_view(&device);
        let heavy: Vec<Arc<ClippedPrimitive>> =
            (0..400).map(|index| quad((index % 40) as f32)).collect();

        cycle(&mut ui, &device, &queue, &view, &heavy, 1);
        let [vertex_bytes, index_bytes] = ui.mesh_buffer_capacity_bytes();
        let [vertex_mirror, index_mirror] = ui.mesh_mirror_capacity();
        assert!(vertex_bytes > UI_MIN_VERTEX_BYTES as u64);
        assert!(index_bytes > UI_MIN_INDEX_BYTES as u64);
        assert!(ui.supersample_target_allocated());

        // UI goes empty: capacities must decay and the idle target release.
        ui.clear();
        for _ in 0..(SHRINK_LOW_TICKS + UI_TARGET_IDLE_RELEASE_TICKS + 1) {
            ui.shrink_tick(&device, &queue);
        }
        let [shrunk_vertex_bytes, shrunk_index_bytes] = ui.mesh_buffer_capacity_bytes();
        let [shrunk_vertex_mirror, shrunk_index_mirror] = ui.mesh_mirror_capacity();
        assert!(shrunk_vertex_bytes < vertex_bytes);
        assert!(shrunk_index_bytes < index_bytes);
        assert!(shrunk_vertex_mirror < vertex_mirror);
        assert!(shrunk_index_mirror < index_mirror);
        assert!(!ui.supersample_target_allocated());

        // Recreating the target forces a redraw of the retained raster.
        let redraws = ui.ui_supersample_redraws();
        cycle(&mut ui, &device, &queue, &view, &heavy, 2);
        assert_eq!(ui.ui_supersample_redraws(), redraws + 1);
        assert!(ui.supersample_target_allocated());
    });
}
