use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_meshlets::{DEFAULT_LOD_TARGET_RATIOS, LodSurfaceRange, LodVertex, build_lod_sets};

fn grid_mesh(size: u32) -> (Vec<LodVertex>, Vec<u32>, Vec<LodSurfaceRange>) {
    let mut vertices = Vec::with_capacity(((size + 1) * (size + 1)) as usize);
    for y in 0..=size {
        for x in 0..=size {
            vertices.push(LodVertex {
                position: [x as f32, y as f32, ((x * 13 + y * 7) % 5) as f32 * 0.02],
                normal: [0.0, 0.0, 1.0],
                uv: [x as f32 / size as f32, y as f32 / size as f32],
            });
        }
    }
    let stride = size + 1;
    let mut indices = Vec::with_capacity((size * size * 6) as usize);
    for y in 0..size {
        for x in 0..size {
            let a = y * stride + x;
            let b = a + 1;
            let c = a + stride;
            let d = c + 1;
            indices.extend_from_slice(&[a, b, d, a, d, c]);
        }
    }
    let surfaces = vec![LodSurfaceRange {
        index_start: 0,
        index_count: indices.len() as u32,
    }];
    (vertices, indices, surfaces)
}

fn split_surfaces(indices: &[u32], surface_count: usize) -> Vec<LodSurfaceRange> {
    let tri_count = indices.len() / 3;
    let tris_per_surface = tri_count.div_ceil(surface_count);
    let mut surfaces = Vec::new();
    let mut tri_start = 0usize;
    while tri_start < tri_count {
        let tri_end = (tri_start + tris_per_surface).min(tri_count);
        surfaces.push(LodSurfaceRange {
            index_start: (tri_start * 3) as u32,
            index_count: ((tri_end - tri_start) * 3) as u32,
        });
        tri_start = tri_end;
    }
    surfaces
}

fn bench_lod_builder(c: &mut Criterion) {
    let mut group = c.benchmark_group("lod_builder_grid");
    group.sample_size(10);
    for size in [8u32, 16, 24] {
        let (vertices, indices, surfaces) = grid_mesh(size);
        let tri_count = indices.len() / 3;
        group.bench_with_input(BenchmarkId::from_parameter(tri_count), &size, |b, _| {
            b.iter(|| {
                build_lod_sets(
                    black_box(&vertices),
                    black_box(&indices),
                    black_box(&surfaces),
                    black_box(&DEFAULT_LOD_TARGET_RATIOS),
                )
            });
        });
    }
    group.finish();

    let mut parallel = c.benchmark_group("lod_builder_multi_surface");
    parallel.sample_size(10);
    for &(size, surface_count) in &[(24u32, 4usize), (32u32, 8usize)] {
        let (vertices, indices, _) = grid_mesh(size);
        let surfaces = split_surfaces(&indices, surface_count);
        let tri_count = indices.len() / 3;
        parallel.bench_with_input(
            BenchmarkId::new(format!("{surface_count}_surfaces"), tri_count),
            &(size, surface_count),
            |b, _| {
                b.iter(|| {
                    build_lod_sets(
                        black_box(&vertices),
                        black_box(&indices),
                        black_box(&surfaces),
                        black_box(&DEFAULT_LOD_TARGET_RATIOS),
                    )
                });
            },
        );
    }
    parallel.finish();
}

criterion_group!(benches, bench_lod_builder);
criterion_main!(benches);
