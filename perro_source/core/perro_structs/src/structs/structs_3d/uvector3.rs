use super::{IVector3, Vector3};
use glam::UVec3;
use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

/// A 3D vector with unsigned integer `x`, `y`, and `z` components.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct UVector3 {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

impl fmt::Display for UVector3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UVector3({}, {}, {})", self.x, self.y, self.z)
    }
}

impl UVector3 {
    /// Zero uvector3 constant (0, 0, 0)
    pub const ZERO: Self = Self { x: 0, y: 0, z: 0 };

    /// One uvector3 constant (1, 1, 1)
    pub const ONE: Self = Self { x: 1, y: 1, z: 1 };

    /// Creates a new 3D unsigned integer vector
    #[inline]
    pub const fn new(x: u32, y: u32, z: u32) -> Self {
        Self { x, y, z }
    }

    /// Returns components as an array.
    #[inline]
    pub const fn to_array(self) -> [u32; 3] {
        [self.x, self.y, self.z]
    }

    /// Returns components as a tuple.
    #[inline]
    pub const fn to_tuple(self) -> (u32, u32, u32) {
        (self.x, self.y, self.z)
    }

    /// Converts to `Vector3`.
    #[inline]
    pub fn as_vector3(self) -> Vector3 {
        Vector3::new(self.x as f32, self.y as f32, self.z as f32)
    }

    /// Converts to `IVector3` by clamping values above `i32::MAX`.
    #[inline]
    pub fn as_ivector3_saturating(self) -> IVector3 {
        IVector3::new(
            u32_to_i32_saturating(self.x),
            u32_to_i32_saturating(self.y),
            u32_to_i32_saturating(self.z),
        )
    }

    /// Dot product between this vector and another.
    #[inline]
    pub fn dot(self, rhs: Self) -> u64 {
        self.x as u64 * rhs.x as u64 + self.y as u64 * rhs.y as u64 + self.z as u64 * rhs.z as u64
    }

    /// Squared length.
    #[inline]
    pub fn length_squared(self) -> u64 {
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

    /// Returns a wrapping-negated copy.
    #[inline]
    pub fn negated(self) -> Self {
        Self::new(
            self.x.wrapping_neg(),
            self.y.wrapping_neg(),
            self.z.wrapping_neg(),
        )
    }

    /// Wrapping-negates this vector in place.
    #[inline]
    pub fn negate(&mut self) -> &mut Self {
        *self = self.negated();
        self
    }

    /// Returns a copy stepped toward `to` by at most `step` per component.
    #[inline]
    pub fn stepped(self, to: Self, step: u32) -> Self {
        Self::new(
            step_u32_toward(self.x, to.x, step),
            step_u32_toward(self.y, to.y, step),
            step_u32_toward(self.z, to.z, step),
        )
    }

    /// Steps this vector toward `to` by at most `step` per component in place.
    #[inline]
    pub fn step(&mut self, to: Self, step: u32) -> &mut Self {
        *self = self.stepped(to, step);
        self
    }
}

impl From<UVector3> for UVec3 {
    #[inline]
    fn from(v: UVector3) -> Self {
        UVec3::new(v.x, v.y, v.z)
    }
}

impl From<UVec3> for UVector3 {
    #[inline]
    fn from(v: UVec3) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

impl From<[u32; 3]> for UVector3 {
    #[inline]
    fn from(v: [u32; 3]) -> Self {
        Self::new(v[0], v[1], v[2])
    }
}

impl From<UVector3> for [u32; 3] {
    #[inline]
    fn from(v: UVector3) -> Self {
        v.to_array()
    }
}

impl From<(u32, u32, u32)> for UVector3 {
    #[inline]
    fn from(v: (u32, u32, u32)) -> Self {
        Self::new(v.0, v.1, v.2)
    }
}

impl From<UVector3> for (u32, u32, u32) {
    #[inline]
    fn from(v: UVector3) -> Self {
        v.to_tuple()
    }
}

impl From<UVector3> for Vector3 {
    #[inline]
    fn from(v: UVector3) -> Self {
        v.as_vector3()
    }
}

impl From<Vector3> for UVector3 {
    #[inline]
    fn from(v: Vector3) -> Self {
        v.as_uvector3_saturating()
    }
}

impl Add for UVector3 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

#[inline]
fn step_u32_toward(current: u32, target: u32, step: u32) -> u32 {
    if current < target {
        current.saturating_add(step).min(target)
    } else if current > target {
        current.saturating_sub(step).max(target)
    } else {
        current
    }
}

#[inline]
fn u32_to_i32_saturating(value: u32) -> i32 {
    value.min(i32::MAX as u32) as i32
}

impl AddAssign for UVector3 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
    }
}

