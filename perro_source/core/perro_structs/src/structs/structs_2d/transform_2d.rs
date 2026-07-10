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
}
