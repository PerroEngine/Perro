use glam::Vec3;
use std::fmt;

/// A simple 3D vector struct that holds (x,y,z) values
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
    pub fn dot(self, rhs: Self) -> f32 {
        self.to_glam().dot(rhs.to_glam())
    }

    /// Cross product returns a vector perpendicular to both inputs
    pub fn cross(self, rhs: Self) -> Self {
        Self::from_glam(self.to_glam().cross(rhs.to_glam()))
    }

    /// Squared length (avoids a sqrt when only comparing distances)
    pub fn length_squared(&self) -> f32 {
        self.to_glam().length_squared()
    }

    /// Magnitude (length) of the vector
    pub fn length(&self) -> f32 {
        self.to_glam().length()
    }

    /// Returns a new `Vector3` with length = 1 (same direction)
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
