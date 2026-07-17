use super::*;

#[test]
fn generic_matrix_mul_handles_rectangular_dims() {
    let a = Matrix::<2, 3>::new([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]);
    let b = Matrix::<3, 2>::new([[7.0, 8.0], [9.0, 10.0], [11.0, 12.0]]);

    assert_eq!(a * b, Matrix::<2, 2>::new([[58.0, 64.0], [139.0, 154.0]]));
}

#[test]
fn generic_integer_matrix_uses_named_mul() {
    let a = Matrix::<2, 3, i32>::new([[1, 2, 3], [4, 5, 6]]);
    let b = Matrix::<3, 2, i32>::new([[7, 8], [9, 10], [11, 12]]);

    assert_eq!(
        a.mul_generic(b),
        Matrix::<2, 2, i32>::new([[58, 64], [139, 154]])
    );
}

#[test]
fn generic_matrix_slice_and_transpose() {
    let matrix = Matrix::<2, 3, i32>::new([[1, 2, 3], [4, 5, 6]]);

    assert_eq!(matrix.as_slice(), &[1, 2, 3, 4, 5, 6]);
    assert_eq!(
        Matrix::<2, 3, i32>::from_slice(matrix.as_slice()),
        Some(matrix)
    );
    assert_eq!(matrix.col(1), [2, 5]);
    assert_eq!(
        matrix.transposed(),
        Matrix::<3, 2, i32>::new([[1, 4], [2, 5], [3, 6]])
    );
}

#[test]
fn matrix_position_helpers_handle_row_col_and_flat_lookup() {
    let mut matrix = Matrix::<2, 3, i32>::new([[1, 2, 3], [4, 5, 6]]);

    assert_eq!(Matrix::<2, 3, i32>::flat_len(), 6);
    assert_eq!(Matrix::<2, 3, i32>::rows_len(), 2);
    assert_eq!(Matrix::<2, 3, i32>::cols_len(), 3);
    assert_eq!(Matrix::<2, 3, i32>::shape(), (2, 3));
    assert_eq!(Matrix::<2, 3, i32>::cell_count(), 6);
    assert!(!Matrix::<2, 3, i32>::is_square());
    assert!(Matrix::<3, 3, i32>::is_square());
    assert_eq!(Matrix::<2, 3, i32>::flat_index(1, 2), Some(5));
    assert_eq!(Matrix::<2, 3, i32>::flat_index(2, 0), None);
    assert_eq!(Matrix::<2, 3, i32>::row_col(4), Some((1, 1)));
    assert_eq!(Matrix::<2, 3, i32>::row_col(6), None);
    assert_eq!(matrix.get(1, 2), Some(&6));
    assert_eq!(matrix.get_flat(3), Some(&4));
    // SAFETY: indexes checked above and inside matrix bounds.
    unsafe {
        assert_eq!(*matrix.get_unchecked(1, 2), 6);
        assert_eq!(*matrix.get_flat_unchecked(3), 4);
    }
    assert_eq!(matrix.find_position(&5), Some((1, 1)));
    assert_eq!(matrix.find_flat_index(&5), Some(4));

    assert!(matrix.set(0, 1, 20));
    assert!(!matrix.set(9, 9, 0));
    assert_eq!(matrix[(0, 1)], 20);

    *matrix.get_flat_mut(5).unwrap() = 60;
    assert_eq!(matrix[(1, 2)], 60);

    // SAFETY: indexes are inside matrix bounds.
    unsafe {
        *matrix.get_unchecked_mut(0, 0) = 10;
        *matrix.get_flat_unchecked_mut(4) = 50;
    }
    assert_eq!(matrix[(0, 0)], 10);
    assert_eq!(matrix[(1, 1)], 50);
}

