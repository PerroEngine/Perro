use std::{
    borrow::Cow,
    ops::{Deref, DerefMut},
};

use serde::{Deserialize, Serialize};

use crate::{Color, Node3D};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SpotLight3D {
    #[serde(rename = "type", default = "default_type")]
    pub ty: Cow<'static, str>,

    #[serde(default = "Color::default")]
    pub color: Color,

    #[serde(default = "default_intensity")]
    pub intensity: f32,

    #[serde(default = "default_range")]
    pub range: f32,

    #[serde(default = "default_inner_angle")]
    pub inner_angle: f32,

    #[serde(default = "default_outer_angle")]
    pub outer_angle: f32,

    #[serde(rename = "base", default = "Node3D::default")]
    pub base: Node3D,
}

// ---------- Default fallback values ----------

fn default_type() -> Cow<'static, str> {
    Cow::Borrowed("SpotLight3D")
}

fn default_intensity() -> f32 {
    1.0
}

fn default_range() -> f32 {
    15.0
}

fn default_inner_angle() -> f32 {
    25.0
}

fn default_outer_angle() -> f32 {
    45.0
}

// ---------- Implement Default ----------

impl Default for SpotLight3D {
    fn default() -> Self {
        Self {
            ty: default_type(),
            color: Color::default(),
            intensity: default_intensity(),
            range: default_range(),
            inner_angle: default_inner_angle(),
            outer_angle: default_outer_angle(),
            base: Node3D::default(),
        }
    }
}

// ---------- Convenience Constructor ----------

impl SpotLight3D {
    pub fn new() -> Self {
        let mut base = Node3D::new();
        base.name = Cow::Borrowed("SpotLight3D");
        Self {
            base,
            ..Default::default()
        }
    }
}

// ---------- Deref forwarding ----------

impl Deref for SpotLight3D {
    type Target = Node3D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for SpotLight3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
