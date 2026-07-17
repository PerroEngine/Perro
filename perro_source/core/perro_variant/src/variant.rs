// perro_variant/src/lib.rs

#![forbid(unsafe_code)]

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, LinkedList, VecDeque};
use std::fmt;
use std::hash::Hash;
use std::num::{
    NonZeroI8, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI128, NonZeroIsize, NonZeroU8,
    NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU128, NonZeroUsize, Saturating, Wrapping,
};
use std::ops::{Range, RangeInclusive};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicU32, AtomicU64, AtomicUsize, Ordering},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use perro_ids::*;
use perro_structs::*;
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};

/// `i128` stored as two 8-aligned halves. A bare `i128` member is 16-aligned,
/// which alone would pad `Number` to 32 bytes and `Variant` to 48; the split
/// keeps `Number` at 24 with no boxing and `Copy` intact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PackedI128 {
    hi: i64,
    lo: u64,
}

impl PackedI128 {
    #[inline]
    pub const fn new(v: i128) -> Self {
        Self {
            hi: (v >> 64) as i64,
            lo: v as u64,
        }
    }

    #[inline]
    pub const fn get(self) -> i128 {
        ((self.hi as i128) << 64) | self.lo as i128
    }
}

/// See [`PackedI128`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PackedU128 {
    hi: u64,
    lo: u64,
}

impl PackedU128 {
    #[inline]
    pub const fn new(v: u128) -> Self {
        Self {
            hi: (v >> 64) as u64,
            lo: v as u64,
        }
    }

    #[inline]
    pub const fn get(self) -> u128 {
        ((self.hi as u128) << 64) | self.lo as u128
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Number {
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(PackedI128),

    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(PackedU128),

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
            Number::I128(v) => i64::try_from(v.get()).ok(),
            Number::U8(v) => Some(v as i64),
            Number::U16(v) => Some(v as i64),
            Number::U32(v) => Some(v as i64),
            Number::U64(v) => i64::try_from(v).ok(),
            Number::U128(v) => i64::try_from(v.get()).ok(),
            Number::F32(_) | Number::F64(_) => None,
        }
    }

