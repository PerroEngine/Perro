use perro_structs::Vector3;
use perro_terrain::{BatchInsertMode, BrushShape, ChunkCoord, TerrainChunk};
use std::time::Instant;

const PERF_RUNS: usize = 9;
const PERF_WARMUP_RUNS: usize = 1;

#[test]
#[ignore]
fn perf_insert_vertex_coplanar_bulk() {
    let points = generate_grid_points(40, 40, -20.0, -20.0, 1.0, 0.0);
    let iterations = points.len();
    let mut samples_s = Vec::with_capacity(PERF_RUNS);
    let mut final_verts = 0usize;
    let mut final_tris = 0usize;

    for run_idx in 0..(PERF_WARMUP_RUNS + PERF_RUNS) {
        let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
        let start = Instant::now();
        for p in &points {
            let _ = chunk.insert_vertex(*p).expect("insert should succeed");
        }
        let elapsed_s = start.elapsed().as_secs_f64();
        if run_idx >= PERF_WARMUP_RUNS {
            samples_s.push(elapsed_s);
        }
        final_verts = chunk.vertex_count();
        final_tris = chunk.triangle_count();
    }

    print_perf_summary(
        "insert_vertex coplanar bulk",
        iterations,
        &samples_s,
        &format!("final_verts={} final_tris={}", final_verts, final_tris),
    );
}

#[test]
#[ignore]
fn perf_insert_vertex_non_coplanar_bulk() {
    let points = generate_wave_points(40, 40, -20.0, -20.0, 1.0, 0.75);
    let iterations = points.len();
    let mut samples_s = Vec::with_capacity(PERF_RUNS);
    let mut final_verts = 0usize;
    let mut final_tris = 0usize;

    for run_idx in 0..(PERF_WARMUP_RUNS + PERF_RUNS) {
        let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
        let start = Instant::now();
        for p in &points {
            let _ = chunk.insert_vertex(*p).expect("insert should succeed");
        }
        let elapsed_s = start.elapsed().as_secs_f64();
        if run_idx >= PERF_WARMUP_RUNS {
            samples_s.push(elapsed_s);
        }
        final_verts = chunk.vertex_count();
        final_tris = chunk.triangle_count();
    }

    print_perf_summary(
        "insert_vertex non-coplanar bulk",
        iterations,
        &samples_s,
        &format!("final_verts={} final_tris={}", final_verts, final_tris),
    );
}

#[test]
#[ignore]
fn perf_insert_vertex_non_coplanar_bulk_batch_mode() {
    let points = generate_wave_points(40, 40, -20.0, -20.0, 1.0, 0.75);
    let iterations = points.len();
    let mut samples_s = Vec::with_capacity(PERF_RUNS);
    let mut last_summary = String::new();
    let mut final_verts = 0usize;
    let mut final_tris = 0usize;

    for run_idx in 0..(PERF_WARMUP_RUNS + PERF_RUNS) {
        let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
        let start = Instant::now();
        let summary = chunk
            .insert_vertices_batch(&points, BatchInsertMode::AssumeNonCoplanar)
            .expect("batch insert should succeed");
        let elapsed_s = start.elapsed().as_secs_f64();
        if run_idx >= PERF_WARMUP_RUNS {
            samples_s.push(elapsed_s);
        }
        final_verts = chunk.vertex_count();
        final_tris = chunk.triangle_count();
        last_summary = format!(
            "inserted={} removed={} skipped={}",
            summary.inserted, summary.removed_as_coplanar, summary.skipped_outside_mesh
        );
    }

    print_perf_summary(
        "insert_vertex non-coplanar bulk (batch)",
        iterations,
        &samples_s,
        &format!("{last_summary} final_verts={final_verts} final_tris={final_tris}"),
    );
}

#[test]
#[ignore]
fn perf_insert_brush_circle_bulk() {
    let centers = generate_grid_points(20, 20, -20.0, -20.0, 2.0, 0.5);
    let iterations = centers.len();
    let mut samples_s = Vec::with_capacity(PERF_RUNS);
    let mut generated_points = 0usize;
    let mut final_verts = 0usize;
    let mut final_tris = 0usize;

    for run_idx in 0..(PERF_WARMUP_RUNS + PERF_RUNS) {
        let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
        let start = Instant::now();
        let mut points_this_run = 0usize;
        for c in &centers {
            let results = chunk
                .insert_brush(*c, 5.0, BrushShape::Circle)
                .expect("brush insert should succeed");
            points_this_run += results.len();
        }
        let elapsed_s = start.elapsed().as_secs_f64();
        if run_idx >= PERF_WARMUP_RUNS {
            samples_s.push(elapsed_s);
        }
        generated_points = points_this_run;
        final_verts = chunk.vertex_count();
        final_tris = chunk.triangle_count();
    }

    print_perf_summary(
        "insert_brush circle bulk",
        iterations,
        &samples_s,
        &format!(
            "generated_points={} final_verts={} final_tris={}",
            generated_points, final_verts, final_tris
        ),
    );
}

