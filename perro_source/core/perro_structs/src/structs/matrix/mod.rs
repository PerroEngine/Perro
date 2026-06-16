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
    pub fn set(&mut self, row: usize, col: usize, value: T) -> bool {
        let Some(slot) = self.get_mut(row, col) else {
            return false;
        };
        *slot = value;
        true
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

impl Matrix<2, 2, f32> {
    #[inline]
    pub fn to_glam(self) -> Mat2 {
        Matrix2::from(self).0
    }

    #[inline]
    pub fn from_glam(mat: Mat2) -> Self {
        Matrix2(mat).into()
    }
}

impl Matrix<3, 3, f32> {
    #[inline]
    pub fn to_glam(self) -> Mat3 {
        Matrix3::from(self).0
    }

    #[inline]
    pub fn from_glam(mat: Mat3) -> Self {
        Matrix3(mat).into()
    }
}

impl Matrix<4, 4, f32> {
    #[inline]
    pub fn to_glam(self) -> Mat4 {
        Matrix4::from(self).0
    }

    #[inline]
    pub fn from_glam(mat: Mat4) -> Self {
        Matrix4(mat).into()
    }
}

impl<const ROWS: usize, const COLS: usize, T> Default for Matrix<ROWS, COLS, T>
where
    T: Copy + Default,
{
    #[inline]
    fn default() -> Self {
        Self([[T::default(); COLS]; ROWS])
    }
}

impl<const ROWS: usize, const COLS: usize, T> fmt::Display for Matrix<ROWS, COLS, T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Matrix[{ROWS}x{COLS}]")?;

        for r in 0..ROWS {
            write!(f, "[")?;

            for c in 0..COLS {
                if c > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", self.0[r][c])?;
            }

            write!(f, "]")?;

            if r + 1 < ROWS {
                writeln!(f)?;
            }
        }

        Ok(())
    }
}

impl<const ROWS: usize, const COLS: usize, T> Index<(usize, usize)> for Matrix<ROWS, COLS, T> {
    type Output = T;

    #[inline]
    fn index(&self, index: (usize, usize)) -> &Self::Output {
        &self.0[index.0][index.1]
    }
}

impl<const ROWS: usize, const COLS: usize, T> IndexMut<(usize, usize)> for Matrix<ROWS, COLS, T> {
    #[inline]
    fn index_mut(&mut self, index: (usize, usize)) -> &mut Self::Output {
        &mut self.0[index.0][index.1]
    }
}

impl<const ROWS: usize, const COLS: usize, T> From<[[T; COLS]; ROWS]> for Matrix<ROWS, COLS, T> {
    #[inline]
    fn from(rows: [[T; COLS]; ROWS]) -> Self {
        Self(rows)
    }
}

impl<const ROWS: usize, const COLS: usize, T> From<Matrix<ROWS, COLS, T>> for [[T; COLS]; ROWS] {
    #[inline]
    fn from(matrix: Matrix<ROWS, COLS, T>) -> Self {
        matrix.0
    }
}

impl<const ROWS: usize, const COLS: usize, T> Add for Matrix<ROWS, COLS, T>
where
    T: Copy + MatrixElementOps,
{
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        let mut out = self;
        out += rhs;
        out
    }
}

impl<const ROWS: usize, const COLS: usize, T> AddAssign for Matrix<ROWS, COLS, T>
where
    T: Copy + MatrixElementOps,
{
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        T::add_assign_matrix(self.as_mut_slice(), rhs.as_slice());
    }
}

impl<const ROWS: usize, const COLS: usize, T> Sub for Matrix<ROWS, COLS, T>
where
    T: Copy + MatrixElementOps,
{
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        let mut out = self;
        out -= rhs;
        out
    }
}

impl<const ROWS: usize, const COLS: usize, T> SubAssign for Matrix<ROWS, COLS, T>
where
    T: Copy + MatrixElementOps,
{
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        T::sub_assign_matrix(self.as_mut_slice(), rhs.as_slice());
    }
}

