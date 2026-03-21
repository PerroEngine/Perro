use perro_structs::ColorBlindFilter;

pub trait VisualAccessibilityAPI {
    fn enable_color_blind_filter(&self, mode: ColorBlindFilter, strength: f32);
    fn disable_color_blind_filter(&self);
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
