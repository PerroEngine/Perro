//! Render-pass structure guards:
//!
//! * sky renders as a draw inside `perro_mesh_pass` instead of owning a
//!   fullscreen pass of its own, and its far-plane fragments are killed by
//!   depth wherever opaque geometry already wrote;
//! * the mesh-blend seam stage (mask pass + full-res scene copy + fullscreen
//!   seam pass) is skipped outright when every blend source is off screen, and
//!   restricted to the sources' screen footprint when they are not;
//! * mesh-blend sources that share a receiver set share one receiver-depth
//!   pass instead of re-rasterizing the same receivers per source;
//! * the 3D water pass attaches a private depth target and takes no scene-depth
//!   copy (see the note on `water_depth_attachment`) - pinned so the full-res
//!   per-frame blit cannot come back silently.
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
    // These tests never prepare, so the view only needs the arena's handles
    // (refcounted, so they outlive the local arena).
    let mesh_arena = SharedMeshArena::new(device, false, false);
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
        &mesh_arena,
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
        gpu.render_pass(&queue, &mut encoder, &view, clear, false, &camera, true, None);
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
        gpu.render_pass(&queue, &mut encoder, &view, clear, false, &camera, false, None);
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
        gpu.render_pass(&queue, &mut encoder, &view, clear, false, &camera, true, None);
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

// -------------------------------------------- mesh-blend source depth reuse

// Source/receiver batches that draw nothing: this case is about how many passes
// the loop encodes, not what lands in them.
fn empty_batch(instance_start: u32, mesh_blend: bool, blend_depth_receiver: bool) -> DrawBatch {
    let mut batch = super::tests::test_batch(instance_start, 0, 1.0);
    batch.mesh.index_count = 0;
    batch.mesh_blend = mesh_blend;
    batch.mesh_blend_depth = blend_depth_receiver;
    batch.render_state = render_state_key(
        batch.state_key,
        batch.material_texture_key.state_hash(),
        0,
        0,
        false,
        0,
        mesh_blend,
    );
    batch
}

// Two receivers, three sources: the first two share a receiver set, the third
// has its own.
fn prime_blend_sources(gpu: &mut Gpu3D, device: &wgpu::Device, blend_depth_receivers: bool) {
    gpu.draw_batches.clear();
    gpu.draw_batches
        .push(empty_batch(0, false, blend_depth_receivers));
    gpu.draw_batches
        .push(empty_batch(1, false, blend_depth_receivers));
    for source in 0..3u32 {
        gpu.draw_batches.push(empty_batch(2 + source, true, false));
    }
    gpu.staged_instance_transforms.clear();
    gpu.rebuild_batch_views();
    gpu.ensure_mesh_blend_targets(device);
    gpu.mesh_blend_receiver_indices = vec![0, 1, 0, 1, 0];
    gpu.mesh_blend_source_receivers = vec![(2, 0..2), (3, 2..4), (4, 4..5)];
}

#[test]
fn mesh_blend_sources_sharing_a_receiver_set_share_one_depth_pass() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip mesh-blend depth reuse test: no wgpu adapter");
            return;
        };
        let mut gpu = new_gpu_3d(&device, &queue);
        gpu.ensure_material_fallback_texture(&device, &queue, &mut SharedTextureStore::default());
        let (_target, view) = color_target(&device);
        let camera = Camera3DState::default();
        let clear = wgpu::Color::BLACK;

        // Receivers outside the global blend-depth list: nothing to seed from,
        // so the first source renders, the second reuses it, the third (a
        // different set) renders again. Without the reuse this is 3 passes.
        prime_blend_sources(&mut gpu, &device, false);
        let error_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perro_render_pass_test_blend_depth_encoder"),
        });
        gpu.render_pass(&queue, &mut encoder, &view, clear, false, &camera, false, None);
        let counters = gpu.pass_counters;
        queue.submit([encoder.finish()]);
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        let validation_error = error_scope.pop().await;
        assert!(
            validation_error.is_none(),
            "reused blend depth failed validation: {validation_error:?}"
        );
        assert_eq!(counters.mesh_blend_source_depth_passes, 2);
        assert_eq!(counters.mesh_blend_source_depth_reuses, 1);

        // Same receivers, now also the frame's global blend-depth set: the two
        // sources over that set need no pass of their own at all.
        prime_blend_sources(&mut gpu, &device, true);
        assert_eq!(gpu.mesh_blend_depth_batch_indices, vec![0, 1]);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perro_render_pass_test_blend_depth_seed_encoder"),
        });
        gpu.render_pass(&queue, &mut encoder, &view, clear, false, &camera, false, None);
        let counters = gpu.pass_counters;
        queue.submit([encoder.finish()]);
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        assert_eq!(counters.mesh_blend_source_depth_passes, 1);
        assert_eq!(counters.mesh_blend_source_depth_reuses, 2);
    });
}