#[test]
fn matrix_cell_walk_helpers_use_row_major_order() {
    let mut matrix = Matrix::<2, 3, i32>::new([[1, 2, 3], [4, 5, 6]]);
    let mut seen = Vec::new();
    let mut positions = Vec::new();

    matrix.for_positions(|row, col| positions.push((row, col)));
    matrix.for_each(|row, col, value| seen.push((row, col, *value)));
    matrix.for_each_mut(|row, col, value| *value += (row * 10 + col) as i32);

    assert_eq!(
        positions,
        vec![(0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (1, 2)]
    );
    assert_eq!(
        seen,
        vec![
            (0, 0, 1),
            (0, 1, 2),
            (0, 2, 3),
            (1, 0, 4),
            (1, 1, 5),
            (1, 2, 6)
        ]
    );
    assert_eq!(matrix, Matrix::<2, 3, i32>::new([[1, 3, 5], [14, 16, 18]]));
}

#[test]
fn matrix_zip_helpers_pair_cells_by_position() {
    let mut matrix = Matrix::<2, 3, i32>::new([[1, 2, 3], [4, 5, 6]]);
    let rhs = Matrix::<2, 3, i32>::new([[10, 20, 30], [40, 50, 60]]);
    let mut seen = Vec::new();

    matrix.zip_each(&rhs, |row, col, left, right| {
        seen.push((row, col, *left, *right));
    });
    matrix.zip_each_mut(&rhs, |row, col, left, right| {
        *left += *right + (row + col) as i32;
    });

    assert_eq!(
        seen,
        vec![
            (0, 0, 1, 10),
            (0, 1, 2, 20),
            (0, 2, 3, 30),
            (1, 0, 4, 40),
            (1, 1, 5, 50),
            (1, 2, 6, 60)
        ]
    );
    assert_eq!(
        matrix,
        Matrix::<2, 3, i32>::new([[11, 23, 35], [45, 57, 69]])
    );
}

#[test]
fn matrix_neighbor_helpers_walk_bounds_checked_cells() {
    let matrix = Matrix::<3, 3, i32>::new([[1, 2, 3], [4, 5, 6], [7, 8, 9]]);
    let mut four = Vec::new();
    let mut eight = Vec::new();
    let mut corner = Vec::new();

    assert!(matrix.for_neighbors_4(1, 1, |row, col, value| {
        four.push((row, col, *value));
    }));
    assert!(matrix.for_neighbors_8(1, 1, |row, col, value| {
        eight.push((row, col, *value));
    }));
    assert!(matrix.for_neighbors_8(0, 0, |row, col, value| {
        corner.push((row, col, *value));
    }));

    assert_eq!(four, vec![(0, 1, 2), (1, 0, 4), (1, 2, 6), (2, 1, 8)]);
    assert_eq!(
        eight,
        vec![
            (0, 0, 1),
            (0, 1, 2),
            (0, 2, 3),
            (1, 0, 4),
            (1, 2, 6),
            (2, 0, 7),
            (2, 1, 8),
            (2, 2, 9)
        ]
    );
    assert_eq!(corner, vec![(0, 1, 2), (1, 0, 4), (1, 1, 5)]);
    assert_eq!(
        matrix.count_neighbors_4(1, 1, |_, _, value| *value % 2 == 0),
        4
    );
    assert_eq!(matrix.count_neighbors_8(0, 0, |_, _, value| *value > 3), 2);
    assert!(!matrix.for_neighbors_4(9, 9, |_, _, _| unreachable!()));
    assert_eq!(matrix.count_neighbors_8(9, 9, |_, _, _| true), 0);
}

#[test]
fn matrix_api_cell_helpers_match_unchecked_reference_loops() {
    let matrix = Matrix::<3, 4, i32>::new([[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12]]);
    let rhs = Matrix::<3, 4, i32>::new([[2, 4, 6, 8], [10, 12, 14, 16], [18, 20, 22, 24]]);

    let mut api_positions = Vec::new();
    matrix.for_positions(|row, col| api_positions.push((row, col)));
    let mut unchecked_positions = Vec::new();
    for row in 0..Matrix::<3, 4, i32>::rows_len() {
        for col in 0..Matrix::<3, 4, i32>::cols_len() {
            unchecked_positions.push((row, col));
        }
    }
    assert_eq!(api_positions, unchecked_positions);

    let mut api_cells = Vec::new();
    matrix.for_each(|row, col, value| api_cells.push((row, col, *value)));
    let mut unchecked_cells = Vec::new();
    for (row, col) in unchecked_positions.iter().copied() {
        // SAFETY: row/col come from in-bounds loop ranges above.
        unchecked_cells.push((row, col, unsafe { *matrix.get_unchecked(row, col) }));
    }
    assert_eq!(api_cells, unchecked_cells);

    let mut api_zip = Vec::new();
    matrix.zip_each(&rhs, |row, col, left, right| {
        api_zip.push((row, col, *left, *right));
    });
    let mut unchecked_zip = Vec::new();
    for (row, col) in unchecked_positions.iter().copied() {
        // SAFETY: row/col come from in-bounds loop ranges above.
        unchecked_zip.push((
            row,
            col,
            unsafe { *matrix.get_unchecked(row, col) },
            unsafe { *rhs.get_unchecked(row, col) },
        ));
    }
    assert_eq!(api_zip, unchecked_zip);

    let api_map = matrix.map_cells(|row, col, value| value + (row * 10 + col) as i32);
    let unchecked_map = Matrix::<3, 4, i32>::new(std::array::from_fn(|row| {
        std::array::from_fn(|col| {
            // SAFETY: array::from_fn indexes are in matrix bounds.
            unsafe { *matrix.get_unchecked(row, col) + (row * 10 + col) as i32 }
        })
    }));
    assert_eq!(api_map, unchecked_map);

    let mut api_mut = matrix;
    api_mut.for_each_mut(|row, col, value| *value += (row * 3 + col) as i32);
    let mut unchecked_mut = matrix;
    for row in 0..Matrix::<3, 4, i32>::rows_len() {
        for col in 0..Matrix::<3, 4, i32>::cols_len() {
            // SAFETY: row/col come from in-bounds loop ranges above.
            unsafe {
                *unchecked_mut.get_unchecked_mut(row, col) += (row * 3 + col) as i32;
            }
        }
    }
    assert_eq!(api_mut, unchecked_mut);

    let mut api_zip_mut = matrix;
    api_zip_mut.zip_each_mut(&rhs, |row, col, left, right| {
        *left = *left * 2 + *right + (row + col) as i32;
    });
    let mut unchecked_zip_mut = matrix;
    for row in 0..Matrix::<3, 4, i32>::rows_len() {
        for col in 0..Matrix::<3, 4, i32>::cols_len() {
            // SAFETY: row/col come from in-bounds loop ranges above.
            unsafe {
                let current = *unchecked_zip_mut.get_unchecked(row, col);
                *unchecked_zip_mut.get_unchecked_mut(row, col) =
                    current * 2 + *rhs.get_unchecked(row, col) + (row + col) as i32;
            }
        }
    }
    assert_eq!(api_zip_mut, unchecked_zip_mut);

    let mut api_copy = Matrix::<3, 4, i32>::default();
    api_copy.copy_from(&matrix);
    let mut unchecked_copy = Matrix::<3, 4, i32>::default();
    unchecked_copy
        .as_mut_slice()
        .copy_from_slice(matrix.as_slice());
    assert_eq!(api_copy, unchecked_copy);

    let mut slice_matrix = matrix;
    slice_matrix.as_mut_slice()[5] = 99;
    // SAFETY: flat index 5 is inside 3x4 matrix bounds.
    unsafe {
        assert_eq!(*slice_matrix.get_flat_unchecked(5), 99);
    }
}

#[test]
fn matrix_neighbor_helpers_match_unchecked_reference_loops() {
    let matrix = Matrix::<4, 5, i32>::new(std::array::from_fn(|row| {
        std::array::from_fn(|col| (row * 10 + col) as i32)
    }));

    for row in 0..Matrix::<4, 5, i32>::rows_len() {
        for col in 0..Matrix::<4, 5, i32>::cols_len() {
            let mut api_4 = Vec::new();
            let mut api_8 = Vec::new();
            assert!(
                matrix.for_neighbors_4(row, col, |next_row, next_col, value| {
                    api_4.push((next_row, next_col, *value));
                })
            );
            assert!(
                matrix.for_neighbors_8(row, col, |next_row, next_col, value| {
                    api_8.push((next_row, next_col, *value));
                })
            );

            let mut unchecked_4 = Vec::new();
            for (next_row, next_col) in [
                (row.wrapping_sub(1), col),
                (row, col.wrapping_sub(1)),
                (row, col + 1),
                (row + 1, col),
            ] {
                if Matrix::<4, 5, i32>::in_bounds(next_row, next_col) {
                    // SAFETY: in_bounds checked above.
                    unchecked_4.push((next_row, next_col, unsafe {
                        *matrix.get_unchecked(next_row, next_col)
                    }));
                }
            }

            let mut unchecked_8 = Vec::new();
            let row_start = row.saturating_sub(1);
            let row_end = (row + 1).min(Matrix::<4, 5, i32>::rows_len() - 1);
            let col_start = col.saturating_sub(1);
            let col_end = (col + 1).min(Matrix::<4, 5, i32>::cols_len() - 1);
            for next_row in row_start..=row_end {
                for next_col in col_start..=col_end {
                    if next_row != row || next_col != col {
                        // SAFETY: ranges clamp to matrix bounds.
                        unchecked_8.push((next_row, next_col, unsafe {
                            *matrix.get_unchecked(next_row, next_col)
                        }));
                    }
                }
            }

            assert_eq!(api_4, unchecked_4);
            assert_eq!(api_8, unchecked_8);
            assert_eq!(
                matrix.count_neighbors_4(row, col, |_, _, value| *value % 2 == 0),
                unchecked_4
                    .iter()
                    .filter(|(_, _, value)| value % 2 == 0)
                    .count()
            );
            assert_eq!(
                matrix.count_neighbors_8(row, col, |_, _, value| *value > 11),
                unchecked_8
                    .iter()
                    .filter(|(_, _, value)| *value > 11)
                    .count()
            );
        }
    }
}

#[test]
fn matrix_row_col_iter_helpers_cover_checked_access() {
    let mut matrix = Matrix::<2, 3, i32>::new([[1, 2, 3], [4, 5, 6]]);

    assert_eq!(
        matrix.rows_iter().copied().collect::<Vec<_>>(),
        vec![[1, 2, 3], [4, 5, 6]]
    );
    assert_eq!(
        matrix.row_iter(1).unwrap().copied().collect::<Vec<_>>(),
        vec![4, 5, 6]
    );
    assert!(matrix.row_iter(2).is_none());
    assert_eq!(
        matrix.col_iter(1).unwrap().copied().collect::<Vec<_>>(),
        vec![2, 5]
    );
    assert!(matrix.col_iter(3).is_none());

    matrix
        .row_iter_mut(0)
        .unwrap()
        .for_each(|value| *value += 10);
    matrix
        .col_iter_mut(2)
        .unwrap()
        .for_each(|value| *value *= 2);
    matrix.rows_iter_mut().for_each(|row| row[0] *= -1);

    assert_eq!(
        matrix,
        Matrix::<2, 3, i32>::new([[-11, 12, 26], [-4, 5, 12]])
    );
}

#[test]
fn matrix_map_and_resize_helpers_preserve_expected_cells() {
    let matrix = Matrix::<2, 3, i32>::new([[1, 2, 3], [4, 5, 6]]);

    assert_eq!(
        matrix.map_cells(|row, col, value| value + (row + col) as i32),
        Matrix::<2, 3, i32>::new([[1, 3, 5], [5, 7, 9]])
    );
    assert_eq!(
        matrix.resize::<3, 2>(0),
        Matrix::<3, 2, i32>::new([[1, 2], [4, 5], [0, 0]])
    );
    assert_eq!(
        matrix.resize_with::<3, 4>(|row, col| (row * 10 + col) as i32),
        Matrix::<3, 4, i32>::new([[1, 2, 3, 3], [4, 5, 6, 13], [20, 21, 22, 23]])
    );
    assert_eq!(
        matrix.resize_default::<1, 4>(),
        Matrix::<1, 4, i32>::new([[1, 2, 3, 0]])
    );
}

#[test]
fn matrix_iter_query_and_fill_helpers_stay_flat_and_no_alloc() {
    let mut matrix = Matrix::<2, 3, i32>::new([[1, 2, 3], [4, 5, 6]]);

    assert_eq!(matrix.iter().copied().sum::<i32>(), 21);
    assert_eq!(
        matrix.cells().collect::<Vec<_>>(),
        vec![
            (0, 0, &1),
            (0, 1, &2),
            (0, 2, &3),
            (1, 0, &4),
            (1, 1, &5),
            (1, 2, &6)
        ]
    );
    assert!(matrix.any_cell(|_, _, value| *value == 4));
    assert!(matrix.all_cells(|_, _, value| *value > 0));
    assert_eq!(matrix.count_cells(|_, _, value| *value % 2 == 0), 3);
    assert_eq!(
        matrix.find_cell(|row, _, value| row == 1 && *value == 5),
        Some((1, 1))
    );

    matrix.fill(7);
    assert_eq!(matrix, Matrix::<2, 3, i32>::new([[7, 7, 7], [7, 7, 7]]));

    matrix.fill_with(|row, col| (row * 10 + col) as i32);
    assert_eq!(matrix, Matrix::<2, 3, i32>::new([[0, 1, 2], [10, 11, 12]]));

    assert!(!matrix.copy_from_slice(&[1, 2, 3]));
    assert!(matrix.copy_from_slice(&[1, 2, 3, 4, 5, 6]));
    assert_eq!(matrix.as_slice(), &[1, 2, 3, 4, 5, 6]);

    matrix.iter_mut().for_each(|value| *value *= 2);
    assert_eq!(matrix, Matrix::<2, 3, i32>::new([[2, 4, 6], [8, 10, 12]]));
}

#[test]
fn matrix_bulk_copy_and_swap_helpers_use_flat_storage() {
    let mut matrix = Matrix::<2, 3, i32>::default();
    let src = Matrix::<2, 3, i32>::new([[1, 2, 3], [4, 5, 6]]);
    let mut out = [0; 6];

    matrix.clone_from_matrix(&src);
    assert_eq!(matrix, src);
    matrix.fill(0).copy_from(&src);
    assert_eq!(matrix, src);
    assert_eq!(matrix.copy_to_slice(&mut out), Some(6));
    assert_eq!(out, [1, 2, 3, 4, 5, 6]);
    assert_eq!(matrix.copy_to_slice(&mut out[..5]), None);

    assert!(matrix.swap_cells((0, 1), (1, 2)));
    assert_eq!(matrix, Matrix::<2, 3, i32>::new([[1, 6, 3], [4, 5, 2]]));
    assert!(!matrix.swap_cells((2, 0), (0, 0)));

    assert!(matrix.swap_flat(0, 5));
    assert_eq!(matrix, Matrix::<2, 3, i32>::new([[2, 6, 3], [4, 5, 1]]));
    assert!(!matrix.swap_flat(0, 6));
}

#[test]
fn matrix_from_vec_rows_uses_row_windows() {
    let matrix = Matrix::<2, 3, i32>::new([[1, 2, 3], [4, 5, 6]]);

    assert_eq!(
        Matrix::<2, 3, i32>::from_vec_rows(vec![
            vec![1, 2, 3, 9],
            vec![4, 5, 6, 9],
            vec![7, 8, 9, 9]
        ]),
        Some(matrix)
    );
    assert_eq!(
        Matrix::<2, 3, i32>::from_vec_rows_offset(
            vec![vec![0, 0, 0, 0], vec![0, 1, 2, 3], vec![0, 4, 5, 6]],
            1,
            1
        ),
        Some(matrix)
    );
    assert_eq!(
        Matrix::<2, 3, i32>::from_vec_rows(vec![vec![1, 2, 3], vec![4, 5]]),
        None
    );
}

#[test]
fn matrix_aggregate_and_conversion_helpers_use_flat_order() {
    let matrix = Matrix::<2, 3, i32>::new([[1, 2, 3], [4, 5, 6]]);

    assert_eq!(matrix.sum(), 21);
    assert_eq!(matrix.product(), 720);
    assert_eq!(matrix.min_cell(), Some((0, 0, 1)));
    assert_eq!(matrix.max_cell(), Some((1, 2, 6)));
    assert_eq!(
        matrix.fold_cells(Vec::new(), |mut out, row, col, value| {
            out.push((row, col, *value));
            out
        }),
        vec![
            (0, 0, 1),
            (0, 1, 2),
            (0, 2, 3),
            (1, 0, 4),
            (1, 1, 5),
            (1, 2, 6)
        ]
    );
    assert_eq!(matrix.to_rows(), [[1, 2, 3], [4, 5, 6]]);
    assert_eq!(matrix.to_cols(), [[1, 4], [2, 5], [3, 6]]);
    assert_eq!(matrix.into_rows(), [[1, 2, 3], [4, 5, 6]]);
    assert_eq!(matrix.to_vec(), vec![1, 2, 3, 4, 5, 6]);
    assert_eq!(matrix.into_vec(), vec![1, 2, 3, 4, 5, 6]);
}

#[test]
fn matrix3_fast_path_matches_generic_rows() {
    let rows = [[1.0, 2.0, 3.0], [0.0, 1.0, 4.0], [5.0, 6.0, 0.0]];
    let generic = Matrix::<3, 3>::new(rows);
    let fast = Matrix3::from(generic);

    assert_eq!(fast.to_rows(), rows);
    assert_eq!(Matrix::<3, 3>::from(fast), generic);
}

#[test]
fn square_generic_matrix_exposes_glam_roundtrip() {
    let matrix = Matrix::<4, 4>::identity();
    let glam = matrix.to_glam();

    assert_eq!(glam, Mat4::IDENTITY);
    assert_eq!(Matrix::<4, 4>::from_glam(glam), matrix);
}

#[test]
fn matrix_mul_covers_square_and_rectangular_sizes() {
    assert_eq!(
        Matrix::<1, 1>::new([[3.0]]) * Matrix::<1, 1>::new([[4.0]]),
        Matrix::<1, 1>::new([[12.0]])
    );

    assert_eq!(
        Matrix::<2, 2>::new([[1.0, 2.0], [3.0, 4.0]])
            * Matrix::<2, 2>::new([[5.0, 6.0], [7.0, 8.0]]),
        Matrix::<2, 2>::new([[19.0, 22.0], [43.0, 50.0]])
    );

    assert_eq!(
        Matrix::<2, 3>::new([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]])
            * Matrix::<3, 4>::new([
                [7.0, 8.0, 9.0, 10.0],
                [11.0, 12.0, 13.0, 14.0],
                [15.0, 16.0, 17.0, 18.0],
            ]),
        Matrix::<2, 4>::new([[74.0, 80.0, 86.0, 92.0], [173.0, 188.0, 203.0, 218.0]])
    );

    assert_eq!(
        Matrix::<4, 2, i32>::new([[1, 2], [3, 4], [5, 6], [7, 8]])
            .mul_generic(Matrix::<2, 3, i32>::new([[9, 10, 11], [12, 13, 14]])),
        Matrix::<4, 3, i32>::new([[33, 36, 39], [75, 82, 89], [117, 128, 139], [159, 174, 189]])
    );
}

