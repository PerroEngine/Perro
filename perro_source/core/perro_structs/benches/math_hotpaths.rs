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

fn matrix_api_for_each_sum<const ROWS: usize, const COLS: usize>(
    matrix: &Matrix<ROWS, COLS>,
) -> f32 {
    let mut sum = 0.0;
    matrix.for_each(|row, col, value| {
        sum += *value + row as f32 * 0.001 + col as f32 * 0.002;
    });
    sum
}

fn matrix_unchecked_sum<const ROWS: usize, const COLS: usize>(matrix: &Matrix<ROWS, COLS>) -> f32 {
    let mut sum = 0.0;
    for row in 0..ROWS {
        for col in 0..COLS {
            // SAFETY: row/col come from matrix bounds.
            let value = unsafe { *matrix.get_unchecked(row, col) };
            sum += value + row as f32 * 0.001 + col as f32 * 0.002;
        }
    }
    sum
}

fn matrix_slice_sum<const ROWS: usize, const COLS: usize>(matrix: &Matrix<ROWS, COLS>) -> f32 {
    matrix
        .as_slice()
        .iter()
        .enumerate()
        .map(|(index, value)| *value + index as f32 * 0.0001)
        .sum()
}

fn matrix_api_zip_update<const ROWS: usize, const COLS: usize>(
    out: &mut Matrix<ROWS, COLS>,
    rhs: &Matrix<ROWS, COLS>,
) {
    out.zip_each_mut(rhs, |row, col, left, right| {
        *left = *left * 0.5 + *right + row as f32 * 0.001 + col as f32 * 0.002;
    });
}

fn matrix_unchecked_zip_update<const ROWS: usize, const COLS: usize>(
    out: &mut Matrix<ROWS, COLS>,
    rhs: &Matrix<ROWS, COLS>,
) {
    for row in 0..ROWS {
        for col in 0..COLS {
            // SAFETY: row/col come from matrix bounds.
            unsafe {
                let left = *out.get_unchecked(row, col);
                let right = *rhs.get_unchecked(row, col);
                *out.get_unchecked_mut(row, col) =
                    left * 0.5 + right + row as f32 * 0.001 + col as f32 * 0.002;
            }
        }
    }
}

fn matrix_api_map<const ROWS: usize, const COLS: usize>(
    matrix: Matrix<ROWS, COLS>,
) -> Matrix<ROWS, COLS> {
    matrix.map_cells(|row, col, value| value + row as f32 * 0.001 + col as f32 * 0.002)
}

fn matrix_unchecked_map<const ROWS: usize, const COLS: usize>(
    matrix: Matrix<ROWS, COLS>,
) -> Matrix<ROWS, COLS> {
    Matrix::<ROWS, COLS>::new(std::array::from_fn(|row| {
        std::array::from_fn(|col| {
            // SAFETY: array::from_fn indexes are in matrix bounds.
            unsafe { *matrix.get_unchecked(row, col) + row as f32 * 0.001 + col as f32 * 0.002 }
        })
    }))
}

fn matrix_api_neighbor_sum<const ROWS: usize, const COLS: usize>(
    matrix: &Matrix<ROWS, COLS>,
) -> f32 {
    let mut sum = 0.0;
    matrix.for_positions(|row, col| {
        matrix.for_neighbors_8(row, col, |next_row, next_col, value| {
            sum += *value + next_row as f32 * 0.001 + next_col as f32 * 0.002;
        });
    });
    sum
}

fn matrix_unchecked_neighbor_sum<const ROWS: usize, const COLS: usize>(
    matrix: &Matrix<ROWS, COLS>,
) -> f32 {
    let mut sum = 0.0;
    for row in 0..ROWS {
        for col in 0..COLS {
            let row_start = row.saturating_sub(1);
            let row_end = (row + 1).min(ROWS - 1);
            let col_start = col.saturating_sub(1);
            let col_end = (col + 1).min(COLS - 1);
            for next_row in row_start..=row_end {
                for next_col in col_start..=col_end {
                    if next_row != row || next_col != col {
                        // SAFETY: ranges clamp to matrix bounds.
                        let value = unsafe { *matrix.get_unchecked(next_row, next_col) };
                        sum += value + next_row as f32 * 0.001 + next_col as f32 * 0.002;
                    }
                }
            }
        }
    }
    sum
}

fn matrix_api_neighbor_count<const ROWS: usize, const COLS: usize>(
    matrix: &Matrix<ROWS, COLS>,
) -> usize {
    let mut count = 0;
    matrix.for_positions(|row, col| {
        count += matrix.count_neighbors_4(row, col, |_, _, value| *value > 2.0);
    });
    count
}

fn matrix_api_neighbor_count_8<const ROWS: usize, const COLS: usize>(
    matrix: &Matrix<ROWS, COLS>,
) -> usize {
    let mut count = 0;
    matrix.for_positions(|row, col| {
        count += matrix.count_neighbors_8(row, col, |_, _, value| *value > 2.0);
    });
    count
}

fn matrix_unchecked_neighbor_count<const ROWS: usize, const COLS: usize>(
    matrix: &Matrix<ROWS, COLS>,
) -> usize {
    let mut count = 0;
    for row in 0..ROWS {
        for col in 0..COLS {
            for (next_row, next_col) in [
                (row.wrapping_sub(1), col),
                (row, col.wrapping_sub(1)),
                (row, col + 1),
                (row + 1, col),
            ] {
                if Matrix::<ROWS, COLS>::in_bounds(next_row, next_col) {
                    // SAFETY: in_bounds checked above.
                    if unsafe { *matrix.get_unchecked(next_row, next_col) } > 2.0 {
                        count += 1;
                    }
                }
            }
        }
    }
    count
}

