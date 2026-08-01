//! Multimesh shadow-caster submission under a moving camera.
//!
//! A moving camera re-fits every cascade window, so the cascade layers cannot
//! keep their cached depth and re-render every frame. What made that expensive
//! was the multimesh caster path: it culled at *batch* granularity on the CPU
//! and then drew every instance of every surviving batch, once per cascade.
//! These cases pin the per-layer instance submission (which the per-layer GPU
//! instance cull collapses) and the static-camera cache (which must stay at
//! zero re-renders).
//!
//! Runs the real `Gpu3D::prepare` + `Gpu3D::render_pass` against a headless
//! wgpu device; skipped with a note when no adapter is available.
use super::*;
use crate::resources::ResourceStore;
use crate::three_d::renderer::DenseMultiMeshDraw3D;
use perro_ids::{MaterialID, MeshID, NodeID};
use perro_render_bridge::{
    DenseInstancePose3D, LODOptions3D, MeshSurfaceBinding3D, RayLight3DState,
};
use perro_structs::Color;

/// CPU mirror of `instance_world_sphere` in multimesh_cull.wgsl. Kept in the
/// test so the GPU cull has an independent reference to be checked against.
fn instance_world_sphere_reference(
    param: &MultiMeshDrawParamGpu,
    instance: &MultiMeshInstanceGpu,
    mesh_radius: f32,
) -> (Vec3, f32) {
    let p = Vec4::new(
        instance.position[0],
        instance.position[1],
        instance.position[2],
        1.0,
    );
    let row = |r: [f32; 4]| Vec4::from(r).dot(p);
    let center = Vec3::new(
        row(param.model_row_0),
        row(param.model_row_1),
        row(param.model_row_2),
    );
    let draw_scale = f32::from_bits(param.scale_bits);
    let inst_scale = instance.scale[0]
        .max(instance.scale[1])
        .max(instance.scale[2])
        * draw_scale;
    let col2 = |r: [f32; 4]| Vec3::new(r[0], r[1], r[2]).length_squared();
    let model_scale = col2(param.model_row_0)
        .max(col2(param.model_row_1))
        .max(col2(param.model_row_2))
        .max(1.0e-12)
        .sqrt();
    (center, mesh_radius * inst_scale.max(0.0) * model_scale)
}

// 64 fields x 1_600 instances = 102_400 caster instances, spread over a world
// far wider than one cascade so the cascades disagree about what is visible.
const FIELDS: u32 = 64;
const FIELD_INSTANCES: u32 = 1_600;
const FIELD_SPAN: f32 = 40.0;
const FIELD_GRID: u32 = 8;
const FIELD_PITCH: f32 = 60.0;
const TARGET: u32 = 256;
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

async fn test_device(
    want_indirect_first_instance: bool,
) -> Option<(wgpu::Device, wgpu::Queue, bool)> {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        })
        .await
        .ok()?;
    let has_first_instance = adapter
        .features()
        .contains(wgpu::Features::INDIRECT_FIRST_INSTANCE);
    let features = if want_indirect_first_instance && has_first_instance {
        wgpu::Features::INDIRECT_FIRST_INSTANCE
    } else {
        wgpu::Features::empty()
    };
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("perro_shadow_multimesh_test_device"),
            required_features: features,
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
        })
        .await
        .ok()?;
    Some((
        device,
        queue,
        want_indirect_first_instance && has_first_instance,
    ))
}

fn new_gpu_3d(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    arena: &SharedMeshArena,
    indirect_first_instance: bool,
) -> Gpu3D {
    let cache = PipelineRegistryCache::new();
    let pipelines = cache.get_or_create(device, COLOR_FORMAT, 1);
    Gpu3D::new(
        device,
        queue,
        COLOR_FORMAT,
        Gpu3DConfig {
            sample_count: 1,
            width: TARGET,
            height: TARGET,
            meshlets_enabled: false,
            dev_meshlets: false,
            meshlet_debug_view: false,
            occlusion_culling: crate::OcclusionCullingMode::Off,
            ssao: crate::SsaoQuality::Off,
            indirect_first_instance_enabled: indirect_first_instance,
            multi_draw_indirect_enabled: false,
            multi_draw_indirect_count_enabled: false,
            texture_filter: TextureFilterMode::default(),
            shader_variant_mode: crate::ShaderVariantMode::Generic,
            shadow_pcf_high: false,
        },
        pipelines,
        arena,
    )
}

