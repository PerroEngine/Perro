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

#[derive(Clone, Debug, PartialEq)]
pub struct UiProgressBar {
    pub base: UiNode,
    pub value: f32,
    pub background_style: UiStyle,
    pub fill_style: UiStyle,
}

impl UiProgressBar {
    pub fn new() -> Self {
        let mut fill_style = UiStyle::panel();
        fill_style.fill = Color::WHITE;
        Self {
            base: UiNode::new(),
            value: 0.0,
            background_style: UiStyle::panel(),
            fill_style,
        }
    }

    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(0.0, 1.0);
    }

    pub fn percent(&self) -> f32 {
        self.value.clamp(0.0, 1.0) * 100.0
    }
}

impl Default for UiProgressBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiProgressBar {
    type Target = UiNode;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiProgressBar {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiProgressBar {
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

#[derive(Clone, Debug, PartialEq)]
pub struct UiNineSliceButton {
    pub base: UiNode,
    pub texture: TextureID,
    pub texture_region: Option<[f32; 4]>,
    pub margins: [f32; 4],
    pub tint: Color,
    pub hover_tint: Color,
    pub pressed_tint: Color,
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

impl UiNineSliceButton {
    pub const fn new() -> Self {
        Self {
            base: UiNode::new(),
            texture: TextureID::nil(),
            texture_region: None,
            margins: [8.0, 8.0, 8.0, 8.0],
            tint: Color::WHITE,
            hover_tint: Color::WHITE,
            pressed_tint: Color::WHITE,
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