// ------------------------------------------------------------- water depth

#[test]
fn water_depth_attachment_takes_a_private_target_not_a_scene_copy() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip water depth target test: no wgpu adapter");
            return;
        };
        let mut gpu = new_gpu_3d(&device, &queue);
        gpu.pass_counters = PassCounters::default();
        let error_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perro_render_pass_test_water_encoder"),
        });
        let _view = gpu.water_depth_attachment(&device, &mut encoder);
        queue.submit([encoder.finish()]);
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        let validation_error = error_scope.pop().await;
        assert!(
            validation_error.is_none(),
            "private water depth failed validation: {validation_error:?}"
        );
        // Read-only attachment is still rejected (the surface needs its depth
        // writes: alpha-blended, unsorted, wave-displaced chunks, plus the
        // flip-splash pass testing against them), but those writes only ever
        // need water-vs-water, so they go to a private target that is cleared
        // instead of filled with a full-res copy of the scene depth. Scene
        // occlusion moved into the water shaders. Pinned at zero copies so the
        // per-frame Depth32Float blit cannot come back unnoticed.
        assert_eq!(gpu.pass_counters.water_depth_copies, 0);
        assert_eq!(gpu.pass_counters.water_depth_clears, 1);
    });
}

// The compare the water shaders run in place of the hardware depth test, kept
// verbatim from water_3d_render.wgsl / water_flip_render.wgsl. `vs_gradient`
// sweeps fragment depth across the occluder plane (so the boundary pixels where
// water meets geometry are all exercised) and `vs_flat` sits exactly on it (the
// LessEqual equality case).
const DEPTH_REJECT_TEST_WGSL: &str = r#"
@group(0) @binding(0) var scene_depth_tex: texture_depth_2d;

