use criterion::{BatchSize, BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_graphics::{DrawFrameTiming, GraphicsBackend, PerroGraphics};
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_render_bridge::{
    Camera2DState, Camera3DState, Command2D, Command3D, DenseInstancePose3D, LODOptions3D,
    Material3D, Mesh3D, MeshBlendOptions3D, MeshSurfaceBinding3D, PointLight2DState,
    PointLight3DState, PostProcessingCommand, Rect2DCommand, RenderBridge, RenderCommand,
    RenderEvent, RenderRequestID, ResourceCommand, RuntimeMeshVertex, Sky3DState, SkyTime3DState,
    Sprite2DCommand,
};
use perro_structs::{Color, PostProcessEffect, PostProcessSet};
use std::sync::Arc;

#[inline]
fn color(v: [f32; 4]) -> Color {
    v.into()
}

fn tiny_mesh() -> Mesh3D {
    Mesh3D {
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
    }
}

fn surface(material: MaterialID) -> Arc<[MeshSurfaceBinding3D]> {
    Arc::from([MeshSurfaceBinding3D {
        material: Some(material),
        overrides: Arc::from([]),
        modulate: Color::WHITE,
    }])
}

fn model_3d(i: u32) -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, (i % 256) as f32 * 2.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, (i / 256) as f32 * 2.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn rect_command(i: u32) -> RenderCommand {
    RenderCommand::TwoD(Command2D::UpsertRect {
        node: NodeID::from_parts(i + 1, 0),
        rect: Rect2DCommand {
            center: [(i % 256) as f32 * 4.0, (i / 256) as f32 * 4.0],
            size: [3.0, 3.0],
            color: color([0.2, 0.7, 1.0, 1.0]),
            z_index: i as i32,
        },
    })
}

fn sprite_command(i: u32, texture: TextureID) -> RenderCommand {
    RenderCommand::TwoD(Command2D::UpsertSprite {
        node: NodeID::from_parts(i + 1, 0),
        sprite: Sprite2DCommand {
            texture,
            model: [
                [16.0, 0.0, 0.0],
                [0.0, 16.0, 0.0],
                [(i % 256) as f32 * 4.0, (i / 256) as f32 * 4.0, 1.0],
            ],
            tint: color([1.0, 1.0, 1.0, 1.0]),
            uv_min: [0.0, 0.0],
            uv_max: [1.0, 1.0],
            size: [16.0, 16.0],
            z_index: i as i32,
        },
    })
}

fn draw_command(i: u32, mesh: MeshID, material: MaterialID) -> RenderCommand {
    RenderCommand::ThreeD(Box::new(Command3D::Draw {
        mesh,
        surfaces: surface(material),
        node: NodeID::from_parts(i + 1, 0),
        model: model_3d(i),
        skeleton: None,
        blend_shape_weights: Arc::from([]),
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend: MeshBlendOptions3D::default(),
        cast_shadows: true,
        receive_shadows: true,
    }))
}

fn draw_multi_command(count: u32, mesh: MeshID, material: MaterialID) -> RenderCommand {
    let mats: Arc<[[[f32; 4]; 4]]> = (0..count).map(model_3d).collect::<Vec<_>>().into();
    RenderCommand::ThreeD(Box::new(Command3D::DrawMulti {
        mesh,
        surfaces: surface(material),
        node: NodeID::from_parts(1, 0),
        instance_mats: mats,
        skeleton: None,
        blend_shape_weights: Arc::from([]),
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend: MeshBlendOptions3D::default(),
        cast_shadows: true,
        receive_shadows: true,
    }))
}

fn draw_multi_dense_command(count: u32, mesh: MeshID, material: MaterialID) -> RenderCommand {
    let instances: Arc<[DenseInstancePose3D]> = (0..count)
        .map(|i| DenseInstancePose3D {
            position: [(i % 256) as f32 * 2.0, 0.0, (i / 256) as f32 * 2.0],
            scale: [1.0, 1.0, 1.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            has_blend_shape_weight_override: false,
            blend_shape_weights: Arc::from([]),
        })
        .collect::<Vec<_>>()
        .into();
    RenderCommand::ThreeD(Box::new(Command3D::DrawMultiDense {
        mesh,
        surfaces: surface(material),
        node: NodeID::from_parts(1, 0),
        node_model: model_3d(0),
        instance_scale: 1.0,
        instances,
        blend_shape_weights: Arc::from([]),
        meshlet_override: None,
        lod: LODOptions3D::default(),
        blend: MeshBlendOptions3D::default(),
        cast_shadows: true,
        receive_shadows: true,
    }))
}

fn point_light_2d_command(i: u32) -> RenderCommand {
    RenderCommand::TwoD(Command2D::SetPointLight {
        node: NodeID::from_parts(i + 1, 0),
        light: PointLight2DState {
            position: [(i % 256) as f32 * 4.0, (i / 256) as f32 * 4.0],
            color: [1.0, 0.8, 0.5],
            intensity: 2.0,
            range: 128.0,
            z_index: i as i32,
            cast_shadows: false,
        },
    })
}

fn point_light_3d_command(i: u32) -> RenderCommand {
    RenderCommand::ThreeD(Box::new(Command3D::SetPointLight {
        node: NodeID::from_parts(i + 1, 0),
        light: PointLight3DState {
            position: [(i % 256) as f32 * 2.0, 4.0, (i / 256) as f32 * 2.0],
            color: [1.0, 0.8, 0.5],
            intensity: 3.0,
            range: 64.0,
            cast_shadows: false,
            shadow_strength: 0.82,
            shadow_depth_bias: 0.00018,
            shadow_normal_bias: 0.045,
        },
    }))
}

fn sky_command(paused: bool) -> RenderCommand {
    RenderCommand::ThreeD(Box::new(Command3D::SetSky {
        node: NodeID::from_parts(1, 0),
        sky: Box::new(Sky3DState {
            day_colors: Arc::from([[0.42, 0.7, 1.0], [0.1, 0.35, 0.8]]),
            evening_colors: Arc::from([[1.0, 0.45, 0.2], [0.25, 0.08, 0.3]]),
            night_colors: Arc::from([[0.02, 0.03, 0.08], [0.0, 0.0, 0.02]]),
            horizon_colors: Arc::from([[0.55, 0.57, 0.60], [0.35, 0.36, 0.38]]),
            time: SkyTime3DState {
                time_of_day: 0.5,
                paused,
                scale: 1.0,
            },
            shaders: Arc::from([]),
        }),
    }))
}

fn create_texture(graphics: &mut PerroGraphics) -> TextureID {
    graphics.submit(RenderCommand::Resource(
        ResourceCommand::CreateRuntimeTexture {
            request: RenderRequestID::new(1),
            id: TextureID::nil(),
            source: "runtime://frame-timing-bench".to_string(),
            reserved: true,
            width: 1,
            height: 1,
            rgba: Arc::from([255, 255, 255, 255]),
        },
    ));
    graphics.draw_frame();
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

fn post_chain() -> PostProcessSet {
    PostProcessSet::from_effects(vec![
        PostProcessEffect::Bloom {
            strength: 0.7,
            threshold: 0.8,
            radius: 1.2,
        },
        PostProcessEffect::Crt {
            scanline_strength: 0.25,
            curvature: 0.1,
            chromatic: 0.5,
            vignette: 0.2,
        },
        PostProcessEffect::ColorGrade {
            exposure: 0.1,
            contrast: 1.2,
            brightness: -0.05,
            saturation: 1.3,
            gamma: 0.95,
            temperature: 0.2,
            tint: -0.1,
            hue_shift: 0.05,
            vibrance: 0.4,
            lift: [0.01, 0.02, 0.03],
            gain: [1.1, 1.05, 1.0],
            offset: [-0.01, -0.02, -0.03],
        },
        PostProcessEffect::Vignette {
            strength: 0.35,
            radius: 0.75,
            softness: 0.25,
        },
        PostProcessEffect::Saturate { amount: 1.2 },
        PostProcessEffect::Warp {
            waves: 8.0,
            strength: 0.03,
        },
        PostProcessEffect::Pixelate { size: 1.0 },
        PostProcessEffect::BlackWhite { amount: 0.0 },
    ])
}

fn black_box_timing(timing: DrawFrameTiming) {
    black_box(timing.process_commands);
    black_box(timing.prepare_cpu);
    black_box(timing.draw_calls_2d);
    black_box(timing.draw_calls_3d);
    black_box(timing.draw_instances_3d);
    black_box(timing.draw_material_refs_3d);
    black_box(timing.total);
}

fn force_2d_redraw(graphics: &mut PerroGraphics) {
    graphics.submit(RenderCommand::TwoD(Command2D::SetCamera {
        camera: Camera2DState::default(),
    }));
}

fn force_3d_redraw(graphics: &mut PerroGraphics) {
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::SetCamera {
        camera: Camera3DState::default(),
    })));
}

