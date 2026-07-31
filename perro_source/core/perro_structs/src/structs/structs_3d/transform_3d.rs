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

    /// Compose `parent ∘ local` directly in TRS space, skipping the
    /// `to_mat4` / matmul / `to_scale_rotation_translation` round-trip (and
    /// its sqrt-heavy decompose) on the hot transform-propagation path.
    ///
    /// Exact (up to fp rounding) whenever the parent scale is uniform
    /// (`sx == sy == sz`, negative-uniform point reflections included) or the
    /// child rotation is ~identity. The remaining case — non-uniform parent
    /// scale with a rotated child — produces shear that TRS cannot represent;
    /// there this falls back to the legacy matrix round-trip so behavior
    /// matches `from_mat4(parent * local)` exactly. Note both forms are
    /// approximations in the shear case (the matrix round-trip also discards
    /// shear at every store); the TRS path is drift-free for the
    /// representable cases.
    pub fn compose(parent: Self, local: Self) -> Self {
        // Translation lane is exact in every case.
        let position = parent.position
            + parent
                .rotation
                .rotate_vector3(parent.scale * local.position);

        let (sx, sy, sz) = (parent.scale.x, parent.scale.y, parent.scale.z);
        let mag = sx.abs().max(sy.abs()).max(sz.abs()).max(1.0);
        let uniform = (sx - sy).abs() <= 1.0e-6 * mag && (sy - sz).abs() <= 1.0e-6 * mag;
        if uniform {
            return Self {
                position,
                rotation: normalized_or_identity(parent.rotation.mul_quat(local.rotation)),
                scale: parent.scale * local.scale,
            };
        }
        if quat_is_near_identity(local.rotation) {
            return Self {
                position,
                rotation: parent.rotation,
                scale: parent.scale * local.scale,
            };
        }
        // Shear / mirror-non-uniform case: keep legacy matrix behavior.
        Self::from_mat4(parent.to_mat4() * local.to_mat4())
    }

    /// Compute `local = inverse(parent) ∘ global` directly in TRS space.
    ///
    /// Same exactness envelope as [`Self::compose`]; translation is exact in
    /// every case. Parent scale components with magnitude <= 1e-8 invert as
    /// `1.0` (matching the runtime's reparent guard) so degenerate parents
    /// stay finite instead of collapsing to NaN.
    pub fn inverse_compose(parent: Self, global: Self) -> Self {
        #[inline]
        fn safe_inv(value: f32) -> f32 {
            if value.abs() <= 1.0e-8 {
                1.0
            } else {
                1.0 / value
            }
        }

        let inv_scale = Vector3::new(
            safe_inv(parent.scale.x),
            safe_inv(parent.scale.y),
            safe_inv(parent.scale.z),
        );
        let inv_rot = parent.rotation.inverse();
        let position = inv_scale * inv_rot.rotate_vector3(global.position - parent.position);
        let scale = inv_scale * global.scale;

        let (sx, sy, sz) = (parent.scale.x, parent.scale.y, parent.scale.z);
        let mag = sx.abs().max(sy.abs()).max(sz.abs()).max(1.0);
        let uniform = (sx - sy).abs() <= 1.0e-6 * mag && (sy - sz).abs() <= 1.0e-6 * mag;
        let rel = normalized_or_identity(inv_rot.mul_quat(global.rotation));
        if uniform || quat_is_near_identity(rel) {
            return Self {
                position,
                rotation: rel,
                scale,
            };
        }
        // Shear case: legacy matrix path with the same degenerate-scale guard
        // the runtime reparent used (`inverse_basis_mat4`).
        let mut safe = parent;
        if safe.scale.x.abs() <= 1.0e-8 {
            safe.scale.x = 1.0;
        }
        if safe.scale.y.abs() <= 1.0e-8 {
            safe.scale.y = 1.0;
        }
        if safe.scale.z.abs() <= 1.0e-8 {
            safe.scale.z = 1.0;
        }
        Self::from_mat4(safe.to_mat4().inverse() * global.to_mat4())
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

/// Normalize a quaternion, falling back to identity for zero / non-finite
/// input (same guard as [`Transform3D::to_mat4`]).
#[inline]
fn normalized_or_identity(q: Quaternion) -> Quaternion {
    let glam: Quat = q.into();
    if glam.is_finite() && glam.length_squared() > 1.0e-8 {
        Quaternion::from_quat(glam.normalize())
    } else {
        Quaternion::IDENTITY
    }
}

/// True when `q` encodes a rotation within ~1e-4 rad of identity.
#[inline]
fn quat_is_near_identity(q: Quaternion) -> bool {
    let len_sq = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
    if !len_sq.is_finite() || len_sq <= 1.0e-8 {
        return false;
    }
    // sin^2(theta/2) <= eps  <=>  w^2 >= len_sq * (1 - eps)
    q.w * q.w >= len_sq * (1.0 - 1.0e-9)
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

    fn assert_mat4_close(actual: Mat4, expected: Mat4, eps: f32) {
        for (actual, expected) in actual
            .to_cols_array()
            .into_iter()
            .zip(expected.to_cols_array())
        {
            assert!(
                (actual - expected).abs() <= eps,
                "{actual} != {expected} (eps {eps})"
            );
        }
    }

    fn quat_from_axis_angle(axis: Vector3, angle: f32) -> Quaternion {
        Quaternion::from_quat(Quat::from_axis_angle(
            Vec3::new(axis.x, axis.y, axis.z).normalize(),
            angle,
        ))
    }

    #[test]
    fn compose_matches_matrix_product_uniform_scale() {
        let parent = Transform3D::new(
            Vector3::new(3.0, -2.0, 1.0),
            quat_from_axis_angle(Vector3::new(1.0, 2.0, -0.5), 0.8),
            Vector3::new(2.0, 2.0, 2.0),
        );
        let local = Transform3D::new(
            Vector3::new(-1.5, 4.0, 0.25),
            quat_from_axis_angle(Vector3::new(-0.3, 1.0, 0.7), -1.1),
            Vector3::new(0.5, 3.0, 1.25),
        );
        let composed = Transform3D::compose(parent, local);
        assert_mat4_close(
            composed.to_mat4(),
            parent.to_mat4() * local.to_mat4(),
            1.0e-4,
        );
    }

    #[test]
    fn compose_matches_matrix_product_rotation_only_and_translation_only() {
        let rot_p = Transform3D::new(
            Vector3::ZERO,
            quat_from_axis_angle(Vector3::new(0.0, 1.0, 0.0), 1.2),
            Vector3::ONE,
        );
        let rot_l = Transform3D::new(
            Vector3::ZERO,
            quat_from_axis_angle(Vector3::new(1.0, 0.0, 1.0), -0.7),
            Vector3::ONE,
        );
        let composed = Transform3D::compose(rot_p, rot_l);
        assert_mat4_close(
            composed.to_mat4(),
            rot_p.to_mat4() * rot_l.to_mat4(),
            1.0e-5,
        );

        let tr_p = Transform3D::new(
            Vector3::new(5.0, -7.0, 2.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        );
        let tr_l = Transform3D::new(
            Vector3::new(-2.0, 3.0, 9.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        );
        let composed = Transform3D::compose(tr_p, tr_l);
        assert_mat4_close(composed.to_mat4(), tr_p.to_mat4() * tr_l.to_mat4(), 1.0e-6);
    }

    #[test]
    fn compose_matches_matrix_product_nonuniform_scale_unrotated_child() {
        let parent = Transform3D::new(
            Vector3::new(1.0, 2.0, -3.0),
            quat_from_axis_angle(Vector3::new(0.2, 1.0, 0.4), 0.9),
            Vector3::new(3.0, 0.5, 1.5),
        );
        let local = Transform3D::new(
            Vector3::new(-4.0, 6.0, 2.0),
            Quaternion::IDENTITY,
            Vector3::new(2.0, 5.0, 0.75),
        );
        let composed = Transform3D::compose(parent, local);
        assert_mat4_close(
            composed.to_mat4(),
            parent.to_mat4() * local.to_mat4(),
            1.0e-4,
        );
    }

    /// Non-uniform parent scale + rotated child is not TRS-representable
    /// (shear). Both the legacy mat-roundtrip and compose discard the shear;
    /// compose falls back to the legacy path so they agree exactly.
    #[test]
    fn compose_shear_case_matches_legacy_roundtrip() {
        let parent = Transform3D::new(
            Vector3::new(1.0, -1.0, 0.5),
            quat_from_axis_angle(Vector3::new(0.0, 1.0, 0.0), 0.7),
            Vector3::new(3.0, 0.5, 1.0),
        );
        let local = Transform3D::new(
            Vector3::new(2.0, 2.0, -2.0),
            quat_from_axis_angle(Vector3::new(1.0, 0.0, 0.0), 1.1),
            Vector3::new(1.0, 2.0, 1.0),
        );
        let composed = Transform3D::compose(parent, local);
        let legacy = Transform3D::from_mat4(parent.to_mat4() * local.to_mat4());
        assert_mat4_close(composed.to_mat4(), legacy.to_mat4(), 1.0e-5);
    }

    #[test]
    fn inverse_compose_roundtrips_through_compose() {
        let cases = [
            // uniform parent
            (
                Transform3D::new(
                    Vector3::new(3.0, -2.0, 1.0),
                    quat_from_axis_angle(Vector3::new(1.0, 2.0, -0.5), 0.8),
                    Vector3::new(2.0, 2.0, 2.0),
                ),
                Transform3D::new(
                    Vector3::new(-1.5, 4.0, 0.25),
                    quat_from_axis_angle(Vector3::new(-0.3, 1.0, 0.7), -1.1),
                    Vector3::new(0.5, 3.0, 1.25),
                ),
            ),
            // non-uniform parent, matching (unrotated-relative) child
            (
                Transform3D::new(
                    Vector3::new(0.0, 1.0, 2.0),
                    quat_from_axis_angle(Vector3::new(0.1, 1.0, 0.0), 0.5),
                    Vector3::new(3.0, 0.5, 2.0),
                ),
                Transform3D::new(
                    Vector3::new(4.0, -4.0, 1.0),
                    quat_from_axis_angle(Vector3::new(0.1, 1.0, 0.0), 0.5),
                    Vector3::new(2.0, 1.0, 0.5),
                ),
            ),
        ];
        for (parent, global) in cases {
            let local = Transform3D::inverse_compose(parent, global);
            let rebuilt = Transform3D::compose(parent, local);
            assert_mat4_close(rebuilt.to_mat4(), global.to_mat4(), 1.0e-4);
        }
    }
}
