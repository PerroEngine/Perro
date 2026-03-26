use super::vector3::Vector3;
use glam::{Mat3, Quat, Vec3};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// A quaternion representing rotation in 3D space.
///
/// # Example
///
/// ```rust
/// use perro_structs::{Quaternion, Vector3};
///
/// let q = Quaternion::looking_at(Vector3::new(0.0, 0.0, -1.0), Vector3::new(0.0, 1.0, 0.0));
/// let fwd = q.rotate_vector3(Vector3::new(0.0, 0.0, -1.0));
/// assert!((fwd.z + 1.0).abs() < 1e-5);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl std::fmt::Display for Quaternion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Quaternion({}, {}, {}, {})",
            self.x, self.y, self.z, self.w
        )
    }
}

impl Quaternion {
    pub const IDENTITY: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    /// Convert to glam Quat
    #[inline]
    pub fn to_quat(self) -> Quat {
        Quat::from_xyzw(self.x, self.y, self.z, self.w)
    }

    /// Create from glam Quat
    #[inline]
    pub fn from_quat(quat: Quat) -> Self {
        Self {
            x: quat.x,
            y: quat.y,
            z: quat.z,
            w: quat.w,
        }
    }

    /// Returns a normalized copy of this quaternion.
    ///
    /// # Example
    ///
    /// ```rust
    /// use perro_structs::Quaternion;
    ///
    /// let q = Quaternion::new(0.0, 0.0, 2.0, 2.0).normalized();
    /// let len2 = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
    /// assert!((len2 - 1.0).abs() < 1e-6);
    /// ```
    #[inline]
    pub fn normalized(self) -> Self {
        Self::from_quat(self.to_quat().normalize())
    }

    /// Returns the inverse rotation.
    ///
    /// # Example
    ///
    /// ```rust
    /// use perro_structs::{Quaternion, Vector3};
    ///
    /// let q = Quaternion::new(0.0, 0.70710677, 0.0, 0.70710677).normalized();
    /// let v = q.rotate_vector3(Vector3::new(0.0, 0.0, -1.0));
    /// let back = q.inverse().rotate_vector3(v);
    /// assert!((back.z + 1.0).abs() < 1e-5);
    /// ```
    #[inline]
    pub fn inverse(self) -> Self {
        Self::from_quat(self.to_quat().inverse())
    }

    /// Quaternion multiplication (`self * rhs`).
    #[inline]
    pub fn mul_quat(self, rhs: Self) -> Self {
        Self::from_quat(self.to_quat() * rhs.to_quat())
    }

    /// Dot product between this quaternion and another.
    #[inline]
    pub fn dot(self, rhs: Self) -> f32 {
        self.to_quat().dot(rhs.to_quat())
    }

    /// Rotates a `Vector3` by this quaternion.
    ///
    /// # Example
    ///
    /// ```rust
    /// use perro_structs::{Quaternion, Vector3};
    ///
    /// let q = Quaternion::new(0.0, 0.70710677, 0.0, 0.70710677).normalized();
    /// let v = q.rotate_vector3(Vector3::new(0.0, 0.0, -1.0));
    /// assert!((v.x + 1.0).abs() < 1e-5);
    /// ```
    #[inline]
    pub fn rotate_vector3(self, v: Vector3) -> Vector3 {
        let out = self.to_quat() * Vec3::new(v.x, v.y, v.z);
        Vector3::new(out.x, out.y, out.z)
    }

    /// Returns an interpolated copy between this quaternion and `to`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use perro_structs::Quaternion;
    ///
    /// let a = Quaternion::IDENTITY;
    /// let b = Quaternion::new(0.0, 1.0, 0.0, 0.0).normalized();
    /// let mid = a.slerped(b, 0.5);
    /// let len2 = mid.x * mid.x + mid.y * mid.y + mid.z * mid.z + mid.w * mid.w;
    /// assert!((len2 - 1.0).abs() < 1e-5);
    /// ```
    #[inline]
    pub fn slerped(self, to: Self, t: f32) -> Self {
        Self::from_quat(self.to_quat().slerp(to.to_quat(), t))
    }

    /// Spherically interpolates this quaternion toward `to` in place.
    #[inline]
    pub fn slerp(&mut self, to: Self, t: f32) -> &mut Self {
        *self = self.slerped(to, t);
        self
    }

    /// Builds a quaternion that points local -Z toward `direction` while keeping `up` as close as possible.
    ///
    /// # Example
    ///
    /// ```rust
    /// use perro_structs::{Quaternion, Vector3};
    ///
    /// let q = Quaternion::looking_at(Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, 1.0, 0.0));
    /// let fwd = q.rotate_vector3(Vector3::new(0.0, 0.0, -1.0));
    /// assert!((fwd.x - 1.0).abs() < 1e-4);
    /// ```
    #[inline]
    pub fn looking_at(direction: Vector3, up: Vector3) -> Self {
        let forward = Vec3::new(direction.x, direction.y, direction.z).normalize_or_zero();
        if forward.length_squared() <= f32::EPSILON {
            return Self::IDENTITY;
        }

        let mut up_vec = Vec3::new(up.x, up.y, up.z).normalize_or_zero();
        if up_vec.length_squared() <= f32::EPSILON {
            up_vec = Vec3::Y;
        }

        let mut right = forward.cross(up_vec).normalize_or_zero();
        if right.length_squared() <= f32::EPSILON {
            let fallback_up = if forward.y.abs() < 0.999 {
                Vec3::Y
            } else {
                Vec3::Z
            };
            right = forward.cross(fallback_up).normalize_or_zero();
            if right.length_squared() <= f32::EPSILON {
                return Self::IDENTITY;
            }
        }

        let corrected_up = right.cross(forward).normalize_or_zero();
        let basis = Mat3::from_cols(right, corrected_up, -forward);
        Self::from_quat(Quat::from_mat3(&basis).normalize())
    }

