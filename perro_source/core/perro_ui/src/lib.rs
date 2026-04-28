use perro_structs::{Color, Vector2};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

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

    pub fn resolve(self, parent_size: Vector2) -> Vector2 {
        Vector2::new(self.x.resolve(parent_size.x), self.y.resolve(parent_size.y))
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
    #[default]
    Left,
    Center,
    Right,
    Fill,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiVerticalAlign {
    #[default]
    Top,
    Center,
    Bottom,
    Fill,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiTextAlign {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiContainerKind {
    #[default]
    None,
    HBox,
    VBox,
    Grid,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiLayout {
    pub position: UiVector2,
    pub size: UiVector2,
    pub pivot: UiVector2,
    pub translation: Vector2,
    pub min_size: Vector2,
    pub margin: UiRect,
    pub padding: UiRect,
    pub h_size: UiSizeMode,
    pub v_size: UiSizeMode,
    pub h_align: UiHorizontalAlign,
    pub v_align: UiVerticalAlign,
    pub z_index: i32,
}

impl UiLayout {
    pub const fn new() -> Self {
        Self {
            position: UiVector2::percent(50.0, 50.0),
            size: UiVector2::ZERO,
            pivot: UiVector2::percent(50.0, 50.0),
            translation: Vector2::ZERO,
            min_size: Vector2::ZERO,
            margin: UiRect::ZERO,
            padding: UiRect::ZERO,
            h_size: UiSizeMode::Fixed,
            v_size: UiSizeMode::Fixed,
            h_align: UiHorizontalAlign::Left,
            v_align: UiVerticalAlign::Top,
            z_index: 0,
        }
    }

    pub fn resolved_position(&self, parent_size: Vector2) -> Vector2 {
        self.position.resolve(parent_size) + self.translation
    }

    pub fn resolved_size(&self, parent_size: Vector2) -> Vector2 {
        self.size.resolve(parent_size)
    }

    pub fn resolved_pivot_offset(&self, resolved_size: Vector2) -> Vector2 {
        self.pivot.resolve(resolved_size)
    }

    pub fn resolved_origin(&self, parent_size: Vector2) -> Vector2 {
        let size = self.resolved_size(parent_size);
        self.resolved_position(parent_size) - self.resolved_pivot_offset(size)
    }
}

impl Default for UiLayout {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiRoot {
    pub layout: UiLayout,
    pub visible: bool,
    pub input_enabled: bool,
    pub mouse_filter: UiMouseFilter,
}

impl UiRoot {
    pub const fn new() -> Self {
        Self {
            layout: UiLayout::new(),
            visible: true,
            input_enabled: true,
            mouse_filter: UiMouseFilter::Stop,
        }
    }
}

impl Default for UiRoot {
    fn default() -> Self {
        Self::new()
    }
}

pub trait UiNodeBase {
    fn ui_base(&self) -> &UiRoot;
    fn ui_base_mut(&mut self) -> &mut UiRoot;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiMouseFilter {
    #[default]
    Stop,
    Pass,
    Ignore,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiStyle {
    pub fill: Color,
    pub stroke: Color,
    pub stroke_width: f32,
    pub corner_radius: f32,
}

impl UiStyle {
    pub const fn panel() -> Self {
        Self {
            fill: Color::new(0.11, 0.12, 0.14, 0.92),
            stroke: Color::new(0.22, 0.24, 0.28, 1.0),
            stroke_width: 1.0,
            corner_radius: 4.0,
        }
    }

    pub const fn button() -> Self {
        Self {
            fill: Color::new(0.18, 0.20, 0.24, 1.0),
            stroke: Color::new(0.32, 0.35, 0.40, 1.0),
            stroke_width: 1.0,
            corner_radius: 4.0,
        }
    }
}

impl Default for UiStyle {
    fn default() -> Self {
        Self::panel()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiPanel {
    pub base: UiRoot,
    pub style: UiStyle,
}

impl UiPanel {
    pub const fn new() -> Self {
        Self {
            base: UiRoot::new(),
            style: UiStyle::panel(),
        }
    }
}

impl Default for UiPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiPanel {
    type Target = UiRoot;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiPanel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiPanel {
    fn ui_base(&self) -> &UiRoot {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiRoot {
        &mut self.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiLabel {
    pub base: UiRoot,
    pub text: Cow<'static, str>,
    pub color: Color,
    pub font_size: f32,
    pub h_align: UiTextAlign,
    pub v_align: UiTextAlign,
}

impl UiLabel {
    pub const fn new() -> Self {
        Self {
            base: UiRoot::new(),
            text: Cow::Borrowed(""),
            color: Color::WHITE,
            font_size: 16.0,
            h_align: UiTextAlign::Start,
            v_align: UiTextAlign::Start,
        }
    }

    pub fn with_text<T>(mut self, text: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        self.text = text.into();
        self
    }
}

impl Default for UiLabel {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiLabel {
    type Target = UiRoot;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiLabel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiLabel {
    fn ui_base(&self) -> &UiRoot {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiRoot {
        &mut self.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiButton {
    pub base: UiRoot,
    pub text: Cow<'static, str>,
    pub text_color: Color,
    pub style: UiStyle,
    pub pressed_style: UiStyle,
    pub hover_style: UiStyle,
    pub disabled: bool,
}

impl UiButton {
    pub const fn new() -> Self {
        Self {
            base: UiRoot::new(),
            text: Cow::Borrowed("Button"),
            text_color: Color::WHITE,
            style: UiStyle::button(),
            pressed_style: UiStyle {
                fill: Color::new(0.12, 0.14, 0.18, 1.0),
                stroke: Color::new(0.42, 0.46, 0.54, 1.0),
                stroke_width: 1.0,
                corner_radius: 4.0,
            },
            hover_style: UiStyle {
                fill: Color::new(0.24, 0.27, 0.32, 1.0),
                stroke: Color::new(0.42, 0.46, 0.54, 1.0),
                stroke_width: 1.0,
                corner_radius: 4.0,
            },
            disabled: false,
        }
    }

    pub fn with_text<T>(mut self, text: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        self.text = text.into();
        self
    }
}

impl Default for UiButton {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiButton {
    type Target = UiRoot;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiButton {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiButton {
    fn ui_base(&self) -> &UiRoot {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiRoot {
        &mut self.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiBoxContainer {
    pub base: UiRoot,
    pub direction: UiBoxDirection,
    pub spacing: f32,
}

impl UiBoxContainer {
    pub const fn horizontal() -> Self {
        Self {
            base: UiRoot::new(),
            direction: UiBoxDirection::Horizontal,
            spacing: 0.0,
        }
    }

    pub const fn vertical() -> Self {
        Self {
            base: UiRoot::new(),
            direction: UiBoxDirection::Vertical,
            spacing: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UiBoxDirection {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiHBox {
    pub inner: UiBoxContainer,
}

impl UiHBox {
    pub const fn new() -> Self {
        Self {
            inner: UiBoxContainer::horizontal(),
        }
    }
}

impl Default for UiHBox {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiHBox {
    type Target = UiRoot;

    fn deref(&self) -> &Self::Target {
        &self.inner.base
    }
}

impl DerefMut for UiHBox {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner.base
    }
}

impl UiNodeBase for UiHBox {
    fn ui_base(&self) -> &UiRoot {
        &self.inner.base
    }

    fn ui_base_mut(&mut self) -> &mut UiRoot {
        &mut self.inner.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiVBox {
    pub inner: UiBoxContainer,
}

impl UiVBox {
    pub const fn new() -> Self {
        Self {
            inner: UiBoxContainer::vertical(),
        }
    }
}

impl Default for UiVBox {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiVBox {
    type Target = UiRoot;

    fn deref(&self) -> &Self::Target {
        &self.inner.base
    }
}

impl DerefMut for UiVBox {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner.base
    }
}

impl UiNodeBase for UiVBox {
    fn ui_base(&self) -> &UiRoot {
        &self.inner.base
    }

    fn ui_base_mut(&mut self) -> &mut UiRoot {
        &mut self.inner.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiGrid {
    pub base: UiRoot,
    pub columns: u32,
    pub h_spacing: f32,
    pub v_spacing: f32,
}

impl UiGrid {
    pub const fn new() -> Self {
        Self {
            base: UiRoot::new(),
            columns: 1,
            h_spacing: 0.0,
            v_spacing: 0.0,
        }
    }
}

impl Default for UiGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiGrid {
    type Target = UiRoot;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiGrid {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiGrid {
    fn ui_base(&self) -> &UiRoot {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiRoot {
        &mut self.base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_position_resolves_against_viewport() {
        let mut layout = UiLayout::new();
        layout.position = UiVector2::percent(50.0, 50.0);

        assert_eq!(
            layout.resolved_position(Vector2::new(1920.0, 1080.0)),
            Vector2::new(960.0, 540.0)
        );
    }

    #[test]
    fn default_layout_origin_centers_in_parent() {
        let mut layout = UiLayout::new();
        layout.size = UiVector2::pixels(200.0, 100.0);

        assert_eq!(
            layout.resolved_origin(Vector2::new(800.0, 600.0)),
            Vector2::new(300.0, 250.0)
        );
    }

    #[test]
    fn pixel_and_percent_units_can_mix() {
        let value = UiVector2::new(UiUnit::px(24.0), UiUnit::pct(25.0));

        assert_eq!(
            value.resolve(Vector2::new(800.0, 600.0)),
            Vector2::new(24.0, 150.0)
        );
    }
}
