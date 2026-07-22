#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HdrMode {
    Off,
    #[default]
    Auto,
    On,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HdrColorSpace {
    #[default]
    SdrSrgb,
    ExtendedSrgbLinear,
    ExtendedSrgb,
    Bt2100Pq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdrFallback {
    Disabled,
    SurfaceUnsupported,
    DisplayUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HdrStatus {
    pub requested: HdrMode,
    pub supported: bool,
    pub active: bool,
    pub scene_hdr: bool,
    pub color_space: HdrColorSpace,
    pub headroom: f32,
    pub peak_nits: Option<f32>,
    pub fallback: Option<HdrFallback>,
}

impl Default for HdrStatus {
    fn default() -> Self {
        Self {
            requested: HdrMode::Auto,
            supported: false,
            active: false,
            scene_hdr: false,
            color_space: HdrColorSpace::SdrSrgb,
            headroom: 1.0,
            peak_nits: None,
            fallback: Some(HdrFallback::DisplayUnavailable),
        }
    }
}
