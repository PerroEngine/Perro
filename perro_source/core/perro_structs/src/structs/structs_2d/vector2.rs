use super::{IVector2, UVector2};
use glam::Vec2;
use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// A 2D vector with `x` and `y` components.
///
/// # Example
///
/// ```rust
/// use perro_structs::Vector2;
///
/// let a = Vector2::new(0.0, 0.0);
/// let b = Vector2::new(3.0, 4.0);
/// assert_eq!(a.distance_to(b), 5.0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

impl fmt::Display for Vector2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Vector2({}, {})", self.x, self.y)
    }
}

impl Vector2 {
    /// Zero vector2 constant (0, 0)
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Half vector2 constant (0.5, 0.5)
    pub const HALF: Self = Self { x: 0.5, y: 0.5 };

    /// One vector2 constant (1, 1)
    pub const ONE: Self = Self { x: 1.0, y: 1.0 };

    /// Creates a new 2D vector
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Returns components as an array.
    #[inline]
    pub const fn to_array(self) -> [f32; 2] {
        [self.x, self.y]
    }

    /// Returns components as a tuple.
    #[inline]
    pub const fn to_tuple(self) -> (f32, f32) {
        (self.x, self.y)
    }

    /// Converts to `UVector2` by clamping negatives/non-finite values to zero and truncating decimals.
    #[inline]
    pub fn as_uvector2_saturating(self) -> UVector2 {
        UVector2::new(f32_to_u32_saturating(self.x), f32_to_u32_saturating(self.y))
    }

    /// Converts to `UVector2` by flooring each component first.
    #[inline]
    pub fn as_uvector2_floor(self) -> UVector2 {
        UVector2::new(
            f32_to_u32_saturating(self.x.floor()),
            f32_to_u32_saturating(self.y.floor()),
        )
    }

    /// Converts to `UVector2` by rounding each component first.
    #[inline]
    pub fn as_uvector2_round(self) -> UVector2 {
        UVector2::new(
            f32_to_u32_saturating(self.x.round()),
            f32_to_u32_saturating(self.y.round()),
        )
    }

    /// Converts to `UVector2` by ceiling each component first.
    #[inline]
    pub fn as_uvector2_ceil(self) -> UVector2 {
        UVector2::new(
            f32_to_u32_saturating(self.x.ceil()),
            f32_to_u32_saturating(self.y.ceil()),
        )
    }

    /// Converts to `IVector2` by clamping non-finite/out-of-range values and truncating decimals.
    #[inline]
    pub fn as_ivector2_saturating(self) -> IVector2 {
        IVector2::new(f32_to_i32_saturating(self.x), f32_to_i32_saturating(self.y))
    }

    /// Converts to `IVector2` by flooring each component first.
    #[inline]
    pub fn as_ivector2_floor(self) -> IVector2 {
        IVector2::new(
            f32_to_i32_saturating(self.x.floor()),
            f32_to_i32_saturating(self.y.floor()),
        )
    }

    /// Converts to `IVector2` by rounding each component first.
    #[inline]
    pub fn as_ivector2_round(self) -> IVector2 {
        IVector2::new(
            f32_to_i32_saturating(self.x.round()),
            f32_to_i32_saturating(self.y.round()),
        )
    }

    /// Converts to `IVector2` by ceiling each component first.
    #[inline]
    pub fn as_ivector2_ceil(self) -> IVector2 {
        IVector2::new(
            f32_to_i32_saturating(self.x.ceil()),
            f32_to_i32_saturating(self.y.ceil()),
        )
    }

