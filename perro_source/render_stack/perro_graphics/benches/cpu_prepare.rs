use criterion::{BatchSize, BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_graphics::{GraphicsBackend, PerroGraphics};
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_render_bridge::{
    Command2D, Command3D, DenseInstancePose3D, LODOptions3D, Material3D, Mesh3D,
    MeshBlendOptions3D, MeshSurfaceBinding3D, RenderBridge, RenderCommand, RenderEvent,
    RenderRequestID, ResourceCommand, RuntimeMeshVertex, Sprite2DCommand, Water2DState,
    WaterIdleModeState, WaterShapeState,
};
use perro_structs::{BitMask, Color};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct BenchAllocator;
static COUNT_ALLOCS: AtomicBool = AtomicBool::new(false);
static ALLOC_CALLS: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static ALLOCATOR: BenchAllocator = BenchAllocator;

// Count requests, including reallocations, only around the sampled frame.
unsafe impl GlobalAlloc for BenchAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNT_ALLOCS.load(Ordering::Relaxed) {
            ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if COUNT_ALLOCS.load(Ordering::Relaxed) {
            ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(size, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, size) }
    }
}

#[inline]
fn color(v: [f32; 4]) -> Color {
    v.into()
}

fn rect_command(i: u32) -> RenderCommand {
    rect_command_offset(i, 0.0)
}

fn rect_command_offset(i: u32, offset: f32) -> RenderCommand {
    let x = (i % 256) as f32 * 4.0;
    let y = (i / 256) as f32 * 4.0;
    RenderCommand::TwoD(Command2D::UpsertRect {
        node: NodeID::from_parts(i + 1, 0),
        rect: perro_render_bridge::Rect2DCommand {
            center: [x + offset, y],
            size: [3.0, 3.0],
            color: color([0.2, 0.7, 1.0, 1.0]),
            z_index: i as i32,
        },
    })
}

fn sprite_command(i: u32, texture: TextureID) -> RenderCommand {
    sprite_command_z(i, texture, 0)
}

fn sprite_command_z(i: u32, texture: TextureID, z_index: i32) -> RenderCommand {
    let x = (i % 256) as f32 * 4.0;
    let y = (i / 256) as f32 * 4.0;
    RenderCommand::TwoD(Command2D::UpsertSprite {
        node: NodeID::from_parts(i + 1, 0),
        sprite: Sprite2DCommand {
            texture,
            model: [[16.0, 0.0, 0.0], [0.0, 16.0, 0.0], [x, y, 1.0]],
            tint: color([1.0, 1.0, 1.0, 1.0]),
            uv_min: [0.0, 0.0],
            uv_max: [1.0, 1.0],
            uv_normalized: true,
            size: [16.0, 16.0],
            z_index,
        },
    })
}

fn draw_3d_command(i: u32, mesh: MeshID, material: MaterialID) -> RenderCommand {
    draw_3d_command_with_blend(i, mesh, material, MeshBlendOptions3D::default())
}

fn draw_3d_command_with_blend(
    i: u32,
    mesh: MeshID,
    material: MaterialID,
    blend: MeshBlendOptions3D,
) -> RenderCommand {
    let x = (i % 256) as f32 * 2.0;
    let z = (i / 256) as f32 * 2.0;
    RenderCommand::ThreeD(Box::new(Command3D::Draw {
        mesh,
        surfaces: Arc::from([MeshSurfaceBinding3D {
            material: Some(material),
            overrides: Arc::from([]),
            modulate: Color::WHITE,
        }]),
        node: NodeID::from_parts(i + 1, 0),
        model: [
            [1.0, 0.0, 0.0, x],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, z],
            [0.0, 0.0, 0.0, 1.0],
        ],
        skeleton: None,
        blend_shape_weights: Arc::from([]),
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend,
        cast_shadows: true,
        receive_shadows: true,
    }))
}

