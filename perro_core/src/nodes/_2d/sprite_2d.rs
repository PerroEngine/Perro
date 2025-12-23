use std::ops::{Deref, DerefMut};

use crate::nodes::_2d::node_2d::Node2D;
use crate::nodes::node_registry::NodeType;
use serde::{Deserialize, Serialize};

use std::borrow::Cow;

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct Sprite2D {
    #[serde(rename = "type")]
    pub ty: NodeType,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub texture_path: Option<Cow<'static, str>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<[f32; 4]>,

    #[serde(rename = "base")]
    pub base: Node2D,
}

impl Sprite2D {
    pub fn new() -> Self {
        let mut base = Node2D::new();
        base.name = Cow::Borrowed("Sprite2D");
        Self {
            ty: NodeType::Sprite2D,
            texture_path: None,
            region: None,
            base,
        }
    }
}

impl Deref for Sprite2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Sprite2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
