use std::{
    borrow::Cow,
    ops::{Deref, DerefMut},
};

use serde::{Deserialize, Serialize};

use crate::ids::LightID;
use crate::{Color, Node3D, nodes::node_registry::NodeType};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OmniLight3D {
    #[serde(rename = "type")]
    pub ty: NodeType,

    pub color: Color,
    pub intensity: f32,
    pub range: f32,

    #[serde(rename = "base")]
    pub base: Node3D,

    /// Runtime-only: allocated from Graphics.light_manager when first queued.
    #[serde(skip)]
    pub light_id: Option<LightID>,
}

impl Default for OmniLight3D {
    fn default() -> Self {
        Self {
            ty: NodeType::OmniLight3D,
            color: Color::default(),
            intensity: 1.0,
            range: 10.0,
            base: Node3D::default(),
            light_id: None,
        }
    }
}

impl OmniLight3D {
    pub fn new() -> Self {
        let mut base = Node3D::new();
        base.name = Cow::Borrowed("OmniLight3D");
        Self {
            ty: NodeType::OmniLight3D,
            color: Color::default(),
            intensity: 1.0,
            range: 10.0,
            base,
            light_id: None,
        }
    }
}

impl Deref for OmniLight3D {
    type Target = Node3D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}
impl DerefMut for OmniLight3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
