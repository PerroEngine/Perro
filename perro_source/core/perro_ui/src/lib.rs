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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComputedUiRect {
    pub center: Vector2,
    pub size: Vector2,
}

impl ComputedUiRect {
    pub const fn new(center: Vector2, size: Vector2) -> Self {
        Self { center, size }
    }

    pub fn min(self) -> Vector2 {
        self.center - self.size * 0.5
    }

    pub fn max(self) -> Vector2 {
        self.center + self.size * 0.5
    }

    pub fn contains(self, point: Vector2) -> bool {
        let min = self.min();
        let max = self.max();
        point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
    }

    pub fn inset(self, inset: UiRect) -> Self {
        let min = self.min() + Vector2::new(inset.left, inset.bottom);
        let max = self.max() - Vector2::new(inset.right, inset.top);
        let size = Vector2::new((max.x - min.x).max(0.0), (max.y - min.y).max(0.0));
        Self::new(min + size * 0.5, size)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiTransform {
    pub position: UiVector2,
    pub pivot: UiVector2,
    pub translation: Vector2,
    pub scale: Vector2,
    pub rotation: f32,
}

impl UiTransform {
    pub const fn new() -> Self {
        Self {
            position: UiVector2::percent(50.0, 50.0),
            pivot: UiVector2::percent(50.0, 50.0),
            translation: Vector2::ZERO,
            scale: Vector2::ONE,
            rotation: 0.0,
        }
    }

    pub fn resolved_position(&self, parent_size: Vector2) -> Vector2 {
        self.position.resolve(parent_size) + self.translation
    }

    pub fn scale_size(&self, size: Vector2) -> Vector2 {
        Vector2::new(size.x * self.scale.x, size.y * self.scale.y)
    }

    pub fn resolved_pivot_offset(&self, resolved_size: Vector2) -> Vector2 {
        self.pivot.resolve(resolved_size)
    }
}

impl Default for UiTransform {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiLayoutData {
    pub anchor: UiAnchor,
    pub size: UiVector2,
    pub min_size: Vector2,
    pub max_size: Vector2,
    pub margin: UiRect,
    pub padding: UiRect,
    pub h_size: UiSizeMode,
    pub v_size: UiSizeMode,
    pub h_align: UiHorizontalAlign,
    pub v_align: UiVerticalAlign,
    pub z_index: i32,
}

impl UiLayoutData {
    pub const NO_MAX_SIZE: Vector2 = Vector2::new(f32::INFINITY, f32::INFINITY);

    pub const fn new() -> Self {
        Self {
            anchor: UiAnchor::Center,
            size: UiVector2::ZERO,
            min_size: Vector2::ZERO,
            max_size: Self::NO_MAX_SIZE,
            margin: UiRect::ZERO,
            padding: UiRect::ZERO,
            h_size: UiSizeMode::Fixed,
            v_size: UiSizeMode::Fixed,
            h_align: UiHorizontalAlign::Center,
            v_align: UiVerticalAlign::Center,
            z_index: 0,
        }
    }

    pub fn resolved_size(&self, parent_size: Vector2) -> Vector2 {
        let size = self.size.resolve(parent_size);
        self.clamp_size(size)
    }

    pub fn clamp_size(&self, size: Vector2) -> Vector2 {
        Vector2::new(
            size.x.max(self.min_size.x).min(self.max_size.x),
            size.y.max(self.min_size.y).min(self.max_size.y),
        )
    }

    pub fn resolved_scaled_size(&self, transform: &UiTransform, parent_size: Vector2) -> Vector2 {
        let size = self.resolved_size(parent_size);
        transform.scale_size(size)
    }

    pub fn resolved_origin(&self, transform: &UiTransform, parent_size: Vector2) -> Vector2 {
        let size = self.resolved_size(parent_size);
        transform.resolved_position(parent_size) - transform.resolved_pivot_offset(size)
    }

    pub fn compute_rect(&self, transform: &UiTransform, parent: ComputedUiRect) -> ComputedUiRect {
        let size = self.resolved_scaled_size(transform, parent.size);
        self.compute_rect_with_size(transform, parent, size)
    }

    pub fn compute_rect_with_size(
        &self,
        transform: &UiTransform,
        parent: ComputedUiRect,
        size: Vector2,
    ) -> ComputedUiRect {
        let anchor = self.anchor.direction();
        let anchor_point = parent.center
            + Vector2::new(
                parent.size.x * 0.5 * anchor.x,
                parent.size.y * 0.5 * anchor.y,
            );
        let inward_from_edge = Vector2::new(size.x * 0.5 * anchor.x, size.y * 0.5 * anchor.y);
        let pivot = transform.pivot.resolve(Vector2::new(1.0, 1.0));
        let pivot_offset = Vector2::new((0.5 - pivot.x) * size.x, (0.5 - pivot.y) * size.y);
        let position = transform.position.resolve_centered(parent.size);

        ComputedUiRect::new(
            anchor_point - inward_from_edge + position + transform.translation + pivot_offset,
            size,
        )
    }
}

impl Default for UiLayoutData {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiBox {
    pub transform: UiTransform,
    pub layout: UiLayoutData,
    pub visible: bool,
    pub input_enabled: bool,
    pub mouse_filter: UiMouseFilter,
}

impl UiBox {
    pub const fn new() -> Self {
        Self {
            transform: UiTransform::new(),
            layout: UiLayoutData::new(),
            visible: true,
            input_enabled: true,
            mouse_filter: UiMouseFilter::Stop,
        }
    }
}

impl Default for UiBox {
    fn default() -> Self {
        Self::new()
    }
}

pub trait UiNodeBase {
    fn ui_base(&self) -> &UiBox;
    fn ui_base_mut(&mut self) -> &mut UiBox;
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
    pub base: UiBox,
    pub style: UiStyle,
}

impl UiPanel {
    pub const fn new() -> Self {
        Self {
            base: UiBox::new(),
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
    type Target = UiBox;

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
    fn ui_base(&self) -> &UiBox {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiBox {
        &mut self.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiLabel {
    pub base: UiBox,
    pub text: Cow<'static, str>,
    pub color: Color,
    pub font_size: f32,
    pub h_align: UiTextAlign,
    pub v_align: UiTextAlign,
}

impl UiLabel {
    pub const fn new() -> Self {
        Self {
            base: UiBox::new(),
            text: Cow::Borrowed(""),
            color: Color::WHITE,
            font_size: 16.0,
            h_align: UiTextAlign::Center,
            v_align: UiTextAlign::Center,
        }
    }

    pub fn with_text<T>(mut self, text: T) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        self.text = text.into();
        self
    }

    pub fn set_text<T>(&mut self, text: T)
    where
        T: Into<Cow<'static, str>>,
    {
        self.text = text.into();
    }
}

impl Default for UiLabel {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiLabel {
    type Target = UiBox;

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
    fn ui_base(&self) -> &UiBox {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiBox {
        &mut self.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiButton {
    pub base: UiBox,
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
            base: UiBox::new(),
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
    type Target = UiBox;

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
    fn ui_base(&self) -> &UiBox {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiBox {
        &mut self.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiLayoutContainer {
    pub base: UiBox,
    pub mode: UiLayoutMode,
    pub spacing: f32,
    pub h_spacing: f32,
    pub v_spacing: f32,
    pub columns: u32,
}

impl UiLayoutContainer {
    pub const fn new(mode: UiLayoutMode) -> Self {
        Self {
            base: UiBox::new(),
            mode,
            spacing: 0.0,
            h_spacing: 0.0,
            v_spacing: 0.0,
            columns: 1,
        }
    }

    pub const fn horizontal() -> Self {
        let mut value = Self::new(UiLayoutMode::H);
        value.base.layout.v_align = UiVerticalAlign::Center;
        value
    }

    pub const fn vertical() -> Self {
        let mut value = Self::new(UiLayoutMode::V);
        value.base.layout.h_align = UiHorizontalAlign::Center;
        value
    }

    pub const fn grid() -> Self {
        Self::new(UiLayoutMode::Grid)
    }
}

impl Default for UiLayoutContainer {
    fn default() -> Self {
        Self::new(UiLayoutMode::H)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiFixedLayoutContainer {
    pub base: UiBox,
    pub spacing: f32,
    pub h_spacing: f32,
    pub v_spacing: f32,
    pub columns: u32,
}

impl UiFixedLayoutContainer {
    pub const fn new() -> Self {
        let mut value = Self {
            base: UiBox::new(),
            spacing: 0.0,
            h_spacing: 0.0,
            v_spacing: 0.0,
            columns: 1,
        };
        value.base.layout.v_align = UiVerticalAlign::Center;
        value
    }
}

impl Default for UiFixedLayoutContainer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiLayout {
    pub inner: UiLayoutContainer,
}

impl UiLayout {
    pub const fn new() -> Self {
        Self {
            inner: UiLayoutContainer::horizontal(),
        }
    }
}

impl Default for UiLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiLayout {
    type Target = UiBox;

    fn deref(&self) -> &Self::Target {
        &self.inner.base
    }
}

impl DerefMut for UiLayout {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner.base
    }
}

impl UiNodeBase for UiLayout {
    fn ui_base(&self) -> &UiBox {
        &self.inner.base
    }

    fn ui_base_mut(&mut self) -> &mut UiBox {
        &mut self.inner.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiHLayout {
    pub inner: UiFixedLayoutContainer,
}

impl UiHLayout {
    pub const fn new() -> Self {
        Self {
            inner: UiFixedLayoutContainer::new(),
        }
    }

    pub const fn mode(&self) -> UiLayoutMode {
        UiLayoutMode::H
    }
}

impl Default for UiHLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiHLayout {
    type Target = UiBox;

    fn deref(&self) -> &Self::Target {
        &self.inner.base
    }
}

impl DerefMut for UiHLayout {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner.base
    }
}

impl UiNodeBase for UiHLayout {
    fn ui_base(&self) -> &UiBox {
        &self.inner.base
    }

    fn ui_base_mut(&mut self) -> &mut UiBox {
        &mut self.inner.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiVLayout {
    pub inner: UiFixedLayoutContainer,
}

impl UiVLayout {
    pub const fn new() -> Self {
        let mut value = Self {
            inner: UiFixedLayoutContainer::new(),
        };
        value.inner.base.layout.h_align = UiHorizontalAlign::Center;
        value
    }

    pub const fn mode(&self) -> UiLayoutMode {
        UiLayoutMode::V
    }
}

impl Default for UiVLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiVLayout {
    type Target = UiBox;

    fn deref(&self) -> &Self::Target {
        &self.inner.base
    }
}

impl DerefMut for UiVLayout {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner.base
    }
}

impl UiNodeBase for UiVLayout {
    fn ui_base(&self) -> &UiBox {
        &self.inner.base
    }

    fn ui_base_mut(&mut self) -> &mut UiBox {
        &mut self.inner.base
    }
}

pub type UiHBox = UiHLayout;
pub type UiVBox = UiVLayout;

#[derive(Clone, Debug, PartialEq)]
pub struct UiGrid {
    pub base: UiBox,
    pub columns: u32,
    pub h_spacing: f32,
    pub v_spacing: f32,
}

impl UiGrid {
    pub const fn new() -> Self {
        Self {
            base: UiBox::new(),
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
    type Target = UiBox;

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
    fn ui_base(&self) -> &UiBox {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiBox {
        &mut self.base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_position_resolves_against_viewport() {
        let mut transform = UiTransform::new();
        transform.position = UiVector2::percent(50.0, 50.0);

        assert_eq!(
            transform.resolved_position(Vector2::new(1920.0, 1080.0)),
            Vector2::new(960.0, 540.0)
        );
    }

    #[test]
    fn default_layout_origin_centers_in_parent() {
        let mut layout = UiLayoutData::new();
        let transform = UiTransform::new();
        layout.size = UiVector2::pixels(200.0, 100.0);

        assert_eq!(
            layout.resolved_origin(&transform, Vector2::new(800.0, 600.0)),
            Vector2::new(300.0, 250.0)
        );
    }

    #[test]
    fn default_layout_aligns_children_to_center() {
        let transform = UiTransform::new();
        let layout = UiLayoutData::new();

        assert_eq!(layout.anchor, UiAnchor::Center);
        assert_eq!(transform.position, UiVector2::ratio(0.5, 0.5));
        assert_eq!(layout.h_align, UiHorizontalAlign::Center);
        assert_eq!(layout.v_align, UiVerticalAlign::Center);
    }

    #[test]
    fn label_text_align_defaults_to_center() {
        let label = UiLabel::new();

        assert_eq!(label.h_align, UiTextAlign::Center);
        assert_eq!(label.v_align, UiTextAlign::Center);
    }

    #[test]
    fn label_set_text_accepts_static_str_string_and_cow() {
        let mut label = UiLabel::new();

        label.set_text("static text");
        assert!(matches!(label.text, Cow::Borrowed("static text")));

        label.set_text(String::from("owned text"));
        assert!(matches!(label.text, Cow::Owned(ref text) if text == "owned text"));

        label.set_text(Cow::Borrowed("cow text"));
        assert!(matches!(label.text, Cow::Borrowed("cow text")));
    }

    #[test]
    fn pixel_and_percent_units_can_mix() {
        let value = UiVector2::new(UiUnit::px(24.0), UiUnit::pct(25.0));

        assert_eq!(
            value.resolve(Vector2::new(800.0, 600.0)),
            Vector2::new(24.0, 150.0)
        );
    }

    #[test]
    fn centered_position_percent_resolves_as_offset() {
        let value = UiVector2::percent(50.0, 25.0);

        assert_eq!(
            value.resolve_centered(Vector2::new(800.0, 600.0)),
            Vector2::new(0.0, -150.0)
        );
    }

    #[test]
    fn ratio_units_match_percent_units() {
        let value = UiVector2::ratio(0.5, 0.25);

        assert_eq!(
            value.resolve(Vector2::new(800.0, 600.0)),
            Vector2::new(400.0, 150.0)
        );
        assert_eq!(
            value.resolve_centered(Vector2::new(800.0, 600.0)),
            Vector2::new(0.0, -150.0)
        );
    }

    #[test]
    fn size_respects_min_and_max_size() {
        let mut layout = UiLayoutData::new();
        layout.size = UiVector2::ratio(0.5, 0.1);
        layout.min_size = Vector2::new(300.0, 80.0);
        layout.max_size = Vector2::new(1200.0, 90.0);

        assert_eq!(
            layout.resolved_size(Vector2::new(3000.0, 1000.0)),
            Vector2::new(1200.0, 90.0)
        );
        assert_eq!(
            layout.resolved_size(Vector2::new(400.0, 400.0)),
            Vector2::new(300.0, 80.0)
        );
    }

    #[test]
    fn scale_applies_after_size_clamp() {
        let mut layout = UiLayoutData::new();
        let mut transform = UiTransform::new();
        layout.size = UiVector2::ratio(0.5, 0.1);
        layout.max_size = Vector2::new(1200.0, 90.0);
        transform.scale = Vector2::new(2.0, 0.5);

        let parent = ComputedUiRect::new(Vector2::ZERO, Vector2::new(3000.0, 1000.0));
        let rect = layout.compute_rect(&transform, parent);

        assert_eq!(rect.size, Vector2::new(2400.0, 45.0));
    }

    #[test]
    fn rect_inset_uses_top_bottom_edges() {
        let rect = ComputedUiRect::new(Vector2::ZERO, Vector2::new(100.0, 80.0));

        assert_eq!(
            rect.inset(UiRect::new(10.0, 20.0, 30.0, 5.0)),
            ComputedUiRect::new(Vector2::new(-10.0, -7.5), Vector2::new(60.0, 55.0))
        );
    }

    #[test]
    fn right_anchor_offsets_size_inward() {
        let mut layout = UiLayoutData::new();
        let mut transform = UiTransform::new();
        layout.anchor = UiAnchor::Right;
        transform.position = UiVector2::ZERO;
        layout.size = UiVector2::pixels(200.0, 100.0);
        transform.pivot = UiVector2::percent(50.0, 50.0);

        let parent = ComputedUiRect::new(Vector2::ZERO, Vector2::new(800.0, 600.0));
        let rect = layout.compute_rect(&transform, parent);

        assert_eq!(rect.center, Vector2::new(300.0, 0.0));
        assert_eq!(rect.max().x, 400.0);
    }

    #[test]
    fn top_left_anchor_uses_center_origin_y_up() {
        let mut layout = UiLayoutData::new();
        let mut transform = UiTransform::new();
        layout.anchor = UiAnchor::TopLeft;
        transform.position = UiVector2::ZERO;
        layout.size = UiVector2::pixels(100.0, 50.0);
        transform.pivot = UiVector2::percent(50.0, 50.0);

        let parent = ComputedUiRect::new(Vector2::ZERO, Vector2::new(800.0, 600.0));
        let rect = layout.compute_rect(&transform, parent);

        assert_eq!(rect.center, Vector2::new(-350.0, 275.0));
        assert_eq!(rect.min(), Vector2::new(-400.0, 250.0));
        assert_eq!(rect.max(), Vector2::new(-300.0, 300.0));
    }
}
