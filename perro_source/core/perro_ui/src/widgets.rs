use super::*;

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiImageScaleMode {
    #[default]
    Stretch,
    Fit,
    Cover,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiImage {
    pub base: UiBox,
    pub texture: TextureID,
    pub texture_region: Option<[f32; 4]>,
    pub tint: Color,
    pub scale_mode: UiImageScaleMode,
    pub h_align: UiTextAlign,
    pub v_align: UiTextAlign,
    pub aspect_ratio: f32,
}

impl UiImage {
    pub const fn new() -> Self {
        Self {
            base: UiBox::new(),
            texture: TextureID::nil(),
            texture_region: None,
            tint: Color::WHITE,
            scale_mode: UiImageScaleMode::Stretch,
            h_align: UiTextAlign::Center,
            v_align: UiTextAlign::Center,
            aspect_ratio: 0.0,
        }
    }
}

impl Default for UiImage {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiImage {
    type Target = UiBox;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiImage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiImage {
    fn ui_base(&self) -> &UiBox {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiBox {
        &mut self.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiAnimatedImageFrameSet {
    pub name: Cow<'static, str>,
    pub start: [f32; 2],
    pub frame_size: [f32; 2],
    pub frame_count: u32,
    pub columns: u32,
    pub fps: f32,
}

impl Default for UiAnimatedImageFrameSet {
    fn default() -> Self {
        Self::new("default")
    }
}

impl UiAnimatedImageFrameSet {
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self {
            name: name.into(),
            start: [0.0, 0.0],
            frame_size: [0.0, 0.0],
            frame_count: 1,
            columns: 0,
            fps: 12.0,
        }
    }

    pub fn texture_region_for_frame(&self, current_frame: u32) -> Option<[f32; 4]> {
        let [w, h] = self.frame_size;
        if !(w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
            return None;
        }

        let frame_count = self.frame_count.max(1);
        let frame = current_frame.min(frame_count.saturating_sub(1));
        let (column, row) = if self.columns > 0 {
            (frame % self.columns, frame / self.columns)
        } else {
            (frame, 0)
        };
        let [base_x, base_y] = self.start;

        Some([base_x + column as f32 * w, base_y + row as f32 * h, w, h])
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiAnimatedImage {
    pub base: UiBox,
    pub texture: TextureID,
    pub animations: Vec<UiAnimatedImageFrameSet>,
    pub current_animation: Cow<'static, str>,
    pub current_frame: u32,
    pub fps_scale: f32,
    pub playing: bool,
    pub looping: bool,
    pub frame_accum: f32,
    pub tint: Color,
    pub scale_mode: UiImageScaleMode,
    pub h_align: UiTextAlign,
    pub v_align: UiTextAlign,
    pub aspect_ratio: f32,
}

impl UiAnimatedImage {
    pub const fn new() -> Self {
        Self {
            base: UiBox::new(),
            texture: TextureID::nil(),
            animations: Vec::new(),
            current_animation: Cow::Borrowed("default"),
            current_frame: 0,
            fps_scale: 1.0,
            playing: true,
            looping: true,
            frame_accum: 0.0,
            tint: Color::WHITE,
            scale_mode: UiImageScaleMode::Stretch,
            h_align: UiTextAlign::Center,
            v_align: UiTextAlign::Center,
            aspect_ratio: 0.0,
        }
    }

    pub fn current_animation_data(&self) -> Option<&UiAnimatedImageFrameSet> {
        self.animations
            .iter()
            .find(|animation| animation.name.as_ref() == self.current_animation.as_ref())
            .or_else(|| self.animations.first())
    }

    pub fn current_texture_region(&self) -> Option<[f32; 4]> {
        self.current_animation_data()
            .and_then(|animation| animation.texture_region_for_frame(self.current_frame))
    }
}

impl Default for UiAnimatedImage {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiAnimatedImage {
    type Target = UiBox;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiAnimatedImage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiAnimatedImage {
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
    pub text_size_ratio: f32,
    pub font_sizing: UiFontSizing,
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
            text_size_ratio: 0.5,
            font_sizing: UiFontSizing::new(),
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
pub struct UiTextEdit {
    pub base: UiBox,
    pub style: UiStyle,
    pub focused_style: UiStyle,
    pub hover_signals: Vec<SignalID>,
    pub hover_exit_signals: Vec<SignalID>,
    pub focused_signals: Vec<SignalID>,
    pub unfocused_signals: Vec<SignalID>,
    pub text_changed_signals: Vec<SignalID>,
    pub text: Cow<'static, str>,
    pub placeholder: Cow<'static, str>,
    pub color: Color,
    pub placeholder_color: Color,
    pub selection_color: Color,
    pub caret_color: Color,
    pub font_size: f32,
    pub text_size_ratio: f32,
    pub font_sizing: UiFontSizing,
    pub padding: UiRect,
    pub h_scroll: f32,
    pub v_scroll: f32,
    pub caret: usize,
    pub anchor: usize,
    pub editable: bool,
    pub multiline: bool,
}

impl UiTextEdit {
    pub const fn new(multiline: bool) -> Self {
        Self {
            base: UiBox::new(),
            style: UiStyle::panel(),
            focused_style: UiStyle {
                fill: Color::new(0.10, 0.11, 0.13, 0.96),
                stroke: Color::new(0.45, 0.58, 0.85, 1.0),
                stroke_width: 1.0,
                corner_radius: 0.2,
                shadow: UiDepthEffect::none(),
                highlight: UiDepthEffect::none(),
            },
            hover_signals: Vec::new(),
            hover_exit_signals: Vec::new(),
            focused_signals: Vec::new(),
            unfocused_signals: Vec::new(),
            text_changed_signals: Vec::new(),
            text: Cow::Borrowed(""),
            placeholder: Cow::Borrowed(""),
            color: Color::WHITE,
            placeholder_color: Color::new(0.58, 0.62, 0.70, 1.0),
            selection_color: Color::new(0.25, 0.42, 0.85, 0.55),
            caret_color: Color::WHITE,
            font_size: 16.0,
            text_size_ratio: 0.5,
            font_sizing: UiFontSizing::new(),
            padding: UiRect::symmetric(8.0, 6.0),
            h_scroll: 0.0,
            v_scroll: 0.0,
            caret: 0,
            anchor: 0,
            editable: true,
            multiline,
        }
    }

    pub fn set_text<T>(&mut self, text: T)
    where
        T: Into<Cow<'static, str>>,
    {
        self.text = text.into();
        self.caret = clamp_to_char_boundary(self.text.as_ref(), self.caret.min(self.text.len()));
        self.anchor = clamp_to_char_boundary(self.text.as_ref(), self.anchor.min(self.text.len()));
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiFontSizing {
    pub relative_to_virtual: bool,
    pub min_scale: f32,
    pub max_scale: f32,
}

impl UiFontSizing {
    pub const fn new() -> Self {
        Self {
            relative_to_virtual: false,
            min_scale: 0.0,
            max_scale: f32::INFINITY,
        }
    }

    pub fn clamp_scale(self, scale: f32) -> f32 {
        let value = if scale.is_finite() { scale } else { 1.0 };
        let min = if self.min_scale.is_finite() {
            self.min_scale.max(0.0)
        } else {
            0.0
        };
        let mut max = if self.max_scale.is_finite() {
            self.max_scale.max(0.0)
        } else {
            f32::INFINITY
        };
        if max < min {
            max = min;
        }
        value.clamp(min, max)
    }
}

impl Default for UiFontSizing {
    fn default() -> Self {
        Self::new()
    }
}

impl UiNodeBase for UiTextEdit {
    fn ui_base(&self) -> &UiBox {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiBox {
        &mut self.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiTextBox {
    pub inner: UiTextEdit,
}

impl UiTextBox {
    pub const fn new() -> Self {
        Self {
            inner: UiTextEdit::new(false),
        }
    }
}

impl Default for UiTextBox {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiTextBox {
    type Target = UiTextEdit;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for UiTextBox {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl UiNodeBase for UiTextBox {
    fn ui_base(&self) -> &UiBox {
        &self.inner.base
    }

    fn ui_base_mut(&mut self) -> &mut UiBox {
        &mut self.inner.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiTextBlock {
    pub inner: UiTextEdit,
}

impl UiTextBlock {
    pub const fn new() -> Self {
        Self {
            inner: UiTextEdit::new(true),
        }
    }
}

impl Default for UiTextBlock {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiTextBlock {
    type Target = UiTextEdit;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for UiTextBlock {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl UiNodeBase for UiTextBlock {
    fn ui_base(&self) -> &UiBox {
        &self.inner.base
    }

    fn ui_base_mut(&mut self) -> &mut UiBox {
        &mut self.inner.base
    }
}

fn clamp_to_char_boundary(text: &str, mut index: usize) -> usize {
    index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiButton {
    pub base: UiBox,
    pub style: UiStyle,
    pub hover_style: UiStyle,
    pub pressed_style: UiStyle,
    pub cursor_icon: CursorIcon,
    pub hover_base: Option<UiBox>,
    pub pressed_base: Option<UiBox>,
    pub hover_size_override: bool,
    pub pressed_size_override: bool,
    pub hover_signals: Vec<SignalID>,
    pub hover_exit_signals: Vec<SignalID>,
    pub pressed_signals: Vec<SignalID>,
    pub released_signals: Vec<SignalID>,
    pub click_signals: Vec<SignalID>,
    pub disabled: bool,
}

impl UiButton {
    pub const fn new() -> Self {
        Self {
            base: UiBox::new(),
            style: UiStyle::button(),
            hover_style: UiStyle {
                fill: Color::new(0.24, 0.27, 0.32, 1.0),
                stroke: Color::new(0.42, 0.46, 0.54, 1.0),
                stroke_width: 1.0,
                corner_radius: 0.2,
                shadow: UiDepthEffect::none(),
                highlight: UiDepthEffect::none(),
            },
            pressed_style: UiStyle {
                fill: Color::new(0.12, 0.14, 0.18, 1.0),
                stroke: Color::new(0.42, 0.46, 0.54, 1.0),
                stroke_width: 1.0,
                corner_radius: 0.2,
                shadow: UiDepthEffect::none(),
                highlight: UiDepthEffect::none(),
            },
            cursor_icon: CursorIcon::Pointer,
            hover_base: None,
            pressed_base: None,
            hover_size_override: false,
            pressed_size_override: false,
            hover_signals: Vec::new(),
            hover_exit_signals: Vec::new(),
            pressed_signals: Vec::new(),
            released_signals: Vec::new(),
            click_signals: Vec::new(),
            disabled: false,
        }
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
pub struct UiScrollContainer {
    pub base: UiBox,
    pub scroll: Vector2,
}

impl UiScrollContainer {
    pub const fn new() -> Self {
        let mut base = UiBox::new();
        base.clip_children = true;
        Self {
            base,
            scroll: Vector2::ZERO,
        }
    }
}

impl Default for UiScrollContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiScrollContainer {
    type Target = UiBox;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiScrollContainer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiScrollContainer {
    fn ui_base(&self) -> &UiBox {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiBox {
        &mut self.base
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
