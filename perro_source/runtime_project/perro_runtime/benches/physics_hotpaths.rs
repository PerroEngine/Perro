use criterion::{Criterion, black_box, criterion_group, criterion_main};
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_nodes::{
    MeshInstance3D, PhysicsForceEmitter2D, PhysicsForceEmitter3D, PhysicsForceProfile, Sprite2D,
};
use perro_render_bridge::RenderCommand;
use perro_runtime::Runtime;
use perro_runtime_api::sub_apis::{NodeAPI, PhysicsAPI, PhysicsQueryFilter};
use perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};

const DT: f32 = 1.0 / 60.0;

fn bench_physics_scan_ids_clone_vs_scratch(c: &mut Criterion) {
    let ids: Vec<NodeID> = (1..=200_000).map(|i| NodeID::from_parts(i, 0)).collect();
    let mut group = c.benchmark_group("physics/scan_ids_clone_vs_scratch");
    group.bench_function("clone", |b| b.iter(|| black_box(black_box(&ids).clone())));
    group.bench_function("scratch_extend", |b| {
        let mut scratch = Vec::<NodeID>::new();
        b.iter(|| {
            scratch.clear();
            scratch.extend_from_slice(black_box(&ids));
            black_box(scratch.len())
        })
    });
    group.finish();
}

fn bench_physics_children_clone_vs_slice_scan(c: &mut Criterion) {
    let body_count = 100_000usize;
    let children_per_body = 4usize;
    let mut children = Vec::with_capacity(body_count);
    for body in 0..body_count {
        let mut ids = Vec::with_capacity(children_per_body);
        for i in 0..children_per_body {
            ids.push(NodeID::from_parts(
                (body * children_per_body + i + 1) as u32,
                0,
            ));
        }
        children.push(ids);
    }

    let mut group = c.benchmark_group("physics/children_clone_vs_slice_scan");
    group.bench_function("clone_then_scan", |b| {
        b.iter(|| {
            let mut acc = 0u64;
            for ids in black_box(&children) {
                let copied = ids.to_vec();
                for id in &copied {
                    acc = acc.wrapping_add(id.as_u64());
                }
            }
            black_box(acc)
        })
    });
    group.bench_function("slice_scan", |b| {
        b.iter(|| {
            let mut acc = 0u64;
            for ids in black_box(&children) {
                for &id in ids {
                    acc = acc.wrapping_add(id.as_u64());
                }
            }
            black_box(acc)
        })
    });
    group.finish();
}

fn bench_physics_raycast_2d_query_filter(c: &mut Criterion) {
    c.bench_function("physics/raycast_2d_query_filter", |b| {
        let mut runtime = Runtime::new();
        for i in 0..256 {
            let body = NodeAPI::create::<perro_nodes::StaticBody2D>(&mut runtime);
            let shape = NodeAPI::create::<perro_nodes::CollisionShape2D>(&mut runtime);
            assert!(NodeAPI::reparent(&mut runtime, body, shape));
            let _ = NodeAPI::set_global_transform_2d(
                &mut runtime,
                body,
                Transform2D::new(Vector2::new(i as f32 * 2.0, 0.0), 0.0, Vector2::ONE),
            );
        }
        let filter = PhysicsQueryFilter::default();
        b.iter(|| {
            black_box(Runtime::physics_raycast_2d(
                &mut runtime,
                Vector2::new(-10.0, 0.0),
                Vector2::new(1.0, 0.0),
                1_000.0,
                &filter,
            ))
        })
    });
}

fn bench_physics_shape_cast_3d(c: &mut Criterion) {
    c.bench_function("physics/shape_cast_3d", |b| {
        let mut runtime = Runtime::new();
        for i in 0..128 {
            let body = NodeAPI::create::<perro_nodes::StaticBody3D>(&mut runtime);
            let shape = NodeAPI::create::<perro_nodes::CollisionShape3D>(&mut runtime);
            assert!(NodeAPI::reparent(&mut runtime, body, shape));
            let _ = NodeAPI::set_global_transform_3d(
                &mut runtime,
                body,
                Transform3D::new(
                    Vector3::new(0.0, 0.0, i as f32 * 2.0),
                    perro_structs::Quaternion::IDENTITY,
                    Vector3::ONE,
                ),
            );
        }
        let filter = PhysicsQueryFilter::default();
        b.iter(|| {
            black_box(Runtime::physics_shape_cast_3d(
                &mut runtime,
                perro_nodes::Shape3D::Sphere { radius: 0.25 },
                Vector3::new(0.0, 0.0, -5.0),
                Vector3::new(0.0, 0.0, 1.0),
                1_000.0,
                &filter,
            ))
        })
    });
}

