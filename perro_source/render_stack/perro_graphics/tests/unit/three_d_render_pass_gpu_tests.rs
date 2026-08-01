//! Render-pass structure guards:
//!
//! * sky renders as a draw inside `perro_mesh_pass` instead of owning a
//!   fullscreen pass of its own, and its far-plane fragments are killed by
//!   depth wherever opaque geometry already wrote;
//! * the mesh-blend seam stage (mask pass + full-res scene copy + fullscreen
//!   seam pass) is skipped outright when every blend source is off screen, and
//!   restricted to the sources' screen footprint when they are not;
//! * the 3D water pass still takes a full-res scene-depth copy (see the note on
//!   `water_depth_attachment`) - pinned so the copy cannot come back silently
//!   after being removed, or vanish without the surrounding reasoning changing.
//!
//! GPU cases run against a headless wgpu device and are skipped with a note
//! when no adapter is available; the rect-math cases are pure CPU.
use super::*;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const TARGET_SIZE: u32 = 64;
// 64 px * 4 bytes: already the 256-byte copy row alignment.
const BYTES_PER_ROW: u32 = TARGET_SIZE * 4;

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
            label: Some("perro_render_pass_test_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        })
        .await
        .ok()
}

fn new_gpu_3d(device: &wgpu::Device, queue: &wgpu::Queue) -> Gpu3D {
    let cache = PipelineRegistryCache::new();
    let pipelines = cache.get_or_create(device, COLOR_FORMAT, 1);
    Gpu3D::new(
        device,
        queue,
        COLOR_FORMAT,
        Gpu3DConfig {
            sample_count: 1,
            width: TARGET_SIZE,
            height: TARGET_SIZE,
            meshlets_enabled: false,
            dev_meshlets: false,
            meshlet_debug_view: false,
            occlusion_culling: crate::OcclusionCullingMode::Off,
            ssao: crate::SsaoQuality::Off,
            indirect_first_instance_enabled: false,
            multi_draw_indirect_enabled: false,
            multi_draw_indirect_count_enabled: false,
            texture_filter: TextureFilterMode::default(),
            shader_variant_mode: crate::ShaderVariantMode::Generic,
            shadow_pcf_high: false,
        },
        pipelines,
    )
}

fn color_target(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_render_pass_test_color"),
        size: wgpu::Extent3d {
            width: TARGET_SIZE,
            height: TARGET_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: COLOR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

// Flat-white sky with an identity inverse view-projection: every fragment the
// sky shader survives to write comes out (255, 255, 255, 255), which no clear
// color used here can be confused with.
fn write_flat_white_sky(queue: &wgpu::Queue, gpu: &Gpu3D) {
    let white = [[1.0f32, 1.0, 1.0, 1.0]; 3];
    let sky = SkyUniform {
        inv_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        camera_pos: [0.0, 0.0, 0.0, 0.0],
        day_colors: white,
        evening_colors: white,
        night_colors: white,
        horizon_colors: white,
        // Full day weight, no evening / night mix.
        params0: [0.0, 1.0, 0.0, 0.0],
        params1: [0.0, 0.0, 0.0, 0.0],
    };
    queue.write_buffer(&gpu.sky_buffer, 0, bytemuck::bytes_of(&sky));
}

fn read_first_pixel(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> [u8; 4] {
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("perro_render_pass_test_readback"),
        size: u64::from(BYTES_PER_ROW) * u64::from(TARGET_SIZE),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("perro_render_pass_test_readback_encoder"),
    });
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(BYTES_PER_ROW),
                rows_per_image: Some(TARGET_SIZE),
            },
        },
        wgpu::Extent3d {
            width: TARGET_SIZE,
            height: TARGET_SIZE,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    rx.recv().expect("map callback").expect("readback map");
    let pixels = slice
        .get_mapped_range()
        .expect("mapped readback range")
        .to_vec();
    staging.unmap();
    [pixels[0], pixels[1], pixels[2], pixels[3]]
}

// ------------------------------------------------------------------ sky merge