fn bench_upsert_frames(c: &mut Criterion) {
    let mut group = c.benchmark_group("graphics_frame_upsert");
    for count in [1_000u32, 10_000, 100_000] {
        group.bench_with_input(BenchmarkId::new("rects", count), &count, |b, &count| {
            let mut graphics = PerroGraphics::new();
            b.iter_batched(
                || (0..count).map(rect_command).collect::<Vec<_>>(),
                |commands| {
                    graphics.submit_many(commands);
                    black_box_timing(graphics.draw_frame_timed().expect("timing"));
                },
                BatchSize::LargeInput,
            );
        });
        group.bench_with_input(BenchmarkId::new("sprites", count), &count, |b, &count| {
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
                    black_box_timing(graphics.draw_frame_timed().expect("timing"));
                },
                BatchSize::LargeInput,
            );
        });
        group.bench_with_input(BenchmarkId::new("meshes", count), &count, |b, &count| {
            let mut graphics = PerroGraphics::new();
            let (mesh, material) = create_mesh_material(&mut graphics);
            b.iter_batched(
                || {
                    (0..count)
                        .map(|i| draw_command(i, mesh, material))
                        .collect::<Vec<_>>()
                },
                |commands| {
                    graphics.submit_many(commands);
                    black_box_timing(graphics.draw_frame_timed().expect("timing"));
                },
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

fn bench_retained_frames(c: &mut Criterion) {
    let mut group = c.benchmark_group("graphics_frame_retained_redraw");
    // Regression notes:
    // - sprites 100k once spent ~9ms/frame in retained redraw; keep near fixed cost.
    // - rects 100k once rebuilt the retained rect cache every redraw (~112us); keep sub-us.
    // - meshes 100k once recounted retained instances every redraw; keep count cached.
    // - a 3d sorted-cache fast path regressed mesh upsert; bench before trying again.
    for count in [1_000u32, 10_000, 100_000] {
        group.bench_with_input(BenchmarkId::new("rects", count), &count, |b, &count| {
            let mut graphics = PerroGraphics::new();
            graphics.submit_many((0..count).map(rect_command));
            black_box_timing(graphics.draw_frame_timed().expect("timing"));
            b.iter(|| {
                force_2d_redraw(&mut graphics);
                black_box_timing(graphics.draw_frame_timed().expect("timing"));
            });
        });
        group.bench_with_input(BenchmarkId::new("sprites", count), &count, |b, &count| {
            let mut graphics = PerroGraphics::new();
            let texture = create_texture(&mut graphics);
            graphics.submit_many((0..count).map(|i| sprite_command(i, texture)));
            black_box_timing(graphics.draw_frame_timed().expect("timing"));
            b.iter(|| {
                force_2d_redraw(&mut graphics);
                black_box_timing(graphics.draw_frame_timed().expect("timing"));
            });
        });
        group.bench_with_input(BenchmarkId::new("meshes", count), &count, |b, &count| {
            let mut graphics = PerroGraphics::new();
            let (mesh, material) = create_mesh_material(&mut graphics);
            graphics.submit_many((0..count).map(|i| draw_command(i, mesh, material)));
            black_box_timing(graphics.draw_frame_timed().expect("timing"));
            b.iter(|| {
                force_3d_redraw(&mut graphics);
                black_box_timing(graphics.draw_frame_timed().expect("timing"));
            });
        });
    }
    group.finish();
}

fn bench_multimesh_frames(c: &mut Criterion) {
    let mut group = c.benchmark_group("graphics_frame_multimesh");
    // Dense multimesh is the preferred bulk instance path:
    // 100k dense should stay much faster than 100k full matrices.
    for count in [10_000u32, 100_000] {
        group.bench_with_input(
            BenchmarkId::new("draw_multi_mats", count),
            &count,
            |b, &count| {
                let mut graphics = PerroGraphics::new();
                let (mesh, material) = create_mesh_material(&mut graphics);
                b.iter_batched(
                    || vec![draw_multi_command(count, mesh, material)],
                    |commands| {
                        graphics.submit_many(commands);
                        black_box_timing(graphics.draw_frame_timed().expect("timing"));
                    },
                    BatchSize::LargeInput,
                );
            },
        );
        group.bench_with_input(
            BenchmarkId::new("draw_multi_dense", count),
            &count,
            |b, &count| {
                let mut graphics = PerroGraphics::new();
                let (mesh, material) = create_mesh_material(&mut graphics);
                b.iter_batched(
                    || vec![draw_multi_dense_command(count, mesh, material)],
                    |commands| {
                        graphics.submit_many(commands);
                        black_box_timing(graphics.draw_frame_timed().expect("timing"));
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }
    group.finish();
}

fn bench_postprocess_frames(c: &mut Criterion) {
    let mut group = c.benchmark_group("graphics_frame_postprocess");
    group.bench_function("set_global_8_effects", |b| {
        let mut graphics = PerroGraphics::new();
        graphics.submit(rect_command(0));
        black_box_timing(graphics.draw_frame_timed().expect("timing"));
        b.iter_batched(
            post_chain,
            |chain| {
                graphics.submit(RenderCommand::PostProcessing(
                    PostProcessingCommand::SetGlobal(chain),
                ));
                black_box_timing(graphics.draw_frame_timed().expect("timing"));
            },
            BatchSize::SmallInput,
        );
    });
    group.bench_function("retained_8_effects", |b| {
        let mut graphics = PerroGraphics::new();
        graphics.submit_many([
            rect_command(0),
            RenderCommand::PostProcessing(PostProcessingCommand::SetGlobal(post_chain())),
        ]);
        black_box_timing(graphics.draw_frame_timed().expect("timing"));
        b.iter(|| {
            force_2d_redraw(&mut graphics);
            black_box_timing(graphics.draw_frame_timed().expect("timing"));
        });
    });
    group.finish();
}

fn bench_light_frames(c: &mut Criterion) {
    let mut group = c.benchmark_group("graphics_frame_lights");
    for count in [8u32, 64, 1_024] {
        group.bench_with_input(
            BenchmarkId::new("upsert_2d_point", count),
            &count,
            |b, &count| {
                let mut graphics = PerroGraphics::new();
                graphics.submit(rect_command(0));
                black_box_timing(graphics.draw_frame_timed().expect("timing"));
                b.iter_batched(
                    || (0..count).map(point_light_2d_command).collect::<Vec<_>>(),
                    |commands| {
                        graphics.submit_many(commands);
                        black_box_timing(graphics.draw_frame_timed().expect("timing"));
                    },
                    BatchSize::LargeInput,
                );
            },
        );
        group.bench_with_input(
            BenchmarkId::new("retained_2d_point", count),
            &count,
            |b, &count| {
                let mut graphics = PerroGraphics::new();
                graphics.submit(rect_command(0));
                graphics.submit_many((0..count).map(point_light_2d_command));
                black_box_timing(graphics.draw_frame_timed().expect("timing"));
                b.iter(|| {
                    force_2d_redraw(&mut graphics);
                    black_box_timing(graphics.draw_frame_timed().expect("timing"));
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("upsert_3d_point", count),
            &count,
            |b, &count| {
                let mut graphics = PerroGraphics::new();
                let (mesh, material) = create_mesh_material(&mut graphics);
                graphics.submit(draw_command(0, mesh, material));
                black_box_timing(graphics.draw_frame_timed().expect("timing"));
                b.iter_batched(
                    || (0..count).map(point_light_3d_command).collect::<Vec<_>>(),
                    |commands| {
                        graphics.submit_many(commands);
                        black_box_timing(graphics.draw_frame_timed().expect("timing"));
                    },
                    BatchSize::LargeInput,
                );
            },
        );
        group.bench_with_input(
            BenchmarkId::new("retained_3d_point", count),
            &count,
            |b, &count| {
                let mut graphics = PerroGraphics::new();
                let (mesh, material) = create_mesh_material(&mut graphics);
                graphics.submit(draw_command(0, mesh, material));
                graphics.submit_many((0..count).map(point_light_3d_command));
                black_box_timing(graphics.draw_frame_timed().expect("timing"));
                b.iter(|| {
                    force_3d_redraw(&mut graphics);
                    black_box_timing(graphics.draw_frame_timed().expect("timing"));
                });
            },
        );
    }
    group.finish();
}

fn bench_sky_frames(c: &mut Criterion) {
    let mut group = c.benchmark_group("graphics_frame_sky");
    group.bench_function("set_paused", |b| {
        let mut graphics = PerroGraphics::new();
        b.iter_batched(
            || sky_command(true),
            |command| {
                graphics.submit(command);
                black_box_timing(graphics.draw_frame_timed().expect("timing"));
            },
            BatchSize::SmallInput,
        );
    });
    group.bench_function("retained_paused_redraw", |b| {
        let mut graphics = PerroGraphics::new();
        graphics.submit(sky_command(true));
        black_box_timing(graphics.draw_frame_timed().expect("timing"));
        b.iter(|| {
            force_3d_redraw(&mut graphics);
            black_box_timing(graphics.draw_frame_timed().expect("timing"));
        });
    });
    group.bench_function("retained_animated", |b| {
        let mut graphics = PerroGraphics::new();
        graphics.submit(sky_command(false));
        black_box_timing(graphics.draw_frame_timed().expect("timing"));
        b.iter(|| black_box_timing(graphics.draw_frame_timed().expect("timing")));
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_upsert_frames,
    bench_retained_frames,
    bench_multimesh_frames,
    bench_postprocess_frames,
    bench_light_frames,
    bench_sky_frames
);
criterion_main!(benches);