impl Sub for UVector3 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl SubAssign for UVector3 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
    }
}

impl Mul for UVector3 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(self.x * rhs.x, self.y * rhs.y, self.z * rhs.z)
    }
}

impl MulAssign for UVector3 {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        self.x *= rhs.x;
        self.y *= rhs.y;
        self.z *= rhs.z;
    }
}

impl Mul<u32> for UVector3 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: u32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

impl MulAssign<u32> for UVector3 {
    #[inline]
    fn mul_assign(&mut self, rhs: u32) {
        self.x *= rhs;
        self.y *= rhs;
        self.z *= rhs;
    }
}

impl Div for UVector3 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        Self::new(self.x / rhs.x, self.y / rhs.y, self.z / rhs.z)
    }
}

impl DivAssign for UVector3 {
    #[inline]
    fn div_assign(&mut self, rhs: Self) {
        self.x /= rhs.x;
        self.y /= rhs.y;
        self.z /= rhs.z;
    }
}

impl Div<u32> for UVector3 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: u32) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs, self.z / rhs)
    }
}

impl DivAssign<u32> for UVector3 {
    #[inline]
    fn div_assign(&mut self, rhs: u32) {
        self.x /= rhs;
        self.y /= rhs;
        self.z /= rhs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uvector3_converts_to_vector3() {
        let v = UVector3::new(4, 9, 12);
        assert_eq!(v.to_array(), [4, 9, 12]);
        assert_eq!(v.to_tuple(), (4, 9, 12));
        assert_eq!(UVector3::from([1, 2, 3]), UVector3::new(1, 2, 3));
        assert_eq!(UVector3::from((3, 4, 5)), UVector3::new(3, 4, 5));
        assert_eq!(v.as_vector3(), Vector3::new(4.0, 9.0, 12.0));
        assert_eq!(Vector3::from(v), Vector3::new(4.0, 9.0, 12.0));
    }

    #[test]
    fn vector3_converts_to_uvector3_saturating() {
        assert_eq!(
            Vector3::new(4.9, -2.0, 7.1).as_uvector3_saturating(),
            UVector3::new(4, 0, 7)
        );
        assert_eq!(
            Vector3::new(f32::NAN, 8.2, f32::INFINITY).as_uvector3_saturating(),
            UVector3::new(0, 8, 0)
        );
        assert_eq!(
            UVector3::from(Vector3::new(3.7, 5.1, 9.9)),
            UVector3::new(3, 5, 9)
        );
    }

    #[test]
    fn vector3_round_modes_convert_to_uvector3() {
        let v = Vector3::new(2.2, 2.5, 2.8);
        assert_eq!(v.as_uvector3_floor(), UVector3::new(2, 2, 2));
        assert_eq!(v.as_uvector3_round(), UVector3::new(2, 3, 3));
        assert_eq!(v.as_uvector3_ceil(), UVector3::new(3, 3, 3));
    }

    #[test]
    fn uvector3_math_helpers() {
        let v = UVector3::new(2, 3, 4);
        assert_eq!(v.dot(UVector3::new(5, 6, 7)), 56);
        assert_eq!(v.length_squared(), 29);
        assert_eq!(v.min(UVector3::new(1, 4, 3)), UVector3::new(1, 3, 3));
        assert_eq!(v.max(UVector3::new(1, 4, 3)), UVector3::new(2, 4, 4));
        assert_eq!(
            v.clamp(UVector3::new(3, 1, 2), UVector3::new(5, 2, 6)),
            UVector3::new(3, 2, 4)
        );
    }

    #[test]
    fn uvector3_negate_matches_negated() {
        let mut v = UVector3::new(3, 0, 5);
        assert_eq!(v.negated(), UVector3::new(u32::MAX - 2, 0, u32::MAX - 4));
        assert_eq!(
            v.negate(),
            &mut UVector3::new(u32::MAX - 2, 0, u32::MAX - 4)
        );
        assert_eq!(v, UVector3::new(u32::MAX - 2, 0, u32::MAX - 4));
    }

    #[test]
    fn uvector3_steps_toward_target() {
        let mut v = UVector3::new(0, 10, 5);
        assert_eq!(v.stepped(UVector3::new(5, 4, 5), 3), UVector3::new(3, 7, 5));
        v.step(UVector3::new(5, 4, 9), 10);
        assert_eq!(v, UVector3::new(5, 4, 9));
    }

    #[test]
    fn uvector3_converts_to_ivector3_saturating() {
        assert_eq!(
            UVector3::new(u32::MAX, 9, 12).as_ivector3_saturating(),
            IVector3::new(i32::MAX, 9, 12)
        );
    }
}