fn draw_3d_dense_command_with_blend(
    count: u32,
    mesh: MeshID,
    material: MaterialID,
    blend: MeshBlendOptions3D,
) -> RenderCommand {
    let instances: Arc<[DenseInstancePose3D]> = (0..count)
        .map(|i| DenseInstancePose3D {
            position: [(i % 2048) as f32 * 0.08, 0.0, (i / 2048) as f32 * 0.08],
            scale: [1.0, 1.0, 1.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            has_blend_shape_weight_override: false,
            blend_shape_weights: Arc::from([]),
        })
        .collect::<Vec<_>>()
        .into();
    RenderCommand::ThreeD(Box::new(Command3D::DrawMultiDense {
        mesh,
        surfaces: Arc::from([MeshSurfaceBinding3D {
            material: Some(material),
            overrides: Arc::from([]),
            modulate: Color::WHITE,
        }]),
        node: NodeID::from_parts(900_000, 0),
        node_model: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        instance_scale: 1.0,
        instances,
        blend_shape_weights: Arc::from([]),
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend,
        cast_shadows: true,
        receive_shadows: true,
    }))
}

fn water_command(i: u32, _resolution: u32, impacts: u32) -> RenderCommand {
    water_command_with_idle(i, impacts, WaterIdleModeState::Sine)
}

fn water_command_with_idle(i: u32, impacts: u32, idle_mode: WaterIdleModeState) -> RenderCommand {
    let x = (i % 64) as f32 * 36.0;
    let y = (i / 64) as f32 * 36.0;
    RenderCommand::TwoD(Command2D::UpsertWater {
        node: NodeID::from_parts(500_000 + i, 0),
        water: Box::new(Water2DState {
            model: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [x, y, 1.0]],
            z_index: i as i32,
            paused: false,
            simulation_time: 0.0,
            simulation_delta: 1.0 / 60.0,
            size: [32.0, 32.0],
            shape: WaterShapeState::Rect,
            quality: perro_structs::WaterQuality::Ultra,
            depth: 4.0,
            flow: [0.1, 0.0],
            wind: [1.0, 0.2],
            idle_mode,
            wave_speed: 1.0,
            wave_scale: 1.0,
            wave_length: 18.0,
            damping: 0.985,
            wake_strength: 1.0,
            foam_strength: 0.65,
            sample_readback_rate: 30.0,
            collision_layers: BitMask::with([1]),
            collision_mask: BitMask::NONE,
            deep_color: color([0.02, 0.16, 0.28, 0.86]),
            shallow_color: color([0.08, 0.46, 0.62, 0.48]),
            shallow_depth: -1.0,
            sky_bias_ratio: 0.0,
            transparency: 0.24,
            reflectivity: 0.46,
            roughness: 0.18,
            fresnel_power: 5.0,
            normal_strength: 1.15,
            ripple_scale: 1.0,
            foam_color: color([0.86, 0.96, 1.0, 1.0]),
            foam_amount: 0.72,
            crest_foam_threshold: 0.58,
            caustic_strength: 0.20,
            refraction_strength: 0.12,
            scattering_strength: 0.18,
            distance_fog_strength: 0.32,
            coastline_foam_color: color([0.9, 0.97, 1.0, 1.0]),
            coastline_foam_strength: if impacts > 0 { 0.75 } else { 0.0 },
            coastline_foam_width: 1.5,
            coastline_cutoff_softness: 0.25,
            coastline_wave_reflection: 0.45,
            coastline_wave_damping: 0.35,
            coastline_edge_noise: 0.2,
            debug: false,
            links: Arc::from([]),
            queries: Arc::from([]),
            impacts: (0..impacts)
                .map(|j| perro_render_bridge::WaterImpact2D {
                    position: [(j % 32) as f32, (j / 32) as f32],
                    velocity: [1.0, -2.0],
                    strength: 1.0 + j as f32 * 0.01,
                    radius: 2.0,
                    cavitation: 0.0,
                })
                .collect::<Vec<_>>()
                .into(),
            coastline_shapes: Arc::from([]),
        }),
    })
}