#[test]
fn f32_square_ops_use_fast_path_results() {
    let a = Matrix::<3, 3>::new([[1.0, 2.0, 3.0], [0.0, 1.0, 4.0], [5.0, 6.0, 0.0]]);
    let b = Matrix::<3, 3>::new([[-2.0, 1.0, 0.0], [3.0, 0.0, 1.0], [4.0, 1.0, 0.0]]);

    let expected = Matrix::<3, 3>::new([[16.0, 4.0, 2.0], [19.0, 4.0, 1.0], [8.0, 5.0, 6.0]]);
    assert_eq!(a.mul_f32(b), expected);
    assert_eq!(a * b, expected);
    assert!((a.determinant() - 1.0).abs() < 1.0e-6);
    assert_eq!(a.inverse().unwrap().mul_f32(a), Matrix::<3, 3>::identity());
}

#[test]
fn f32_matrix_simd_ops_match_scalar_results() {
    let a = Matrix::<4, 4>::new([
        [1.0, 2.0, 3.0, 4.0],
        [5.0, 6.0, 7.0, 8.0],
        [9.0, 10.0, 11.0, 12.0],
        [13.0, 14.0, 15.0, 16.0],
    ]);
    let b = Matrix::<4, 4>::new([
        [16.0, 15.0, 14.0, 13.0],
        [12.0, 11.0, 10.0, 9.0],
        [8.0, 7.0, 6.0, 5.0],
        [4.0, 3.0, 2.0, 1.0],
    ]);

    assert_eq!(a.add_f32(b), a + b);
    assert_eq!(a.sub_f32(b), a - b);
    assert_eq!(a.scale_f32(0.25), a * 0.25);
}

