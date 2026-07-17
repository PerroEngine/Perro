use glam::{Mat2, Mat3, Mat4};
use std::fmt;
use std::ops::{
    Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Shl, ShlAssign, Shr,
    ShrAssign, Sub, SubAssign,
};

#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
mod wasm32;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod x86;

/// Row-major const-size matrix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix<const ROWS: usize, const COLS: usize, T = f32>(pub [[T; COLS]; ROWS]);

/// Row-major square const-size matrix.
pub type SqMatrix<const SZ: usize, T = f32> = Matrix<SZ, SZ, T>;

/// Fast 2x2 f32 matrix backed by glam.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix2(pub Mat2);

/// Fast 3x3 f32 matrix backed by glam.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix3(pub Mat3);

/// Fast 4x4 f32 matrix backed by glam.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix4(pub Mat4);

trait MatrixElementOps: Copy {
    fn add_assign_matrix(out: &mut [Self], rhs: &[Self]);
    fn sub_assign_matrix(out: &mut [Self], rhs: &[Self]);
}

macro_rules! impl_scalar_matrix_element_ops {
    ($($ty:ty),* $(,)?) => {
        $(
            impl MatrixElementOps for $ty {
                #[inline]
                fn add_assign_matrix(out: &mut [Self], rhs: &[Self]) {
                    scalar_add_assign_generic(out, rhs);
                }

                #[inline]
                fn sub_assign_matrix(out: &mut [Self], rhs: &[Self]) {
                    scalar_sub_assign_generic(out, rhs);
                }
            }
        )*
    };
}

impl MatrixElementOps for f32 {
    #[inline]
    fn add_assign_matrix(out: &mut [Self], rhs: &[Self]) {
        simd_add_assign(out, rhs);
    }

    #[inline]
    fn sub_assign_matrix(out: &mut [Self], rhs: &[Self]) {
        simd_sub_assign(out, rhs);
    }
}

impl MatrixElementOps for f64 {
    #[inline]
    fn add_assign_matrix(out: &mut [Self], rhs: &[Self]) {
        simd_add_assign_f64(out, rhs);
    }

    #[inline]
    fn sub_assign_matrix(out: &mut [Self], rhs: &[Self]) {
        simd_sub_assign_f64(out, rhs);
    }
}

impl_scalar_matrix_element_ops!(
    i8, u8, i16, u16, i32, u32, i64, u64, i128, u128, isize, usize
);

macro_rules! impl_scalar_matrix_ops {
    ($ty:ty, $scale:path) => {
        impl<const ROWS: usize, const COLS: usize> Mul<$ty> for Matrix<ROWS, COLS, $ty> {
            type Output = Self;

            #[inline]
            fn mul(self, rhs: $ty) -> Self::Output {
                let mut out = self;
                out *= rhs;
                out
            }
        }

        impl<const ROWS: usize, const COLS: usize> MulAssign<$ty> for Matrix<ROWS, COLS, $ty> {
            #[inline]
            fn mul_assign(&mut self, rhs: $ty) {
                $scale(self.as_mut_slice(), rhs);
            }
        }

        impl<const ROWS: usize, const COLS: usize> Div<$ty> for Matrix<ROWS, COLS, $ty> {
            type Output = Self;

            #[inline]
            fn div(self, rhs: $ty) -> Self::Output {
                let mut out = self;
                out /= rhs;
                out
            }
        }

        impl<const ROWS: usize, const COLS: usize> DivAssign<$ty> for Matrix<ROWS, COLS, $ty> {
            #[inline]
            fn div_assign(&mut self, rhs: $ty) {
                scalar_div_assign_generic(self.as_mut_slice(), rhs);
            }
        }
    };
}

macro_rules! impl_shift_matrix_ops {
    ($($ty:ty),* $(,)?) => {
        $(
            impl<const ROWS: usize, const COLS: usize> Shl<u32> for Matrix<ROWS, COLS, $ty> {
                type Output = Self;

                #[inline]
                fn shl(self, rhs: u32) -> Self::Output {
                    let mut out = self;
                    out <<= rhs;
                    out
                }
            }

            impl<const ROWS: usize, const COLS: usize> ShlAssign<u32> for Matrix<ROWS, COLS, $ty> {
                #[inline]
                fn shl_assign(&mut self, rhs: u32) {
                    scalar_shl_assign_generic(self.as_mut_slice(), rhs);
                }
            }

            impl<const ROWS: usize, const COLS: usize> Shr<u32> for Matrix<ROWS, COLS, $ty> {
                type Output = Self;

                #[inline]
                fn shr(self, rhs: u32) -> Self::Output {
                    let mut out = self;
                    out >>= rhs;
                    out
                }
            }

            impl<const ROWS: usize, const COLS: usize> ShrAssign<u32> for Matrix<ROWS, COLS, $ty> {
                #[inline]
                fn shr_assign(&mut self, rhs: u32) {
                    scalar_shr_assign_generic(self.as_mut_slice(), rhs);
                }
            }
        )*
    };
}

impl<const ROWS: usize, const COLS: usize, T> Matrix<ROWS, COLS, T> {
    #[inline]
    pub const fn new(rows: [[T; COLS]; ROWS]) -> Self {
        Self(rows)
    }

    #[inline]
    pub const fn rows(&self) -> &[[T; COLS]; ROWS] {
        &self.0
    }

    #[inline]
    pub const fn rows_mut(&mut self) -> &mut [[T; COLS]; ROWS] {
        &mut self.0
    }

