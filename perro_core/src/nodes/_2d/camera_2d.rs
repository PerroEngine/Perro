use std::ops::{Deref, DerefMut};
use std::borrow::Cow;
use serde::{Deserialize, Serialize};

use crate::nodes::_2d::node_2d::Node2D;

/// 2D Camera node. Controls world-space rendering position and zoom.
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct Camera2D {
    #[serde(rename = "type")]
    pub ty: Cow<'static, str>,

    /// Zoom factor (1.0 = normal, >1.0 zoom in, <1.0 zoom out)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoom: Option<f32>,

    /// Whether this camera is currently active
    #[serde(default)]
    pub active: bool,

    /// The base Node2D containing transform, z-index, etc.
    pub node_2d: Node2D,
}

impl Camera2D {
    pub fn new(name: &str) -> Self {
        Self {
            ty: Cow::Borrowed("Camera2D"),
            zoom: Some(1.0),
            active: false,
            node_2d: Node2D::new(name),
        }
    }

    /// Get the zoom value (defaults to 1.0)
    pub fn zoom(&self) -> f32 {
        self.zoom.unwrap_or(1.0)
    }
}

impl Deref for Camera2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.node_2d
    }
}

impl DerefMut for Camera2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.node_2d
    }
}