fn matrix_unchecked_neighbor_count_8<const ROWS: usize, const COLS: usize>(
    matrix: &Matrix<ROWS, COLS>,
) -> usize {
    let mut count = 0;
    for row in 0..ROWS {
        for col in 0..COLS {
            let row_start = row.saturating_sub(1);
            let row_end = (row + 1).min(ROWS - 1);
            let col_start = col.saturating_sub(1);
            let col_end = (col + 1).min(COLS - 1);
            for next_row in row_start..=row_end {
                for next_col in col_start..=col_end {
                    if next_row != row || next_col != col {
                        // SAFETY: ranges clamp to matrix bounds.
                        if unsafe { *matrix.get_unchecked(next_row, next_col) } > 2.0 {
                            count += 1;
                        }
                    }
                }
            }
        }
    }
    count
}

fn bench_matrix_api_vs_unchecked_helpers(c: &mut Criterion) {
    let matrices = make_matrices::<20, 20>(HUGE_N);
    let rhs = make_matrices::<20, 20>(HUGE_N);
    let mut group = c.benchmark_group("perro_structs/matrix_api_vs_unchecked_helpers");

    group.bench_function("for_each_api", |bench| {
        bench.iter(|| {
            let mut sum = 0.0;
            for matrix in black_box(&matrices) {
                sum += matrix_api_for_each_sum(matrix);
            }
            black_box(sum)
        })
    });

    group.bench_function("unchecked_get", |bench| {
        bench.iter(|| {
            let mut sum = 0.0;
            for matrix in black_box(&matrices) {
                sum += matrix_unchecked_sum(matrix);
            }
            black_box(sum)
        })
    });

    group.bench_function("as_slice_iter", |bench| {
        bench.iter(|| {
            let mut sum = 0.0;
            for matrix in black_box(&matrices) {
                sum += matrix_slice_sum(matrix);
            }
            black_box(sum)
        })
    });

    group.bench_function("zip_each_mut_api", |bench| {
        bench.iter(|| {
            let mut acc = Matrix::<20, 20>::default();
            for (&lhs, rhs) in black_box(matrices.iter().zip(&rhs)) {
                let mut out = lhs;
                matrix_api_zip_update(&mut out, rhs);
                acc += out;
            }
            black_box(acc)
        })
    });

    group.bench_function("zip_each_mut_unchecked", |bench| {
        bench.iter(|| {
            let mut acc = Matrix::<20, 20>::default();
            for (&lhs, rhs) in black_box(matrices.iter().zip(&rhs)) {
                let mut out = lhs;
                matrix_unchecked_zip_update(&mut out, rhs);
                acc += out;
            }
            black_box(acc)
        })
    });

    group.bench_function("map_cells_api", |bench| {
        bench.iter(|| {
            let mut acc = Matrix::<20, 20>::default();
            for &matrix in black_box(&matrices) {
                acc += matrix_api_map(matrix);
            }
            black_box(acc)
        })
    });

    group.bench_function("map_cells_unchecked", |bench| {
        bench.iter(|| {
            let mut acc = Matrix::<20, 20>::default();
            for &matrix in black_box(&matrices) {
                acc += matrix_unchecked_map(matrix);
            }
            black_box(acc)
        })
    });

    group.bench_function("copy_from_api", |bench| {
        bench.iter(|| {
            let mut out = Matrix::<20, 20>::default();
            for matrix in black_box(&matrices) {
                out.copy_from(matrix);
            }
            black_box(out)
        })
    });

    group.bench_function("copy_from_slice_raw", |bench| {
        bench.iter(|| {
            let mut out = Matrix::<20, 20>::default();
            for matrix in black_box(&matrices) {
                out.as_mut_slice().copy_from_slice(matrix.as_slice());
            }
            black_box(out)
        })
    });

    group.bench_function("neighbors_8_api", |bench| {
        bench.iter(|| {
            let mut sum = 0.0;
            for matrix in black_box(&matrices) {
                sum += matrix_api_neighbor_sum(matrix);
            }
            black_box(sum)
        })
    });

    group.bench_function("neighbors_8_unchecked", |bench| {
        bench.iter(|| {
            let mut sum = 0.0;
            for matrix in black_box(&matrices) {
                sum += matrix_unchecked_neighbor_sum(matrix);
            }
            black_box(sum)
        })
    });

    group.bench_function("count_neighbors_4_api", |bench| {
        bench.iter(|| {
            let mut count = 0;
            for matrix in black_box(&matrices) {
                count += matrix_api_neighbor_count(matrix);
            }
            black_box(count)
        })
    });

    group.bench_function("count_neighbors_4_unchecked", |bench| {
        bench.iter(|| {
            let mut count = 0;
            for matrix in black_box(&matrices) {
                count += matrix_unchecked_neighbor_count(matrix);
            }
            black_box(count)
        })
    });

    group.bench_function("count_neighbors_8_api", |bench| {
        bench.iter(|| {
            let mut count = 0;
            for matrix in black_box(&matrices) {
                count += matrix_api_neighbor_count_8(matrix);
            }
            black_box(count)
        })
    });

    group.bench_function("count_neighbors_8_unchecked", |bench| {
        bench.iter(|| {
            let mut count = 0;
            for matrix in black_box(&matrices) {
                count += matrix_unchecked_neighbor_count_8(matrix);
            }
            black_box(count)
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
    bench_matrix_huge_parallel_pack,
    bench_matrix_api_vs_unchecked_helpers
);
criterion_main!(benches);
