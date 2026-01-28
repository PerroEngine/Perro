use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

use crate::nodes::_2d::node_2d::Node2D;
use crate::nodes::node_registry::NodeType;

/// 2D Camera node. Controls world-space rendering position and zoom.
// Optimized field order: ty (1 byte), active (1 byte), zoom (4 bytes), base (large)
// Groups small fields together to minimize padding
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct Camera2D {
    #[serde(rename = "type")]
    pub ty: NodeType,

    /// Whether this camera is currently active
    #[serde(default)]
    pub active: bool,

    /// Zoom factor (0.0 = normal, positive = zoom in, negative = zoom out)
    #[serde(default)]
    pub zoom: f32,

    /// The base Node2D containing transform, z-index, etc.
    #[serde(rename = "base")]
    pub base: Node2D,
}

impl Camera2D {
    pub fn new() -> Self {
        let mut base = Node2D::new();
        base.name = Cow::Borrowed("Camera2D");
        Self {
            ty: NodeType::Camera2D,
            active: false,
            zoom: 0.0,
            base,
        }
    }

    /// Get the zoom value directly (0.0 = normal, positive = zoom in, negative = zoom out)
    pub fn zoom(&self) -> f32 {
        self.zoom
    }
}

impl Deref for Camera2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Camera2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
