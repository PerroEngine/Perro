use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_structs::Vector3;

fn closest_point_on_triangle(p: Vector3, a: Vector3, b: Vector3, c: Vector3) -> Vector3 {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

fn mesh_query_workload(tri_count: usize, surface_count: usize) -> f32 {
    let mut out = 0.0_f32;
    let p = Vector3::new(0.13, 0.21, -0.37);
    for tri in 0..tri_count {
        let s = (tri % surface_count.max(1)) as f32 * 0.0001;
        let a = Vector3::new(s, 0.1 + s, -0.2);
        let b = Vector3::new(0.4 + s, 0.3, 0.15);
        let c = Vector3::new(-0.2, 0.5 + s, 0.35);
        out += closest_point_on_triangle(p, a, b, c).length_squared();
    }
    out
}

fn bench_mesh_query_synthetic(c: &mut Criterion) {
    let mut group = c.benchmark_group("mesh_query/synthetic");
    for (triangles, surfaces) in [(128usize, 1usize), (2_048, 8), (16_384, 16)] {
        group.bench_with_input(
            BenchmarkId::new(
                "closest_point_scan",
                format!("{triangles}_tri_{surfaces}_surf"),
            ),
            &(triangles, surfaces),
            |b, &(triangles, surfaces)| {
                b.iter(|| {
                    black_box(mesh_query_workload(
                        black_box(triangles),
                        black_box(surfaces),
                    ))
                })
            },
        );
    }
    group.finish();
}

fn benches(c: &mut Criterion) {
    bench_mesh_query_synthetic(c);
}

criterion_group! {
    name = mesh_query_hotpaths;
    config = Criterion::default().sample_size(10);
    targets = benches
}
criterion_main!(mesh_query_hotpaths);
