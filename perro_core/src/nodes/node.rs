// node.rs
use crate::ids::NodeID;
use cow_map::CowMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::{
    borrow::Cow,
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::node_registry::NodeType;

/// Const-friendly JSON number for script_exp_vars (no heap, no serde_json in generated project).
#[derive(Clone, Debug, PartialEq)]
pub enum JsonNumber {
    I64(i64),
    U64(u64),
    F64(f64),
}

impl JsonNumber {
    pub const fn i64(n: i64) -> Self {
        JsonNumber::I64(n)
    }
    pub const fn u64(n: u64) -> Self {
        JsonNumber::U64(n)
    }
    pub fn f64(n: f64) -> Self {
        JsonNumber::F64(if n.is_finite() { n } else { 0.0 })
    }
    fn to_serde_number(&self) -> serde_json::Number {
        match self {
            JsonNumber::I64(i) => serde_json::Number::from(*i),
            JsonNumber::U64(u) => serde_json::Number::from(*u),
            JsonNumber::F64(f) => {
                serde_json::Number::from_f64(*f).unwrap_or(serde_json::Number::from(0))
            }
        }
    }
}

/// Const-friendly JSON value for metadata (no NodeRef, no serde_json in generated project).
/// Same shape as ScriptExpVarValue but without NodeRef.
#[derive(Clone, Debug)]
pub enum MetadataValue {
    Null,
    Bool(bool),
    Number(JsonNumber),
    String(Cow<'static, str>),
    Array(Cow<'static, [MetadataValue]>),
    Object(CowMap<Cow<'static, str>, MetadataValue>),
}

impl MetadataValue {
    pub fn to_json_value(&self) -> Value {
        match self {
            MetadataValue::Null => Value::Null,
            MetadataValue::Bool(b) => Value::Bool(*b),
            MetadataValue::Number(n) => Value::Number(n.to_serde_number()),
            MetadataValue::String(s) => Value::String(s.to_string()),
            MetadataValue::Array(a) => {
                Value::Array(a.iter().map(MetadataValue::to_json_value).collect())
            }
            MetadataValue::Object(m) => Value::Object(
                m.iter()
                    .map(|(k, v)| (k.to_string(), v.to_json_value()))
                    .collect(),
            ),
        }
    }

    pub fn from_json_value(v: &Value) -> Self {
        match v {
            Value::Null => MetadataValue::Null,
            Value::Bool(b) => MetadataValue::Bool(*b),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    MetadataValue::Number(JsonNumber::I64(i))
                } else if let Some(u) = n.as_u64() {
                    MetadataValue::Number(JsonNumber::U64(u))
                } else {
                    MetadataValue::Number(JsonNumber::F64(n.as_f64().unwrap_or(0.0)))
                }
            }
            Value::String(s) => MetadataValue::String(Cow::Owned(s.clone())),
            Value::Array(arr) => MetadataValue::Array(Cow::Owned(
                arr.iter().map(MetadataValue::from_json_value).collect(),
            )),
            Value::Object(obj) => MetadataValue::Object(CowMap::from(
                obj.iter()
                    .map(|(k, v)| (Cow::Owned(k.clone()), MetadataValue::from_json_value(v)))
                    .collect::<HashMap<_, _>>(),
            )),
        }
    }

    pub const fn null() -> Self {
        MetadataValue::Null
    }
    pub const fn bool(b: bool) -> Self {
        MetadataValue::Bool(b)
    }
    pub const fn number_i64(i: i64) -> Self {
        MetadataValue::Number(JsonNumber::I64(i))
    }
    pub const fn number_u64(u: u64) -> Self {
        MetadataValue::Number(JsonNumber::U64(u))
    }
    pub fn number_f64(f: f64) -> Self {
        MetadataValue::Number(JsonNumber::f64(f))
    }
    pub fn string(s: impl Into<Cow<'static, str>>) -> Self {
        MetadataValue::String(s.into())
    }
    pub fn array(arr: impl IntoIterator<Item = MetadataValue>) -> Self {
        MetadataValue::Array(Cow::Owned(arr.into_iter().collect()))
    }
    pub fn object(entries: impl IntoIterator<Item = (Cow<'static, str>, MetadataValue)>) -> Self {
        MetadataValue::Object(CowMap::from(
            entries
                .into_iter()
                .collect::<HashMap<Cow<'static, str>, MetadataValue>>(),
        ))
    }
}

impl PartialEq for MetadataValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (MetadataValue::Null, MetadataValue::Null) => true,
            (MetadataValue::Bool(a), MetadataValue::Bool(b)) => a == b,
            (MetadataValue::Number(a), MetadataValue::Number(b)) => a == b,
            (MetadataValue::String(a), MetadataValue::String(b)) => a == b,
            (MetadataValue::Array(a), MetadataValue::Array(b)) => a == b,
            (MetadataValue::Object(a), MetadataValue::Object(b)) => {
                a.to_hashmap() == b.to_hashmap()
            }
            _ => false,
        }
    }
}

