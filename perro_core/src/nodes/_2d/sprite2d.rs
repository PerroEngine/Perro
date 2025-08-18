use std::ops::{Deref, DerefMut};

use serde::{Serialize, Deserialize};
use wgpu::naga::Handle;
use crate::{impl_scene_node, nodes::_2d::node2d::Node2D};


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Sprite2D {
  #[serde(rename="type")] pub ty:   String,
  pub texture_path: Option<String>,

  pub region: Option<[f32;4]>,

  pub node2d: Node2D,
}

impl Sprite2D {
  pub fn new(name: &str, texture_path: Option<&str>) -> Self {
          Self {
              ty: "Sprite2D".into(),
              texture_path: texture_path.map(|s| s.to_string()),
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


impl_scene_node!(Sprite2D);