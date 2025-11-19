use crate::{Color, nodes::_3d::node_3d::Node3D};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct DirectionalLight3D {
    #[serde(rename = "type")]
    pub ty: Cow<'static, str>,

    pub color: Color,
    pub intensity: f32,

    pub node_3d: Node3D,
}

impl DirectionalLight3D {
    pub fn new(name: &str) -> Self {
        Self {
            ty: Cow::Borrowed("DirectionalLight3D"),
            color: Color::default(),
            intensity: 1.0,
            node_3d: Node3D::new(name),
        }
    }
}

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