#[test]
fn sky_draws_inside_the_mesh_pass_and_costs_no_extra_pass() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip sky merge test: no wgpu adapter");
            return;
        };
        let mut gpu = new_gpu_3d(&device, &queue);
        gpu.sky_enabled = true;
        write_flat_white_sky(&queue, &gpu);
        let (target, view) = color_target(&device);
        let camera = Camera3DState::default();
        let clear = wgpu::Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };

        let error_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perro_render_pass_test_sky_encoder"),
        });
        gpu.render_pass(&queue, &mut encoder, &view, clear, false, &camera, true);
        let with_sky = gpu.pass_counters;
        queue.submit([encoder.finish()]);
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        let validation_error = error_scope.pop().await;
        assert!(
            validation_error.is_none(),
            "sky-in-mesh-pass failed validation: {validation_error:?}"
        );

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perro_render_pass_test_no_sky_encoder"),
        });
        gpu.render_pass(&queue, &mut encoder, &view, clear, false, &camera, false);
        let without_sky = gpu.pass_counters;
        queue.submit([encoder.finish()]);
        let _ = device.poll(wgpu::PollType::wait_indefinitely());

        assert_eq!(with_sky.sky_draws, 1, "sky drawn once, inside the mesh pass");
        assert_eq!(without_sky.sky_draws, 0);
        assert_eq!(
            with_sky.render_passes, without_sky.render_passes,
            "sky must not add a render pass any more"
        );
        assert_eq!(with_sky.render_passes, 1, "one pass owns clear + sky");

        // The mesh pass now owns the clear, so the sky still has to reach every
        // background pixel: red clear in, white sky out.
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perro_render_pass_test_sky_pixel_encoder"),
        });
        gpu.render_pass(&queue, &mut encoder, &view, clear, false, &camera, true);
        queue.submit([encoder.finish()]);
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        assert_eq!(
            read_first_pixel(&device, &queue, &target),
            [255, 255, 255, 255],
            "sky did not reach the background through the merged pass"
        );
    });
}

#[test]
fn sky_pipeline_is_depth_killed_where_geometry_already_wrote() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip sky depth-kill test: no wgpu adapter");
            return;
        };
        let gpu = new_gpu_3d(&device, &queue);
        write_flat_white_sky(&queue, &gpu);
        let (target, view) = color_target(&device);
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("perro_render_pass_test_depth"),
            size: wgpu::Extent3d {
                width: TARGET_SIZE,
                height: TARGET_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: crate::scene_depth_format(1),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        let red = wgpu::Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };

        // `cleared_depth` stands in for what the opaque geometry left behind:
        // 1.0 = nothing drawn (sky must fill), 0.5 = covered (sky must die
        // before its fragment shader runs).
        let render_sky_over = |cleared_depth: f32| {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("perro_render_pass_test_sky_depth_encoder"),
            });
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("perro_render_pass_test_sky_depth_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(red),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(cleared_depth),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(gpu.pipelines.sky());
                pass.set_bind_group(0, &gpu.sky_bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
            queue.submit([encoder.finish()]);
            let _ = device.poll(wgpu::PollType::wait_indefinitely());
            read_first_pixel(&device, &queue, &target)
        };

        assert_eq!(
            render_sky_over(1.0),
            [255, 255, 255, 255],
            "sky must fill pixels no geometry covered"
        );
        assert_eq!(
            render_sky_over(0.5),
            [255, 0, 0, 255],
            "sky must be depth-killed where geometry is in front"
        );
    });
}

// ------------------------------------------------------------- seam gating

fn seam_source_batch(instance_start: u32) -> DrawBatch {
    let mut batch = super::tests::test_batch(instance_start, 1, 1.0);
    batch.mesh_blend_screen = true;
    batch
}

fn camera_at_origin_looking_down_negative_z() -> Camera3DState {
    // Default camera: identity rotation, origin, perspective. glam's
    // right-handed look direction is -Z, so a source at z = -8 is in view and
    // one at z = +8 is behind the camera.
    Camera3DState::default()
}

fn prime_seam_scene(gpu: &mut Gpu3D, source_world_z: f32) {
    gpu.draw_batches.clear();
    gpu.draw_batches.push(seam_source_batch(0));
    gpu.staged_instance_transforms.clear();
    gpu.staged_instance_transforms.push(TransformInstanceGpu {
        model_row_0: [1.0, 0.0, 0.0, 0.0],
        model_row_1: [0.0, 1.0, 0.0, 0.0],
        model_row_2: [0.0, 0.0, 1.0, source_world_z],
    });
    gpu.mesh_blend_screen_active = true;
    gpu.mesh_blend_mask_batch_entries.clear();
    gpu.mesh_blend_mask_batch_entries
        .push(MeshBlendMaskEntry::Draw {
            batch_index: 0,
            id: 1,
        });
}