fn tiny_mesh() -> std::sync::Arc<Mesh3D> {
    std::sync::Arc::new(Mesh3D {
        vertices: vec![
            RuntimeMeshVertex {
                position: [0.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
                paint_uv: [0.0, 0.0],
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0].into(),
            },
            RuntimeMeshVertex {
                position: [1.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 0.0],
                paint_uv: [1.0, 0.0],
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0].into(),
            },
            RuntimeMeshVertex {
                position: [0.0, 1.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 1.0],
                paint_uv: [0.0, 1.0],
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0].into(),
            },
        ],
        indices: vec![0, 1, 2],
        surface_ranges: vec![],
        blend_shapes: vec![],
    })
}

fn drain_texture(graphics: &mut PerroGraphics) -> TextureID {
    let mut events = Vec::new();
    graphics.drain_events(&mut events);
    events
        .into_iter()
        .find_map(|event| match event {
            RenderEvent::TextureCreated { id, .. } => Some(id),
            _ => None,
        })
        .expect("texture event")
}

fn drain_mesh_material(graphics: &mut PerroGraphics) -> (MeshID, MaterialID) {
    let mut events = Vec::new();
    graphics.drain_events(&mut events);
    let mut mesh = MeshID::nil();
    let mut material = MaterialID::nil();
    for event in events {
        match event {
            RenderEvent::MeshCreated { id, .. } => mesh = id,
            RenderEvent::MaterialCreated { id, .. } => material = id,
            _ => {}
        }
    }
    assert!(!mesh.is_nil());
    assert!(!material.is_nil());
    (mesh, material)
}

fn create_texture(graphics: &mut PerroGraphics) -> TextureID {
    graphics.submit(RenderCommand::Resource(Box::new(
        ResourceCommand::CreateRuntimeTexture {
            request: RenderRequestID::new(1),
            id: TextureID::nil(),
            source: "runtime://cpu-prepare-bench".to_string(),
            reserved: true,
            width: 1,
            height: 1,
            rgba: Arc::from([255, 255, 255, 255]),
        },
    )));
    graphics.draw_frame();
    drain_texture(graphics)
}

fn create_mesh_material(graphics: &mut PerroGraphics) -> (MeshID, MaterialID) {
    graphics.submit_many([
        RenderCommand::Resource(Box::new(ResourceCommand::CreateRuntimeMesh {
            request: RenderRequestID::new(2),
            id: MeshID::nil(),
            source: "__bench_mesh__".to_string(),
            reserved: true,
            mesh: tiny_mesh(),
        })),
        RenderCommand::Resource(Box::new(ResourceCommand::CreateMaterial {
            request: RenderRequestID::new(3),
            id: MaterialID::nil(),
            material: Material3D::default().into(),
            source: Some("__bench_material__".to_string()),
            reserved: true,
        })),
    ]);
    graphics.draw_frame();
    drain_mesh_material(graphics)
}

fn create_material(graphics: &mut PerroGraphics, request: u64, source: &str) -> MaterialID {
    graphics.submit(RenderCommand::Resource(Box::new(
        ResourceCommand::CreateMaterial {
            request: RenderRequestID::new(request),
            id: MaterialID::nil(),
            material: Material3D::default().into(),
            source: Some(source.to_string()),
            reserved: true,
        },
    )));
    graphics.draw_frame();
    let mut events = Vec::new();
    graphics.drain_events(&mut events);
    events
        .into_iter()
        .find_map(|event| match event {
            RenderEvent::MaterialCreated { id, .. } => Some(id),
            _ => None,
        })
        .expect("material event")
}

