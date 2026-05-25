use perro_structs::{BitMask, CustomPostParam, Transform3D};
use std::borrow::Cow;

#[derive(Clone, Debug)]
pub struct SkyShaderPass {
    pub path: Cow<'static, str>,
    pub params: Vec<CustomPostParam>,
}

impl SkyShaderPass {
    pub fn new(path: impl Into<Cow<'static, str>>) -> Self {
        Self {
            path: path.into(),
            params: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SkyTime {
    pub time_of_day: f32,
    pub paused: bool,
    pub scale: f32,
}

impl SkyTime {
    pub const fn new() -> Self {
        Self {
            time_of_day: 0.25,
            paused: false,
            scale: 1.0,
        }
    }
}

impl Default for SkyTime {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct Sky3D {
    pub transform: Transform3D,
    pub visible: bool,
    pub active: bool,
    pub day_colors: Vec<[f32; 3]>,
    pub evening_colors: Vec<[f32; 3]>,
    pub night_colors: Vec<[f32; 3]>,
    pub horizon_colors: Vec<[f32; 3]>,
    pub time: SkyTime,
    pub shaders: Vec<SkyShaderPass>,
    pub render_layers: BitMask,
}

impl Sky3D {
    pub fn new() -> Self {
        Self {
            transform: Transform3D::IDENTITY,
            visible: true,
            active: true,
            day_colors: vec![[0.06, 0.12, 0.25], [0.35, 0.55, 0.9], [0.8, 0.9, 1.0]],
            evening_colors: vec![[1.00, 0.62, 0.40], [0.95, 0.42, 0.58], [0.42, 0.20, 0.42]],
            night_colors: vec![[0.01, 0.02, 0.06], [0.04, 0.06, 0.15], [0.09, 0.12, 0.25]],
            horizon_colors: vec![[0.55, 0.57, 0.60], [0.42, 0.43, 0.45], [0.30, 0.31, 0.33]],
            time: SkyTime::new(),
            shaders: Vec::new(),
            render_layers: BitMask::ALL,
        }
    }
}

impl Default for Sky3D {
    fn default() -> Self {
        Self::new()
    }
}