#[test]
fn seam_stage_is_skipped_when_every_blend_source_is_off_screen() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip seam gating test: no wgpu adapter");
            return;
        };
        let mut gpu = new_gpu_3d(&device, &queue);
        let camera = camera_at_origin_looking_down_negative_z();
        let (scene_texture, scene_view) = color_target(&device);

        // Source parked behind the camera: nothing the seam pass could rewrite.
        prime_seam_scene(&mut gpu, 8.0);
        gpu.update_mesh_blend_seam_region(&camera);
        assert_eq!(gpu.mesh_blend_seam_region, SeamRegion::Skip);
        gpu.pass_counters = PassCounters::default();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perro_render_pass_test_seam_skip_encoder"),
        });
        gpu.mesh_blend_screen_pass(&device, &mut encoder, &scene_texture, &scene_view);
        assert_eq!(
            gpu.pass_counters.mesh_blend_scene_copies, 0,
            "off-screen sources must not pay the full-res scene copy"
        );
        assert_eq!(gpu.pass_counters.mesh_blend_seam_passes, 0);
        assert_eq!(gpu.pass_counters.mesh_blend_copy_pixels, 0);
        queue.submit([encoder.finish()]);

        // Same source in front of the camera: the stage runs, and the copy is
        // restricted to the footprint the shader can actually read.
        prime_seam_scene(&mut gpu, -8.0);
        gpu.update_mesh_blend_seam_region(&camera);
        assert_ne!(gpu.mesh_blend_seam_region, SeamRegion::Skip);
        gpu.pass_counters = PassCounters::default();
        let error_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perro_render_pass_test_seam_run_encoder"),
        });
        gpu.mesh_blend_screen_pass(&device, &mut encoder, &scene_texture, &scene_view);
        assert_eq!(gpu.pass_counters.mesh_blend_scene_copies, 1);
        assert_eq!(gpu.pass_counters.mesh_blend_seam_passes, 1);
        assert!(
            gpu.pass_counters.mesh_blend_copy_pixels > 0
                && gpu.pass_counters.mesh_blend_copy_pixels <= TARGET_SIZE * TARGET_SIZE,
            "copy footprint {} out of range",
            gpu.pass_counters.mesh_blend_copy_pixels
        );
        queue.submit([encoder.finish()]);
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        let validation_error = error_scope.pop().await;
        assert!(
            validation_error.is_none(),
            "scissored seam pass / partial copy failed validation: {validation_error:?}"
        );
    });
}

#[test]
fn seam_region_falls_back_to_full_screen_without_a_usable_bound() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip seam fallback test: no wgpu adapter");
            return;
        };
        let mut gpu = new_gpu_3d(&device, &queue);
        let camera = camera_at_origin_looking_down_negative_z();

        // Sentinel radius: batch_merged_world_sphere yields no bound, so the
        // source counts as visible and the stage stays full-screen.
        prime_seam_scene(&mut gpu, -8.0);
        gpu.draw_batches[0].local_radius = 1.0e9;
        gpu.update_mesh_blend_seam_region(&camera);
        assert_eq!(gpu.mesh_blend_seam_region, SeamRegion::Full);

        // A source big enough to span the viewport also lands on Full.
        prime_seam_scene(&mut gpu, -8.0);
        gpu.draw_batches[0].local_radius = 400.0;
        gpu.update_mesh_blend_seam_region(&camera);
        assert_eq!(gpu.mesh_blend_seam_region, SeamRegion::Full);

        // No screen-blend participants at all: nothing to do.
        gpu.mesh_blend_screen_active = false;
        gpu.update_mesh_blend_seam_region(&camera);
        assert_eq!(gpu.mesh_blend_seam_region, SeamRegion::Skip);
    });
}

