use crate::{Color, nodes::_3d::node_3d::Node3D};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DirectionalLight3D {
    #[serde(rename = "type", default = "default_type")]
    pub ty: Cow<'static, str>,

    #[serde(default = "Color::default")]
    pub color: Color,

    #[serde(default = "default_intensity")]
    pub intensity: f32,

    #[serde(default = "Node3D::default")]
    pub node_3d: Node3D,
}

// ---------- Default fallback functions ----------

fn default_type() -> Cow<'static, str> {
    Cow::Borrowed("DirectionalLight3D")
}

fn default_intensity() -> f32 {
    1.0
}

// ---------- Implement Default manually ----------

impl Default for DirectionalLight3D {
    fn default() -> Self {
        Self {
            ty: default_type(),
            color: Color::default(),
            intensity: default_intensity(),
            node_3d: Node3D::default(),
        }
    }
}

// ---------- Convenience constructor ----------

impl DirectionalLight3D {
    pub fn new(name: &str) -> Self {
        Self {
            node_3d: Node3D::new(name),
            ..Default::default()
        }
    }
}

// ---------- Deref forwarding ----------

impl Deref for DirectionalLight3D {
    type Target = Node3D;
    fn deref(&self) -> &Self::Target {
        &self.node_3d
    }
}

impl DerefMut for DirectionalLight3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.node_3d
    }
}
