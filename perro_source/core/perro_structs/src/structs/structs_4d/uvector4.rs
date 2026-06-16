use super::{IVector4, Vector4};
use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct UVector4 {
    pub x: u32,
    pub y: u32,
    pub z: u32,
    pub w: u32,
}

impl fmt::Display for UVector4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UVector4({}, {}, {}, {})",
            self.x, self.y, self.z, self.w
        )
    }
}

impl UVector4 {
    pub const ZERO: Self = Self::new(0, 0, 0, 0);
    pub const ONE: Self = Self::new(1, 1, 1, 1);

    #[inline]
    pub const fn new(x: u32, y: u32, z: u32, w: u32) -> Self {
        Self { x, y, z, w }
    }

    #[inline]
    pub const fn to_array(self) -> [u32; 4] {
        [self.x, self.y, self.z, self.w]
    }

    #[inline]
    pub const fn to_tuple(self) -> (u32, u32, u32, u32) {
        (self.x, self.y, self.z, self.w)
    }

    #[inline]
    pub fn as_vector4(self) -> Vector4 {
        Vector4::new(self.x as f32, self.y as f32, self.z as f32, self.w as f32)
    }

    #[inline]
    pub fn as_ivector4_saturating(self) -> IVector4 {
        IVector4::new(
            u32_to_i32_saturating(self.x),
            u32_to_i32_saturating(self.y),
            u32_to_i32_saturating(self.z),
            u32_to_i32_saturating(self.w),
        )
    }

    #[inline]
    pub fn dot(self, rhs: Self) -> u64 {
        self.x as u64 * rhs.x as u64
            + self.y as u64 * rhs.y as u64
            + self.z as u64 * rhs.z as u64
            + self.w as u64 * rhs.w as u64
    }

    #[inline]
    pub fn length_squared(self) -> u64 {
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
}

impl From<[u32; 4]> for UVector4 {
    #[inline]
    fn from(v: [u32; 4]) -> Self {
        Self::new(v[0], v[1], v[2], v[3])
    }
}

impl From<UVector4> for [u32; 4] {
    #[inline]
    fn from(v: UVector4) -> Self {
        v.to_array()
    }
}

impl From<UVector4> for Vector4 {
    #[inline]
    fn from(v: UVector4) -> Self {
        v.as_vector4()
    }
}

impl From<Vector4> for UVector4 {
    #[inline]
    fn from(v: Vector4) -> Self {
        v.as_uvector4_saturating()
    }
}

macro_rules! impl_uvec4_op {
    ($trait:ident, $fn:ident, $op:tt) => {
        impl $trait for UVector4 {
            type Output = Self;
            #[inline]
            fn $fn(self, rhs: Self) -> Self::Output {
                Self::new(self.x $op rhs.x, self.y $op rhs.y, self.z $op rhs.z, self.w $op rhs.w)
            }
        }
    };
}

impl_uvec4_op!(Add, add, +);
impl_uvec4_op!(Sub, sub, -);
impl_uvec4_op!(Mul, mul, *);
impl_uvec4_op!(Div, div, /);

impl AddAssign for UVector4 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}
impl SubAssign for UVector4 {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}
impl MulAssign for UVector4 {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}
impl DivAssign for UVector4 {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}

#[inline]
fn u32_to_i32_saturating(value: u32) -> i32 {
    value.min(i32::MAX as u32) as i32
}
