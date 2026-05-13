use criterion::{BatchSize, BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_graphics::{GraphicsBackend, PerroGraphics};
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_render_bridge::{
    Command2D, Command3D, LODOptions3D, Material3D, Mesh3D, MeshSurfaceBinding3D, RenderBridge,
    RenderCommand, RenderEvent, RenderRequestID, ResourceCommand, RuntimeMeshVertex,
    Sprite2DCommand,
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

criterion_group!(
    benches,
    bench_2d_rect_prepare,
    bench_2d_sprite_prepare,
    bench_3d_draw_prepare,
    bench_resource_churn
);
criterion_main!(benches);
