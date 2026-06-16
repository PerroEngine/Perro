use criterion::{Criterion, black_box, criterion_group, criterion_main};
use perro_structs::{Matrix, Quaternion, Transform2D, Transform3D, Vector2, Vector3};
use rayon::prelude::*;

const N: usize = 16_384;
const HUGE_N: usize = 512;

fn make_vec2s() -> Vec<Vector2> {
    (0..N)
        .map(|i| {
            let f = i as f32;
            Vector2::new(f.mul_add(0.013, 1.0), f.mul_add(0.021, 2.0))
        })
        .collect()
}

fn make_vec3s() -> Vec<Vector3> {
    (0..N)
        .map(|i| {
            let f = i as f32;
            Vector3::new(
                f.mul_add(0.011, 1.0),
                f.mul_add(0.017, 2.0),
                f.mul_add(0.023, 3.0),
            )
        })
        .collect()
}

fn make_quats() -> Vec<Quaternion> {
    (0..N)
        .map(|i| {
            let f = i as f32;
            Quaternion::from_euler_xyz(f * 0.0003, f * 0.0005, f * 0.0007)
        })
        .collect()
}

fn make_transforms_2d() -> Vec<Transform2D> {
    (0..N)
        .map(|i| {
            let f = i as f32;
            Transform2D::new(
                Vector2::new(f * 0.125, f * -0.25),
                f * 0.0009,
                Vector2::new(1.0 + (i % 7) as f32 * 0.01, 1.0 + (i % 11) as f32 * 0.01),
            )
        })
        .collect()
}

fn make_transforms_3d() -> Vec<Transform3D> {
    (0..N)
        .map(|i| {
            let f = i as f32;
            Transform3D::new(
                Vector3::new(f * 0.125, f * -0.25, f * 0.03125),
                Quaternion::from_euler_xyz(f * 0.0003, f * 0.0005, f * 0.0007),
                Vector3::new(
                    1.0 + (i % 7) as f32 * 0.01,
                    1.0 + (i % 11) as f32 * 0.01,
                    1.0 + (i % 13) as f32 * 0.01,
                ),
            )
        })
        .collect()
}

fn make_matrix4s() -> Vec<Matrix<4, 4>> {
    (0..N)
        .map(|i| {
            let f = i as f32;
            Matrix::<4, 4>::new([
                [
                    f.mul_add(0.011, 1.0),
                    f.mul_add(0.017, 2.0),
                    f.mul_add(0.023, 3.0),
                    1.0,
                ],
                [
                    f.mul_add(0.031, 4.0),
                    f.mul_add(0.037, 5.0),
                    f.mul_add(0.041, 6.0),
                    1.0,
                ],
                [
                    f.mul_add(0.043, 7.0),
                    f.mul_add(0.047, 8.0),
                    f.mul_add(0.053, 9.0),
                    1.0,
                ],
                [
                    f.mul_add(0.059, 10.0),
                    f.mul_add(0.061, 11.0),
                    f.mul_add(0.067, 12.0),
                    1.0,
                ],
            ])
        })
        .collect()
}

fn make_matrix3s() -> Vec<Matrix<3, 3>> {
    (0..N)
        .map(|i| {
            let f = i as f32;
            Matrix::<3, 3>::new([
                [
                    f.mul_add(0.011, 1.0),
                    f.mul_add(0.017, 2.0),
                    f.mul_add(0.023, 3.0),
                ],
                [
                    f.mul_add(0.031, 4.0),
                    f.mul_add(0.037, 5.0),
                    f.mul_add(0.041, 6.0),
                ],
                [
                    f.mul_add(0.043, 7.0),
                    f.mul_add(0.047, 8.0),
                    f.mul_add(0.053, 9.0),
                ],
            ])
        })
        .collect()
}

fn make_matrix2s() -> Vec<Matrix<2, 2>> {
    (0..N)
        .map(|i| {
            let f = i as f32;
            Matrix::<2, 2>::new([
                [f.mul_add(0.011, 1.0), f.mul_add(0.017, 2.0)],
                [f.mul_add(0.031, 4.0), f.mul_add(0.037, 5.0)],
            ])
        })
        .collect()
}

