use std::ops::{Deref, DerefMut};
use std::any::Any;
use std::collections::HashMap;
use serde_json::Value;

use crate::nodes::_2d::node_2d::Node2D;
use crate::nodes::node_registry::BaseNode;
use crate::scripting::api::ScriptApi;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use uuid::Uuid;

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct Area2D {
    #[serde(rename = "type")]
    pub ty: Cow<'static, str>,

    pub node_2d: Node2D,
}

impl Area2D {
    pub fn new(name: &str) -> Self {
        Self {
            ty: Cow::Borrowed("Area2D"),
            node_2d: Node2D::new(name),
        }
    }
}

impl Deref for Area2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.node_2d
    }
}

impl DerefMut for Area2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.node_2d
    }
}

// Additional BaseNode methods for internal fixed update
impl BaseNode for Area2D {
    fn needs_internal_fixed_update(&self) -> bool {
        true // Area2D needs internal fixed updates for physics
    }

    fn internal_fixed_update(&mut self, api: &mut ScriptApi) {
        // Physics update logic will go here
        // e.g., update Rapier physics bodies, handle collisions
    }
}
