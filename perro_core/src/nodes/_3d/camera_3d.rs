use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

use crate::nodes::_3d::node_3d::Node3D;
use crate::nodes::node_registry::NodeType;

/// 3D Camera node. Controls the view and projection for 3D rendering.
// Optimized field order: ty (1 byte), active (1 byte), then Option<f32> fields (12 bytes), base (large)
// Groups small fields together to minimize padding
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct Camera3D {
    #[serde(rename = "type")]
    pub ty: NodeType,

    /// Whether this camera is currently active
    #[serde(default)]
    pub active: bool,

    /// Field of view, in degrees (typically 70°–90°)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fov: Option<f32>,

    /// Near clipping distance (default = 0.1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub near: Option<f32>,

    /// Far clipping distance (default = 1000.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub far: Option<f32>,

    /// Embedded base Node3D (provides transform, visibility, etc.)
    #[serde(rename = "base")]
    pub base: Node3D,
}

impl Camera3D {
    /// Creates a new Camera3D with standard default parameters.
    pub fn new() -> Self {
        Self {
            ty: NodeType::Camera3D,
            active: false,
            fov: Some(70.0),
            near: Some(0.1),
            far: Some(1000.0),
            base: {
                // Use nil ID for graphics-only cameras that aren't part of the scene tree
                let mut base = Node3D::new_with_nil_id();
                base.name = Cow::Borrowed("Camera3D");
                base
            },
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
        let pos = self.transform.position.to_glam_public();
        let rot = self.transform.rotation.to_glam_public();
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
        &self.base
    }
}

impl DerefMut for Camera3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