impl<const ROWS: usize, const COLS: usize> Mul<f32> for Matrix<ROWS, COLS, f32> {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        let mut out = self;
        out *= rhs;
        out
    }
}

impl<const ROWS: usize, const COLS: usize, const K: usize> Mul<Matrix<COLS, K, f32>>
    for Matrix<ROWS, COLS, f32>
{
    type Output = Matrix<ROWS, K, f32>;

    #[inline]
    fn mul(self, rhs: Matrix<COLS, K, f32>) -> Self::Output {
        self.mul_f32(rhs)
    }
}

impl<const ROWS: usize, const COLS: usize> MulAssign<f32> for Matrix<ROWS, COLS, f32> {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        self.scale_assign_f32(rhs);
    }
}

impl<const ROWS: usize, const COLS: usize> Div<f32> for Matrix<ROWS, COLS, f32> {
    type Output = Self;

    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        let mut out = self;
        out /= rhs;
        out
    }
}

impl<const ROWS: usize, const COLS: usize> DivAssign<f32> for Matrix<ROWS, COLS, f32> {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        simd_scale_assign(self.as_mut_slice(), rhs.recip());
    }
}

impl_scalar_matrix_ops!(f64, simd_scale_assign_f64);
impl_scalar_matrix_ops!(i32, simd_scale_assign_i32);
impl_scalar_matrix_ops!(u32, simd_scale_assign_u32);
impl_scalar_matrix_ops!(i8, scalar_scale_assign_generic);
impl_scalar_matrix_ops!(u8, scalar_scale_assign_generic);
impl_scalar_matrix_ops!(i16, simd_scale_assign_i16);
impl_scalar_matrix_ops!(u16, simd_scale_assign_u16);
impl_scalar_matrix_ops!(i64, scalar_scale_assign_generic);
impl_scalar_matrix_ops!(u64, scalar_scale_assign_generic);
impl_scalar_matrix_ops!(i128, scalar_scale_assign_generic);
impl_scalar_matrix_ops!(u128, scalar_scale_assign_generic);
impl_scalar_matrix_ops!(isize, scalar_scale_assign_generic);
impl_scalar_matrix_ops!(usize, scalar_scale_assign_generic);
impl_shift_matrix_ops!(
    i8, u8, i16, u16, i32, u32, i64, u64, i128, u128, isize, usize
);

impl Matrix2 {
    pub const IDENTITY: Self = Self(Mat2::IDENTITY);
    pub const ZERO: Self = Self(Mat2::ZERO);

    #[inline]
    pub const fn new(mat: Mat2) -> Self {
        Self(mat)
    }

    #[inline]
    pub fn from_rows(rows: [[f32; 2]; 2]) -> Self {
        Self(Mat2::from_cols_array_2d(&[
            [rows[0][0], rows[1][0]],
            [rows[0][1], rows[1][1]],
        ]))
    }

    #[inline]
    pub fn to_rows(self) -> [[f32; 2]; 2] {
        let cols = self.0.to_cols_array_2d();
        [[cols[0][0], cols[1][0]], [cols[0][1], cols[1][1]]]
    }

    #[inline]
    pub fn write_packed(self, out: &mut [f32]) -> Option<usize> {
        Matrix::<2, 2>::from(self).write_packed(out)
    }

    #[inline]
    pub fn as_bytes(self) -> [u8; 16] {
        let mut out = [0; 16];
        out.copy_from_slice(Matrix::<2, 2>::from(self).as_bytes());
        out
    }

    #[inline]
    pub fn transposed(self) -> Self {
        Self(self.0.transpose())
    }

    #[inline]
    pub fn inverse(self) -> Self {
        Self(self.0.inverse())
    }
}

impl Matrix3 {
    pub const IDENTITY: Self = Self(Mat3::IDENTITY);
    pub const ZERO: Self = Self(Mat3::ZERO);

    #[inline]
    pub const fn new(mat: Mat3) -> Self {
        Self(mat)
    }

