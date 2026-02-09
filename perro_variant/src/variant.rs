// perro_variant/src/lib.rs

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::sync::Arc;

use perro_core::structs::*;
use perro_ids::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Number {
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),

    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),

    F32(f32),
    F64(f64),
}

impl Number {
    #[inline]
    pub const fn is_int(&self) -> bool {
        matches!(
            self,
            Number::I8(_)
                | Number::I16(_)
                | Number::I32(_)
                | Number::I64(_)
                | Number::I128(_)
                | Number::U8(_)
                | Number::U16(_)
                | Number::U32(_)
                | Number::U64(_)
                | Number::U128(_)
        )
    }

    #[inline]
    pub const fn is_float(&self) -> bool {
        matches!(self, Number::F32(_) | Number::F64(_))
    }

    #[inline]
    pub fn as_i64_lossy(&self) -> Option<i64> {
        match *self {
            Number::I8(v) => Some(v as i64),
            Number::I16(v) => Some(v as i64),
            Number::I32(v) => Some(v as i64),
            Number::I64(v) => Some(v),
            Number::I128(v) => i64::try_from(v).ok(),
            Number::U8(v) => Some(v as i64),
            Number::U16(v) => Some(v as i64),
            Number::U32(v) => Some(v as i64),
            Number::U64(v) => i64::try_from(v).ok(),
            Number::U128(v) => i64::try_from(v).ok(),
            Number::F32(_) | Number::F64(_) => None,
        }
    }

    #[inline]
    pub fn as_f64_lossy(&self) -> Option<f64> {
        match *self {
            Number::I8(v) => Some(v as f64),
            Number::I16(v) => Some(v as f64),
            Number::I32(v) => Some(v as f64),
            Number::I64(v) => Some(v as f64),
            Number::I128(v) => Some(v as f64),
            Number::U8(v) => Some(v as f64),
            Number::U16(v) => Some(v as f64),
            Number::U32(v) => Some(v as f64),
            Number::U64(v) => Some(v as f64),
            Number::U128(v) => Some(v as f64),
            Number::F32(v) => Some(v as f64),
            Number::F64(v) => Some(v),
        }
    }
}

/// A flexible, type-safe variant type for dynamic data storage and interchange.
#[derive(Clone, Debug, PartialEq)]
pub enum Variant {
    // --- Nullary ---
    Null,

    // --- Primitives ---
    Bool(bool),
    Number(Number),

    // --- Text/Binary ---
    String(Arc<str>),
    Bytes(Arc<[u8]>),

    // --- Engine handles (stable IDs) ---
    NodeID(NodeID),
    TextureID(TextureID),

    // --- Math primitives ---
    Vector2(Vector2),
    Vector3(Vector3),
    Transform2D(Transform2D),
    Transform3D(Transform3D),
    Quaternion(Quaternion),

    // --- Containers (serde_json-like) ---
    Array(Vec<Variant>),

    // Deterministic ordering by default (better diffs, stable serialization).
    // If you want raw speed, swap to HashMap.
    Object(BTreeMap<Arc<str>, Variant>),
}

// -------------------- Constructors --------------------

impl Variant {
    #[inline]
    pub const fn null() -> Self {
        Variant::Null
    }
    #[inline]
    pub const fn is_null(&self) -> bool {
        matches!(self, Variant::Null)
    }

    #[inline]
    pub fn string<S: AsRef<str>>(s: S) -> Self {
        Variant::String(Arc::<str>::from(s.as_ref()))
    }

    #[inline]
    pub fn bytes<B: AsRef<[u8]>>(b: B) -> Self {
        Variant::Bytes(Arc::<[u8]>::from(b.as_ref()))
    }

    #[inline]
    pub fn object() -> Self {
        Variant::Object(BTreeMap::new())
    }

    #[inline]
    pub fn array() -> Self {
        Variant::Array(Vec::new())
    }
}

// -------------------- Accessors (extensible pattern) --------------------

impl Variant {
    #[inline]
    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Variant::Bool(v) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_number(&self) -> Option<Number> {
        match *self {
            Variant::Number(n) => Some(n),
            _ => None,
        }
    }

