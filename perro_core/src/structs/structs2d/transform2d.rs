use serde::{Deserialize, Serialize};

use crate::structs2d::Vector2;

fn default_position() -> Vector2 {
    Vector2::zero()
}
fn is_default_position(v: &Vector2) -> bool {
    *v == default_position()
}

fn default_rotation() -> f32 {
    0.0
}
fn is_default_rotation(v: &f32) -> bool {
    *v == default_rotation()
}

fn default_scale() -> Vector2 {
    Vector2::one()
}
fn is_default_scale(v: &Vector2) -> bool {
    *v == default_scale()
}

#[derive(Serialize, Deserialize, Clone, Debug, Copy)]
pub struct Transform2D {
    #[serde(
        default = "default_position",
        skip_serializing_if = "is_default_position"
    )]
    pub position: Vector2,

    #[serde(
        default = "default_rotation",
        skip_serializing_if = "is_default_rotation"
    )]
    pub rotation: f32, // Rotation in radians

    #[serde(default = "default_scale", skip_serializing_if = "is_default_scale")]
    pub scale: Vector2,
}

impl Transform2D {
    pub fn new(pos: Vector2, rot: f32, scale: Vector2) -> Self {
        Self {
            position: pos,
            rotation: rot,
            scale: scale,
        }
    }
}

impl Default for Transform2D {
    fn default() -> Self {
        Self {
            position: default_position(),
            rotation: default_rotation(),
            scale: default_scale(),
        }
    }
}

impl Transform2D {
    /// Returns a `glam::Mat4` representing scale→rotate→translate
    pub fn to_mat4(&self) -> glam::Mat4 {
        let t =
            glam::Mat4::from_translation(glam::Vec3::new(self.position.x, self.position.y, 0.0));
        let r = glam::Mat4::from_rotation_z(self.rotation);
        let s = glam::Mat4::from_scale(glam::Vec3::new(self.scale.x, self.scale.y, 1.0));
        t * r * s
    }
}
