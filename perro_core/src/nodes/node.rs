// node.rs
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Node {

    #[serde(skip)]
    pub id:    Uuid,

    #[serde(rename = "type")]
    pub ty:    String,

    pub name:  String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_path: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_exp_vars: Option<HashMap<String, Value>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<Uuid>,

    #[serde(skip)]
    pub children: Option<Vec<Uuid>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, Value>>,

    #[serde(skip)]
    pub dirty: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_root_of: Option<String>,
}

impl Node {
    pub fn new(name: &str, parent: Option<Uuid>) -> Self {
        Self {
            id:       Uuid::new_v4(),
            ty:       "Node".into(),
            name:     name.into(),
            script_path: None,
            script_exp_vars : None,
            parent,
            children: None,
            metadata: None,

            dirty: false,

            is_root_of: None
        }
    }



    // convenience for building dynamic scenes
    pub fn into_value(self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}
