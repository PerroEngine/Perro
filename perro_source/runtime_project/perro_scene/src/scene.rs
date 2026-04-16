use perro_structs::ConstParamValue;
use std::borrow::Cow;

pub type SceneObjectField = (Cow<'static, str>, SceneValue);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SceneValueKey(pub Cow<'static, str>);

impl AsRef<str> for SceneValueKey {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl std::fmt::Display for SceneValueKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_ref())
    }
}

impl From<&'static str> for SceneValueKey {
    fn from(value: &'static str) -> Self {
        Self(Cow::Borrowed(value))
    }
}

impl From<String> for SceneValueKey {
    fn from(value: String) -> Self {
        Self(Cow::Owned(value))
    }
}

#[derive(Clone, Debug)]
pub enum SceneValue {
    Bool(bool),
    I32(i32),
    F32(f32),
    Vec2 { x: f32, y: f32 },
    Vec3 { x: f32, y: f32, z: f32 },
    Vec4 { x: f32, y: f32, z: f32, w: f32 },
    Str(Cow<'static, str>),
    Hashed(u64),
    Key(SceneValueKey),
    Object(Cow<'static, [SceneObjectField]>),
    Array(Cow<'static, [SceneValue]>),
}

impl SceneValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Self::I32(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Self::F32(v) => Some(*v),
            Self::I32(v) => Some(*v as f32),
            _ => None,
        }
    }

    pub fn as_vec2(&self) -> Option<(f32, f32)> {
        match self {
            Self::Vec2 { x, y } => Some((*x, *y)),
            _ => None,
        }
    }

    pub fn as_vec3(&self) -> Option<(f32, f32, f32)> {
        match self {
            Self::Vec3 { x, y, z } => Some((*x, *y, *z)),
            _ => None,
        }
    }

    pub fn as_vec4(&self) -> Option<(f32, f32, f32, f32)> {
        match self {
            Self::Vec4 { x, y, z, w } => Some((*x, *y, *z, *w)),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(v) => Some(v.as_ref()),
            _ => None,
        }
    }

    pub fn as_hashed(&self) -> Option<u64> {
        match self {
            Self::Hashed(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_key(&self) -> Option<&str> {
        match self {
            Self::Key(v) => Some(v.as_ref()),
            _ => None,
        }
    }

    #[inline]
    pub fn as_const_param(&self) -> Option<ConstParamValue> {
        ConstParamValue::try_from(self).ok()
    }
}

impl TryFrom<&SceneValue> for ConstParamValue {
    type Error = ();

    fn try_from(value: &SceneValue) -> Result<Self, Self::Error> {
        match value {
            SceneValue::Bool(v) => Ok(Self::Bool(*v)),
            SceneValue::I32(v) => Ok(Self::I32(*v)),
            SceneValue::F32(v) => Ok(Self::F32(*v)),
            SceneValue::Vec2 { x, y } => Ok(Self::Vec2([*x, *y])),
            SceneValue::Vec3 { x, y, z } => Ok(Self::Vec3([*x, *y, *z])),
            SceneValue::Vec4 { x, y, z, w } => Ok(Self::Vec4([*x, *y, *z, *w])),
            _ => Err(()),
        }
    }
}

impl From<ConstParamValue> for SceneValue {
    fn from(value: ConstParamValue) -> Self {
        match value {
            ConstParamValue::Bool(v) => Self::Bool(v),
            ConstParamValue::I32(v) => Self::I32(v),
            ConstParamValue::F32(v) => Self::F32(v),
            ConstParamValue::Vec2(v) => Self::Vec2 { x: v[0], y: v[1] },
            ConstParamValue::Vec3(v) => Self::Vec3 {
                x: v[0],
                y: v[1],
                z: v[2],
            },
            ConstParamValue::Vec4(v) => Self::Vec4 {
                x: v[0],
                y: v[1],
                z: v[2],
                w: v[3],
            },
        }
    }
}

#[derive(Clone, Copy)]
pub struct SceneFieldIterRef<'a> {
    fields: &'a [SceneObjectField],
}

impl<'a> SceneFieldIterRef<'a> {
    pub fn new(fields: &'a [SceneObjectField]) -> Self {
        Self { fields }
    }

    pub fn for_each(self, mut f: impl FnMut(&str, &'a SceneValue)) {
        for (name, value) in self.fields {
            f(name.as_ref(), value);
        }
    }
}

#[derive(Debug, Clone)]
pub struct Scene {
    pub nodes: Cow<'static, [SceneNodeEntry]>,
    pub root: Option<SceneKey>,
}

#[derive(Debug, Clone)]
pub struct SceneNodeEntry {
    pub data: SceneNodeData,
    pub has_data_override: bool,
    pub key: SceneKey,
    pub name: Option<Cow<'static, str>>,
    pub tags: Cow<'static, [Cow<'static, str>]>,
    pub children: Cow<'static, [SceneKey]>,
    pub parent: Option<SceneKey>,
    pub script: Option<Cow<'static, str>>,
    pub script_hash: Option<u64>,
    pub clear_script: bool,
    pub root_of: Option<Cow<'static, str>>,
    pub root_of_hash: Option<u64>,
    pub script_vars: Cow<'static, [SceneObjectField]>,
}

#[derive(Debug, Clone)]
pub struct SceneNodeData {
    pub ty: Cow<'static, str>,
    pub fields: Cow<'static, [SceneObjectField]>,
    pub base: Option<SceneNodeDataBase>,
}

#[derive(Debug, Clone)]
pub enum SceneNodeDataBase {
    Borrowed(&'static SceneNodeData),
    Owned(Box<SceneNodeData>),
}

impl SceneNodeData {
    pub fn base_ref(&self) -> Option<&SceneNodeData> {
        match &self.base {
            Some(SceneNodeDataBase::Borrowed(v)) => Some(*v),
            Some(SceneNodeDataBase::Owned(v)) => Some(v.as_ref()),
            None => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SceneKey(pub Cow<'static, str>);

impl AsRef<str> for SceneKey {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl From<&'static str> for SceneKey {
    fn from(value: &'static str) -> Self {
        Self(Cow::Borrowed(value))
    }
}

impl From<String> for SceneKey {
    fn from(value: String) -> Self {
        Self(Cow::Owned(value))
    }
}
