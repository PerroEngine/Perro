use crate::{Matrix4, Quaternion, Vector3};
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

    /// Convert to a fast perro matrix backed by glam.
    #[inline]
    pub fn to_matrix4(&self) -> Matrix4 {
        Matrix4(self.to_mat4())
    }

    /// Create from a fast perro matrix backed by glam.
    #[inline]
    pub fn from_matrix4(matrix: Matrix4) -> Self {
        Self::from_mat4(matrix.0)
    }

    /// Create a transform looking at a target
    #[inline]
    pub fn looking_at(eye: Vector3, target: Vector3, up: Vector3) -> Self {
        Self {
            position: eye,
            scale: Vector3::ONE,
            rotation: Quaternion::looking_at(target - eye, up),
        }
    }

    /// Local forward axis in world space (`rotation * -Z`).
    #[inline]
    pub fn forward(&self) -> Vector3 {
        self.rotation.rotate_vector3(Vector3::new(0.0, 0.0, -1.0))
    }

    /// Local right axis in world space (`rotation * +X`).
    #[inline]
    pub fn right(&self) -> Vector3 {
        self.rotation.rotate_vector3(Vector3::new(1.0, 0.0, 0.0))
    }

    /// Local up axis in world space (`rotation * +Y`).
    #[inline]
    pub fn up(&self) -> Vector3 {
        self.rotation.rotate_vector3(Vector3::new(0.0, 1.0, 0.0))
    }
}

impl Default for Transform3D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looking_at_points_local_forward_at_target() {
        let cases = [
            (Vector3::new(3.0, 2.0, 5.0), Vector3::new(7.0, -1.0, -2.0)),
            (Vector3::new(-4.0, 0.5, 1.0), Vector3::new(-8.0, 3.0, 6.0)),
            (Vector3::ZERO, Vector3::new(0.0, 0.0, -10.0)),
        ];

        for (eye, target) in cases {
            let transform = Transform3D::looking_at(eye, target, Vector3::new(0.0, 1.0, 0.0));
            let forward = transform
                .rotation
                .rotate_vector3(Vector3::new(0.0, 0.0, -1.0));
            let expected = (target - eye).normalized();

            assert!((forward - expected).length() < 1.0e-5);
            assert_eq!(transform.position, eye);
            assert_eq!(transform.scale, Vector3::ONE);
        }
    }

    #[test]
    fn basis_axes_match_rotation_of_unit_vectors() {
        let target = Vector3::new(5.0, 0.0, 0.0);
        let transform = Transform3D::looking_at(Vector3::ZERO, target, Vector3::new(0.0, 1.0, 0.0));

        // Facing +X: forward points +X, right points +Z, up stays +Y.
        assert!((transform.forward() - Vector3::new(1.0, 0.0, 0.0)).length() < 1.0e-5);
        assert!((transform.right() - Vector3::new(0.0, 0.0, 1.0)).length() < 1.0e-5);
        assert!((transform.up() - Vector3::new(0.0, 1.0, 0.0)).length() < 1.0e-5);
    }

    #[test]
    fn identity_basis_uses_default_axes() {
        let t = Transform3D::IDENTITY;

        assert!((t.forward() - Vector3::new(0.0, 0.0, -1.0)).length() < 1.0e-6);
        assert!((t.right() - Vector3::new(1.0, 0.0, 0.0)).length() < 1.0e-6);
        assert!((t.up() - Vector3::new(0.0, 1.0, 0.0)).length() < 1.0e-6);
    }

    #[test]
    fn looking_at_same_point_uses_identity_rotation() {
        let eye = Vector3::new(1.0, 2.0, 3.0);
        let transform = Transform3D::looking_at(eye, eye, Vector3::new(0.0, 1.0, 0.0));

        assert_eq!(transform.rotation, Quaternion::IDENTITY);
    }
}
