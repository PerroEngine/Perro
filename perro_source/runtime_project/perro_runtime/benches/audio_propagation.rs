use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_nodes::{
    CollisionShape2D, CollisionShape3D, Node2D, Node3D, SceneNodeData, StaticBody2D, StaticBody3D,
};
use perro_runtime::Runtime;
use perro_runtime_context::sub_apis::{
    AudioEffects, NodeAPI, RuntimeAudio, RuntimeAudioAPI, SpatialAudioOptions,
};
use perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};

fn looped_audio() -> RuntimeAudio<'static> {
    RuntimeAudio {
        source: "res://bench.wav",
        looped: true,
        volume: 1.0,
        effects: AudioEffects::new(),
        from_start: 0.0,
        from_end: 0.0,
    }
}

fn runtime_2d(walls: usize, sounds: usize) -> Runtime {
    let mut runtime = Runtime::new();
    for i in 0..walls {
        let body = NodeAPI::create::<StaticBody2D>(&mut runtime);
        let shape = NodeAPI::create::<CollisionShape2D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, body, shape));
        if let Some(node) = runtime.nodes.get_mut(body)
            && let SceneNodeData::StaticBody2D(body) = &mut node.data
        {
            body.audio_diffusion.damping = 0.45;
            body.audio_diffusion.compression = 0.2;
            body.audio_diffusion.hardness = 0.65;
        }
        let x = 4.0 + (i % 16) as f32 * 2.0;
        let y = (i / 16) as f32 * 1.5 - 6.0;
        assert!(NodeAPI::set_global_transform_2d(
            &mut runtime,
            body,
            Transform2D::new(Vector2::new(x, y), 0.0, Vector2::ONE),
        ));
    }
    for i in 0..sounds {
        let node = NodeAPI::create::<Node2D>(&mut runtime);
        let x = 30.0 + (i % 8) as f32 * 2.5;
        let y = (i / 8) as f32 * 2.0 - 8.0;
        assert!(NodeAPI::set_global_transform_2d(
            &mut runtime,
            node,
            Transform2D::new(Vector2::new(x, y), 0.0, Vector2::ONE),
        ));
        assert!(runtime.play_runtime_audio_attached(
            looped_audio(),
            node,
            SpatialAudioOptions::new(80.0),
        ));
    }
    runtime.update(1.0);
    runtime
}

fn runtime_3d(walls: usize, sounds: usize) -> Runtime {
    let mut runtime = Runtime::new();
    for i in 0..walls {
        let body = NodeAPI::create::<StaticBody3D>(&mut runtime);
        let shape = NodeAPI::create::<CollisionShape3D>(&mut runtime);
        assert!(NodeAPI::reparent(&mut runtime, body, shape));
        if let Some(node) = runtime.nodes.get_mut(body)
            && let SceneNodeData::StaticBody3D(body) = &mut node.data
        {
            body.audio_diffusion.damping = 0.45;
            body.audio_diffusion.compression = 0.2;
            body.audio_diffusion.hardness = 0.65;
        }
        let x = 4.0 + (i % 8) as f32 * 2.5;
        let y = ((i / 8) % 4) as f32 * 2.0 - 4.0;
        let z = (i / 32) as f32 * 2.0 - 4.0;
        assert!(NodeAPI::set_global_transform_3d(
            &mut runtime,
            body,
            Transform3D::new(Vector3::new(x, y, z), Quaternion::IDENTITY, Vector3::ONE),
        ));
    }
    for i in 0..sounds {
        let node = NodeAPI::create::<Node3D>(&mut runtime);
        let x = 30.0 + (i % 8) as f32 * 3.0;
        let y = ((i / 8) % 4) as f32 * 2.0 - 4.0;
        let z = (i / 32) as f32 * 3.0 - 4.0;
        assert!(NodeAPI::set_global_transform_3d(
            &mut runtime,
            node,
            Transform3D::new(Vector3::new(x, y, z), Quaternion::IDENTITY, Vector3::ONE),
        ));
        assert!(runtime.play_runtime_audio_attached(
            looped_audio(),
            node,
            SpatialAudioOptions::new(90.0),
        ));
    }
    runtime.update(1.0);
    runtime
}

fn bench_audio_propagation(c: &mut Criterion) {
    let mut small_2d = c.benchmark_group("audio_propagation_2d_small");
    for walls in [4usize, 12] {
        small_2d.bench_with_input(
            BenchmarkId::new("1_sound_walls", walls),
            &walls,
            |b, walls| {
                let mut runtime = runtime_2d(*walls, 1);
                b.iter(|| {
                    runtime.update(black_box(0.05));
                });
            },
        );
    }
    small_2d.finish();

    let mut small_3d = c.benchmark_group("audio_propagation_3d_small");
    for walls in [4usize, 12] {
        small_3d.bench_with_input(
            BenchmarkId::new("1_sound_walls", walls),
            &walls,
            |b, walls| {
                let mut runtime = runtime_3d(*walls, 1);
                b.iter(|| {
                    runtime.update(black_box(0.05));
                });
            },
        );
    }
    small_3d.finish();

    let mut threshold_2d = c.benchmark_group("audio_propagation_2d_sound_count");
    for sounds in [1usize, 4, 8, 16, 32, 64, 128, 256] {
        threshold_2d.bench_with_input(
            BenchmarkId::new("64_walls", sounds),
            &sounds,
            |b, sounds| {
                let mut runtime = runtime_2d(64, *sounds);
                b.iter(|| {
                    runtime.update(black_box(0.05));
                });
            },
        );
    }
    threshold_2d.finish();

    let mut threshold_3d = c.benchmark_group("audio_propagation_3d_sound_count");
    for sounds in [1usize, 4, 8, 16, 32, 64, 128, 256] {
        threshold_3d.bench_with_input(
            BenchmarkId::new("64_walls", sounds),
            &sounds,
            |b, sounds| {
                let mut runtime = runtime_3d(64, *sounds);
                b.iter(|| {
                    runtime.update(black_box(0.05));
                });
            },
        );
    }
    threshold_3d.finish();

    c.bench_function("audio_propagation_2d_64_walls_32_sounds", |b| {
        let mut runtime = runtime_2d(64, 32);
        b.iter(|| {
            runtime.update(black_box(0.05));
        });
    });

    c.bench_function("audio_propagation_3d_64_walls_32_sounds", |b| {
        let mut runtime = runtime_3d(64, 32);
        b.iter(|| {
            runtime.update(black_box(0.05));
        });
    });
}

criterion_group!(benches, bench_audio_propagation);
criterion_main!(benches);
