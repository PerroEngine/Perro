use super::{UVector3, Vector3};
use glam::IVec3;
use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// A 3D vector with signed integer `x`, `y`, and `z` components.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct IVector3 {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl fmt::Display for IVector3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "IVector3({}, {}, {})", self.x, self.y, self.z)
    }
}

impl IVector3 {
    /// Zero ivector3 constant (0, 0, 0)
    pub const ZERO: Self = Self { x: 0, y: 0, z: 0 };

    /// One ivector3 constant (1, 1, 1)
    pub const ONE: Self = Self { x: 1, y: 1, z: 1 };

    /// Negative one ivector3 constant (-1, -1, -1)
    pub const NEG_ONE: Self = Self {
        x: -1,
        y: -1,
        z: -1,
    };

    /// Creates a new 3D signed integer vector
    #[inline]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Returns components as an array.
    #[inline]
    pub const fn to_array(self) -> [i32; 3] {
        [self.x, self.y, self.z]
    }

    /// Returns components as a tuple.
    #[inline]
    pub const fn to_tuple(self) -> (i32, i32, i32) {
        (self.x, self.y, self.z)
    }

    /// Converts to `Vector3`.
    #[inline]
    pub fn as_vector3(self) -> Vector3 {
        Vector3::new(self.x as f32, self.y as f32, self.z as f32)
    }

    /// Converts to `UVector3` by clamping negative values to zero.
    #[inline]
    pub fn as_uvector3_saturating(self) -> UVector3 {
        UVector3::new(
            i32_to_u32_saturating(self.x),
            i32_to_u32_saturating(self.y),
            i32_to_u32_saturating(self.z),
        )
    }

    /// Dot product between this vector and another.
    #[inline]
    pub fn dot(self, rhs: Self) -> i64 {
        self.x as i64 * rhs.x as i64 + self.y as i64 * rhs.y as i64 + self.z as i64 * rhs.z as i64
    }

    /// Squared length.
    #[inline]
    pub fn length_squared(self) -> i64 {
        self.dot(self)
    }

    /// Component-wise minimum.
    #[inline]
    pub fn min(self, rhs: Self) -> Self {
        Self::new(self.x.min(rhs.x), self.y.min(rhs.y), self.z.min(rhs.z))
    }

    /// Component-wise maximum.
    #[inline]
    pub fn max(self, rhs: Self) -> Self {
        Self::new(self.x.max(rhs.x), self.y.max(rhs.y), self.z.max(rhs.z))
    }

    /// Component-wise clamp.
    #[inline]
    pub fn clamp(self, min: Self, max: Self) -> Self {
        Self::new(
            self.x.clamp(min.x, max.x),
            self.y.clamp(min.y, max.y),
            self.z.clamp(min.z, max.z),
        )
    }

    /// Component-wise absolute value, saturating `i32::MIN` to `i32::MAX`.
    #[inline]
    pub fn abs(self) -> Self {
        Self::new(
            self.x.saturating_abs(),
            self.y.saturating_abs(),
            self.z.saturating_abs(),
        )
    }

    /// Component-wise sign (-1, 0, or 1).
    #[inline]
    pub fn signum(self) -> Self {
        Self::new(self.x.signum(), self.y.signum(), self.z.signum())
    }

    /// Returns a copy stepped toward `to` by at most `step` per component.
    #[inline]
    pub fn stepped(self, to: Self, step: u32) -> Self {
        Self::new(
            step_i32_toward(self.x, to.x, step),
            step_i32_toward(self.y, to.y, step),
            step_i32_toward(self.z, to.z, step),
        )
    }

    /// Steps this vector toward `to` by at most `step` per component in place.
    #[inline]
    pub fn step(&mut self, to: Self, step: u32) -> &mut Self {
        *self = self.stepped(to, step);
        self
    }
}

impl From<IVector3> for IVec3 {
    #[inline]
    fn from(v: IVector3) -> Self {
        IVec3::new(v.x, v.y, v.z)
    }
}

