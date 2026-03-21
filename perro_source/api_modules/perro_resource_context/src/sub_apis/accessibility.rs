use perro_structs::ColorBlindFilter;

pub trait AccessibilityAPI {
    fn enable_color_blind_filter(&self, mode: ColorBlindFilter, strength: f32);
    fn disable_color_blind_filter(&self);
}

pub struct AccessibilityModule<'res, R: AccessibilityAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: AccessibilityAPI + ?Sized> AccessibilityModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn enable_color_blind(&self, mode: ColorBlindFilter, strength: f32) {
        self.api.enable_color_blind_filter(mode, strength);
    }

    #[inline]
    pub fn disable_color_blind(&self) {
        self.api.disable_color_blind_filter();
    }
}

#[macro_export]
macro_rules! enable_colorblind_filter {
    ($res:expr, $mode:expr, $strength:expr) => {
        $res.enable_colorblind_filter($mode, $strength)
    };
}

#[macro_export]
macro_rules! disable_colorblind_filter {
    ($res:expr) => {
        $res.disable_colorblind_filter()
    };
}
