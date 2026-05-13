use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_nodes::{
    WaterIdleMode, WaterPhysicsSample, WaterSurfaceParams, water_impact_strength,
    water_physics_sample_or_idle,
};
use perro_structs::Vector2;

#[derive(Clone, Copy)]
struct Body2 {
    pos: Vector2,
    vel: Vector2,
    mass: f32,
}

fn bodies(count: usize) -> Vec<Body2> {
    (0..count)
        .map(|i| Body2 {
            pos: Vector2::new((i % 128) as f32 - 64.0, (i / 128) as f32 * 0.15),
            vel: Vector2::new((i % 7) as f32 * 0.2, -1.0 - (i % 11) as f32 * 0.1),
            mass: 1.0 + (i % 31) as f32 * 0.2,
        })
        .collect()
}

fn water_surface(resolution: u32) -> WaterSurfaceParams {
    let mut surface = WaterSurfaceParams {
        size: Vector2::new(128.0, 128.0),
        resolution: [resolution, resolution],
        idle_mode: WaterIdleMode::Chop,
        ..Default::default()
    };
    surface.wave.speed = 1.35;
    surface.wave.scale = 1.2;
    surface.physics.buoyancy = 2.5;
    surface.physics.drag = 0.35;
    surface
}

fn bench_water_physics(c: &mut Criterion) {
    let mut group = c.benchmark_group("runtime_water_physics");
    for (body_count, resolution, cached) in [
        (100usize, 64u32, false),
        (100, 64, true),
        (1_000, 128, false),
        (1_000, 128, true),
        (10_000, 256, false),
        (10_000, 256, true),
    ] {
        group.bench_with_input(
            BenchmarkId::new(
                format!("{body_count}_bodies"),
                format!(
                    "{resolution}r_{}",
                    if cached { "cached" } else { "fallback" }
                ),
            ),
            &(body_count, resolution, cached),
            |b, &(body_count, resolution, cached)| {
                let bodies = bodies(body_count);
                let surface = water_surface(resolution);
                let sample = cached.then_some(WaterPhysicsSample {
                    height: 0.75,
                    velocity: Vector2::new(0.1, 0.0),
                    foam: 0.2,
                });
                b.iter(|| {
                    let mut total_force = 0.0f32;
                    let mut total_wake = 0.0f32;
                    for body in &bodies {
                        let sample = water_physics_sample_or_idle(&surface, body.pos, 7.5, sample);
                        let submerged = (sample.height - body.pos.y).max(0.0);
                        total_force += submerged * surface.physics.buoyancy * body.mass
                            - body.vel.y * surface.physics.drag;
                        total_wake += water_impact_strength(
                            body.mass,
                            body.vel,
                            surface.physics.wake_strength,
                        );
                    }
                    black_box((total_force, total_wake))
                });
            },
        );
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_water_physics
}
criterion_main!(benches);
