use glam::Vec2;
use std::fmt;

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

    /// Linear interpolation between two vectors
    #[inline]
    pub fn lerp(a: Self, b: Self, t: f32) -> Self {
        Self::from_glam(a.to_glam().lerp(b.to_glam(), t))
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
