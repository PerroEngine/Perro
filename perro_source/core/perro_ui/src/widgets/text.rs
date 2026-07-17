use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct UiLabel {
    pub base: UiNode,
    pub text: Cow<'static, str>,
    pub color: Color,
    pub font_size: f32,
    pub font: UiFont,
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
            font: UiFont::Default,
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
    pub font: UiFont,
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
            font: UiFont::Default,
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

pub(super) fn clamp_to_char_boundary(text: &str, mut index: usize) -> usize {
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