impl Serialize for MetadataValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_json_value().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MetadataValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(deserializer)?;
        Ok(MetadataValue::from_json_value(&v))
    }
}

/// Value for script_exp_vars: const-friendly representation (Cow/slices/CowMap) or NodeRef.
/// At runtime we detect NodeRef and remap to the actual runtime NodeID.
#[derive(Clone, Debug)]
pub enum ScriptExpVarValue {
    Null,
    Bool(bool),
    Number(JsonNumber),
    String(Cow<'static, str>),
    Array(Cow<'static, [ScriptExpVarValue]>),
    Object(CowMap<Cow<'static, str>, ScriptExpVarValue>),
    NodeRef(NodeID),
}

impl ScriptExpVarValue {
    /// Convert to serde_json::Value for the script API (apply_exposed). NodeRef becomes hex string.
    pub fn to_json_value(&self) -> Value {
        match self {
            ScriptExpVarValue::Null => Value::Null,
            ScriptExpVarValue::Bool(b) => Value::Bool(*b),
            ScriptExpVarValue::Number(n) => Value::Number(n.to_serde_number()),
            ScriptExpVarValue::String(s) => Value::String(s.to_string()),
            ScriptExpVarValue::Array(a) => {
                Value::Array(a.iter().map(ScriptExpVarValue::to_json_value).collect())
            }
            ScriptExpVarValue::Object(m) => Value::Object(
                m.iter()
                    .map(|(k, v)| (k.to_string(), v.to_json_value()))
                    .collect(),
            ),
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
        match v {
            Value::Null => ScriptExpVarValue::Null,
            Value::Bool(b) => ScriptExpVarValue::Bool(*b),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    ScriptExpVarValue::Number(JsonNumber::I64(i))
                } else if let Some(u) = n.as_u64() {
                    ScriptExpVarValue::Number(JsonNumber::U64(u))
                } else {
                    ScriptExpVarValue::Number(JsonNumber::F64(n.as_f64().unwrap_or(0.0)))
                }
            }
            Value::String(s) => ScriptExpVarValue::String(Cow::Owned(s.clone())),
            Value::Array(arr) => ScriptExpVarValue::Array(Cow::Owned(
                arr.iter().map(ScriptExpVarValue::from_json_value).collect(),
            )),
            Value::Object(obj) => ScriptExpVarValue::Object(CowMap::from(
                obj.iter()
                    .map(|(k, v)| (Cow::Owned(k.clone()), ScriptExpVarValue::from_json_value(v)))
                    .collect::<HashMap<_, _>>(),
            )),
        }
    }

    // --- Constructors for codegen: const-friendly, no serde_json in generated project ---

    pub const fn null() -> Self {
        ScriptExpVarValue::Null
    }

    pub const fn bool(b: bool) -> Self {
        ScriptExpVarValue::Bool(b)
    }

    pub const fn number_i64(i: i64) -> Self {
        ScriptExpVarValue::Number(JsonNumber::I64(i))
    }

    pub const fn number_u64(u: u64) -> Self {
        ScriptExpVarValue::Number(JsonNumber::U64(u))
    }

    pub fn number_f64(f: f64) -> Self {
        ScriptExpVarValue::Number(JsonNumber::f64(f))
    }

    pub fn string(s: impl Into<Cow<'static, str>>) -> Self {
        ScriptExpVarValue::String(s.into())
    }

    pub fn array(arr: impl IntoIterator<Item = ScriptExpVarValue>) -> Self {
        ScriptExpVarValue::Array(Cow::Owned(arr.into_iter().collect()))
    }

    pub fn object(
        entries: impl IntoIterator<Item = (Cow<'static, str>, ScriptExpVarValue)>,
    ) -> Self {
        ScriptExpVarValue::Object(CowMap::from(
            entries
                .into_iter()
                .collect::<HashMap<Cow<'static, str>, ScriptExpVarValue>>(),
        ))
    }
}

impl PartialEq for ScriptExpVarValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ScriptExpVarValue::Null, ScriptExpVarValue::Null) => true,
            (ScriptExpVarValue::Bool(a), ScriptExpVarValue::Bool(b)) => a == b,
            (ScriptExpVarValue::Number(a), ScriptExpVarValue::Number(b)) => a == b,
            (ScriptExpVarValue::String(a), ScriptExpVarValue::String(b)) => a == b,
            (ScriptExpVarValue::Array(a), ScriptExpVarValue::Array(b)) => a == b,
            (ScriptExpVarValue::Object(a), ScriptExpVarValue::Object(b)) => {
                a.to_hashmap() == b.to_hashmap()
            }
            (ScriptExpVarValue::NodeRef(a), ScriptExpVarValue::NodeRef(b)) => a == b,
            _ => false,
        }
    }
}

