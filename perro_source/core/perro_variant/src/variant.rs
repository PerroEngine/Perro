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
    pub fn as_u64_lossy(&self) -> Option<u64> {
        match *self {
            Number::I8(v) => u64::try_from(v).ok(),
            Number::I16(v) => u64::try_from(v).ok(),
            Number::I32(v) => u64::try_from(v).ok(),
            Number::I64(v) => u64::try_from(v).ok(),
            Number::I128(v) => u64::try_from(v).ok(),
            Number::U8(v) => Some(v as u64),
            Number::U16(v) => Some(v as u64),
            Number::U32(v) => Some(v as u64),
            Number::U64(v) => Some(v),
            Number::U128(v) => u64::try_from(v).ok(),
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

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Number::I8(v) => write!(f, "{v}"),
            Number::I16(v) => write!(f, "{v}"),
            Number::I32(v) => write!(f, "{v}"),
            Number::I64(v) => write!(f, "{v}"),
            Number::I128(v) => write!(f, "{v}"),
            Number::U8(v) => write!(f, "{v}"),
            Number::U16(v) => write!(f, "{v}"),
            Number::U32(v) => write!(f, "{v}"),
            Number::U64(v) => write!(f, "{v}"),
            Number::U128(v) => write!(f, "{v}"),
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
    Matrix3(Matrix3),
    Matrix4(Matrix4),
    Transform2D(Transform2D),
    Transform3D(Transform3D),
    Quaternion(Quaternion),
    PostProcessSet(PostProcessSet),
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

    /// Decode this [`Variant`] into typed value via [`DeriveVariant`].
    ///
    /// Returns `Err` when shape/type does not match `T`.
    ///
    /// # Example
    /// ```rust
    /// use perro_variant::Variant;
    ///
    /// let value = Variant::from(42_i32);
    /// let parsed = value.parse::<i32>();
    /// assert_eq!(parsed, Ok(42));
    /// ```
    #[inline]
    pub fn parse<T>(&self) -> Result<T, VariantParseError>
    where
        T: DeriveVariant,
    {
        T::from_variant(self).ok_or(VariantParseError {
            target: std::any::type_name::<T>(),
            actual: self.kind_name(),
        })
    }

    /// Decode this [`Variant`] into typed value, returning `None` on mismatch.
    #[inline]
    pub fn as_type<T>(&self) -> Option<T>
    where
        T: DeriveVariant,
    {
        T::from_variant(self)
    }

    /// Check whether this [`Variant`] can decode into `T`.
    #[inline]
    pub fn is_type<T>(&self) -> bool
    where
        T: DeriveVariant,
    {
        T::from_variant(self).is_some()
    }

    /// Decode this [`Variant`] into typed value while consuming the variant.
    ///
    /// This avoids clone work for container-heavy values when the caller no
    /// longer needs the intermediate variant.
    #[inline]
    pub fn into_parse<T>(self) -> Result<T, VariantParseError>
    where
        T: DeriveVariant,
    {
        let actual = self.kind_name();
        T::from_owned_variant(self).ok_or(VariantParseError {
            target: std::any::type_name::<T>(),
            actual,
        })
    }

    /// Decode this [`Variant`] into typed value while consuming it, returning `None` on mismatch.
    #[inline]
    pub fn into_type<T>(self) -> Option<T>
    where
        T: DeriveVariant,
    {
        T::from_owned_variant(self)
    }
}

impl DeriveVariant for Variant {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        Some(value.clone())
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        Some(value)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.clone()
    }

    #[inline]
    fn into_variant(self) -> Variant {
        self
    }
}

impl DeriveVariant for bool {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_bool()
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for () {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        matches!(value, Variant::Null).then_some(())
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Null
    }
}

macro_rules! impl_statefield_signed {
    ($ty:ty, $pat:pat => $expr:expr) => {
        impl DeriveVariant for $ty {
            #[inline]
            fn from_variant(value: &Variant) -> Option<Self> {
                match value.as_number() {
                    Some($pat) => Some($expr),
                    _ => None,
                }
            }

            #[inline]
            fn to_variant(&self) -> Variant {
                Variant::from(*self)
            }
        }
    };
}

macro_rules! impl_statefield_unsigned {
    ($ty:ty, $pat:pat => $expr:expr) => {
        impl DeriveVariant for $ty {
            #[inline]
            fn from_variant(value: &Variant) -> Option<Self> {
                match value.as_number() {
                    Some($pat) => Some($expr),
                    _ => None,
                }
            }

            #[inline]
            fn to_variant(&self) -> Variant {
                Variant::from(*self)
            }
        }
    };
}

impl_statefield_signed!(i8, Number::I8(v) => v);
impl_statefield_signed!(i16, Number::I16(v) => v);
impl_statefield_signed!(i32, Number::I32(v) => v);
impl_statefield_signed!(i64, Number::I64(v) => v);
impl_statefield_signed!(i128, Number::I128(v) => v);

impl_statefield_unsigned!(u8, Number::U8(v) => v);
impl_statefield_unsigned!(u16, Number::U16(v) => v);
impl_statefield_unsigned!(u32, Number::U32(v) => v);
impl_statefield_unsigned!(u64, Number::U64(v) => v);
impl_statefield_unsigned!(u128, Number::U128(v) => v);

macro_rules! impl_nonzero_derive_variant {
    ($nonzero:ty, $inner:ty) => {
        impl DeriveVariant for $nonzero {
            #[inline]
            fn from_variant(value: &Variant) -> Option<Self> {
                <$inner as DeriveVariant>::from_variant(value).and_then(Self::new)
            }

            #[inline]
            fn from_owned_variant(value: Variant) -> Option<Self> {
                <$inner as DeriveVariant>::from_owned_variant(value).and_then(Self::new)
            }

            #[inline]
            fn to_variant(&self) -> Variant {
                <$inner as DeriveVariant>::to_variant(&self.get())
            }
        }
    };
}

impl_nonzero_derive_variant!(NonZeroI8, i8);
impl_nonzero_derive_variant!(NonZeroI16, i16);
impl_nonzero_derive_variant!(NonZeroI32, i32);
impl_nonzero_derive_variant!(NonZeroI64, i64);
impl_nonzero_derive_variant!(NonZeroI128, i128);
impl_nonzero_derive_variant!(NonZeroIsize, isize);
impl_nonzero_derive_variant!(NonZeroU8, u8);
impl_nonzero_derive_variant!(NonZeroU16, u16);
impl_nonzero_derive_variant!(NonZeroU32, u32);
impl_nonzero_derive_variant!(NonZeroU64, u64);
impl_nonzero_derive_variant!(NonZeroU128, u128);
impl_nonzero_derive_variant!(NonZeroUsize, usize);

impl DeriveVariant for isize {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        match value.as_number() {
            Some(Number::I64(v)) => isize::try_from(v).ok(),
            _ => None,
        }
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        i64::try_from(*self)
            .map(Variant::from)
            .unwrap_or(Variant::Null)
    }
}

impl DeriveVariant for usize {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        match value.as_number() {
            Some(Number::U64(v)) => usize::try_from(v).ok(),
            _ => None,
        }
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        u64::try_from(*self)
            .map(Variant::from)
            .unwrap_or(Variant::Null)
    }
}

impl DeriveVariant for f32 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_f32()
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for Unit {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_f32().map(Self::new).or_else(|| {
            value
                .as_number()
                .and_then(|value| value.as_u64_lossy())
                .and_then(|value| u8::try_from(value).ok())
                .map(Self::from_u8)
        })
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(self.to_f32())
    }
}

impl DeriveVariant for f64 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_f64()
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for String {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_str().map(ToString::to_string)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        value.as_str().map(ToString::to_string)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(self.clone())
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::from(self)
    }
}

impl DeriveVariant for char {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let mut chars = value.as_str()?.chars();
        let out = chars.next()?;
        chars.next().is_none().then_some(out)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(self.to_string())
    }
}

impl DeriveVariant for Arc<str> {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_str().map(Arc::<str>::from)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        match value {
            Variant::String(v) => Some(v),
            _ => None,
        }
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(Arc::clone(self))
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::from(self)
    }
}

impl DeriveVariant for Box<str> {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_str().map(Box::<str>::from)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        match value {
            Variant::String(v) => Some(Box::<str>::from(v.as_ref())),
            _ => None,
        }
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(self.as_ref())
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::from(String::from(self))
    }
}

impl DeriveVariant for Cow<'static, str> {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_str().map(|value| Cow::Owned(value.to_string()))
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        match value {
            Variant::String(v) => Some(Cow::Owned(v.to_string())),
            _ => None,
        }
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(self.as_ref())
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::from(self.into_owned())
    }
}

impl DeriveVariant for PathBuf {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_str().map(PathBuf::from)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        match value {
            Variant::String(v) => Some(PathBuf::from(v.as_ref())),
            _ => None,
        }
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.to_str().map(Variant::from).unwrap_or(Variant::Null)
    }

    #[inline]
    fn into_variant(self) -> Variant {
        self.into_os_string()
            .into_string()
            .map(Variant::from)
            .unwrap_or(Variant::Null)
    }
}

