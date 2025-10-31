
use std::{collections::HashMap, ops::{Deref, DerefMut}};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{impl_scene_node, script::Var, ui_element::UIElement, Node};


fn default_visible() -> bool { true }
fn is_default_visible(v: &bool) -> bool { *v == default_visible() }



#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct UINode {
    #[serde(rename="type")] pub ty:   String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fur_path: Option<String>,

    #[serde(skip)]
    pub props: HashMap<String, Var>,

    #[serde(skip)]
    pub elements: IndexMap<Uuid, UIElement>,
    #[serde(skip)]
    pub root_ids: Vec<Uuid>,

    #[serde(default = "default_visible", skip_serializing_if = "is_default_visible")]
    pub visible: bool,

    // Parent
    pub node:    Node,
}

impl UINode {
  pub fn new(name: &str) -> Self {
      Self {
      ty:    "UI".into(),
      visible: default_visible(),
      // Parent
      node: Node::new(name, None),
      fur_path: None,
      props: HashMap::new(),
      elements: IndexMap::new(),
      root_ids: Vec::new(),
      }
    }
    pub fn get_visible(&self) -> bool {
      self.visible
    }
    
    pub fn set_visible(&mut self, visible: bool) {
      self.visible = visible;
    }
    
}


impl Deref for UINode {
  type Target = Node;

  fn deref(&self) -> &Self::Target {
      &self.node
  }
}

impl DerefMut for UINode {
  fn deref_mut(&mut self) -> &mut Self::Target {
      &mut self.node
  }
}

