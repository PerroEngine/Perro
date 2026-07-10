use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiFillKind {
    #[default]
    Solid,
    Linear,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiLinearGradient {
    pub start_color: Color,
    pub end_color: Color,
    pub vector: Vector2,
}

impl UiLinearGradient {
    pub const fn new(start_color: Color, end_color: Color, vector: Vector2) -> Self {
        Self {
            start_color,
            end_color,
            vector,
        }
    }

    pub const fn none() -> Self {
        Self::new(
            Color::TRANSPARENT,
            Color::TRANSPARENT,
            Vector2::new(0.0, -1.0),
        )
    }
}

impl Default for UiLinearGradient {
    fn default() -> Self {
        Self::none()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiCornerRadii {
    pub tl: f32,
    pub tr: f32,
    pub br: f32,
    pub bl: f32,
}

impl UiCornerRadii {
    pub const fn new(tl: f32, tr: f32, br: f32, bl: f32) -> Self {
        Self { tl, tr, br, bl }
    }

    pub const fn zero() -> Self {
        Self::all(0.0)
    }

    pub const fn all(radius: f32) -> Self {
        Self::new(radius, radius, radius, radius)
    }
}

impl Default for UiCornerRadii {
    fn default() -> Self {
        Self::zero()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiStyle {
    pub fill: Color,
    pub fill_kind: UiFillKind,
    pub gradient: UiLinearGradient,
    pub stroke: Color,
    pub stroke_width: f32,
    /// 0.0 = square corners, 1.0 = half of the shortest side.
    pub corner_radii: UiCornerRadii,
    pub outer_shadow: UiDepthEffect,
    pub inner_shadow: UiDepthEffect,
    pub outer_highlight: UiDepthEffect,
    pub inner_highlight: UiDepthEffect,
}

impl UiStyle {
    pub const fn panel() -> Self {
        Self {
            fill: Color::new(0.11, 0.12, 0.14, 0.92),
            fill_kind: UiFillKind::Solid,
            gradient: UiLinearGradient::none(),
            stroke: Color::new(0.22, 0.24, 0.28, 1.0),
            stroke_width: 1.0,
            corner_radii: UiCornerRadii::all(0.2),
            outer_shadow: UiDepthEffect {
                color: Color::new(0.0, 0.0, 0.0, 0.28),
                distance: 2.0,
                falloff: 4.0,
                vector: Vector2::new(0.0, -1.0),
                size: 1.0,
            },
            inner_shadow: UiDepthEffect::none(),
            outer_highlight: UiDepthEffect::none(),
            inner_highlight: UiDepthEffect {
                color: Color::new(1.0, 1.0, 1.0, 0.035),
                distance: 1.0,
                falloff: 1.0,
                vector: Vector2::new(0.0, 1.0),
                size: 1.0,
            },
        }
    }

    pub const fn button() -> Self {
        Self {
            fill: Color::new(0.18, 0.20, 0.24, 1.0),
            fill_kind: UiFillKind::Solid,
            gradient: UiLinearGradient::none(),
            stroke: Color::new(0.32, 0.35, 0.40, 1.0),
            stroke_width: 1.0,
            corner_radii: UiCornerRadii::all(0.2),
            outer_shadow: UiDepthEffect {
                color: Color::new(0.0, 0.0, 0.0, 0.22),
                distance: 1.0,
                falloff: 2.0,
                vector: Vector2::new(0.0, -1.0),
                size: 1.0,
            },
            inner_shadow: UiDepthEffect::none(),
            outer_highlight: UiDepthEffect::none(),
            inner_highlight: UiDepthEffect {
                color: Color::new(1.0, 1.0, 1.0, 0.045),
                distance: 1.0,
                falloff: 1.0,
                vector: Vector2::new(0.0, 1.0),
                size: 1.0,
            },
        }
    }

    pub fn set_corner_radius(&mut self, radius: f32) {
        self.corner_radii = UiCornerRadii::all(radius);
    }

    pub const fn corner_radius(&self) -> f32 {
        self.corner_radii.tl
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
