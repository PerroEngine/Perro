use glam::Vec2;
use std::fmt;

/// A simple 2D vector struct that holds (x,y) values
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
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    // Helper to convert to glam for operations
    #[inline(always)]
    fn to_glam(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    // Helper to create from glam
    #[inline(always)]
    fn from_glam(v: Vec2) -> Self {
        Self { x: v.x, y: v.y }
    }

    // ------------------ Math Ops ------------------

    /// Dot product between this vector and another
    pub fn dot(self, rhs: Self) -> f32 {
        self.to_glam().dot(rhs.to_glam())
    }

    /// 2D "cross product" returns a scalar value (signed magnitude of z‑component)
    pub fn cross(self, rhs: Self) -> f32 {
        self.x * rhs.y - self.y * rhs.x
    }

    /// Squared length (avoids a sqrt when only comparing distances)
    pub fn length_squared(&self) -> f32 {
        self.to_glam().length_squared()
    }

    /// Magnitude (length) of the vector
    pub fn length(&self) -> f32 {
        self.to_glam().length()
    }

    /// Returns a new `Vector2` with length = 1 (same direction)
    pub fn normalized(&self) -> Self {
        Self::from_glam(self.to_glam().normalize_or_zero())
    }

    /// Distance between two vectors
    pub fn distance(a: Self, b: Self) -> f32 {
        a.to_glam().distance(b.to_glam())
    }

    /// Linear interpolation between two vectors
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
