use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

impl Serialize for Vector2 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        [self.x, self.y].serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Vector2 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let arr = <[f32; 2]>::deserialize(deserializer)?;
        Ok(Vector2::new(arr[0], arr[1]))
    }
}

impl Vector2 {
    /// Creates a new 2D vector
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// (0, 0)
    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    /// (1, 1)
    pub fn one() -> Self {
        Self { x: 1.0, y: 1.0 }
    }

    /// Whether the pivot is the default (0.5, 0.5)
    pub fn is_half_half(pivot: &Vector2) -> bool {
        pivot.x == 0.5 && pivot.y == 0.5
    }

    /// The default pivot value (0.5, 0.5)
    pub fn default_pivot() -> Vector2 {
        Vector2::new(0.5, 0.5)
    }

    // Helper to convert to glam for operations
    #[inline(always)]
    fn to_glam(self) -> glam::Vec2 {
        glam::Vec2::new(self.x, self.y)
    }

    // Helper to create from glam
    #[inline(always)]
    fn from_glam(v: glam::Vec2) -> Self {
        Self { x: v.x, y: v.y }
    }

    /// Converts this vector into a `glam::Vec2` (for operations that need glam types).
    pub fn to_glam_public(self) -> glam::Vec2 {
        self.to_glam()
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

    /// Creates a `Vector2` from a `glam::Vec2`.
    pub fn from_glam_public(v: glam::Vec2) -> Self {
        Self { x: v.x, y: v.y }
    }
}

// --- Add ---
impl Add for Vector2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::from_glam(self.to_glam() + rhs.to_glam())
    }
}
impl AddAssign for Vector2 {
    fn add_assign(&mut self, rhs: Self) {
        let result = self.to_glam() + rhs.to_glam();
        self.x = result.x;
        self.y = result.y;
    }
}

// --- Sub ---
impl Sub for Vector2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::from_glam(self.to_glam() - rhs.to_glam())
    }
}
impl SubAssign for Vector2 {
    fn sub_assign(&mut self, rhs: Self) {
        let result = self.to_glam() - rhs.to_glam();
        self.x = result.x;
        self.y = result.y;
    }
}

// --- Mul (scalar) ---
impl Mul<f32> for Vector2 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self::Output {
        Self::from_glam(self.to_glam() * rhs)
    }
}
impl MulAssign<f32> for Vector2 {
    fn mul_assign(&mut self, rhs: f32) {
        let result = self.to_glam() * rhs;
        self.x = result.x;
        self.y = result.y;
    }
}

// --- Div (scalar) ---
impl Div<f32> for Vector2 {
    type Output = Self;
    fn div(self, rhs: f32) -> Self::Output {
        Self::from_glam(self.to_glam() / rhs)
    }
}
impl DivAssign<f32> for Vector2 {
    fn div_assign(&mut self, rhs: f32) {
        let result = self.to_glam() / rhs;
        self.x = result.x;
        self.y = result.y;
    }
}

// --- Optional: element-wise Mul/Div (like your original) ---
impl Mul for Vector2 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self::from_glam(self.to_glam() * rhs.to_glam())
    }
}
impl Div for Vector2 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        Self::from_glam(self.to_glam() / rhs.to_glam())
    }
}