    /// Sets this quaternion to a look rotation.
    ///
    /// # Example
    ///
    /// ```rust
    /// use perro_structs::{Quaternion, Vector3};
    ///
    /// let mut q = Quaternion::IDENTITY;
    /// q.look_at(Vector3::new(0.0, 0.0, -1.0), Vector3::new(0.0, 1.0, 0.0));
    /// let fwd = q.rotate_vector3(Vector3::new(0.0, 0.0, -1.0));
    /// assert!((fwd.z + 1.0).abs() < 1e-5);
    /// ```
    #[inline]
    pub fn look_at(&mut self, direction: Vector3, up: Vector3) -> &mut Self {
        *self = Self::looking_at(direction, up);
        self
    }

    /// Rotate around local X axis by `radians`.
    #[inline]
    pub fn rotate_x(&mut self, radians: f32) -> &mut Self {
        let rotated = Quat::from_rotation_x(radians) * self.to_quat();
        *self = rotated.into();
        self
    }

    /// Rotate around local Y axis by `radians`.
    #[inline]
    pub fn rotate_y(&mut self, radians: f32) -> &mut Self {
        let rotated = Quat::from_rotation_y(radians) * self.to_quat();
        *self = rotated.into();
        self
    }

    /// Rotate around local Z axis by `radians`.
    #[inline]
    pub fn rotate_z(&mut self, radians: f32) -> &mut Self {
        let rotated = Quat::from_rotation_z(radians) * self.to_quat();
        *self = rotated.into();
        self
    }

    /// Apply X, then Y, then Z local-axis rotations.
    #[inline]
    pub fn rotate_xyz(&mut self, x: f32, y: f32, z: f32) -> &mut Self {
        self.rotate_x(x).rotate_y(y).rotate_z(z)
    }

    /// Normalize the quaternion to unit length.
    #[inline]
    pub fn normalize(&mut self) -> &mut Self {
        let len_sq = self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w;
        if len_sq > 0.0 {
            let inv_len = len_sq.sqrt().recip();
            self.x *= inv_len;
            self.y *= inv_len;
            self.z *= inv_len;
            self.w *= inv_len;
        }
        self
    }
}

// Convenient conversions using From/Into traits
impl From<Quat> for Quaternion {
    #[inline]
    fn from(quat: Quat) -> Self {
        Self::from_quat(quat)
    }
}

impl From<Quaternion> for Quat {
    #[inline]
    fn from(q: Quaternion) -> Self {
        q.to_quat()
    }
}

impl Default for Quaternion {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Add for Quaternion {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(
            self.x + rhs.x,
            self.y + rhs.y,
            self.z + rhs.z,
            self.w + rhs.w,
        )
    }
}

impl AddAssign for Quaternion {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
        self.w += rhs.w;
    }
}

impl Sub for Quaternion {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(
            self.x - rhs.x,
            self.y - rhs.y,
            self.z - rhs.z,
            self.w - rhs.w,
        )
    }
}

impl SubAssign for Quaternion {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
        self.w -= rhs.w;
    }
}

impl Mul for Quaternion {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        self.mul_quat(rhs)
    }
}

impl MulAssign for Quaternion {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = self.mul_quat(rhs);
    }
}

impl Mul<f32> for Quaternion {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs, self.w * rhs)
    }
}

impl Mul<Vector3> for Quaternion {
    type Output = Vector3;

    #[inline]
    fn mul(self, rhs: Vector3) -> Self::Output {
        self.rotate_vector3(rhs)
    }
}

impl MulAssign<f32> for Quaternion {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
        self.z *= rhs;
        self.w *= rhs;
    }
}

impl Div for Quaternion {
    type Output = Self;

    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        Self::new(
            self.x / rhs.x,
            self.y / rhs.y,
            self.z / rhs.z,
            self.w / rhs.w,
        )
    }
}

impl DivAssign for Quaternion {
    #[inline]
    fn div_assign(&mut self, rhs: Self) {
        self.x /= rhs.x;
        self.y /= rhs.y;
        self.z /= rhs.z;
        self.w /= rhs.w;
    }
}

impl Div<f32> for Quaternion {
    type Output = Self;

    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs, self.z / rhs, self.w / rhs)
    }
}

impl DivAssign<f32> for Quaternion {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        self.x /= rhs;
        self.y /= rhs;
        self.z /= rhs;
        self.w /= rhs;
    }
}

impl Neg for Quaternion {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y, -self.z, -self.w)
    }
}