#[test]
fn f32_matrix_ops_use_internal_fast_paths() {
    fn value(seed: usize, index: usize) -> f32 {
        let mixed = (seed * 37 + index * 17 + (index % 5) * 11) as f32;
        (mixed % 23.0) * 0.25 - 2.75
    }

    for seed in 0..32 {
        let a = Matrix::<3, 5>::new(std::array::from_fn(|r| {
            std::array::from_fn(|c| value(seed, r * 5 + c))
        }));
        let b = Matrix::<3, 5>::new(std::array::from_fn(|r| {
            std::array::from_fn(|c| value(seed + 11, r * 5 + c))
        }));

        let mut add_assign = a;
        add_assign += b;
        assert_eq!(add_assign, a.add_f32(b));
        assert_eq!(a + b, a.add_f32(b));

        let mut sub_assign = a;
        sub_assign -= b;
        assert_eq!(sub_assign, a.sub_f32(b));
        assert_eq!(a - b, a.sub_f32(b));
        assert_eq!(a * 1.75, a.scale_f32(1.75));

        let mut mul_assign = a;
        mul_assign *= 1.75;
        assert_eq!(mul_assign, a.scale_f32(1.75));

        let mut div_assign = a;
        div_assign /= 2.0;
        assert_eq!(div_assign, a.scale_f32(0.5));
        assert_eq!(a / 2.0, a.scale_f32(0.5));
    }
}

