use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

use crate::nodes::_3d::node_3d::Node3D;

/// 3D Camera node. Controls the view and projection for 3D rendering.
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct Camera3D {
    #[serde(rename = "type")]
    pub ty: Cow<'static, str>,

    /// Field of view, in degrees (typically 70°–90°)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fov: Option<f32>,

    /// Near clipping distance (default = 0.1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub near: Option<f32>,

    /// Far clipping distance (default = 1000.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub far: Option<f32>,

    /// Whether this camera is currently active
    #[serde(default)]
    pub active: bool,

    /// Embedded base Node3D (provides transform, visibility, etc.)
    pub node_3d: Node3D,
}

impl Camera3D {
    /// Creates a new Camera3D with standard default parameters.
    pub fn new(name: &str) -> Self {
        Self {
            ty: Cow::Borrowed("Camera3D"),
            fov: Some(70.0),
            near: Some(0.1),
            far: Some(1000.0),
            active: false,
            node_3d: Node3D::new(name),
        }
    }

    /// Returns the camera's field of view in degrees
    pub fn fov(&self) -> f32 {
        self.fov.unwrap_or(70.0)
    }

    /// Returns the near clip distance
    pub fn near(&self) -> f32 {
        self.near.unwrap_or(0.1)
    }

    /// Returns the far clip distance
    pub fn far(&self) -> f32 {
        self.far.unwrap_or(1000.0)
    }

    /// Computes the view matrix from the node's transform.
    pub fn view_matrix(&self) -> glam::Mat4 {
        let pos = self.node_3d.transform.position.to_glam();
        let rot = self.node_3d.transform.rotation.to_glam();
        glam::Mat4::from_rotation_translation(rot, pos).inverse()
    }

    /// Builds a perspective projection matrix
    pub fn projection_matrix(&self, aspect_ratio: f32) -> glam::Mat4 {
        glam::Mat4::perspective_rh_gl(
            self.fov().to_radians(),
            aspect_ratio,
            self.near(),
            self.far(),
        )
    }
}

impl Deref for Camera3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.node_3d
    }
}

impl DerefMut for Camera3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.node_3d
    }
}
