use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiUnit {
    Pixels(f32),
    Percent(f32),
}

impl UiUnit {
    pub const ZERO: Self = Self::Pixels(0.0);

    pub const fn px(value: f32) -> Self {
        Self::Pixels(value)
    }

    pub const fn pct(value: f32) -> Self {
        Self::Percent(value)
    }

    pub const fn ratio(value: f32) -> Self {
        Self::Percent(value * 100.0)
    }

    pub fn resolve(self, parent_axis: f32) -> f32 {
        match self {
            Self::Pixels(value) => value,
            Self::Percent(value) => parent_axis * (value * 0.01),
        }
    }
}

impl Default for UiUnit {
    fn default() -> Self {
        Self::ZERO
    }
}

impl From<f32> for UiUnit {
    fn from(value: f32) -> Self {
        Self::Pixels(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiVector2 {
    pub x: UiUnit,
    pub y: UiUnit,
}

impl UiVector2 {
    pub const ZERO: Self = Self::pixels(0.0, 0.0);

    pub const fn new(x: UiUnit, y: UiUnit) -> Self {
        Self { x, y }
    }

    pub const fn pixels(x: f32, y: f32) -> Self {
        Self {
            x: UiUnit::Pixels(x),
            y: UiUnit::Pixels(y),
        }
    }

    pub const fn percent(x: f32, y: f32) -> Self {
        Self {
            x: UiUnit::Percent(x),
            y: UiUnit::Percent(y),
        }
    }

    pub const fn ratio(x: f32, y: f32) -> Self {
        Self {
            x: UiUnit::ratio(x),
            y: UiUnit::ratio(y),
        }
    }

    pub fn resolve(self, parent_size: Vector2) -> Vector2 {
        Vector2::new(self.x.resolve(parent_size.x), self.y.resolve(parent_size.y))
    }

    pub fn resolve_centered(self, parent_size: Vector2) -> Vector2 {
        Vector2::new(
            resolve_centered_unit(self.x, parent_size.x),
            resolve_centered_unit(self.y, parent_size.y),
        )
    }
}

fn resolve_centered_unit(unit: UiUnit, parent_axis: f32) -> f32 {
    match unit {
        UiUnit::Pixels(value) => value,
        UiUnit::Percent(value) => parent_axis * ((value - 50.0) * 0.01),
    }
}

impl Default for UiVector2 {
    fn default() -> Self {
        Self::ZERO
    }
}

impl From<Vector2> for UiVector2 {
    fn from(value: Vector2) -> Self {
        Self::pixels(value.x, value.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl UiRect {
    pub const ZERO: Self = Self::all(0.0);

    pub const fn all(value: f32) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }

    pub const fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Self {
            left: horizontal,
            top: vertical,
            right: horizontal,
            bottom: vertical,
        }
    }

    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub const fn horizontal(self) -> f32 {
        self.left + self.right
    }

    pub const fn vertical(self) -> f32 {
        self.top + self.bottom
    }
}

impl Default for UiRect {
    fn default() -> Self {
        Self::ZERO
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiSizeMode {
    #[default]
    Fixed,
    Fill,
    FitChildren,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiHorizontalAlign {
    Left,
    #[default]
    Center,
    Right,
    Fill,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiVerticalAlign {
    Top,
    #[default]
    Center,
    Bottom,
    Fill,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiTextAlign {
    Start,
    #[default]
    Center,
    End,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiContainerKind {
    #[default]
    None,
    HLayout,
    VLayout,
    Grid,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiLayoutMode {
    #[default]
    H,
    V,
    Grid,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiLayoutSpacingMode {
    #[default]
    Fixed,
    Fill,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiAnchor {
    #[default]
    Center,
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl UiAnchor {
    pub const fn direction(self) -> Vector2 {
        match self {
            Self::Center => Vector2::ZERO,
            Self::Left => Vector2::new(-1.0, 0.0),
            Self::Right => Vector2::new(1.0, 0.0),
            Self::Top => Vector2::new(0.0, 1.0),
            Self::Bottom => Vector2::new(0.0, -1.0),
            Self::TopLeft => Vector2::new(-1.0, 1.0),
            Self::TopRight => Vector2::new(1.0, 1.0),
            Self::BottomLeft => Vector2::new(-1.0, -1.0),
            Self::BottomRight => Vector2::new(1.0, -1.0),
        }
    }
}
