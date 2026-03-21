#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorBlindFilter {
    Protan,
    Deuteran,
    Tritan,
    Achroma,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorBlindSetting {
    pub filter: ColorBlindFilter,
    pub strength: f32,
}

impl ColorBlindSetting {
    #[inline]
    pub fn new(filter: ColorBlindFilter, strength: f32) -> Self {
        Self { filter, strength }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VisualAccessibilitySettings {
    pub color_blind: Option<ColorBlindSetting>,
}

impl VisualAccessibilitySettings {
    #[inline]
    pub const fn new() -> Self {
        Self { color_blind: None }
    }

    #[inline]
    pub fn with_color_blind(mut self, filter: ColorBlindFilter, strength: f32) -> Self {
        self.color_blind = Some(ColorBlindSetting { filter, strength });
        self
    }

    #[inline]
    pub fn clear_color_blind(&mut self) {
        self.color_blind = None;
    }
}
