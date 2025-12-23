use crate::{Color, nodes::_3d::node_3d::Node3D, nodes::node_registry::NodeType};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DirectionalLight3D {
    #[serde(rename = "type")]
    pub ty: NodeType,

    #[serde(default = "Color::default")]
    pub color: Color,

    #[serde(default = "default_intensity")]
    pub intensity: f32,

    #[serde(rename = "base", default = "Node3D::default")]
    pub base: Node3D,
}

// ---------- Default fallback functions ----------

fn default_intensity() -> f32 {
    1.0
}

// ---------- Implement Default manually ----------

impl Default for DirectionalLight3D {
    fn default() -> Self {
        Self {
            ty: NodeType::DirectionalLight3D,
            color: Color::default(),
            intensity: default_intensity(),
            base: Node3D::default(),
        }
    }
}

// ---------- Convenience constructor ----------

impl DirectionalLight3D {
    pub fn new() -> Self {
        let mut base = Node3D::new();
        base.name = Cow::Borrowed("DirectionalLight3D");
        Self {
            base,
            ..Default::default()
        }
    }
}

// ---------- Deref forwarding ----------

impl Deref for DirectionalLight3D {
    type Target = Node3D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for DirectionalLight3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