#[test]
fn matrix_scalar_mul_div_and_shifts_work_through_ops() {
    let mut f64_matrix = Matrix::<2, 2, f64>::new([[2.0, 4.0], [6.0, 8.0]]);
    f64_matrix *= 0.5;
    assert_eq!(
        f64_matrix,
        Matrix::<2, 2, f64>::new([[1.0, 2.0], [3.0, 4.0]])
    );
    f64_matrix /= 2.0;
    assert_eq!(
        f64_matrix,
        Matrix::<2, 2, f64>::new([[0.5, 1.0], [1.5, 2.0]])
    );

    let mut i32_matrix = Matrix::<2, 3, i32>::new([[1, 2, 3], [4, 5, 6]]);
    i32_matrix *= 3;
    assert_eq!(
        i32_matrix,
        Matrix::<2, 3, i32>::new([[3, 6, 9], [12, 15, 18]])
    );
    i32_matrix /= 3;
    assert_eq!(i32_matrix, Matrix::<2, 3, i32>::new([[1, 2, 3], [4, 5, 6]]));
    i32_matrix <<= 1;
    assert_eq!(
        i32_matrix,
        Matrix::<2, 3, i32>::new([[2, 4, 6], [8, 10, 12]])
    );
    i32_matrix >>= 1;
    assert_eq!(i32_matrix, Matrix::<2, 3, i32>::new([[1, 2, 3], [4, 5, 6]]));

    let u16_matrix = Matrix::<2, 2, u16>::new([[1, 2], [3, 4]]);
    assert_eq!(u16_matrix * 4, Matrix::<2, 2, u16>::new([[4, 8], [12, 16]]));
    assert_eq!(
        u16_matrix << 2,
        Matrix::<2, 2, u16>::new([[4, 8], [12, 16]])
    );
    assert_eq!(
        Matrix::<2, 2, u16>::new([[8, 16], [24, 32]]) >> 3,
        Matrix::<2, 2, u16>::new([[1, 2], [3, 4]])
    );
}