fn make_matrix2x3s() -> Vec<Matrix<2, 3>> {
    (0..N)
        .map(|i| {
            let f = i as f32;
            Matrix::<2, 3>::new([
                [
                    f.mul_add(0.011, 1.0),
                    f.mul_add(0.017, 2.0),
                    f.mul_add(0.023, 3.0),
                ],
                [
                    f.mul_add(0.031, 4.0),
                    f.mul_add(0.037, 5.0),
                    f.mul_add(0.041, 6.0),
                ],
            ])
        })
        .collect()
}

fn make_matrices<const ROWS: usize, const COLS: usize>(len: usize) -> Vec<Matrix<ROWS, COLS>> {
    (0..len)
        .map(|i| {
            let base = i as f32;
            Matrix::<ROWS, COLS>::new(std::array::from_fn(|r| {
                std::array::from_fn(|c| {
                    base.mul_add(0.0031, 1.0) + (r as f32).mul_add(0.17, c as f32 * 0.11)
                })
            }))
        })
        .collect()
}

fn bench_vector2_bulk_ops(c: &mut Criterion) {
    let a = make_vec2s();
    let b = make_vec2s();

    c.bench_function("perro_structs/vector2_bulk_ops", |bench| {
        bench.iter(|| {
            let mut acc = Vector2::ZERO;
            let mut scalar = 0.0f32;
            for (&lhs, &rhs) in black_box(a.iter().zip(&b)) {
                let mixed = (lhs + rhs) * 0.5 - rhs.normalized();
                scalar += lhs.dot(rhs) + lhs.cross(rhs) + lhs.distance_to(rhs);
                acc += mixed.lerped(rhs, 0.25);
            }
            black_box((acc, scalar))
        })
    });
}

fn bench_vector3_bulk_ops(c: &mut Criterion) {
    let a = make_vec3s();
    let b = make_vec3s();

    c.bench_function("perro_structs/vector3_bulk_ops", |bench| {
        bench.iter(|| {
            let mut acc = Vector3::ZERO;
            let mut scalar = 0.0f32;
            for (&lhs, &rhs) in black_box(a.iter().zip(&b)) {
                let mixed = (lhs + rhs) * 0.5 - rhs.normalized();
                scalar += lhs.dot(rhs) + lhs.cross(rhs).length() + lhs.distance_to(rhs);
                acc += mixed.lerped(rhs.project_on(lhs), 0.25);
            }
            black_box((acc, scalar))
        })
    });
}

fn bench_quaternion_bulk_ops(c: &mut Criterion) {
    let quats = make_quats();
    let vectors = make_vec3s();

    c.bench_function("perro_structs/quaternion_bulk_ops", |bench| {
        bench.iter(|| {
            let mut acc = Quaternion::IDENTITY;
            let mut vec_acc = Vector3::ZERO;
            let mut scalar = 0.0f32;
            for (&q, &v) in black_box(quats.iter().zip(&vectors)) {
                let mixed = acc.mul_quat(q).normalized();
                vec_acc += mixed.rotate_vector3(v);
                scalar += mixed.dot(q) + mixed.inverse().dot(acc);
                acc = mixed.slerped(q, 0.35);
            }
            black_box((acc, vec_acc, scalar))
        })
    });
}

fn bench_quaternion_lerp_modes(c: &mut Criterion) {
    let quats = make_quats();
    let mut group = c.benchmark_group("perro_structs/quaternion_lerp_modes");

    group.bench_function("slerp", |bench| {
        bench.iter(|| {
            let mut acc = Quaternion::IDENTITY;
            for &q in black_box(&quats) {
                acc = acc.slerped(q, 0.35);
            }
            black_box(acc)
        })
    });

    group.bench_function("nlerp", |bench| {
        bench.iter(|| {
            let mut acc = Quaternion::IDENTITY;
            for &q in black_box(&quats) {
                acc = acc.nlerped(q, 0.35);
            }
            black_box(acc)
        })
    });

    group.finish();
}

fn bench_transform2d_mat_roundtrip(c: &mut Criterion) {
    let transforms = make_transforms_2d();

    c.bench_function("perro_structs/transform2d_mat_roundtrip", |bench| {
        bench.iter(|| {
            let mut acc = Vector2::ZERO;
            let mut rot = 0.0f32;
            for &transform in black_box(&transforms) {
                let rebuilt = Transform2D::from_mat3(transform.to_mat3());
                acc += rebuilt.position + rebuilt.scale;
                rot += rebuilt.rotation;
            }
            black_box((acc, rot))
        })
    });
}

