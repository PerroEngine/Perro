use serde::{Serialize, Deserialize};
use crate::Transform2D;
use crate::{nodes::node::Node};
use std::ops::{Deref, DerefMut};


fn default_visible() -> bool { true }
fn is_default_visible(v: &bool) -> bool { *v == default_visible() }



#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Node2D {
    #[serde(rename="type")] pub ty:   String,

    pub transform: Transform2D,

    #[serde(default = "default_visible", skip_serializing_if = "is_default_visible")]
    pub visible: bool,

    // Parent
    pub node:    Node,
}


impl Node2D {
  pub fn new(name: &str) -> Self {
    Self {
    ty:    "Node2D".into(),
    transform: Transform2D::default(),
    visible: default_visible(),
    // Parent
    node: Node::new(name, None),
    }
  }
  pub fn get_visible(&self) -> bool {
    self.visible
  }
  
  pub fn set_visible(&mut self, visible: bool) {
    self.visible = visible;
  }
}

impl Deref for Node2D {
  type Target = Node;

  fn deref(&self) -> &Self::Target {
      &self.node
  }
}

impl DerefMut for Node2D {
  fn deref_mut(&mut self) -> &mut Self::Target {
      &mut self.node
  }
}