/// Scene shape. `Clusters` mirrors the Demo3D multimesh scene (many small
/// fields, which the CPU batch sphere cull already filters well); `WideFields`
/// mirrors a grass/foliage scene (a few fields each spanning the world, where
/// batch-level culling is all-or-nothing and can filter nothing at all).
#[derive(Clone, Copy)]
enum SceneShape {
    Clusters,
    WideFields,
}

impl SceneShape {
    fn fields(self) -> u32 {
        match self {
            SceneShape::Clusters => FIELDS,
            SceneShape::WideFields => 4,
        }
    }

    fn instances(self) -> u32 {
        match self {
            SceneShape::Clusters => FIELD_INSTANCES,
            SceneShape::WideFields => 25_600,
        }
    }

    fn span(self) -> f32 {
        match self {
            SceneShape::Clusters => FIELD_SPAN,
            SceneShape::WideFields => 480.0,
        }
    }

    fn pitch(self) -> f32 {
        match self {
            SceneShape::Clusters => FIELD_PITCH,
            SceneShape::WideFields => 0.0,
        }
    }

    fn grid(self) -> u32 {
        match self {
            SceneShape::Clusters => FIELD_GRID,
            SceneShape::WideFields => 2,
        }
    }
}

fn field_draw(index: u32, mesh: MeshID, material: MaterialID, shape: SceneShape) -> Draw3DInstance {
    let field_instances = shape.instances();
    let field_span = shape.span();
    let field_grid = shape.grid();
    let field_pitch = shape.pitch();
    let instances: Arc<[DenseInstancePose3D]> = (0..field_instances)
        .map(|i| {
            let side = (field_instances as f32).sqrt() as u32;
            let step = field_span / side as f32;
            DenseInstancePose3D {
                position: [
                    (i % side) as f32 * step - field_span * 0.5,
                    0.0,
                    (i / side) as f32 * step - field_span * 0.5,
                ],
                scale: [1.0, 1.0, 1.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                has_blend_shape_weight_override: false,
                blend_shape_weights: Arc::from([]),
            }
        })
        .collect::<Vec<_>>()
        .into();
    let gx = (index % field_grid) as f32 - field_grid as f32 * 0.5;
    let gz = (index / field_grid) as f32 - field_grid as f32 * 0.5;
    let node_model = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [gx * field_pitch, 0.0, gz * field_pitch, 1.0],
    ];
    Draw3DInstance {
        node: NodeID::from_parts(700_000 + index, 0),
        kind: Draw3DKind::Mesh(mesh),
        surfaces: Arc::from([MeshSurfaceBinding3D {
            material: Some(material),
            overrides: Arc::from([]),
            modulate: Color::WHITE,
        }]),
        instance_mats: Arc::from([]),
        blend_shape_weights: Arc::from([]),
        debug_color: None,
        skeleton: None,
        dense_multimesh: Some(DenseMultiMeshDraw3D {
            node_model,
            instance_scale: 1.0,
            instances,
        }),
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend: MeshBlendOptions3D::default(),
        cast_shadows: true,
        receive_shadows: true,
    }
}

fn sun() -> Lighting3DState {
    let mut lighting = Lighting3DState::default();
    lighting.ray_lights[0] = Some(RayLight3DState {
        direction: [-0.4, -1.0, -0.3],
        color: [1.0, 1.0, 1.0],
        intensity: 3.0,
        cast_shadows: true,
        shadow_strength: 1.0,
        shadow_depth_bias: 0.0,
        shadow_normal_bias: 0.0,
    });
    lighting
}

fn set_sun_shadows(harness: &mut ShadowHarness, on: bool) {
    let sun = harness.lighting.ray_lights[0]
        .as_mut()
        .expect("harness lighting always carries the sun");
    sun.cast_shadows = on;
}

