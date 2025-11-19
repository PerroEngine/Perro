use std::{
    borrow::Cow,
    ops::{Deref, DerefMut},
};

use serde::{Deserialize, Serialize};

use crate::{Color, Node3D};

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct OmniLight3D {
    #[serde(rename = "type")]
    pub ty: Cow<'static, str>,

    pub color: Color,
    pub intensity: f32,
    pub range: f32,

    pub node_3d: Node3D,
}

impl OmniLight3D {
    pub fn new(name: &str) -> Self {
        Self {
            ty: Cow::Borrowed("OmniLight3D"),
            color: Color::default(),
            intensity: 1.0,
            range: 10.0,
            node_3d: Node3D::new(name),
        }
    }
}

impl Deref for OmniLight3D {
    type Target = Node3D;
    fn deref(&self) -> &Self::Target {
        &self.node_3d
    }
}
impl DerefMut for OmniLight3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.node_3d
    }
}