    #[inline]
    pub fn as_u64_lossy(&self) -> Option<u64> {
        match *self {
            Number::I8(v) => u64::try_from(v).ok(),
            Number::I16(v) => u64::try_from(v).ok(),
            Number::I32(v) => u64::try_from(v).ok(),
            Number::I64(v) => u64::try_from(v).ok(),
            Number::I128(v) => u64::try_from(v.get()).ok(),
            Number::U8(v) => Some(v as u64),
            Number::U16(v) => Some(v as u64),
            Number::U32(v) => Some(v as u64),
            Number::U64(v) => Some(v),
            Number::U128(v) => u64::try_from(v.get()).ok(),
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
            Number::I128(v) => Some(v.get() as f64),
            Number::U8(v) => Some(v as f64),
            Number::U16(v) => Some(v as f64),
            Number::U32(v) => Some(v as f64),
            Number::U64(v) => Some(v as f64),
            Number::U128(v) => Some(v.get() as f64),
            Number::F32(v) => Some(v as f64),
            Number::F64(v) => Some(v),
        }
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Number::I8(v) => write!(f, "{v}"),
            Number::I16(v) => write!(f, "{v}"),
            Number::I32(v) => write!(f, "{v}"),
            Number::I64(v) => write!(f, "{v}"),
            Number::I128(v) => write!(f, "{}", v.get()),
            Number::U8(v) => write!(f, "{v}"),
            Number::U16(v) => write!(f, "{v}"),
            Number::U32(v) => write!(f, "{v}"),
            Number::U64(v) => write!(f, "{v}"),
            Number::U128(v) => write!(f, "{}", v.get()),
            Number::F32(v) => write!(f, "{v}"),
            Number::F64(v) => write!(f, "{v}"),
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

    // --- Engine handles ---
    ID(IDs),

    // --- Engine structs ---
    EngineStruct(EngineStruct),

    // --- Containers (serde_json-like) ---
    Array(Vec<Variant>),

    // Deterministic ordering by default (better diffs, stable serialization).
    // If you want raw speed, swap to HashMap.
    Object(BTreeMap<Arc<str>, Variant>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VariantKind {
    Null,
    Bool,
    Number,
    String,
    Bytes,
    ID,
    EngineStruct,
    Array,
    Object,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MatrixShape {
    pub rows: usize,
    pub cols: usize,
    pub cell_type: MatrixCellType,
}

impl MatrixShape {
    #[inline]
    pub fn new(rows: usize, cols: usize, cell_type: MatrixCellType) -> Self {
        Self {
            rows,
            cols,
            cell_type,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MatrixCellType {
    Null,
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    F32,
    F64,
    String,
    Bytes,
    ID,
    Vector2,
    Vector3,
    Vector4,
    IVector2,
    IVector3,
    IVector4,
    UVector2,
    UVector3,
    UVector4,
    UnitVector2,
    UnitVector3,
    UnitVector4,
    Matrix(Box<MatrixShape>),
    Transform2D,
    Transform3D,
    Quaternion,
    PostProcessSet,
    VisualAccessibilitySettings,
    Array,
    Object,
    Mixed,
}

impl VariantKind {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            VariantKind::Null => "Null",
            VariantKind::Bool => "Bool",
            VariantKind::Number => "Number",
            VariantKind::String => "String",
            VariantKind::Bytes => "Bytes",
            VariantKind::ID => "ID",
            VariantKind::EngineStruct => "EngineStruct",
            VariantKind::Array => "Array",
            VariantKind::Object => "Object",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IDs {
    Node(NodeID),
    Texture(TextureID),
    Material(MaterialID),
    Mesh(MeshID),
    Animation(AnimationID),
    Light(LightID),
    Signal(SignalID),
    AudioBus(AudioBusID),
    Tag(TagID),
    PreloadedScene(PreloadedSceneID),
}

impl IDs {
    #[inline]
    pub const fn as_u64(self) -> u64 {
        match self {
            IDs::Node(v) => v.as_u64(),
            IDs::Texture(v) => v.as_u64(),
            IDs::Material(v) => v.as_u64(),
            IDs::Mesh(v) => v.as_u64(),
            IDs::Animation(v) => v.as_u64(),
            IDs::Light(v) => v.as_u64(),
            IDs::Signal(v) => v.as_u64(),
            IDs::AudioBus(v) => v.as_u64(),
            IDs::Tag(v) => v.as_u64(),
            IDs::PreloadedScene(v) => v.as_u64(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum EngineStruct {
    Vector2(Vector2),
    Vector3(Vector3),
    Vector4(Vector4),
    IVector2(IVector2),
    IVector3(IVector3),
    IVector4(IVector4),
    UVector2(UVector2),
    UVector3(UVector3),
    UVector4(UVector4),
    UnitVector2(UnitVector2),
    UnitVector3(UnitVector3),
    UnitVector4(UnitVector4),
    Matrix2(Matrix2),
    // Members above 24 bytes are boxed so `EngineStruct` (and with it
    // `Variant`) stays small; every Vec<Variant> element and object node
    // pays the inline size. Accessors hide the box.
    Matrix3(Box<Matrix3>),
    Matrix4(Box<Matrix4>),
    Transform2D(Box<Transform2D>),
    Transform3D(Box<Transform3D>),
    Quaternion(Quaternion),
    PostProcessSet(Box<PostProcessSet>),
    VisualAccessibilitySettings(VisualAccessibilitySettings),
}

/// Typed conversion contract used by script state and method parameter conversion.
///
/// Implement this trait for custom structs/enums (typically via `#[derive(Variant)]`).
pub trait DeriveVariant: Sized {
    fn from_variant(value: &Variant) -> Option<Self>;
    fn from_owned_variant(value: Variant) -> Option<Self> {
        Self::from_variant(&value)
    }
    fn to_variant(&self) -> Variant;
    fn into_variant(self) -> Variant {
        self.to_variant()
    }
}

/// Optional compile-time introspection metadata for Variant-derived types.
///
/// By default types expose no fields (`&[]`). `#[derive(Variant)]` on structs
/// emits direct field-name metadata.
pub trait VariantSchema {
    fn field_names() -> &'static [&'static str] {
        &[]
    }
}

macro_rules! impl_empty_variant_schema {
    ($($ty:ty),+ $(,)?) => {
        $(impl VariantSchema for $ty {})+
    };
}

impl_empty_variant_schema!(
    bool,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    f32,
    f64,
    char,
    (),
    String,
    Cow<'static, str>,
    Variant,
    Unit,
    NodeID,
    TextureID,
    MaterialID,
    MeshID,
    AnimationID,
    LightID,
    SignalID,
    AudioBusID,
    TagID,
    PreloadedSceneID,
    Vector2,
    Vector3,
    Vector4,
    IVector2,
    IVector3,
    IVector4,
    UVector2,
    UVector3,
    UVector4,
    UnitVector2,
    UnitVector3,
    UnitVector4,
    Matrix2,
    Matrix3,
    Matrix4,
    Quaternion,
    Transform2D,
    Transform3D,
    PostProcessSet,
    VisualAccessibilitySettings,
    Duration,
    SystemTime,
    PathBuf,
    AtomicBool,
    AtomicI32,
    AtomicI64,
    AtomicU32,
    AtomicU64,
    AtomicUsize,
    NonZeroI8,
    NonZeroI16,
    NonZeroI32,
    NonZeroI64,
    NonZeroI128,
    NonZeroIsize,
    NonZeroU8,
    NonZeroU16,
    NonZeroU32,
    NonZeroU64,
    NonZeroU128,
    NonZeroUsize,
);

impl<T> VariantSchema for Option<T> {}
impl<T: ?Sized> VariantSchema for Box<T> {}
impl<T: ?Sized> VariantSchema for Arc<T> {}
impl<T: ?Sized> VariantSchema for Rc<T> {}
impl<T> VariantSchema for Cell<T> {}
impl<T> VariantSchema for RefCell<T> {}
impl<T> VariantSchema for Wrapping<T> {}
impl<T> VariantSchema for Saturating<T> {}
impl<T> VariantSchema for Reverse<T> {}
impl<T, const N: usize> VariantSchema for [T; N] {}
impl<T> VariantSchema for Vec<T> {}
impl<T> VariantSchema for LinkedList<T> {}
impl<T> VariantSchema for VecDeque<T> {}
impl<T> VariantSchema for BTreeSet<T> {}
impl<T> VariantSchema for HashSet<T> {}
impl<T> VariantSchema for BinaryHeap<T> {}
impl<K, T> VariantSchema for BTreeMap<K, T> {}
impl<K, T> VariantSchema for HashMap<K, T> {}
impl<T> VariantSchema for Range<T> {}
impl<T> VariantSchema for RangeInclusive<T> {}

pub trait VariantMatrixCell: Sized {
    fn from_matrix_cell_variant(value: &Variant) -> Option<Self>;
    fn to_matrix_cell_variant(&self) -> Variant;
    fn as_matrix_cell_f32(&self) -> Option<f32> {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VariantParseError {
    pub target: &'static str,
    pub actual: &'static str,
}

impl fmt::Display for VariantParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "variant parse mismatch: target={}, actual={}",
            self.target, self.actual
        )
    }
}

impl fmt::Display for Variant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Variant::Null => write!(f, "null"),
            Variant::Bool(v) => write!(f, "{v}"),
            Variant::Number(v) => write!(f, "{v}"),
            Variant::String(v) => write!(f, "{:?}", v.as_ref()),
            Variant::Bytes(v) => write!(f, "<bytes:{}>", v.len()),
            Variant::ID(v) => write!(f, "{v:?}"),
            Variant::EngineStruct(v) => write!(f, "{v:?}"),
            Variant::Array(values) => {
                write!(f, "[")?;
                for (i, value) in values.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{value}")?;
                }
                write!(f, "]")
            }
            Variant::Object(map) => {
                write!(f, "{{")?;
                for (i, (key, value)) in map.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:?}: {}", key.as_ref(), value)?;
                }
                write!(f, "}}")
            }
        }
    }
}

mod access;
mod convert;
mod derive;
mod json;
#[cfg(test)]
mod size_probe;
use json::*;
