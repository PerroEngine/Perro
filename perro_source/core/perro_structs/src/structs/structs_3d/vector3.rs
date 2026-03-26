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

    /// Linear interpolation between two vectors
    #[inline]
    pub fn lerp(a: Self, b: Self, t: f32) -> Self {
        Self::from_glam(a.to_glam().lerp(b.to_glam(), t))
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

impl Add for Vector3 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
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
