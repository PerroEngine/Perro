use glam::Mat4;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::{Quaternion, Vector3};

fn default_position() -> Vector3 {
    Vector3::ZERO
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
    Vector3::ONE
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

impl fmt::Display for Transform3D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Transform3D(position:{}, rotation:{}, scale:{})",
            self.position, self.rotation, self.scale
        )
    }
}

impl Transform3D {
    pub fn new(position: Vector3, rotation: Quaternion, scale: Vector3) -> Self {
        Self {
            position,
            rotation,
            scale,
        }
    }

    pub fn rotation_euler(&self) -> Vector3 {
        // Scripts/scene use degrees for consistency (like 2D).
        let (pitch_deg, yaw_deg, roll_deg) = self.rotation.to_euler_degrees();
        Vector3::new(pitch_deg, yaw_deg, roll_deg)
    }

    pub fn set_rotation_euler(&mut self, e: Vector3) {
        // Scripts/scene use degrees for consistency (like 2D).
        self.rotation = Quaternion::from_euler_degrees(e.x, e.y, e.z);
    }

    /// Applies incremental Euler rotations (like Godot's rotation.x += delta)
    /// This avoids gimbal lock by applying quaternion rotations incrementally
    pub fn rotate_euler(&mut self, delta_pitch: f32, delta_yaw: f32, delta_roll: f32) {
        // Deltas are in degrees.
        self.rotation = self.rotation.rotate_euler_degrees(delta_pitch, delta_yaw, delta_roll);
    }

    /// Rotate only around pitch (X-axis)
    pub fn rotate_x(&mut self, delta_pitch: f32) {
        self.rotation = self.rotation.rotate_x(delta_pitch);
    }

    /// Rotate only around yaw (Y-axis)
    pub fn rotate_y(&mut self, delta_yaw: f32) {
        self.rotation = self.rotation.rotate_y(delta_yaw);
    }

    /// Rotate only around roll (Z-axis)
    pub fn rotate_z(&mut self, delta_roll: f32) {
        self.rotation = self.rotation.rotate_z(delta_roll);
    }

    pub fn set_rotation(&mut self, q: Quaternion) {
        self.rotation = q.normalize();
    }

    pub fn rotation(&self) -> Quaternion {
        self.rotation
    }

    pub fn is_default(&self) -> bool {
        is_default_position(&self.position)
            && is_default_rotation(&self.rotation)
            && is_default_scale(&self.scale)
    }

    pub fn forward(&self) -> glam::Vec3 {
        self.rotation.to_glam_public() * glam::Vec3::new(0.0, 0.0, -1.0)
    }
    pub fn up(&self) -> glam::Vec3 {
        self.rotation.to_glam_public() * glam::Vec3::new(0.0, 1.0, 0.0)
    }
    pub fn right(&self) -> glam::Vec3 {
        self.rotation.to_glam_public() * glam::Vec3::new(1.0, 0.0, 0.0)
    }

    /// Returns a `glam::Mat4` representing scale→rotate→translate
    pub fn to_mat4(&self) -> Mat4 {
        let s = Mat4::from_scale(self.scale.to_glam_public());
        let r = Mat4::from_quat(self.rotation.to_glam_public());
        let t = Mat4::from_translation(self.position.to_glam_public());
        t * r * s
    }

    /// Create Transform3D from a Mat4 (decomposes scale → rotation → translation)
    pub fn from_mat4(m: Mat4) -> Self {
        let translation = Vector3::from_glam_public(m.w_axis.truncate());
        let scale = Vector3::from_glam_public(glam::Vec3::new(
            m.x_axis.length(),
            m.y_axis.length(),
            m.z_axis.length(),
        ));
        let inv_sx = if scale.x > 1e-6 { 1.0 / scale.x } else { 1.0 };
        let inv_sy = if scale.y > 1e-6 { 1.0 / scale.y } else { 1.0 };
        let inv_sz = if scale.z > 1e-6 { 1.0 / scale.z } else { 1.0 };
        let rot_mat = Mat4::from_cols(
            m.x_axis * inv_sx,
            m.y_axis * inv_sy,
            m.z_axis * inv_sz,
            glam::Vec4::W,
        );
        let rotation = Quaternion::from_glam_public(glam::Quat::from_mat4(&rot_mat));
        Self {
            position: translation,
            rotation,
            scale,
        }
    }

    /// Compose two transforms: result = self * other (other is applied in local space of self)
    pub fn multiply(&self, child: &Transform3D) -> Transform3D {
        let combined = self.to_mat4() * child.to_mat4();
        Self::from_mat4(combined)
    }

    /// Inverse transform (for world-to-local: local = parent_global.inverse() * desired_global)
    #[inline]
    pub fn inverse(&self) -> Transform3D {
        Self::from_mat4(self.to_mat4().inverse())
    }

    /// Calculate global transform from parent and local (same pattern as Transform2D)
    #[inline]
    pub fn calculate_global(parent_global: &Transform3D, local: &Transform3D) -> Transform3D {
        if parent_global.is_default() {
            return *local;
        }
        parent_global.multiply(local)
    }

    /// Batch calculate global transforms for multiple children
    pub fn batch_calculate_global(
        parent_global: &Transform3D,
        local_transforms: &[Transform3D],
    ) -> Vec<Transform3D> {
        if parent_global.is_default() {
            return local_transforms.to_vec();
        }
        let parent_mat = parent_global.to_mat4();
        local_transforms
            .iter()
            .map(|local| Self::from_mat4(parent_mat * local.to_mat4()))
            .collect()
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
