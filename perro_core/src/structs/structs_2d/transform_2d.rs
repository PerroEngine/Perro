use crate::Vector2;
use glam::{Mat3, Vec3};

/// A 2D transformation consisting of position, rotation, and scale.
#[derive(Debug, Clone, Copy)]
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

        // Extract scale from the basis vectors
        let scale_x = mat.x_axis.truncate().length();
        let scale_y = mat.y_axis.truncate().length();
        let scale = Vector2::new(scale_x, scale_y);

        // Extract rotation (atan2 of normalized basis)
        let rotation = (mat.x_axis.y / scale_x).atan2(mat.x_axis.x / scale_x);

        Self {
            position,
            scale,
            rotation,
        }
    }
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}
