use super::*;
use std::hint::black_box;
use std::time::{Duration, Instant};

fn tri_workload(tri_count: usize, surface_count: usize) -> f32 {
    let mut out = 0.0_f32;
    let p = Vec3::new(0.13, 0.21, -0.37);
    for tri in 0..tri_count {
        let s = (tri % surface_count.max(1)) as f32 * 0.0001;
        let a = Vec3::new(s, 0.1 + s, -0.2);
        let b = Vec3::new(0.4 + s, 0.3, 0.15);
        let c = Vec3::new(-0.2, 0.5 + s, 0.35);
        out += closest_point_on_triangle(p, a, b, c).length_squared();
    }
    out
}

fn build_synth_query_mesh(tri_count: usize, surface_count: usize) -> QueryMeshData {
    let surface_count = surface_count.max(1);
    let mut vertices = Vec::with_capacity(tri_count.saturating_mul(3));
    let mut triangles = Vec::with_capacity(tri_count);
    for tri_index in 0..tri_count {
        let base = (tri_index * 3) as u32;
        let s = (tri_index % surface_count) as f32;
        let x = tri_index as f32 * 0.0003;
        vertices.push(Vec3::new(x, s * 0.0002, 0.0));
        vertices.push(Vec3::new(x + 0.0001, 0.001 + s * 0.0001, 0.0));
        vertices.push(Vec3::new(x, 0.0002 + s * 0.0001, 0.001));
        triangles.push(QueryTri {
            a: base,
            b: base + 1,
            c: base + 2,
            surface_index: (tri_index % surface_count) as u32,
        });
    }
    build_query_mesh_data(vertices, triangles).expect("synth query mesh")
}

#[test]
fn runtime_mesh_data_builds_query_surfaces() {
    let vertex = |position| perro_render_bridge::RuntimeMeshVertex {
        position,
        normal: [0.0, 1.0, 0.0],
        uv: [0.0, 0.0],
        joints: [0; 4],
        weights: [0.0; 4],
    };
    let mesh = Mesh3D {
        vertices: vec![
            vertex([0.0, 0.0, 0.0]),
            vertex([1.0, 0.0, 0.0]),
            vertex([0.0, 1.0, 0.0]),
            vertex([0.0, 0.0, 1.0]),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        surface_ranges: vec![
            perro_render_bridge::MeshSurfaceRange {
                index_start: 0,
                index_count: 3,
            },
            perro_render_bridge::MeshSurfaceRange {
                index_start: 3,
                index_count: 3,
            },
        ],
    };

    let query = build_query_mesh_from_runtime_mesh(&mesh).expect("query mesh");

    assert_eq!(query.triangles.len(), 2);
    assert_eq!(query.triangles[0].surface_index, 0);
    assert_eq!(query.triangles[1].surface_index, 1);
}

fn point_query_workload(mesh: &QueryMeshData, p_local: Vec3) -> f32 {
    if mesh.bvh_nodes.is_empty() {
        return 0.0;
    }
    let mut best_metric = f32::INFINITY;
    let mut best_surface = 0_u32;
    let mut stack = vec![0_u32];
    while let Some(node_idx) = stack.pop() {
        let Some(bvh) = mesh.bvh_nodes.get(node_idx as usize).copied() else {
            continue;
        };
        let node_d2 = aabb_distance2(p_local, bvh.aabb_min, bvh.aabb_max);
        if node_d2 >= best_metric {
            continue;
        }
        if bvh.left == u32::MAX || bvh.right == u32::MAX {
            let start = bvh.tri_start as usize;
            let end = start + bvh.tri_count as usize;
            for &tri_idx in &mesh.bvh_tri_indices[start..end] {
                let tri_idx = tri_idx as usize;
                let tri = mesh.triangles[tri_idx];
                let acc = mesh.tri_accel[tri_idx];
                let tri_d2 = aabb_distance2(p_local, acc.aabb_min, acc.aabb_max);
                if tri_d2 >= best_metric {
                    continue;
                }
                let a = mesh.vertices[tri.a as usize];
                let b = mesh.vertices[tri.b as usize];
                let c = mesh.vertices[tri.c as usize];
                let nearest_local = closest_point_on_triangle(p_local, a, b, c);
                let d2 = nearest_local.distance_squared(p_local);
                if d2 < best_metric {
                    best_metric = d2;
                    best_surface = tri.surface_index;
                }
            }
        } else {
            let left = mesh.bvh_nodes[bvh.left as usize];
            let right = mesh.bvh_nodes[bvh.right as usize];
            let ld2 = aabb_distance2(p_local, left.aabb_min, left.aabb_max);
            let rd2 = aabb_distance2(p_local, right.aabb_min, right.aabb_max);
            if ld2 < rd2 {
                stack.push(bvh.right);
                stack.push(bvh.left);
            } else {
                stack.push(bvh.left);
                stack.push(bvh.right);
            }
        }
    }
    if best_metric.is_finite() {
        best_metric + best_surface as f32 * 1e-6
    } else {
        0.0
    }
}

fn measure_us_per_query(mesh: &QueryMeshData) -> f64 {
    let points = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(0.2, 0.1, -0.1),
        Vec3::new(-0.1, 0.3, 0.2),
        Vec3::new(0.4, -0.2, 0.15),
        Vec3::new(0.05, 0.07, -0.11),
        Vec3::new(-0.33, 0.44, 0.12),
        Vec3::new(0.6, -0.1, 0.05),
        Vec3::new(-0.25, -0.15, 0.4),
    ];
    let mut iters = 64usize;
    loop {
        let start = Instant::now();
        let mut acc = 0.0_f32;
        for i in 0..iters {
            acc += point_query_workload(mesh, points[i % points.len()]);
        }
        let elapsed = start.elapsed();
        black_box(acc);
        if elapsed >= Duration::from_millis(25) || iters >= (1 << 22) {
            return elapsed.as_secs_f64() * 1_000_000.0 / iters as f64;
        }
        iters *= 2;
    }
}

