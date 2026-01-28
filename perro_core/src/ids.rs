//! Type-safe identifiers with optional generations (slotmap-style).
//! No global counters or new() — IDs are created by their owning arena/manager.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::hash::Hash;

// ---- Generational ID (NodeID): u64 = index (low 32) | generation (high 32) ----
// Index 0 is reserved for nil. Real indices start at 1.

/// Node ID: generational slotmap-style. Low 32 bits = index, high 32 bits = generation.
/// Created by NodeArena (allocate/insert). No global counter.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeID(pub u64);

impl NodeID {
    pub fn nil() -> Self {
        Self(0)
    }

    #[inline]
    pub fn index(self) -> u32 {
        (self.0 & 0xFFFF_FFFF) as u32
    }

    #[inline]
    pub fn generation(self) -> u32 {
        (self.0 >> 32) as u32
    }

    pub fn from_parts(index: u32, generation: u32) -> Self {
        Self((index as u64) | ((generation as u64) << 32))
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }

    pub fn from_u64(value: u64) -> Self {
        Self(value)
    }

    pub fn is_nil(self) -> bool {
        self.0 == 0
    }

    /// Legacy: build from a raw u32 (index in low 32, generation 0). For deserialization / scene load.
    pub fn from_u32(index: u32) -> Self {
        Self::from_parts(index, 0)
    }

    /// Parse hex string (8 or 16 chars, optional 0x prefix) into NodeID.
    pub fn parse_str(s: &str) -> Result<Self, String> {
        let s = s.strip_prefix("0x").unwrap_or(s).replace('-', "");
        if s.len() <= 8 {
            u32::from_str_radix(&s, 16)
                .map(|u| Self::from_parts(u, 0))
                .map_err(|e| format!("Invalid NodeID string: {}", e))
        } else {
            u64::from_str_radix(&s[..16.min(s.len())], 16)
                .map(Self::from_u64)
                .map_err(|e| format!("Invalid NodeID string: {}", e))
        }
    }
}

impl Default for NodeID {
    fn default() -> Self {
        Self::nil()
    }
}

impl fmt::Debug for NodeID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeID({}:{})", self.index(), self.generation())
    }
}

impl fmt::Display for NodeID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.index(), self.generation())
    }
}

impl Serialize for NodeID {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{:016x}", self.0))
    }
}

impl<'de> Deserialize<'de> for NodeID {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct NodeIDVisitor;
        impl<'de> serde::de::Visitor<'de> for NodeIDVisitor {
            type Value = NodeID;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a hex string (8 or 16 chars) or u64")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                let s = v.strip_prefix("0x").unwrap_or(v);
                if s.len() <= 8 {
                    u32::from_str_radix(s, 16)
                        .map(|u| NodeID::from_parts(u, 0))
                        .map_err(E::custom)
                } else {
                    u64::from_str_radix(s, 16)
                        .map(NodeID::from_u64)
                        .map_err(E::custom)
                }
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(NodeID::from_u64(v))
            }
        }
        deserializer.deserialize_any(NodeIDVisitor)
    }
}

// ---- Simple u64 IDs (TextureID, MaterialID, MeshID, LightID, UIElementID) ----
// No generation unless we add arenas later. Allocated by their manager (e.g. TextureManager).

macro_rules! define_simple_id {
    ($type_name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $type_name(pub u64);

        impl $type_name {
            pub fn nil() -> Self {
                Self(0)
            }
            pub fn from_index(index: u32) -> Self {
                Self(index as u64)
            }
            pub fn from_u64(value: u64) -> Self {
                Self(value)
            }
            pub fn as_u64(self) -> u64 {
                self.0
            }
            pub fn is_nil(self) -> bool {
                self.0 == 0
            }
            /// Legacy: from u32 (low 32 bits, high 32 = 0).
            pub fn from_u32(value: u32) -> Self {
                Self(value as u64)
            }
        }

        impl Default for $type_name {
            fn default() -> Self {
                Self::nil()
            }
        }

        impl fmt::Debug for $type_name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($type_name), "({})"), self.0)
            }
        }

        impl fmt::Display for $type_name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl Serialize for $type_name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&format!("{:016x}", self.0))
            }
        }

        impl<'de> Deserialize<'de> for $type_name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct Visitor;
                impl<'de> serde::de::Visitor<'de> for Visitor {
                    type Value = $type_name;
                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("hex string or u64")
                    }
                    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                        let s = v.strip_prefix("0x").unwrap_or(v);
                        if s.len() <= 8 {
                            u32::from_str_radix(s, 16)
                                .map(|u| $type_name::from_u32(u))
                                .map_err(E::custom)
                        } else {
                            u64::from_str_radix(s, 16)
                                .map($type_name::from_u64)
                                .map_err(E::custom)
                        }
                    }
                    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                        Ok($type_name::from_u64(v))
                    }
                }
                deserializer.deserialize_any(Visitor)
            }
        }
    };
}

define_simple_id!(TextureID, "Texture ID — allocated by TextureManager.");
impl TextureID {
    /// Parse hex string (8 or 16 chars, optional 0x prefix) into TextureID.
    pub fn parse_str(s: &str) -> Result<Self, String> {
        let s = s.strip_prefix("0x").unwrap_or(s).replace('-', "");
        if s.len() <= 8 {
            u32::from_str_radix(&s, 16)
                .map(Self::from_u32)
                .map_err(|e| format!("Invalid TextureID string: {}", e))
        } else {
            u64::from_str_radix(&s[..16.min(s.len())], 16)
                .map(Self::from_u64)
                .map_err(|e| format!("Invalid TextureID string: {}", e))
        }
    }
}
define_simple_id!(MaterialID, "Material ID — allocated by material system.");
define_simple_id!(MeshID, "Mesh ID — allocated by mesh system.");
define_simple_id!(LightID, "Light ID — allocated by light system.");
define_simple_id!(UIElementID, "UI Element ID — synthetic or from UI tree.");

// UIElementID: supports from_string for synthetic IDs (e.g. "{parent}-border")
impl UIElementID {
    pub fn from_string(s: &str) -> Self {
        const FNV_OFFSET: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;
        let mut hash = FNV_OFFSET;
        for b in s.bytes() {
            hash ^= b as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        Self(if hash == 0 { 1 } else { hash })
    }

    pub fn to_string(self) -> String {
        format!("{:016x}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_nil() {
        assert!(NodeID::nil().is_nil());
        assert_eq!(NodeID::nil().index(), 0);
        assert_eq!(NodeID::nil().generation(), 0);
    }

    #[test]
    fn node_id_parts() {
        let id = NodeID::from_parts(5, 2);
        assert_eq!(id.index(), 5);
        assert_eq!(id.generation(), 2);
        assert!(!id.is_nil());
    }

    #[test]
    fn node_id_roundtrip_u64() {
        let id = NodeID::from_parts(1, 1);
        assert_eq!(NodeID::from_u64(id.as_u64()), id);
    }

    #[test]
    fn texture_id_nil() {
        assert!(TextureID::nil().is_nil());
    }

    #[test]
    fn ui_element_from_string() {
        let a = UIElementID::from_string("x-border");
        let b = UIElementID::from_string("x-border");
        assert_eq!(a, b);
    }
}
