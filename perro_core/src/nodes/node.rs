// node.rs
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{borrow::Cow, collections::HashMap, time::{SystemTime, UNIX_EPOCH}};
use uuid::Uuid;

// Helper function for serde default
fn uuid_nil() -> Uuid {
    Uuid::nil()
}

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

    #[serde(rename = "parent", default = "uuid_nil", skip_serializing_if = "Uuid::is_nil")]
    pub parent_id: Uuid,

    #[serde(skip)]
    pub children: Option<Vec<Uuid>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, Value>>,

    #[serde(skip)]
    pub dirty: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_root_of: Option<Cow<'static, str>>,

    /// Timestamp when the node was created (Unix time in seconds as u64)
    /// Used for tie-breaking when z_index values are the same (newer nodes render above older)
    #[serde(skip)]
    pub created_timestamp: u64,
}

impl Node {
    pub fn new() -> Self {
        // Get current Unix timestamp in seconds
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        Self {
            id: Uuid::new_v4(),
            local_id: Uuid::new_v4(),
            ty: Cow::Borrowed("Node"),
            name: Cow::Borrowed("Node"),

            script_path: None,
            script_exp_vars: None,
            parent_id: Uuid::nil(),
            children: None,
            metadata: None,

            dirty: true,

            is_root_of: None,
            created_timestamp: timestamp,
        }
    }
}