impl Serialize for ScriptExpVarValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.to_json_value().serialize(serializer)
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
        Ok(ScriptExpVarValue::from_json_value(&v))
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

fn deserialize_script_exp_vars<'de, D>(
    deserializer: D,
) -> Result<Option<CowMap<&'static str, ScriptExpVarValue>>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<HashMap<String, ScriptExpVarValue>> = Option::deserialize(deserializer)?;
    Ok(opt.map(|hm| {
        CowMap::from(
            hm.into_iter()
                .map(|(k, v)| (&*Box::leak(k.into_boxed_str()), v))
                .collect::<HashMap<&'static str, ScriptExpVarValue>>(),
        )
    }))
}

fn serialize_script_exp_vars<S>(
    value: &Option<CowMap<&'static str, ScriptExpVarValue>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let opt = value.as_ref().map(|m| {
        m.iter()
            .map(|(k, v)| (k.to_string(), v.to_json_value()))
            .collect::<HashMap<String, Value>>()
    });
    opt.serialize(serializer)
}

fn deserialize_metadata<'de, D>(
    deserializer: D,
) -> Result<Option<CowMap<&'static str, MetadataValue>>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<HashMap<String, Value>> = Option::deserialize(deserializer)?;
    Ok(opt.map(|hm| {
        CowMap::from(
            hm.into_iter()
                .map(|(k, v)| {
                    (
                        &*Box::leak(k.into_boxed_str()),
                        MetadataValue::from_json_value(&v),
                    )
                })
                .collect::<HashMap<&'static str, MetadataValue>>(),
        )
    }))
}

fn serialize_metadata<S>(
    value: &Option<CowMap<&'static str, MetadataValue>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let opt = value.as_ref().map(|m| {
        m.iter()
            .map(|(k, v)| (k.to_string(), v.to_json_value()))
            .collect::<HashMap<String, Value>>()
    });
    opt.serialize(serializer)
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Node {
    #[serde(skip)]
    pub id: NodeID,

    #[serde(rename = "type")]
    pub ty: NodeType,

    pub name: Cow<'static, str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_path: Option<Cow<'static, str>>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_script_exp_vars",
        serialize_with = "serialize_script_exp_vars"
    )]
    pub script_exp_vars: Option<CowMap<&'static str, ScriptExpVarValue>>,

    #[serde(
        rename = "parent",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_parent"
    )]
    pub parent: Option<ParentType>,

    #[serde(skip)]
    pub children: Option<Cow<'static, [NodeID]>>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_metadata",
        serialize_with = "serialize_metadata"
    )]
    pub metadata: Option<CowMap<&'static str, MetadataValue>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_root_of: Option<Cow<'static, str>>,

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

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.ty == other.ty
            && self.name == other.name
            && self.script_path == other.script_path
            && self.script_exp_vars.as_ref().map(|m| m.to_hashmap())
                == other.script_exp_vars.as_ref().map(|m| m.to_hashmap())
            && self.parent == other.parent
            && self.children.as_deref() == other.children.as_deref()
            && self.metadata.as_ref().map(|m| m.to_hashmap())
                == other.metadata.as_ref().map(|m| m.to_hashmap())
            && self.is_root_of == other.is_root_of
            && self.created_timestamp == other.created_timestamp
    }
}