    #[inline]
    pub fn from_rows(rows: [[f32; 3]; 3]) -> Self {
        Self(Mat3::from_cols_array_2d(&[
            [rows[0][0], rows[1][0], rows[2][0]],
            [rows[0][1], rows[1][1], rows[2][1]],
            [rows[0][2], rows[1][2], rows[2][2]],
        ]))
    }

    #[inline]
    pub fn to_rows(self) -> [[f32; 3]; 3] {
        let cols = self.0.to_cols_array_2d();
        [
            [cols[0][0], cols[1][0], cols[2][0]],
            [cols[0][1], cols[1][1], cols[2][1]],
            [cols[0][2], cols[1][2], cols[2][2]],
        ]
    }

    #[inline]
    pub fn write_packed(self, out: &mut [f32]) -> Option<usize> {
        Matrix::<3, 3>::from(self).write_packed(out)
    }

    #[inline]
    pub fn as_bytes(self) -> [u8; 36] {
        let mut out = [0; 36];
        out.copy_from_slice(Matrix::<3, 3>::from(self).as_bytes());
        out
    }

    #[inline]
    pub fn transposed(self) -> Self {
        Self(self.0.transpose())
    }

    #[inline]
    pub fn inverse(self) -> Self {
        Self(self.0.inverse())
    }
}

impl Matrix4 {
    pub const IDENTITY: Self = Self(Mat4::IDENTITY);
    pub const ZERO: Self = Self(Mat4::ZERO);

    #[inline]
    pub const fn new(mat: Mat4) -> Self {
        Self(mat)
    }

    #[inline]
    pub fn from_rows(rows: [[f32; 4]; 4]) -> Self {
        Self(Mat4::from_cols_array_2d(&[
            [rows[0][0], rows[1][0], rows[2][0], rows[3][0]],
            [rows[0][1], rows[1][1], rows[2][1], rows[3][1]],
            [rows[0][2], rows[1][2], rows[2][2], rows[3][2]],
            [rows[0][3], rows[1][3], rows[2][3], rows[3][3]],
        ]))
    }

    #[inline]
    pub fn to_rows(self) -> [[f32; 4]; 4] {
        let cols = self.0.to_cols_array_2d();
        [
            [cols[0][0], cols[1][0], cols[2][0], cols[3][0]],
            [cols[0][1], cols[1][1], cols[2][1], cols[3][1]],
            [cols[0][2], cols[1][2], cols[2][2], cols[3][2]],
            [cols[0][3], cols[1][3], cols[2][3], cols[3][3]],
        ]
    }

    #[inline]
    pub fn write_packed(self, out: &mut [f32]) -> Option<usize> {
        Matrix::<4, 4>::from(self).write_packed(out)
    }

    #[inline]
    pub fn as_bytes(self) -> [u8; 64] {
        let mut out = [0; 64];
        out.copy_from_slice(Matrix::<4, 4>::from(self).as_bytes());
        out
    }

    #[inline]
    pub fn transposed(self) -> Self {
        Self(self.0.transpose())
    }

    #[inline]
    pub fn inverse(self) -> Self {
        Self(self.0.inverse())
    }
}

impl From<Matrix<2, 2, f32>> for Matrix2 {
    #[inline]
    fn from(matrix: Matrix<2, 2, f32>) -> Self {
        Self::from_rows(matrix.0)
    }
}

impl From<Matrix2> for Matrix<2, 2, f32> {
    #[inline]
    fn from(matrix: Matrix2) -> Self {
        Self(matrix.to_rows())
    }
}

impl From<Matrix<3, 3, f32>> for Matrix3 {
    #[inline]
    fn from(matrix: Matrix<3, 3, f32>) -> Self {
        Self::from_rows(matrix.0)
    }
}

impl From<Matrix3> for Matrix<3, 3, f32> {
    #[inline]
    fn from(matrix: Matrix3) -> Self {
        Self(matrix.to_rows())
    }
}