#[test]
fn fast_matrix_wrappers_support_scalar_division() {
    let matrix = Matrix4::from_rows([
        [2.0, 0.0, 0.0, 0.0],
        [0.0, 4.0, 0.0, 0.0],
        [0.0, 0.0, 6.0, 0.0],
        [0.0, 0.0, 0.0, 8.0],
    ]);

    assert_eq!(
        (matrix / 2.0).to_rows(),
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 2.0, 0.0, 0.0],
            [0.0, 0.0, 3.0, 0.0],
            [0.0, 0.0, 0.0, 4.0],
        ]
    );
}

#[test]
fn f32_matrix_division_matches_scalar_for_subnormal_divisor() {
    let divisor = f32::from_bits(1);
    let matrix = Matrix::<1, 3, f32>::new([[divisor, 1.0, -divisor]]);
    let divided = matrix / divisor;

    assert_eq!(divided[(0, 0)], divisor / divisor);
    assert_eq!(divided[(0, 1)], 1.0 / divisor);
    assert_eq!(divided[(0, 2)], -divisor / divisor);
}

#[test]
fn f32_matrix_mul_matches_scalar_reference_for_many_shapes() {
    fn value(seed: usize, index: usize) -> f32 {
        let mixed = (seed * 13 + index * 29 + (index % 7) * 5) as f32;
        (mixed % 19.0) * 0.125 - 1.0
    }

    for seed in 0..24 {
        let a = Matrix::<4, 5>::new(std::array::from_fn(|r| {
            std::array::from_fn(|c| value(seed, r * 5 + c))
        }));
        let b = Matrix::<5, 3>::new(std::array::from_fn(|r| {
            std::array::from_fn(|c| value(seed + 17, r * 3 + c))
        }));

        assert_matrix_near(a * b, a.mul_generic(b), 1.0e-5);

        let square_a = Matrix::<4, 4>::new(std::array::from_fn(|r| {
            std::array::from_fn(|c| value(seed + 23, r * 4 + c))
        }));
        let square_b = Matrix::<4, 4>::new(std::array::from_fn(|r| {
            std::array::from_fn(|c| value(seed + 31, r * 4 + c))
        }));

        assert_matrix_near(square_a * square_b, square_a.mul_generic(square_b), 1.0e-5);
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[test]
fn forced_x86_simd_helpers_match_scalar_for_tail_lengths() {
    if std::is_x86_feature_detected!("sse") {
        for len in 0..19 {
            let lhs: Vec<f32> = (0..len).map(|i| i as f32 * 0.5 - 3.0).collect();
            let rhs: Vec<f32> = (0..len).map(|i| i as f32 * -0.25 + 2.0).collect();

            let mut simd = lhs.clone();
            let mut scalar = lhs.clone();
            assert!(x86::try_add_assign_f32(&mut simd, &rhs));
            scalar_add_assign(&mut scalar, &rhs);
            assert_eq!(simd, scalar);

            let mut simd = lhs.clone();
            let mut scalar = lhs.clone();
            assert!(x86::try_sub_assign_f32(&mut simd, &rhs));
            scalar_sub_assign(&mut scalar, &rhs);
            assert_eq!(simd, scalar);

            let mut simd = lhs.clone();
            let mut scalar = lhs.clone();
            assert!(x86::try_scale_assign_f32(&mut simd, 1.25));
            scalar_scale_assign(&mut scalar, 1.25);
            assert_eq!(simd, scalar);

            let simd_dot = x86::try_dot_f32(&lhs, &rhs).unwrap();
            let scalar_dot = scalar_dot_f32(&lhs, &rhs);
            assert!((simd_dot - scalar_dot).abs() <= 1.0e-5);
        }
    }

    if std::is_x86_feature_detected!("sse2") {
        for len in 0..19 {
            let lhs: Vec<f64> = (0..len).map(|i| i as f64 * 0.5 - 3.0).collect();
            let rhs: Vec<f64> = (0..len).map(|i| i as f64 * -0.25 + 2.0).collect();

            let mut simd = lhs.clone();
            let mut scalar = lhs.clone();
            assert!(x86::try_add_assign_f64(&mut simd, &rhs));
            scalar_add_assign_generic(&mut scalar, &rhs);
            assert_eq!(simd, scalar);

            let mut simd = lhs.clone();
            let mut scalar = lhs.clone();
            assert!(x86::try_sub_assign_f64(&mut simd, &rhs));
            scalar_sub_assign_generic(&mut scalar, &rhs);
            assert_eq!(simd, scalar);

            let mut simd = lhs.clone();
            let mut scalar = lhs.clone();
            assert!(x86::try_scale_assign_f64(&mut simd, 1.25));
            scalar_scale_assign_generic(&mut scalar, 1.25);
            assert_eq!(simd, scalar);

            let lhs_i32: Vec<i32> = (0..len).map(|i| i - 8).collect();
            let rhs_i32: Vec<i32> = (0..len).map(|i| 16 - i).collect();

            let mut simd = lhs_i32.clone();
            let mut scalar = lhs_i32.clone();
            assert!(x86::try_add_assign_i32(&mut simd, &rhs_i32));
            scalar_add_assign_generic(&mut scalar, &rhs_i32);
            assert_eq!(simd, scalar);

            let mut simd = lhs_i32.clone();
            let mut scalar = lhs_i32.clone();
            assert!(x86::try_sub_assign_i32(&mut simd, &rhs_i32));
            scalar_sub_assign_generic(&mut scalar, &rhs_i32);
            assert_eq!(simd, scalar);
        }
    }

    if std::is_x86_feature_detected!("sse4.1") {
        for len in 0..19 {
            let lhs: Vec<i32> = (0..len).map(|i| i - 8).collect();
            let mut simd = lhs.clone();
            let mut scalar = lhs.clone();
            assert!(x86::try_scale_assign_i32(&mut simd, 3));
            scalar_scale_assign_generic(&mut scalar, 3);
            assert_eq!(simd, scalar);
        }
    }
}

#[test]
fn fast_numeric_ops_cover_square_and_rectangular_types() {
    let f32_a = Matrix::<2, 3>::new([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]);
    let f32_b = Matrix::<2, 3>::new([[6.0, 5.0, 4.0], [3.0, 2.0, 1.0]]);
    assert_eq!(
        f32_a.add_fast(f32_b),
        Matrix::<2, 3>::new([[7.0, 7.0, 7.0], [7.0, 7.0, 7.0]])
    );

    let f64_a = Matrix::<3, 2, f64>::new([[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]]);
    let f64_b = Matrix::<3, 2, f64>::new([[0.5, 1.0], [1.5, 2.0], [2.5, 3.0]]);
    assert_eq!(
        f64_a.sub_fast(f64_b),
        Matrix::<3, 2, f64>::new([[0.5, 1.0], [1.5, 2.0], [2.5, 3.0]])
    );
    assert_eq!(
        f64_a.scale_fast(2.0),
        Matrix::<3, 2, f64>::new([[2.0, 4.0], [6.0, 8.0], [10.0, 12.0]])
    );

    let i32_a = Matrix::<3, 3, i32>::new([[1, 2, 3], [4, 5, 6], [7, 8, 9]]);
    let i32_b = Matrix::<3, 3, i32>::new([[9, 8, 7], [6, 5, 4], [3, 2, 1]]);
    assert_eq!(
        i32_a.add_fast(i32_b),
        Matrix::<3, 3, i32>::new([[10, 10, 10], [10, 10, 10], [10, 10, 10]])
    );
    assert_eq!(
        i32_a.scale_fast(3),
        Matrix::<3, 3, i32>::new([[3, 6, 9], [12, 15, 18], [21, 24, 27]])
    );

    let u32_a = Matrix::<2, 5, u32>::new([[1, 2, 3, 4, 5], [6, 7, 8, 9, 10]]);
    let u32_b = Matrix::<2, 5, u32>::new([[1, 1, 1, 1, 1], [2, 2, 2, 2, 2]]);
    assert_eq!(
        u32_a.sub_fast(u32_b),
        Matrix::<2, 5, u32>::new([[0, 1, 2, 3, 4], [4, 5, 6, 7, 8]])
    );
    assert_eq!(
        u32_a.scale_fast(2),
        Matrix::<2, 5, u32>::new([[2, 4, 6, 8, 10], [12, 14, 16, 18, 20]])
    );

    let i16_a = Matrix::<2, 2, i16>::new([[1, 2], [3, 4]]);
    let i16_b = Matrix::<2, 2, i16>::new([[5, 6], [7, 8]]);
    assert_eq!(
        i16_a.add_fast(i16_b),
        Matrix::<2, 2, i16>::new([[6, 8], [10, 12]])
    );
    assert_eq!(
        i16_a.scale_fast(2),
        Matrix::<2, 2, i16>::new([[2, 4], [6, 8]])
    );

    let i8_a = Matrix::<2, 3, i8>::new([[1, 2, 3], [4, 5, 6]]);
    let i8_b = Matrix::<2, 3, i8>::new([[1, 1, 1], [2, 2, 2]]);
    assert_eq!(
        i8_a.sub_fast(i8_b),
        Matrix::<2, 3, i8>::new([[0, 1, 2], [2, 3, 4]])
    );

    let u8_a = Matrix::<2, 3, u8>::new([[1, 2, 3], [4, 5, 6]]);
    let u8_b = Matrix::<2, 3, u8>::new([[1, 1, 1], [2, 2, 2]]);
    assert_eq!(
        u8_a.add_fast(u8_b),
        Matrix::<2, 3, u8>::new([[2, 3, 4], [6, 7, 8]])
    );

    let u16_a = Matrix::<2, 2, u16>::new([[1, 2], [3, 4]]);
    assert_eq!(
        u16_a.scale_fast(3),
        Matrix::<2, 2, u16>::new([[3, 6], [9, 12]])
    );

    let i64_a = Matrix::<2, 2, i64>::new([[1, 2], [3, 4]]);
    let i64_b = Matrix::<2, 2, i64>::new([[4, 3], [2, 1]]);
    assert_eq!(
        i64_a.add_fast(i64_b),
        Matrix::<2, 2, i64>::new([[5, 5], [5, 5]])
    );

    let u64_a = Matrix::<2, 2, u64>::new([[5, 6], [7, 8]]);
    let u64_b = Matrix::<2, 2, u64>::new([[1, 2], [3, 4]]);
    assert_eq!(
        u64_a.sub_fast(u64_b),
        Matrix::<2, 2, u64>::new([[4, 4], [4, 4]])
    );
}

#[test]
fn matrix_pack_uses_row_major_f32_order() {
    let matrix = Matrix::<3, 3>::new([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]);
    let mut packed = [0.0; 9];

    assert_eq!(matrix.write_packed(&mut packed), Some(9));
    assert_eq!(packed, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
    assert_eq!(Matrix::<3, 3>::read_packed(&packed), Some(matrix));
    assert_eq!(matrix.as_bytes().len(), 36);
}

#[test]
fn matrix_flat_pack_covers_non_square_generic_values() {
    let matrix = Matrix::<2, 4, u16>::new([[1, 2, 3, 4], [5, 6, 7, 8]]);
    let mut packed = [0; 8];

    assert_eq!(matrix.write_flat(&mut packed), Some(8));
    assert_eq!(packed, [1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(Matrix::<2, 4, u16>::from_slice(&packed), Some(matrix));
    assert_eq!(Matrix::<2, 4, u16>::from_vec(packed.to_vec()), Some(matrix));
    assert_eq!(
        Matrix::<2, 4, u16>::from_vec(vec![1, 2, 3, 4, 5, 6, 7, 8, 9]),
        Some(matrix)
    );
    assert_eq!(
        Matrix::<2, 4, u16>::from_vec_offset(vec![0, 1, 2, 3, 4, 5, 6, 7, 8], 1),
        Some(matrix)
    );
    assert_eq!(Matrix::<2, 4, u16>::from_vec(vec![1, 2, 3]), None);
}

#[test]
fn generic_inverse_returns_none_for_singular() {
    let matrix = Matrix::<2, 2>::new([[1.0, 2.0], [2.0, 4.0]]);

    assert_eq!(matrix.determinant(), 0.0);
    assert_eq!(matrix.inverse(), None);
}

fn assert_matrix_near<const ROWS: usize, const COLS: usize>(
    actual: Matrix<ROWS, COLS>,
    expected: Matrix<ROWS, COLS>,
    epsilon: f32,
) {
    for r in 0..ROWS {
        for c in 0..COLS {
            let delta = (actual[(r, c)] - expected[(r, c)]).abs();
            assert!(
                delta <= epsilon,
                "matrix mismatch at ({r}, {c}): actual {} expected {} delta {}",
                actual[(r, c)],
                expected[(r, c)],
                delta
            );
        }
    }
}
