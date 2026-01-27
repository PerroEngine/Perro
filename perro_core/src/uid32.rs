//! 32-bit unique identifiers with type-safe wrappers and separate atomic counters per type.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU32, Ordering};

/// Base 32-bit unique identifier type
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Uid32(u32);

impl Uid32 {
    pub fn nil() -> Self {
        Self(0)
    }
    
    pub fn from_u32(value: u32) -> Self {
        Self(value)
    }
    
    pub fn as_u32(&self) -> u32 {
        self.0
    }
    
    pub fn from_string(s: &str) -> Self {
        const FNV_OFFSET_BASIS: u32 = 0x811c9dc5;
        const FNV_PRIME: u32 = 0x01000193;
        
        let mut hash = FNV_OFFSET_BASIS;
        for byte in s.as_bytes() {
            hash ^= *byte as u32;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        
        Self(if hash == 0 { 1 } else { hash })
    }
    
    pub fn parse_str(s: &str) -> Result<Self, String> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        u32::from_str_radix(s, 16)
            .map(Self)
            .map_err(|e| format!("Invalid Uid32 string: {}", e))
    }
    
    pub fn parse_uuid_str(s: &str) -> Result<Self, String> {
        let s_no_dashes = s.replace('-', "");
        if let Ok(value) = u32::from_str_radix(&s_no_dashes[..8.min(s_no_dashes.len())], 16) {
            Ok(Self(value))
        } else {
            Ok(Self::from_string(s))
        }
    }
    
    pub fn to_string(&self) -> String {
        format!("{:08x}", self.0)
    }
    
    pub fn to_string_uppercase(&self) -> String {
        format!("{:08X}", self.0)
    }
    
    pub fn is_nil(&self) -> bool {
        self.0 == 0
    }
}

// Standard library trait implementations
impl Default for Uid32 {
    fn default() -> Self {
        Self::nil()
    }
}

impl fmt::Debug for Uid32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Uid32({:08x})", self.0)
    }
}

impl fmt::Display for Uid32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08x}", self.0)
    }
}

impl Hash for Uid32 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialOrd for Uid32 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for Uid32 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

// Serde trait implementations
impl Serialize for Uid32 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Uid32 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Uid32Visitor;
        
        impl<'de> serde::de::Visitor<'de> for Uid32Visitor {
            type Value = Uid32;
            
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a hex string or u32")
            }
            
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Uid32::parse_str(v).map_err(E::custom)
            }
            
            fn visit_u32<E: serde::de::Error>(self, v: u32) -> Result<Self::Value, E> {
                Ok(Uid32::from_u32(v))
            }
        }
        
        deserializer.deserialize_any(Uid32Visitor)
    }
}

// Type-safe ID wrappers with separate atomic counters per type
static NODE_COUNTER: AtomicU32 = AtomicU32::new(1);
static TEXTURE_COUNTER: AtomicU32 = AtomicU32::new(1);
static MATERIAL_COUNTER: AtomicU32 = AtomicU32::new(1);
static MESH_COUNTER: AtomicU32 = AtomicU32::new(1);
static LIGHT_COUNTER: AtomicU32 = AtomicU32::new(1);
static UI_ELEMENT_COUNTER: AtomicU32 = AtomicU32::new(1);

macro_rules! define_id_type {
    ($type_name:ident, $counter:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $type_name(Uid32);

        impl $type_name {
            pub fn new() -> Self {
                let counter = $counter.fetch_add(1, Ordering::Relaxed);
                let id_value = if counter == 0 { 1 } else { counter };
                Self(Uid32::from_u32(id_value))
            }
            
            pub fn nil() -> Self {
                Self(Uid32::nil())
            }
            
            /// Create from a u32 value directly (bypasses atomic counter)
            /// Useful for deserialization and deterministic ID creation
            pub fn from_u32(value: u32) -> Self {
                Self(Uid32::from_u32(value))
            }
            
            pub fn from_uid32(uid: Uid32) -> Self {
                Self(uid)
            }
            
            pub fn as_uid32(&self) -> Uid32 {
                self.0
            }
            
            pub fn is_nil(&self) -> bool {
                self.0.is_nil()
            }
        }

        // Standard library trait implementations
        impl Default for $type_name {
            fn default() -> Self {
                Self::nil()
            }
        }

        impl From<Uid32> for $type_name {
            fn from(uid: Uid32) -> Self {
                Self(uid)
            }
        }

        impl From<$type_name> for Uid32 {
            fn from(id: $type_name) -> Self {
                id.0
            }
        }

        impl fmt::Debug for $type_name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(
                    f,
                    concat!(stringify!($type_name), "({})"),
                    self.0.as_u32()
                )
            }
        }
        
        impl fmt::Display for $type_name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0.as_u32())
            }
        }
        

        // Serde trait implementations
        impl Serialize for $type_name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                self.0.serialize(serializer)
            }
        }

        impl<'de> Deserialize<'de> for $type_name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                Uid32::deserialize(deserializer).map(Self)
            }
        }
    };
}

define_id_type!(NodeID, NODE_COUNTER, "Node and Script IDs");
define_id_type!(TextureID, TEXTURE_COUNTER, "Texture IDs");
define_id_type!(MaterialID, MATERIAL_COUNTER, "Material IDs");
define_id_type!(MeshID, MESH_COUNTER, "Mesh IDs");
define_id_type!(LightID, LIGHT_COUNTER, "Light IDs");
define_id_type!(UIElementID, UI_ELEMENT_COUNTER, "UI Element IDs");

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_node_id_new() {
        let id1 = NodeID::new();
        let id2 = NodeID::new();
        assert_ne!(id1, id2);
        assert!(!id1.is_nil());
        assert!(!id2.is_nil());
    }
    
    #[test]
    fn test_different_types_can_have_same_value() {
        let node_id = NodeID::from_uid32(Uid32::from_u32(1234));
        let texture_id = TextureID::from_uid32(Uid32::from_u32(1234));
        assert_eq!(node_id.as_uid32().as_u32(), texture_id.as_uid32().as_u32());
    }
    
    #[test]
    fn test_nil() {
        let nil = Uid32::nil();
        assert_eq!(nil.as_u32(), 0);
        assert!(nil.is_nil());
        
        let node_nil = NodeID::nil();
        assert!(node_nil.is_nil());
    }
    
    #[test]
    fn test_from_string() {
        let uid1 = Uid32::from_string("test");
        let uid2 = Uid32::from_string("test");
        assert_eq!(uid1, uid2); // Deterministic
        
        let uid3 = Uid32::from_string("different");
        assert_ne!(uid1, uid3);
    }
    
    #[test]
    fn test_parse_str() {
        let uid = Uid32::parse_str("a1b2c3d4").unwrap();
        assert_eq!(uid.as_u32(), 0xa1b2c3d4);
        
        let uid2 = Uid32::parse_str("0x12345678").unwrap();
        assert_eq!(uid2.as_u32(), 0x12345678);
    }
    
    #[test]
    fn test_serialize() {
        let uid = Uid32::from_u32(0x12345678);
        let json = serde_json::to_string(&uid).unwrap();
        assert_eq!(json, "\"12345678\"");
    }
    
    #[test]
    fn test_deserialize() {
        let json = "\"12345678\"";
        let uid: Uid32 = serde_json::from_str(json).unwrap();
        assert_eq!(uid.as_u32(), 0x12345678);
    }
}
