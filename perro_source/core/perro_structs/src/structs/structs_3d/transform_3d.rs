use crate::{Quaternion, Vector3};
use glam::{Mat3, Mat4, Quat, Vec3};

#[derive(Debug, Clone, Copy, PartialEq)]
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
        let rotation = {
            let q: Quat = self.rotation.into();
            if q.is_finite() && q.length_squared() > 1.0e-8 {
                q.normalize()
            } else {
                Quat::IDENTITY
            }
        };
        Mat4::from_scale_rotation_translation(self.scale.into(), rotation, self.position.into())
    }

    /// Create from a Mat4 (extracts TRS components)
    #[inline]
    pub fn from_mat4(mat: Mat4) -> Self {
        let (_, _, position) = mat.to_scale_rotation_translation();

        let basis = Mat3::from_mat4(mat);
        let mut x = basis.x_axis;
        let mut y = basis.y_axis;
        let mut z = basis.z_axis;

        let mut sx = x.length();
        let mut sy = y.length();
        let mut sz = z.length();

        // Guard against degenerate transforms.
        if sx <= 1.0e-8 {
            sx = 1.0;
            x = Vec3::X;
        }
        if sy <= 1.0e-8 {
            sy = 1.0;
            y = Vec3::Y;
        }
        if sz <= 1.0e-8 {
            sz = 1.0;
            z = Vec3::Z;
        }

        // Preserve handedness by assigning the sign to one axis.
        let det = x.cross(y).dot(z);
        if det < 0.0 {
            sx = -sx;
            x = -x;
        }

        let rot_basis = Mat3::from_cols(x / sx.abs(), y / sy.abs(), z / sz.abs());
        let rotation = glam::Quat::from_mat3(&rot_basis).normalize();
        let scale = Vec3::new(sx, sy, sz);

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
