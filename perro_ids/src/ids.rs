//! Type-safe generational identifiers (slotmap-style) for arenas.
//! All IDs use u64 = index (low 32 bits) | generation (high 32 bits). Index 0 = nil.
//! IDs are created by their owning arena/manager; slot reuse bumps generation so stale IDs are invalid.

use std::fmt;
use std::hash::Hash;

pub const fn string_to_u64(s: &str) -> u64 {
    let mut hash: u64 = 0xA0761D6478BD642F;
    let bytes = s.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(0xE7037ED1A0B428DB);
        hash = mix64(hash);
        i += 1;
    }

    mix64(hash ^ (bytes.len() as u64))
}

pub const fn mix64(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58476D1CE4E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D049BB133111EB);
    x ^= x >> 31;
    x
}

// ---- Generational ID: base encoding ----
// u64 layout: low 32 = index (0 = nil, 1.. = slot), high 32 = generation.
// When a slot is reused, generation is bumped so old IDs no longer match.

/// Defines a generational ID type (NodeID, TextureID, MaterialID, etc.).
/// All such IDs use index + generation for safe arena slot reuse.
macro_rules! define_generational_id {
    ($type_name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $type_name(pub u64);

        impl $type_name {
            #[inline]
            pub const fn new(id: u32) -> Self {
                Self::from_parts(id, 0)
            }

            #[inline]
            pub const fn nil() -> Self {
                Self(0)
            }

            #[inline]
            pub const fn index(self) -> u32 {
                (self.0 & 0xFFFF_FFFF) as u32
            }

            #[inline]
            pub const fn generation(self) -> u32 {
                (self.0 >> 32) as u32
            }

            #[inline]
            pub const fn from_parts(index: u32, generation: u32) -> Self {
                Self((index as u64) | ((generation as u64) << 32))
            }

            #[inline]
            pub const fn as_u64(self) -> u64 {
                self.0
            }

            #[inline]
            pub const fn from_u64(value: u64) -> Self {
                Self(value)
            }

            #[inline]
            pub const fn is_nil(self) -> bool {
                self.0 == 0
            }

            /// Legacy: index in low 32, generation 0 (e.g. deserialization).
            #[inline]
            pub const fn from_u32(index: u32) -> Self {
                Self::from_parts(index, 0)
            }
        }

        impl Default for $type_name {
            fn default() -> Self {
                Self::nil()
            }
        }

        impl fmt::Debug for $type_name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(
                    f,
                    concat!(stringify!($type_name), "({}:{})"),
                    self.index(),
                    self.generation()
                )
            }
        }

        impl fmt::Display for $type_name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}", self.index(), self.generation())
            }
        }
    };
}

define_generational_id!(
    NodeID,
    "Node ID — allocated by NodeArena. Index + generation."
);
define_generational_id!(
    TextureID,
    "Texture ID — allocated by TextureManager. Index + generation."
);
define_generational_id!(
    MaterialID,
    "Material ID — allocated by material system. Index + generation."
);
define_generational_id!(
    MeshID,
    "Mesh ID — allocated by mesh system. Index + generation."
);
define_generational_id!(
    LightID,
    "Light ID — allocated by light system. Index + generation."
);
define_generational_id!(
    UIElementID,
    "UI Element ID — allocated by UI or synthetic. Index + generation."
);
define_generational_id!(
    SignalID,
    "Signal ID — hash of signal name (or generational). Used for connect/emit."
);

impl NodeID {
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

impl UIElementID {
    /// Synthetic ID from string (e.g. "{parent}-border"). Uses hash; generation 0.
    pub fn from_string(s: &str) -> Self {
        Self::from_u64(string_to_u64(s))
    }

    /// Parse hex string (8 or 16 chars, optional 0x prefix) into UIElementID (for serialization/deserialization).
    pub fn parse_str(s: &str) -> Result<Self, String> {
        let s = s.strip_prefix("0x").unwrap_or(s).replace('-', "");
        if s.len() <= 8 {
            u32::from_str_radix(&s, 16)
                .map(|u| Self::from_parts(u, 0))
                .map_err(|e| format!("Invalid UIElementID string: {}", e))
        } else {
            u64::from_str_radix(&s[..16.min(s.len())], 16)
                .map(Self::from_u64)
                .map_err(|e| format!("Invalid UIElementID string: {}", e))
        }
    }

    pub fn to_string(self) -> String {
        format!("{:016x}", self.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ScriptMemberID(pub u64);

impl ScriptMemberID {
    pub const fn from_string(s: &str) -> Self {
        Self(string_to_u64(s))
    }
}
