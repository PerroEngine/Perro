use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Serialize for Quaternion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        [self.x, self.y, self.z, self.w].serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Quaternion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let arr = <[f32; 4]>::deserialize(deserializer)?;
        Ok(Quaternion::new(arr[0], arr[1], arr[2], arr[3]))
    }
}

impl fmt::Display for Quaternion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Quaternion({}, {}, {}, {})",
            self.x, self.y, self.z, self.w
        )
    }
}

impl Quaternion {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    pub fn identity() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        }
    }

    // Helper to convert to glam for operations
    #[inline(always)]
    fn to_glam(self) -> glam::Quat {
        glam::Quat::from_xyzw(self.x, self.y, self.z, self.w)
    }

    // Helper to create from glam
    #[inline(always)]
    fn from_glam(q: glam::Quat) -> Self {
        Self {
            x: q.x,
            y: q.y,
            z: q.z,
            w: q.w,
        }
    }

    /// Converts this quaternion into a `glam::Quat` (for operations that need glam types).
    pub fn to_glam_public(self) -> glam::Quat {
        self.to_glam()
    }

    /// Create quaternion from Euler angles in **degrees** (pitch, yaw, roll).
    /// Internally converted to radians for glam.
    pub fn from_euler_degrees(pitch_deg: f32, yaw_deg: f32, roll_deg: f32) -> Self {
        Self::from_euler(
            pitch_deg.to_radians(),
            yaw_deg.to_radians(),
            roll_deg.to_radians(),
        )
    }

    pub fn from_euler(pitch: f32, yaw: f32, roll: f32) -> Self {
        Self::from_glam(glam::Quat::from_euler(
            glam::EulerRot::YXZ,
            yaw,
            pitch,
            roll,
        ))
    }

    /// Creates a quaternion from a 2D rotation angle (degrees, around Z axis).
    /// Used when unifying DynNode transform.rotation (2D nodes use f32 in degrees, 3D use Quaternion).
    pub fn from_rotation_2d(angle_degrees: f32) -> Self {
        let radians = angle_degrees.to_radians();
        Self::from_glam(glam::Quat::from_axis_angle(glam::Vec3::Z, radians))
    }

    /// Extracts the 2D rotation angle (degrees, Z axis) from this quaternion.
    /// Used for implicit Quaternion -> f32 conversion when assigning to a float.
    pub fn to_rotation_2d(&self) -> f32 {
        let (_, _, roll_radians) = self.to_euler();
        roll_radians.to_degrees()
    }

    pub fn to_euler(&self) -> (f32, f32, f32) {
        let (yaw, pitch, roll) = self.to_glam().to_euler(glam::EulerRot::YXZ);
        (pitch, yaw, roll)
    }

    /// Convert quaternion to Euler angles in **degrees** (pitch, yaw, roll).
    pub fn to_euler_degrees(&self) -> (f32, f32, f32) {
        let (p, y, r) = self.to_euler();
        (p.to_degrees(), y.to_degrees(), r.to_degrees())
    }

    /// Convenience: convert this quaternion to Euler degrees as a `Vector3(pitch, yaw, roll)`.
    /// This matches script expectations for `q.as_euler()`.
    pub fn as_euler(&self) -> crate::structs3d::Vector3 {
        let (p, y, r) = self.to_euler_degrees();
        crate::structs3d::Vector3::new(p, y, r)
    }

    pub fn normalize(&self) -> Self {
        Self::from_glam(self.to_glam().normalize())
    }

    pub fn inverse(&self) -> Self {
        Self::from_glam(self.to_glam().inverse())
    }

    pub fn mul(&self, rhs: Self) -> Self {
        Self::from_glam(self.to_glam() * rhs.to_glam())
    }

    pub fn rotate_vec3(&self, v: crate::structs3d::Vector3) -> crate::structs3d::Vector3 {
        let result = self.to_glam() * v.to_glam_public();
        crate::structs3d::Vector3::from_glam_public(result)
    }

    /// Apply an incremental Euler rotation (degrees) to this quaternion and return the new quaternion.
    ///
    /// This is the 3D equivalent of "rotation.x += delta" style updates, but implemented via
    /// quaternion multiplication (avoids gimbal lock and keeps the quaternion normalized).
    pub fn rotate_euler_degrees(
        &self,
        delta_pitch_deg: f32,
        delta_yaw_deg: f32,
        delta_roll_deg: f32,
    ) -> Self {
        self.mul(Quaternion::from_euler_degrees(
            delta_pitch_deg,
            delta_yaw_deg,
            delta_roll_deg,
        ))
        .normalize()
    }

    /// Rotate around X axis by `delta_pitch_deg` degrees and return the new quaternion.
    pub fn rotate_x(&self, delta_pitch_deg: f32) -> Self {
        self.rotate_euler_degrees(delta_pitch_deg, 0.0, 0.0)
    }

    /// Rotate around Y axis by `delta_yaw_deg` degrees and return the new quaternion.
    pub fn rotate_y(&self, delta_yaw_deg: f32) -> Self {
        self.rotate_euler_degrees(0.0, delta_yaw_deg, 0.0)
    }

    /// Rotate around Z axis by `delta_roll_deg` degrees and return the new quaternion.
    pub fn rotate_z(&self, delta_roll_deg: f32) -> Self {
        self.rotate_euler_degrees(0.0, 0.0, delta_roll_deg)
    }

    /// Creates a `Quaternion` from a `glam::Quat`.
    pub fn from_glam_public(q: glam::Quat) -> Self {
        Self {
            x: q.x,
            y: q.y,
            z: q.z,
            w: q.w,
        }
    }
}

impl Default for Quaternion {
    fn default() -> Self {
        Self::identity()
    }
}