fn bench_2d_rect_prepare(c: &mut Criterion) {
    let mut group = c.benchmark_group("graphics_2d_rect_prepare");
    for count in [1_000u32, 10_000, 100_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            let mut graphics = PerroGraphics::new();
            b.iter_batched(
                || (0..count).map(rect_command).collect::<Vec<_>>(),
                |commands| {
                    graphics.submit_many(commands);
                    let timing = graphics.draw_frame_timed().expect("timing");
                    black_box(timing.prepare_cpu);
                },
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

fn bench_2d_rect_sparse_updates(c: &mut Criterion) {
    const RETAINED: u32 = 100_000;
    const UPDATED: u32 = 10_000;
    let mut graphics = PerroGraphics::new();
    graphics.submit_many((0..RETAINED).map(rect_command));
    graphics.draw_frame();
    let mut offset = 0.0;

    c.bench_function("graphics_2d_rect_sparse_updates/10000_of_100000", |b| {
        b.iter_batched(
            || {
                offset = if offset == 0.0 { 0.5 } else { 0.0 };
                (0..UPDATED)
                    .map(|i| rect_command_offset(i * 2, offset))
                    .collect::<Vec<_>>()
            },
            |commands| {
                graphics.submit_many(commands);
                let timing = graphics.draw_frame_timed().expect("timing");
                black_box(timing.prepare_cpu);
            },
            BatchSize::LargeInput,
        );
    });
}

fn bench_2d_sprite_prepare(c: &mut Criterion) {
    let mut group = c.benchmark_group("graphics_2d_sprite_prepare_same_z");
    for count in [1_000u32, 10_000, 100_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            let mut graphics = PerroGraphics::new();
            let texture = create_texture(&mut graphics);
            b.iter_batched(
                || {
                    (0..count)
                        .map(|i| sprite_command(i, texture))
                        .collect::<Vec<_>>()
                },
                |commands| {
                    graphics.submit_many(commands);
                    let timing = graphics.draw_frame_timed().expect("timing");
                    black_box(timing.prepare_cpu);
                },
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

fn bench_2d_sprite_prepare_unique_z(c: &mut Criterion) {
    let mut group = c.benchmark_group("graphics_2d_sprite_prepare_unique_z");
    for count in [1_000u32, 10_000, 100_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            let mut graphics = PerroGraphics::new();
            let texture = create_texture(&mut graphics);
            b.iter_batched(
                || {
                    (0..count)
                        .map(|i| sprite_command_z(i, texture, i as i32))
                        .collect::<Vec<_>>()
                },
                |commands| {
                    graphics.submit_many(commands);
                    let timing = graphics.draw_frame_timed().expect("timing");
                    black_box(timing.prepare_cpu);
                    black_box(timing.draw_calls_2d);
                },
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

fn bench_3d_draw_prepare(c: &mut Criterion) {
    let mut group = c.benchmark_group("graphics_3d_draw_prepare");
    for count in [1_000u32, 10_000, 100_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            let mut graphics = PerroGraphics::new();
            let (mesh, material) = create_mesh_material(&mut graphics);
            b.iter_batched(
                || {
                    (0..count)
                        .map(|i| draw_3d_command(i, mesh, material))
                        .collect::<Vec<_>>()
                },
                |commands| {
                    graphics.submit_many(commands);
                    let timing = graphics.draw_frame_timed().expect("timing");
                    black_box(timing.draw_instances_3d);
                    black_box(timing.prepare_cpu);
                },
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

fn bench_3d_blend_prepare(c: &mut Criterion) {
    let mut group = c.benchmark_group("graphics_3d_blend_prepare");
    for count in [1_000u32, 10_000, 100_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            let mut graphics = PerroGraphics::new();
            let (mesh, material) = create_mesh_material(&mut graphics);
            let blend = MeshBlendOptions3D {
                enabled: true,
                screen_blending: true,
                normal_blending: false,
                blend_layers: BitMask::with([1]),
                blend_mask: BitMask::with([1]),
                distance: 0.25,
                min_distance: 0.02,
                noise_factor: 0.35,
                noise_scale: 8.0,
                slope_factor: 2.0,
                strength: 1.0,
                salt_instances: true,
            };
            b.iter_batched(
                || {
                    (0..count)
                        .map(|i| draw_3d_command_with_blend(i, mesh, material, blend))
                        .collect::<Vec<_>>()
                },
                |commands| {
                    graphics.submit_many(commands);
                    let timing = graphics.draw_frame_timed().expect("timing");
                    black_box(timing.draw_instances_3d);
                    black_box(timing.prepare_cpu);
                },
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

// Exercise the retained cache with real transform changes. Command construction
// stays outside the timed region; submission and frame preparation stay inside.
fn bench_3d_retained_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("graphics_3d_retained_updates");
    for count in [1_000u32, 10_000] {
        for (name, updated) in [
            ("idle", 0),
            ("one", 1),
            ("percent", count / 100),
            ("all", count),
        ] {
            group.bench_with_input(BenchmarkId::new(name, count), &updated, |b, &updated| {
                let mut graphics = PerroGraphics::new();
                let (mesh, material) = create_mesh_material(&mut graphics);
                graphics.submit_many((0..count).map(|i| draw_3d_command(i, mesh, material)));
                graphics.draw_frame();
                let mut offset = 0.0;
                b.iter_batched(
                    || {
                        offset = if offset == 0.0 { 0.5 } else { 0.0 };
                        (0..updated)
                            .map(|i| {
                                let mut command =
                                    draw_3d_command(i * (count / updated), mesh, material);
                                if let RenderCommand::ThreeD(command) = &mut command
                                    && let Command3D::Draw { model, .. } = command.as_mut()
                                {
                                    model[0][3] += offset;
                                }
                                command
                            })
                            .collect::<Vec<_>>()
                    },
                    |commands| {
                        graphics.submit_many(commands);
                        black_box(graphics.draw_frame_timed().expect("timing"));
                    },
                    BatchSize::PerIteration,
                );
            });
        }
    }
    group.finish();
}

// Measure the CPU half separately from the gpu_frame resource cases. These
// commands change no pixels, but must still preserve setter events and refs.
fn bench_3d_resource_metadata(c: &mut Criterion) {
    let mut group = c.benchmark_group("graphics_3d_resource_metadata");
    for count in [1_000u32, 10_000] {
        for name in ["identical_material", "mesh_reservation"] {
            group.bench_with_input(BenchmarkId::new(name, count), &count, |b, &count| {
                let mut graphics = PerroGraphics::new();
                let (mesh, material) = create_mesh_material(&mut graphics);
                graphics.submit_many((0..count).map(|i| draw_3d_command(i, mesh, material)));
                graphics.draw_frame();
                let mut events = Vec::new();
                b.iter(|| {
                    let command = match name {
                        "identical_material" => ResourceCommand::WriteMaterialData {
                            id: material,
                            material: Material3D::default().into(),
                        },
                        _ => ResourceCommand::SetMeshReserved {
                            id: mesh,
                            reserved: true,
                        },
                    };
                    graphics.submit(RenderCommand::Resource(Box::new(command)));
                    black_box(graphics.draw_frame_timed().expect("timing"));
                    graphics.drain_events(&mut events);
                    events.clear();
                });
            });
        }
    }
    group.finish();
}

fn bench_3d_bulk_remove(c: &mut Criterion) {
    let mut group = c.benchmark_group("graphics_3d_bulk_remove");
    for count in [1_000u32, 10_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter_batched_ref(
                || {
                    let mut graphics = PerroGraphics::new();
                    let (mesh, material) = create_mesh_material(&mut graphics);
                    graphics.submit_many((0..count).map(|i| draw_3d_command(i, mesh, material)));
                    graphics.draw_frame();
                    let commands = (0..count)
                        .map(|i| {
                            RenderCommand::ThreeD(Box::new(Command3D::RemoveNode {
                                node: NodeID::from_parts(i + 1, 0),
                            }))
                        })
                        .collect::<Vec<_>>();
                    (graphics, commands)
                },
                |(graphics, commands)| {
                    graphics.submit_many(commands.drain(..));
                    black_box(graphics.draw_frame_timed().expect("timing"));
                },
                BatchSize::PerIteration,
            );
        });
    }
    group.finish();
}

fn bench_3d_revision_gate_paths(c: &mut Criterion) {
    const RETAINED: u32 = 10_000;
    let mut group = c.benchmark_group("graphics_3d_revision_gate_paths");

    group.bench_function("sparse_transform_1_of_10000", |b| {
        let mut graphics = PerroGraphics::new();
        let (mesh, material) = create_mesh_material(&mut graphics);
        graphics.submit_many((0..RETAINED).map(|i| draw_3d_command(i, mesh, material)));
        graphics.draw_frame();
        let mut offset = 0.0;
        b.iter_batched(
            || {
                offset = if offset == 0.0 { 0.5 } else { 0.0 };
                let mut command = draw_3d_command(0, mesh, material);
                if let RenderCommand::ThreeD(command) = &mut command
                    && let Command3D::Draw { model, .. } = command.as_mut()
                {
                    model[0][3] += offset;
                }
                command
            },
            |command| {
                graphics.submit(command);
                black_box(graphics.draw_frame_timed().expect("timing"));
            },
            BatchSize::PerIteration,
        );
    });

    group.bench_function("binding_change_1_of_10000", |b| {
        let mut graphics = PerroGraphics::new();
        let (mesh, material_a) = create_mesh_material(&mut graphics);
        let material_b = create_material(&mut graphics, 4, "__bench_material_b__");
        graphics.submit_many((0..RETAINED).map(|i| draw_3d_command(i, mesh, material_a)));
        graphics.draw_frame();
        let mut use_b = false;
        b.iter_batched(
            || {
                use_b = !use_b;
                draw_3d_command(0, mesh, if use_b { material_b } else { material_a })
            },
            |command| {
                graphics.submit(command);
                black_box(graphics.draw_frame_timed().expect("timing"));
            },
            BatchSize::PerIteration,
        );
    });

    group.bench_function("unready_binding_fallback_1_of_10000", |b| {
        let mut graphics = PerroGraphics::new();
        let (mesh, material) = create_mesh_material(&mut graphics);
        graphics.submit_many((0..RETAINED).map(|i| draw_3d_command(i, mesh, material)));
        graphics.draw_frame();
        let missing_mesh = MeshID::from_parts(123_456, 0);
        let missing_material = MaterialID::from_parts(123_456, 0);
        let mut offset = 0.0;
        b.iter_batched(
            || {
                offset = if offset == 0.0 { 0.5 } else { 0.0 };
                let mut command = draw_3d_command(0, missing_mesh, missing_material);
                if let RenderCommand::ThreeD(command) = &mut command
                    && let Command3D::Draw { model, .. } = command.as_mut()
                {
                    model[0][3] += offset;
                }
                command
            },
            |command| {
                graphics.submit(command);
                black_box(graphics.draw_frame_timed().expect("timing"));
            },
            BatchSize::PerIteration,
        );
    });

    group.bench_function("dense_instance_count_change", |b| {
        let mut graphics = PerroGraphics::new();
        let (mesh, material) = create_mesh_material(&mut graphics);
        graphics.submit(draw_3d_dense_command_with_blend(
            100_000,
            mesh,
            material,
            MeshBlendOptions3D::default(),
        ));
        graphics.draw_frame();
        let mut count = 100_000;
        b.iter_batched(
            || {
                count = if count == 100_000 { 100_001 } else { 100_000 };
                draw_3d_dense_command_with_blend(
                    count,
                    mesh,
                    material,
                    MeshBlendOptions3D::default(),
                )
            },
            |command| {
                graphics.submit(command);
                black_box(graphics.draw_frame_timed().expect("timing"));
            },
            BatchSize::LargeInput,
        );
    });

    group.bench_function("single_add_remove_at_10000", |b| {
        let mut graphics = PerroGraphics::new();
        let (mesh, material) = create_mesh_material(&mut graphics);
        graphics.submit_many((0..RETAINED).map(|i| draw_3d_command(i, mesh, material)));
        graphics.draw_frame();
        let mut add = false;
        b.iter_batched(
            || {
                add = !add;
                if add {
                    draw_3d_command(RETAINED, mesh, material)
                } else {
                    RenderCommand::ThreeD(Box::new(Command3D::RemoveNode {
                        node: NodeID::from_parts(RETAINED + 1, 0),
                    }))
                }
            },
            |command| {
                graphics.submit(command);
                black_box(graphics.draw_frame_timed().expect("timing"));
            },
            BatchSize::PerIteration,
        );
    });

    group.finish();
}

// Run with PERRO_BENCH_ALLOCS=1 and a nonmatching Criterion filter to print
// frame-only allocation requests without running the timing suite.
fn bench_3d_allocations(_: &mut Criterion) {
    if std::env::var_os("PERRO_BENCH_ALLOCS").is_none() {
        return;
    }
    for count in [1_000u32, 10_000] {
        for (name, updated) in [
            ("idle", 0),
            ("one", 1),
            ("percent", count / 100),
            ("all", count),
            ("remove_all", count),
        ] {
            let mut graphics = PerroGraphics::new();
            let (mesh, material) = create_mesh_material(&mut graphics);
            graphics.submit_many((0..count).map(|i| draw_3d_command(i, mesh, material)));
            graphics.draw_frame();
            let commands = (0..updated)
                .map(|i| {
                    if name == "remove_all" {
                        RenderCommand::ThreeD(Box::new(Command3D::RemoveNode {
                            node: NodeID::from_parts(i + 1, 0),
                        }))
                    } else {
                        let mut command = draw_3d_command(i * (count / updated), mesh, material);
                        if let RenderCommand::ThreeD(command) = &mut command
                            && let Command3D::Draw { model, .. } = command.as_mut()
                        {
                            model[0][3] += 0.5;
                        }
                        command
                    }
                })
                .collect::<Vec<_>>();
            ALLOC_CALLS.store(0, Ordering::Relaxed);
            ALLOC_BYTES.store(0, Ordering::Relaxed);
            COUNT_ALLOCS.store(true, Ordering::Relaxed);
            graphics.submit_many(commands);
            let timing = graphics.draw_frame_timed().expect("timing");
            COUNT_ALLOCS.store(false, Ordering::Relaxed);
            println!(
                "allocations/{name}/{count}: calls={} bytes={} process={:?} prepare={:?}",
                ALLOC_CALLS.load(Ordering::Relaxed),
                ALLOC_BYTES.load(Ordering::Relaxed),
                timing.process_commands,
                timing.prepare_cpu
            );
        }
    }
}

fn bench_3d_blend_dense_prepare(c: &mut Criterion) {
    let mut group = c.benchmark_group("graphics_3d_blend_dense_prepare");
    for count in [100_000u32, 1_000_000, 5_000_000] {
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            let mut graphics = PerroGraphics::new();
            let (mesh, material) = create_mesh_material(&mut graphics);
            let blend = MeshBlendOptions3D {
                enabled: true,
                screen_blending: true,
                normal_blending: false,
                blend_layers: BitMask::with([1]),
                blend_mask: BitMask::with([1]),
                distance: 0.25,
                min_distance: 0.02,
                noise_factor: 0.35,
                noise_scale: 8.0,
                slope_factor: 2.0,
                strength: 1.0,
                salt_instances: true,
            };
            let command = draw_3d_dense_command_with_blend(count, mesh, material, blend);
            b.iter(|| {
                graphics.submit(command.clone());
                let timing = graphics.draw_frame_timed().expect("timing");
                black_box(timing.draw_instances_3d);
                black_box(timing.prepare_cpu);
            });
        });
    }
    group.finish();
}

fn bench_resource_churn(c: &mut Criterion) {
    c.bench_function("graphics_resource_churn_1k", |b| {
        b.iter_batched(
            || {
                (0..1_000u32)
                    .flat_map(|i| {
                        [
                            RenderCommand::Resource(Box::new(ResourceCommand::CreateTexture {
                                request: RenderRequestID::new(i as u64),
                                id: TextureID::nil(),
                                source: format!("__bench_texture_{i}__"),
                                reserved: false,
                            })),
                            RenderCommand::Resource(Box::new(ResourceCommand::CreateMaterial {
                                request: RenderRequestID::new(10_000 + i as u64),
                                id: MaterialID::nil(),
                                material: Material3D::default().into(),
                                source: Some(format!("__bench_material_{i}__")),
                                reserved: false,
                            })),
                        ]
                    })
                    .collect::<Vec<_>>()
            },
            |commands| {
                let mut graphics = PerroGraphics::new();
                graphics.submit_many(commands);
                let timing = graphics.draw_frame_timed().expect("timing");
                black_box(graphics.profile_snapshot());
                black_box(timing.process_commands);
            },
            BatchSize::LargeInput,
        );
    });
}

fn bench_water_prepare(c: &mut Criterion) {
    let mut group = c.benchmark_group("graphics_water_prepare");
    for (count, resolution, impacts) in [
        (1u32, 64u32, 0u32),
        (16, 64, 8),
        (64, 128, 16),
        (128, 256, 32),
    ] {
        group.bench_with_input(
            BenchmarkId::new(
                format!("{count}_water"),
                format!("{resolution}r_{impacts}i"),
            ),
            &(count, resolution, impacts),
            |b, &(count, resolution, impacts)| {
                let mut graphics = PerroGraphics::new();
                b.iter_batched(
                    || {
                        (0..count)
                            .map(|i| water_command(i, resolution, impacts))
                            .collect::<Vec<_>>()
                    },
                    |commands| {
                        graphics.submit_many(commands);
                        let timing = graphics.draw_frame_timed().expect("timing");
                        black_box(timing.prepare_cpu);
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }
    group.finish();
}

fn bench_water_idle_prepare(c: &mut Criterion) {
    let mut group = c.benchmark_group("graphics_water_idle_prepare");
    for (name, idle_mode) in [
        ("calm", WaterIdleModeState::Calm),
        ("sine", WaterIdleModeState::Sine),
        ("chop", WaterIdleModeState::Chop),
        ("storm", WaterIdleModeState::Storm),
        ("river", WaterIdleModeState::River),
    ] {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &idle_mode,
            |b, idle_mode| {
                let mut graphics = PerroGraphics::new();
                b.iter_batched(
                    || vec![water_command_with_idle(0, 2, *idle_mode)],
                    |commands| {
                        graphics.submit_many(commands);
                        let timing = graphics.draw_frame_timed().expect("timing");
                        black_box(timing.prepare_cpu);
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_2d_rect_prepare,
    bench_2d_rect_sparse_updates,
    bench_2d_sprite_prepare,
    bench_2d_sprite_prepare_unique_z,
    bench_3d_draw_prepare,
    bench_3d_retained_updates,
    bench_3d_resource_metadata,
    bench_3d_bulk_remove,
    bench_3d_revision_gate_paths,
    bench_3d_allocations,
    bench_3d_blend_prepare,
    bench_3d_blend_dense_prepare,
    bench_water_prepare,
    bench_water_idle_prepare,
    bench_resource_churn
);
criterion_main!(benches);