fn bench_transform3d_mat_roundtrip(c: &mut Criterion) {
    let transforms = make_transforms_3d();

    c.bench_function("perro_structs/transform3d_mat_roundtrip", |bench| {
        bench.iter(|| {
            let mut acc = Vector3::ZERO;
            let mut scalar = 0.0f32;
            for &transform in black_box(&transforms) {
                let rebuilt = Transform3D::from_mat4(transform.to_mat4());
                acc += rebuilt.position + rebuilt.scale;
                scalar += rebuilt.rotation.dot(transform.rotation);
            }
            black_box((acc, scalar))
        })
    });
}

fn bench_matrix4_bulk_ops(c: &mut Criterion) {
    let a = make_matrix4s();
    let b = make_matrix4s();

    c.bench_function("perro_structs/matrix4_bulk_ops", |bench| {
        bench.iter(|| {
            let mut acc = Matrix::<4, 4>::identity();
            for (&lhs, &rhs) in black_box(a.iter().zip(&b)) {
                acc = acc.add_f32(lhs).sub_f32(rhs.scale_f32(0.25));
            }
            black_box(acc)
        })
    });
}

fn bench_matrix_sizes_bulk_ops(c: &mut Criterion) {
    let m2 = make_matrix2s();
    let m3 = make_matrix3s();
    let rect = make_matrix2x3s();

    let mut group = c.benchmark_group("perro_structs/matrix_sizes_bulk_ops");

    group.bench_function("matrix2x2", |bench| {
        bench.iter(|| {
            let mut acc = Matrix::<2, 2>::identity();
            for &matrix in black_box(&m2) {
                acc = acc.add_fast(matrix).scale_fast(0.5);
            }
            black_box(acc)
        })
    });

    group.bench_function("matrix3x3", |bench| {
        bench.iter(|| {
            let mut acc = Matrix::<3, 3>::identity();
            for &matrix in black_box(&m3) {
                acc = acc.add_fast(matrix).sub_fast(matrix.scale_fast(0.25));
            }
            black_box(acc)
        })
    });

    group.bench_function("matrix2x3", |bench| {
        bench.iter(|| {
            let mut acc = Matrix::<2, 3>::default();
            for &matrix in black_box(&rect) {
                acc = acc.add_fast(matrix).scale_fast(0.5);
            }
            black_box(acc)
        })
    });

    group.finish();
}

fn bench_matrix4_pack(c: &mut Criterion) {
    let matrices = make_matrix4s();
    let mut packed = vec![0.0f32; matrices.len() * Matrix::<4, 4>::packed_len()];

    c.bench_function("perro_structs/matrix4_pack_row_major", |bench| {
        bench.iter(|| {
            let mut offset = 0;
            for &matrix in black_box(&matrices) {
                offset += matrix.write_packed(&mut packed[offset..]).unwrap();
            }
            black_box((offset, packed.as_ptr()))
        })
    });
}

fn bench_matrix_sizes_pack(c: &mut Criterion) {
    let m2 = make_matrix2s();
    let m3 = make_matrix3s();
    let rect = make_matrix2x3s();
    let mut packed2 = vec![0.0f32; m2.len() * Matrix::<2, 2>::packed_len()];
    let mut packed3 = vec![0.0f32; m3.len() * Matrix::<3, 3>::packed_len()];
    let mut packed_rect = vec![0.0f32; rect.len() * Matrix::<2, 3>::packed_len()];

    let mut group = c.benchmark_group("perro_structs/matrix_sizes_pack");

    group.bench_function("matrix2x2", |bench| {
        bench.iter(|| {
            let mut offset = 0;
            for &matrix in black_box(&m2) {
                offset += matrix.write_packed(&mut packed2[offset..]).unwrap();
            }
            black_box((offset, packed2.as_ptr()))
        })
    });

    group.bench_function("matrix3x3", |bench| {
        bench.iter(|| {
            let mut offset = 0;
            for &matrix in black_box(&m3) {
                offset += matrix.write_packed(&mut packed3[offset..]).unwrap();
            }
            black_box((offset, packed3.as_ptr()))
        })
    });

    group.bench_function("matrix2x3", |bench| {
        bench.iter(|| {
            let mut offset = 0;
            for &matrix in black_box(&rect) {
                offset += matrix.write_packed(&mut packed_rect[offset..]).unwrap();
            }
            black_box((offset, packed_rect.as_ptr()))
        })
    });

    group.finish();
}