    // Helper to convert to glam for operations
    #[inline(always)]
    const fn to_glam(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    // Helper to create from glam
    #[inline(always)]
    const fn from_glam(v: Vec2) -> Self {
        Self { x: v.x, y: v.y }
    }

    // ------------------ Math Ops ------------------

    /// Dot product between this vector and another
    #[inline]
    pub fn dot(self, rhs: Self) -> f32 {
        self.to_glam().dot(rhs.to_glam())
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

    /// 2D cross product (signed Z magnitude).
    #[inline]
    pub fn cross(self, rhs: Self) -> f32 {
        self.x * rhs.y - self.y * rhs.x
    }

    /// Squared length (avoids a sqrt when only comparing distances)
    #[inline]
    pub fn length_squared(&self) -> f32 {
        self.to_glam().length_squared()
    }

    /// Magnitude (length) of the vector
    #[inline]
    pub fn length(&self) -> f32 {
        self.to_glam().length()
    }

    /// Returns a new `Vector2` with length = 1 (same direction)
    #[inline]
    pub fn normalized(&self) -> Self {
        Self::from_glam(self.to_glam().normalize_or_zero())
    }

    /// Distance between two vectors.
    ///
    /// # Example
    ///
    /// ```rust
    /// use perro_structs::Vector2;
    ///
    /// let d = Vector2::distance(Vector2::new(0.0, 0.0), Vector2::new(3.0, 4.0));
    /// assert_eq!(d, 5.0);
    /// ```
    #[inline]
    pub fn distance(a: Self, b: Self) -> f32 {
        a.to_glam().distance(b.to_glam())
    }

    /// Distance from this vector to another vector.
    ///
    /// # Example
    ///
    /// ```rust
    /// use perro_structs::Vector2;
    ///
    /// let a = Vector2::new(1.0, 1.0);
    /// let b = Vector2::new(4.0, 5.0);
    /// assert_eq!(a.distance_to(b), 5.0);
    /// ```
    #[inline]
    pub fn distance_to(self, other: Self) -> f32 {
        self.to_glam().distance(other.to_glam())
    }

    /// Normalized direction from this vector to another vector.
    ///
    /// # Example
    ///
    /// ```rust
    /// use perro_structs::Vector2;
    ///
    /// let dir = Vector2::new(1.0, 1.0).direction_to(Vector2::new(4.0, 5.0));
    /// assert!((dir.x - 0.6).abs() < 1e-6);
    /// assert!((dir.y - 0.8).abs() < 1e-6);
    /// ```
    #[inline]
    pub fn direction_to(self, other: Self) -> Self {
        Self::from_glam((other.to_glam() - self.to_glam()).normalize_or_zero())
    }

    /// Signed angle in radians from this vector to another vector.
    ///
    /// # Example
    ///
    /// ```rust
    /// use perro_structs::Vector2;
    ///
    /// let a = Vector2::new(1.0, 0.0);
    /// let b = Vector2::new(0.0, 1.0);
    /// assert!((a.angle_to(b) - core::f32::consts::FRAC_PI_2).abs() < 1e-6);
    /// ```
    #[inline]
    pub fn angle_to(self, other: Self) -> f32 {
        self.cross(other).atan2(self.dot(other))
    }

    /// Returns an interpolated copy between this vector and `to`.
    #[inline]
    pub fn lerped(self, to: Self, t: f32) -> Self {
        Self::from_glam(self.to_glam().lerp(to.to_glam(), t))
    }

    /// Linearly interpolates this vector toward `to` in place.
    #[inline]
    pub fn lerp(&mut self, to: Self, t: f32) -> &mut Self {
        *self = self.lerped(to, t);
        self
    }

    /// Returns a spherically interpolated copy between this vector and `to`.
    #[inline]
    pub fn slerped(self, to: Self, t: f32) -> Self {
        let from_len = self.length();
        let to_len = to.length();
        if from_len <= f32::EPSILON || to_len <= f32::EPSILON {
            return self.lerped(to, t);
        }

        let from_dir = self / from_len;
        let to_dir = to / to_len;
        let dot = from_dir.dot(to_dir).clamp(-1.0, 1.0);
        let theta = dot.acos();
        if theta.abs() <= f32::EPSILON {
            return self.lerped(to, t);
        }

        let sin_theta = theta.sin();
        if sin_theta.abs() <= f32::EPSILON {
            return self.lerped(to, t);
        }

        let a = ((1.0 - t) * theta).sin() / sin_theta;
        let b = (t * theta).sin() / sin_theta;
        let dir = from_dir * a + to_dir * b;
        dir * (from_len + (to_len - from_len) * t)
    }

    /// Spherically interpolates this vector toward `to` in place.
    #[inline]
    pub fn slerp(&mut self, to: Self, t: f32) -> &mut Self {
        *self = self.slerped(to, t);
        self
    }
}

// Conversion traits for seamless glam integration
impl From<Vector2> for Vec2 {
    #[inline]
    fn from(v: Vector2) -> Self {
        Vec2::new(v.x, v.y)
    }
}

impl From<Vec2> for Vector2 {
    #[inline]
    fn from(v: Vec2) -> Self {
        Self { x: v.x, y: v.y }
    }
}

impl From<[f32; 2]> for Vector2 {
    #[inline]
    fn from(v: [f32; 2]) -> Self {
        Self::new(v[0], v[1])
    }
}

impl From<Vector2> for [f32; 2] {
    #[inline]
    fn from(v: Vector2) -> Self {
        v.to_array()
    }
}

impl From<(f32, f32)> for Vector2 {
    #[inline]
    fn from(v: (f32, f32)) -> Self {
        Self::new(v.0, v.1)
    }
}

impl From<Vector2> for (f32, f32) {
    #[inline]
    fn from(v: Vector2) -> Self {
        v.to_tuple()
    }
}

impl Add for Vector2 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
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

impl AddAssign for Vector2 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub for Vector2 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl SubAssign for Vector2 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl Mul for Vector2 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(self.x * rhs.x, self.y * rhs.y)
    }
}

