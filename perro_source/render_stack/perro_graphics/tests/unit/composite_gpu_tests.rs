//! Pixel checks through the real scene resolve, UI/2D, final effects and present.
use super::*;
use crate::ui::renderer::UiRenderer;
use epaint::{Color32, Mesh, Primitive, Rect, TextureId, pos2};
use perro_ids::TextureID;
use perro_render_bridge::{UiCommand, UiCornerRadiiState, UiRectState, UiTextAlignState};

const LINEAR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const OUTPUT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const SIZE: [u32; 2] = [64, 64];

async fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
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
            label: Some("composite test"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        })
        .await
        .ok()
}

fn target(device: &wgpu::Device, size: [u32; 2], format: wgpu::TextureFormat) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("composite test target"),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

fn clear(encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, color: [f64; 3]) {
    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("composite test clear"),
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
    let pixel_bytes = output
        .format()
        .block_copy_size(None)
        .expect("GPU test setup must succeed");
    let stride = (output.width() * pixel_bytes).div_ceil(256) * 256;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("composite pixels"),
        size: u64::from(stride * output.height()),
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
                rows_per_image: Some(output.height()),
            },
        },
        output.size(),
    );
    queue.submit([encoder.finish()]);
    let slice = staging.slice(..);
    let (tx, rx) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).expect("GPU test setup must succeed");
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("GPU test setup must succeed");
    rx.recv()
        .expect("GPU test setup must succeed")
        .expect("GPU test setup must succeed");
    let result = slice
        .get_mapped_range()
        .expect("GPU test setup must succeed")
        .chunks(stride as usize)
        .flat_map(|row| {
            row[..(output.width() * pixel_bytes) as usize]
                .iter()
                .copied()
        })
        .collect();
    staging.unmap();
    result
}

fn quad(rect: [f32; 4], texture: TextureId, color: Color32) -> Arc<ClippedPrimitive> {
    let mut mesh = Mesh::with_texture(texture);
    mesh.add_rect_with_uv(
        Rect::from_min_max(pos2(rect[0], rect[1]), pos2(rect[2], rect[3])),
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        color,
    );
    Arc::new(ClippedPrimitive {
        clip_rect: Rect::EVERYTHING,
        primitive: Primitive::Mesh(mesh),
    })
}

fn tint() -> PostProcessEffect {
    PostProcessEffect::ColorFilter {
        color: [0.5, 1.0, 1.0],
        strength: 1.0,
    }
}

struct Fixture {
    present: PresentProcessor,
    composite: FrameComposite,
    scene: wgpu::TextureView,
    stream_color: [f64; 3],
    output: wgpu::Texture,
    ui: GpuUi,
    label: UiRenderer,
    overlay: Gpu2D,
    stream_post: FrameComposite,
    white: wgpu::TextureView,
    resources: ResourceStore,
    shared: SharedTextureStore,
}

