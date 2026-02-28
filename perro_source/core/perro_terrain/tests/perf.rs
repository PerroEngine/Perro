use perro_structs::Vector3;
use perro_terrain::{BrushShape, ChunkCoord, TerrainChunk};
use std::time::Instant;

#[test]
#[ignore]
fn perf_insert_vertex_coplanar_bulk() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let points = generate_grid_points(40, 40, -20.0, -20.0, 1.0, 0.0);
    let iterations = points.len();

    let start = Instant::now();
    for p in points {
        let _ = chunk.insert_vertex(p).expect("insert should succeed");
    }
    let elapsed = start.elapsed();

    println!(
        "[perf] insert_vertex coplanar bulk: iters={} total_ms={:.3} per_op_us={:.3} final_verts={} final_tris={}",
        iterations,
        elapsed.as_secs_f64() * 1000.0,
        elapsed.as_secs_f64() * 1_000_000.0 / iterations as f64,
        chunk.vertex_count(),
        chunk.triangle_count()
    );
}

#[test]
#[ignore]
fn perf_insert_vertex_non_coplanar_bulk() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let points = generate_wave_points(40, 40, -20.0, -20.0, 1.0, 0.75);
    let iterations = points.len();

    let start = Instant::now();
    for p in points {
        let _ = chunk.insert_vertex(p).expect("insert should succeed");
    }
    let elapsed = start.elapsed();

    println!(
        "[perf] insert_vertex non-coplanar bulk: iters={} total_ms={:.3} per_op_us={:.3} final_verts={} final_tris={}",
        iterations,
        elapsed.as_secs_f64() * 1000.0,
        elapsed.as_secs_f64() * 1_000_000.0 / iterations as f64,
        chunk.vertex_count(),
        chunk.triangle_count()
    );
}

#[test]
#[ignore]
fn perf_insert_brush_circle_bulk() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let centers = generate_grid_points(20, 20, -20.0, -20.0, 2.0, 0.5);
    let iterations = centers.len();

    let start = Instant::now();
    let mut generated_points = 0usize;
    for c in centers {
        let results = chunk
            .insert_brush(c, 5.0, BrushShape::Circle)
            .expect("brush insert should succeed");
        generated_points += results.len();
    }
    let elapsed = start.elapsed();

    println!(
        "[perf] insert_brush circle bulk: brush_iters={} generated_points={} total_ms={:.3} per_brush_us={:.3} final_verts={} final_tris={}",
        iterations,
        generated_points,
        elapsed.as_secs_f64() * 1000.0,
        elapsed.as_secs_f64() * 1_000_000.0 / iterations as f64,
        chunk.vertex_count(),
        chunk.triangle_count()
    );
}

#[test]
#[ignore]
fn perf_4096_points_single_coplanar_plane_1m_spacing() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let points = generate_centered_grid_points_64(0.0, |_, _, base_y| base_y);
    let iterations = points.len();
    let mut removed = 0usize;
    let mut kept = 0usize;

    let start = Instant::now();
    for p in points {
        let r = chunk.insert_vertex(p).expect("insert should succeed");
        if r.removed_as_coplanar {
            removed += 1;
        } else {
            kept += 1;
        }
    }
    let elapsed = start.elapsed();

    assert!(chunk.validate(1.0e-6).is_ok());
    assert_eq!(iterations, 4096);
    assert_eq!(chunk.vertex_count(), 4);
    assert_eq!(chunk.triangle_count(), 2);

    println!(
        "[perf] 4096 single-plane coplanar: iters={} removed={} kept={} total_ms={:.3} per_op_us={:.3} final_verts={} final_tris={}",
        iterations,
        removed,
        kept,
        elapsed.as_secs_f64() * 1000.0,
        elapsed.as_secs_f64() * 1_000_000.0 / iterations as f64,
        chunk.vertex_count(),
        chunk.triangle_count()
    );
}

#[test]
#[ignore]
fn perf_4096_points_piecewise_coplanar_planes_1m_spacing() {
    let mut chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
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
    let mut removed = 0usize;
    let mut kept = 0usize;

    let start = Instant::now();
    for p in points {
        let r = chunk.insert_vertex(p).expect("insert should succeed");
        if r.removed_as_coplanar {
            removed += 1;
        } else {
            kept += 1;
        }
    }
    let elapsed = start.elapsed();

    assert!(chunk.validate(1.0e-6).is_ok());
    assert_eq!(iterations, 4096);
    assert!(chunk.vertex_count() > 4);
    assert!(chunk.vertex_count() <= 4100);
    assert!(removed > 0);
    assert!(kept > 0);

    println!(
        "[perf] 4096 piecewise-planar: iters={} removed={} kept={} total_ms={:.3} per_op_us={:.3} final_verts={} final_tris={}",
        iterations,
        removed,
        kept,
        elapsed.as_secs_f64() * 1000.0,
        elapsed.as_secs_f64() * 1_000_000.0 / iterations as f64,
        chunk.vertex_count(),
        chunk.triangle_count()
    );
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