impl DeriveVariant for NodeID {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value
            .as_node()
            .or_else(|| {
                value
                    .as_number()
                    .and_then(|n| n.as_i64_lossy())
                    .and_then(|n| u64::try_from(n).ok())
                    .map(NodeID::from_u64)
            })
            .or_else(|| value.as_str().and_then(|s| NodeID::parse_str(s).ok()))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for TextureID {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value
            .as_texture()
            .or_else(|| {
                value
                    .as_number()
                    .and_then(|n| n.as_i64_lossy())
                    .and_then(|n| u64::try_from(n).ok())
                    .map(TextureID::from_u64)
            })
            .or_else(|| value.as_str().and_then(|s| TextureID::parse_str(s).ok()))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

macro_rules! impl_statefield_plain_id {
    ($id_ty:ty) => {
        impl DeriveVariant for $id_ty {
            #[inline]
            fn from_variant(value: &Variant) -> Option<Self> {
                value
                    .as_number()
                    .and_then(|n| n.as_u64_lossy())
                    .map(<$id_ty>::from_u64)
                    .or_else(|| value.as_str().and_then(|s| s.parse::<$id_ty>().ok()))
                    .or_else(|| value.as_id().map(IDs::as_u64).map(<$id_ty>::from_u64))
            }

            #[inline]
            fn to_variant(&self) -> Variant {
                Variant::from(*self)
            }
        }
    };
}

impl_statefield_plain_id!(MaterialID);
impl_statefield_plain_id!(MeshID);
impl_statefield_plain_id!(AnimationID);
impl_statefield_plain_id!(LightID);
impl_statefield_plain_id!(SignalID);
impl_statefield_plain_id!(AudioBusID);
impl_statefield_plain_id!(TagID);
impl_statefield_plain_id!(PreloadedSceneID);

impl DeriveVariant for Vector2 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_vec2() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let x = obj.get("x")?.as_f32()?;
        let y = obj.get("y")?.as_f32()?;
        Some(Vector2::new(x, y))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for Vector3 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_vec3() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let x = obj.get("x")?.as_f32()?;
        let y = obj.get("y")?.as_f32()?;
        let z = obj.get("z")?.as_f32()?;
        Some(Vector3::new(x, y, z))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for Vector4 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_vec4() {
            return Some(v);
        }
        if let Variant::Array(values) = value
            && values.len() == 4
        {
            return Some(Self::new(
                values[0].as_f32()?,
                values[1].as_f32()?,
                values[2].as_f32()?,
                values[3].as_f32()?,
            ));
        }
        let obj = value.as_object()?;
        let x = obj.get("x")?.as_f32()?;
        let y = obj.get("y")?.as_f32()?;
        let z = obj.get("z")?.as_f32()?;
        let w = obj.get("w")?.as_f32()?;
        Some(Vector4::new(x, y, z, w))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for IVector2 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_ivec2() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let x = obj.get("x")?.as_number()?.as_i64_lossy()?;
        let y = obj.get("y")?.as_number()?.as_i64_lossy()?;
        Some(IVector2::new(
            i32::try_from(x).ok()?,
            i32::try_from(y).ok()?,
        ))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for IVector3 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_ivec3() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let x = obj.get("x")?.as_number()?.as_i64_lossy()?;
        let y = obj.get("y")?.as_number()?.as_i64_lossy()?;
        let z = obj.get("z")?.as_number()?.as_i64_lossy()?;
        Some(IVector3::new(
            i32::try_from(x).ok()?,
            i32::try_from(y).ok()?,
            i32::try_from(z).ok()?,
        ))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for IVector4 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_ivec4() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let x = i32::try_from(obj.get("x")?.as_number()?.as_i64_lossy()?).ok()?;
        let y = i32::try_from(obj.get("y")?.as_number()?.as_i64_lossy()?).ok()?;
        let z = i32::try_from(obj.get("z")?.as_number()?.as_i64_lossy()?).ok()?;
        let w = i32::try_from(obj.get("w")?.as_number()?.as_i64_lossy()?).ok()?;
        Some(IVector4::new(x, y, z, w))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for UVector2 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_uvec2() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let x = variant_to_u32(obj.get("x")?)?;
        let y = variant_to_u32(obj.get("y")?)?;
        Some(UVector2::new(x, y))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for UVector3 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_uvec3() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let x = variant_to_u32(obj.get("x")?)?;
        let y = variant_to_u32(obj.get("y")?)?;
        let z = variant_to_u32(obj.get("z")?)?;
        Some(UVector3::new(x, y, z))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for UVector4 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_uvec4() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let x = variant_to_u32(obj.get("x")?)?;
        let y = variant_to_u32(obj.get("y")?)?;
        let z = variant_to_u32(obj.get("z")?)?;
        let w = variant_to_u32(obj.get("w")?)?;
        Some(UVector4::new(x, y, z, w))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for UnitVector2 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_unit_vec2() {
            return Some(v);
        }
        if let Some(v) = value.as_vec2() {
            return Some(Self::from(v));
        }
        let obj = value.as_object()?;
        let x = obj.get("x")?.as_f32()?;
        let y = obj.get("y")?.as_f32()?;
        Some(Self::new(x, y))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for UnitVector3 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_unit_vec3() {
            return Some(v);
        }
        if let Some(v) = value.as_vec3() {
            return Some(Self::from(v));
        }
        let obj = value.as_object()?;
        let x = obj.get("x")?.as_f32()?;
        let y = obj.get("y")?.as_f32()?;
        let z = obj.get("z")?.as_f32()?;
        Some(Self::new(x, y, z))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for UnitVector4 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_unit_vec4() {
            return Some(v);
        }
        if let Some(v) = value.as_vec4() {
            return Some(Self::new(v.to_array()));
        }
        if let Variant::Array(values) = value
            && values.len() == 4
        {
            return Some(Self::new([
                values[0].as_f32()?,
                values[1].as_f32()?,
                values[2].as_f32()?,
                values[3].as_f32()?,
            ]));
        }
        let obj = value.as_object()?;
        let x = obj.get("x")?.as_f32()?;
        let y = obj.get("y")?.as_f32()?;
        let z = obj.get("z")?.as_f32()?;
        let w = obj.get("w")?.as_f32()?;
        Some(Self::new([x, y, z, w]))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for Matrix2 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value
            .as_matrix2()
            .or_else(|| parse_matrix_rows::<2>(value).map(Self::from_rows))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for Matrix3 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value
            .as_matrix3()
            .or_else(|| parse_matrix_rows::<3>(value).map(Self::from_rows))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for Matrix4 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value
            .as_matrix4()
            .or_else(|| parse_matrix_rows::<4>(value).map(Self::from_rows))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl<const ROWS: usize, const COLS: usize, T> DeriveVariant for Matrix<ROWS, COLS, T>
where
    T: VariantMatrixCell,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        // Cell types handle their own lossy numeric coercion (see the
        // `VariantMatrixCell` impls for `f32`/`f64`), so a single direct
        // parse covers every rescuable shape. A `serde_json` round-trip
        // fallback used to run here on first-parse miss, but it could never
        // rescue integer cell types (JSON normalizes all integers to
        // `I64`/`U64`, which still fails an exact-match `iN`/`uN` parse)
        // and floats are now rescued directly without serializing at all.
        parse_matrix_rows_generic(value)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        matrix_to_fast_variant(self).unwrap_or_else(|| matrix_to_variant_array(self))
    }
}

macro_rules! impl_variant_matrix_cell {
    ($($ty:ty),* $(,)?) => {
        $(
            impl VariantMatrixCell for $ty {
                #[inline]
                fn from_matrix_cell_variant(value: &Variant) -> Option<Self> {
                    <Self as DeriveVariant>::from_variant(value)
                }

                #[inline]
                fn to_matrix_cell_variant(&self) -> Variant {
                    <Self as DeriveVariant>::to_variant(self)
                }
            }
        )*
    };
}

impl_variant_matrix_cell!(
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
    String,
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
);

impl VariantMatrixCell for f32 {
    #[inline]
    fn from_matrix_cell_variant(value: &Variant) -> Option<Self> {
        value.as_number()?.as_f64_lossy().map(|value| value as f32)
    }

    #[inline]
    fn to_matrix_cell_variant(&self) -> Variant {
        <Self as DeriveVariant>::to_variant(self)
    }

    #[inline]
    fn as_matrix_cell_f32(&self) -> Option<f32> {
        Some(*self)
    }
}

impl VariantMatrixCell for f64 {
    // Lossy numeric retry (mirrors the `from_matrix_cell_variant` half of
    // the `f32` impl above): accepts any numeric variant, not just an exact
    // `Number::F64` match. This is what rescues e.g. a matrix cell that was
    // authored/typed as `f32` but the matrix element type is `f64` —
    // previously handled by round-tripping the whole `Variant` through
    // `serde_json`, which only ever widened `F32 -> F64` anyway (integer
    // cell types can never be rescued by a JSON round trip, since ints
    // always come back as `I64`/`U64`).
    //
    // Note: `as_matrix_cell_f32` is intentionally left at the trait default
    // (`None`), not overridden to `Some(*self as f32)` — doing so would
    // route square f64 matrices through the f32-backed `Matrix2/3/4` fast
    // `to_variant()` path and silently lose precision on serialize. This
    // fix only touches deserialization (`from_variant`).
    #[inline]
    fn from_matrix_cell_variant(value: &Variant) -> Option<Self> {
        value.as_number()?.as_f64_lossy()
    }

    #[inline]
    fn to_matrix_cell_variant(&self) -> Variant {
        <Self as DeriveVariant>::to_variant(self)
    }
}

impl<const ROWS: usize, const COLS: usize, T> VariantMatrixCell for Matrix<ROWS, COLS, T>
where
    T: VariantMatrixCell,
{
    #[inline]
    fn from_matrix_cell_variant(value: &Variant) -> Option<Self> {
        <Self as DeriveVariant>::from_variant(value)
    }

    #[inline]
    fn to_matrix_cell_variant(&self) -> Variant {
        <Self as DeriveVariant>::to_variant(self)
    }
}

