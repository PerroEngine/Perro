use super::{IVector2, Vector2};
use glam::UVec2;
use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

/// A 2D vector with unsigned integer `x` and `y` components.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct UVector2 {
    pub x: u32,
    pub y: u32,
}

impl fmt::Display for UVector2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UVector2({}, {})", self.x, self.y)
    }
}

impl UVector2 {
    /// Zero uvector2 constant (0, 0)
    pub const ZERO: Self = Self { x: 0, y: 0 };

    /// One uvector2 constant (1, 1)
    pub const ONE: Self = Self { x: 1, y: 1 };

    /// Creates a new 2D unsigned integer vector
    #[inline]
    pub const fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }

    /// Returns components as an array.
    #[inline]
    pub const fn to_array(self) -> [u32; 2] {
        [self.x, self.y]
    }

    /// Returns components as a tuple.
    #[inline]
    pub const fn to_tuple(self) -> (u32, u32) {
        (self.x, self.y)
    }

    /// Converts to `Vector2`.
    #[inline]
    pub fn as_vector2(self) -> Vector2 {
        Vector2::new(self.x as f32, self.y as f32)
    }

    /// Converts to `IVector2` by clamping values above `i32::MAX`.
    #[inline]
    pub fn as_ivector2_saturating(self) -> IVector2 {
        IVector2::new(u32_to_i32_saturating(self.x), u32_to_i32_saturating(self.y))
    }

    /// Dot product between this vector and another.
    #[inline]
    pub fn dot(self, rhs: Self) -> u64 {
        self.x as u64 * rhs.x as u64 + self.y as u64 * rhs.y as u64
    }

    /// Squared length.
    #[inline]
    pub fn length_squared(self) -> u64 {
        self.dot(self)
    }

    /// Component-wise minimum.
    #[inline]
    pub fn min(self, rhs: Self) -> Self {
        Self::new(self.x.min(rhs.x), self.y.min(rhs.y))
    }

    /// Component-wise maximum.
    #[inline]
    pub fn max(self, rhs: Self) -> Self {
        Self::new(self.x.max(rhs.x), self.y.max(rhs.y))
    }

    /// Component-wise clamp.
    #[inline]
    pub fn clamp(self, min: Self, max: Self) -> Self {
        Self::new(self.x.clamp(min.x, max.x), self.y.clamp(min.y, max.y))
    }

    /// Returns a wrapping-negated copy.
    #[inline]
    pub fn negated(self) -> Self {
        Self::new(self.x.wrapping_neg(), self.y.wrapping_neg())
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
        )
    }

    /// Steps this vector toward `to` by at most `step` per component in place.
    #[inline]
    pub fn step(&mut self, to: Self, step: u32) -> &mut Self {
        *self = self.stepped(to, step);
        self
    }
}

impl From<UVector2> for UVec2 {
    #[inline]
    fn from(v: UVector2) -> Self {
        UVec2::new(v.x, v.y)
    }
}

impl From<UVec2> for UVector2 {
    #[inline]
    fn from(v: UVec2) -> Self {
        Self { x: v.x, y: v.y }
    }
}

impl From<[u32; 2]> for UVector2 {
    #[inline]
    fn from(v: [u32; 2]) -> Self {
        Self::new(v[0], v[1])
    }
}

impl From<UVector2> for [u32; 2] {
    #[inline]
    fn from(v: UVector2) -> Self {
        v.to_array()
    }
}

impl From<(u32, u32)> for UVector2 {
    #[inline]
    fn from(v: (u32, u32)) -> Self {
        Self::new(v.0, v.1)
    }
}

impl From<UVector2> for (u32, u32) {
    #[inline]
    fn from(v: UVector2) -> Self {
        v.to_tuple()
    }
}

impl From<UVector2> for Vector2 {
    #[inline]
    fn from(v: UVector2) -> Self {
        v.as_vector2()
    }
}

impl From<Vector2> for UVector2 {
    #[inline]
    fn from(v: Vector2) -> Self {
        v.as_uvector2_saturating()
    }
}

