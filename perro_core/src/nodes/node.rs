// node.rs
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{borrow::Cow, collections::HashMap};
use uuid::Uuid;

use crate::nodes::node_registry::BaseNode;
use crate::scripting::api::ScriptApi;
use std::any::Any;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Node {
    #[serde(skip)]
    pub id: Uuid,
    #[serde(skip)]
    pub local_id: Uuid,

    #[serde(rename = "type")]
    pub ty: Cow<'static, str>,

    pub name: Cow<'static, str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_path: Option<Cow<'static, str>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_exp_vars: Option<HashMap<String, Value>>,

    #[serde(rename = "parent", skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<Uuid>,

    #[serde(skip)]
    pub children: Option<Vec<Uuid>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, Value>>,

    #[serde(skip)]
    pub dirty: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_root_of: Option<Cow<'static, str>>,
}

impl Node {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            local_id: Uuid::new_v4(),
            ty: Cow::Borrowed("Node"),
            name: Cow::Borrowed("Node"),

            script_path: None,
            script_exp_vars: None,
            parent_id: None,
            children: None,
            metadata: None,

            dirty: true,

            is_root_of: None,
        }
    }
}