impl DeriveVariant for Quaternion {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_quat() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let x = obj.get("x")?.as_f32()?;
        let y = obj.get("y")?.as_f32()?;
        let z = obj.get("z")?.as_f32()?;
        let w = obj.get("w")?.as_f32()?;
        Some(Quaternion::new(x, y, z, w))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for Transform2D {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_transform2d() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let position = Vector2::from_variant(obj.get("position")?)?;
        let scale = Vector2::from_variant(obj.get("scale")?)?;
        let rotation = obj.get("rotation")?.as_f32()?;
        Some(Transform2D::new(position, rotation, scale))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for Transform3D {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_transform3d() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let position = Vector3::from_variant(obj.get("position")?)?;
        let scale = Vector3::from_variant(obj.get("scale")?)?;
        let rotation = Quaternion::from_variant(obj.get("rotation")?)?;
        Some(Transform3D::new(position, rotation, scale))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for PostProcessSet {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_post_process_set().cloned()
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        match value {
            Variant::EngineStruct(EngineStruct::PostProcessSet(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(self.clone())
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::from(self)
    }
}

impl DeriveVariant for VisualAccessibilitySettings {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_visual_accessibility_settings()
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl<T> DeriveVariant for Option<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if matches!(value, Variant::Null) {
            Some(None)
        } else {
            T::from_variant(value).map(Some)
        }
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        if matches!(value, Variant::Null) {
            Some(None)
        } else {
            T::from_owned_variant(value).map(Some)
        }
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        match self {
            Some(v) => v.to_variant(),
            None => Variant::Null,
        }
    }

    #[inline]
    fn into_variant(self) -> Variant {
        match self {
            Some(v) => v.into_variant(),
            None => Variant::Null,
        }
    }
}

impl<T, E> DeriveVariant for Result<T, E>
where
    T: DeriveVariant,
    E: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let obj = value.as_object()?;
        let tag = obj.get("__variant")?.as_str()?;
        let data = obj.get("__data")?;
        match tag {
            "Ok" => T::from_variant(data).map(Ok),
            "Err" => E::from_variant(data).map(Err),
            _ => None,
        }
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let mut obj = match value {
            Variant::Object(obj) => obj,
            _ => return None,
        };
        let tag = match obj.remove("__variant")? {
            Variant::String(tag) => tag,
            _ => return None,
        };
        let data = obj.remove("__data")?;
        match tag.as_ref() {
            "Ok" => T::from_owned_variant(data).map(Ok),
            "Err" => E::from_owned_variant(data).map(Err),
            _ => None,
        }
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        let mut obj = BTreeMap::new();
        match self {
            Ok(value) => {
                obj.insert(Arc::from("__variant"), Variant::String(Arc::from("Ok")));
                obj.insert(Arc::from("__data"), value.to_variant());
            }
            Err(err) => {
                obj.insert(Arc::from("__variant"), Variant::String(Arc::from("Err")));
                obj.insert(Arc::from("__data"), err.to_variant());
            }
        }
        Variant::Object(obj)
    }

    #[inline]
    fn into_variant(self) -> Variant {
        let mut obj = BTreeMap::new();
        match self {
            Ok(value) => {
                obj.insert(Arc::from("__variant"), Variant::String(Arc::from("Ok")));
                obj.insert(Arc::from("__data"), value.into_variant());
            }
            Err(err) => {
                obj.insert(Arc::from("__variant"), Variant::String(Arc::from("Err")));
                obj.insert(Arc::from("__data"), err.into_variant());
            }
        }
        Variant::Object(obj)
    }
}

impl<T> DeriveVariant for Box<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        T::from_variant(value).map(Box::new)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        T::from_owned_variant(value).map(Box::new)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.as_ref().to_variant()
    }

    #[inline]
    fn into_variant(self) -> Variant {
        (*self).into_variant()
    }
}

impl<T> DeriveVariant for Arc<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        T::from_variant(value).map(Arc::new)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        T::from_owned_variant(value).map(Arc::new)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.as_ref().to_variant()
    }

    #[inline]
    fn into_variant(self) -> Variant {
        self.as_ref().to_variant()
    }
}

impl<T> DeriveVariant for Rc<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        T::from_variant(value).map(Rc::new)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        T::from_owned_variant(value).map(Rc::new)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.as_ref().to_variant()
    }

    #[inline]
    fn into_variant(self) -> Variant {
        self.as_ref().to_variant()
    }
}

impl<T> DeriveVariant for Cell<T>
where
    T: Copy + DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        T::from_variant(value).map(Cell::new)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        T::from_owned_variant(value).map(Cell::new)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.get().to_variant()
    }

    #[inline]
    fn into_variant(self) -> Variant {
        self.into_inner().into_variant()
    }
}

macro_rules! impl_atomic_derive_variant {
    ($atomic:ty, $inner:ty) => {
        impl DeriveVariant for $atomic {
            #[inline]
            fn from_variant(value: &Variant) -> Option<Self> {
                <$inner as DeriveVariant>::from_variant(value).map(<$atomic>::new)
            }

            #[inline]
            fn from_owned_variant(value: Variant) -> Option<Self> {
                <$inner as DeriveVariant>::from_owned_variant(value).map(<$atomic>::new)
            }

            #[inline]
            fn to_variant(&self) -> Variant {
                self.load(Ordering::SeqCst).to_variant()
            }

            #[inline]
            fn into_variant(self) -> Variant {
                self.into_inner().into_variant()
            }
        }
    };
}

impl_atomic_derive_variant!(AtomicBool, bool);
impl_atomic_derive_variant!(AtomicI32, i32);
impl_atomic_derive_variant!(AtomicI64, i64);
impl_atomic_derive_variant!(AtomicU32, u32);
impl_atomic_derive_variant!(AtomicU64, u64);
impl_atomic_derive_variant!(AtomicUsize, usize);

impl<T> DeriveVariant for RefCell<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        T::from_variant(value).map(RefCell::new)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        T::from_owned_variant(value).map(RefCell::new)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.borrow().to_variant()
    }

    #[inline]
    fn into_variant(self) -> Variant {
        self.into_inner().into_variant()
    }
}

impl<T> DeriveVariant for Wrapping<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        T::from_variant(value).map(Wrapping)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        T::from_owned_variant(value).map(Wrapping)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.0.to_variant()
    }

    #[inline]
    fn into_variant(self) -> Variant {
        self.0.into_variant()
    }
}

impl<T> DeriveVariant for Saturating<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        T::from_variant(value).map(Saturating)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        T::from_owned_variant(value).map(Saturating)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.0.to_variant()
    }

    #[inline]
    fn into_variant(self) -> Variant {
        self.0.into_variant()
    }
}

impl<T> DeriveVariant for Reverse<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        T::from_variant(value).map(Reverse)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        T::from_owned_variant(value).map(Reverse)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.0.to_variant()
    }

    #[inline]
    fn into_variant(self) -> Variant {
        self.0.into_variant()
    }
}

impl<T, const N: usize> DeriveVariant for [T; N]
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        if items.len() != N {
            return None;
        }
        let mut out = Vec::with_capacity(N);
        for item in items {
            out.push(T::from_variant(item)?);
        }
        let boxed: Box<[T]> = out.into_boxed_slice();
        let boxed: Box<[T; N]> = boxed.try_into().ok()?;
        Some(*boxed)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        if items.len() != N {
            return None;
        }
        let mut out = Vec::with_capacity(N);
        for item in items {
            out.push(T::from_owned_variant(item)?);
        }
        let boxed: Box<[T]> = out.into_boxed_slice();
        let boxed: Box<[T; N]> = boxed.try_into().ok()?;
        Some(*boxed)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(self.iter().map(DeriveVariant::to_variant).collect())
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::Array(self.into_iter().map(DeriveVariant::into_variant).collect())
    }
}

impl<T> DeriveVariant for Box<[T]>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(T::from_variant(item)?);
        }
        Some(out.into_boxed_slice())
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(T::from_owned_variant(item)?);
        }
        Some(out.into_boxed_slice())
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(self.iter().map(DeriveVariant::to_variant).collect())
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::Array(
            Vec::from(self)
                .into_iter()
                .map(DeriveVariant::into_variant)
                .collect(),
        )
    }
}

impl<T> DeriveVariant for Arc<[T]>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(T::from_variant(item)?);
        }
        Some(Arc::from(out.into_boxed_slice()))
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(T::from_owned_variant(item)?);
        }
        Some(Arc::from(out.into_boxed_slice()))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(self.iter().map(DeriveVariant::to_variant).collect())
    }
}

impl<T> DeriveVariant for Rc<[T]>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(T::from_variant(item)?);
        }
        Some(Rc::from(out.into_boxed_slice()))
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(T::from_owned_variant(item)?);
        }
        Some(Rc::from(out.into_boxed_slice()))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(self.iter().map(DeriveVariant::to_variant).collect())
    }
}

impl<T> DeriveVariant for Vec<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(T::from_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(T::from_owned_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(self.iter().map(DeriveVariant::to_variant).collect())
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::Array(self.into_iter().map(DeriveVariant::into_variant).collect())
    }
}

