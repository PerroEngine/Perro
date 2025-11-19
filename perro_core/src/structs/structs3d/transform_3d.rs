use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

use crate::{Quaternion, Vector3};

fn default_position() -> Vector3 {
    Vector3::zero()
}
fn is_default_position(v: &Vector3) -> bool {
    *v == default_position()
}

fn default_rotation() -> Quaternion {
    Quaternion::identity()
}
fn is_default_rotation(q: &Quaternion) -> bool {
    *q == default_rotation()
}

fn default_scale() -> Vector3 {
    Vector3::one()
}
fn is_default_scale(v: &Vector3) -> bool {
    *v == default_scale()
}

#[derive(Clone, Debug, Serialize, Deserialize, Copy)]
pub struct Transform3D {
    #[serde(
        default = "default_position",
        skip_serializing_if = "is_default_position"
    )]
    pub position: Vector3,

    #[serde(
        default = "default_rotation",
        skip_serializing_if = "is_default_rotation"
    )]
    pub rotation: Quaternion,

    #[serde(default = "default_scale", skip_serializing_if = "is_default_scale")]
    pub scale: Vector3,
}

impl Transform3D {
    pub fn new(position: Vector3, rotation: Quaternion, scale: Vector3) -> Self {
        Self {
            position,
            rotation,
            scale,
        }
    }

    pub fn is_default(&self) -> bool {
        is_default_position(&self.position)
            && is_default_rotation(&self.rotation)
            && is_default_scale(&self.scale)
    }

    /// Returns a `glam::Mat4` representing scale→rotate→translate
    pub fn to_mat4(&self) -> Mat4 {
        let s = Mat4::from_scale(self.scale.to_glam());
        let r = Mat4::from_quat(self.rotation.to_glam());
        let t = Mat4::from_translation(self.position.to_glam());
        t * r * s
    }
}

impl Default for Transform3D {
    fn default() -> Self {
        Self {
            position: default_position(),
            rotation: default_rotation(),
            scale: default_scale(),
        }
    }
}
