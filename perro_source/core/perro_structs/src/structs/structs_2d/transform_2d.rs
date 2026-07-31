use crate::{Matrix3, Vector2};
use glam::{Mat3, Vec3};

/// A 2D transformation consisting of position, rotation, and scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform2D {
    pub position: Vector2,
    pub scale: Vector2,
    pub rotation: f32,
}

impl Transform2D {
    pub const IDENTITY: Self = Self {
        position: Vector2::ZERO,
        scale: Vector2::ONE,
        rotation: 0.0,
    };

    #[inline]
    pub const fn new(pos: Vector2, rot: f32, scale: Vector2) -> Self {
        Self {
            position: pos,
            scale,
            rotation: rot,
        }
    }

    /// Convert to a Mat3 for transformations (TRS order)
    #[inline]
    pub fn to_mat3(&self) -> Mat3 {
        let cos = self.rotation.cos();
        let sin = self.rotation.sin();

        // Manual construction is faster than Mat3::from_scale_angle_translation
        // Column-major order: [col0, col1, col2]
        Mat3::from_cols(
            Vec3::new(cos * self.scale.x, sin * self.scale.x, 0.0),
            Vec3::new(-sin * self.scale.y, cos * self.scale.y, 0.0),
            Vec3::new(self.position.x, self.position.y, 1.0),
        )
    }

    /// Create from a Mat3 (extracts TRS components)
    #[inline]
    pub fn from_mat3(mat: Mat3) -> Self {
        let position = Vector2::new(mat.z_axis.x, mat.z_axis.y);

        let x_axis = mat.x_axis.truncate();
        let y_axis = mat.y_axis.truncate();
        let scale_x = x_axis.length();
        let mut scale_y = y_axis.length();
        if x_axis.perp_dot(y_axis) < 0.0 {
            scale_y = -scale_y;
        }

        // Either basis axis can recover rotation. Avoid normalizing a zero
        // axis so collapsed transforms still produce a finite angle.
        let rotation = if scale_x > 0.0 {
            x_axis.y.atan2(x_axis.x)
        } else if scale_y != 0.0 {
            (-y_axis.x).atan2(y_axis.y)
        } else {
            0.0
        };
        let scale = Vector2::new(scale_x, scale_y);

        Self {
            position,
            scale,
            rotation,
        }
    }

    /// Compose `parent ∘ local` directly in TRS space, skipping the
    /// `to_mat3` / matmul / `from_mat3` round-trip (and its `atan2` +
    /// `sqrt` decompose) on the hot transform-propagation path.
    ///
    /// Exact (up to fp rounding) whenever the parent scale has uniform
    /// magnitude (`|sx| == |sy|`, mirrors included: a reflection flips the
    /// child's spin direction) or the child rotation is ~zero. The remaining
    /// case — non-uniform parent scale with a rotated child — produces shear
    /// that TRS cannot represent; there this falls back to the legacy matrix
    /// round-trip so behavior matches `from_mat3(parent * local)` exactly.
    /// Note both forms are approximations in the shear case (the matrix
    /// round-trip also discards shear at every store); the TRS path is
    /// drift-free for the representable cases.
    pub fn compose(parent: Self, local: Self) -> Self {
        let (sin, cos) = parent.rotation.sin_cos();
        let sx = parent.scale.x * local.position.x;
        let sy = parent.scale.y * local.position.y;
        // Translation lane is exact in every case.
        let position = Vector2::new(
            parent.position.x + cos * sx - sin * sy,
            parent.position.y + sin * sx + cos * sy,
        );

        let ax = parent.scale.x.abs();
        let ay = parent.scale.y.abs();
        let uniform = (ax - ay).abs() <= 1.0e-6 * ax.max(ay).max(1.0);
        if uniform {
            let mirrored = (parent.scale.x < 0.0) != (parent.scale.y < 0.0);
            let rotation = if mirrored {
                // S_p * R(a) == R(-a) * S_p for uniform-magnitude mirrors.
                parent.rotation - local.rotation
            } else {
                parent.rotation + local.rotation
            };
            return Self {
                position,
                rotation: wrap_angle(rotation),
                scale: Vector2::new(
                    parent.scale.x * local.scale.x,
                    parent.scale.y * local.scale.y,
                ),
            };
        }
        if local.rotation.abs() <= 1.0e-6 {
            return Self {
                position,
                rotation: wrap_angle(parent.rotation + local.rotation),
                scale: Vector2::new(
                    parent.scale.x * local.scale.x,
                    parent.scale.y * local.scale.y,
                ),
            };
        }
        // Shear case: keep legacy behavior bit-for-bit in spirit.
        Self::from_mat3(parent.to_mat3() * local.to_mat3())
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

        let inv_sx = safe_inv(parent.scale.x);
        let inv_sy = safe_inv(parent.scale.y);
        let (sin, cos) = parent.rotation.sin_cos();
        let dx = global.position.x - parent.position.x;
        let dy = global.position.y - parent.position.y;
        // R(-parent.rotation) * delta, then component-wise inverse scale.
        let position = Vector2::new(
            (cos * dx + sin * dy) * inv_sx,
            (-sin * dx + cos * dy) * inv_sy,
        );
        let scale = Vector2::new(global.scale.x * inv_sx, global.scale.y * inv_sy);

        let ax = parent.scale.x.abs();
        let ay = parent.scale.y.abs();
        let uniform = (ax - ay).abs() <= 1.0e-6 * ax.max(ay).max(1.0);
        if uniform {
            let mirrored = (parent.scale.x < 0.0) != (parent.scale.y < 0.0);
            let rotation = if mirrored {
                parent.rotation - global.rotation
            } else {
                global.rotation - parent.rotation
            };
            return Self {
                position,
                rotation: wrap_angle(rotation),
                scale,
            };
        }
        let rel = global.rotation - parent.rotation;
        if rel.abs() <= 1.0e-6 {
            return Self {
                position,
                rotation: rel,
                scale,
            };
        }
        // Shear case: legacy matrix path.
        let mut safe = parent;
        if safe.scale.x.abs() <= 1.0e-8 {
            safe.scale.x = 1.0;
        }
        if safe.scale.y.abs() <= 1.0e-8 {
            safe.scale.y = 1.0;
        }
        Self::from_mat3(safe.to_mat3().inverse() * global.to_mat3())
    }

