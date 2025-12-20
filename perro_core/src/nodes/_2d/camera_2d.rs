use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

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
    #[serde(rename = "base")]
    pub base: Node2D,
}

impl Camera2D {
    pub fn new() -> Self {
        let mut base = Node2D::new();
        base.name = Cow::Borrowed("Camera2D");
        Self {
            ty: Cow::Borrowed("Camera2D"),
            zoom: Some(1.0),
            active: false,
            base,
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
        &self.base
    }
}

impl DerefMut for Camera2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
