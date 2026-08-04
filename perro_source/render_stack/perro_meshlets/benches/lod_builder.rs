use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use gltf::Gltf;
use perro_meshlets::{DEFAULT_LOD_TARGET_RATIOS, LodSurfaceRange, LodVertex, build_lod_sets};
use std::time::Duration;

const ROCK_GLB: &[u8] = include_bytes!("../../../../demos/Demo3D/res/models/rock_a.glb");

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

fn decode_glb(bytes: &[u8]) -> (Vec<LodVertex>, Vec<u32>) {
    let Gltf { document, blob } = Gltf::from_slice(bytes).expect("benchmark GLB must parse");
    let buffers = gltf::import_buffers(&document, None, blob).expect("benchmark buffers must load");
    let mesh = document
        .meshes()
        .next()
        .expect("benchmark GLB must hold mesh");
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for primitive in mesh.primitives() {
        let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|b| b.0.as_slice()));
        let positions = reader
            .read_positions()
            .expect("benchmark primitive must hold positions")
            .collect::<Vec<_>>();
        let normals = reader
            .read_normals()
            .map(Iterator::collect::<Vec<_>>)
            .unwrap_or_default();
        let uvs = reader
            .read_tex_coords(0)
            .map(|values| values.into_f32().collect::<Vec<_>>())
            .unwrap_or_default();
        let base = vertices.len() as u32;
        vertices.extend(
            positions
                .iter()
                .enumerate()
                .map(|(index, &position)| LodVertex {
                    position,
                    normal: normals.get(index).copied().unwrap_or([0.0, 1.0, 0.0]),
                    uv: uvs.get(index).copied().unwrap_or([0.0, 0.0]),
                }),
        );
        if let Some(source_indices) = reader.read_indices() {
            indices.extend(source_indices.into_u32().map(|index| base + index));
        } else {
            indices.extend((0..positions.len() as u32).map(|index| base + index));
        }
    }
    (vertices, indices)
}

fn repeat_mesh(
    source_vertices: &[LodVertex],
    source_indices: &[u32],
    copies: usize,
    split: bool,
) -> (Vec<LodVertex>, Vec<u32>, Vec<LodSurfaceRange>) {
    let mut vertices = Vec::with_capacity(source_vertices.len() * copies);
    let mut indices = Vec::with_capacity(source_indices.len() * copies);
    let mut surfaces = Vec::with_capacity(if split { copies } else { 1 });
    let row = (copies as f32).sqrt().ceil() as usize;
    for copy in 0..copies {
        let base_vertex = vertices.len() as u32;
        let x = (copy % row) as f32 * 4.0;
        let z = (copy / row) as f32 * 4.0;
        vertices.extend(source_vertices.iter().map(|vertex| {
            let mut vertex = *vertex;
            vertex.position[0] += x;
            vertex.position[2] += z;
            vertex
        }));
        let index_start = indices.len() as u32;
        indices.extend(source_indices.iter().map(|index| base_vertex + index));
        if split {
            surfaces.push(LodSurfaceRange {
                index_start,
                index_count: source_indices.len() as u32,
            });
        }
    }
    if !split {
        surfaces.push(LodSurfaceRange {
            index_start: 0,
            index_count: indices.len() as u32,
        });
    }
    (vertices, indices, surfaces)
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

    let mut huge = c.benchmark_group("lod_builder_huge_grid");
    huge.sample_size(10);
    huge.warm_up_time(Duration::from_secs(2));
    huge.measurement_time(Duration::from_secs(15));
    for size in [256u32, 512] {
        let (vertices, indices, surfaces) = grid_mesh(size);
        huge.bench_with_input(
            BenchmarkId::from_parameter(indices.len() / 3),
            &size,
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
    huge.finish();

    let (rock_vertices, rock_indices) = decode_glb(ROCK_GLB);
    let mut glb = c.benchmark_group("lod_builder_tiled_glb");
    glb.sample_size(10);
    glb.warm_up_time(Duration::from_secs(2));
    glb.measurement_time(Duration::from_secs(15));
    for &(copies, split) in &[(64usize, false), (256, false), (256, true), (512, true)] {
        let (vertices, indices, surfaces) =
            repeat_mesh(&rock_vertices, &rock_indices, copies, split);
        let label = if split { "split" } else { "merged" };
        glb.bench_with_input(
            BenchmarkId::new(label, indices.len() / 3),
            &(copies, split),
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
    glb.finish();
}

criterion_group!(benches, bench_lod_builder);
criterion_main!(benches);
