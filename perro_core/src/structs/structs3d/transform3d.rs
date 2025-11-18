use serde::{Deserialize, Serialize};
use glam::{Mat4};

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
fn is_default_rotation(v: &Quaternion) -> bool {
    *v == default_rotation()
}

fn default_scale() -> Vector3 {
    Vector3::one()
}
fn is_default_scale(v: &Vector3) -> bool {
    *v == default_scale()
}

/// 3D transform, analogous to `Transform2D`
///
/// Includes position (`Vector3`), rotation (`Quaternion`), and scale (`Vector3`).
#[derive(Serialize, Deserialize, Clone, Debug, Copy, PartialEq)]
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
    /// Create a new `Transform3D`
    pub fn new(position: Vector3, rotation: Quaternion, scale: Vector3) -> Self {
        Self {
            position,
            rotation,
            scale,
        }
    }

    /// Build a transform from position, euler rotation (in radians), and scale.
    pub fn from_euler(position: Vector3, euler: Vector3, scale: Vector3) -> Self {
        Self {
            position,
            rotation: Quaternion::from_euler(euler.x, euler.y, euler.z),
            scale,
        }
    }

    /// Check whether all components are default.
    pub fn is_default(&self) -> bool {
        is_default_position(&self.position)
            && is_default_rotation(&self.rotation)
            && is_default_scale(&self.scale)
    }

    /// Converts to a `glam::Mat4` (Scale → Rotate → Translate)
    pub fn to_mat4(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            self.scale.to_glam(),
            self.rotation.to_glam(),
            self.position.to_glam(),
        )
    }

    /// Converts a `Mat4` back into a `Transform3D`
    /// (approximation for non‑uniform scaling)
    pub fn from_mat4(mat: Mat4) -> Self {
        let pos = Vector3::from_glam(mat.w_axis.truncate());

        // extract basis vectors -> scale
        let sx = mat.x_axis.truncate().length();
        let sy = mat.y_axis.truncate().length();
        let sz = mat.z_axis.truncate().length();
        let scale = Vector3::new(sx, sy, sz);

        // normalize matrix for rotation extraction
        let rot_mat = Mat4::from_cols(
            mat.x_axis / sx,
            mat.y_axis / sy,
            mat.z_axis / sz,
            glam::Vec4::W,
        );
        let rotation = Quaternion::from_glam(glam::Quat::from_mat4(&rot_mat));

        Transform3D {
            position: pos,
            rotation,
            scale,
        }
    }

    /// Combine two transforms (like multiplying matrices)
    pub fn composed(&self, other: &Transform3D) -> Transform3D {
        let new_pos = self.position + self.rotation.rotate_vec3(other.position * self.scale);
        let new_rot = self.rotation.mul(other.rotation);
        let new_scale = self.scale * other.scale;
        Transform3D {
            position: new_pos,
            rotation: new_rot,
            scale: new_scale,
        }
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