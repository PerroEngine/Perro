use super::{IVector3, UVector3};
use glam::Vec3;
use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// A 3D vector with `x`, `y`, and `z` components.
///
/// # Example
///
/// ```rust
/// use perro_structs::Vector3;
///
/// let from = Vector3::new(0.0, 0.0, 0.0);
/// let to = Vector3::new(0.0, 0.0, -3.0);
/// assert_eq!(from.distance_to(to), 3.0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl fmt::Display for Vector3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Vector3({}, {}, {})", self.x, self.y, self.z)
    }
}

impl Vector3 {
    /// Zero vector3 constant (0, 0, 0)
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// Half vector3 constant (0.5, 0.5, 0.5)
    pub const HALF: Self = Self {
        x: 0.5,
        y: 0.5,
        z: 0.5,
    };

    /// One vector3 constant (1, 1, 1)
    pub const ONE: Self = Self {
        x: 1.0,
        y: 1.0,
        z: 1.0,
    };

    /// Creates a new 3D vector
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Returns components as an array.
    #[inline]
    pub const fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }

    /// Returns components as a tuple.
    #[inline]
    pub const fn to_tuple(self) -> (f32, f32, f32) {
        (self.x, self.y, self.z)
    }

    /// Converts to `UVector3` by clamping negatives/non-finite values to zero and truncating decimals.
    #[inline]
    pub fn as_uvector3_saturating(self) -> UVector3 {
        UVector3::new(
            f32_to_u32_saturating(self.x),
            f32_to_u32_saturating(self.y),
            f32_to_u32_saturating(self.z),
        )
    }

    /// Converts to `UVector3` by flooring each component first.
    #[inline]
    pub fn as_uvector3_floor(self) -> UVector3 {
        UVector3::new(
            f32_to_u32_saturating(self.x.floor()),
            f32_to_u32_saturating(self.y.floor()),
            f32_to_u32_saturating(self.z.floor()),
        )
    }

    /// Converts to `UVector3` by rounding each component first.
    #[inline]
    pub fn as_uvector3_round(self) -> UVector3 {
        UVector3::new(
            f32_to_u32_saturating(self.x.round()),
            f32_to_u32_saturating(self.y.round()),
            f32_to_u32_saturating(self.z.round()),
        )
    }

    /// Converts to `UVector3` by ceiling each component first.
    #[inline]
    pub fn as_uvector3_ceil(self) -> UVector3 {
        UVector3::new(
            f32_to_u32_saturating(self.x.ceil()),
            f32_to_u32_saturating(self.y.ceil()),
            f32_to_u32_saturating(self.z.ceil()),
        )
    }

    /// Converts to `IVector3` by clamping non-finite/out-of-range values and truncating decimals.
    #[inline]
    pub fn as_ivector3_saturating(self) -> IVector3 {
        IVector3::new(
            f32_to_i32_saturating(self.x),
            f32_to_i32_saturating(self.y),
            f32_to_i32_saturating(self.z),
        )
    }

    /// Converts to `IVector3` by flooring each component first.
    #[inline]
    pub fn as_ivector3_floor(self) -> IVector3 {
        IVector3::new(
            f32_to_i32_saturating(self.x.floor()),
            f32_to_i32_saturating(self.y.floor()),
            f32_to_i32_saturating(self.z.floor()),
        )
    }

    /// Converts to `IVector3` by rounding each component first.
    #[inline]
    pub fn as_ivector3_round(self) -> IVector3 {
        IVector3::new(
            f32_to_i32_saturating(self.x.round()),
            f32_to_i32_saturating(self.y.round()),
            f32_to_i32_saturating(self.z.round()),
        )
    }

    /// Converts to `IVector3` by ceiling each component first.
    #[inline]
    pub fn as_ivector3_ceil(self) -> IVector3 {
        IVector3::new(
            f32_to_i32_saturating(self.x.ceil()),
            f32_to_i32_saturating(self.y.ceil()),
            f32_to_i32_saturating(self.z.ceil()),
        )
    }

    // Helper to convert to glam for operations
    #[inline(always)]
    const fn to_glam(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }

    // Helper to create from glam
    #[inline(always)]
    const fn from_glam(v: Vec3) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
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

    /// Cross product returns a vector perpendicular to both inputs
    #[inline]
    pub fn cross(self, rhs: Self) -> Self {
        Self::from_glam(self.to_glam().cross(rhs.to_glam()))
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

    /// Returns a new `Vector3` with length = 1 (same direction)
    #[inline]
    pub fn normalized(&self) -> Self {
        Self::from_glam(self.to_glam().normalize_or_zero())
    }

    /// Distance between two vectors.
    ///
    /// # Example
    ///
    /// ```rust
    /// use perro_structs::Vector3;
    ///
    /// let d = Vector3::distance(Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 3.0));
    /// assert_eq!(d, 3.0);
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
    /// use perro_structs::Vector3;
    ///
    /// let d = Vector3::new(1.0, 2.0, 3.0).distance_to(Vector3::new(1.0, 2.0, 6.0));
    /// assert_eq!(d, 3.0);
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
    /// use perro_structs::Vector3;
    ///
    /// let dir = Vector3::new(0.0, 0.0, 0.0).direction_to(Vector3::new(0.0, 0.0, -2.0));
    /// assert_eq!(dir, Vector3::new(0.0, 0.0, -1.0));
    /// ```
    #[inline]
    pub fn direction_to(self, other: Self) -> Self {
        Self::from_glam((other.to_glam() - self.to_glam()).normalize_or_zero())
    }

    /// Angle in radians from this vector to another vector.
    ///
    /// # Example
    ///
    /// ```rust
    /// use perro_structs::Vector3;
    ///
    /// let a = Vector3::new(1.0, 0.0, 0.0);
    /// let b = Vector3::new(0.0, 1.0, 0.0);
    /// assert!((a.angle_to(b) - core::f32::consts::FRAC_PI_2).abs() < 1e-6);
    /// ```
    #[inline]
    pub fn angle_to(self, other: Self) -> f32 {
        self.to_glam().angle_between(other.to_glam())
    }

    /// Projects this vector onto another vector.
    ///
    /// # Example
    ///
    /// ```rust
    /// use perro_structs::Vector3;
    ///
    /// let v = Vector3::new(2.0, 3.0, 0.0);
    /// let x = Vector3::new(1.0, 0.0, 0.0);
    /// assert_eq!(v.project_on(x), Vector3::new(2.0, 0.0, 0.0));
    /// ```
    #[inline]
    pub fn project_on(self, onto: Self) -> Self {
        let onto_len_sq = onto.length_squared();
        if onto_len_sq <= f32::EPSILON {
            return Self::ZERO;
        }
        let scale = self.dot(onto) / onto_len_sq;
        Self::new(onto.x * scale, onto.y * scale, onto.z * scale)
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
impl From<Vector3> for Vec3 {
    #[inline]
    fn from(v: Vector3) -> Self {
        Vec3::new(v.x, v.y, v.z)
    }
}

impl From<Vec3> for Vector3 {
    #[inline]
    fn from(v: Vec3) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

impl From<[f32; 3]> for Vector3 {
    #[inline]
    fn from(v: [f32; 3]) -> Self {
        Self::new(v[0], v[1], v[2])
    }
}

impl From<Vector3> for [f32; 3] {
    #[inline]
    fn from(v: Vector3) -> Self {
        v.to_array()
    }
}

impl From<(f32, f32, f32)> for Vector3 {
    #[inline]
    fn from(v: (f32, f32, f32)) -> Self {
        Self::new(v.0, v.1, v.2)
    }
}

impl From<Vector3> for (f32, f32, f32) {
    #[inline]
    fn from(v: Vector3) -> Self {
        v.to_tuple()
    }
}

impl Add for Vector3 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
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

impl AddAssign for Vector3 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
    }
}

impl Sub for Vector3 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl SubAssign for Vector3 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
    }
}