impl<T> DeriveVariant for VecDeque<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        let mut out = VecDeque::with_capacity(items.len());
        for item in items {
            out.push_back(T::from_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        let mut out = VecDeque::with_capacity(items.len());
        for item in items {
            out.push_back(T::from_owned_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(self.iter().map(DeriveVariant::to_variant).collect())
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::Array(self.into_iter().map(DeriveVariant::into_variant).collect())
    }
}

impl<T> DeriveVariant for LinkedList<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        let mut out = LinkedList::new();
        for item in items {
            out.push_back(T::from_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        let mut out = LinkedList::new();
        for item in items {
            out.push_back(T::from_owned_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(self.iter().map(DeriveVariant::to_variant).collect())
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::Array(self.into_iter().map(DeriveVariant::into_variant).collect())
    }
}

impl<T> DeriveVariant for BinaryHeap<T>
where
    T: Ord + DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        let mut out = BinaryHeap::with_capacity(items.len());
        for item in items {
            out.push(T::from_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        let mut out = BinaryHeap::with_capacity(items.len());
        for item in items {
            out.push(T::from_owned_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(self.iter().map(DeriveVariant::to_variant).collect())
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::Array(
            self.into_vec()
                .into_iter()
                .map(DeriveVariant::into_variant)
                .collect(),
        )
    }
}

macro_rules! impl_tuple_derive_variant {
    ($len:expr; $($ty:ident: $idx:tt),+ $(,)?) => {
        impl<$($ty),+> DeriveVariant for ($($ty,)+)
        where
            $($ty: DeriveVariant),+
        {
            #[inline]
            fn from_variant(value: &Variant) -> Option<Self> {
                let items = value.as_array()?;
                if items.len() != $len {
                    return None;
                }
                Some(($(<$ty as DeriveVariant>::from_variant(items.get($idx)?)?,)+))
            }

            #[inline]
            fn from_owned_variant(value: Variant) -> Option<Self> {
                let items = match value {
                    Variant::Array(items) => items,
                    _ => return None,
                };
                if items.len() != $len {
                    return None;
                }
                let mut items = items.into_iter();
                Some(($(<$ty as DeriveVariant>::from_owned_variant(items.next()?)?,)+))
            }

            #[inline]
            fn to_variant(&self) -> Variant {
                Variant::Array(vec![$(self.$idx.to_variant()),+])
            }

            #[inline]
            fn into_variant(self) -> Variant {
                Variant::Array(vec![$(self.$idx.into_variant()),+])
            }
        }

        impl<$($ty),+> VariantSchema for ($($ty,)+) {}
    };
}

impl_tuple_derive_variant!(2; A: 0, B: 1);
impl_tuple_derive_variant!(3; A: 0, B: 1, C: 2);
impl_tuple_derive_variant!(4; A: 0, B: 1, C: 2, D: 3);
impl_tuple_derive_variant!(5; A: 0, B: 1, C: 2, D: 3, E: 4);
impl_tuple_derive_variant!(6; A: 0, B: 1, C: 2, D: 3, E: 4, F: 5);

trait VariantObjectKey: Sized {
    fn from_arc_key(key: Arc<str>) -> Self;
    fn to_arc_key(&self) -> Arc<str>;
    fn into_arc_key(self) -> Arc<str>;
}

impl VariantObjectKey for Arc<str> {
    #[inline]
    fn from_arc_key(key: Arc<str>) -> Self {
        key
    }

    #[inline]
    fn to_arc_key(&self) -> Arc<str> {
        Arc::clone(self)
    }

    #[inline]
    fn into_arc_key(self) -> Arc<str> {
        self
    }
}

impl VariantObjectKey for String {
    #[inline]
    fn from_arc_key(key: Arc<str>) -> Self {
        key.to_string()
    }

    #[inline]
    fn to_arc_key(&self) -> Arc<str> {
        Arc::from(self.as_str())
    }

    #[inline]
    fn into_arc_key(self) -> Arc<str> {
        Arc::from(self)
    }
}

impl VariantObjectKey for Box<str> {
    #[inline]
    fn from_arc_key(key: Arc<str>) -> Self {
        Box::from(key.as_ref())
    }

    #[inline]
    fn to_arc_key(&self) -> Arc<str> {
        Arc::from(self.as_ref())
    }

    #[inline]
    fn into_arc_key(self) -> Arc<str> {
        Arc::from(String::from(self))
    }
}

impl VariantObjectKey for Cow<'static, str> {
    #[inline]
    fn from_arc_key(key: Arc<str>) -> Self {
        Cow::Owned(key.to_string())
    }

    #[inline]
    fn to_arc_key(&self) -> Arc<str> {
        Arc::from(self.as_ref())
    }

    #[inline]
    fn into_arc_key(self) -> Arc<str> {
        Arc::from(self.into_owned())
    }
}

impl<K, T> DeriveVariant for BTreeMap<K, T>
where
    K: Ord + VariantObjectKey,
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let object = value.as_object()?;
        let mut out = BTreeMap::new();
        for (k, v) in object {
            out.insert(K::from_arc_key(Arc::clone(k)), T::from_variant(v)?);
        }
        Some(out)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let object = match value {
            Variant::Object(object) => object,
            _ => return None,
        };
        let mut out = BTreeMap::new();
        for (k, v) in object {
            out.insert(K::from_arc_key(k), T::from_owned_variant(v)?);
        }
        Some(out)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        let mut out = BTreeMap::new();
        for (k, v) in self {
            out.insert(k.to_arc_key(), v.to_variant());
        }
        Variant::Object(out)
    }

    #[inline]
    fn into_variant(self) -> Variant {
        let mut out = BTreeMap::new();
        for (k, v) in self {
            out.insert(k.into_arc_key(), v.into_variant());
        }
        Variant::Object(out)
    }
}

impl<K, T> DeriveVariant for HashMap<K, T>
where
    K: Eq + Hash + VariantObjectKey,
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let object = value.as_object()?;
        let mut out = HashMap::with_capacity(object.len());
        for (k, v) in object {
            out.insert(K::from_arc_key(Arc::clone(k)), T::from_variant(v)?);
        }
        Some(out)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let object = match value {
            Variant::Object(object) => object,
            _ => return None,
        };
        let mut out = HashMap::with_capacity(object.len());
        for (k, v) in object {
            out.insert(K::from_arc_key(k), T::from_owned_variant(v)?);
        }
        Some(out)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        let mut out = BTreeMap::new();
        for (k, v) in self {
            out.insert(k.to_arc_key(), v.to_variant());
        }
        Variant::Object(out)
    }

    #[inline]
    fn into_variant(self) -> Variant {
        let mut out = BTreeMap::new();
        for (k, v) in self {
            out.insert(k.into_arc_key(), v.into_variant());
        }
        Variant::Object(out)
    }
}

impl<T> DeriveVariant for BTreeSet<T>
where
    T: Ord + DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        let mut out = BTreeSet::new();
        for item in items {
            out.insert(T::from_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        let mut out = BTreeSet::new();
        for item in items {
            out.insert(T::from_owned_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(self.iter().map(DeriveVariant::to_variant).collect())
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::Array(self.into_iter().map(DeriveVariant::into_variant).collect())
    }
}

impl<T> DeriveVariant for HashSet<T>
where
    T: Eq + Hash + DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        let mut out = HashSet::with_capacity(items.len());
        for item in items {
            out.insert(T::from_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        let mut out = HashSet::with_capacity(items.len());
        for item in items {
            out.insert(T::from_owned_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(self.iter().map(DeriveVariant::to_variant).collect())
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::Array(self.into_iter().map(DeriveVariant::into_variant).collect())
    }
}

impl<T> DeriveVariant for Range<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        if items.len() != 2 {
            return None;
        }
        let start = T::from_variant(&items[0])?;
        let end = T::from_variant(&items[1])?;
        Some(start..end)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        if items.len() != 2 {
            return None;
        }
        let mut items = items.into_iter();
        let start = T::from_owned_variant(items.next()?)?;
        let end = T::from_owned_variant(items.next()?)?;
        Some(start..end)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(vec![self.start.to_variant(), self.end.to_variant()])
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::Array(vec![self.start.into_variant(), self.end.into_variant()])
    }
}

impl<T> DeriveVariant for RangeInclusive<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        if items.len() != 2 {
            return None;
        }
        let start = T::from_variant(&items[0])?;
        let end = T::from_variant(&items[1])?;
        Some(start..=end)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        if items.len() != 2 {
            return None;
        }
        let mut items = items.into_iter();
        let start = T::from_owned_variant(items.next()?)?;
        let end = T::from_owned_variant(items.next()?)?;
        Some(start..=end)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(vec![self.start().to_variant(), self.end().to_variant()])
    }

    #[inline]
    fn into_variant(self) -> Variant {
        let (start, end) = self.into_inner();
        Variant::Array(vec![start.into_variant(), end.into_variant()])
    }
}

impl DeriveVariant for Duration {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(secs) = value.as_f64() {
            return Duration::try_from_secs_f64(secs).ok();
        }
        if let Some(secs) = value.as_u64() {
            return Some(Duration::from_secs(secs));
        }
        let obj = value.as_object()?;
        let secs = obj.get("secs")?.as_u64()?;
        let nanos = obj.get("nanos")?.as_u32()?;
        (nanos < 1_000_000_000).then_some(Duration::new(secs, nanos))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        let mut out = BTreeMap::new();
        out.insert(Arc::from("secs"), Variant::from(self.as_secs()));
        out.insert(Arc::from("nanos"), Variant::from(self.subsec_nanos()));
        Variant::Object(out)
    }
}

impl DeriveVariant for SystemTime {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let duration = Duration::from_variant(value)?;
        UNIX_EPOCH.checked_add(duration)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let duration = Duration::from_owned_variant(value)?;
        UNIX_EPOCH.checked_add(duration)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.duration_since(UNIX_EPOCH)
            .map(|duration| duration.to_variant())
            .unwrap_or(Variant::Null)
    }
}

// -------------------- Accessors (extensible pattern) --------------------

impl Variant {
    #[inline]
    pub fn kind(&self) -> VariantKind {
        match self {
            Variant::Null => VariantKind::Null,
            Variant::Bool(_) => VariantKind::Bool,
            Variant::Number(_) => VariantKind::Number,
            Variant::String(_) => VariantKind::String,
            Variant::Bytes(_) => VariantKind::Bytes,
            Variant::ID(_) => VariantKind::ID,
            Variant::EngineStruct(_) => VariantKind::EngineStruct,
            Variant::Array(_) => VariantKind::Array,
            Variant::Object(_) => VariantKind::Object,
        }
    }

    #[deprecated(note = "use Variant::kind()")]
    #[inline]
    pub fn get_kind(&self) -> VariantKind {
        self.kind()
    }