#[test]
fn seam_region_restricts_to_the_source_footprint_at_screen_resolution() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip seam footprint test: no wgpu adapter");
            return;
        };
        let mut gpu = new_gpu_3d(&device, &queue);
        // Gate math is pure CPU; point it at a real viewport instead of the
        // 64x64 test target, where a unit sphere fills the frame anyway.
        gpu.depth_size = (1920, 1080);
        let camera = camera_at_origin_looking_down_negative_z();
        prime_seam_scene(&mut gpu, -20.0);
        gpu.update_mesh_blend_seam_region(&camera);
        let SeamRegion::Rect(x, y, w, h) = gpu.mesh_blend_seam_region else {
            panic!("expected a restricted rect, got {:?}", gpu.mesh_blend_seam_region);
        };
        assert!(x + w <= 1920 && y + h <= 1080, "rect escapes the viewport");
        let covered = u64::from(w) * u64::from(h);
        let full = 1920u64 * 1080;
        assert!(
            covered * 4 < full,
            "seam scissor {covered}px is not a meaningful cut of {full}px"
        );
    });
}

// ------------------------------------------------------------- water depth

#[test]
fn water_depth_attachment_still_copies_the_scene_depth() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip water depth copy test: no wgpu adapter");
            return;
        };
        let mut gpu = new_gpu_3d(&device, &queue);
        gpu.pass_counters = PassCounters::default();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perro_render_pass_test_water_encoder"),
        });
        let _view = gpu.water_depth_attachment(&device, &mut encoder);
        queue.submit([encoder.finish()]);
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        // The read-only-attachment alternative was rejected: the water surface
        // pipeline is alpha-blended, unsorted and wave-displaced, so it relies
        // on its own depth writes for self-occlusion, and the flip-splash pass
        // that follows depth-tests against them. Dropping the copy would mean
        // dropping those writes. Pinned at one copy per water frame so the cost
        // stays visible and a future fix has a counter to move.
        assert_eq!(gpu.pass_counters.water_depth_copies, 1);
    });
}

// ------------------------------------------------------------- rect math

#[test]
fn seam_bounds_grow_by_the_tap_reach_and_clamp_to_the_target() {
    let reach = super::mesh_blend_screen::SEAM_TAP_REACH_PX as f32;
    // Small rect in the middle: grown by the reach on every side.
    let region = seam_region_from_bounds([100.0, 100.0, 140.0, 130.0], 640, 480);
    let SeamRegion::Rect(x, y, w, h) = region else {
        panic!("expected a restricted rect, got {region:?}");
    };
    assert_eq!(x, (100.0 - reach) as u32);
    assert_eq!(y, (100.0 - reach) as u32);
    assert_eq!(w, (140.0 + reach) as u32 - x);
    assert_eq!(h, (130.0 + reach) as u32 - y);

    // Bounds off the left/top edge clamp instead of wrapping.
    let region = seam_region_from_bounds([-500.0, -500.0, 10.0, 10.0], 640, 480);
    assert_eq!(region, SeamRegion::Rect(0, 0, 10 + reach as u32, 10 + reach as u32));

    // Covering the whole target collapses to Full (no scissor, plain copy).
    assert_eq!(
        seam_region_from_bounds([-10.0, -10.0, 700.0, 700.0], 640, 480),
        SeamRegion::Full
    );
    // Non-finite bounds never produce a rect.
    assert_eq!(
        seam_region_from_bounds([f32::NAN, 0.0, 10.0, 10.0], 640, 480),
        SeamRegion::Full
    );
}

#[test]
fn sphere_screen_bounds_rejects_spheres_crossing_the_camera_plane() {
    let view = Mat4::look_at_rh(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
    let proj = Mat4::perspective_rh(0.9, 1.0, 0.1, 100.0);
    let view_proj = proj * view;
    // Well in front: a finite, on-screen rect.
    let bounds =
        sphere_screen_bounds(view_proj, Vec3::ZERO, 1.0, 640, 480).expect("in-view sphere bounds");
    assert!(bounds[0] < bounds[2] && bounds[1] < bounds[3]);
    assert!(bounds[0] > 0.0 && bounds[2] < 640.0);
    // Straddling the eye: the projection folds, so no rect is returned.
    assert!(sphere_screen_bounds(view_proj, Vec3::new(0.0, 0.0, 5.0), 2.0, 640, 480).is_none());
}
