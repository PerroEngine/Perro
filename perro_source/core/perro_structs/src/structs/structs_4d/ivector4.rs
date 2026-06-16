use super::{UVector4, Vector4};
use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct IVector4 {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub w: i32,
}

impl fmt::Display for IVector4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "IVector4({}, {}, {}, {})",
            self.x, self.y, self.z, self.w
        )
    }
}

impl IVector4 {
    pub const ZERO: Self = Self::new(0, 0, 0, 0);
    pub const ONE: Self = Self::new(1, 1, 1, 1);
    pub const NEG_ONE: Self = Self::new(-1, -1, -1, -1);

    #[inline]
    pub const fn new(x: i32, y: i32, z: i32, w: i32) -> Self {
        Self { x, y, z, w }
    }

    #[inline]
    pub const fn to_array(self) -> [i32; 4] {
        [self.x, self.y, self.z, self.w]
    }

    #[inline]
    pub const fn to_tuple(self) -> (i32, i32, i32, i32) {
        (self.x, self.y, self.z, self.w)
    }

    #[inline]
    pub fn as_vector4(self) -> Vector4 {
        Vector4::new(self.x as f32, self.y as f32, self.z as f32, self.w as f32)
    }

    #[inline]
    pub fn as_uvector4_saturating(self) -> UVector4 {
        UVector4::new(
            i32_to_u32_saturating(self.x),
            i32_to_u32_saturating(self.y),
            i32_to_u32_saturating(self.z),
            i32_to_u32_saturating(self.w),
        )
    }

    #[inline]
    pub fn dot(self, rhs: Self) -> i64 {
        self.x as i64 * rhs.x as i64
            + self.y as i64 * rhs.y as i64
            + self.z as i64 * rhs.z as i64
            + self.w as i64 * rhs.w as i64
    }

    #[inline]
    pub fn length_squared(self) -> i64 {
        self.dot(self)
    }

    #[inline]
    pub fn min(self, rhs: Self) -> Self {
        Self::new(
            self.x.min(rhs.x),
            self.y.min(rhs.y),
            self.z.min(rhs.z),
            self.w.min(rhs.w),
        )
    }

    #[inline]
    pub fn max(self, rhs: Self) -> Self {
        Self::new(
            self.x.max(rhs.x),
            self.y.max(rhs.y),
            self.z.max(rhs.z),
            self.w.max(rhs.w),
        )
    }

    #[inline]
    pub fn clamp(self, min: Self, max: Self) -> Self {
        Self::new(
            self.x.clamp(min.x, max.x),
            self.y.clamp(min.y, max.y),
            self.z.clamp(min.z, max.z),
            self.w.clamp(min.w, max.w),
        )
    }

    #[inline]
    pub fn negated(self) -> Self {
        Self::new(
            self.x.saturating_neg(),
            self.y.saturating_neg(),
            self.z.saturating_neg(),
            self.w.saturating_neg(),
        )
    }
}

impl From<[i32; 4]> for IVector4 {
    #[inline]
    fn from(v: [i32; 4]) -> Self {
        Self::new(v[0], v[1], v[2], v[3])
    }
}

impl From<IVector4> for [i32; 4] {
    #[inline]
    fn from(v: IVector4) -> Self {
        v.to_array()
    }
}

impl From<IVector4> for Vector4 {
    #[inline]
    fn from(v: IVector4) -> Self {
        v.as_vector4()
    }
}

impl From<Vector4> for IVector4 {
    #[inline]
    fn from(v: Vector4) -> Self {
        v.as_ivector4_saturating()
    }
}

macro_rules! impl_ivec4_op {
    ($trait:ident, $fn:ident, $op:tt) => {
        impl $trait for IVector4 {
            type Output = Self;
            #[inline]
            fn $fn(self, rhs: Self) -> Self::Output {
                Self::new(self.x $op rhs.x, self.y $op rhs.y, self.z $op rhs.z, self.w $op rhs.w)
            }
        }
    };
}

impl_ivec4_op!(Add, add, +);
impl_ivec4_op!(Sub, sub, -);
impl_ivec4_op!(Mul, mul, *);
impl_ivec4_op!(Div, div, /);

impl AddAssign for IVector4 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}
impl SubAssign for IVector4 {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}
impl MulAssign for IVector4 {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}
impl DivAssign for IVector4 {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}

impl Neg for IVector4 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y, -self.z, -self.w)
    }
}

#[inline]
fn i32_to_u32_saturating(value: i32) -> u32 {
    if value <= 0 { 0 } else { value as u32 }
}
