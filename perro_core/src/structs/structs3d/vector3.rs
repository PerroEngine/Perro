use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Serialize for Vector3 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        [self.x, self.y, self.z].serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Vector3 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let arr = <[f32; 3]>::deserialize(deserializer)?;
        Ok(Vector3::new(arr[0], arr[1], arr[2]))
    }
}

impl Vector3 {
    /// Creates a new `Vector3`.
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Zero vector (0, 0, 0)
    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0, z: 0.0 }
    }

    /// One vector (1, 1, 1)
    pub fn one() -> Self {
        Self { x: 1.0, y: 1.0, z: 1.0 }
    }

    // Helper to convert to glam for operations
    #[inline(always)]
    fn to_glam(self) -> glam::Vec3 {
        glam::Vec3::new(self.x, self.y, self.z)
    }

    // Helper to create from glam
    #[inline(always)]
    fn from_glam(v: glam::Vec3) -> Self {
        Self { x: v.x, y: v.y, z: v.z }
    }

    /// Converts this vector into a `glam::Vec3` (for operations that need glam types).
    pub fn to_glam_public(self) -> glam::Vec3 {
        self.to_glam()
    }

    /// Returns the dot product between `self` and `rhs`.
    pub fn dot(self, rhs: Self) -> f32 {
        self.to_glam().dot(rhs.to_glam())
    }

    pub fn to_array(&self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }

    /// Returns the cross product between `self` and `rhs`.
    pub fn cross(self, rhs: Self) -> Self {
        Self::from_glam(self.to_glam().cross(rhs.to_glam()))
    }

    /// Returns the vector's magnitude.
    pub fn length(&self) -> f32 {
        self.to_glam().length()
    }

    /// Returns a normalized copy of the vector.
    pub fn normalized(&self) -> Self {
        Self::from_glam(self.to_glam().normalize_or_zero())
    }

    pub fn is_half_half_half(pivot: &Vector3) -> bool {
        pivot.x == 0.5 && pivot.y == 0.5 && pivot.z == 0.5
    }

    pub fn default_pivot() -> Vector3 {
        Vector3::new(0.5, 0.5, 0.5)
    }

    /// Creates a `Vector3` from a `glam::Vec3`.
    pub fn from_glam_public(v: glam::Vec3) -> Self {
        Self { x: v.x, y: v.y, z: v.z }
    }
}

// ---------------------- Arithmetic Ops ----------------------

impl Add for Vector3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::from_glam(self.to_glam() + rhs.to_glam())
    }
}
impl AddAssign for Vector3 {
    fn add_assign(&mut self, rhs: Self) {
        let result = self.to_glam() + rhs.to_glam();
        self.x = result.x;
        self.y = result.y;
        self.z = result.z;
    }
}

impl Sub for Vector3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::from_glam(self.to_glam() - rhs.to_glam())
    }
}
impl SubAssign for Vector3 {
    fn sub_assign(&mut self, rhs: Self) {
        let result = self.to_glam() - rhs.to_glam();
        self.x = result.x;
        self.y = result.y;
        self.z = result.z;
    }
}

// Scalar multiply
impl Mul<f32> for Vector3 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self::Output {
        Self::from_glam(self.to_glam() * rhs)
    }
}
impl MulAssign<f32> for Vector3 {
    fn mul_assign(&mut self, rhs: f32) {
        let result = self.to_glam() * rhs;
        self.x = result.x;
        self.y = result.y;
        self.z = result.z;
    }
}

// Scalar divide
impl Div<f32> for Vector3 {
    type Output = Self;
    fn div(self, rhs: f32) -> Self::Output {
        Self::from_glam(self.to_glam() / rhs)
    }
}
impl DivAssign<f32> for Vector3 {
    fn div_assign(&mut self, rhs: f32) {
        let result = self.to_glam() / rhs;
        self.x = result.x;
        self.y = result.y;
        self.z = result.z;
    }
}

// Element-wise multiply/divide (optional, matches Vector2)
impl Mul for Vector3 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self::from_glam(self.to_glam() * rhs.to_glam())
    }
}
impl Div for Vector3 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        Self::from_glam(self.to_glam() / rhs.to_glam())
    }
}
