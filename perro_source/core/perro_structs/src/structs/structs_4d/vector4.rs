use super::{IVector4, UVector4};
use crate::Quaternion;
use glam::Vec4;
use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vector4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl fmt::Display for Vector4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Vector4({}, {}, {}, {})", self.x, self.y, self.z, self.w)
    }
}

impl Vector4 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0, 0.0);
    pub const HALF: Self = Self::new(0.5, 0.5, 0.5, 0.5);
    pub const ONE: Self = Self::new(1.0, 1.0, 1.0, 1.0);

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    #[inline]
    pub const fn to_array(self) -> [f32; 4] {
        [self.x, self.y, self.z, self.w]
    }

    #[inline]
    pub const fn to_tuple(self) -> (f32, f32, f32, f32) {
        (self.x, self.y, self.z, self.w)
    }

    #[inline]
    pub const fn as_quaternion(self) -> Quaternion {
        Quaternion::new(self.x, self.y, self.z, self.w)
    }

    #[inline]
    pub fn as_uvector4_saturating(self) -> UVector4 {
        UVector4::new(
            f32_to_u32_saturating(self.x),
            f32_to_u32_saturating(self.y),
            f32_to_u32_saturating(self.z),
            f32_to_u32_saturating(self.w),
        )
    }

    #[inline]
    pub fn as_ivector4_saturating(self) -> IVector4 {
        IVector4::new(
            f32_to_i32_saturating(self.x),
            f32_to_i32_saturating(self.y),
            f32_to_i32_saturating(self.z),
            f32_to_i32_saturating(self.w),
        )
    }

    #[inline]
    fn to_glam(self) -> Vec4 {
        Vec4::new(self.x, self.y, self.z, self.w)
    }

    #[inline]
    fn from_glam(v: Vec4) -> Self {
        Self::new(v.x, v.y, v.z, v.w)
    }

    #[inline]
    pub fn dot(self, rhs: Self) -> f32 {
        self.to_glam().dot(rhs.to_glam())
    }

    #[inline]
    pub fn length_squared(self) -> f32 {
        self.to_glam().length_squared()
    }

    #[inline]
    pub fn length(self) -> f32 {
        self.to_glam().length()
    }

    #[inline]
    pub fn normalized(self) -> Self {
        Self::from_glam(self.to_glam().normalize_or_zero())
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
        -self
    }
}

impl From<[f32; 4]> for Vector4 {
    #[inline]
    fn from(v: [f32; 4]) -> Self {
        Self::new(v[0], v[1], v[2], v[3])
    }
}

impl From<Vector4> for [f32; 4] {
    #[inline]
    fn from(v: Vector4) -> Self {
        v.to_array()
    }
}

impl From<(f32, f32, f32, f32)> for Vector4 {
    #[inline]
    fn from(v: (f32, f32, f32, f32)) -> Self {
        Self::new(v.0, v.1, v.2, v.3)
    }
}

impl From<Vector4> for (f32, f32, f32, f32) {
    #[inline]
    fn from(v: Vector4) -> Self {
        v.to_tuple()
    }
}

impl From<Quaternion> for Vector4 {
    #[inline]
    fn from(v: Quaternion) -> Self {
        Self::new(v.x, v.y, v.z, v.w)
    }
}

impl From<Vector4> for Quaternion {
    #[inline]
    fn from(v: Vector4) -> Self {
        v.as_quaternion()
    }
}

macro_rules! impl_vec4_op {
    ($trait:ident, $fn:ident, $op:tt) => {
        impl $trait for Vector4 {
            type Output = Self;
            #[inline]
            fn $fn(self, rhs: Self) -> Self::Output {
                Self::new(self.x $op rhs.x, self.y $op rhs.y, self.z $op rhs.z, self.w $op rhs.w)
            }
        }
    };
}

impl_vec4_op!(Add, add, +);
impl_vec4_op!(Sub, sub, -);
impl_vec4_op!(Mul, mul, *);
impl_vec4_op!(Div, div, /);

impl AddAssign for Vector4 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for Vector4 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl MulAssign for Vector4 {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl DivAssign for Vector4 {
    #[inline]
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}

impl Mul<f32> for Vector4 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs, self.w * rhs)
    }
}

impl Div<f32> for Vector4 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs, self.z / rhs, self.w / rhs)
    }
}

impl Neg for Vector4 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y, -self.z, -self.w)
    }
}

#[inline]
fn f32_to_u32_saturating(value: f32) -> u32 {
    if !value.is_finite() || value <= 0.0 {
        0
    } else if value >= u32::MAX as f32 {
        u32::MAX
    } else {
        value as u32
    }
}

#[inline]
fn f32_to_i32_saturating(value: f32) -> i32 {
    if value.is_nan() {
        0
    } else if value <= i32::MIN as f32 {
        i32::MIN
    } else if value >= i32::MAX as f32 {
        i32::MAX
    } else {
        value as i32
    }
}