#[test]
#[ignore]
fn perf_4096_points_single_coplanar_plane_1m_spacing() {
    let points = generate_centered_grid_points_64(0.0, |_, _, base_y| base_y);
    let iterations = points.len();
    let mut samples_s = Vec::with_capacity(PERF_RUNS);
    let mut removed = 0usize;
    let mut kept = 0usize;
    let mut final_verts = 0usize;
    let mut final_tris = 0usize;

    for run_idx in 0..(PERF_WARMUP_RUNS + PERF_RUNS) {
        let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
        let mut removed_this_run = 0usize;
        let mut kept_this_run = 0usize;
        let start = Instant::now();
        for p in &points {
            let r = chunk.insert_vertex(*p).expect("insert should succeed");
            if r.removed_as_coplanar {
                removed_this_run += 1;
            } else {
                kept_this_run += 1;
            }
        }
        let elapsed_s = start.elapsed().as_secs_f64();
        if run_idx >= PERF_WARMUP_RUNS {
            samples_s.push(elapsed_s);
        }

        assert!(chunk.validate(1.0e-6).is_ok());
        assert_eq!(iterations, 4096);
        assert_eq!(chunk.vertex_count(), 4);
        assert_eq!(chunk.triangle_count(), 2);

        removed = removed_this_run;
        kept = kept_this_run;
        final_verts = chunk.vertex_count();
        final_tris = chunk.triangle_count();
    }

    print_perf_summary(
        "4096 single-plane coplanar",
        iterations,
        &samples_s,
        &format!(
            "removed={} kept={} final_verts={} final_tris={}",
            removed, kept, final_verts, final_tris
        ),
    );
}

#[test]
#[ignore]
fn perf_4096_points_piecewise_coplanar_planes_1m_spacing() {
    let points = generate_centered_grid_points_64(0.0, |x, z, _| {
        if x < 0.0 && z < 0.0 {
            0.0
        } else if x >= 0.0 && z < 0.0 {
            0.08 * x + 1.0
        } else if x < 0.0 && z >= 0.0 {
            -0.06 * z - 1.0
        } else {
            0.04 * x + 0.04 * z + 2.0
        }
    });
    let iterations = points.len();
    let mut samples_s = Vec::with_capacity(PERF_RUNS);
    let mut removed = 0usize;
    let mut kept = 0usize;
    let mut final_verts = 0usize;
    let mut final_tris = 0usize;

    for run_idx in 0..(PERF_WARMUP_RUNS + PERF_RUNS) {
        let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
        let mut removed_this_run = 0usize;
        let mut kept_this_run = 0usize;
        let start = Instant::now();
        for p in &points {
            let r = chunk.insert_vertex(*p).expect("insert should succeed");
            if r.removed_as_coplanar {
                removed_this_run += 1;
            } else {
                kept_this_run += 1;
            }
        }
        let elapsed_s = start.elapsed().as_secs_f64();
        if run_idx >= PERF_WARMUP_RUNS {
            samples_s.push(elapsed_s);
        }

        assert!(chunk.validate(1.0e-6).is_ok());
        assert_eq!(iterations, 4096);
        assert!(chunk.vertex_count() > 4);
        assert!(chunk.vertex_count() <= 4100);
        assert!(removed_this_run > 0);
        assert!(kept_this_run > 0);

        removed = removed_this_run;
        kept = kept_this_run;
        final_verts = chunk.vertex_count();
        final_tris = chunk.triangle_count();
    }

    print_perf_summary(
        "4096 piecewise-planar",
        iterations,
        &samples_s,
        &format!(
            "removed={} kept={} final_verts={} final_tris={}",
            removed, kept, final_verts, final_tris
        ),
    );
}

