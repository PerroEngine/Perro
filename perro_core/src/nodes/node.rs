// node.rs
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{borrow::Cow, collections::HashMap, time::{SystemTime, UNIX_EPOCH}};
use crate::ids::NodeID;

use crate::node_registry::NodeType;

// Note: Use NodeID::nil() for nil IDs

/// Represents a parent node with both its ID and type
/// This allows runtime type checking without needing to query the scene
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ParentType {
    pub id: NodeID,
    #[serde(rename = "type")]
    pub node_type: NodeType,
}

impl ParentType {
    pub fn new(id: NodeID, node_type: NodeType) -> Self {
        Self { id, node_type }
    }
}

/// Custom deserializer for parent field that accepts either:
/// - A u32 index (for new format with SceneData)
/// - A NodeID hex string (8 or 16 chars) - legacy format
/// - A full ParentType object with id and type fields
/// 
/// Note: When deserializing from SceneData, u32 indices become NodeID (u64).
/// during SceneData deserialization, so this should not be called directly for SceneData.
fn deserialize_parent<'de, D>(deserializer: D) -> Result<Option<ParentType>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    
    let value = Value::deserialize(deserializer)?;
    
    match value {
        Value::Number(n) => {
            // u32 index — becomes NodeID (index, gen 0)
            // This is a fallback - SceneData should handle conversion
            if let Some(idx) = n.as_u64() {
                // NodeID from index (gen 0)
                // SceneData will remap this properly
                let id = NodeID::from_u32(idx as u32);
                Ok(Some(ParentType::new(id, NodeType::Node)))
            } else {
                Err(D::Error::custom("parent index must be a u32"))
            }
        }
        Value::String(uid_str) => {
            // Parse hex string (8 or 16 char) and create ParentType with default NodeType
            let s = uid_str.strip_prefix("0x").unwrap_or(uid_str.as_str());
            let id = if s.len() <= 8 {
                u32::from_str_radix(s, 16)
                    .map(|u| NodeID::from_parts(u, 0))
                    .map_err(|e| D::Error::custom(format!("Invalid NodeID string: {}", e)))?
            } else {
                u64::from_str_radix(s, 16)
                    .map(NodeID::from_u64)
                    .map_err(|e| D::Error::custom(format!("Invalid NodeID string: {}", e)))?
            };
            Ok(Some(ParentType::new(id, NodeType::Node)))
        }
        Value::Object(_) => {
            // Deserialize as full ParentType object
            let parent = ParentType::deserialize(value)
                .map_err(D::Error::custom)?;
            Ok(Some(parent))
        }
        Value::Null => Ok(None),
        _ => Err(D::Error::custom("parent must be a u32 index, NodeID hex string, ParentType object, or null")),
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Node {
    #[serde(skip)]
    pub id: NodeID,

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
    pub children: Option<Vec<NodeID>>,

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
            id: NodeID::nil(),
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
    
    /// Create a new Node with a nil ID (for use when ID will be set later)
    pub fn new_with_nil_id() -> Self {
        // Get current Unix timestamp in seconds
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        Self {
            id: NodeID::nil(),
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