fn runtime_with_rigid_bodies_2d(count: u32) -> (Runtime, Vec<NodeID>) {
    let mut runtime = Runtime::new();
    let mut bodies = Vec::with_capacity(count as usize);
    for i in 0..count {
        let body = NodeAPI::create::<perro_nodes::RigidBody2D>(&mut runtime);
        let shape = NodeAPI::create::<perro_nodes::CollisionShape2D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, body, shape));
        let _ =
            NodeAPI::with_node_mut::<perro_nodes::RigidBody2D, _, _>(&mut runtime, body, |node| {
                node.transform = Transform2D::new(
                    Vector2::new((i % 64) as f32 - 32.0, 2.0 + (i / 64) as f32 * 1.25),
                    0.0,
                    Vector2::ONE,
                );
                node.can_sleep = false;
                node.continuous_collision_detection = false;
                node.linear_velocity = Vector2::new((i % 7) as f32 * 0.05, 0.0);
            });
        bodies.push(body);
    }
    (runtime, bodies)
}

fn runtime_with_rigid_bodies_3d(count: u32) -> (Runtime, Vec<NodeID>) {
    let mut runtime = Runtime::new();
    let mut bodies = Vec::with_capacity(count as usize);
    for i in 0..count {
        let body = NodeAPI::create::<perro_nodes::RigidBody3D>(&mut runtime);
        let shape = NodeAPI::create::<perro_nodes::CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, body, shape));
        let _ =
            NodeAPI::with_node_mut::<perro_nodes::RigidBody3D, _, _>(&mut runtime, body, |node| {
                node.transform = Transform3D::new(
                    Vector3::new(
                        (i % 16) as f32 - 8.0,
                        2.0 + (i / 256) as f32 * 1.25,
                        ((i / 16) % 16) as f32 - 8.0,
                    ),
                    Quaternion::IDENTITY,
                    Vector3::ONE,
                );
                node.can_sleep = false;
                node.continuous_collision_detection = false;
                node.linear_velocity =
                    Vector3::new((i % 7) as f32 * 0.05, 0.0, (i % 5) as f32 * -0.03);
            });
        bodies.push(body);
    }
    (runtime, bodies)
}

fn bench_runtime_force_impulse_queue_2d(c: &mut Criterion) {
    c.bench_function("physics/runtime_force_impulse_queue_2d_4096", |b| {
        let (mut runtime, bodies) = runtime_with_rigid_bodies_2d(4096);
        b.iter(|| {
            for &body in black_box(&bodies) {
                black_box(PhysicsAPI::apply_force_2d(
                    &mut runtime,
                    body,
                    Vector2::new(0.4, 0.1),
                ));
                black_box(PhysicsAPI::apply_impulse_2d(
                    &mut runtime,
                    body,
                    Vector2::new(0.02, 0.01),
                ));
            }
        })
    });
}

fn bench_runtime_force_impulse_queue_3d(c: &mut Criterion) {
    c.bench_function("physics/runtime_force_impulse_queue_3d_4096", |b| {
        let (mut runtime, bodies) = runtime_with_rigid_bodies_3d(4096);
        b.iter(|| {
            for &body in black_box(&bodies) {
                black_box(PhysicsAPI::apply_force_3d(
                    &mut runtime,
                    body,
                    Vector3::new(0.4, 0.1, -0.2),
                ));
                black_box(PhysicsAPI::apply_impulse_3d(
                    &mut runtime,
                    body,
                    Vector3::new(0.02, 0.01, 0.03),
                ));
            }
        })
    });
}

fn bench_runtime_predict_body(c: &mut Criterion) {
    let mut group = c.benchmark_group("physics/runtime_predict_body");
    group.bench_function("predict_2d_4096", |b| {
        let (mut runtime, bodies) = runtime_with_rigid_bodies_2d(4096);
        b.iter(|| {
            let mut acc = Vector2::ZERO;
            for (i, &body) in black_box(&bodies).iter().enumerate() {
                if let Some(predicted) = PhysicsAPI::predict_body_2d(
                    &mut runtime,
                    body,
                    0.25 + i as f32 * 0.0001,
                    Vector2::new(0.1, -0.2),
                ) {
                    acc += predicted.position + predicted.velocity;
                }
            }
            black_box(acc)
        })
    });
    group.bench_function("predict_3d_4096", |b| {
        let (mut runtime, bodies) = runtime_with_rigid_bodies_3d(4096);
        b.iter(|| {
            let mut acc = Vector3::ZERO;
            for (i, &body) in black_box(&bodies).iter().enumerate() {
                if let Some(predicted) = PhysicsAPI::predict_body_3d(
                    &mut runtime,
                    body,
                    0.25 + i as f32 * 0.0001,
                    Vector3::new(0.1, -0.2, 0.3),
                ) {
                    acc += predicted.position + predicted.velocity;
                }
            }
            black_box(acc)
        })
    });
    group.finish();
}