impl Fixture {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let mut present = PresentProcessor::new(device, OUTPUT);
        present.set_output_size(SIZE[0], SIZE[1]);
        let composite = FrameComposite::new(device, queue, LINEAR, SIZE, &present);
        // Half-res scene and independent subview; main UI still uses SIZE.
        let scene = target(device, [32, 32], LINEAR).create_view(&Default::default());
        let white = target(device, [1, 1], LINEAR).create_view(&Default::default());
        let stream_post = FrameComposite::new(device, queue, LINEAR, [8, 8], &present);
        let mut ui = GpuUi::new(device, LINEAR, TextureFilterMode::Nearest);
        ui.upsert_external_image_texture(device, TextureID::from_u64(91), white.clone(), [1, 1]);
        let mut label = UiRenderer::new();
        label.submit(UiCommand::UpsertLabel {
            node: NodeID::from_parts(1, 0),
            rect: UiRectState {
                center: [-16.0, -16.0],
                size: [30.0, 30.0],
                pivot: [0.5; 2],
                rotation_radians: 0.0,
                z_index: 0,
            },
            clip_rect: [0.0, 32.0, 32.0, 64.0],
            text: Arc::from("UI"),
            color: perro_structs::Color::from_rgba([0.0, 1.0, 0.0, 1.0]),
            font_size: 20.0,
            font: perro_ui::UiFont::Default,
            wrap_width: None,
            h_align: UiTextAlignState::Center,
            v_align: UiTextAlignState::Center,
            backdrop_color: perro_structs::Color::TRANSPARENT,
            corner_radii: UiCornerRadiiState::default(),
            padding: [0.0; 4],
            projected_quad: None,
            depth_test: false,
            fit_content: false,
        });
        Self {
            present,
            composite,
            scene,
            stream_color: [0.8, 0.2, 0.1],
            white,
            stream_post,
            ui,
            label,
            output: target(device, SIZE, OUTPUT),
            overlay: Gpu2D::new(device, LINEAR, 1, TextureFilterMode::Nearest),
            resources: ResourceStore::new(),
            shared: SharedTextureStore::default(),
        }
    }

    fn frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera_fx: &[PostProcessEffect],
        global_fx: &[PostProcessEffect],
        access: VisualAccessibilitySettings,
        layers: bool,
    ) -> Vec<u8> {
        let mut encoder = device.create_command_encoder(&Default::default());
        clear(&mut encoder, &self.scene, [0.8, 0.2, 0.1]);
        clear(&mut encoder, &self.white, [1.0; 3]);
        // Subview-local tint, kept separate from the final main-frame chain.
        clear(
            &mut encoder,
            self.stream_post.post.scene_view(),
            self.stream_color,
        );
        let camera = Camera3DState::default();
        let stream_input = self.stream_post.post.scene_view().clone();
        self.stream_post.apply_global(
            device,
            queue,
            &mut encoder,
            &self.present,
            LINEAR,
            &stream_input,
            1,
            &camera,
            &[tint()],
            None,
            VisualAccessibilitySettings::default(),
            None,
            None,
            false,
            None,
        );
        let stream_view = self
            .stream_post
            .accessibility
            .as_ref()
            .expect("GPU test setup must succeed")
            .intermediate_view()
            .clone();
        self.ui
            .upsert_external_image_texture(device, TextureID::from_u64(92), stream_view, [8, 8]);
        self.ui.note_live_texture_write(TextureID::from_u64(92));
        let view = self.composite.post.scene_view().clone();
        let taa_frame = self.present.taa_active().then(|| PresentTaaFrame {
            depth_view: depth_target(device, &mut encoder, [32, 32], 0.5),
            inv_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            prev_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        });
        self.present.compose_scene(
            queue,
            &mut encoder,
            &self.scene,
            1,
            &view,
            LINEAR,
            [32, 32],
            taa_frame.as_ref(),
        );
        let (camera_active, _) = self.composite.apply_camera(
            device,
            queue,
            &mut encoder,
            LINEAR,
            &view,
            self.composite.generation(),
            &camera,
            camera_fx,
            None,
            None,
            None,
            false,
            None,
        );
        if camera_active {
            let camera_view = self
                .composite
                .camera_scene_view()
                .expect("camera test post target");
            self.present.blit_scene(
                &mut encoder,
                camera_view,
                self.composite.camera_generation(),
                1,
                &view,
                LINEAR,
            );
        }
        if layers {
            let paint = self.label.prepare_paint([64.0, 64.0]);
            let mut primitives = paint.primitives.to_vec();
            primitives.push(quad(
                [0.0, 0.0, 24.0, 24.0],
                TextureId::User(91),
                Color32::from_rgba_unmultiplied(255, 128, 64, 128),
            ));
            primitives.push(quad(
                [32.0, 0.0, 56.0, 24.0],
                TextureId::User(92),
                Color32::WHITE,
            ));
            self.ui.prepare(
                device,
                queue,
                UiPrepareInput {
                    resources: &self.resources,
                    shared_textures: &mut self.shared,
                    viewport: SIZE,
                    primitives: &primitives,
                    world_projections: &[],
                    textures_delta: paint.textures_delta,
                    texture_size: paint.texture_size,
                    revision: 1,
                    static_texture_lookup: None,
                },
            );
            self.ui.render_pass(device, &mut encoder, &view, SIZE, None);
            self.overlay.prepare(
                device,
                queue,
                Prepare2D {
                    resources: &self.resources,
                    shared_textures: &mut self.shared,
                    camera: Camera2DUniform {
                        view: Mat4::IDENTITY.to_cols_array_2d(),
                        ndc_scale: [1.0 / 32.0; 2],
                        pad: [0.0; 2],
                    },
                    rects: &[RectInstanceGpu {
                        center: [16.0, -16.0],
                        size: [24.0; 2],
                        color: [64, 128, 255, 255],
                        z_index: 0,
                        shape_kind: 1,
                        thickness: 0.0,
                    }],
                    upload: &RectUploadPlan {
                        full_reupload: true,
                        dirty_ranges: vec![],
                        draw_count: 1,
                    },
                    sprites: &[],
                    sprites_revision: 0,
                    force_sprite_prepare: false,
                    point_lights: &[],
                    point_lights_revision: 0,
                    shadow_casters: &[],
                    shadow_casters_revision: 0,
                    static_texture_lookup: None,
                },
            );
            self.overlay.render_pass(&mut encoder, &view, None, 1);
        }
        let (intermediate, _, _) = self.composite.apply_global(
            device,
            queue,
            &mut encoder,
            &self.present,
            LINEAR,
            &view,
            1,
            &camera,
            global_fx,
            None,
            access,
            None,
            None,
            false,
            None,
        );
        self.present.apply(
            queue,
            &mut encoder,
            self.composite.present_bind_group(intermediate),
            &self.output.create_view(&Default::default()),
            SIZE,
            1.0 / 60.0,
            PresentExposureSettings::default(),
            HdrStatus::default(),
            None,
        );
        queue.submit([encoder.finish()]);
        read(device, queue, &self.output)
    }
}

