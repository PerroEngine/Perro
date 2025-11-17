use std::ops::{Deref, DerefMut};

use crate::nodes::_2d::node2d::Node2D;
use serde::{Deserialize, Serialize};

use std::borrow::Cow;

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct Sprite2D {
    #[serde(rename = "type")]
    pub ty: Cow<'static, str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub texture_path: Option<Cow<'static, str>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<[f32; 4]>,

    pub node2d: Node2D,
}

impl Sprite2D {
    pub fn new(name: &str, texture_path: Cow<'static, str>) -> Self {
        Self {
            ty: Cow::Borrowed("Sprite2D"),
            texture_path: Some(texture_path),
            region: None,
            node2d: Node2D::new(name),
        }
    }
}

impl Deref for Sprite2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.node2d
    }
}

impl DerefMut for Sprite2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.node2d
    }
}
