use super::*;

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
        scalar_div_assign_generic(self.as_mut_slice(), rhs);
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