impl From<Matrix<4, 4, f32>> for Matrix4 {
    #[inline]
    fn from(matrix: Matrix<4, 4, f32>) -> Self {
        Self::from_rows(matrix.0)
    }
}

impl From<Matrix4> for Matrix<4, 4, f32> {
    #[inline]
    fn from(matrix: Matrix4) -> Self {
        Self(matrix.to_rows())
    }
}

impl From<Mat2> for Matrix2 {
    #[inline]
    fn from(mat: Mat2) -> Self {
        Self(mat)
    }
}

impl From<Matrix2> for Mat2 {
    #[inline]
    fn from(matrix: Matrix2) -> Self {
        matrix.0
    }
}

impl From<Mat3> for Matrix3 {
    #[inline]
    fn from(mat: Mat3) -> Self {
        Self(mat)
    }
}

impl From<Matrix3> for Mat3 {
    #[inline]
    fn from(matrix: Matrix3) -> Self {
        matrix.0
    }
}

impl From<Mat4> for Matrix4 {
    #[inline]
    fn from(mat: Mat4) -> Self {
        Self(mat)
    }
}

impl From<Matrix4> for Mat4 {
    #[inline]
    fn from(matrix: Matrix4) -> Self {
        matrix.0
    }
}

macro_rules! impl_fast_matrix_ops {
    ($name:ident) => {
        impl Add for $name {
            type Output = Self;

            #[inline]
            fn add(self, rhs: Self) -> Self::Output {
                Self(self.0 + rhs.0)
            }
        }

        impl AddAssign for $name {
            #[inline]
            fn add_assign(&mut self, rhs: Self) {
                self.0 += rhs.0;
            }
        }

        impl Sub for $name {
            type Output = Self;

            #[inline]
            fn sub(self, rhs: Self) -> Self::Output {
                Self(self.0 - rhs.0)
            }
        }

        impl SubAssign for $name {
            #[inline]
            fn sub_assign(&mut self, rhs: Self) {
                self.0 -= rhs.0;
            }
        }

        impl Mul for $name {
            type Output = Self;

            #[inline]
            fn mul(self, rhs: Self) -> Self::Output {
                Self(self.0 * rhs.0)
            }
        }

        impl MulAssign for $name {
            #[inline]
            fn mul_assign(&mut self, rhs: Self) {
                self.0 *= rhs.0;
            }
        }

        impl Mul<f32> for $name {
            type Output = Self;

            #[inline]
            fn mul(self, rhs: f32) -> Self::Output {
                Self(self.0 * rhs)
            }
        }

        impl MulAssign<f32> for $name {
            #[inline]
            fn mul_assign(&mut self, rhs: f32) {
                self.0 *= rhs;
            }
        }

        impl Div<f32> for $name {
            type Output = Self;

            #[inline]
            fn div(self, rhs: f32) -> Self::Output {
                Self(self.0 / rhs)
            }
        }

        impl DivAssign<f32> for $name {
            #[inline]
            fn div_assign(&mut self, rhs: f32) {
                self.0 /= rhs;
            }
        }
    };
}

impl_fast_matrix_ops!(Matrix2);
impl_fast_matrix_ops!(Matrix3);
impl_fast_matrix_ops!(Matrix4);