fn bench_matrix_huge_ops(c: &mut Criterion) {
    let square = make_matrices::<20, 20>(HUGE_N);
    let rect_wide = make_matrices::<25, 15>(HUGE_N);
    let rect_tall = make_matrices::<15, 25>(HUGE_N);

    let mut group = c.benchmark_group("perro_structs/matrix_huge_ops");

    group.bench_function("matrix20x20_add_scale", |bench| {
        bench.iter(|| {
            let mut acc = Matrix::<20, 20>::default();
            for &matrix in black_box(&square) {
                acc = acc.add_fast(matrix).scale_fast(0.99);
            }
            black_box(acc)
        })
    });

    group.bench_function("matrix25x15_add_scale", |bench| {
        bench.iter(|| {
            let mut acc = Matrix::<25, 15>::default();
            for &matrix in black_box(&rect_wide) {
                acc = acc.add_fast(matrix).scale_fast(0.99);
            }
            black_box(acc)
        })
    });

    group.bench_function("matrix15x25_add_scale", |bench| {
        bench.iter(|| {
            let mut acc = Matrix::<15, 25>::default();
            for &matrix in black_box(&rect_tall) {
                acc = acc.add_fast(matrix).scale_fast(0.99);
            }
            black_box(acc)
        })
    });

    group.bench_function("matrix20x20_mul", |bench| {
        bench.iter(|| {
            let mut acc = Matrix::<20, 20>::identity();
            for &matrix in black_box(&square) {
                acc = acc.mul_f32(matrix);
            }
            black_box(acc)
        })
    });

    group.bench_function("matrix25x15_mul_15x25", |bench| {
        bench.iter(|| {
            let mut acc = Matrix::<25, 25>::default();
            for (&lhs, &rhs) in black_box(rect_wide.iter().zip(&rect_tall)) {
                acc = lhs.mul_f32(rhs);
            }
            black_box(acc)
        })
    });

    group.bench_function("matrix15x25_mul_25x15", |bench| {
        bench.iter(|| {
            let mut acc = Matrix::<15, 15>::default();
            for (&lhs, &rhs) in black_box(rect_tall.iter().zip(&rect_wide)) {
                acc = lhs.mul_f32(rhs);
            }
            black_box(acc)
        })
    });

    group.finish();
}

fn bench_matrix_huge_pack(c: &mut Criterion) {
    let square = make_matrices::<20, 20>(HUGE_N);
    let rect_wide = make_matrices::<25, 15>(HUGE_N);
    let rect_tall = make_matrices::<15, 25>(HUGE_N);
    let mut packed_square = vec![0.0f32; square.len() * Matrix::<20, 20>::packed_len()];
    let mut packed_wide = vec![0.0f32; rect_wide.len() * Matrix::<25, 15>::packed_len()];
    let mut packed_tall = vec![0.0f32; rect_tall.len() * Matrix::<15, 25>::packed_len()];

    let mut group = c.benchmark_group("perro_structs/matrix_huge_pack");

    group.bench_function("matrix20x20", |bench| {
        bench.iter(|| {
            let mut offset = 0;
            for &matrix in black_box(&square) {
                offset += matrix.write_packed(&mut packed_square[offset..]).unwrap();
            }
            black_box((offset, packed_square.as_ptr()))
        })
    });

    group.bench_function("matrix25x15", |bench| {
        bench.iter(|| {
            let mut offset = 0;
            for &matrix in black_box(&rect_wide) {
                offset += matrix.write_packed(&mut packed_wide[offset..]).unwrap();
            }
            black_box((offset, packed_wide.as_ptr()))
        })
    });

    group.bench_function("matrix15x25", |bench| {
        bench.iter(|| {
            let mut offset = 0;
            for &matrix in black_box(&rect_tall) {
                offset += matrix.write_packed(&mut packed_tall[offset..]).unwrap();
            }
            black_box((offset, packed_tall.as_ptr()))
        })
    });

    group.finish();
}