    #[inline]
    pub fn into_rows(self) -> [[T; COLS]; ROWS] {
        self.0
    }

    #[inline]
    pub fn to_rows(&self) -> [[T; COLS]; ROWS]
    where
        T: Copy,
    {
        self.0
    }

    #[inline]
    pub fn into_cols(self) -> [[T; ROWS]; COLS]
    where
        T: Copy + Default,
    {
        self.transposed().0
    }

    #[inline]
    pub fn to_cols(&self) -> [[T; ROWS]; COLS]
    where
        T: Copy + Default,
    {
        (*self).into_cols()
    }

    #[inline]
    pub const fn row(&self, row: usize) -> &[T; COLS] {
        &self.0[row]
    }

    #[inline]
    pub fn row_mut(&mut self, row: usize) -> &mut [T; COLS] {
        &mut self.0[row]
    }

    #[inline]
    pub const fn flat_len() -> usize {
        ROWS * COLS
    }

    #[inline]
    pub const fn rows_len() -> usize {
        ROWS
    }

    #[inline]
    pub const fn cols_len() -> usize {
        COLS
    }

    #[inline]
    pub const fn shape() -> (usize, usize) {
        (ROWS, COLS)
    }

    #[inline]
    pub const fn cell_count() -> usize {
        ROWS * COLS
    }

    #[inline]
    pub const fn is_square() -> bool {
        ROWS == COLS
    }

    #[inline]
    pub const fn in_bounds(row: usize, col: usize) -> bool {
        row < ROWS && col < COLS
    }

    #[inline]
    pub const fn flat_index(row: usize, col: usize) -> Option<usize> {
        if Self::in_bounds(row, col) {
            Some(row * COLS + col)
        } else {
            None
        }
    }

    #[inline]
    pub const fn row_col(index: usize) -> Option<(usize, usize)> {
        if index < ROWS * COLS {
            Some((index / COLS, index % COLS))
        } else {
            None
        }
    }

    #[inline]
    pub const fn get(&self, row: usize, col: usize) -> Option<&T> {
        if row < ROWS && col < COLS {
            Some(&self.0[row][col])
        } else {
            None
        }
    }

    #[inline]
    pub fn get_mut(&mut self, row: usize, col: usize) -> Option<&mut T> {
        if row < ROWS && col < COLS {
            Some(&mut self.0[row][col])
        } else {
            None
        }
    }

    #[inline]
    pub fn get_flat(&self, index: usize) -> Option<&T> {
        let (row, col) = Self::row_col(index)?;
        Some(&self.0[row][col])
    }

    #[inline]
    pub fn get_flat_mut(&mut self, index: usize) -> Option<&mut T> {
        let (row, col) = Self::row_col(index)?;
        Some(&mut self.0[row][col])
    }

    #[inline]
    /// Gets a cell without bounds checks.
    ///
    /// # Safety
    ///
    /// Caller must ensure `row < ROWS` and `col < COLS`.
    pub unsafe fn get_unchecked(&self, row: usize, col: usize) -> &T {
        debug_assert!(Self::in_bounds(row, col));
        // SAFETY: caller must ensure row < ROWS and col < COLS.
        unsafe { self.0.get_unchecked(row).get_unchecked(col) }
    }

    #[inline]
    /// Gets a mutable cell without bounds checks.
    ///
    /// # Safety
    ///
    /// Caller must ensure `row < ROWS` and `col < COLS`.
    pub unsafe fn get_unchecked_mut(&mut self, row: usize, col: usize) -> &mut T {
        debug_assert!(Self::in_bounds(row, col));
        // SAFETY: caller must ensure row < ROWS and col < COLS.
        unsafe { self.0.get_unchecked_mut(row).get_unchecked_mut(col) }
    }

    #[inline]
    /// Gets a flat cell without bounds checks.
    ///
    /// # Safety
    ///
    /// Caller must ensure `index < ROWS * COLS`.
    pub unsafe fn get_flat_unchecked(&self, index: usize) -> &T {
        debug_assert!(index < Self::flat_len());
        // SAFETY: caller must ensure index < ROWS * COLS.
        unsafe { self.as_slice().get_unchecked(index) }
    }

    #[inline]
    /// Gets a mutable flat cell without bounds checks.
    ///
    /// # Safety
    ///
    /// Caller must ensure `index < ROWS * COLS`.
    pub unsafe fn get_flat_unchecked_mut(&mut self, index: usize) -> &mut T {
        debug_assert!(index < Self::flat_len());
        // SAFETY: caller must ensure index < ROWS * COLS.
        unsafe { self.as_mut_slice().get_unchecked_mut(index) }
    }

    #[inline]
    pub fn set(&mut self, row: usize, col: usize, value: T) -> bool {
        let Some(slot) = self.get_mut(row, col) else {
            return false;
        };
        *slot = value;
        true
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.as_slice().iter()
    }