    #[inline]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Variant::String(s) => Some(s),
            _ => None,
        }
    }

    #[inline]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Variant::Bytes(b) => Some(b),
            _ => None,
        }
    }

    #[inline]
    pub fn as_node(&self) -> Option<NodeID> {
        match *self {
            Variant::NodeID(id) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_texture(&self) -> Option<TextureID> {
        match *self {
            Variant::TextureID(id) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_vec2(&self) -> Option<Vector2> {
        match *self {
            Variant::Vector2(v) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_vec3(&self) -> Option<Vector3> {
        match *self {
            Variant::Vector3(v) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_transform2d(&self) -> Option<Transform2D> {
        match *self {
            Variant::Transform2D(t) => Some(t),
            _ => None,
        }
    }

    #[inline]
    pub fn as_transform3d(&self) -> Option<Transform3D> {
        match *self {
            Variant::Transform3D(t) => Some(t),
            _ => None,
        }
    }

    #[inline]
    pub fn as_quat(&self) -> Option<Quaternion> {
        match *self {
            Variant::Quaternion(q) => Some(q),
            _ => None,
        }
    }

    #[inline]
    pub fn as_array(&self) -> Option<&[Variant]> {
        match self {
            Variant::Array(v) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Variant>> {
        match self {
            Variant::Array(v) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_object(&self) -> Option<&BTreeMap<Arc<str>, Variant>> {
        match self {
            Variant::Object(m) => Some(m),
            _ => None,
        }
    }

    #[inline]
    pub fn as_object_mut(&mut self) -> Option<&mut BTreeMap<Arc<str>, Variant>> {
        match self {
            Variant::Object(m) => Some(m),
            _ => None,
        }
    }
}

// -------------------- From impls (ergonomic construction) --------------------

// Primitives
impl From<bool> for Variant {
    #[inline]
    fn from(v: bool) -> Self {
        Variant::Bool(v)
    }
}
impl From<Number> for Variant {
    #[inline]
    fn from(v: Number) -> Self {
        Variant::Number(v)
    }
}

// Signed ints
impl From<i8> for Variant {
    #[inline]
    fn from(v: i8) -> Self {
        Variant::Number(Number::I8(v))
    }
}
impl From<i16> for Variant {
    #[inline]
    fn from(v: i16) -> Self {
        Variant::Number(Number::I16(v))
    }
}
impl From<i32> for Variant {
    #[inline]
    fn from(v: i32) -> Self {
        Variant::Number(Number::I32(v))
    }
}
impl From<i64> for Variant {
    #[inline]
    fn from(v: i64) -> Self {
        Variant::Number(Number::I64(v))
    }
}
impl From<i128> for Variant {
    #[inline]
    fn from(v: i128) -> Self {
        Variant::Number(Number::I128(v))
    }
}

// Unsigned ints
impl From<u8> for Variant {
    #[inline]
    fn from(v: u8) -> Self {
        Variant::Number(Number::U8(v))
    }
}
impl From<u16> for Variant {
    #[inline]
    fn from(v: u16) -> Self {
        Variant::Number(Number::U16(v))
    }
}
impl From<u32> for Variant {
    #[inline]
    fn from(v: u32) -> Self {
        Variant::Number(Number::U32(v))
    }
}
impl From<u64> for Variant {
    #[inline]
    fn from(v: u64) -> Self {
        Variant::Number(Number::U64(v))
    }
}
impl From<u128> for Variant {
    #[inline]
    fn from(v: u128) -> Self {
        Variant::Number(Number::U128(v))
    }
}

// Floats
impl From<f32> for Variant {
    #[inline]
    fn from(v: f32) -> Self {
        Variant::Number(Number::F32(v))
    }
}
impl From<f64> for Variant {
    #[inline]
    fn from(v: f64) -> Self {
        Variant::Number(Number::F64(v))
    }
}

// Text/Binary
impl From<&str> for Variant {
    #[inline]
    fn from(v: &str) -> Self {
        Variant::String(Arc::<str>::from(v))
    }
}
impl From<String> for Variant {
    #[inline]
    fn from(v: String) -> Self {
        Variant::String(Arc::<str>::from(v))
    }
}
impl From<Arc<str>> for Variant {
    #[inline]
    fn from(v: Arc<str>) -> Self {
        Variant::String(v)
    }
}

impl From<&[u8]> for Variant {
    #[inline]
    fn from(v: &[u8]) -> Self {
        Variant::Bytes(Arc::<[u8]>::from(v))
    }
}
impl From<Vec<u8>> for Variant {
    #[inline]
    fn from(v: Vec<u8>) -> Self {
        Variant::Bytes(Arc::<[u8]>::from(v.into_boxed_slice()))
    }
}
impl From<Arc<[u8]>> for Variant {
    #[inline]
    fn from(v: Arc<[u8]>) -> Self {
        Variant::Bytes(v)
    }
}

// Engine handles
impl From<NodeID> for Variant {
    #[inline]
    fn from(v: NodeID) -> Self {
        Variant::NodeID(v)
    }
}
impl From<TextureID> for Variant {
    #[inline]
    fn from(v: TextureID) -> Self {
        Variant::TextureID(v)
    }
}

// Math
impl From<Vector2> for Variant {
    #[inline]
    fn from(v: Vector2) -> Self {
        Variant::Vector2(v)
    }
}
impl From<Vector3> for Variant {
    #[inline]
    fn from(v: Vector3) -> Self {
        Variant::Vector3(v)
    }
}
impl From<Transform2D> for Variant {
    #[inline]
    fn from(v: Transform2D) -> Self {
        Variant::Transform2D(v)
    }
}
impl From<Transform3D> for Variant {
    #[inline]
    fn from(v: Transform3D) -> Self {
        Variant::Transform3D(v)
    }
}
impl From<Quaternion> for Variant {
    #[inline]
    fn from(v: Quaternion) -> Self {
        Variant::Quaternion(v)
    }
}

// Containers
impl From<Vec<Variant>> for Variant {
    #[inline]
    fn from(v: Vec<Variant>) -> Self {
        Variant::Array(v)
    }
}
impl From<BTreeMap<Arc<str>, Variant>> for Variant {
    #[inline]
    fn from(v: BTreeMap<Arc<str>, Variant>) -> Self {
        Variant::Object(v)
    }
}
