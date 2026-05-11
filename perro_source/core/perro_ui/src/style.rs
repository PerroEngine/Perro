use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct UiStyle {
    pub fill: Color,
    pub stroke: Color,
    pub stroke_width: f32,
    /// 0.0 = square corners, 1.0 = half of the shortest side.
    pub corner_radius: f32,
    pub shadow: UiDepthEffect,
    pub highlight: UiDepthEffect,
}

impl UiStyle {
    pub const fn panel() -> Self {
        Self {
            fill: Color::new(0.11, 0.12, 0.14, 0.92),
            stroke: Color::new(0.22, 0.24, 0.28, 1.0),
            stroke_width: 1.0,
            corner_radius: 0.2,
            shadow: UiDepthEffect::none(),
            highlight: UiDepthEffect::none(),
        }
    }

    pub const fn button() -> Self {
        Self {
            fill: Color::new(0.18, 0.20, 0.24, 1.0),
            stroke: Color::new(0.32, 0.35, 0.40, 1.0),
            stroke_width: 1.0,
            corner_radius: 0.2,
            shadow: UiDepthEffect::none(),
            highlight: UiDepthEffect::none(),
        }
    }
}

impl Default for UiStyle {
    fn default() -> Self {
        Self::panel()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiDepthEffect {
    pub color: Color,
    pub distance: f32,
    pub falloff: f32,
    pub vector: Vector2,
    pub size: f32,
}

impl UiDepthEffect {
    pub const fn none() -> Self {
        Self {
            color: Color::TRANSPARENT,
            distance: 0.0,
            falloff: 0.0,
            vector: Vector2::new(0.0, -1.0),
            size: 1.0,
        }
    }
}

impl Default for UiDepthEffect {
    fn default() -> Self {
        Self::none()
    }
}