fn pixel(bytes: &[u8], x: usize, y: usize) -> &[u8] {
    &bytes[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4]
}

fn depth_target(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    size: [u32; 2],
    depth: f32,
) -> wgpu::TextureView {
    let view =
        target(device, size, wgpu::TextureFormat::Depth32Float).create_view(&Default::default());
    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("composite test depth"),
        color_attachments: &[],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(depth),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    drop(_pass);
    view
}

fn linear_pixels(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    input: &wgpu::TextureView,
    size: [u32; 2],
) -> Vec<f32> {
    let output = target(device, size, LINEAR);
    let mut copy = PresentProcessor::new(device, OUTPUT);
    let mut encoder = device.create_command_encoder(&Default::default());
    copy.compose_scene(
        queue,
        &mut encoder,
        input,
        1,
        &output.create_view(&Default::default()),
        LINEAR,
        size,
        None,
    );
    queue.submit([encoder.finish()]);
    read(device, queue, &output)
        .chunks_exact(2)
        .map(|bytes| half::f16::from_le_bytes([bytes[0], bytes[1]]).to_f32())
        .collect()
}

#[test]
fn scene_taa_excludes_ui_and_final_effects_from_history() {
    pollster::block_on(async {
        let Some((device, queue)) = device().await else {
            eprintln!("skip composite TAA test: no adapter");
            return;
        };
        let mut f = Fixture::new(&device, &queue);
        f.present.set_taa_active(true);
        for _ in 0..3 {
            f.frame(
                &device,
                &queue,
                &[tint()],
                &[PostProcessEffect::BlackWhite { amount: 1.0 }],
                VisualAccessibilitySettings::default(),
                true,
            );
            let taa = f.present.taa.as_ref().expect("GPU test setup must succeed");
            assert!(
                taa.ldr_view.is_none(),
                "scene TAA must reuse existing input"
            );
            assert!(
                taa.history_valid,
                "final present must preserve scene history"
            );
            let values = linear_pixels(
                &device,
                &queue,
                &taa.history_views[taa.history_index],
                [32, 32],
            );
            for pixel in values.chunks_exact(4) {
                for (got, expected) in pixel.iter().zip([0.8, 0.2, 0.1, 1.0]) {
                    assert!(
                        (got - expected).abs() < 0.002,
                        "history contains UI/fx: {pixel:?}"
                    );
                }
            }
        }
    });
}

#[test]
fn scene_taa_preserves_hdr_and_rebuilds_on_resize() {
    pollster::block_on(async {
        let Some((device, queue)) = device().await else {
            eprintln!("skip composite HDR/TAA test: no adapter");
            return;
        };
        let mut present = PresentProcessor::new(&device, OUTPUT);
        present.set_taa_active(true);
        let mut composite = FrameComposite::new(&device, &queue, LINEAR, SIZE, &present);
        for (generation, (scene_size, output_size)) in
            [([16, 16], SIZE), ([16, 16], [80, 48]), ([32, 32], [32, 32])]
                .into_iter()
                .enumerate()
        {
            composite.resize(&device, output_size, &present);
            present.set_output_size(output_size[0], output_size[1]);
            let input = target(&device, scene_size, LINEAR).create_view(&Default::default());
            let mut encoder = device.create_command_encoder(&Default::default());
            clear(&mut encoder, &input, [4.0, 2.0, 0.25]);
            let frame = PresentTaaFrame {
                depth_view: depth_target(&device, &mut encoder, scene_size, 0.5),
                inv_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
                prev_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            };
            present.compose_scene(
                &queue,
                &mut encoder,
                &input,
                generation as u64 + 1,
                composite.post.scene_view(),
                LINEAR,
                scene_size,
                Some(&frame),
            );
            queue.submit([encoder.finish()]);
            assert_eq!(
                present.last_taa_passes,
                if scene_size == output_size { 1 } else { 2 }
            );
            let values = linear_pixels(&device, &queue, composite.post.scene_view(), output_size);
            assert!(
                (values[0] - 4.0).abs() < 0.01 && (values[1] - 2.0).abs() < 0.01,
                "HDR must survive TAA: {:?}",
                &values[..4]
            );
            assert_eq!(
                present
                    .taa
                    .as_ref()
                    .expect("GPU test setup must succeed")
                    .size,
                scene_size
            );
        }
    });
}

