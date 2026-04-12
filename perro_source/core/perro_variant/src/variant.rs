// perro_variant/src/lib.rs

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

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
pub enum IDs {
    Node(NodeID),
    Texture(TextureID),
    Material(MaterialID),
    Mesh(MeshID),
    Animation(AnimationID),
    Light(LightID),
    UIElement(UIElementID),
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
            IDs::UIElement(v) => v.as_u64(),
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
    Transform2D(Transform2D),
    Transform3D(Transform3D),
    Quaternion(Quaternion),
    PostProcessSet(PostProcessSet),
    VisualAccessibilitySettings(VisualAccessibilitySettings),
}

/// Typed conversion contract used by script state and method parameter conversion.
///
/// Implement this trait for custom structs/enums (typically via `#[derive(Variant)]`).
pub trait VariantCodec: Sized {
    fn from_variant(value: &Variant) -> Option<Self>;
    fn to_variant(&self) -> Variant;
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
}

impl VariantCodec for Variant {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        Some(value.clone())
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.clone()
    }
}

impl VariantCodec for bool {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_bool()
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

macro_rules! impl_statefield_signed {
    ($ty:ty, $pat:pat => $expr:expr) => {
        impl VariantCodec for $ty {
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
        impl VariantCodec for $ty {
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

impl VariantCodec for isize {
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

impl VariantCodec for usize {
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

impl VariantCodec for f32 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_f32()
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl VariantCodec for f64 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_f64()
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl VariantCodec for String {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_str().map(ToString::to_string)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(self.clone())
    }
}

impl VariantCodec for Arc<str> {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_str().map(Arc::<str>::from)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(Arc::clone(self))
    }
}

impl VariantCodec for NodeID {
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

impl VariantCodec for TextureID {
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
        impl VariantCodec for $id_ty {
            #[inline]
            fn from_variant(value: &Variant) -> Option<Self> {
                value
                    .as_number()
                    .and_then(|n| n.as_i64_lossy())
                    .and_then(|n| u64::try_from(n).ok())
                    .map(<$id_ty>::from_u64)
                    .or_else(|| {
                        value
                            .as_str()
                            .and_then(parse_u64_id_string)
                            .map(<$id_ty>::from_u64)
                    })
                    .or_else(|| value.as_id().map(IDs::as_u64).map(<$id_ty>::from_u64))
            }

            #[inline]
            fn to_variant(&self) -> Variant {
                Variant::from(self.as_u64())
            }
        }
    };
}

impl_statefield_plain_id!(MaterialID);
impl_statefield_plain_id!(MeshID);
impl_statefield_plain_id!(AnimationID);
impl_statefield_plain_id!(LightID);
impl_statefield_plain_id!(UIElementID);
impl_statefield_plain_id!(SignalID);
impl_statefield_plain_id!(AudioBusID);
impl_statefield_plain_id!(TagID);
impl_statefield_plain_id!(PreloadedSceneID);

impl VariantCodec for Vector2 {
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

impl VariantCodec for Vector3 {
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

impl VariantCodec for Quaternion {
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

impl VariantCodec for Transform2D {
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

impl VariantCodec for Transform3D {
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

impl VariantCodec for PostProcessSet {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_post_process_set().cloned()
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(self.clone())
    }
}

impl VariantCodec for VisualAccessibilitySettings {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_visual_accessibility_settings()
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl<T> VariantCodec for Option<T>
where
    T: VariantCodec,
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
    fn to_variant(&self) -> Variant {
        match self {
            Some(v) => v.to_variant(),
            None => Variant::Null,
        }
    }
}

impl<T> VariantCodec for Vec<T>
where
    T: VariantCodec,
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
    fn to_variant(&self) -> Variant {
        Variant::Array(self.iter().map(VariantCodec::to_variant).collect())
    }
}

impl<T> VariantCodec for BTreeMap<Arc<str>, T>
where
    T: VariantCodec,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let object = value.as_object()?;
        let mut out = BTreeMap::new();
        for (k, v) in object {
            out.insert(Arc::clone(k), T::from_variant(v)?);
        }
        Some(out)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        let mut out = BTreeMap::new();
        for (k, v) in self {
            out.insert(Arc::clone(k), v.to_variant());
        }
        Variant::Object(out)
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

    #[inline]
    pub fn as_texture(&self) -> Option<TextureID> {
        match *self {
            Variant::ID(IDs::Texture(id)) => Some(id),
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
    pub fn as_transform2d(&self) -> Option<Transform2D> {
        match self {
            Variant::EngineStruct(EngineStruct::Transform2D(t)) => Some(*t),
            _ => None,
        }
    }

    #[inline]
    pub fn as_transform3d(&self) -> Option<Transform3D> {
        match self {
            Variant::EngineStruct(EngineStruct::Transform3D(t)) => Some(*t),
            _ => None,
        }
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
impl From<UIElementID> for Variant {
    #[inline]
    fn from(v: UIElementID) -> Self {
        Variant::ID(IDs::UIElement(v))
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
            Variant::EngineStruct(EngineStruct::PostProcessSet(v)) => JsonValue::Array(
                v.as_slice()
                    .iter()
                    .map(post_process_effect_to_json)
                    .collect(),
            ),
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

fn parse_u64_id_string(s: &str) -> Option<u64> {
    let compact = s.strip_prefix("0x").unwrap_or(s).replace('-', "");
    if compact.is_empty() {
        return None;
    }
    if compact.chars().all(|c| c.is_ascii_hexdigit()) {
        u64::from_str_radix(&compact[..16.min(compact.len())], 16).ok()
    } else {
        compact.parse::<u64>().ok()
    }
}