impl Add for UVector2 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
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

impl AddAssign for UVector2 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub for UVector2 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl SubAssign for UVector2 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl Mul for UVector2 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(self.x * rhs.x, self.y * rhs.y)
    }
}

impl MulAssign for UVector2 {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        self.x *= rhs.x;
        self.y *= rhs.y;
    }
}

impl Mul<u32> for UVector2 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: u32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl MulAssign<u32> for UVector2 {
    #[inline]
    fn mul_assign(&mut self, rhs: u32) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

impl Div for UVector2 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        Self::new(self.x / rhs.x, self.y / rhs.y)
    }
}

impl DivAssign for UVector2 {
    #[inline]
    fn div_assign(&mut self, rhs: Self) {
        self.x /= rhs.x;
        self.y /= rhs.y;
    }
}

impl Div<u32> for UVector2 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: u32) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs)
    }
}

impl DivAssign<u32> for UVector2 {
    #[inline]
    fn div_assign(&mut self, rhs: u32) {
        self.x /= rhs;
        self.y /= rhs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uvector2_converts_to_vector2() {
        let v = UVector2::new(4, 9);
        assert_eq!(v.to_array(), [4, 9]);
        assert_eq!(v.to_tuple(), (4, 9));
        assert_eq!(UVector2::from([1, 2]), UVector2::new(1, 2));
        assert_eq!(UVector2::from((3, 4)), UVector2::new(3, 4));
        assert_eq!(v.as_vector2(), Vector2::new(4.0, 9.0));
        assert_eq!(Vector2::from(v), Vector2::new(4.0, 9.0));
    }

    #[test]
    fn vector2_converts_to_uvector2_saturating() {
        assert_eq!(
            Vector2::new(4.9, -2.0).as_uvector2_saturating(),
            UVector2::new(4, 0)
        );
        assert_eq!(
            Vector2::new(f32::INFINITY, 8.2).as_uvector2_saturating(),
            UVector2::new(0, 8)
        );
        assert_eq!(UVector2::from(Vector2::new(3.7, 5.1)), UVector2::new(3, 5));
    }

    #[test]
    fn vector2_round_modes_convert_to_uvector2() {
        let v = Vector2::new(2.2, 2.8);
        assert_eq!(v.as_uvector2_floor(), UVector2::new(2, 2));
        assert_eq!(v.as_uvector2_round(), UVector2::new(2, 3));
        assert_eq!(v.as_uvector2_ceil(), UVector2::new(3, 3));
    }

    #[test]
    fn uvector2_math_helpers() {
        let v = UVector2::new(3, 4);
        assert_eq!(v.dot(UVector2::new(2, 5)), 26);
        assert_eq!(v.length_squared(), 25);
        assert_eq!(v.min(UVector2::new(4, 2)), UVector2::new(3, 2));
        assert_eq!(v.max(UVector2::new(4, 2)), UVector2::new(4, 4));
        assert_eq!(
            v.clamp(UVector2::new(4, 1), UVector2::new(6, 3)),
            UVector2::new(4, 3)
        );
    }

    #[test]
    fn uvector2_negate_matches_negated() {
        let mut v = UVector2::new(3, 0);
        assert_eq!(v.negated(), UVector2::new(u32::MAX - 2, 0));
        assert_eq!(v.negate(), &mut UVector2::new(u32::MAX - 2, 0));
        assert_eq!(v, UVector2::new(u32::MAX - 2, 0));
    }

    #[test]
    fn uvector2_steps_toward_target() {
        let mut v = UVector2::new(0, 10);
        assert_eq!(v.stepped(UVector2::new(5, 4), 3), UVector2::new(3, 7));
        v.step(UVector2::new(5, 4), 10);
        assert_eq!(v, UVector2::new(5, 4));
    }

    #[test]
    fn uvector2_converts_to_ivector2_saturating() {
        assert_eq!(
            UVector2::new(u32::MAX, 9).as_ivector2_saturating(),
            IVector2::new(i32::MAX, 9)
        );
    }
}
