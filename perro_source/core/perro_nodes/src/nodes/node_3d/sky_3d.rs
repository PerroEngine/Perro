use perro_structs::Transform3D;
use std::borrow::Cow;

#[derive(Clone, Debug)]
pub struct SkyClouds {
    pub size: f32,
    pub density: f32,
    pub variance: f32,
    pub wind_vector: [f32; 2],
}

impl SkyClouds {
    pub const fn new() -> Self {
        Self {
            size: 0.72,
            density: 0.58,
            variance: 0.52,
            wind_vector: [0.06, 0.015],
        }
    }
}

impl Default for SkyClouds {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct SkyStars {
    pub size: f32,
    pub scatter: f32,
    pub gleam: f32,
}

impl SkyStars {
    pub const fn new() -> Self {
        Self {
            size: 1.0,
            scatter: 0.25,
            gleam: 0.4,
        }
    }
}

impl Default for SkyStars {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct SkySun {
    pub size: f32,
}

impl SkySun {
    pub const fn new() -> Self {
        Self { size: 1.0 }
    }
}

impl Default for SkySun {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct SkyMoon {
    pub size: f32,
}

impl SkyMoon {
    pub const fn new() -> Self {
        Self { size: 0.6 }
    }
}

impl Default for SkyMoon {
    fn default() -> Self {
        Self::new()
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
    pub day_colors: Cow<'static, [[f32; 3]]>,
    pub night_colors: Cow<'static, [[f32; 3]]>,
    pub sky_angle: f32,
    pub time: SkyTime,
    pub clouds: SkyClouds,
    pub stars: SkyStars,
    pub sun: SkySun,
    pub moon: SkyMoon,
    pub sky_shader: Option<Cow<'static, str>>,
}

impl Sky3D {
    pub const fn new() -> Self {
        Self {
            transform: Transform3D::IDENTITY,
            visible: true,
            active: true,
            day_colors: Cow::Borrowed(&[[0.06, 0.12, 0.25], [0.35, 0.55, 0.9], [0.8, 0.9, 1.0]]),
            night_colors: Cow::Borrowed(&[[0.01, 0.02, 0.06], [0.04, 0.06, 0.15], [0.09, 0.12, 0.25]]),
            sky_angle: 0.0,
            time: SkyTime::new(),
            clouds: SkyClouds::new(),
            stars: SkyStars::new(),
            sun: SkySun::new(),
            moon: SkyMoon::new(),
            sky_shader: None,
        }
    }
}

impl Default for Sky3D {
    fn default() -> Self {
        Self::new()
    }
}
