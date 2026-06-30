use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct UiPanel {
    pub base: UiNode,
    pub style: UiStyle,
}

impl UiPanel {
    pub const fn new() -> Self {
        Self {
            base: UiNode::new(),
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
    type Target = UiNode;

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
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
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
    pub base: UiNode,
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
            base: UiNode::new(),
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

#[derive(Clone, Debug, PartialEq)]
pub struct UiNineSlice {
    pub base: UiNode,
    pub texture: TextureID,
    pub texture_region: Option<[f32; 4]>,
    pub margins: [f32; 4],
    pub tint: Color,
}

impl UiNineSlice {
    pub const fn new() -> Self {
        Self {
            base: UiNode::new(),
            texture: TextureID::nil(),
            texture_region: None,
            margins: [8.0, 8.0, 8.0, 8.0],
            tint: Color::WHITE,
        }
    }
}

impl Default for UiNineSlice {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiNineSlice {
    type Target = UiNode;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiNineSlice {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiNineSlice {
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.base
    }
}

impl Default for UiImage {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiImageButton {
    pub base: UiNode,
    pub texture: TextureID,
    pub texture_region: Option<[f32; 4]>,
    pub tint: Color,
    pub hover_tint: Color,
    pub pressed_tint: Color,
    pub scale_mode: UiImageScaleMode,
    pub h_align: UiTextAlign,
    pub v_align: UiTextAlign,
    pub aspect_ratio: f32,
    pub input_mask: UiInputMask,
    pub cursor_icon: CursorIcon,
    pub hover_base: Option<UiNode>,
    pub pressed_base: Option<UiNode>,
    pub hover_size_override: bool,
    pub pressed_size_override: bool,
    pub hover_signals: Vec<SignalID>,
    pub hover_exit_signals: Vec<SignalID>,
    pub pressed_signals: Vec<SignalID>,
    pub released_signals: Vec<SignalID>,
    pub clicked_signals: Vec<SignalID>,
    pub web: Option<UiButtonWebAction>,
    pub disabled: bool,
}

impl UiImageButton {
    pub const fn new() -> Self {
        Self {
            base: UiNode::new(),
            texture: TextureID::nil(),
            texture_region: None,
            tint: Color::WHITE,
            hover_tint: Color::WHITE,
            pressed_tint: Color::WHITE,
            scale_mode: UiImageScaleMode::Stretch,
            h_align: UiTextAlign::Center,
            v_align: UiTextAlign::Center,
            aspect_ratio: 0.0,
            input_mask: UiInputMask::new(),
            cursor_icon: CursorIcon::Pointer,
            hover_base: None,
            pressed_base: None,
            hover_size_override: false,
            pressed_size_override: false,
            hover_signals: Vec::new(),
            hover_exit_signals: Vec::new(),
            pressed_signals: Vec::new(),
            released_signals: Vec::new(),
            clicked_signals: Vec::new(),
            web: None,
            disabled: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiCheckbox {
    pub button: UiButton,
    pub checked: bool,
    pub checked_style: UiStyle,
    pub checked_hover_style: UiStyle,
    pub checked_pressed_style: UiStyle,
    pub dot_fill: Color,
}

impl UiCheckbox {
    pub fn new() -> Self {
        let button = UiButton::new();
        let checked_style = button.style.clone();
        let checked_hover_style = button.hover_style.clone();
        let checked_pressed_style = button.pressed_style.clone();
        Self {
            button,
            checked: false,
            checked_style,
            checked_hover_style,
            checked_pressed_style,
            dot_fill: Color::WHITE,
        }
    }
}

impl Default for UiCheckbox {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiCheckbox {
    type Target = UiButton;

    fn deref(&self) -> &Self::Target {
        &self.button
    }
}

impl DerefMut for UiCheckbox {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.button
    }
}

impl UiNodeBase for UiCheckbox {
    fn ui_base(&self) -> &UiNode {
        &self.button.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.button.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiColorPicker {
    pub button: UiButton,
    pub color: Color,
    pub popup_open: bool,
    pub popup_style: UiStyle,
    pub popup_size: [f32; 2],
    pub wheel_radius: f32,
    pub internal_swatch_button: NodeID,
    pub internal_popup_panel: NodeID,
    pub internal_rgba_box: NodeID,
    pub internal_hsv_box: NodeID,
    pub internal_rgba_boxes: [NodeID; 4],
    pub internal_hsv_boxes: [NodeID; 3],
    pub internal_hex_box: NodeID,
    pub color_changed_signals: Vec<SignalID>,
}

impl UiColorPicker {
    pub fn new() -> Self {
        let mut button = UiButton::new();
        button.style.fill = Color::WHITE;
        button.hover_style.fill = Color::LIGHT_GRAY;
        button.pressed_style.fill = Color::GRAY;
        Self {
            button,
            color: Color::WHITE,
            popup_open: false,
            popup_style: UiStyle::panel(),
            popup_size: [260.0, 340.0],
            wheel_radius: 72.0,
            internal_swatch_button: NodeID::nil(),
            internal_popup_panel: NodeID::nil(),
            internal_rgba_box: NodeID::nil(),
            internal_hsv_box: NodeID::nil(),
            internal_rgba_boxes: [NodeID::nil(); 4],
            internal_hsv_boxes: [NodeID::nil(); 3],
            internal_hex_box: NodeID::nil(),
            color_changed_signals: Vec::new(),
        }
    }
}

impl Default for UiColorPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiColorPicker {
    type Target = UiButton;

    fn deref(&self) -> &Self::Target {
        &self.button
    }
}

impl DerefMut for UiColorPicker {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.button
    }
}

impl UiNodeBase for UiColorPicker {
    fn ui_base(&self) -> &UiNode {
        &self.button.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.button.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiDropdownOption {
    pub label: Cow<'static, str>,
    pub value: perro_variant::Variant,
}

impl UiDropdownOption {
    pub fn new(label: impl Into<Cow<'static, str>>, value: perro_variant::Variant) -> Self {
        Self {
            label: label.into(),
            value,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiDropdown {
    pub button: UiButton,
    pub options: Vec<UiDropdownOption>,
    pub selected_index: usize,
    pub open: bool,
    pub popup_style: UiStyle,
    pub option_style: UiStyle,
    pub option_hover_style: UiStyle,
    pub option_pressed_style: UiStyle,
    pub option_height: f32,
    pub internal_label: NodeID,
    pub internal_option_buttons: Vec<NodeID>,
    pub internal_option_labels: Vec<NodeID>,
    pub selected_signals: Vec<SignalID>,
}

impl UiDropdown {
    pub fn new() -> Self {
        Self {
            button: UiButton::new(),
            options: Vec::new(),
            selected_index: 0,
            open: false,
            popup_style: UiStyle::panel(),
            option_style: UiStyle::button(),
            option_hover_style: UiStyle {
                fill: Color::new(0.24, 0.27, 0.32, 1.0),
                stroke: Color::new(0.42, 0.46, 0.54, 1.0),
                ..UiStyle::button()
            },
            option_pressed_style: UiStyle {
                fill: Color::new(0.12, 0.14, 0.18, 1.0),
                stroke: Color::new(0.42, 0.46, 0.54, 1.0),
                ..UiStyle::button()
            },
            option_height: 28.0,
            internal_label: NodeID::nil(),
            internal_option_buttons: Vec::new(),
            internal_option_labels: Vec::new(),
            selected_signals: Vec::new(),
        }
    }

    pub fn selected_label(&self) -> Cow<'static, str> {
        self.options
            .get(self.selected_index)
            .map(|option| option.label.clone())
            .unwrap_or_else(|| Cow::Borrowed(""))
    }
}

impl Default for UiDropdown {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiDropdown {
    type Target = UiButton;

    fn deref(&self) -> &Self::Target {
        &self.button
    }
}

impl DerefMut for UiDropdown {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.button
    }
}

impl UiNodeBase for UiDropdown {
    fn ui_base(&self) -> &UiNode {
        &self.button.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.button.base
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiShapeKind {
    #[default]
    Rect,
    Circle,
    Triangle,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiShape {
    pub base: UiNode,
    pub kind: UiShapeKind,
    pub fill: Color,
    pub stroke: Color,
    pub stroke_width: f32,
}

impl UiShape {
    pub const fn new() -> Self {
        Self {
            base: UiNode::new(),
            kind: UiShapeKind::Rect,
            fill: Color::WHITE,
            stroke: Color::TRANSPARENT,
            stroke_width: 0.0,
        }
    }
}

impl Default for UiShape {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiShape {
    type Target = UiNode;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiShape {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiShape {
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.base
    }
}

impl Default for UiImageButton {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiImageButton {
    type Target = UiNode;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiImageButton {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiImageButton {
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.base
    }
}

impl Deref for UiImage {
    type Target = UiNode;

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
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
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
    pub base: UiNode,
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
            base: UiNode::new(),
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
    type Target = UiNode;

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
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiLabel {
    pub base: UiNode,
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
            base: UiNode::new(),
            text: Cow::Borrowed(""),
            color: Color::WHITE,
            font_size: 20.0,
            text_size_ratio: 0.68,
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
    type Target = UiNode;

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
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.base
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiInputMask {
    pub allow_players: Vec<usize>,
    pub deny_players: Vec<usize>,
    pub allow_gamepads: Vec<usize>,
    pub deny_gamepads: Vec<usize>,
    pub allow_joycons: Vec<usize>,
    pub deny_joycons: Vec<usize>,
    pub allow_kbm: bool,
    pub deny_kbm: bool,
}

impl UiInputMask {
    pub const fn new() -> Self {
        Self {
            allow_players: Vec::new(),
            deny_players: Vec::new(),
            allow_gamepads: Vec::new(),
            deny_gamepads: Vec::new(),
            allow_joycons: Vec::new(),
            deny_joycons: Vec::new(),
            allow_kbm: false,
            deny_kbm: false,
        }
    }

    pub fn has_allow_filter(&self) -> bool {
        self.allow_kbm
            || !self.allow_players.is_empty()
            || !self.allow_gamepads.is_empty()
            || !self.allow_joycons.is_empty()
    }
}

impl Default for UiInputMask {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiTextEdit {
    pub base: UiNode,
    pub style: UiStyle,
    pub focused_style: UiStyle,
    pub input_mask: UiInputMask,
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
    pub h_align: UiTextAlign,
    pub v_align: UiTextAlign,
    pub padding: UiRect,
    pub h_scroll: f32,
    pub v_scroll: f32,
    pub caret: usize,
    pub anchor: usize,
    pub input_type: UiTextInputType,
    pub editable: bool,
    pub multiline: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiTextInputType {
    #[default]
    Any,
    Letters,
    SignedInteger,
    UnsignedInteger,
    SignedFloat,
    UnsignedFloat,
}

impl UiTextEdit {
    pub const fn new(multiline: bool) -> Self {
        Self {
            base: UiNode::new(),
            style: UiStyle::panel(),
            focused_style: UiStyle {
                fill: Color::new(0.10, 0.11, 0.13, 0.96),
                stroke: Color::new(0.45, 0.58, 0.85, 1.0),
                ..UiStyle::panel()
            },
            input_mask: UiInputMask::new(),
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
            font_size: 20.0,
            text_size_ratio: 0.68,
            font_sizing: UiFontSizing::new(),
            h_align: UiTextAlign::Start,
            v_align: UiTextAlign::Center,
            padding: UiRect::symmetric(8.0, 6.0),
            h_scroll: 0.0,
            v_scroll: 0.0,
            caret: 0,
            anchor: 0,
            input_type: UiTextInputType::Any,
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
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
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
    fn ui_base(&self) -> &UiNode {
        &self.inner.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
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
    fn ui_base(&self) -> &UiNode {
        &self.inner.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiButtonWebAction {
    pub href: Cow<'static, str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiButton {
    pub base: UiNode,
    pub style: UiStyle,
    pub hover_style: UiStyle,
    pub pressed_style: UiStyle,
    pub input_mask: UiInputMask,
    pub cursor_icon: CursorIcon,
    pub hover_base: Option<UiNode>,
    pub pressed_base: Option<UiNode>,
    pub hover_size_override: bool,
    pub pressed_size_override: bool,
    pub hover_signals: Vec<SignalID>,
    pub hover_exit_signals: Vec<SignalID>,
    pub pressed_signals: Vec<SignalID>,
    pub released_signals: Vec<SignalID>,
    pub clicked_signals: Vec<SignalID>,
    pub web: Option<UiButtonWebAction>,
    pub disabled: bool,
}

impl UiButton {
    pub const fn new() -> Self {
        Self {
            base: UiNode::new(),
            style: UiStyle::button(),
            hover_style: UiStyle {
                fill: Color::new(0.24, 0.27, 0.32, 1.0),
                stroke: Color::new(0.42, 0.46, 0.54, 1.0),
                ..UiStyle::button()
            },
            pressed_style: UiStyle {
                fill: Color::new(0.12, 0.14, 0.18, 1.0),
                stroke: Color::new(0.42, 0.46, 0.54, 1.0),
                ..UiStyle::button()
            },
            input_mask: UiInputMask::new(),
            cursor_icon: CursorIcon::Pointer,
            hover_base: None,
            pressed_base: None,
            hover_size_override: false,
            pressed_size_override: false,
            hover_signals: Vec::new(),
            hover_exit_signals: Vec::new(),
            pressed_signals: Vec::new(),
            released_signals: Vec::new(),
            clicked_signals: Vec::new(),
            web: None,
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
    type Target = UiNode;

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
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiLayoutContainer {
    pub base: UiNode,
    pub mode: UiLayoutMode,
    pub spacing: f32,
    pub h_spacing: f32,
    pub v_spacing: f32,
    pub spacing_mode: UiLayoutSpacingMode,
    pub h_spacing_mode: UiLayoutSpacingMode,
    pub v_spacing_mode: UiLayoutSpacingMode,
    pub columns: u32,
}

impl UiLayoutContainer {
    pub const fn new(mode: UiLayoutMode) -> Self {
        Self {
            base: UiNode::new(),
            mode,
            spacing: 0.0,
            h_spacing: 0.0,
            v_spacing: 0.0,
            spacing_mode: UiLayoutSpacingMode::Fixed,
            h_spacing_mode: UiLayoutSpacingMode::Fixed,
            v_spacing_mode: UiLayoutSpacingMode::Fixed,
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
    pub base: UiNode,
    pub scroll: Vector2,
    pub scroll_dir: UiScrollDirection,
    pub scroll_bar_side: UiScrollBarSide,
    pub scroll_bar_padding: f32,
}

impl UiScrollContainer {
    pub const fn new() -> Self {
        let mut base = UiNode::new();
        base.clip_children = true;
        Self {
            base,
            scroll: Vector2::ZERO,
            scroll_dir: UiScrollDirection::Vertical,
            scroll_bar_side: UiScrollBarSide::Right,
            scroll_bar_padding: -1.0,
        }
    }
}

impl Default for UiScrollContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiScrollContainer {
    type Target = UiNode;

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
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.base
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiScrollDirection {
    Horizontal,
    #[default]
    Vertical,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiScrollBarSide {
    Left,
    #[default]
    Right,
    Top,
    Bottom,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiFixedLayoutContainer {
    pub base: UiNode,
    pub spacing: f32,
    pub h_spacing: f32,
    pub v_spacing: f32,
    pub spacing_mode: UiLayoutSpacingMode,
    pub h_spacing_mode: UiLayoutSpacingMode,
    pub v_spacing_mode: UiLayoutSpacingMode,
    pub columns: u32,
}

impl UiFixedLayoutContainer {
    pub const fn new() -> Self {
        let mut value = Self {
            base: UiNode::new(),
            spacing: 0.0,
            h_spacing: 0.0,
            v_spacing: 0.0,
            spacing_mode: UiLayoutSpacingMode::Fixed,
            h_spacing_mode: UiLayoutSpacingMode::Fixed,
            v_spacing_mode: UiLayoutSpacingMode::Fixed,
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
    type Target = UiNode;

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
    fn ui_base(&self) -> &UiNode {
        &self.inner.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
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
    type Target = UiNode;

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
    fn ui_base(&self) -> &UiNode {
        &self.inner.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
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
    type Target = UiNode;

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
    fn ui_base(&self) -> &UiNode {
        &self.inner.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.inner.base
    }
}

pub type UiHBox = UiHLayout;
pub type UiVBox = UiVLayout;

#[derive(Clone, Debug, PartialEq)]
pub struct UiGrid {
    pub base: UiNode,
    pub columns: u32,
    pub h_spacing: f32,
    pub v_spacing: f32,
    pub h_spacing_mode: UiLayoutSpacingMode,
    pub v_spacing_mode: UiLayoutSpacingMode,
}

impl UiGrid {
    pub const fn new() -> Self {
        Self {
            base: UiNode::new(),
            columns: 1,
            h_spacing: 0.0,
            v_spacing: 0.0,
            h_spacing_mode: UiLayoutSpacingMode::Fixed,
            v_spacing_mode: UiLayoutSpacingMode::Fixed,
        }
    }
}

impl Default for UiGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiGrid {
    type Target = UiNode;

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
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.base
    }
}
