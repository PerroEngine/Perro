use criterion::{BatchSize, BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_graphics::{GraphicsBackend, PerroGraphics};
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_render_bridge::{
    Command2D, Command3D, LODOptions3D, Material3D, Mesh3D, MeshSurfaceBinding3D, RenderBridge,
    RenderCommand, RenderEvent, RenderRequestID, ResourceCommand, RuntimeMeshVertex,
    Sprite2DCommand, Water2DState, WaterIdleModeState,
};
use std::sync::Arc;

fn rect_command(i: u32) -> RenderCommand {
    let x = (i % 256) as f32 * 4.0;
    let y = (i / 256) as f32 * 4.0;
    RenderCommand::TwoD(Command2D::UpsertRect {
        node: NodeID::from_parts(i + 1, 0),
        rect: perro_render_bridge::Rect2DCommand {
            center: [x, y],
            size: [3.0, 3.0],
            color: [0.2, 0.7, 1.0, 1.0],
            z_index: i as i32,
        },
    })
}

fn sprite_command(i: u32, texture: TextureID) -> RenderCommand {
    let x = (i % 256) as f32 * 4.0;
    let y = (i / 256) as f32 * 4.0;
    RenderCommand::TwoD(Command2D::UpsertSprite {
        node: NodeID::from_parts(i + 1, 0),
        sprite: Sprite2DCommand {
            texture,
            model: [[16.0, 0.0, 0.0], [0.0, 16.0, 0.0], [x, y, 1.0]],
            tint: [1.0, 1.0, 1.0, 1.0],
            uv_min: [0.0, 0.0],
            uv_max: [1.0, 1.0],
            size: [16.0, 16.0],
            z_index: i as i32,
        },
    })
}

fn draw_3d_command(i: u32, mesh: MeshID, material: MaterialID) -> RenderCommand {
    let x = (i % 256) as f32 * 2.0;
    let z = (i / 256) as f32 * 2.0;
    RenderCommand::ThreeD(Box::new(Command3D::Draw {
        mesh,
        surfaces: Arc::from([MeshSurfaceBinding3D {
            material: Some(material),
            overrides: Arc::from([]),
            modulate: [1.0, 1.0, 1.0, 1.0],
        }]),
        node: NodeID::from_parts(i + 1, 0),
        model: [
            [1.0, 0.0, 0.0, x],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, z],
            [0.0, 0.0, 0.0, 1.0],
        ],
        skeleton: None,
        meshlet_override: None,
        lod: LODOptions3D::default(),
    }))
}

fn water_command(i: u32, resolution: u32, impacts: u32) -> RenderCommand {
    let x = (i % 64) as f32 * 36.0;
    let y = (i / 64) as f32 * 36.0;
    RenderCommand::TwoD(Command2D::UpsertWater {
        node: NodeID::from_parts(500_000 + i, 0),
        water: Box::new(Water2DState {
            model: [[1.0, 0.0, x], [0.0, 1.0, y], [0.0, 0.0, 1.0]],
            z_index: i as i32,
            size: [32.0, 32.0],
            resolution: [resolution, resolution],
            depth: 4.0,
            flow: [0.1, 0.0],
            wind: [1.0, 0.2],
            idle_mode: WaterIdleModeState::Sine,
            wave_speed: 1.0,
            wave_scale: 1.0,
            damping: 0.985,
            wake_strength: 1.0,
            foam_strength: 0.65,
            shoreline_mask: impacts > 0,
            static_body_wakes: true,
            debug: false,
            impacts: (0..impacts)
                .map(|j| perro_render_bridge::WaterImpact2D {
                    position: [(j % 32) as f32, (j / 32) as f32],
                    velocity: [1.0, -2.0],
                    strength: 1.0 + j as f32 * 0.01,
                    radius: 2.0,
                })
                .collect::<Vec<_>>()
                .into(),
        }),
    })
}

fn tiny_mesh() -> Mesh3D {
    Mesh3D {
        vertices: vec![
            RuntimeMeshVertex {
                position: [0.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
            RuntimeMeshVertex {
                position: [1.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 0.0],
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
            RuntimeMeshVertex {
                position: [0.0, 1.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 1.0],
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
        ],
        indices: vec![0, 1, 2],
        surface_ranges: vec![],
    }
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
    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateTexture {
        request: RenderRequestID::new(1),
        id: TextureID::nil(),
        source: "__bench_texture__".to_string(),
        reserved: true,
    }));
    graphics.draw_frame();
    drain_texture(graphics)
}

fn create_mesh_material(graphics: &mut PerroGraphics) -> (MeshID, MaterialID) {
    graphics.submit_many([
        RenderCommand::Resource(ResourceCommand::CreateRuntimeMesh {
            request: RenderRequestID::new(2),
            id: MeshID::nil(),
            source: "__bench_mesh__".to_string(),
            reserved: true,
            mesh: tiny_mesh(),
        }),
        RenderCommand::Resource(ResourceCommand::CreateMaterial {
            request: RenderRequestID::new(3),
            id: MaterialID::nil(),
            material: Material3D::default(),
            source: Some("__bench_material__".to_string()),
            reserved: true,
        }),
    ]);
    graphics.draw_frame();
    drain_mesh_material(graphics)
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

fn bench_2d_sprite_prepare(c: &mut Criterion) {
    let mut group = c.benchmark_group("graphics_2d_sprite_prepare");
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

fn bench_resource_churn(c: &mut Criterion) {
    c.bench_function("graphics_resource_churn_1k", |b| {
        b.iter_batched(
            || {
                (0..1_000u32)
                    .flat_map(|i| {
                        [
                            RenderCommand::Resource(ResourceCommand::CreateTexture {
                                request: RenderRequestID::new(i as u64),
                                id: TextureID::nil(),
                                source: format!("__bench_texture_{i}__"),
                                reserved: false,
                            }),
                            RenderCommand::Resource(ResourceCommand::CreateMaterial {
                                request: RenderRequestID::new(10_000 + i as u64),
                                id: MaterialID::nil(),
                                material: Material3D::default(),
                                source: Some(format!("__bench_material_{i}__")),
                                reserved: false,
                            }),
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

criterion_group!(
    benches,
    bench_2d_rect_prepare,
    bench_2d_sprite_prepare,
    bench_3d_draw_prepare,
    bench_water_prepare,
    bench_resource_churn
);
criterion_main!(benches);
