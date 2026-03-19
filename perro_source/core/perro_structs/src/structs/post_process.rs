use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq)]
pub enum PostProcessEffect {
    Blur { strength: f32 },
    Pixelate { size: f32 },
    Warp { waves: f32, strength: f32 },
    Vignette {
        strength: f32,
        radius: f32,
        softness: f32,
    },
    Crt {
        scanline_strength: f32,
        curvature: f32,
        chromatic: f32,
        vignette: f32,
    },
    ColorFilter {
        color: [f32; 3],
        strength: f32,
    },
    ReverseFilter {
        color: [f32; 3],
        strength: f32,
        softness: f32,
    },
    Bloom {
        strength: f32,
        threshold: f32,
        radius: f32,
    },
    Saturate {
        amount: f32,
    },
    BlackWhite {
        amount: f32,
    },
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