fn bench_runtime_fixed_step_forces(c: &mut Criterion) {
    let mut group = c.benchmark_group("physics/runtime_fixed_step_forces");
    group.bench_function("fixed_step_2d_512", |b| {
        b.iter_batched(
            || runtime_with_rigid_bodies_2d(512),
            |(mut runtime, bodies)| {
                for &body in &bodies {
                    black_box(PhysicsAPI::apply_force_2d(
                        &mut runtime,
                        body,
                        Vector2::new(0.4, 0.1),
                    ));
                    black_box(PhysicsAPI::apply_impulse_2d(
                        &mut runtime,
                        body,
                        Vector2::new(0.02, 0.01),
                    ));
                }
                runtime.fixed_update(DT);
                black_box(runtime.nodes.len())
            },
            criterion::BatchSize::LargeInput,
        )
    });
    group.bench_function("fixed_step_3d_512", |b| {
        b.iter_batched(
            || runtime_with_rigid_bodies_3d(512),
            |(mut runtime, bodies)| {
                for &body in &bodies {
                    black_box(PhysicsAPI::apply_force_3d(
                        &mut runtime,
                        body,
                        Vector3::new(0.4, 0.1, -0.2),
                    ));
                    black_box(PhysicsAPI::apply_impulse_3d(
                        &mut runtime,
                        body,
                        Vector3::new(0.02, 0.01, 0.03),
                    ));
                }
                runtime.fixed_update(DT);
                black_box(runtime.nodes.len())
            },
            criterion::BatchSize::LargeInput,
        )
    });
    group.finish();
}

fn bench_runtime_force_emitters(c: &mut Criterion) {
    let mut group = c.benchmark_group("physics/runtime_force_emitters");
    group.bench_function("custom_2d_512", |b| {
        b.iter_batched(
            || {
                let (runtime, _bodies) = runtime_with_rigid_bodies_2d(512);
                runtime
            },
            |mut runtime| {
                let mut emitter = PhysicsForceEmitter2D::new();
                emitter.profile = PhysicsForceProfile::Custom;
                emitter.radius = 96.0;
                emitter.strength = 2.0;
                emitter.falloff = 1.0;
                emitter.vectors = vec![
                    Vector2::new(0.0, 1.0),
                    Vector2::new(1.0, 0.25),
                    Vector2::new(-0.25, 0.8),
                ];
                black_box(PhysicsAPI::emit_force_2d(&mut runtime, emitter));
                runtime.fixed_update(DT);
                black_box(runtime.nodes.len())
            },
            criterion::BatchSize::LargeInput,
        )
    });
    group.bench_function("custom_3d_512", |b| {
        b.iter_batched(
            || {
                let (runtime, _bodies) = runtime_with_rigid_bodies_3d(512);
                runtime
            },
            |mut runtime| {
                let mut emitter = PhysicsForceEmitter3D::new();
                emitter.profile = PhysicsForceProfile::Custom;
                emitter.radius = 96.0;
                emitter.strength = 2.0;
                emitter.falloff = 1.0;
                emitter.vectors = vec![
                    Vector3::new(0.0, 1.0, 0.0),
                    Vector3::new(1.0, 0.25, -0.25),
                    Vector3::new(-0.25, 0.8, 0.5),
                ];
                black_box(PhysicsAPI::emit_force_3d(&mut runtime, emitter));
                runtime.fixed_update(DT);
                black_box(runtime.nodes.len())
            },
            criterion::BatchSize::LargeInput,
        )
    });
    group.finish();
}

