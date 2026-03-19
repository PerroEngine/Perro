use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq)]
pub enum PostProcessEffect {
    Blur { strength: f32 },
    Pixelate { size: f32 },
    Warp { waves: f32, strength: f32 },
    Custom {
        shader_path: Cow<'static, str>,
        params: Cow<'static, [CustomPostParam]>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum CustomPostParamValue {
    F32(f32),
    I32(i32),
    Bool(bool),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CustomPostParam {
    pub name: Option<Cow<'static, str>>,
    pub value: CustomPostParamValue,
}

impl CustomPostParam {
    #[inline]
    pub fn named(name: impl Into<Cow<'static, str>>, value: CustomPostParamValue) -> Self {
        Self {
            name: Some(name.into()),
            value,
        }
    }

    #[inline]
    pub fn unnamed(value: CustomPostParamValue) -> Self {
        Self { name: None, value }
    }
}
