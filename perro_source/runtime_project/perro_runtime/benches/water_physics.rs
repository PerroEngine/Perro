use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_nodes::{
    WaterIdleMode, WaterPhysicsSample, WaterShape, WaterSurfaceParams, water_impact_strength,
    water_physics_sample_or_idle,
};
use perro_structs::Vector2;
use rayon::prelude::*;

const WATER_FORCE_PAR_BODY_THRESHOLD: usize = 1024;

#[derive(Clone, Copy)]
struct Body2 {
    pos: Vector2,
    vel: Vector2,
    mass: f32,
}

#[derive(Clone, Copy)]
struct LinkedWater2 {
    center: Vector2,
    half: Vector2,
    surface: WaterSurfaceParams,
}

struct LinkedWaterIndex {
    waters: Vec<LinkedWater2>,
    bins: Vec<Vec<usize>>,
    origin_x: f32,
    inv_cell_width: f32,
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

fn crossing_bodies(count: usize, water_count: usize) -> Vec<Body2> {
    let span = water_count as f32 * 14.0;
    (0..count)
        .map(|i| Body2 {
            pos: Vector2::new((i as f32 * 3.7) % span - 8.0, -1.0 - (i % 5) as f32 * 0.1),
            vel: Vector2::new(4.0 + (i % 13) as f32 * 0.2, -1.0),
            mass: 1.0 + (i % 17) as f32 * 0.15,
        })
        .collect()
}

fn linked_waters(count: usize, resolution: u32) -> Vec<LinkedWater2> {
    (0..count)
        .map(|i| {
            let mut surface = water_surface(resolution);
            surface.shape = WaterShape::rect(Vector2::new(16.0, 16.0));
            surface.physics.buoyancy = 2.0;
            surface.physics.drag = 0.35;
            LinkedWater2 {
                center: Vector2::new(i as f32 * 14.0, 0.0),
                half: Vector2::new(8.0, 8.0),
                surface,
            }
        })
        .collect()
}

impl LinkedWaterIndex {
    fn new(waters: Vec<LinkedWater2>) -> Self {
        let (bins, origin_x, inv_cell_width) =
            build_bins(waters.iter().map(|water| (water.center.x, water.half.x)));
        Self {
            waters,
            bins,
            origin_x,
            inv_cell_width,
        }
    }
}

fn build_bins(waters: impl Iterator<Item = (f32, f32)> + Clone) -> (Vec<Vec<usize>>, f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_half_x = 0.0f32;
    let mut count = 0usize;
    for (x, half_x) in waters.clone() {
        min_x = min_x.min(x - half_x);
        max_x = max_x.max(x + half_x);
        max_half_x = max_half_x.max(half_x);
        count += 1;
    }
    if count == 0 {
        return (Vec::new(), 0.0, 1.0);
    }
    let inv_cell_width = 1.0 / max_half_x.max(1.0);
    let bin_count = (((max_x - min_x) * inv_cell_width).ceil() as usize)
        .saturating_add(1)
        .max(1);
    let mut bins = vec![Vec::new(); bin_count];
    for (idx, (x, half_x)) in waters.enumerate() {
        let first = (((x - half_x - min_x) * inv_cell_width).floor() as isize)
            .clamp(0, bin_count.saturating_sub(1) as isize) as usize;
        let last = (((x + half_x - min_x) * inv_cell_width).floor() as isize)
            .clamp(0, bin_count.saturating_sub(1) as isize) as usize;
        for bin in &mut bins[first..=last] {
            bin.push(idx);
        }
    }
    (bins, min_x, inv_cell_width)
}

fn water_surface(resolution: u32) -> WaterSurfaceParams {
    let mut surface = WaterSurfaceParams {
        shape: WaterShape::rect(Vector2::new(128.0, 128.0)),
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

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn linked_water_force(
    body: Body2,
    waters: &LinkedWaterIndex,
    cached: Option<WaterPhysicsSample>,
) -> f32 {
    let mut total_weight = 0.0;
    let mut surface_y = 0.0;
    let mut buoyancy = 0.0;
    let mut drag = 0.0;
    let bin = ((body.pos.x - waters.origin_x) * waters.inv_cell_width).floor() as isize;
    if bin < 0 || bin as usize >= waters.bins.len() {
        return 0.0;
    }
    for &idx in &waters.bins[bin as usize] {
        let water = waters.waters[idx];
        let local = body.pos - water.center;
        if local.x.abs() > water.half.x || local.y.abs() > water.half.y {
            continue;
        }
        let edge = (water.half.x - local.x.abs()).min(water.half.y - local.y.abs());
        let weight = smoothstep((edge / 2.0).clamp(0.0, 1.0)).max(0.001);
        let sample = water_physics_sample_or_idle(&water.surface, local, 7.5, cached);
        total_weight += weight;
        surface_y += (water.center.y + sample.height) * weight;
        buoyancy += water.surface.physics.buoyancy * weight;
        drag += water.surface.physics.drag * weight;
    }
    if total_weight <= 0.0 {
        return 0.0;
    }
    let inv = 1.0 / total_weight;
    let submerged = (surface_y * inv - body.pos.y).max(0.0);
    submerged * buoyancy * inv * body.mass - body.vel.y * drag * inv
}

fn bench_linked_water_physics(c: &mut Criterion) {
    let mut group = c.benchmark_group("runtime_linked_water_physics");
    for (body_count, water_count, cached) in [
        (100usize, 8usize, true),
        (1_000, 16, true),
        (10_000, 32, true),
        (10_000, 64, true),
        (10_000, 128, true),
        (1_000, 16, false),
    ] {
        group.bench_with_input(
            BenchmarkId::new(
                format!("{body_count}_bodies"),
                format!(
                    "{water_count}_linked_{}",
                    if cached { "cached" } else { "fallback" }
                ),
            ),
            &(body_count, water_count, cached),
            |b, &(body_count, water_count, cached)| {
                let bodies = crossing_bodies(body_count, water_count);
                let waters = LinkedWaterIndex::new(linked_waters(water_count, 128));
                let sample = cached.then_some(WaterPhysicsSample {
                    height: 0.75,
                    velocity: Vector2::new(0.1, 0.0),
                    foam: 0.2,
                });
                b.iter(|| {
                    let total_force = if bodies.len() >= WATER_FORCE_PAR_BODY_THRESHOLD {
                        bodies
                            .par_iter()
                            .map(|body| linked_water_force(*body, &waters, sample))
                            .sum::<f32>()
                    } else {
                        bodies
                            .iter()
                            .map(|body| linked_water_force(*body, &waters, sample))
                            .sum::<f32>()
                    };
                    black_box(total_force)
                });
            },
        );
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_water_physics, bench_linked_water_physics
}
criterion_main!(benches);
