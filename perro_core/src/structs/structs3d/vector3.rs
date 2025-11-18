use serde::{Deserialize, Serialize};
use std::ops::{
    Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign,
};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3 {
    /// Creates a new `Vector3`.
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Zero vector (0, 0, 0)
    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    /// One vector (1, 1, 1)
    pub fn one() -> Self {
        Self::new(1.0, 1.0, 1.0)
    }

    /// Returns the dot product between `self` and `rhs`.
    pub fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    /// Returns the cross product between `self` and `rhs`.
    pub fn cross(self, rhs: Self) -> Self {
        Self {
            x: self.y * rhs.z - self.z * rhs.y,
            y: self.z * rhs.x - self.x * rhs.z,
            z: self.x * rhs.y - self.y * rhs.x,
        }
    }

    /// Returns the vector's magnitude.
    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    /// Returns a normalized copy of the vector.
    pub fn normalized(&self) -> Self {
        let len = self.length();
        if len != 0.0 {
            *self / len
        } else {
            Self::zero()
        }
    }

    /// Converts this vector into a `glam::Vec3`.
    pub fn to_glam(self) -> glam::Vec3 {
        glam::Vec3::new(self.x, self.y, self.z)
    }

    /// Creates a `Vector3` from a `glam::Vec3`.
    pub fn from_glam(v: glam::Vec3) -> Self {
        Self::new(v.x, v.y, v.z)
    }

    pub fn is_half_half_half(pivot: &Vector3) -> bool {
        pivot.x == 0.5 && pivot.y == 0.5 && pivot.z == 0.5
    }

    pub fn default_pivot() -> Vector3 {
        Vector3::new(0.5, 0.5, 0.5)
    }
}

// ---------------------- Arithmetic Ops ----------------------

impl Add for Vector3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}
impl AddAssign for Vector3 {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
    }
}

impl Sub for Vector3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}
impl SubAssign for Vector3 {
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
    }
}

// Scalar multiply
impl Mul<f32> for Vector3 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}
impl MulAssign<f32> for Vector3 {
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
        self.z *= rhs;
    }
}

// Scalar divide
impl Div<f32> for Vector3 {
    type Output = Self;
    fn div(self, rhs: f32) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs, self.z / rhs)
    }
}
impl DivAssign<f32> for Vector3 {
    fn div_assign(&mut self, rhs: f32) {
        self.x /= rhs;
        self.y /= rhs;
        self.z /= rhs;
    }
}

// Element-wise multiply/divide (optional, matches Vector2)
impl Mul for Vector3 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(self.x * rhs.x, self.y * rhs.y, self.z * rhs.z)
    }
}
impl Div for Vector3 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        Self::new(self.x / rhs.x, self.y / rhs.y, self.z / rhs.z)
    }
}