    #[inline]
    pub fn kind_name(&self) -> &'static str {
        self.kind().as_str()
    }

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
    pub fn as_i8(&self) -> Option<i8> {
        match *self {
            Variant::Number(Number::I8(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_i16(&self) -> Option<i16> {
        match *self {
            Variant::Number(Number::I16(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_i32(&self) -> Option<i32> {
        match *self {
            Variant::Number(Number::I32(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Variant::Number(Number::I64(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_i128(&self) -> Option<i128> {
        match *self {
            Variant::Number(Number::I128(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_u8(&self) -> Option<u8> {
        match *self {
            Variant::Number(Number::U8(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_u16(&self) -> Option<u16> {
        match *self {
            Variant::Number(Number::U16(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_u32(&self) -> Option<u32> {
        match *self {
            Variant::Number(Number::U32(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_u64(&self) -> Option<u64> {
        match *self {
            Variant::Number(Number::U64(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_u128(&self) -> Option<u128> {
        match *self {
            Variant::Number(Number::U128(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_f32(&self) -> Option<f32> {
        match *self {
            Variant::Number(Number::F32(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Variant::Number(Number::F64(v)) => Some(v),
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
            Variant::ID(IDs::Node(id)) => Some(id),
            _ => None,
        }
    }

    /// Node id if this holds one, else [`NodeID::nil`]. Convenience for the
    /// common "read a node-ref var, default to nil" pattern.
    #[inline]
    pub fn as_node_or_nil(&self) -> NodeID {
        self.as_node().unwrap_or(NodeID::nil())
    }

    #[inline]
    pub fn as_texture(&self) -> Option<TextureID> {
        match *self {
            Variant::ID(IDs::Texture(id)) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_material(&self) -> Option<MaterialID> {
        match *self {
            Variant::ID(IDs::Material(id)) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_mesh(&self) -> Option<MeshID> {
        match *self {
            Variant::ID(IDs::Mesh(id)) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_animation(&self) -> Option<AnimationID> {
        match *self {
            Variant::ID(IDs::Animation(id)) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_light(&self) -> Option<LightID> {
        match *self {
            Variant::ID(IDs::Light(id)) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_signal(&self) -> Option<SignalID> {
        match *self {
            Variant::ID(IDs::Signal(id)) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_audio_bus(&self) -> Option<AudioBusID> {
        match *self {
            Variant::ID(IDs::AudioBus(id)) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_tag(&self) -> Option<TagID> {
        match *self {
            Variant::ID(IDs::Tag(id)) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_preloaded_scene(&self) -> Option<PreloadedSceneID> {
        match *self {
            Variant::ID(IDs::PreloadedScene(id)) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_id(&self) -> Option<IDs> {
        match *self {
            Variant::ID(id) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_vec2(&self) -> Option<Vector2> {
        match self {
            Variant::EngineStruct(EngineStruct::Vector2(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_vec3(&self) -> Option<Vector3> {
        match self {
            Variant::EngineStruct(EngineStruct::Vector3(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_vec4(&self) -> Option<Vector4> {
        match self {
            Variant::EngineStruct(EngineStruct::Vector4(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_ivec2(&self) -> Option<IVector2> {
        match self {
            Variant::EngineStruct(EngineStruct::IVector2(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_ivec3(&self) -> Option<IVector3> {
        match self {
            Variant::EngineStruct(EngineStruct::IVector3(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_ivec4(&self) -> Option<IVector4> {
        match self {
            Variant::EngineStruct(EngineStruct::IVector4(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_uvec2(&self) -> Option<UVector2> {
        match self {
            Variant::EngineStruct(EngineStruct::UVector2(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_uvec3(&self) -> Option<UVector3> {
        match self {
            Variant::EngineStruct(EngineStruct::UVector3(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_uvec4(&self) -> Option<UVector4> {
        match self {
            Variant::EngineStruct(EngineStruct::UVector4(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unit_vec2(&self) -> Option<UnitVector2> {
        match self {
            Variant::EngineStruct(EngineStruct::UnitVector2(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unit_vec3(&self) -> Option<UnitVector3> {
        match self {
            Variant::EngineStruct(EngineStruct::UnitVector3(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unit_vec4(&self) -> Option<UnitVector4> {
        match self {
            Variant::EngineStruct(EngineStruct::UnitVector4(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_matrix2(&self) -> Option<Matrix2> {
        match self {
            Variant::EngineStruct(EngineStruct::Matrix2(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_matrix3(&self) -> Option<Matrix3> {
        match self {
            Variant::EngineStruct(EngineStruct::Matrix3(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_matrix4(&self) -> Option<Matrix4> {
        match self {
            Variant::EngineStruct(EngineStruct::Matrix4(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_matrix2x2(&self) -> Option<Matrix<2, 2, f32>> {
        self.as_matrix2().map(Matrix::<2, 2>::from)
    }

    #[inline]
    pub fn as_matrix3x3(&self) -> Option<Matrix<3, 3, f32>> {
        self.as_matrix3().map(Matrix::<3, 3>::from)
    }

    #[inline]
    pub fn as_matrix4x4(&self) -> Option<Matrix<4, 4, f32>> {
        self.as_matrix4().map(Matrix::<4, 4>::from)
    }

    #[inline]
    pub fn matrix_shape(&self) -> Option<MatrixShape> {
        match self {
            Variant::EngineStruct(EngineStruct::Matrix2(_)) => {
                Some(MatrixShape::new(2, 2, MatrixCellType::F32))
            }
            Variant::EngineStruct(EngineStruct::Matrix3(_)) => {
                Some(MatrixShape::new(3, 3, MatrixCellType::F32))
            }
            Variant::EngineStruct(EngineStruct::Matrix4(_)) => {
                Some(MatrixShape::new(4, 4, MatrixCellType::F32))
            }
            Variant::Object(obj) => obj.get("rows")?.matrix_shape(),
            Variant::Array(rows) => {
                let first_row = rows.first()?.as_array()?;
                let cols = first_row.len();
                let mut cell_type = first_row
                    .first()
                    .and_then(matrix_cell_type_for_variant)
                    .unwrap_or(MatrixCellType::Null);

                for row in rows {
                    let row = row.as_array()?;
                    if row.len() != cols {
                        return None;
                    }

                    for cell in row {
                        let next_type = matrix_cell_type_for_variant(cell)?;
                        if cell_type != next_type {
                            cell_type = MatrixCellType::Mixed;
                        }
                    }
                }

                Some(MatrixShape::new(rows.len(), cols, cell_type))
            }
            _ => None,
        }
    }

    #[inline]
    pub fn as_transform2(&self) -> Option<Transform2D> {
        match self {
            Variant::EngineStruct(EngineStruct::Transform2D(t)) => Some(*t),
            _ => None,
        }
    }

    #[inline]
    pub fn as_transform3(&self) -> Option<Transform3D> {
        match self {
            Variant::EngineStruct(EngineStruct::Transform3D(t)) => Some(*t),
            _ => None,
        }
    }

    #[inline]
    pub fn as_transform2d(&self) -> Option<Transform2D> {
        self.as_transform2()
    }

    #[inline]
    pub fn as_transform3d(&self) -> Option<Transform3D> {
        self.as_transform3()
    }

    #[inline]
    pub fn as_quat(&self) -> Option<Quaternion> {
        match self {
            Variant::EngineStruct(EngineStruct::Quaternion(q)) => Some(*q),
            _ => None,
        }
    }

    #[inline]
    pub fn as_post_process_set(&self) -> Option<&PostProcessSet> {
        match self {
            Variant::EngineStruct(EngineStruct::PostProcessSet(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_visual_accessibility_settings(&self) -> Option<VisualAccessibilitySettings> {
        match self {
            Variant::EngineStruct(EngineStruct::VisualAccessibilitySettings(v)) => Some(*v),
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

fn matrix_cell_type_for_variant(value: &Variant) -> Option<MatrixCellType> {
    Some(match value {
        Variant::Null => MatrixCellType::Null,
        Variant::Bool(_) => MatrixCellType::Bool,
        Variant::Number(Number::I8(_)) => MatrixCellType::I8,
        Variant::Number(Number::I16(_)) => MatrixCellType::I16,
        Variant::Number(Number::I32(_)) => MatrixCellType::I32,
        Variant::Number(Number::I64(_)) => MatrixCellType::I64,
        Variant::Number(Number::I128(_)) => MatrixCellType::I128,
        Variant::Number(Number::U8(_)) => MatrixCellType::U8,
        Variant::Number(Number::U16(_)) => MatrixCellType::U16,
        Variant::Number(Number::U32(_)) => MatrixCellType::U32,
        Variant::Number(Number::U64(_)) => MatrixCellType::U64,
        Variant::Number(Number::U128(_)) => MatrixCellType::U128,
        Variant::Number(Number::F32(_)) => MatrixCellType::F32,
        Variant::Number(Number::F64(_)) => MatrixCellType::F64,
        Variant::String(_) => MatrixCellType::String,
        Variant::Bytes(_) => MatrixCellType::Bytes,
        Variant::ID(_) => MatrixCellType::ID,
        Variant::EngineStruct(EngineStruct::Vector2(_)) => MatrixCellType::Vector2,
        Variant::EngineStruct(EngineStruct::Vector3(_)) => MatrixCellType::Vector3,
        Variant::EngineStruct(EngineStruct::Vector4(_)) => MatrixCellType::Vector4,
        Variant::EngineStruct(EngineStruct::IVector2(_)) => MatrixCellType::IVector2,
        Variant::EngineStruct(EngineStruct::IVector3(_)) => MatrixCellType::IVector3,
        Variant::EngineStruct(EngineStruct::IVector4(_)) => MatrixCellType::IVector4,
        Variant::EngineStruct(EngineStruct::UVector2(_)) => MatrixCellType::UVector2,
        Variant::EngineStruct(EngineStruct::UVector3(_)) => MatrixCellType::UVector3,
        Variant::EngineStruct(EngineStruct::UVector4(_)) => MatrixCellType::UVector4,
        Variant::EngineStruct(EngineStruct::UnitVector2(_)) => MatrixCellType::UnitVector2,
        Variant::EngineStruct(EngineStruct::UnitVector3(_)) => MatrixCellType::UnitVector3,
        Variant::EngineStruct(EngineStruct::UnitVector4(_)) => MatrixCellType::UnitVector4,
        Variant::EngineStruct(EngineStruct::Matrix2(_)) => {
            MatrixCellType::Matrix(Box::new(MatrixShape::new(2, 2, MatrixCellType::F32)))
        }
        Variant::EngineStruct(EngineStruct::Matrix3(_)) => {
            MatrixCellType::Matrix(Box::new(MatrixShape::new(3, 3, MatrixCellType::F32)))
        }
        Variant::EngineStruct(EngineStruct::Matrix4(_)) => {
            MatrixCellType::Matrix(Box::new(MatrixShape::new(4, 4, MatrixCellType::F32)))
        }
        Variant::EngineStruct(EngineStruct::Transform2D(_)) => MatrixCellType::Transform2D,
        Variant::EngineStruct(EngineStruct::Transform3D(_)) => MatrixCellType::Transform3D,
        Variant::EngineStruct(EngineStruct::Quaternion(_)) => MatrixCellType::Quaternion,
        Variant::EngineStruct(EngineStruct::PostProcessSet(_)) => MatrixCellType::PostProcessSet,
        Variant::EngineStruct(EngineStruct::VisualAccessibilitySettings(_)) => {
            MatrixCellType::VisualAccessibilitySettings
        }
        Variant::Array(_) => value
            .matrix_shape()
            .map(|shape| MatrixCellType::Matrix(Box::new(shape)))
            .unwrap_or(MatrixCellType::Array),
        Variant::Object(obj) => obj
            .get("rows")
            .and_then(Variant::matrix_shape)
            .map(|shape| MatrixCellType::Matrix(Box::new(shape)))
            .unwrap_or(MatrixCellType::Object),
    })
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

impl<T> From<Arc<T>> for Variant
where
    T: DeriveVariant,
{
    #[inline]
    fn from(v: Arc<T>) -> Self {
        v.to_variant()
    }
}

impl<T> From<Rc<T>> for Variant
where
    T: DeriveVariant,
{
    #[inline]
    fn from(v: Rc<T>) -> Self {
        v.to_variant()
    }
}

impl<T> From<RefCell<T>> for Variant
where
    T: DeriveVariant,
{
    #[inline]
    fn from(v: RefCell<T>) -> Self {
        v.into_variant()
    }
}

// Engine handles
impl From<NodeID> for Variant {
    #[inline]
    fn from(v: NodeID) -> Self {
        Variant::ID(IDs::Node(v))
    }
}
impl From<TextureID> for Variant {
    #[inline]
    fn from(v: TextureID) -> Self {
        Variant::ID(IDs::Texture(v))
    }
}
impl From<MaterialID> for Variant {
    #[inline]
    fn from(v: MaterialID) -> Self {
        Variant::ID(IDs::Material(v))
    }
}
impl From<MeshID> for Variant {
    #[inline]
    fn from(v: MeshID) -> Self {
        Variant::ID(IDs::Mesh(v))
    }
}
impl From<AnimationID> for Variant {
    #[inline]
    fn from(v: AnimationID) -> Self {
        Variant::ID(IDs::Animation(v))
    }
}
impl From<LightID> for Variant {
    #[inline]
    fn from(v: LightID) -> Self {
        Variant::ID(IDs::Light(v))
    }
}
impl From<SignalID> for Variant {
    #[inline]
    fn from(v: SignalID) -> Self {
        Variant::ID(IDs::Signal(v))
    }
}
impl From<AudioBusID> for Variant {
    #[inline]
    fn from(v: AudioBusID) -> Self {
        Variant::ID(IDs::AudioBus(v))
    }
}
impl From<TagID> for Variant {
    #[inline]
    fn from(v: TagID) -> Self {
        Variant::ID(IDs::Tag(v))
    }
}
impl From<PreloadedSceneID> for Variant {
    #[inline]
    fn from(v: PreloadedSceneID) -> Self {
        Variant::ID(IDs::PreloadedScene(v))
    }
}

// Math
impl From<Vector2> for Variant {
    #[inline]
    fn from(v: Vector2) -> Self {
        Variant::EngineStruct(EngineStruct::Vector2(v))
    }
}
impl From<Vector3> for Variant {
    #[inline]
    fn from(v: Vector3) -> Self {
        Variant::EngineStruct(EngineStruct::Vector3(v))
    }
}
impl From<Vector4> for Variant {
    #[inline]
    fn from(v: Vector4) -> Self {
        Variant::EngineStruct(EngineStruct::Vector4(v))
    }
}
impl From<IVector2> for Variant {
    #[inline]
    fn from(v: IVector2) -> Self {
        Variant::EngineStruct(EngineStruct::IVector2(v))
    }
}
impl From<IVector3> for Variant {
    #[inline]
    fn from(v: IVector3) -> Self {
        Variant::EngineStruct(EngineStruct::IVector3(v))
    }
}
impl From<IVector4> for Variant {
    #[inline]
    fn from(v: IVector4) -> Self {
        Variant::EngineStruct(EngineStruct::IVector4(v))
    }
}
impl From<UVector2> for Variant {
    #[inline]
    fn from(v: UVector2) -> Self {
        Variant::EngineStruct(EngineStruct::UVector2(v))
    }
}
impl From<UVector3> for Variant {
    #[inline]
    fn from(v: UVector3) -> Self {
        Variant::EngineStruct(EngineStruct::UVector3(v))
    }
}
impl From<UVector4> for Variant {
    #[inline]
    fn from(v: UVector4) -> Self {
        Variant::EngineStruct(EngineStruct::UVector4(v))
    }
}
impl From<UnitVector2> for Variant {
    #[inline]
    fn from(v: UnitVector2) -> Self {
        Variant::EngineStruct(EngineStruct::UnitVector2(v))
    }
}
impl From<UnitVector3> for Variant {
    #[inline]
    fn from(v: UnitVector3) -> Self {
        Variant::EngineStruct(EngineStruct::UnitVector3(v))
    }
}
impl From<UnitVector4> for Variant {
    #[inline]
    fn from(v: UnitVector4) -> Self {
        Variant::EngineStruct(EngineStruct::UnitVector4(v))
    }
}
impl From<Matrix2> for Variant {
    #[inline]
    fn from(v: Matrix2) -> Self {
        Variant::EngineStruct(EngineStruct::Matrix2(v))
    }
}
impl From<Matrix3> for Variant {
    #[inline]
    fn from(v: Matrix3) -> Self {
        Variant::EngineStruct(EngineStruct::Matrix3(v))
    }
}
impl From<Matrix4> for Variant {
    #[inline]
    fn from(v: Matrix4) -> Self {
        Variant::EngineStruct(EngineStruct::Matrix4(v))
    }
}
impl From<Matrix<2, 2, f32>> for Variant {
    #[inline]
    fn from(v: Matrix<2, 2, f32>) -> Self {
        Variant::from(Matrix2::from(v))
    }
}
impl From<Matrix<3, 3, f32>> for Variant {
    #[inline]
    fn from(v: Matrix<3, 3, f32>) -> Self {
        Variant::from(Matrix3::from(v))
    }
}
impl From<Matrix<4, 4, f32>> for Variant {
    #[inline]
    fn from(v: Matrix<4, 4, f32>) -> Self {
        Variant::from(Matrix4::from(v))
    }
}
impl From<Transform2D> for Variant {
    #[inline]
    fn from(v: Transform2D) -> Self {
        Variant::EngineStruct(EngineStruct::Transform2D(v))
    }
}
impl From<Transform3D> for Variant {
    #[inline]
    fn from(v: Transform3D) -> Self {
        Variant::EngineStruct(EngineStruct::Transform3D(v))
    }
}
impl From<Quaternion> for Variant {
    #[inline]
    fn from(v: Quaternion) -> Self {
        Variant::EngineStruct(EngineStruct::Quaternion(v))
    }
}
impl From<PostProcessSet> for Variant {
    #[inline]
    fn from(v: PostProcessSet) -> Self {
        Variant::EngineStruct(EngineStruct::PostProcessSet(v))
    }
}
impl From<VisualAccessibilitySettings> for Variant {
    #[inline]
    fn from(v: VisualAccessibilitySettings) -> Self {
        Variant::EngineStruct(EngineStruct::VisualAccessibilitySettings(v))
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

// -------------------- JSON conversion --------------------

impl Variant {
    pub fn from_json_value(value: JsonValue) -> Self {
        match value {
            JsonValue::Null => Variant::Null,
            JsonValue::Bool(v) => Variant::Bool(v),
            JsonValue::Number(v) => {
                if let Some(i) = v.as_i64() {
                    Variant::from(i)
                } else if let Some(u) = v.as_u64() {
                    Variant::from(u)
                } else if let Some(f) = v.as_f64() {
                    Variant::from(f)
                } else {
                    Variant::Null
                }
            }
            JsonValue::String(v) => Variant::from(v),
            JsonValue::Array(values) => {
                Variant::Array(values.into_iter().map(Variant::from_json_value).collect())
            }
            JsonValue::Object(object) => Variant::Object(
                object
                    .into_iter()
                    .map(|(k, v)| (Arc::<str>::from(k), Variant::from_json_value(v)))
                    .collect::<BTreeMap<Arc<str>, Variant>>(),
            ),
        }
    }

    pub fn to_json_value(&self) -> JsonValue {
        match self {
            Variant::Null => JsonValue::Null,
            Variant::Bool(v) => JsonValue::Bool(*v),
            Variant::Number(v) => number_to_json_value(*v),
            Variant::String(v) => JsonValue::String(v.as_ref().to_string()),
            Variant::Bytes(v) => JsonValue::Array(
                v.iter()
                    .map(|b| JsonValue::Number(JsonNumber::from(*b)))
                    .collect(),
            ),
            Variant::ID(v) => JsonValue::Number(JsonNumber::from(v.as_u64())),
            Variant::EngineStruct(EngineStruct::Vector2(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), float_to_json(v.x as f64));
                map.insert("y".to_string(), float_to_json(v.y as f64));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::Vector3(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), float_to_json(v.x as f64));
                map.insert("y".to_string(), float_to_json(v.y as f64));
                map.insert("z".to_string(), float_to_json(v.z as f64));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::Vector4(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), float_to_json(v.x as f64));
                map.insert("y".to_string(), float_to_json(v.y as f64));
                map.insert("z".to_string(), float_to_json(v.z as f64));
                map.insert("w".to_string(), float_to_json(v.w as f64));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::IVector2(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), JsonValue::Number(JsonNumber::from(v.x)));
                map.insert("y".to_string(), JsonValue::Number(JsonNumber::from(v.y)));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::IVector3(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), JsonValue::Number(JsonNumber::from(v.x)));
                map.insert("y".to_string(), JsonValue::Number(JsonNumber::from(v.y)));
                map.insert("z".to_string(), JsonValue::Number(JsonNumber::from(v.z)));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::IVector4(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), JsonValue::Number(JsonNumber::from(v.x)));
                map.insert("y".to_string(), JsonValue::Number(JsonNumber::from(v.y)));
                map.insert("z".to_string(), JsonValue::Number(JsonNumber::from(v.z)));
                map.insert("w".to_string(), JsonValue::Number(JsonNumber::from(v.w)));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::UVector2(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), JsonValue::Number(JsonNumber::from(v.x)));
                map.insert("y".to_string(), JsonValue::Number(JsonNumber::from(v.y)));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::UVector3(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), JsonValue::Number(JsonNumber::from(v.x)));
                map.insert("y".to_string(), JsonValue::Number(JsonNumber::from(v.y)));
                map.insert("z".to_string(), JsonValue::Number(JsonNumber::from(v.z)));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::UVector4(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), JsonValue::Number(JsonNumber::from(v.x)));
                map.insert("y".to_string(), JsonValue::Number(JsonNumber::from(v.y)));
                map.insert("z".to_string(), JsonValue::Number(JsonNumber::from(v.z)));
                map.insert("w".to_string(), JsonValue::Number(JsonNumber::from(v.w)));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::UnitVector2(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), float_to_json(v.x.to_f32() as f64));
                map.insert("y".to_string(), float_to_json(v.y.to_f32() as f64));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::UnitVector3(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), float_to_json(v.x.to_f32() as f64));
                map.insert("y".to_string(), float_to_json(v.y.to_f32() as f64));
                map.insert("z".to_string(), float_to_json(v.z.to_f32() as f64));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::UnitVector4(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), float_to_json(v.x.to_f32() as f64));
                map.insert("y".to_string(), float_to_json(v.y.to_f32() as f64));
                map.insert("z".to_string(), float_to_json(v.z.to_f32() as f64));
                map.insert("w".to_string(), float_to_json(v.w.to_f32() as f64));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::Matrix2(v)) => matrix_rows_to_json(v.to_rows()),
            Variant::EngineStruct(EngineStruct::Matrix3(v)) => matrix_rows_to_json(v.to_rows()),
            Variant::EngineStruct(EngineStruct::Matrix4(v)) => matrix_rows_to_json(v.to_rows()),
            Variant::EngineStruct(EngineStruct::Transform2D(v)) => {
                let mut position = JsonMap::new();
                position.insert("x".to_string(), float_to_json(v.position.x as f64));
                position.insert("y".to_string(), float_to_json(v.position.y as f64));

                let mut scale = JsonMap::new();
                scale.insert("x".to_string(), float_to_json(v.scale.x as f64));
                scale.insert("y".to_string(), float_to_json(v.scale.y as f64));

                let mut map = JsonMap::new();
                map.insert("position".to_string(), JsonValue::Object(position));
                map.insert("scale".to_string(), JsonValue::Object(scale));
                map.insert("rotation".to_string(), float_to_json(v.rotation as f64));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::Transform3D(v)) => {
                let mut position = JsonMap::new();
                position.insert("x".to_string(), float_to_json(v.position.x as f64));
                position.insert("y".to_string(), float_to_json(v.position.y as f64));
                position.insert("z".to_string(), float_to_json(v.position.z as f64));

                let mut scale = JsonMap::new();
                scale.insert("x".to_string(), float_to_json(v.scale.x as f64));
                scale.insert("y".to_string(), float_to_json(v.scale.y as f64));
                scale.insert("z".to_string(), float_to_json(v.scale.z as f64));

                let mut rotation = JsonMap::new();
                rotation.insert("x".to_string(), float_to_json(v.rotation.x as f64));
                rotation.insert("y".to_string(), float_to_json(v.rotation.y as f64));
                rotation.insert("z".to_string(), float_to_json(v.rotation.z as f64));
                rotation.insert("w".to_string(), float_to_json(v.rotation.w as f64));

                let mut map = JsonMap::new();
                map.insert("position".to_string(), JsonValue::Object(position));
                map.insert("scale".to_string(), JsonValue::Object(scale));
                map.insert("rotation".to_string(), JsonValue::Object(rotation));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::Quaternion(v)) => {
                let mut map = JsonMap::new();
                map.insert("x".to_string(), float_to_json(v.x as f64));
                map.insert("y".to_string(), float_to_json(v.y as f64));
                map.insert("z".to_string(), float_to_json(v.z as f64));
                map.insert("w".to_string(), float_to_json(v.w as f64));
                JsonValue::Object(map)
            }
            Variant::EngineStruct(EngineStruct::PostProcessSet(v)) => {
                JsonValue::Array(v.entries().iter().map(post_process_entry_to_json).collect())
            }
            Variant::EngineStruct(EngineStruct::VisualAccessibilitySettings(v)) => {
                let mut map = JsonMap::new();
                let color_blind = match v.color_blind {
                    Some(setting) => {
                        let mut cb = JsonMap::new();
                        cb.insert(
                            "filter".to_string(),
                            JsonValue::String(
                                color_blind_filter_to_str(setting.filter).to_string(),
                            ),
                        );
                        cb.insert(
                            "strength".to_string(),
                            float_to_json(setting.strength as f64),
                        );
                        JsonValue::Object(cb)
                    }
                    None => JsonValue::Null,
                };
                map.insert("color_blind".to_string(), color_blind);
                JsonValue::Object(map)
            }
            Variant::Array(v) => JsonValue::Array(v.iter().map(Variant::to_json_value).collect()),
            Variant::Object(v) => JsonValue::Object(
                v.iter()
                    .map(|(k, v)| (k.as_ref().to_string(), v.to_json_value()))
                    .collect::<JsonMap<String, JsonValue>>(),
            ),
        }
    }
}

fn number_to_json_value(number: Number) -> JsonValue {
    match number {
        Number::I8(v) => JsonValue::Number(JsonNumber::from(v)),
        Number::I16(v) => JsonValue::Number(JsonNumber::from(v)),
        Number::I32(v) => JsonValue::Number(JsonNumber::from(v)),
        Number::I64(v) => JsonValue::Number(JsonNumber::from(v)),
        Number::I128(v) => match i64::try_from(v) {
            Ok(v) => JsonValue::Number(JsonNumber::from(v)),
            Err(_) => JsonValue::String(v.to_string()),
        },
        Number::U8(v) => JsonValue::Number(JsonNumber::from(v)),
        Number::U16(v) => JsonValue::Number(JsonNumber::from(v)),
        Number::U32(v) => JsonValue::Number(JsonNumber::from(v)),
        Number::U64(v) => JsonValue::Number(JsonNumber::from(v)),
        Number::U128(v) => match u64::try_from(v) {
            Ok(v) => JsonValue::Number(JsonNumber::from(v)),
            Err(_) => JsonValue::String(v.to_string()),
        },
        Number::F32(v) => float_to_json(v as f64),
        Number::F64(v) => float_to_json(v),
    }
}

fn float_to_json(value: f64) -> JsonValue {
    match JsonNumber::from_f64(value) {
        Some(v) => JsonValue::Number(v),
        None => JsonValue::Null,
    }
}

fn color_blind_filter_to_str(filter: ColorBlindFilter) -> &'static str {
    match filter {
        ColorBlindFilter::Protan => "protan",
        ColorBlindFilter::Deuteran => "deuteran",
        ColorBlindFilter::Tritan => "tritan",
        ColorBlindFilter::Achroma => "achroma",
    }
}

fn custom_post_param_value_to_json(value: &CustomPostParamValue) -> JsonValue {
    match value {
        CustomPostParamValue::F32(v) => float_to_json(*v as f64),
        CustomPostParamValue::I32(v) => JsonValue::Number(JsonNumber::from(*v)),
        CustomPostParamValue::Bool(v) => JsonValue::Bool(*v),
        CustomPostParamValue::Vec2(v) => {
            JsonValue::Array(vec![float_to_json(v[0] as f64), float_to_json(v[1] as f64)])
        }
        CustomPostParamValue::Vec3(v) => JsonValue::Array(vec![
            float_to_json(v[0] as f64),
            float_to_json(v[1] as f64),
            float_to_json(v[2] as f64),
        ]),
        CustomPostParamValue::Vec4(v) => JsonValue::Array(vec![
            float_to_json(v[0] as f64),
            float_to_json(v[1] as f64),
            float_to_json(v[2] as f64),
            float_to_json(v[3] as f64),
        ]),
    }
}

fn post_process_entry_to_json(entry: &PostProcessEntry) -> JsonValue {
    let mut value = post_process_effect_to_json(&entry.effect);
    if let JsonValue::Object(map) = &mut value {
        match &entry.name {
            Some(name) => {
                map.insert("name".to_string(), JsonValue::String(name.to_string()));
            }
            None => {
                map.insert("name".to_string(), JsonValue::Null);
            }
        }
    }
    value
}

fn post_process_effect_to_json(effect: &PostProcessEffect) -> JsonValue {
    let mut map = JsonMap::new();
    match effect {
        PostProcessEffect::Blur { strength } => {
            map.insert("type".to_string(), JsonValue::String("blur".to_string()));
            map.insert("strength".to_string(), float_to_json(*strength as f64));
        }
        PostProcessEffect::Pixelate { size } => {
            map.insert(
                "type".to_string(),
                JsonValue::String("pixelate".to_string()),
            );
            map.insert("size".to_string(), float_to_json(*size as f64));
        }
        PostProcessEffect::Warp { waves, strength } => {
            map.insert("type".to_string(), JsonValue::String("warp".to_string()));
            map.insert("waves".to_string(), float_to_json(*waves as f64));
            map.insert("strength".to_string(), float_to_json(*strength as f64));
        }
        PostProcessEffect::Vignette {
            strength,
            radius,
            softness,
        } => {
            map.insert(
                "type".to_string(),
                JsonValue::String("vignette".to_string()),
            );
            map.insert("strength".to_string(), float_to_json(*strength as f64));
            map.insert("radius".to_string(), float_to_json(*radius as f64));
            map.insert("softness".to_string(), float_to_json(*softness as f64));
        }
        PostProcessEffect::Crt {
            scanline_strength,
            curvature,
            chromatic,
            vignette,
        } => {
            map.insert("type".to_string(), JsonValue::String("crt".to_string()));
            map.insert(
                "scanline_strength".to_string(),
                float_to_json(*scanline_strength as f64),
            );
            map.insert("curvature".to_string(), float_to_json(*curvature as f64));
            map.insert("chromatic".to_string(), float_to_json(*chromatic as f64));
            map.insert("vignette".to_string(), float_to_json(*vignette as f64));
        }
        PostProcessEffect::ColorFilter { color, strength } => {
            map.insert(
                "type".to_string(),
                JsonValue::String("color_filter".to_string()),
            );
            map.insert(
                "color".to_string(),
                JsonValue::Array(color.iter().map(|v| float_to_json(*v as f64)).collect()),
            );
            map.insert("strength".to_string(), float_to_json(*strength as f64));
        }
        PostProcessEffect::ReverseFilter {
            color,
            strength,
            softness,
        } => {
            map.insert(
                "type".to_string(),
                JsonValue::String("reverse_filter".to_string()),
            );
            map.insert(
                "color".to_string(),
                JsonValue::Array(color.iter().map(|v| float_to_json(*v as f64)).collect()),
            );
            map.insert("strength".to_string(), float_to_json(*strength as f64));
            map.insert("softness".to_string(), float_to_json(*softness as f64));
        }
        PostProcessEffect::Bloom {
            strength,
            threshold,
            radius,
        } => {
            map.insert("type".to_string(), JsonValue::String("bloom".to_string()));
            map.insert("strength".to_string(), float_to_json(*strength as f64));
            map.insert("threshold".to_string(), float_to_json(*threshold as f64));
            map.insert("radius".to_string(), float_to_json(*radius as f64));
        }
        PostProcessEffect::Exposure {
            exposure,
            auto_exposure,
            min_exposure,
            max_exposure,
            speed_up,
            speed_down,
            target_luminance,
        } => {
            map.insert(
                "type".to_string(),
                JsonValue::String("exposure".to_string()),
            );
            map.insert("exposure".to_string(), float_to_json(*exposure as f64));
            map.insert("auto_exposure".to_string(), JsonValue::Bool(*auto_exposure));
            map.insert(
                "min_exposure".to_string(),
                float_to_json(*min_exposure as f64),
            );
            map.insert(
                "max_exposure".to_string(),
                float_to_json(*max_exposure as f64),
            );
            map.insert("speed_up".to_string(), float_to_json(*speed_up as f64));
            map.insert("speed_down".to_string(), float_to_json(*speed_down as f64));
            map.insert(
                "target_luminance".to_string(),
                float_to_json(*target_luminance as f64),
            );
        }
        PostProcessEffect::Saturate { amount } => {
            map.insert(
                "type".to_string(),
                JsonValue::String("saturate".to_string()),
            );
            map.insert("amount".to_string(), float_to_json(*amount as f64));
        }
        PostProcessEffect::BlackWhite { amount } => {
            map.insert(
                "type".to_string(),
                JsonValue::String("black_white".to_string()),
            );
            map.insert("amount".to_string(), float_to_json(*amount as f64));
        }
        PostProcessEffect::ColorGrade {
            exposure,
            contrast,
            brightness,
            saturation,
            gamma,
            temperature,
            tint,
            hue_shift,
            vibrance,
            lift,
            gain,
            offset,
        } => {
            map.insert(
                "type".to_string(),
                JsonValue::String("color_grade".to_string()),
            );
            map.insert("exposure".to_string(), float_to_json(*exposure as f64));
            map.insert("contrast".to_string(), float_to_json(*contrast as f64));
            map.insert("brightness".to_string(), float_to_json(*brightness as f64));
            map.insert("saturation".to_string(), float_to_json(*saturation as f64));
            map.insert("gamma".to_string(), float_to_json(*gamma as f64));
            map.insert(
                "temperature".to_string(),
                float_to_json(*temperature as f64),
            );
            map.insert("tint".to_string(), float_to_json(*tint as f64));
            map.insert("hue_shift".to_string(), float_to_json(*hue_shift as f64));
            map.insert("vibrance".to_string(), float_to_json(*vibrance as f64));
            map.insert(
                "lift".to_string(),
                JsonValue::Array(lift.iter().map(|v| float_to_json(*v as f64)).collect()),
            );
            map.insert(
                "gain".to_string(),
                JsonValue::Array(gain.iter().map(|v| float_to_json(*v as f64)).collect()),
            );
            map.insert(
                "offset".to_string(),
                JsonValue::Array(offset.iter().map(|v| float_to_json(*v as f64)).collect()),
            );
        }
        PostProcessEffect::Lut2D {
            texture_path,
            size,
            strength,
        } => {
            map.insert("type".to_string(), JsonValue::String("lut2d".to_string()));
            map.insert(
                "texture_path".to_string(),
                JsonValue::String(texture_path.to_string()),
            );
            map.insert(
                "size".to_string(),
                JsonValue::Number(JsonNumber::from(*size)),
            );
            map.insert("strength".to_string(), float_to_json(*strength as f64));
        }
        PostProcessEffect::Lut3D {
            texture_path,
            size,
            strength,
        } => {
            map.insert("type".to_string(), JsonValue::String("lut3d".to_string()));
            map.insert(
                "texture_path".to_string(),
                JsonValue::String(texture_path.to_string()),
            );
            map.insert(
                "size".to_string(),
                JsonValue::Number(JsonNumber::from(*size)),
            );
            map.insert("strength".to_string(), float_to_json(*strength as f64));
        }
        PostProcessEffect::Custom {
            shader_path,
            params,
        } => {
            map.insert("type".to_string(), JsonValue::String("custom".to_string()));
            map.insert(
                "shader_path".to_string(),
                JsonValue::String(shader_path.to_string()),
            );
            let json_params = params
                .iter()
                .map(|p| {
                    let mut pmap = JsonMap::new();
                    match &p.name {
                        Some(name) => {
                            pmap.insert("name".to_string(), JsonValue::String(name.to_string()));
                        }
                        None => {
                            pmap.insert("name".to_string(), JsonValue::Null);
                        }
                    }
                    pmap.insert(
                        "value".to_string(),
                        custom_post_param_value_to_json(&p.value),
                    );
                    JsonValue::Object(pmap)
                })
                .collect();
            map.insert("params".to_string(), JsonValue::Array(json_params));
        }
    }
    JsonValue::Object(map)
}

fn parse_matrix_rows<const N: usize>(value: &Variant) -> Option<[[f32; N]; N]> {
    if let Variant::Object(obj) = value
        && let Some(rows) = obj.get("rows")
    {
        return parse_matrix_rows::<N>(rows);
    }

    let values = value.as_array()?;
    let mut rows = [[0.0; N]; N];
    if values.len() == N {
        for row in 0..N {
            let cols = values[row].as_array()?;
            if cols.len() != N {
                return None;
            }
            for col in 0..N {
                rows[row][col] = cols[col].as_f32()?;
            }
        }
        return Some(rows);
    }

    if values.len() == N * N {
        for row in 0..N {
            for col in 0..N {
                rows[row][col] = values[row * N + col].as_f32()?;
            }
        }
        return Some(rows);
    }

    None
}

fn parse_matrix_rows_generic<const ROWS: usize, const COLS: usize, T>(
    value: &Variant,
) -> Option<Matrix<ROWS, COLS, T>>
where
    T: VariantMatrixCell,
{
    if let Variant::Object(obj) = value
        && let Some(rows) = obj.get("rows")
    {
        return parse_matrix_rows_generic(rows);
    }

    // `to_variant()` on a square 2x2/3x3/4x4 matrix takes the fast path and
    // produces a `Variant::EngineStruct(Matrix2/3/4(..))`, not a plain
    // array. Recognize that shape directly (no `serde_json` round trip
    // needed) by rebuilding it as row arrays and re-dispatching.
    if let Variant::EngineStruct(engine_struct) = value {
        let rows: Option<[[f32; 4]; 4]> = match (engine_struct, ROWS, COLS) {
            (EngineStruct::Matrix2(m), 2, 2) => {
                let r = m.to_rows();
                Some([
                    [r[0][0], r[0][1], 0.0, 0.0],
                    [r[1][0], r[1][1], 0.0, 0.0],
                    [0.0; 4],
                    [0.0; 4],
                ])
            }
            (EngineStruct::Matrix3(m), 3, 3) => {
                let r = m.to_rows();
                Some([
                    [r[0][0], r[0][1], r[0][2], 0.0],
                    [r[1][0], r[1][1], r[1][2], 0.0],
                    [r[2][0], r[2][1], r[2][2], 0.0],
                    [0.0; 4],
                ])
            }
            (EngineStruct::Matrix4(m), 4, 4) => Some(m.to_rows()),
            _ => None,
        };
        if let Some(rows) = rows {
            let array = Variant::Array(
                rows.iter()
                    .take(ROWS)
                    .map(|row| {
                        Variant::Array(row.iter().take(COLS).copied().map(Variant::from).collect())
                    })
                    .collect(),
            );
            return parse_matrix_rows_generic(&array);
        }
        return None;
    }

    let values = value.as_array()?;
    let mut rows = Vec::with_capacity(ROWS);
    if values.len() == ROWS {
        for row in values {
            let cols = row.as_array()?;
            if cols.len() != COLS {
                return None;
            }
            let row = cols
                .iter()
                .map(T::from_matrix_cell_variant)
                .collect::<Option<Vec<_>>>()?
                .try_into()
                .ok()?;
            rows.push(row);
        }
    } else if values.len() == ROWS * COLS {
        for row in 0..ROWS {
            let start = row * COLS;
            let row = values[start..start + COLS]
                .iter()
                .map(T::from_matrix_cell_variant)
                .collect::<Option<Vec<_>>>()?
                .try_into()
                .ok()?;
            rows.push(row);
        }
    } else {
        return None;
    }

    Some(Matrix::new(rows.try_into().ok()?))
}

fn matrix_to_fast_variant<const ROWS: usize, const COLS: usize, T>(
    matrix: &Matrix<ROWS, COLS, T>,
) -> Option<Variant>
where
    T: VariantMatrixCell,
{
    if ROWS != COLS {
        return None;
    }
    let values = matrix_to_f32_values(matrix)?;
    match ROWS {
        2 => Some(Variant::from(Matrix2::from_rows([
            [values[0], values[1]],
            [values[2], values[3]],
        ]))),
        3 => Some(Variant::from(Matrix3::from_rows([
            [values[0], values[1], values[2]],
            [values[3], values[4], values[5]],
            [values[6], values[7], values[8]],
        ]))),
        4 => Some(Variant::from(Matrix4::from_rows([
            [values[0], values[1], values[2], values[3]],
            [values[4], values[5], values[6], values[7]],
            [values[8], values[9], values[10], values[11]],
            [values[12], values[13], values[14], values[15]],
        ]))),
        _ => None,
    }
}

fn matrix_to_f32_values<const ROWS: usize, const COLS: usize, T>(
    matrix: &Matrix<ROWS, COLS, T>,
) -> Option<Vec<f32>>
where
    T: VariantMatrixCell,
{
    let mut out = Vec::with_capacity(ROWS * COLS);
    for row in matrix.rows() {
        for cell in row {
            out.push(cell.as_matrix_cell_f32()?);
        }
    }
    Some(out)
}

fn matrix_to_variant_array<const ROWS: usize, const COLS: usize, T>(
    matrix: &Matrix<ROWS, COLS, T>,
) -> Variant
where
    T: VariantMatrixCell,
{
    Variant::Array(
        matrix
            .rows()
            .iter()
            .map(|row| {
                Variant::Array(
                    row.iter()
                        .map(VariantMatrixCell::to_matrix_cell_variant)
                        .collect::<Vec<_>>(),
                )
            })
            .collect(),
    )
}

fn matrix_rows_to_json<const N: usize>(rows: [[f32; N]; N]) -> JsonValue {
    JsonValue::Array(
        rows.into_iter()
            .map(|row| JsonValue::Array(row.into_iter().map(|v| float_to_json(v as f64)).collect()))
            .collect(),
    )
}

fn variant_to_u32(value: &Variant) -> Option<u32> {
    match value.as_number()? {
        Number::I8(v) => u32::try_from(v).ok(),
        Number::I16(v) => u32::try_from(v).ok(),
        Number::I32(v) => u32::try_from(v).ok(),
        Number::I64(v) => u32::try_from(v).ok(),
        Number::I128(v) => u32::try_from(v).ok(),
        Number::U8(v) => Some(v as u32),
        Number::U16(v) => Some(v as u32),
        Number::U32(v) => Some(v),
        Number::U64(v) => u32::try_from(v).ok(),
        Number::U128(v) => u32::try_from(v).ok(),
        Number::F32(_) | Number::F64(_) => None,
    }
}
