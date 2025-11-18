use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
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

    // ------------------ Math Ops ------------------

    /// Dot product between this vector and another
    pub fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y
    }

    /// 2D “cross product” returns a scalar value (signed magnitude of z‑component)
    pub fn cross(self, rhs: Self) -> f32 {
        self.x * rhs.y - self.y * rhs.x
    }

    /// Squared length (avoids a sqrt when only comparing distances)
    pub fn length_squared(&self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    /// Magnitude (length) of the vector
    pub fn length(&self) -> f32 {
        self.length_squared().sqrt()
    }

    /// Returns a new `Vector2` with length = 1 (same direction)
    pub fn normalized(&self) -> Self {
        let len = self.length();
        if len > 0.0 { *self / len } else { Self::zero() }
    }

    /// Distance between two vectors
    pub fn distance(a: Self, b: Self) -> f32 {
        (a - b).length()
    }

    /// Linear interpolation between two vectors
    pub fn lerp(a: Self, b: Self, t: f32) -> Self {
        a + (b - a) * t
    }
}

// --- Add ---
impl Add for Vector2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}
impl AddAssign for Vector2 {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

// --- Sub ---
impl Sub for Vector2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}
impl SubAssign for Vector2 {
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

// --- Mul (scalar) ---
impl Mul<f32> for Vector2 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs)
    }
}
impl MulAssign<f32> for Vector2 {
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

// --- Div (scalar) ---
impl Div<f32> for Vector2 {
    type Output = Self;
    fn div(self, rhs: f32) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs)
    }
}
impl DivAssign<f32> for Vector2 {
    fn div_assign(&mut self, rhs: f32) {
        self.x /= rhs;
        self.y /= rhs;
    }
}

// --- Optional: element-wise Mul/Div (like your original) ---
impl Mul for Vector2 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(self.x * rhs.x, self.y * rhs.y)
    }
}
impl Div for Vector2 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        Self::new(self.x / rhs.x, self.y / rhs.y)
    }
}