impl Mul for Vector3 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(self.x * rhs.x, self.y * rhs.y, self.z * rhs.z)
    }
}

impl MulAssign for Vector3 {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        self.x *= rhs.x;
        self.y *= rhs.y;
        self.z *= rhs.z;
    }
}

impl Mul<f32> for Vector3 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

impl MulAssign<f32> for Vector3 {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
        self.z *= rhs;
    }
}

impl Div for Vector3 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        Self::new(self.x / rhs.x, self.y / rhs.y, self.z / rhs.z)
    }
}

impl DivAssign for Vector3 {
    #[inline]
    fn div_assign(&mut self, rhs: Self) {
        self.x /= rhs.x;
        self.y /= rhs.y;
        self.z /= rhs.z;
    }
}

impl Div<f32> for Vector3 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs, self.z / rhs)
    }
}

impl DivAssign<f32> for Vector3 {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        self.x /= rhs;
        self.y /= rhs;
        self.z /= rhs;
    }
}

impl Neg for Vector3 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y, -self.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector3_array_tuple_min_max_clamp() {
        let v = Vector3::new(2.0, 5.0, 8.0);
        assert_eq!(v.to_array(), [2.0, 5.0, 8.0]);
        assert_eq!(v.to_tuple(), (2.0, 5.0, 8.0));
        assert_eq!(Vector3::from([1.0, 2.0, 3.0]), Vector3::new(1.0, 2.0, 3.0));
        assert_eq!(Vector3::from((3.0, 4.0, 5.0)), Vector3::new(3.0, 4.0, 5.0));
        assert_eq!(
            v.min(Vector3::new(3.0, 4.0, 9.0)),
            Vector3::new(2.0, 4.0, 8.0)
        );
        assert_eq!(
            v.max(Vector3::new(3.0, 4.0, 9.0)),
            Vector3::new(3.0, 5.0, 9.0)
        );
        assert_eq!(
            v.clamp(Vector3::new(3.0, 1.0, 7.0), Vector3::new(4.0, 4.0, 7.5)),
            Vector3::new(3.0, 4.0, 7.5)
        );
    }

    #[test]
    fn vector3_slerp_rotates_between_axes() {
        let v = Vector3::new(1.0, 0.0, 0.0).slerped(Vector3::new(0.0, 1.0, 0.0), 0.5);
        let s = std::f32::consts::FRAC_1_SQRT_2;
        assert!((v.x - s).abs() < 1e-5);
        assert!((v.y - s).abs() < 1e-5);
        assert!(v.z.abs() < 1e-5);
    }

    #[test]
    fn vector3_round_modes_convert_to_ivector3() {
        let v = Vector3::new(-2.2, 2.5, 2.8);
        assert_eq!(v.as_ivector3_floor(), IVector3::new(-3, 2, 2));
        assert_eq!(v.as_ivector3_round(), IVector3::new(-2, 3, 3));
        assert_eq!(v.as_ivector3_ceil(), IVector3::new(-2, 3, 3));
        assert_eq!(
            Vector3::new(f32::NAN, f32::INFINITY, f32::NEG_INFINITY).as_ivector3_saturating(),
            IVector3::new(0, i32::MAX, i32::MIN)
        );
    }
}