impl From<IVec3> for IVector3 {
    #[inline]
    fn from(v: IVec3) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

impl From<[i32; 3]> for IVector3 {
    #[inline]
    fn from(v: [i32; 3]) -> Self {
        Self::new(v[0], v[1], v[2])
    }
}

impl From<IVector3> for [i32; 3] {
    #[inline]
    fn from(v: IVector3) -> Self {
        v.to_array()
    }
}

impl From<(i32, i32, i32)> for IVector3 {
    #[inline]
    fn from(v: (i32, i32, i32)) -> Self {
        Self::new(v.0, v.1, v.2)
    }
}

impl From<IVector3> for (i32, i32, i32) {
    #[inline]
    fn from(v: IVector3) -> Self {
        v.to_tuple()
    }
}

impl From<IVector3> for Vector3 {
    #[inline]
    fn from(v: IVector3) -> Self {
        v.as_vector3()
    }
}

impl From<Vector3> for IVector3 {
    #[inline]
    fn from(v: Vector3) -> Self {
        v.as_ivector3_saturating()
    }
}

impl From<IVector3> for UVector3 {
    #[inline]
    fn from(v: IVector3) -> Self {
        v.as_uvector3_saturating()
    }
}

impl From<UVector3> for IVector3 {
    #[inline]
    fn from(v: UVector3) -> Self {
        v.as_ivector3_saturating()
    }
}

impl Add for IVector3 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl AddAssign for IVector3 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
    }
}

impl Sub for IVector3 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl SubAssign for IVector3 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
    }
}

impl Mul for IVector3 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(self.x * rhs.x, self.y * rhs.y, self.z * rhs.z)
    }
}

impl MulAssign for IVector3 {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        self.x *= rhs.x;
        self.y *= rhs.y;
        self.z *= rhs.z;
    }
}

impl Mul<i32> for IVector3 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: i32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

impl MulAssign<i32> for IVector3 {
    #[inline]
    fn mul_assign(&mut self, rhs: i32) {
        self.x *= rhs;
        self.y *= rhs;
        self.z *= rhs;
    }
}

impl Div for IVector3 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        Self::new(self.x / rhs.x, self.y / rhs.y, self.z / rhs.z)
    }
}

impl DivAssign for IVector3 {
    #[inline]
    fn div_assign(&mut self, rhs: Self) {
        self.x /= rhs.x;
        self.y /= rhs.y;
        self.z /= rhs.z;
    }
}

impl Div<i32> for IVector3 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: i32) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs, self.z / rhs)
    }
}

impl DivAssign<i32> for IVector3 {
    #[inline]
    fn div_assign(&mut self, rhs: i32) {
        self.x /= rhs;
        self.y /= rhs;
        self.z /= rhs;
    }
}

impl Neg for IVector3 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y, -self.z)
    }
}

#[inline]
fn i32_to_u32_saturating(value: i32) -> u32 {
    if value <= 0 { 0 } else { value as u32 }
}

#[inline]
fn step_i32_toward(current: i32, target: i32, step: u32) -> i32 {
    if current < target {
        current
            .saturating_add(step.min(i32::MAX as u32) as i32)
            .min(target)
    } else if current > target {
        current
            .saturating_sub(step.min(i32::MAX as u32) as i32)
            .max(target)
    } else {
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ivector3_converts_to_vector3_and_uvector3() {
        let v = IVector3::new(-4, 9, -12);
        assert_eq!(v.to_array(), [-4, 9, -12]);
        assert_eq!(v.to_tuple(), (-4, 9, -12));
        assert_eq!(IVector3::from([-1, 2, -3]), IVector3::new(-1, 2, -3));
        assert_eq!(IVector3::from((-3, 4, 5)), IVector3::new(-3, 4, 5));
        assert_eq!(v.as_vector3(), Vector3::new(-4.0, 9.0, -12.0));
        assert_eq!(v.as_uvector3_saturating(), UVector3::new(0, 9, 0));
    }

    #[test]
    fn ivector3_math_helpers() {
        let v = IVector3::new(-2, 3, -4);
        assert_eq!(v.dot(IVector3::new(5, -6, 7)), -56);
        assert_eq!(v.length_squared(), 29);
        assert_eq!(v.min(IVector3::new(-1, 4, -3)), IVector3::new(-2, 3, -4));
        assert_eq!(v.max(IVector3::new(-1, 4, -3)), IVector3::new(-1, 4, -3));
        assert_eq!(
            v.clamp(IVector3::new(-3, 1, -2), IVector3::new(5, 2, 6)),
            IVector3::new(-2, 2, -2)
        );
        assert_eq!(v.abs(), IVector3::new(2, 3, 4));
        assert_eq!(v.signum(), IVector3::new(-1, 1, -1));
    }

    #[test]
    fn ivector3_steps_toward_target() {
        let mut v = IVector3::new(-10, 10, 5);
        assert_eq!(
            v.stepped(IVector3::new(5, 4, 5), 3),
            IVector3::new(-7, 7, 5)
        );
        v.step(IVector3::new(5, 4, 9), 20);
        assert_eq!(v, IVector3::new(5, 4, 9));
    }
}