    /// Convert to a fast perro matrix backed by glam.
    #[inline]
    pub fn to_matrix3(&self) -> Matrix3 {
        Matrix3(self.to_mat3())
    }

    /// Create from a fast perro matrix backed by glam.
    #[inline]
    pub fn from_matrix3(matrix: Matrix3) -> Self {
        Self::from_mat3(matrix.0)
    }
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Wrap an angle into `(-PI, PI]`, matching the `atan2` output range of the
/// legacy `from_mat3` decompose so composed globals keep the same rotation
/// representation scripts observed before.
#[inline]
fn wrap_angle(angle: f32) -> f32 {
    if angle > -std::f32::consts::PI && angle <= std::f32::consts::PI {
        return angle;
    }
    let mut wrapped = angle % std::f32::consts::TAU;
    if wrapped > std::f32::consts::PI {
        wrapped -= std::f32::consts::TAU;
    } else if wrapped <= -std::f32::consts::PI {
        wrapped += std::f32::consts::TAU;
    }
    wrapped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_mat3_close(actual: Mat3, expected: Mat3) {
        for (actual, expected) in actual
            .to_cols_array()
            .into_iter()
            .zip(expected.to_cols_array())
        {
            assert!(
                (actual - expected).abs() <= 1.0e-6,
                "{actual} != {expected}"
            );
        }
    }

    #[test]
    fn from_mat3_preserves_reflected_transform() {
        let source = Transform2D::new(Vector2::new(3.0, -2.0), 0.35, Vector2::new(-2.0, 4.0));
        let rebuilt = Transform2D::from_mat3(source.to_mat3());

        assert_mat3_close(rebuilt.to_mat3(), source.to_mat3());
    }

    #[test]
    fn from_mat3_handles_collapsed_x_axis() {
        let source = Transform2D::new(Vector2::ZERO, 0.7, Vector2::new(0.0, 2.0));
        let rebuilt = Transform2D::from_mat3(source.to_mat3());

        assert!(rebuilt.rotation.is_finite());
        assert_mat3_close(rebuilt.to_mat3(), source.to_mat3());
    }

    fn assert_mat3_close_eps(actual: Mat3, expected: Mat3, eps: f32) {
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

    /// compose(p, l) must agree with the legacy mat-roundtrip result within
    /// epsilon: exact-representable cases match the raw matrix product, and
    /// the shear case matches the legacy `from_mat3(p * l)` snap.
    fn assert_compose_matches(parent: Transform2D, local: Transform2D, eps: f32) {
        let composed = Transform2D::compose(parent, local);
        let legacy = Transform2D::from_mat3(parent.to_mat3() * local.to_mat3());
        assert_mat3_close_eps(composed.to_mat3(), legacy.to_mat3(), eps);
    }

    #[test]
    fn compose_matches_matrix_product_uniform_scale() {
        let cases = [
            (
                Transform2D::new(Vector2::new(3.0, -2.0), 0.35, Vector2::new(2.0, 2.0)),
                Transform2D::new(Vector2::new(-1.5, 4.0), -1.2, Vector2::new(0.5, 0.5)),
            ),
            (
                Transform2D::new(Vector2::new(10.0, 20.0), 2.9, Vector2::new(0.25, 0.25)),
                Transform2D::new(Vector2::new(7.0, -3.0), 0.8, Vector2::new(3.0, 1.5)),
            ),
        ];
        for (parent, local) in cases {
            let composed = Transform2D::compose(parent, local);
            assert_mat3_close_eps(
                composed.to_mat3(),
                parent.to_mat3() * local.to_mat3(),
                1.0e-4,
            );
        }
    }

    #[test]
    fn compose_matches_matrix_product_rotation_only_and_translation_only() {
        let rot_p = Transform2D::new(Vector2::ZERO, 1.1, Vector2::ONE);
        let rot_l = Transform2D::new(Vector2::ZERO, -0.6, Vector2::ONE);
        let composed = Transform2D::compose(rot_p, rot_l);
        assert_mat3_close_eps(
            composed.to_mat3(),
            rot_p.to_mat3() * rot_l.to_mat3(),
            1.0e-5,
        );

        let tr_p = Transform2D::new(Vector2::new(5.0, -7.0), 0.0, Vector2::ONE);
        let tr_l = Transform2D::new(Vector2::new(-2.0, 3.0), 0.0, Vector2::ONE);
        let composed = Transform2D::compose(tr_p, tr_l);
        assert_mat3_close_eps(composed.to_mat3(), tr_p.to_mat3() * tr_l.to_mat3(), 1.0e-6);
    }

    #[test]
    fn compose_matches_matrix_product_nonuniform_scale_unrotated_child() {
        let parent = Transform2D::new(Vector2::new(1.0, 2.0), 0.9, Vector2::new(3.0, 0.5));
        let local = Transform2D::new(Vector2::new(-4.0, 6.0), 0.0, Vector2::new(2.0, 5.0));
        let composed = Transform2D::compose(parent, local);
        assert_mat3_close_eps(
            composed.to_mat3(),
            parent.to_mat3() * local.to_mat3(),
            1.0e-4,
        );
    }

    #[test]
    fn compose_matches_matrix_product_mirrored_parent_with_rotated_child() {
        // scale.x = -1 sprite flip: the reflection must flip the child's spin.
        let parent = Transform2D::new(Vector2::new(2.0, 1.0), 0.4, Vector2::new(-1.0, 1.0));
        let local = Transform2D::new(Vector2::new(0.5, -0.25), 0.9, Vector2::new(2.0, 2.0));
        let composed = Transform2D::compose(parent, local);
        assert_mat3_close_eps(
            composed.to_mat3(),
            parent.to_mat3() * local.to_mat3(),
            1.0e-5,
        );
    }

    /// Non-uniform parent scale + rotated child is not TRS-representable
    /// (shear). Both the legacy mat-roundtrip and compose discard the shear;
    /// compose falls back to the legacy path so they agree exactly.
    #[test]
    fn compose_shear_case_matches_legacy_roundtrip() {
        let parent = Transform2D::new(Vector2::new(1.0, -1.0), 0.7, Vector2::new(3.0, 0.5));
        let local = Transform2D::new(Vector2::new(2.0, 2.0), 1.1, Vector2::new(1.0, 2.0));
        assert_compose_matches(parent, local, 1.0e-5);
    }

    #[test]
    fn compose_wraps_rotation_into_atan2_range() {
        // Legacy mat-roundtrip globals always came out of atan2 in (-PI, PI];
        // the TRS fast path must keep that invariant for scripts reading
        // global rotation.
        let parent = Transform2D::new(Vector2::ZERO, 2.8, Vector2::new(2.0, 2.0));
        let local = Transform2D::new(Vector2::ZERO, 2.8, Vector2::ONE);
        let composed = Transform2D::compose(parent, local);
        assert!(composed.rotation > -std::f32::consts::PI);
        assert!(composed.rotation <= std::f32::consts::PI);
        assert_mat3_close_eps(
            composed.to_mat3(),
            parent.to_mat3() * local.to_mat3(),
            1.0e-5,
        );
    }

    #[test]
    fn inverse_compose_roundtrips_through_compose() {
        let cases = [
            // uniform parent
            (
                Transform2D::new(Vector2::new(3.0, -2.0), 0.35, Vector2::new(2.0, 2.0)),
                Transform2D::new(Vector2::new(-1.5, 4.0), -1.2, Vector2::new(0.5, 3.0)),
            ),
            // mirrored uniform parent
            (
                Transform2D::new(Vector2::new(-1.0, 5.0), 1.4, Vector2::new(-2.0, 2.0)),
                Transform2D::new(Vector2::new(2.0, 2.0), 0.6, Vector2::new(1.5, 1.5)),
            ),
            // non-uniform parent, zero relative rotation (unrotated child)
            (
                Transform2D::new(Vector2::new(0.0, 1.0), 0.5, Vector2::new(3.0, 0.5)),
                Transform2D::new(Vector2::new(4.0, -4.0), 0.5, Vector2::new(2.0, 1.0)),
            ),
        ];
        for (parent, global) in cases {
            let local = Transform2D::inverse_compose(parent, global);
            let rebuilt = Transform2D::compose(parent, local);
            assert_mat3_close_eps(rebuilt.to_mat3(), global.to_mat3(), 1.0e-4);
        }
    }
}
