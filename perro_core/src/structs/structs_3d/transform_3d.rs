use crate::{Quaternion, Vector3};
use glam::Mat4;

#[derive(Debug, Clone, Copy)]
pub struct Transform3D {
    pub position: Vector3,
    pub scale: Vector3,
    pub rotation: Quaternion,
}

impl Transform3D {
    pub const IDENTITY: Self = Self {
        position: Vector3::ZERO,
        scale: Vector3::ONE,
        rotation: Quaternion::IDENTITY,
    };

    #[inline]
    pub const fn new(pos: Vector3, rot: Quaternion, scale: Vector3) -> Self {
        Self {
            position: pos,
            scale,
            rotation: rot,
        }
    }

    /// Convert to a Mat4 for transformations (TRS order)
    #[inline]
    pub fn to_mat4(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            self.scale.into(),
            self.rotation.into(),
            self.position.into(),
        )
    }

    /// Create from a Mat4 (extracts TRS components)
    #[inline]
    pub fn from_mat4(mat: Mat4) -> Self {
        let (scale, rotation, position) = mat.to_scale_rotation_translation();

        Self {
            position: position.into(),
            scale: scale.into(),
            rotation: rotation.into(),
        }
    }

    /// Create a transform looking at a target
    #[inline]
    pub fn looking_at(eye: Vector3, target: Vector3, up: Vector3) -> Self {
        let mat = Mat4::look_at_rh(eye.into(), target.into(), up.into());
        let rotation = glam::Quat::from_mat4(&mat);

        Self {
            position: eye,
            scale: Vector3::ONE,
            rotation: rotation.into(),
        }
    }
}

impl Default for Transform3D {
    fn default() -> Self {
        Self::IDENTITY
    }
}