fn camera_at(x: f32) -> Camera3DState {
    Camera3DState {
        position: [x, 6.0, 0.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
        projection: CameraProjectionState::Perspective {
            fov_y_degrees: 70.0,
            near: 0.1,
            far: 300.0,
        },
        ..Camera3DState::default()
    }
}

struct ShadowHarness {
    device: wgpu::Device,
    queue: wgpu::Queue,
    gpu: Gpu3D,
    resources: ResourceStore,
    shared_textures: SharedTextureStore,
    mesh_arena: SharedMeshArena,
    lighting: Lighting3DState,
    revision: u64,
    // Kept alive for the view below.
    _color: wgpu::Texture,
    color_view: wgpu::TextureView,
}

/// One rendered frame's shadow submission tally.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct FrameShadowStats {
    layers: u32,
    batches: u32,
    instances: u64,
    culled_layers: u32,
}

impl ShadowHarness {
    fn frame(&mut self, draws: &[Draw3DInstance], camera: Camera3DState) -> FrameShadowStats {
        self.timed_frame(draws, camera).0
    }

    fn timed_frame(
        &mut self,
        draws: &[Draw3DInstance],
        camera: Camera3DState,
    ) -> (FrameShadowStats, std::time::Duration) {
        self.revision += 1;
        self.gpu.prepare(
            &self.device,
            &self.queue,
            Prepare3D {
                resources: &self.resources,
                shared_textures: &mut self.shared_textures,
                mesh_arena: &mut self.mesh_arena,
                mesh_arena_compact_allowed: true,
                camera: camera.clone(),
                lighting: &self.lighting,
                draws,
                draws_revision: self.revision,
                force_full_rebuild: false,
                decals: &[],
                decals_revision: 0,
                width: TARGET,
                height: TARGET,
                static_texture_lookup: None,
                static_mesh_lookup: None,
                static_shader_lookup: None,
            },
        );
        let start = std::time::Instant::now();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("perro_shadow_multimesh_test_encoder"),
            });
        self.gpu.render_pass(
            &self.queue,
            &mut encoder,
            &self.color_view,
            wgpu::Color::BLACK,
            false,
            &camera,
            false,
        );
        let submission = self.queue.submit(Some(encoder.finish()));
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: None,
        });
        let elapsed = start.elapsed();
        (
            FrameShadowStats {
                layers: self.gpu.pass_counters.shadow_layer_renders,
                batches: self.gpu.pass_counters.shadow_multimesh_batch_draws,
                instances: self.gpu.pass_counters.shadow_multimesh_instance_draws,
                culled_layers: self.gpu.pass_counters.shadow_multimesh_culled_layers,
            },
            elapsed,
        )
    }
}

impl ShadowHarness {
    /// Per-batch `instance_count` the last rendered shadow layer's cull wrote
    /// into the shadow indirect records.
    fn read_shadow_indirect_counts(&self) -> Vec<u32> {
        let batches = self.gpu.multimesh_batches.len();
        let stride = std::mem::size_of::<DrawIndexedIndirectGpu>();
        let bytes = (batches * stride) as u64;
        if bytes == 0 {
            return Vec::new();
        }
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_shadow_indirect_readback"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("perro_shadow_indirect_readback_encoder"),
            });
        encoder.copy_buffer_to_buffer(
            &self.gpu.multimesh_shadow_indirect_buffer,
            0,
            &staging,
            0,
            bytes,
        );
        let submission = self.queue.submit(Some(encoder.finish()));
        staging.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: None,
        });
        let view = staging
            .slice(..)
            .get_mapped_range()
            .expect("shadow indirect readback must map");
        let counts = view
            .chunks_exact(stride)
            .map(|row| u32::from_le_bytes([row[4], row[5], row[6], row[7]]))
            .collect();
        drop(view);
        staging.unmap();
        counts
    }
}

/// Instances whose world sphere passes `frustum`, computed on the CPU with the
/// same math `multimesh_cull.wgsl` runs per instance.
fn cpu_reference_visible(gpu: &Gpu3D, frustum: &[Vec4; 6]) -> u64 {
    let mut visible = 0u64;
    for batch in gpu.multimesh_batches.iter() {
        if !batch.casts_shadows || batch.mesh_blend {
            continue;
        }
        let start = batch.instance_start as usize;
        let end = (start + batch.instance_count as usize).min(gpu.staged_multimesh_instances.len());
        let Some(param) = gpu
            .staged_multimesh_draw_params
            .get(batch.draw_param_index as usize)
        else {
            continue;
        };
        for instance in &gpu.staged_multimesh_instances[start..end] {
            let (center, radius) =
                instance_world_sphere_reference(param, instance, batch.mesh_local_radius);
            if sphere_in_frustum(center, radius, frustum) {
                visible += 1;
            }
        }
    }
    visible
}