#[test]
#[ignore]
fn perf_4096_points_piecewise_coplanar_planes_1m_spacing_batch_mode() {
    let points = generate_centered_grid_points_64(0.0, |x, z, _| {
        if x < 0.0 && z < 0.0 {
            0.0
        } else if x >= 0.0 && z < 0.0 {
            0.08 * x + 1.0
        } else if x < 0.0 && z >= 0.0 {
            -0.06 * z - 1.0
        } else {
            0.04 * x + 0.04 * z + 2.0
        }
    });
    let iterations = points.len();
    let mut samples_s = Vec::with_capacity(PERF_RUNS);
    let mut last_summary = String::new();
    let mut final_verts = 0usize;
    let mut final_tris = 0usize;

    for run_idx in 0..(PERF_WARMUP_RUNS + PERF_RUNS) {
        let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
        let start = Instant::now();
        let summary = chunk
            .insert_vertices_batch(&points, BatchInsertMode::Default)
            .expect("batch insert should succeed");
        let elapsed_s = start.elapsed().as_secs_f64();
        if run_idx >= PERF_WARMUP_RUNS {
            samples_s.push(elapsed_s);
        }

        assert!(chunk.validate(1.0e-6).is_ok());
        assert_eq!(iterations, 4096);

        last_summary = format!(
            "inserted={} removed={} skipped={}",
            summary.inserted, summary.removed_as_coplanar, summary.skipped_outside_mesh
        );
        final_verts = chunk.vertex_count();
        final_tris = chunk.triangle_count();
    }

    print_perf_summary(
        "4096 piecewise-planar (batch)",
        iterations,
        &samples_s,
        &format!("{last_summary} final_verts={final_verts} final_tris={final_tris}"),
    );
}

fn print_perf_summary(name: &str, units: usize, samples_s: &[f64], extra: &str) {
    let mut ms_samples: Vec<f64> = samples_s.iter().map(|s| s * 1000.0).collect();
    let mut per_unit_us_samples: Vec<f64> = samples_s
        .iter()
        .map(|s| (s * 1_000_000.0) / (units as f64))
        .collect();
    ms_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    per_unit_us_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mean_ms = mean(&ms_samples);
    let mean_per_unit_us = mean(&per_unit_us_samples);
    let p50_ms = percentile_sorted(&ms_samples, 0.50);
    let p95_ms = percentile_sorted(&ms_samples, 0.95);
    let p50_per_unit_us = percentile_sorted(&per_unit_us_samples, 0.50);
    let p95_per_unit_us = percentile_sorted(&per_unit_us_samples, 0.95);
    let min_ms = *ms_samples.first().unwrap_or(&0.0);
    let min_per_unit_us = *per_unit_us_samples.first().unwrap_or(&0.0);

    println!(
        "[perf] {}: runs={} units={} min_ms={:.3} p50_ms={:.3} p95_ms={:.3} mean_ms={:.3} min_unit_us={:.3} p50_unit_us={:.3} p95_unit_us={:.3} mean_unit_us={:.3} {}",
        name,
        samples_s.len(),
        units,
        min_ms,
        p50_ms,
        p95_ms,
        mean_ms,
        min_per_unit_us,
        p50_per_unit_us,
        p95_per_unit_us,
        mean_per_unit_us,
        extra
    );
}

fn percentile_sorted(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let p = p.clamp(0.0, 1.0);
    let idx = ((values.len() - 1) as f64 * p).round() as usize;
    values[idx]
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn generate_grid_points(
    nx: usize,
    nz: usize,
    origin_x: f32,
    origin_z: f32,
    step: f32,
    y: f32,
) -> Vec<Vector3> {
    let mut out = Vec::with_capacity(nx * nz);
    for ix in 0..nx {
        for iz in 0..nz {
            out.push(Vector3::new(
                origin_x + ix as f32 * step,
                y,
                origin_z + iz as f32 * step,
            ));
        }
    }
    out
}

fn generate_centered_grid_points_64<F>(base_y: f32, mut y_fn: F) -> Vec<Vector3>
where
    F: FnMut(f32, f32, f32) -> f32,
{
    let mut out = Vec::with_capacity(64 * 64);
    for ix in 0..64 {
        for iz in 0..64 {
            let x = -31.5 + ix as f32;
            let z = -31.5 + iz as f32;
            let y = y_fn(x, z, base_y);
            out.push(Vector3::new(x, y, z));
        }
    }
    out
}

fn generate_wave_points(
    nx: usize,
    nz: usize,
    origin_x: f32,
    origin_z: f32,
    step: f32,
    amplitude: f32,
) -> Vec<Vector3> {
    let mut out = Vec::with_capacity(nx * nz);
    for ix in 0..nx {
        for iz in 0..nz {
            let x = origin_x + ix as f32 * step;
            let z = origin_z + iz as f32 * step;
            let y = (x * 0.17).sin() * (z * 0.11).cos() * amplitude;
            out.push(Vector3::new(x, y, z));
        }
    }
    out
}
