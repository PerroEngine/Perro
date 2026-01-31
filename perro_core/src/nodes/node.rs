// node.rs
use crate::ids::NodeID;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::{
    borrow::Cow,
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::node_registry::NodeType;

/// Value for script_exp_vars: either a JSON value or a node reference (NodeID).
/// At runtime we detect NodeRef and remap to the actual runtime NodeID.
#[derive(Clone, Debug, PartialEq)]
pub enum ScriptExpVarValue {
    Value(Value),
    NodeRef(NodeID),
}

impl ScriptExpVarValue {
    /// Convert to serde_json::Value for the script API (apply_exposed). NodeRef becomes hex string.
    pub fn to_json_value(&self) -> Value {
        match self {
            ScriptExpVarValue::Value(v) => v.clone(),
            ScriptExpVarValue::NodeRef(id) => Value::String(format!("{:016x}", id.as_u64())),
        }
    }

    /// Build from serde_json::Value (e.g. when setting from editor). String that parses as NodeID → NodeRef.
    pub fn from_json_value(v: &Value) -> Self {
        if let Some(s) = v.as_str() {
            if let Ok(id) = NodeID::parse_str(s) {
                return ScriptExpVarValue::NodeRef(id);
            }
        }
        ScriptExpVarValue::Value(v.clone())
    }

    // --- Constructors for codegen: no serde_json in generated project ---

    pub fn null() -> Self {
        ScriptExpVarValue::Value(Value::Null)
    }

    pub fn bool(b: bool) -> Self {
        ScriptExpVarValue::Value(Value::Bool(b))
    }

    pub fn number_i64(i: i64) -> Self {
        ScriptExpVarValue::Value(Value::Number(serde_json::Number::from(i)))
    }

    pub fn number_u64(u: u64) -> Self {
        ScriptExpVarValue::Value(Value::Number(serde_json::Number::from(u)))
    }

    pub fn number_f64(f: f64) -> Self {
        ScriptExpVarValue::Value(Value::Number(
            serde_json::Number::from_f64(f).unwrap_or(serde_json::Number::from(0)),
        ))
    }

    pub fn string(s: impl Into<Cow<'static, str>>) -> Self {
        ScriptExpVarValue::Value(Value::String(s.into().into_owned()))
    }

    pub fn array(arr: impl IntoIterator<Item = ScriptExpVarValue>) -> Self {
        ScriptExpVarValue::Value(Value::Array(
            arr.into_iter().map(|v| v.to_json_value()).collect(),
        ))
    }

    pub fn object(
        entries: impl IntoIterator<Item = (Cow<'static, str>, ScriptExpVarValue)>,
    ) -> Self {
        ScriptExpVarValue::Value(Value::Object(
            entries
                .into_iter()
                .map(|(k, v)| (k.into_owned(), v.to_json_value()))
                .collect(),
        ))
    }
}

impl Serialize for ScriptExpVarValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            ScriptExpVarValue::Value(v) => v.serialize(serializer),
            // Store as {"@node": scene_key} for round-trip (scene key is id.index())
            ScriptExpVarValue::NodeRef(id) => {
                serde_json::json!({ "@node": id.index() }).serialize(serializer)
            }
        }
    }
}

impl<'de> Deserialize<'de> for ScriptExpVarValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = Value::deserialize(deserializer)?;
        let key_opt = v
            .as_object()
            .filter(|o| o.len() == 1)
            .and_then(|o| o.get("@node"))
            .and_then(|n| n.as_u64())
            .map(|n| n as u32);
        if let Some(key) = key_opt {
            return Ok(ScriptExpVarValue::NodeRef(NodeID::from_u32(key)));
        }
        Ok(ScriptExpVarValue::Value(v))
    }
}

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
            let parent = ParentType::deserialize(value).map_err(D::Error::custom)?;
            Ok(Some(parent))
        }
        Value::Null => Ok(None),
        _ => Err(D::Error::custom(
            "parent must be a u32 index, NodeID hex string, ParentType object, or null",
        )),
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
    pub script_exp_vars: Option<HashMap<Cow<'static, str>, ScriptExpVarValue>>,

    #[serde(
        rename = "parent",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_parent"
    )]
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