fn median(mut samples: Vec<std::time::Duration>) -> std::time::Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

/// Cascade budget knob for the cases below. `None` restores the shipped
/// `CASCADE_RENDER_BUDGET`; `Some(MAX_SHADOW_RAY_CASCADES)` turns the budget off
/// so a case can measure (or assert against) the un-spread behavior.
fn set_cascade_budget(budget: Option<usize>) {
    crate::three_d::gpu::shadows::set_cascade_render_budget_override(budget);
}

/// Cascade window snap granularity knob. `None` restores the shipped
/// `CASCADE_SNAP_STEPS`.
fn set_cascade_snap_steps(steps: Option<f32>) {
    crate::three_d::gpu::shadows::set_cascade_snap_steps_override(steps);
}

fn build_harness(
    indirect_first_instance: bool,
    shape: SceneShape,
) -> Option<(ShadowHarness, Vec<Draw3DInstance>)> {
    let (device, queue, first_instance) = pollster::block_on(test_device(indirect_first_instance))?;
    let mesh_arena = SharedMeshArena::new(&device, false, false);
    let gpu = new_gpu_3d(&device, &queue, &mesh_arena, first_instance);
    let mut resources = ResourceStore::new();
    let mesh = resources.create_mesh(perro_builtin_meshes::CUBE_SOURCE, true);
    let material =
        resources.create_material(Material3D::default(), Some("__shadow_mm_test_mat__"), true);
    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_shadow_multimesh_test_color"),
        size: wgpu::Extent3d {
            width: TARGET,
            height: TARGET,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: COLOR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
    let draws: Vec<Draw3DInstance> = (0..shape.fields())
        .map(|i| field_draw(i, mesh, material, shape))
        .collect();
    Some((
        ShadowHarness {
            device,
            queue,
            gpu,
            resources,
            shared_textures: SharedTextureStore::default(),
            mesh_arena,
            lighting: sun(),
            revision: 0,
            _color: color,
            color_view,
        },
        draws,
    ))
}

/// The measurement the fix is judged on: with the camera parked, the cascade
/// cache holds and nothing re-renders; with the camera translating, every
/// cascade re-renders and the multimesh caster submission per frame is whatever
/// the caster path decided to draw.
#[test]
fn moving_camera_multimesh_shadow_submission() {
    let Some((mut harness, draws)) = build_harness(true, SceneShape::Clusters) else {
        eprintln!("skip moving-camera multimesh shadow test: no wgpu adapter");
        return;
    };
    // Warm: first frames build topology / atlases.
    for _ in 0..3 {
        harness.frame(&draws, camera_at(0.0));
    }
    assert!(
        !harness.gpu.staged_multimesh_instances.is_empty(),
        "multimesh staging never ran; scene setup is wrong"
    );
    let total_instances = harness.gpu.staged_multimesh_instances.len();

    // Static camera: cascades keep their cached depth.
    let mut static_stats = FrameShadowStats::default();
    let mut static_times = Vec::new();
    for _ in 0..9 {
        let (stats, elapsed) = harness.timed_frame(&draws, camera_at(0.0));
        static_stats = stats;
        static_times.push(elapsed);
    }

    // Moving camera: 2 world units per frame re-fits every cascade window.
    let mut moving = Vec::new();
    let mut moving_times = Vec::new();
    for step in 1..=9 {
        let (stats, elapsed) = harness.timed_frame(&draws, camera_at(step as f32 * 2.0));
        moving.push(stats);
        moving_times.push(elapsed);
    }
    let moving_stats = moving[moving.len() - 1];
    let peak_instances = moving.iter().map(|s| s.instances).max().unwrap_or(0);

    // Same motion with the sun's shadows off: isolates the shadow passes'
    // share of a moving frame.
    set_sun_shadows(&mut harness, false);
    let mut no_shadow_times = Vec::new();
    for step in 10..=18 {
        let (_, elapsed) = harness.timed_frame(&draws, camera_at(step as f32 * 2.0));
        no_shadow_times.push(elapsed);
    }
    set_sun_shadows(&mut harness, true);

    println!(
        "shadow mm submission ({FIELDS} fields x {FIELD_INSTANCES} inst = {total_instances} \
         instances): static={static_stats:?} moving={moving_stats:?} peak_inst={peak_instances}"
    );
    println!(
        "frame gpu wall time: static={:?} moving={:?} moving_no_shadows={:?}",
        median(static_times),
        median(moving_times),
        median(no_shadow_times),
    );

    assert_eq!(
        static_stats.layers, 0,
        "a parked camera must keep every shadow layer cached"
    );
    assert_eq!(
        static_stats.instances, 0,
        "a parked camera must submit no multimesh caster instances"
    );
    assert!(
        moving_stats.layers > 0,
        "a moving camera must re-render cascade layers"
    );
    // Cascade windows snap to a coarse world grid, so a translating camera must
    // not re-fit (and re-render) all four cascades on every frame.
    let total_layers: u32 = moving.iter().map(|s| s.layers).sum();
    let frames = moving.len() as u32;
    println!(
        "cascade re-renders over {frames} moving frames: {total_layers} of \
         {} (all cascades every frame)",
        frames * MAX_SHADOW_RAY_CASCADES as u32
    );
    assert!(
        total_layers < frames * MAX_SHADOW_RAY_CASCADES as u32,
        "every cascade re-rendered on every moving frame: the coarse window snap is not holding"
    );
    // ...and the layers that do re-render must cull per instance instead of
    // drawing every instance of every batch the CPU sphere cull kept.
    assert_eq!(
        moving_stats.culled_layers, moving_stats.layers,
        "every re-rendered layer must go through the per-layer instance cull"
    );
    let _ = peak_instances;
}

/// The per-layer GPU cull must agree with a CPU reference frustum test: a caster
/// inside a cascade's window still casts (no popping at the cascade edge).
#[test]
fn shadow_layer_instance_cull_matches_cpu_reference() {
    let Some((mut harness, draws)) = build_harness(true, SceneShape::Clusters) else {
        eprintln!("skip shadow instance-cull reference test: no wgpu adapter");
        return;
    };
    // The cull is what's under test, not the scheduler: run every cascade this
    // frame so the far cascade's readback is available.
    set_cascade_budget(Some(MAX_SHADOW_RAY_CASCADES));
    for _ in 0..3 {
        harness.frame(&draws, camera_at(0.0));
    }
    // A big jump invalidates every cascade, so all four render this frame and
    // the last one to run is the far cascade.
    let stats = harness.frame(&draws, camera_at(-100.0));
    assert_eq!(
        stats.layers, MAX_SHADOW_RAY_CASCADES as u32,
        "a camera jump must re-render every cascade"
    );
    assert_eq!(stats.culled_layers, stats.layers);
    let mut totals: Vec<(u64, u64)> = Vec::new();
    let gpu = &harness.gpu;
    // Reference: count instances whose world sphere passes each cascade's
    // frustum on the CPU, the same test multimesh_cull.wgsl runs per instance.
    for cascade in 0..MAX_SHADOW_RAY_CASCADES {
        let Some(frustum) = gpu.shadow_camera_frustums.get(cascade) else {
            continue;
        };
        let reference = cpu_reference_visible(gpu, frustum);
        // A cascade with casters in it must not be empty, and the CPU batch
        // pre-cull must never drop a batch the reference says is visible.
        let batch_survivors: u64 = gpu
            .multimesh_batches
            .iter()
            .filter(|batch| batch.casts_shadows && !batch.mesh_blend)
            .filter(|batch| {
                super::prepare::multimesh_world_bounds(
                    batch,
                    &gpu.staged_multimesh_draw_params,
                    &gpu.staged_multimesh_instances,
                )
                .is_none_or(|(center, radius)| sphere_in_frustum(center, radius, frustum))
            })
            .map(|batch| u64::from(batch.instance_count))
            .sum();
        println!(
            "cascade {cascade}: cpu-reference visible instances={reference} \
             batch-cull survivors={batch_survivors}"
        );
        assert!(
            batch_survivors >= reference,
            "cascade {cascade}: batch pre-cull dropped instances the frustum keeps \
             ({batch_survivors} < {reference})"
        );
        totals.push((reference, batch_survivors));
    }

    // The shadow indirect buffer holds the LAST rendered layer's survivor
    // counts (one buffer set, recompacted per layer). Read them back and hold
    // the GPU cull to the CPU reference: too few = a caster at the cascade edge
    // stopped casting (popping), too many = the cull is not actually culling.
    let last = MAX_SHADOW_RAY_CASCADES - 1;
    let gpu_counts = harness.read_shadow_indirect_counts();
    let gpu_total: u64 = harness
        .gpu
        .multimesh_batches
        .iter()
        .enumerate()
        .filter(|(_, batch)| batch.casts_shadows && !batch.mesh_blend)
        .map(|(i, _)| u64::from(gpu_counts.get(i).copied().unwrap_or(0)))
        .sum();
    let (reference, batch_survivors) = totals[last];
    println!(
        "cascade {last} readback: gpu-cull survivors={gpu_total} \
         cpu-reference={reference} batch-cull={batch_survivors}"
    );
    // Same math on the same data, so the two agree bar float rounding at the
    // plane boundaries.
    let tolerance = (reference / 200).max(8);
    assert!(
        gpu_total + tolerance >= reference,
        "per-layer cull dropped casters the frustum keeps ({gpu_total} < {reference})"
    );
    assert!(
        gpu_total <= reference + tolerance,
        "per-layer cull kept more than the frustum ({gpu_total} > {reference})"
    );
    assert!(
        gpu_total < batch_survivors,
        "per-layer cull submitted no fewer instances than the batch pre-cull \
         ({gpu_total} vs {batch_survivors})"
    );
    set_cascade_budget(None);
}

/// Where the per-layer instance cull earns its keep: a few world-spanning
/// fields. The CPU batch sphere cull is all-or-nothing per batch, so every
/// cascade keeps every field and rasterizes the whole instance set; the
/// per-instance cull cuts each layer down to what its window actually holds.
#[test]
fn wide_field_shadow_casters_cull_per_instance() {
    let Some((mut harness, draws)) = build_harness(true, SceneShape::WideFields) else {
        eprintln!("skip wide-field shadow cull test: no wgpu adapter");
        return;
    };
    // Measuring the cull, not the scheduler: every cascade renders this frame.
    set_cascade_budget(Some(MAX_SHADOW_RAY_CASCADES));
    for _ in 0..3 {
        harness.frame(&draws, camera_at(0.0));
    }
    let total_instances = harness.gpu.staged_multimesh_instances.len() as u64;
    let stats = harness.frame(&draws, camera_at(-100.0));
    assert_eq!(
        stats.culled_layers, stats.layers,
        "every re-rendered layer must go through the per-layer instance cull"
    );
    // Batch-level submission: what the layer would have drawn without the
    // per-instance cull (the CPU sphere cull kept every field).
    let batch_submission = stats.instances;
    let gpu_counts = harness.read_shadow_indirect_counts();
    let culled_submission: u64 = harness
        .gpu
        .multimesh_batches
        .iter()
        .enumerate()
        .filter(|(_, batch)| batch.casts_shadows && !batch.mesh_blend)
        .map(|(i, _)| u64::from(gpu_counts.get(i).copied().unwrap_or(0)))
        .sum();
    let last_layer_batch_submission = batch_submission / stats.layers.max(1) as u64;
    // The GPU cull reproduces the CPU reference exactly (pinned by
    // `shadow_layer_instance_cull_matches_cpu_reference`), so summing the
    // reference over the rendered layers gives this frame's post-cull total.
    let culled_total: u64 = (0..stats.layers as usize)
        .filter_map(|layer| harness.gpu.shadow_camera_frustums.get(layer))
        .map(|frustum| cpu_reference_visible(&harness.gpu, frustum))
        .sum();
    println!(
        "wide fields ({} x {} = {total_instances} instances): layers={} \
         batch-level submission={batch_submission} (last layer \
         {last_layer_batch_submission}) -> per-instance cull {culled_total} \
         (far layer readback {culled_submission})",
        SceneShape::WideFields.fields(),
        SceneShape::WideFields.instances(),
        stats.layers,
    );
    assert_eq!(
        batch_submission,
        total_instances * stats.layers as u64,
        "batch-level culling keeps every world-spanning field in every layer -- \
         that is the pathology the per-instance cull replaces"
    );
    assert!(
        culled_total * 2 < batch_submission,
        "per-instance cull did not halve the frame's caster submission \
         ({culled_total} vs {batch_submission})"
    );
    assert!(
        culled_submission < last_layer_batch_submission,
        "the far layer must submit fewer instances than its whole batch set \
         ({culled_submission} vs {last_layer_batch_submission})"
    );
    set_cascade_budget(None);
}

/// Lever 1: the cascade round-robin budget. A translating camera drags several
/// cascade windows across their snap grid in the same frame; the budget caps how
/// many of them re-raster per frame and lets the rest ride one more frame on
/// their cached depth. Measured on the same 102_400-caster walk as
/// `moving_camera_multimesh_shadow_submission`, budget off vs on, plus the
/// starvation and settle guarantees the deferral rests on.
#[test]
fn cascade_round_robin_budget_spreads_shadow_renders() {
    let Some((mut harness, draws)) = build_harness(true, SceneShape::Clusters) else {
        eprintln!("skip cascade round-robin budget test: no wgpu adapter");
        return;
    };
    const WALK: u32 = 24;
    const SPEED: f32 = 2.0;

    // Which cascades advanced their window this frame: a rendered cascade gets
    // its new scene uniform applied, a deferred one keeps the old one.
    fn cascade_scenes(harness: &ShadowHarness) -> Vec<Option<Scene3DUniform>> {
        harness.gpu.last_shadow_scenes[..MAX_SHADOW_RAY_CASCADES].to_vec()
    }

    struct WalkResult {
        layers: u32,
        instances: u64,
        peak_layers: u32,
        time: std::time::Duration,
        per_cascade: [u32; MAX_SHADOW_RAY_CASCADES],
        // Longest run of consecutive frames a cascade ASKED for a re-render and
        // did not get one. Frames where its window simply did not move are not
        // waiting, so this reads the scheduler's own age counters rather than
        // inferring from the applied scenes.
        worst_wait: [u32; MAX_SHADOW_RAY_CASCADES],
    }

    fn walk(
        harness: &mut ShadowHarness,
        draws: &[Draw3DInstance],
        first_step: u32,
        budget: Option<usize>,
        snap_steps: Option<f32>,
    ) -> WalkResult {
        set_cascade_budget(budget);
        set_cascade_snap_steps(snap_steps);
        let mut result = WalkResult {
            layers: 0,
            instances: 0,
            peak_layers: 0,
            time: std::time::Duration::ZERO,
            per_cascade: [0; MAX_SHADOW_RAY_CASCADES],
            worst_wait: [0; MAX_SHADOW_RAY_CASCADES],
        };
        let mut times = Vec::new();
        for step in first_step..first_step + WALK {
            let before = cascade_scenes(harness);
            let (stats, elapsed) = harness.timed_frame(draws, camera_at(step as f32 * SPEED));
            let after = cascade_scenes(harness);
            result.layers += stats.layers;
            result.instances += stats.instances;
            result.peak_layers = result.peak_layers.max(stats.layers);
            times.push(elapsed);
            for cascade in 0..MAX_SHADOW_RAY_CASCADES {
                if before[cascade] != after[cascade] {
                    result.per_cascade[cascade] += 1;
                }
                result.worst_wait[cascade] =
                    result.worst_wait[cascade].max(harness.gpu.shadow_cascade_defer_age[cascade]);
            }
        }
        result.time = median(times);
        set_cascade_budget(None);
        set_cascade_snap_steps(None);
        result
    }

    for _ in 0..3 {
        harness.frame(&draws, camera_at(0.0));
    }
    let total_instances = harness.gpu.staged_multimesh_instances.len();

    // Lever 2 on the same repro, with the budget out of the way: the old snap
    // granularity vs the shipped one.
    let snap_32 = walk(
        &mut harness,
        &draws,
        1,
        Some(MAX_SHADOW_RAY_CASCADES),
        Some(32.0),
    );
    // Budget off: every cascade whose window moved re-rasters the same frame.
    let off = walk(
        &mut harness,
        &draws,
        WALK + 1,
        Some(MAX_SHADOW_RAY_CASCADES),
        None,
    );
    // Budget on (shipped value): at most CASCADE_RENDER_BUDGET cascades a frame.
    let on = walk(&mut harness, &draws, WALK * 2 + 1, None, None);

    println!(
        "cascade snap steps on the repro ({WALK} moving frames, budget off): \
         32 -> layers={} instances={} median frame={:?} per-cascade={:?}; \
         16 -> layers={} instances={} median frame={:?} per-cascade={:?}",
        snap_32.layers,
        snap_32.instances,
        snap_32.time,
        snap_32.per_cascade,
        off.layers,
        off.instances,
        off.time,
        off.per_cascade,
    );
    assert!(
        off.layers <= snap_32.layers,
        "the coarser snap grid must not re-render cascades more often \
         ({} vs {})",
        off.layers,
        snap_32.layers
    );

    println!(
        "cascade budget over {WALK} moving frames @ {SPEED} u/frame \
         ({total_instances} caster instances):\n  off: layers={} (peak {}/frame) \
         instances={} median frame={:?} per-cascade renders={:?}\n  on:  layers={} \
         (peak {}/frame) instances={} median frame={:?} per-cascade renders={:?} \
         worst wait={:?}",
        off.layers,
        off.peak_layers,
        off.instances,
        off.time,
        off.per_cascade,
        on.layers,
        on.peak_layers,
        on.instances,
        on.time,
        on.per_cascade,
        on.worst_wait,
    );

    assert!(
        on.peak_layers <= 2,
        "the budget must cap a frame at 2 cascade re-renders, saw {}",
        on.peak_layers
    );
    assert!(
        on.layers <= off.layers,
        "the budget must not add cascade re-renders ({} vs {})",
        on.layers,
        off.layers
    );
    assert!(
        on.instances < off.instances,
        "fewer cascade re-renders must mean fewer caster instances submitted \
         ({} vs {})",
        on.instances,
        off.instances
    );
    // No starvation: every cascade still gets served, within the age cap.
    for cascade in 0..MAX_SHADOW_RAY_CASCADES {
        assert!(
            on.per_cascade[cascade] > 0,
            "cascade {cascade} never rendered across {WALK} moving frames"
        );
        assert!(
            on.worst_wait[cascade] <= 3,
            "cascade {cascade} waited {} frames, past the 3-frame promotion cap",
            on.worst_wait[cascade]
        );
    }
    assert_eq!(
        on.worst_wait[0], 0,
        "cascade 0 holds the closest shadows and must never be deferred"
    );

    // Settle: once the camera parks, the pending cascades flush and the cache
    // takes over again -- the deferral must not leave a layer stale forever.
    let park = camera_at((WALK * 3 + 1) as f32 * SPEED);
    let mut settle_frames = 0;
    let mut settle_layers = 0;
    for _ in 0..8 {
        let stats = harness.frame(&draws, park.clone());
        if stats.layers == 0 && harness.gpu.shadow_cascade_defer_count == 0 {
            break;
        }
        settle_layers += stats.layers;
        settle_frames += 1;
    }
    println!("parked camera settled after {settle_frames} frames ({settle_layers} layers)");
    assert!(
        settle_frames <= 4,
        "a parked camera must flush every pending cascade quickly, took {settle_frames} frames"
    );
    let parked = harness.frame(&draws, park);
    assert_eq!(
        parked.layers, 0,
        "a settled parked camera must keep every shadow layer cached"
    );
    assert_eq!(harness.gpu.shadow_cascade_defer_count, 0);
}

/// Adapters without INDIRECT_FIRST_INSTANCE keep the identity draw path: the
/// shadow visible-index buffer stays primed as `visible_indices[i] == i` and
/// every layer draws its batches directly. Pins that the cull's takeover of
/// that buffer did not leave the fallback reading clobbered rows.
#[test]
fn shadow_casters_fall_back_to_identity_draws_without_indirect_first_instance() {
    let Some((mut harness, draws)) = build_harness(false, SceneShape::Clusters) else {
        eprintln!("skip shadow identity fallback test: no wgpu adapter");
        return;
    };
    for _ in 0..3 {
        harness.frame(&draws, camera_at(0.0));
    }
    let total_instances = harness.gpu.staged_multimesh_instances.len();
    assert!(total_instances >= MULTIMESH_CULL_MIN_INSTANCES);
    let stats = harness.frame(&draws, camera_at(-100.0));
    assert_eq!(
        stats.culled_layers, 0,
        "the cull needs indirect first_instance; without it the identity path runs"
    );
    assert!(stats.layers > 0 && stats.instances > 0);
    assert_eq!(
        harness.gpu.multimesh_shadow_identity_uploaded_len, total_instances,
        "the identity prime must cover the whole instance set on the fallback path"
    );
}