fn bench_physics_visual_interp_render_extract(c: &mut Criterion) {
    c.bench_function("physics/visual_interp_render_extract_2d_1024", |b| {
        let mut runtime = Runtime::new();
        let texture = TextureID::from_parts(77, 0);
        for i in 0..1024 {
            let body = NodeAPI::create::<perro_nodes::RigidBody2D>(&mut runtime);
            let sprite = NodeAPI::create::<Sprite2D>(&mut runtime);
            assert!(NodeAPI::reparent(&mut runtime, body, sprite));
            let _ = NodeAPI::with_node_mut::<perro_nodes::RigidBody2D, _, _>(
                &mut runtime,
                body,
                |node| {
                    node.transform = Transform2D::new(
                        Vector2::new((i % 64) as f32, (i / 64) as f32),
                        0.0,
                        Vector2::ONE,
                    );
                    node.linear_velocity = Vector2::new(1.0 + (i % 7) as f32 * 0.05, 0.0);
                    node.continuous_collision_detection = false;
                    node.can_sleep = false;
                },
            );
            let _ = NodeAPI::with_node_mut::<Sprite2D, _, _>(&mut runtime, sprite, |node| {
                node.texture = texture;
            });
        }
        runtime.fixed_update(DT);
        runtime.fixed_update(DT);
        runtime.extract_render_2d_commands();
        let mut commands = Vec::<RenderCommand>::new();
        runtime.drain_render_commands(&mut commands);
        runtime.clear_dirty_flags();
        commands.clear();
        let mut alpha = 0.0f32;

        b.iter(|| {
            alpha += 0.125;
            if alpha >= 1.0 {
                alpha -= 1.0;
            }
            runtime.set_physics_render_alpha(black_box(alpha));
            runtime.extract_render_2d_commands();
            runtime.drain_render_commands(&mut commands);
            let len = commands.len();
            commands.clear();
            runtime.clear_dirty_flags();
            black_box(len)
        })
    });

    c.bench_function("physics/visual_interp_render_extract_3d_1024", |b| {
        let mut runtime = Runtime::new();
        let mesh_id = MeshID::from_parts(77, 0);
        let material_id = MaterialID::from_parts(78, 0);
        for i in 0..1024 {
            let body = NodeAPI::create::<perro_nodes::RigidBody3D>(&mut runtime);
            let mesh = NodeAPI::create::<MeshInstance3D>(&mut runtime);
            assert!(NodeAPI::reparent(&mut runtime, body, mesh));
            let _ = NodeAPI::with_node_mut::<perro_nodes::RigidBody3D, _, _>(
                &mut runtime,
                body,
                |node| {
                    node.transform = Transform3D::new(
                        Vector3::new((i % 16) as f32, (i / 256) as f32, ((i / 16) % 16) as f32),
                        Quaternion::IDENTITY,
                        Vector3::ONE,
                    );
                    node.linear_velocity = Vector3::new(1.0 + (i % 7) as f32 * 0.05, 0.0, -0.25);
                    node.continuous_collision_detection = false;
                    node.can_sleep = false;
                },
            );
            let _ = NodeAPI::with_node_mut::<MeshInstance3D, _, _>(&mut runtime, mesh, |node| {
                node.mesh = mesh_id;
                node.set_material(material_id);
            });
        }
        runtime.fixed_update(DT);
        runtime.fixed_update(DT);
        runtime.extract_render_3d_commands();
        let mut commands = Vec::<RenderCommand>::new();
        runtime.drain_render_commands(&mut commands);
        runtime.clear_dirty_flags();
        commands.clear();
        let mut alpha = 0.0f32;

        b.iter(|| {
            alpha += 0.125;
            if alpha >= 1.0 {
                alpha -= 1.0;
            }
            runtime.set_physics_render_alpha(black_box(alpha));
            runtime.extract_render_3d_commands();
            runtime.drain_render_commands(&mut commands);
            let len = commands.len();
            commands.clear();
            runtime.clear_dirty_flags();
            black_box(len)
        })
    });
}

fn benches(c: &mut Criterion) {
    bench_physics_scan_ids_clone_vs_scratch(c);
    bench_physics_children_clone_vs_slice_scan(c);
    bench_physics_raycast_2d_query_filter(c);
    bench_physics_shape_cast_3d(c);
    bench_runtime_force_impulse_queue_2d(c);
    bench_runtime_force_impulse_queue_3d(c);
    bench_runtime_predict_body(c);
    bench_runtime_fixed_step_forces(c);
    bench_runtime_force_emitters(c);
    bench_physics_visual_interp_render_extract(c);
}

criterion_group! {
    name = physics_hotpaths;
    config = Criterion::default().sample_size(10);
    targets = benches
}
criterion_main!(physics_hotpaths);