fn depth_shader(_: u64) -> &'static str {
    "fn post_process(uv: vec2<f32>, color: vec4<f32>, depth: f32) -> vec4<f32> { return vec4<f32>(depth, depth, depth, 1.0); }"
}

#[test]
fn final_composite_handles_depth_resize_msaa_and_output_formats() {
    pollster::block_on(async {
        let Some((device, queue)) = device().await else {
            eprintln!("skip composite resize/depth test: no adapter");
            return;
        };
        let mut present = PresentProcessor::new(&device, OUTPUT);
        let mut composite = FrameComposite::new(&device, &queue, LINEAR, SIZE, &present);
        let custom = [PostProcessEffect::Custom {
            shader_path: "test_depth.wgsl".into(),
            params: vec![],
        }];
        for (index, size) in [SIZE, [80, 48], SIZE].into_iter().enumerate() {
            composite.resize(&device, size, &present);
            for samples in [1, 4] {
                present.set_fxaa_active(index == 1 && samples == 1);
                present.set_smaa_active(index == 2 && samples == 1);
                let scene = target(&device, [16, 16], LINEAR).create_view(&Default::default());
                let msaa = create_msaa_color_target(&device, LINEAR, 16, 16, samples);
                for use_depth in [false, true] {
                    let mut encoder = device.create_command_encoder(&Default::default());
                    {
                        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("composite test scene resolve"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: msaa.as_ref().map(|m| &m.view).unwrap_or(&scene),
                                resolve_target: msaa.as_ref().map(|_| &scene),
                                depth_slice: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                            multiview_mask: None,
                        });
                    }
                    let depth =
                        use_depth.then(|| depth_target(&device, &mut encoder, [16, 16], 0.25));
                    present.compose_scene(
                        &queue,
                        &mut encoder,
                        &scene,
                        (index * 8 + samples as usize * 2 + usize::from(use_depth)) as u64 + 1,
                        composite.post.scene_view(),
                        LINEAR,
                        [16, 16],
                        None,
                    );
                    let composite_input = composite.post.scene_view().clone();
                    let (intermediate, _, _) = composite.apply_global(
                        &device,
                        &queue,
                        &mut encoder,
                        &present,
                        LINEAR,
                        &composite_input,
                        1,
                        &Camera3DState::default(),
                        &custom,
                        depth
                            .as_ref()
                            .map(|d| (d, index as u64 * 8 + u64::from(samples) + 1)),
                        VisualAccessibilitySettings::default(),
                        Some(depth_shader),
                        None,
                        false,
                        None,
                    );
                    let output = target(&device, size, OUTPUT);
                    present.apply(
                        &queue,
                        &mut encoder,
                        composite.present_bind_group(intermediate),
                        &output.create_view(&Default::default()),
                        size,
                        1.0 / 60.0,
                        PresentExposureSettings::default(),
                        HdrStatus::default(),
                        None,
                    );
                    queue.submit([encoder.finish()]);
                    let result = read(&device, &queue, &output);
                    assert!(
                        result[0].abs_diff(result[1]) <= 1 && result[1].abs_diff(result[2]) <= 1
                    );
                    assert!(
                        if use_depth {
                            result[0] < 200
                        } else {
                            result[0] > 225
                        },
                        "depth fallback or view invalidation: {:?}",
                        &result[..4]
                    );
                }
            }
        }
        // A display-format switch recreates only present bindings; the frame
        // remains linear and supports the HDR output shader without a window.
        present = PresentProcessor::new(&device, LINEAR);
        composite.rebind_present(&device, &present);
        let output = target(&device, SIZE, LINEAR);
        let mut encoder = device.create_command_encoder(&Default::default());
        clear(&mut encoder, composite.post.scene_view(), [4.0, 2.0, 1.0]);
        present.apply(
            &queue,
            &mut encoder,
            composite.present_bind_group(false),
            &output.create_view(&Default::default()),
            SIZE,
            1.0 / 60.0,
            PresentExposureSettings::default(),
            HdrStatus {
                active: true,
                headroom: 4.0,
                ..HdrStatus::default()
            },
            None,
        );
        queue.submit([encoder.finish()]);
        let result = read(&device, &queue, &output);
        let red = half::f16::from_le_bytes([result[0], result[1]]).to_f32();
        assert!(
            red.is_finite() && red > 1.0,
            "HDR output must retain headroom: {red}"
        );
    });
}