    #[inline]
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.as_mut_slice().iter_mut()
    }

    #[inline]
    pub fn rows_iter(&self) -> std::slice::Iter<'_, [T; COLS]> {
        self.0.iter()
    }

    #[inline]
    pub fn rows_iter_mut(&mut self) -> std::slice::IterMut<'_, [T; COLS]> {
        self.0.iter_mut()
    }

    #[inline]
    pub fn row_iter(&self, row: usize) -> Option<std::slice::Iter<'_, T>> {
        self.0.get(row).map(|row| row.iter())
    }

    #[inline]
    pub fn row_iter_mut(&mut self, row: usize) -> Option<std::slice::IterMut<'_, T>> {
        self.0.get_mut(row).map(|row| row.iter_mut())
    }

    #[inline]
    pub fn col_iter(&self, col: usize) -> Option<impl Iterator<Item = &T>> {
        (col < COLS).then(|| self.0.iter().map(move |row| &row[col]))
    }

    #[inline]
    pub fn col_iter_mut(&mut self, col: usize) -> Option<impl Iterator<Item = &mut T>> {
        (col < COLS).then(|| self.0.iter_mut().map(move |row| &mut row[col]))
    }

    #[inline]
    pub fn cells(&self) -> impl Iterator<Item = (usize, usize, &T)> {
        self.as_slice()
            .iter()
            .enumerate()
            .map(|(index, value)| (index / COLS, index % COLS, value))
    }

    #[inline]
    pub fn cells_mut(&mut self) -> impl Iterator<Item = (usize, usize, &mut T)> {
        self.as_mut_slice()
            .iter_mut()
            .enumerate()
            .map(|(index, value)| (index / COLS, index % COLS, value))
    }

    #[inline]
    pub fn for_positions(&self, mut f: impl FnMut(usize, usize)) {
        for row in 0..ROWS {
            for col in 0..COLS {
                f(row, col);
            }
        }
    }

    #[inline]
    pub fn for_each(&self, mut f: impl FnMut(usize, usize, &T)) {
        for row in 0..ROWS {
            for col in 0..COLS {
                // SAFETY: row/col come from matrix bounds.
                f(row, col, unsafe { self.get_unchecked(row, col) });
            }
        }
    }

    #[inline]
    pub fn for_each_mut(&mut self, mut f: impl FnMut(usize, usize, &mut T)) -> &mut Self {
        for row in 0..ROWS {
            for col in 0..COLS {
                // SAFETY: row/col come from matrix bounds.
                f(row, col, unsafe { self.get_unchecked_mut(row, col) });
            }
        }
        self
    }

    #[inline]
    pub fn zip_each<U>(
        &self,
        rhs: &Matrix<ROWS, COLS, U>,
        mut f: impl FnMut(usize, usize, &T, &U),
    ) {
        for row in 0..ROWS {
            for col in 0..COLS {
                // SAFETY: row/col come from matrix bounds for both same-shape matrices.
                f(row, col, unsafe { self.get_unchecked(row, col) }, unsafe {
                    rhs.get_unchecked(row, col)
                });
            }
        }
    }

    #[inline]
    pub fn zip_each_mut<U>(
        &mut self,
        rhs: &Matrix<ROWS, COLS, U>,
        mut f: impl FnMut(usize, usize, &mut T, &U),
    ) -> &mut Self {
        for row in 0..ROWS {
            for col in 0..COLS {
                // SAFETY: row/col come from matrix bounds for both same-shape matrices.
                f(
                    row,
                    col,
                    unsafe { self.get_unchecked_mut(row, col) },
                    unsafe { rhs.get_unchecked(row, col) },
                );
            }
        }
        self
    }

    #[inline]
    pub fn for_neighbors_4(
        &self,
        row: usize,
        col: usize,
        mut f: impl FnMut(usize, usize, &T),
    ) -> bool {
        if !Self::in_bounds(row, col) {
            return false;
        }
        if row > 0 {
            // SAFETY: branch keeps neighbor inside matrix bounds.
            f(row - 1, col, unsafe { self.get_unchecked(row - 1, col) });
        }
        if col > 0 {
            // SAFETY: branch keeps neighbor inside matrix bounds.
            f(row, col - 1, unsafe { self.get_unchecked(row, col - 1) });
        }
        if col + 1 < COLS {
            // SAFETY: branch keeps neighbor inside matrix bounds.
            f(row, col + 1, unsafe { self.get_unchecked(row, col + 1) });
        }
        if row + 1 < ROWS {
            // SAFETY: branch keeps neighbor inside matrix bounds.
            f(row + 1, col, unsafe { self.get_unchecked(row + 1, col) });
        }
        true
    }

    #[inline]
    pub fn for_neighbors_8(
        &self,
        row: usize,
        col: usize,
        mut f: impl FnMut(usize, usize, &T),
    ) -> bool {
        if !Self::in_bounds(row, col) {
            return false;
        }
        if row > 0 && row + 1 < ROWS && col > 0 && col + 1 < COLS {
            let center = row * COLS + col;
            let cells = self.as_slice();
            // SAFETY: interior branch proves all 8 flat neighbor indexes are in bounds.
            unsafe {
                f(row - 1, col - 1, cells.get_unchecked(center - COLS - 1));
                f(row - 1, col, cells.get_unchecked(center - COLS));
                f(row - 1, col + 1, cells.get_unchecked(center - COLS + 1));
                f(row, col - 1, cells.get_unchecked(center - 1));
                f(row, col + 1, cells.get_unchecked(center + 1));
                f(row + 1, col - 1, cells.get_unchecked(center + COLS - 1));
                f(row + 1, col, cells.get_unchecked(center + COLS));
                f(row + 1, col + 1, cells.get_unchecked(center + COLS + 1));
            }
            return true;
        }
        let row_start = row.saturating_sub(1);
        let row_end = (row + 1).min(ROWS - 1);
        let col_start = col.saturating_sub(1);
        let col_end = (col + 1).min(COLS - 1);
        for next_row in row_start..=row_end {
            for next_col in col_start..=col_end {
                if next_row != row || next_col != col {
                    // SAFETY: row/col ranges clamp to matrix bounds.
                    f(next_row, next_col, unsafe {
                        self.get_unchecked(next_row, next_col)
                    });
                }
            }
        }
        true
    }

    #[inline]
    pub fn count_neighbors_4(
        &self,
        row: usize,
        col: usize,
        mut f: impl FnMut(usize, usize, &T) -> bool,
    ) -> usize {
        if !Self::in_bounds(row, col) {
            return 0;
        }
        let mut count = 0;
        if row > 0 {
            // SAFETY: branch keeps neighbor inside matrix bounds.
            if f(row - 1, col, unsafe { self.get_unchecked(row - 1, col) }) {
                count += 1;
            }
        }
        if col > 0 {
            // SAFETY: branch keeps neighbor inside matrix bounds.
            if f(row, col - 1, unsafe { self.get_unchecked(row, col - 1) }) {
                count += 1;
            }
        }
        if col + 1 < COLS {
            // SAFETY: branch keeps neighbor inside matrix bounds.
            if f(row, col + 1, unsafe { self.get_unchecked(row, col + 1) }) {
                count += 1;
            }
        }
        if row + 1 < ROWS {
            // SAFETY: branch keeps neighbor inside matrix bounds.
            if f(row + 1, col, unsafe { self.get_unchecked(row + 1, col) }) {
                count += 1;
            }
        }
        count
    }

    #[inline]
    pub fn count_neighbors_8(
        &self,
        row: usize,
        col: usize,
        mut f: impl FnMut(usize, usize, &T) -> bool,
    ) -> usize {
        if !Self::in_bounds(row, col) {
            return 0;
        }
        if row > 0 && row + 1 < ROWS && col > 0 && col + 1 < COLS {
            let center = row * COLS + col;
            let cells = self.as_slice();
            let mut count = 0;
            // SAFETY: interior branch proves all 8 flat neighbor indexes are in bounds.
            unsafe {
                if f(row - 1, col - 1, cells.get_unchecked(center - COLS - 1)) {
                    count += 1;
                }
                if f(row - 1, col, cells.get_unchecked(center - COLS)) {
                    count += 1;
                }
                if f(row - 1, col + 1, cells.get_unchecked(center - COLS + 1)) {
                    count += 1;
                }
                if f(row, col - 1, cells.get_unchecked(center - 1)) {
                    count += 1;
                }
                if f(row, col + 1, cells.get_unchecked(center + 1)) {
                    count += 1;
                }
                if f(row + 1, col - 1, cells.get_unchecked(center + COLS - 1)) {
                    count += 1;
                }
                if f(row + 1, col, cells.get_unchecked(center + COLS)) {
                    count += 1;
                }
                if f(row + 1, col + 1, cells.get_unchecked(center + COLS + 1)) {
                    count += 1;
                }
            }
            return count;
        }
        let mut count = 0;
        let row_start = row.saturating_sub(1);
        let row_end = (row + 1).min(ROWS - 1);
        let col_start = col.saturating_sub(1);
        let col_end = (col + 1).min(COLS - 1);
        for next_row in row_start..=row_end {
            for next_col in col_start..=col_end {
                if next_row != row || next_col != col {
                    // SAFETY: row/col ranges clamp to matrix bounds.
                    if f(next_row, next_col, unsafe {
                        self.get_unchecked(next_row, next_col)
                    }) {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    #[inline]
    pub fn any_cell(&self, mut f: impl FnMut(usize, usize, &T) -> bool) -> bool {
        self.cells().any(|(row, col, value)| f(row, col, value))
    }

    #[inline]
    pub fn all_cells(&self, mut f: impl FnMut(usize, usize, &T) -> bool) -> bool {
        self.cells().all(|(row, col, value)| f(row, col, value))
    }

    #[inline]
    pub fn count_cells(&self, mut f: impl FnMut(usize, usize, &T) -> bool) -> usize {
        self.cells()
            .filter(|(row, col, value)| f(*row, *col, value))
            .count()
    }

    #[inline]
    pub fn find_cell(&self, mut f: impl FnMut(usize, usize, &T) -> bool) -> Option<(usize, usize)> {
        self.cells()
            .find(|(row, col, value)| f(*row, *col, value))
            .map(|(row, col, _)| (row, col))
    }

    #[inline]
    pub fn fill(&mut self, value: T) -> &mut Self
    where
        T: Copy,
    {
        self.as_mut_slice().fill(value);
        self
    }

    #[inline]
    pub fn fill_with(&mut self, mut f: impl FnMut(usize, usize) -> T) -> &mut Self {
        for (row, col, value) in self.cells_mut() {
            *value = f(row, col);
        }
        self
    }

    #[inline]
    pub fn copy_from_slice(&mut self, slice: &[T]) -> bool
    where
        T: Copy,
    {
        if slice.len() != Self::flat_len() {
            return false;
        }
        self.as_mut_slice().copy_from_slice(slice);
        true
    }

    #[inline]
    pub fn copy_to_slice(&self, out: &mut [T]) -> Option<usize>
    where
        T: Copy,
    {
        self.write_flat(out)
    }

    #[inline]
    pub fn copy_from(&mut self, src: &Self) -> &mut Self
    where
        T: Copy,
    {
        self.as_mut_slice().copy_from_slice(src.as_slice());
        self
    }

    #[inline]
    pub fn clone_from_matrix(&mut self, src: &Self) -> &mut Self
    where
        T: Clone,
    {
        self.as_mut_slice().clone_from_slice(src.as_slice());
        self
    }

    #[inline]
    pub fn swap_cells(&mut self, a: (usize, usize), b: (usize, usize)) -> bool {
        let Some(a_index) = Self::flat_index(a.0, a.1) else {
            return false;
        };
        let Some(b_index) = Self::flat_index(b.0, b.1) else {
            return false;
        };
        self.as_mut_slice().swap(a_index, b_index);
        true
    }

    #[inline]
    pub fn swap_flat(&mut self, a: usize, b: usize) -> bool {
        if a >= Self::flat_len() || b >= Self::flat_len() {
            return false;
        }
        self.as_mut_slice().swap(a, b);
        true
    }

    #[inline]
    pub fn to_vec(&self) -> Vec<T>
    where
        T: Copy,
    {
        self.as_slice().to_vec()
    }

    #[inline]
    pub fn into_vec(self) -> Vec<T> {
        self.0.into_iter().flatten().collect()
    }

    #[inline]
    pub fn sum(&self) -> T
    where
        T: Copy + Default + Add<Output = T>,
    {
        self.as_slice()
            .iter()
            .copied()
            .fold(T::default(), |sum, value| sum + value)
    }

    #[inline]
    pub fn product(&self) -> T
    where
        T: Copy + From<u8> + Mul<Output = T>,
    {
        self.as_slice()
            .iter()
            .copied()
            .fold(T::from(1), |product, value| product * value)
    }

    #[inline]
    pub fn fold_cells<U>(&self, init: U, mut f: impl FnMut(U, usize, usize, &T) -> U) -> U {
        self.cells()
            .fold(init, |acc, (row, col, value)| f(acc, row, col, value))
    }

    #[inline]
    pub fn min_cell(&self) -> Option<(usize, usize, T)>
    where
        T: Copy + PartialOrd,
    {
        self.cells()
            .filter_map(|(row, col, value)| value.partial_cmp(value).map(|_| (row, col, *value)))
            .min_by(|(_, _, a), (_, _, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    #[inline]
    pub fn max_cell(&self) -> Option<(usize, usize, T)>
    where
        T: Copy + PartialOrd,
    {
        self.cells()
            .filter_map(|(row, col, value)| value.partial_cmp(value).map(|_| (row, col, *value)))
            .max_by(|(_, _, a), (_, _, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    #[inline]
    pub fn map_cells<U>(self, mut f: impl FnMut(usize, usize, T) -> U) -> Matrix<ROWS, COLS, U>
    where
        T: Copy,
    {
        Matrix(std::array::from_fn(|row| {
            std::array::from_fn(|col| {
                // SAFETY: array::from_fn indexes are in matrix bounds.
                f(row, col, unsafe { *self.get_unchecked(row, col) })
            })
        }))
    }

    #[inline]
    pub fn resize<const NEW_ROWS: usize, const NEW_COLS: usize>(
        self,
        fill: T,
    ) -> Matrix<NEW_ROWS, NEW_COLS, T>
    where
        T: Copy,
    {
        self.resize_with(|_, _| fill)
    }

    #[inline]
    pub fn resize_default<const NEW_ROWS: usize, const NEW_COLS: usize>(
        self,
    ) -> Matrix<NEW_ROWS, NEW_COLS, T>
    where
        T: Copy + Default,
    {
        self.resize_with(|_, _| T::default())
    }

    #[inline]
    pub fn resize_with<const NEW_ROWS: usize, const NEW_COLS: usize>(
        self,
        mut fill: impl FnMut(usize, usize) -> T,
    ) -> Matrix<NEW_ROWS, NEW_COLS, T>
    where
        T: Copy,
    {
        Matrix(std::array::from_fn(|row| {
            std::array::from_fn(|col| {
                if row < ROWS && col < COLS {
                    self.0[row][col]
                } else {
                    fill(row, col)
                }
            })
        }))
    }

    #[inline]
    pub fn as_slice(&self) -> &[T] {
        // SAFETY: nested arrays are contiguous and contain exactly ROWS * COLS elements.
        unsafe { std::slice::from_raw_parts(self.0.as_ptr().cast(), ROWS * COLS) }
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        // SAFETY: nested arrays are contiguous and contain exactly ROWS * COLS elements.
        unsafe { std::slice::from_raw_parts_mut(self.0.as_mut_ptr().cast(), ROWS * COLS) }
    }

    /// Build from row-major flat slice.
    ///
    /// Returns `None` when `slice.len() != ROWS * COLS`.
    #[inline]
    pub fn from_slice(slice: &[T]) -> Option<Self>
    where
        T: Copy + Default,
    {
        if slice.len() != ROWS * COLS {
            return None;
        }

        let mut rows = [[T::default(); COLS]; ROWS];
        for r in 0..ROWS {
            for c in 0..COLS {
                rows[r][c] = slice[r * COLS + c];
            }
        }
        Some(Self(rows))
    }

    /// Build from row-major flat vec.
    ///
    /// Extra tail values are ignored.
    /// Returns `None` when vec is too short.
    #[inline]
    pub fn from_vec(values: Vec<T>) -> Option<Self>
    where
        T: Copy + Default,
    {
        Self::from_vec_offset(values, 0)
    }

    /// Build from row-major flat vec starting at `offset`.
    ///
    /// Reads `ROWS * COLS` values from `offset`.
    /// Extra tail values are ignored.
    /// Returns `None` when the window is out of range.
    #[inline]
    pub fn from_vec_offset(values: Vec<T>, offset: usize) -> Option<Self>
    where
        T: Copy + Default,
    {
        let end = offset.checked_add(Self::flat_len())?;
        if values.len() < end {
            return None;
        }
        Self::from_slice(&values[offset..end])
    }

    /// Build from row vecs.
    ///
    /// Each inner vec supplies one row.
    /// Extra rows and extra tail columns are ignored.
    /// Returns `None` when any required row or column is missing.
    #[inline]
    pub fn from_vec_rows(rows: Vec<Vec<T>>) -> Option<Self>
    where
        T: Copy + Default,
    {
        Self::from_vec_rows_offset(rows, 0, 0)
    }

    /// Build from row vecs starting at `row_offset` and `col_offset`.
    ///
    /// Reads `ROWS` rows and `COLS` columns from the row/column window.
    /// Extra rows and extra tail columns are ignored.
    /// Returns `None` when the window is out of range.
    #[inline]
    pub fn from_vec_rows_offset(
        rows: Vec<Vec<T>>,
        row_offset: usize,
        col_offset: usize,
    ) -> Option<Self>
    where
        T: Copy + Default,
    {
        let row_end = row_offset.checked_add(ROWS)?;
        let col_end = col_offset.checked_add(COLS)?;
        if rows.len() < row_end {
            return None;
        }

        let mut out = [[T::default(); COLS]; ROWS];
        for row in 0..ROWS {
            let src = &rows[row_offset + row];
            if src.len() < col_end {
                return None;
            }
            out[row].copy_from_slice(&src[col_offset..col_end]);
        }
        Some(Self(out))
    }

    #[inline]
    pub fn write_flat(&self, out: &mut [T]) -> Option<usize>
    where
        T: Copy,
    {
        let len = Self::flat_len();
        if out.len() < len {
            return None;
        }
        out[..len].copy_from_slice(self.as_slice());
        Some(len)
    }

    #[inline]
    pub fn col(&self, col: usize) -> [T; ROWS]
    where
        T: Copy + Default,
    {
        let mut out = [T::default(); ROWS];
        for (r, out_item) in out.iter_mut().enumerate() {
            *out_item = self.0[r][col];
        }
        out
    }

    #[inline]
    pub fn transposed(self) -> Matrix<COLS, ROWS, T>
    where
        T: Copy + Default,
    {
        let mut out = [[T::default(); ROWS]; COLS];
        for (r, row) in self.0.iter().enumerate() {
            for (c, value) in row.iter().enumerate() {
                out[c][r] = *value;
            }
        }
        Matrix(out)
    }

    #[inline]
    pub fn transpose(&mut self) -> &mut Self
    where
        T: Copy + Default,
    {
        static_assert_square::<ROWS, COLS>();
        for r in 0..ROWS {
            for c in (r + 1)..COLS {
                let tmp = self.0[r][c];
                self.0[r][c] = self.0[c][r];
                self.0[c][r] = tmp;
            }
        }
        self
    }

    #[inline]
    pub fn mul_generic<const K: usize>(self, rhs: Matrix<COLS, K, T>) -> Matrix<ROWS, K, T>
    where
        T: Copy + Default + Add<Output = T> + Mul<Output = T>,
    {
        let mut out = [[T::default(); K]; ROWS];
        for (r, out_row) in out.iter_mut().enumerate() {
            for (k, out_item) in out_row.iter_mut().enumerate() {
                let mut sum = T::default();
                for c in 0..COLS {
                    sum = sum + self.0[r][c] * rhs.0[c][k];
                }
                *out_item = sum;
            }
        }
        Matrix(out)
    }
}

impl<const ROWS: usize, const COLS: usize, T> Matrix<ROWS, COLS, T>
where
    T: PartialEq,
{
    #[inline]
    pub fn find_position(&self, value: &T) -> Option<(usize, usize)> {
        for r in 0..ROWS {
            for c in 0..COLS {
                if &self.0[r][c] == value {
                    return Some((r, c));
                }
            }
        }
        None
    }

    #[inline]
    pub fn find_flat_index(&self, value: &T) -> Option<usize> {
        self.find_position(value)
            .and_then(|(row, col)| Self::flat_index(row, col))
    }
}

impl<const N: usize> Matrix<N, N, f32> {
    #[inline]
    pub fn identity() -> Self {
        let mut rows = [[0.0; N]; N];
        for (i, row) in rows.iter_mut().enumerate() {
            row[i] = 1.0;
        }
        Self(rows)
    }

    #[inline]
    pub fn determinant(self) -> f32 {
        if N == 2 {
            return Matrix2::from_rows(matrix_rows_2(self.0)).0.determinant();
        }
        if N == 3 {
            return Matrix3::from_rows(matrix_rows_3(self.0)).0.determinant();
        }
        if N == 4 {
            return Matrix4::from_rows(matrix_rows_4(self.0)).0.determinant();
        }

        let mut rows = self.0;
        let mut det = 1.0;
        for pivot in 0..N {
            let mut best = pivot;
            let mut best_abs = rows[pivot][pivot].abs();
            for (r, row) in rows.iter().enumerate().skip(pivot + 1) {
                let abs = row[pivot].abs();
                if abs > best_abs {
                    best = r;
                    best_abs = abs;
                }
            }
            if best_abs <= f32::EPSILON {
                return 0.0;
            }
            if best != pivot {
                rows.swap(pivot, best);
                det = -det;
            }

            let pivot_value = rows[pivot][pivot];
            det *= pivot_value;
            let pivot_row = rows[pivot];
            for row in rows.iter_mut().skip(pivot + 1) {
                let factor = row[pivot] / pivot_value;
                row[pivot] = 0.0;
                for (c, item) in row.iter_mut().enumerate().skip(pivot + 1) {
                    *item -= factor * pivot_row[c];
                }
            }
        }
        det
    }

    #[inline]
    pub fn inverse(self) -> Option<Self> {
        if N == 2 {
            if self.determinant().abs() <= f32::EPSILON {
                return None;
            }
            return Some(Self(matrix_from_rows_2(
                Matrix2::from_rows(matrix_rows_2(self.0))
                    .inverse()
                    .to_rows(),
            )));
        }
        if N == 3 {
            if self.determinant().abs() <= f32::EPSILON {
                return None;
            }
            return Some(Self(matrix_from_rows_3(
                Matrix3::from_rows(matrix_rows_3(self.0))
                    .inverse()
                    .to_rows(),
            )));
        }
        if N == 4 {
            if self.determinant().abs() <= f32::EPSILON {
                return None;
            }
            return Some(Self(matrix_from_rows_4(
                Matrix4::from_rows(matrix_rows_4(self.0))
                    .inverse()
                    .to_rows(),
            )));
        }

        let mut lhs = self.0;
        let mut rhs = Self::identity().0;
        for pivot in 0..N {
            let mut best = pivot;
            let mut best_abs = lhs[pivot][pivot].abs();
            for (r, row) in lhs.iter().enumerate().skip(pivot + 1) {
                let abs = row[pivot].abs();
                if abs > best_abs {
                    best = r;
                    best_abs = abs;
                }
            }
            if best_abs <= f32::EPSILON {
                return None;
            }
            if best != pivot {
                lhs.swap(pivot, best);
                rhs.swap(pivot, best);
            }

            let pivot_value = lhs[pivot][pivot];
            for c in 0..N {
                lhs[pivot][c] /= pivot_value;
                rhs[pivot][c] /= pivot_value;
            }

            for r in 0..N {
                if r == pivot {
                    continue;
                }
                let factor = lhs[r][pivot];
                if factor == 0.0 {
                    continue;
                }
                for c in 0..N {
                    lhs[r][c] -= factor * lhs[pivot][c];
                    rhs[r][c] -= factor * rhs[pivot][c];
                }
            }
        }

        Some(Self(rhs))
    }
}

impl<const ROWS: usize, const COLS: usize> Matrix<ROWS, COLS, f32> {
    #[inline]
    pub fn packed_len() -> usize {
        ROWS * COLS
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self.as_slice())
    }

    #[inline]
    pub fn write_packed(&self, out: &mut [f32]) -> Option<usize> {
        let len = Self::packed_len();
        if out.len() < len {
            return None;
        }
        out[..len].copy_from_slice(self.as_slice());
        Some(len)
    }

    #[inline]
    pub fn read_packed(input: &[f32]) -> Option<Self> {
        Self::from_slice(input)
    }

    #[inline]
    pub fn add_fast(self, rhs: Self) -> Self {
        self.add_f32(rhs)
    }

    #[inline]
    pub fn add_f32(self, rhs: Self) -> Self {
        let mut out = self;
        out.add_assign_f32(rhs);
        out
    }

    #[inline]
    pub fn add_assign_f32(&mut self, rhs: Self) -> &mut Self {
        simd_add_assign(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn sub_fast(self, rhs: Self) -> Self {
        self.sub_f32(rhs)
    }

    #[inline]
    pub fn sub_f32(self, rhs: Self) -> Self {
        let mut out = self;
        out.sub_assign_f32(rhs);
        out
    }

    #[inline]
    pub fn sub_assign_f32(&mut self, rhs: Self) -> &mut Self {
        simd_sub_assign(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn scale_fast(self, rhs: f32) -> Self {
        self.scale_f32(rhs)
    }

    #[inline]
    pub fn scale_f32(self, rhs: f32) -> Self {
        let mut out = self;
        out.scale_assign_f32(rhs);
        out
    }

    #[inline]
    pub fn scale_assign_f32(&mut self, rhs: f32) -> &mut Self {
        simd_scale_assign(self.as_mut_slice(), rhs);
        self
    }

    #[inline]
    pub fn mul_f32<const K: usize>(self, rhs: Matrix<COLS, K, f32>) -> Matrix<ROWS, K, f32> {
        if ROWS == 2 && COLS == 2 && K == 2 {
            return Matrix(matrix_from_rows_2(
                (Matrix2::from_rows(matrix_rows_2(self.0))
                    * Matrix2::from_rows(matrix_rows_2(rhs.0)))
                .to_rows(),
            ));
        }
        if ROWS == 3 && COLS == 3 && K == 3 {
            return Matrix(matrix_from_rows_3(
                (Matrix3::from_rows(matrix_rows_3(self.0))
                    * Matrix3::from_rows(matrix_rows_3(rhs.0)))
                .to_rows(),
            ));
        }
        if ROWS == 4 && COLS == 4 && K == 4 {
            return Matrix(matrix_from_rows_4(
                (Matrix4::from_rows(matrix_rows_4(self.0))
                    * Matrix4::from_rows(matrix_rows_4(rhs.0)))
                .to_rows(),
            ));
        }

        self.mul_f32_transposed(rhs)
    }

    #[inline]
    pub fn mul_f32_transposed<const K: usize>(
        self,
        rhs: Matrix<COLS, K, f32>,
    ) -> Matrix<ROWS, K, f32> {
        if K >= COLS {
            return self.mul_generic(rhs);
        }

        let rhs_t = rhs.transposed();
        let mut out = [[0.0; K]; ROWS];
        for (r, out_row) in out.iter_mut().enumerate() {
            let lhs_row = self.row(r);
            for (k, out_item) in out_row.iter_mut().enumerate() {
                *out_item = simd_dot_f32(lhs_row, rhs_t.row(k));
            }
        }
        Matrix(out)
    }
}

impl<const ROWS: usize, const COLS: usize> Matrix<ROWS, COLS, f64> {
    #[inline]
    pub fn add_fast(mut self, rhs: Self) -> Self {
        simd_add_assign_f64(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn sub_fast(mut self, rhs: Self) -> Self {
        simd_sub_assign_f64(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn scale_fast(mut self, rhs: f64) -> Self {
        simd_scale_assign_f64(self.as_mut_slice(), rhs);
        self
    }
}

impl<const ROWS: usize, const COLS: usize> Matrix<ROWS, COLS, i32> {
    #[inline]
    pub fn add_fast(mut self, rhs: Self) -> Self {
        simd_add_assign_i32(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn sub_fast(mut self, rhs: Self) -> Self {
        simd_sub_assign_i32(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn scale_fast(mut self, rhs: i32) -> Self {
        simd_scale_assign_i32(self.as_mut_slice(), rhs);
        self
    }
}

impl<const ROWS: usize, const COLS: usize> Matrix<ROWS, COLS, u32> {
    #[inline]
    pub fn add_fast(mut self, rhs: Self) -> Self {
        simd_add_assign_u32(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn sub_fast(mut self, rhs: Self) -> Self {
        simd_sub_assign_u32(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn scale_fast(mut self, rhs: u32) -> Self {
        simd_scale_assign_u32(self.as_mut_slice(), rhs);
        self
    }
}

impl<const ROWS: usize, const COLS: usize> Matrix<ROWS, COLS, i8> {
    #[inline]
    pub fn add_fast(mut self, rhs: Self) -> Self {
        simd_add_assign_i8(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn sub_fast(mut self, rhs: Self) -> Self {
        simd_sub_assign_i8(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn scale_fast(mut self, rhs: i8) -> Self {
        scalar_scale_assign_generic(self.as_mut_slice(), rhs);
        self
    }
}

impl<const ROWS: usize, const COLS: usize> Matrix<ROWS, COLS, u8> {
    #[inline]
    pub fn add_fast(mut self, rhs: Self) -> Self {
        simd_add_assign_u8(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn sub_fast(mut self, rhs: Self) -> Self {
        simd_sub_assign_u8(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn scale_fast(mut self, rhs: u8) -> Self {
        scalar_scale_assign_generic(self.as_mut_slice(), rhs);
        self
    }
}

impl<const ROWS: usize, const COLS: usize> Matrix<ROWS, COLS, i16> {
    #[inline]
    pub fn add_fast(mut self, rhs: Self) -> Self {
        simd_add_assign_i16(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn sub_fast(mut self, rhs: Self) -> Self {
        simd_sub_assign_i16(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn scale_fast(mut self, rhs: i16) -> Self {
        simd_scale_assign_i16(self.as_mut_slice(), rhs);
        self
    }
}

impl<const ROWS: usize, const COLS: usize> Matrix<ROWS, COLS, u16> {
    #[inline]
    pub fn add_fast(mut self, rhs: Self) -> Self {
        simd_add_assign_u16(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn sub_fast(mut self, rhs: Self) -> Self {
        simd_sub_assign_u16(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn scale_fast(mut self, rhs: u16) -> Self {
        simd_scale_assign_u16(self.as_mut_slice(), rhs);
        self
    }
}

impl<const ROWS: usize, const COLS: usize> Matrix<ROWS, COLS, i64> {
    #[inline]
    pub fn add_fast(mut self, rhs: Self) -> Self {
        simd_add_assign_i64(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn sub_fast(mut self, rhs: Self) -> Self {
        simd_sub_assign_i64(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn scale_fast(mut self, rhs: i64) -> Self {
        scalar_scale_assign_generic(self.as_mut_slice(), rhs);
        self
    }
}

impl<const ROWS: usize, const COLS: usize> Matrix<ROWS, COLS, u64> {
    #[inline]
    pub fn add_fast(mut self, rhs: Self) -> Self {
        simd_add_assign_u64(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn sub_fast(mut self, rhs: Self) -> Self {
        simd_sub_assign_u64(self.as_mut_slice(), rhs.as_slice());
        self
    }

    #[inline]
    pub fn scale_fast(mut self, rhs: u64) -> Self {
        scalar_scale_assign_generic(self.as_mut_slice(), rhs);
        self
    }
}

macro_rules! impl_scalar_fast_matrix_ops {
    ($($ty:ty),* $(,)?) => {
        $(
            impl<const ROWS: usize, const COLS: usize> Matrix<ROWS, COLS, $ty> {
                #[inline]
                pub fn add_fast(mut self, rhs: Self) -> Self {
                    scalar_add_assign_generic(self.as_mut_slice(), rhs.as_slice());
                    self
                }

                #[inline]
                pub fn sub_fast(mut self, rhs: Self) -> Self {
                    scalar_sub_assign_generic(self.as_mut_slice(), rhs.as_slice());
                    self
                }

                #[inline]
                pub fn scale_fast(mut self, rhs: $ty) -> Self {
                    scalar_scale_assign_generic(self.as_mut_slice(), rhs);
                    self
                }
            }
        )*
    };
}

impl_scalar_fast_matrix_ops!(i128, isize, u128, usize);

mod simd;
mod wrappers;
use simd::*;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