#[inline]
fn simd_add_assign(out: &mut [f32], rhs: &[f32]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_f32(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_f32(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_f32(out, rhs) {
        return;
    }
    scalar_add_assign(out, rhs);
}

#[inline]
fn simd_sub_assign(out: &mut [f32], rhs: &[f32]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_f32(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_f32(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_f32(out, rhs) {
        return;
    }
    scalar_sub_assign(out, rhs);
}

#[inline]
fn simd_scale_assign(out: &mut [f32], rhs: f32) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_scale_assign_f32(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_scale_assign_f32(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_scale_assign_f32(out, rhs) {
        return;
    }
    scalar_scale_assign(out, rhs);
}

#[inline]
fn scalar_add_assign(out: &mut [f32], rhs: &[f32]) {
    for (dst, src) in out.iter_mut().zip(rhs) {
        *dst += *src;
    }
}

#[inline]
fn scalar_sub_assign(out: &mut [f32], rhs: &[f32]) {
    for (dst, src) in out.iter_mut().zip(rhs) {
        *dst -= *src;
    }
}

#[inline]
fn scalar_scale_assign(out: &mut [f32], rhs: f32) {
    for dst in out {
        *dst *= rhs;
    }
}

#[inline]
fn scalar_dot_f32(lhs: &[f32], rhs: &[f32]) -> f32 {
    let mut sum = 0.0;
    for i in 0..lhs.len() {
        sum += lhs[i] * rhs[i];
    }
    sum
}

#[inline]
fn simd_dot_f32(lhs: &[f32], rhs: &[f32]) -> f32 {
    debug_assert_eq!(lhs.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if let Some(sum) = x86::try_dot_f32(lhs, rhs) {
        return sum;
    }
    #[cfg(target_arch = "aarch64")]
    if let Some(sum) = aarch64::try_dot_f32(lhs, rhs) {
        return sum;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if let Some(sum) = wasm32::try_dot_f32(lhs, rhs) {
        return sum;
    }
    scalar_dot_f32(lhs, rhs)
}

#[inline]
fn scalar_add_assign_generic<T>(out: &mut [T], rhs: &[T])
where
    T: Copy + AddAssign,
{
    for (dst, src) in out.iter_mut().zip(rhs) {
        *dst += *src;
    }
}

#[inline]
fn scalar_sub_assign_generic<T>(out: &mut [T], rhs: &[T])
where
    T: Copy + SubAssign,
{
    for (dst, src) in out.iter_mut().zip(rhs) {
        *dst -= *src;
    }
}

#[inline]
fn scalar_scale_assign_generic<T>(out: &mut [T], rhs: T)
where
    T: Copy + MulAssign,
{
    for dst in out {
        *dst *= rhs;
    }
}

#[inline]
fn scalar_div_assign_generic<T>(out: &mut [T], rhs: T)
where
    T: Copy + DivAssign,
{
    for dst in out {
        *dst /= rhs;
    }
}

#[inline]
fn scalar_shl_assign_generic<T>(out: &mut [T], rhs: u32)
where
    T: ShlAssign<u32>,
{
    for dst in out {
        *dst <<= rhs;
    }
}

#[inline]
fn scalar_shr_assign_generic<T>(out: &mut [T], rhs: u32)
where
    T: ShrAssign<u32>,
{
    for dst in out {
        *dst >>= rhs;
    }
}

#[inline]
fn simd_add_assign_f64(out: &mut [f64], rhs: &[f64]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_f64(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_f64(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_f64(out, rhs) {
        return;
    }
    scalar_add_assign_generic(out, rhs);
}

#[inline]
fn simd_sub_assign_f64(out: &mut [f64], rhs: &[f64]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_f64(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_f64(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_f64(out, rhs) {
        return;
    }
    scalar_sub_assign_generic(out, rhs);
}

#[inline]
fn simd_scale_assign_f64(out: &mut [f64], rhs: f64) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_scale_assign_f64(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_scale_assign_f64(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_scale_assign_f64(out, rhs) {
        return;
    }
    scalar_scale_assign_generic(out, rhs);
}

#[inline]
fn simd_add_assign_i32(out: &mut [i32], rhs: &[i32]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_i32(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_i32(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_i32(out, rhs) {
        return;
    }
    scalar_add_assign_generic(out, rhs);
}

#[inline]
fn simd_sub_assign_i32(out: &mut [i32], rhs: &[i32]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_i32(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_i32(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_i32(out, rhs) {
        return;
    }
    scalar_sub_assign_generic(out, rhs);
}

#[inline]
fn simd_scale_assign_i32(out: &mut [i32], rhs: i32) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_scale_assign_i32(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_scale_assign_i32(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_scale_assign_i32(out, rhs) {
        return;
    }
    scalar_scale_assign_generic(out, rhs);
}

#[inline]
fn simd_add_assign_i8(out: &mut [i8], rhs: &[i8]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_i8(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_i8(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_i8(out, rhs) {
        return;
    }
    scalar_add_assign_generic(out, rhs);
}

#[inline]
fn simd_sub_assign_i8(out: &mut [i8], rhs: &[i8]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_i8(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_i8(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_i8(out, rhs) {
        return;
    }
    scalar_sub_assign_generic(out, rhs);
}

#[inline]
fn simd_add_assign_i16(out: &mut [i16], rhs: &[i16]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_i16(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_i16(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_i16(out, rhs) {
        return;
    }
    scalar_add_assign_generic(out, rhs);
}

#[inline]
fn simd_sub_assign_i16(out: &mut [i16], rhs: &[i16]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_i16(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_i16(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_i16(out, rhs) {
        return;
    }
    scalar_sub_assign_generic(out, rhs);
}

#[inline]
fn simd_scale_assign_i16(out: &mut [i16], rhs: i16) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_scale_assign_i16(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_scale_assign_i16(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_scale_assign_i16(out, rhs) {
        return;
    }
    scalar_scale_assign_generic(out, rhs);
}

#[inline]
fn simd_add_assign_i64(out: &mut [i64], rhs: &[i64]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_i64(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_i64(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_i64(out, rhs) {
        return;
    }
    scalar_add_assign_generic(out, rhs);
}

#[inline]
fn simd_sub_assign_i64(out: &mut [i64], rhs: &[i64]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_i64(out, rhs) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_i64(out, rhs) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_i64(out, rhs) {
        return;
    }
    scalar_sub_assign_generic(out, rhs);
}

#[inline]
fn simd_add_assign_u32(out: &mut [u32], rhs: &[u32]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_i32(cast_u32_mut(out), cast_u32(rhs)) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_i32(cast_u32_mut(out), cast_u32(rhs)) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_i32(cast_u32_mut(out), cast_u32(rhs)) {
        return;
    }
    scalar_add_assign_generic(out, rhs);
}

#[inline]
fn simd_sub_assign_u32(out: &mut [u32], rhs: &[u32]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_i32(cast_u32_mut(out), cast_u32(rhs)) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_i32(cast_u32_mut(out), cast_u32(rhs)) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_i32(cast_u32_mut(out), cast_u32(rhs)) {
        return;
    }
    scalar_sub_assign_generic(out, rhs);
}

#[inline]
fn simd_scale_assign_u32(out: &mut [u32], rhs: u32) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_scale_assign_i32(cast_u32_mut(out), rhs as i32) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_scale_assign_i32(cast_u32_mut(out), rhs as i32) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_scale_assign_i32(cast_u32_mut(out), rhs as i32) {
        return;
    }
    scalar_scale_assign_generic(out, rhs);
}

#[inline]
fn simd_add_assign_u8(out: &mut [u8], rhs: &[u8]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_i8(cast_u8_mut(out), cast_u8(rhs)) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_i8(cast_u8_mut(out), cast_u8(rhs)) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_i8(cast_u8_mut(out), cast_u8(rhs)) {
        return;
    }
    scalar_add_assign_generic(out, rhs);
}

#[inline]
fn simd_sub_assign_u8(out: &mut [u8], rhs: &[u8]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_i8(cast_u8_mut(out), cast_u8(rhs)) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_i8(cast_u8_mut(out), cast_u8(rhs)) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_i8(cast_u8_mut(out), cast_u8(rhs)) {
        return;
    }
    scalar_sub_assign_generic(out, rhs);
}

#[inline]
fn simd_add_assign_u16(out: &mut [u16], rhs: &[u16]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_i16(cast_u16_mut(out), cast_u16(rhs)) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_i16(cast_u16_mut(out), cast_u16(rhs)) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_i16(cast_u16_mut(out), cast_u16(rhs)) {
        return;
    }
    scalar_add_assign_generic(out, rhs);
}

#[inline]
fn simd_sub_assign_u16(out: &mut [u16], rhs: &[u16]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_i16(cast_u16_mut(out), cast_u16(rhs)) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_i16(cast_u16_mut(out), cast_u16(rhs)) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_i16(cast_u16_mut(out), cast_u16(rhs)) {
        return;
    }
    scalar_sub_assign_generic(out, rhs);
}

#[inline]
fn simd_scale_assign_u16(out: &mut [u16], rhs: u16) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_scale_assign_i16(cast_u16_mut(out), rhs as i16) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_scale_assign_i16(cast_u16_mut(out), rhs as i16) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_scale_assign_i16(cast_u16_mut(out), rhs as i16) {
        return;
    }
    scalar_scale_assign_generic(out, rhs);
}

#[inline]
fn simd_add_assign_u64(out: &mut [u64], rhs: &[u64]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_add_assign_i64(cast_u64_mut(out), cast_u64(rhs)) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_add_assign_i64(cast_u64_mut(out), cast_u64(rhs)) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_add_assign_i64(cast_u64_mut(out), cast_u64(rhs)) {
        return;
    }
    scalar_add_assign_generic(out, rhs);
}

#[inline]
fn simd_sub_assign_u64(out: &mut [u64], rhs: &[u64]) {
    debug_assert_eq!(out.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_sub_assign_i64(cast_u64_mut(out), cast_u64(rhs)) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    if aarch64::try_sub_assign_i64(cast_u64_mut(out), cast_u64(rhs)) {
        return;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    if wasm32::try_sub_assign_i64(cast_u64_mut(out), cast_u64(rhs)) {
        return;
    }
    scalar_sub_assign_generic(out, rhs);
}

#[inline]
#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
fn cast_u8(value: &[u8]) -> &[i8] {
    // SAFETY: u8 and i8 have same size/alignment; length unchanged.
    unsafe { std::slice::from_raw_parts(value.as_ptr().cast(), value.len()) }
}

#[inline]
#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
fn cast_u8_mut(value: &mut [u8]) -> &mut [i8] {
    // SAFETY: u8 and i8 have same size/alignment; length unchanged.
    unsafe { std::slice::from_raw_parts_mut(value.as_mut_ptr().cast(), value.len()) }
}

#[inline]
#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
fn cast_u16(value: &[u16]) -> &[i16] {
    // SAFETY: u16 and i16 have same size/alignment; length unchanged.
    unsafe { std::slice::from_raw_parts(value.as_ptr().cast(), value.len()) }
}

#[inline]
#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
fn cast_u16_mut(value: &mut [u16]) -> &mut [i16] {
    // SAFETY: u16 and i16 have same size/alignment; length unchanged.
    unsafe { std::slice::from_raw_parts_mut(value.as_mut_ptr().cast(), value.len()) }
}

#[inline]
#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
fn cast_u32(value: &[u32]) -> &[i32] {
    // SAFETY: u32 and i32 have same size/alignment; length unchanged.
    unsafe { std::slice::from_raw_parts(value.as_ptr().cast(), value.len()) }
}

#[inline]
#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
fn cast_u32_mut(value: &mut [u32]) -> &mut [i32] {
    // SAFETY: u32 and i32 have same size/alignment; length unchanged.
    unsafe { std::slice::from_raw_parts_mut(value.as_mut_ptr().cast(), value.len()) }
}

#[inline]
#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
fn cast_u64(value: &[u64]) -> &[i64] {
    // SAFETY: u64 and i64 have same size/alignment; length unchanged.
    unsafe { std::slice::from_raw_parts(value.as_ptr().cast(), value.len()) }
}

#[inline]
#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
fn cast_u64_mut(value: &mut [u64]) -> &mut [i64] {
    // SAFETY: u64 and i64 have same size/alignment; length unchanged.
    unsafe { std::slice::from_raw_parts_mut(value.as_mut_ptr().cast(), value.len()) }
}

#[inline]
const fn static_assert_square<const ROWS: usize, const COLS: usize>() {
    assert!(ROWS == COLS, "matrix must be square");
}

#[inline]
fn matrix_rows_2<const ROWS: usize, const COLS: usize>(rows: [[f32; COLS]; ROWS]) -> [[f32; 2]; 2] {
    [[rows[0][0], rows[0][1]], [rows[1][0], rows[1][1]]]
}

#[inline]
fn matrix_rows_3<const ROWS: usize, const COLS: usize>(rows: [[f32; COLS]; ROWS]) -> [[f32; 3]; 3] {
    [
        [rows[0][0], rows[0][1], rows[0][2]],
        [rows[1][0], rows[1][1], rows[1][2]],
        [rows[2][0], rows[2][1], rows[2][2]],
    ]
}

#[inline]
fn matrix_rows_4<const ROWS: usize, const COLS: usize>(rows: [[f32; COLS]; ROWS]) -> [[f32; 4]; 4] {
    [
        [rows[0][0], rows[0][1], rows[0][2], rows[0][3]],
        [rows[1][0], rows[1][1], rows[1][2], rows[1][3]],
        [rows[2][0], rows[2][1], rows[2][2], rows[2][3]],
        [rows[3][0], rows[3][1], rows[3][2], rows[3][3]],
    ]
}

#[inline]
fn matrix_from_rows_2<const ROWS: usize, const COLS: usize>(
    rows: [[f32; 2]; 2],
) -> [[f32; COLS]; ROWS] {
    let mut out = [[0.0; COLS]; ROWS];
    out[0][0] = rows[0][0];
    out[0][1] = rows[0][1];
    out[1][0] = rows[1][0];
    out[1][1] = rows[1][1];
    out
}

#[inline]
fn matrix_from_rows_3<const ROWS: usize, const COLS: usize>(
    rows: [[f32; 3]; 3],
) -> [[f32; COLS]; ROWS] {
    let mut out = [[0.0; COLS]; ROWS];
    for r in 0..3 {
        for c in 0..3 {
            out[r][c] = rows[r][c];
        }
    }
    out
}

#[inline]
fn matrix_from_rows_4<const ROWS: usize, const COLS: usize>(
    rows: [[f32; 4]; 4],
) -> [[f32; COLS]; ROWS] {
    let mut out = [[0.0; COLS]; ROWS];
    for r in 0..4 {
        for c in 0..4 {
            out[r][c] = rows[r][c];
        }
    }
    out
}

#[cfg(test)]
mod tests {
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
        assert_eq!(Matrix::<2, 3, i32>::flat_index(1, 2), Some(5));
        assert_eq!(Matrix::<2, 3, i32>::flat_index(2, 0), None);
        assert_eq!(Matrix::<2, 3, i32>::row_col(4), Some((1, 1)));
        assert_eq!(Matrix::<2, 3, i32>::row_col(6), None);
        assert_eq!(matrix.get(1, 2), Some(&6));
        assert_eq!(matrix.get_flat(3), Some(&4));
        assert_eq!(matrix.find_position(&5), Some((1, 1)));
        assert_eq!(matrix.find_flat_index(&5), Some(4));

        assert!(matrix.set(0, 1, 20));
        assert!(!matrix.set(9, 9, 0));
        assert_eq!(matrix[(0, 1)], 20);

        *matrix.get_flat_mut(5).unwrap() = 60;
        assert_eq!(matrix[(1, 2)], 60);
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
            Matrix::<4, 2, i32>::new([[1, 2], [3, 4], [5, 6], [7, 8]]).mul_generic(Matrix::<
                2,
                3,
                i32,
            >::new(
                [
                [9, 10, 11],
                [12, 13, 14]
            ]
            )),
            Matrix::<4, 3, i32>::new([
                [33, 36, 39],
                [75, 82, 89],
                [117, 128, 139],
                [159, 174, 189]
            ])
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
}