#[test]
fn camera_effects_stop_before_ui_and_global_effects_cover_final_composite() {
    pollster::block_on(async {
        let Some((device, queue)) = device().await else {
            eprintln!("skip composite GPU test: no adapter");
            return;
        };
        let mut f = Fixture::new(&device, &queue);
        let no_access = VisualAccessibilitySettings::default();
        let baseline = f.frame(&device, &queue, &[], &[], no_access, true);
        let glyph = (33..63)
            .flat_map(|y| (0..31).map(move |x| (x, y)))
            .find(|&(x, y)| {
                let p = pixel(&baseline, x, y);
                p[1] > p[0].saturating_add(30)
            })
            .expect("real font glyph must render green");
        let bw = [PostProcessEffect::BlackWhite { amount: 1.0 }];
        let camera_only = f.frame(&device, &queue, &bw, &[], no_access, true);
        let scene = pixel(&camera_only, 28, 28);
        assert!(
            scene[0].abs_diff(scene[1]) <= 1 && scene[1].abs_diff(scene[2]) <= 1,
            "camera FX must affect the scene: {scene:?}"
        );
        let ui = pixel(&camera_only, glyph.0, glyph.1);
        assert!(
            ui[1] > ui[0].saturating_add(30),
            "camera FX must not affect UI: {ui:?}"
        );
        let all_layers = [(28, 28), (8, 8), (40, 8), (48, 48), glyph];
        for (global, access) in [
            (bw.as_slice(), no_access),
            (
                &[][..],
                no_access.with_color_blind(perro_structs::ColorBlindFilter::Achroma, 1.0),
            ),
        ] {
            let result = f.frame(&device, &queue, &[], global, access, true);
            for (x, y) in all_layers {
                let p = pixel(&result, x, y);
                assert!(
                    p[0].abs_diff(p[1]) <= 1 && p[1].abs_diff(p[2]) <= 1,
                    "global FX must affect layer ({x}, {y}): {p:?}"
                );
            }
        }
        let filtered = f.frame(&device, &queue, &bw, &[tint()], no_access, true);
        let p = pixel(&filtered, 28, 28);
        assert!(
            p[0] < p[1] && p[1].abs_diff(p[2]) <= 1,
            "camera grayscale must feed global tint: {p:?}"
        );
        let repeated = f.frame(&device, &queue, &bw, &[tint()], no_access, true);
        assert_eq!(filtered, repeated, "no repeated tint or alpha buildup");
        let tint_only = f.frame(&device, &queue, &[tint()], &[], no_access, true);
        let scene = pixel(&tint_only, 28, 28);
        assert!(
            scene[0] < pixel(&baseline, 28, 28)[0],
            "camera tint must affect the scene: {scene:?}"
        );
        assert_eq!(
            pixel(&tint_only, 40, 8),
            pixel(&baseline, 40, 8),
            "camera tint must leave UI unchanged"
        );
        // Pure scene / no UI / no 3D depth also uses the final effects path.
        let plain = f.frame(&device, &queue, &[], &bw, no_access, false);
        assert!(pixel(&plain, 8, 8)[0].abs_diff(pixel(&plain, 8, 8)[1]) <= 1);
        let restored = f.frame(&device, &queue, &[], &[], no_access, true);
        assert_eq!(
            baseline, restored,
            "removing effects restores the clean composite"
        );
        f.stream_color = [0.2, 0.7, 0.9];
        let changed = f.frame(&device, &queue, &[], &[], no_access, true);
        assert_ne!(
            pixel(&changed, 40, 8),
            pixel(&baseline, 40, 8),
            "live subview pixels must refresh retained UI"
        );
        assert_eq!(
            pixel(&changed, 28, 28),
            pixel(&baseline, 28, 28),
            "subview change must preserve scene"
        );
    });
}
