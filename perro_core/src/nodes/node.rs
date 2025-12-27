// node.rs
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{borrow::Cow, collections::HashMap, time::{SystemTime, UNIX_EPOCH}};
use uuid::Uuid;

use crate::node_registry::NodeType;

// Helper function for serde default
fn uuid_nil() -> Uuid {
    Uuid::nil()
}

/// Represents a parent node with both its ID and type
/// This allows runtime type checking without needing to query the scene
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ParentType {
    pub id: Uuid,
    #[serde(rename = "type")]
    pub node_type: NodeType,
}

impl ParentType {
    pub fn new(id: Uuid, node_type: NodeType) -> Self {
        Self { id, node_type }
    }
}

/// Custom deserializer for parent field that accepts either:
/// - A UUID string (e.g., "00000000-0000-0000-0000-000000000000")
/// - A full ParentType object with id and type fields
fn deserialize_parent<'de, D>(deserializer: D) -> Result<Option<ParentType>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    
    let value = Value::deserialize(deserializer)?;
    
    match value {
        Value::String(uuid_str) => {
            // Parse UUID string and create ParentType with default NodeType
            // The node_type will be fixed later in fix_relationships
            let id = Uuid::parse_str(&uuid_str)
                .map_err(|e| D::Error::custom(format!("Invalid UUID string: {}", e)))?;
            Ok(Some(ParentType::new(id, NodeType::Node)))
        }
        Value::Object(_) => {
            // Deserialize as full ParentType object
            let parent = ParentType::deserialize(value)
                .map_err(D::Error::custom)?;
            Ok(Some(parent))
        }
        Value::Null => Ok(None),
        _ => Err(D::Error::custom("parent must be a UUID string, ParentType object, or null")),
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Node {
    #[serde(skip)]
    pub id: Uuid,
    #[serde(skip)]
    pub local_id: Uuid,

    #[serde(rename = "type")]
    pub ty: NodeType,

    pub name: Cow<'static, str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_path: Option<Cow<'static, str>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_exp_vars: Option<HashMap<String, Value>>,

    #[serde(rename = "parent", default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_parent")]
    pub parent: Option<ParentType>,

    #[serde(skip)]
    pub children: Option<Vec<Uuid>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, Value>>,

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
            ty: NodeType::Node,
            name: Cow::Borrowed("Node"),

            script_path: None,
            script_exp_vars: None,
            parent: None,
            children: None,
            metadata: None,

            is_root_of: None,
            created_timestamp: timestamp,
        }
    }
}