fn bench_matrix_huge_parallel_ops(c: &mut Criterion) {
    let square = make_matrices::<20, 20>(HUGE_N);
    let rect_wide = make_matrices::<25, 15>(HUGE_N);
    let rect_tall = make_matrices::<15, 25>(HUGE_N);

    let mut group = c.benchmark_group("perro_structs/matrix_huge_parallel_ops");

    group.bench_function("matrix20x20_mul_batch", |bench| {
        bench.iter(|| {
            let out: Vec<_> = black_box(&square)
                .par_iter()
                .map(|&matrix| Matrix::<20, 20>::identity().mul_f32(matrix))
                .collect();
            black_box(out)
        })
    });

    group.bench_function("matrix25x15_mul_15x25_batch", |bench| {
        bench.iter(|| {
            let out: Vec<_> = black_box(&rect_wide)
                .par_iter()
                .zip(black_box(&rect_tall))
                .map(|(&lhs, &rhs)| lhs.mul_f32(rhs))
                .collect();
            black_box(out)
        })
    });

    group.bench_function("matrix15x25_mul_25x15_batch", |bench| {
        bench.iter(|| {
            let out: Vec<_> = black_box(&rect_tall)
                .par_iter()
                .zip(black_box(&rect_wide))
                .map(|(&lhs, &rhs)| lhs.mul_f32(rhs))
                .collect();
            black_box(out)
        })
    });

    group.finish();
}

fn bench_matrix_huge_parallel_pack(c: &mut Criterion) {
    let square = make_matrices::<20, 20>(HUGE_N);
    let rect_wide = make_matrices::<25, 15>(HUGE_N);
    let rect_tall = make_matrices::<15, 25>(HUGE_N);
    let mut packed_square = vec![0.0f32; square.len() * Matrix::<20, 20>::packed_len()];
    let mut packed_wide = vec![0.0f32; rect_wide.len() * Matrix::<25, 15>::packed_len()];
    let mut packed_tall = vec![0.0f32; rect_tall.len() * Matrix::<15, 25>::packed_len()];

    let mut group = c.benchmark_group("perro_structs/matrix_huge_parallel_pack");

    group.bench_function("matrix20x20_batch", |bench| {
        bench.iter(|| {
            black_box(&mut packed_square)
                .par_chunks_mut(Matrix::<20, 20>::packed_len())
                .zip(black_box(&square))
                .for_each(|(chunk, &matrix)| {
                    matrix.write_packed(chunk).unwrap();
                });
            black_box(packed_square.as_ptr())
        })
    });

    group.bench_function("matrix25x15_batch", |bench| {
        bench.iter(|| {
            black_box(&mut packed_wide)
                .par_chunks_mut(Matrix::<25, 15>::packed_len())
                .zip(black_box(&rect_wide))
                .for_each(|(chunk, &matrix)| {
                    matrix.write_packed(chunk).unwrap();
                });
            black_box(packed_wide.as_ptr())
        })
    });

    group.bench_function("matrix15x25_batch", |bench| {
        bench.iter(|| {
            black_box(&mut packed_tall)
                .par_chunks_mut(Matrix::<15, 25>::packed_len())
                .zip(black_box(&rect_tall))
                .for_each(|(chunk, &matrix)| {
                    matrix.write_packed(chunk).unwrap();
                });
            black_box(packed_tall.as_ptr())
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_vector2_bulk_ops,
    bench_vector3_bulk_ops,
    bench_quaternion_bulk_ops,
    bench_quaternion_lerp_modes,
    bench_transform2d_mat_roundtrip,
    bench_transform3d_mat_roundtrip,
    bench_matrix4_bulk_ops,
    bench_matrix_sizes_bulk_ops,
    bench_matrix4_pack,
    bench_matrix_sizes_pack,
    bench_matrix_huge_ops,
    bench_matrix_huge_pack,
    bench_matrix_huge_parallel_ops,
    bench_matrix_huge_parallel_pack
);
criterion_main!(benches);
