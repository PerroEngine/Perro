use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Quaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Quaternion {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    pub fn identity() -> Self {
        Self::new(0.0, 0.0, 0.0, 1.0)
    }

    pub fn from_euler(pitch: f32, yaw: f32, roll: f32) -> Self {
        let quat = glam::Quat::from_euler(glam::EulerRot::YXZ, yaw, pitch, roll);
        Self::from_glam(quat)
    }

    pub fn to_glam(self) -> glam::Quat {
        glam::Quat::from_xyzw(self.x, self.y, self.z, self.w)
    }

    pub fn from_glam(q: glam::Quat) -> Self {
        Self::new(q.x, q.y, q.z, q.w)
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
        crate::structs3d::Vector3::from_glam(self.to_glam() * v.to_glam())
    }
}

impl Default for Quaternion {
    fn default() -> Self {
        Self::identity()
    }
}