@vertex
fn vs_occluder(@builtin(vertex_index) vertex: u32) -> @builtin(position) vec4<f32> {
    // Left half of the target at z = 0.5; the right half keeps the 1.0 clear.
    var xy = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(0.0, -1.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(0.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    return vec4<f32>(xy[vertex], 0.5, 1.0);
}

@vertex
fn vs_gradient(@builtin(vertex_index) vertex: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec3<f32>, 3>(
        vec3<f32>(-1.0, -1.0, 0.1),
        vec3<f32>(3.0, -1.0, 0.9),
        vec3<f32>(-1.0, 3.0, 0.55),
    );
    return vec4<f32>(p[vertex], 1.0);
}

@vertex
fn vs_flat(@builtin(vertex_index) vertex: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(p[vertex], 0.5, 1.0);
}

@fragment
fn fs_solid() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}

@fragment
fn fs_reject(@builtin(position) frag_pos: vec4<f32>) -> @location(0) vec4<f32> {
    let dims = vec2<i32>(textureDimensions(scene_depth_tex));
    let coord = clamp(vec2<i32>(floor(frag_pos.xy)), vec2<i32>(0), dims - vec2<i32>(1));
    if frag_pos.z > textureLoad(scene_depth_tex, coord, 0) {
        discard;
    }
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}
"#;

fn depth_target(device: &wgpu::Device, label: &str) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: TARGET_SIZE,
            height: TARGET_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn read_pixels(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("perro_render_pass_test_image_readback"),
        size: u64::from(BYTES_PER_ROW) * u64::from(TARGET_SIZE),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("perro_render_pass_test_image_encoder"),
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
    pixels
}

#[test]
fn shader_scene_depth_reject_matches_the_hardware_depth_test() {
    pollster::block_on(async {
        let Some((device, queue)) = test_device().await else {
            eprintln!("skip depth reject equivalence test: no wgpu adapter");
            return;
        };
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_depth_reject_test_shader"),
            source: wgpu::ShaderSource::Wgsl(DEPTH_REJECT_TEST_WGSL.into()),
        });
        let depth_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_depth_reject_test_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let sampled_layouts = [Some(&depth_bgl)];
        // The occluder only lays down depth: with color writes on it would tint
        // the hardware path's target where the tested draw is rejected, which
        // the private-depth path (no occluder in its pass) cannot reproduce.
        let pipeline = |vertex_entry: &str, fragment_entry: &str, sampled: bool| {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perro_depth_reject_test_layout"),
                bind_group_layouts: if sampled { &sampled_layouts[..] } else { &[] },
                immediate_size: 0,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("perro_depth_reject_test_pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vertex_entry),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment_entry),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend: None,
                        write_mask: if vertex_entry == "vs_occluder" {
                            wgpu::ColorWrites::empty()
                        } else {
                            wgpu::ColorWrites::ALL
                        },
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let occluder = pipeline("vs_occluder", "fs_solid", false);
        let hardware = |vertex_entry| pipeline(vertex_entry, "fs_solid", false);
        let rejected = |vertex_entry| pipeline(vertex_entry, "fs_reject", true);

        // Scene depth: written once by the occluder and never touched again,
        // so the reject path samples exactly what the hardware path's
        // attachment held before the tested draw. `hardware_depth` is that
        // attachment (occluder + the tested draw's own writes) and
        // `private_depth` is water's private target - cleared, water only.
        let (_scene_depth, scene_depth_view) = depth_target(&device, "perro_reject_test_scene");
        let (_hardware_depth, hardware_depth_view) =
            depth_target(&device, "perro_reject_test_hardware");
        let (_private_depth, private_depth_view) =
            depth_target(&device, "perro_reject_test_private");
        let depth_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_depth_reject_test_bg"),
            layout: &depth_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&scene_depth_view),
            }],
        });
        let (hardware_target, hardware_view) = color_target(&device);
        let (rejected_target, rejected_view) = color_target(&device);

        let render = |vertex_entry: &'static str| {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("perro_depth_reject_test_encoder"),
            });
            let mut pass = |color: &wgpu::TextureView,
                            depth: &wgpu::TextureView,
                            draw_occluder: bool,
                            pipe: Option<&wgpu::RenderPipeline>,
                            bind: Option<&wgpu::BindGroup>| {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("perro_depth_reject_test_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: color,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth,
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
                if draw_occluder {
                    pass.set_pipeline(&occluder);
                    pass.draw(0..6, 0..1);
                }
                if let Some(pipe) = pipe {
                    pass.set_pipeline(pipe);
                    if let Some(bind) = bind {
                        pass.set_bind_group(0, bind, &[]);
                    }
                    pass.draw(0..3, 0..1);
                }
            };
            // Scene geometry, into the texture the reject path samples.
            pass(&hardware_view, &scene_depth_view, true, None, None);
            // Old behavior: the same scene depth in the attachment, hardware
            // LessEqual, the tested draw writing its own depth on top.
            pass(
                &hardware_view,
                &hardware_depth_view,
                true,
                Some(&hardware(vertex_entry)),
                None,
            );
            // New behavior: private cleared depth, scene occlusion in-shader.
            pass(
                &rejected_view,
                &private_depth_view,
                false,
                Some(&rejected(vertex_entry)),
                Some(&depth_bind_group),
            );
            queue.submit([encoder.finish()]);
            let _ = device.poll(wgpu::PollType::wait_indefinitely());
            (
                read_pixels(&device, &queue, &hardware_target),
                read_pixels(&device, &queue, &rejected_target),
            )
        };

        for (case, vertex_entry) in [
            // Depth ramp straight through the occluder plane: every boundary
            // pixel between "in front" and "behind" is covered.
            ("depth gradient across the occluder", "vs_gradient"),
            // Fragment depth exactly equal to the stored depth: LessEqual keeps
            // it, and so must `!(z > stored)`.
            ("fragment depth equal to scene depth", "vs_flat"),
        ] {
            let (hardware_pixels, rejected_pixels) = render(vertex_entry);
            let differing = hardware_pixels
                .chunks_exact(4)
                .zip(rejected_pixels.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count();
            let max_diff = hardware_pixels
                .iter()
                .zip(rejected_pixels.iter())
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap_or(0);
            assert_eq!(
                (differing, max_diff),
                (0, 0),
                "{case}: shader reject diverges from the hardware depth test",
            );
            // Guard against a degenerate pass that kept or killed everything.
            let lit = hardware_pixels.chunks_exact(4).filter(|p| p[0] > 0).count();
            assert!(
                lit > 0 && lit < (TARGET_SIZE * TARGET_SIZE) as usize || vertex_entry == "vs_flat",
                "{case}: occlusion did not actually split the image ({lit} lit)"
            );
        }
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