#[test]
#[ignore]
fn bench_mesh_query_parallel_threshold_sweep() {
    let instances = [1usize, 2, 4, 8, 16, 32];
    let triangles = [128usize, 512, 2048, 4096, 8192, 16384];
    let surfaces = [1usize, 2, 4, 8, 16];
    let rounds = 20usize;
    println!("inst,tri,surface,serial_us,parallel_us,speedup");
    for &inst in &instances {
        for &tri in &triangles {
            for &surface in &surfaces {
                let mut serial_acc = 0.0_f32;
                let serial_start = Instant::now();
                for _ in 0..rounds {
                    for _ in 0..inst {
                        serial_acc += tri_workload(tri, surface);
                    }
                }
                let serial_us = serial_start.elapsed().as_micros();

                let mut par_acc = 0.0_f32;
                let parallel_start = Instant::now();
                for _ in 0..rounds {
                    par_acc += (0..inst)
                        .into_par_iter()
                        .map(|_| tri_workload(tri, surface))
                        .sum::<f32>();
                }
                let parallel_us = parallel_start.elapsed().as_micros();
                black_box(serial_acc);
                black_box(par_acc);
                let speedup = serial_us as f64 / parallel_us.max(1) as f64;
                println!("{inst},{tri},{surface},{serial_us},{parallel_us},{speedup:.3}");
            }
        }
    }
}

#[test]
#[ignore]
fn bench_mesh_query_fixed_vertex_count_latency() {
    const TARGET_VERTICES: usize = 1_000_000;
    let surface_counts = [1usize, 4, 16, 64, 256];
    let tri_count = (TARGET_VERTICES / 3).max(1);
    let vertex_count = tri_count.saturating_mul(3);
    println!("running tests w/ vertices={vertex_count}, triangles={tri_count}");
    println!("surfaces,vertices,triangles,time_to_query_us");
    for &surface_count in &surface_counts {
        let mesh = build_synth_query_mesh(tri_count, surface_count);
        let mut samples = [
            measure_us_per_query(&mesh),
            measure_us_per_query(&mesh),
            measure_us_per_query(&mesh),
        ];
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let time_to_query_us = samples[1];
        println!("{surface_count},{vertex_count},{tri_count},{time_to_query_us:.6}");
    }
}