impl MulAssign for Vector2 {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        self.x *= rhs.x;
        self.y *= rhs.y;
    }
}

impl Mul<f32> for Vector2 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl MulAssign<f32> for Vector2 {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

impl Div for Vector2 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        Self::new(self.x / rhs.x, self.y / rhs.y)
    }
}

impl DivAssign for Vector2 {
    #[inline]
    fn div_assign(&mut self, rhs: Self) {
        self.x /= rhs.x;
        self.y /= rhs.y;
    }
}

impl Div<f32> for Vector2 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs)
    }
}

impl DivAssign<f32> for Vector2 {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        self.x /= rhs;
        self.y /= rhs;
    }
}

impl Neg for Vector2 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector2_array_tuple_min_max_clamp() {
        let v = Vector2::new(2.0, 5.0);
        assert_eq!(v.to_array(), [2.0, 5.0]);
        assert_eq!(v.to_tuple(), (2.0, 5.0));
        assert_eq!(Vector2::from([1.0, 2.0]), Vector2::new(1.0, 2.0));
        assert_eq!(Vector2::from((3.0, 4.0)), Vector2::new(3.0, 4.0));
        assert_eq!(v.min(Vector2::new(3.0, 4.0)), Vector2::new(2.0, 4.0));
        assert_eq!(v.max(Vector2::new(3.0, 4.0)), Vector2::new(3.0, 5.0));
        assert_eq!(
            v.clamp(Vector2::new(3.0, 1.0), Vector2::new(4.0, 4.0)),
            Vector2::new(3.0, 4.0)
        );
    }

    #[test]
    fn vector2_slerp_rotates_between_axes() {
        let v = Vector2::new(1.0, 0.0).slerped(Vector2::new(0.0, 1.0), 0.5);
        let s = std::f32::consts::FRAC_1_SQRT_2;
        assert!((v.x - s).abs() < 1e-5);
        assert!((v.y - s).abs() < 1e-5);
    }

    #[test]
    fn vector2_round_modes_convert_to_ivector2() {
        let v = Vector2::new(-2.2, 2.8);
        assert_eq!(v.as_ivector2_floor(), IVector2::new(-3, 2));
        assert_eq!(v.as_ivector2_round(), IVector2::new(-2, 3));
        assert_eq!(v.as_ivector2_ceil(), IVector2::new(-2, 3));
        assert_eq!(
            Vector2::new(f32::NAN, f32::INFINITY).as_ivector2_saturating(),
            IVector2::new(0, i32::MAX)
        );
    }
}
