use super::*;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiColorPickerMode {
    SmoothWheel,
    BlockWheel,
    Swatches,
}

impl UiColorPickerMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "smooth" | "smooth_wheel" | "wheel" => Some(Self::SmoothWheel),
            "block" | "blocky" | "block_wheel" => Some(Self::BlockWheel),
            "swatch" | "swatches" | "palette" => Some(Self::Swatches),
            _ => None,
        }
    }
}

pub fn ui_color_picker_swatches() -> [Color; 24] {
    let rgb = |r, g, b| Color::from_rgba_u8([r, g, b, 255]);
    [
        rgb(248, 250, 252),
        rgb(203, 213, 225),
        rgb(100, 116, 139),
        rgb(30, 41, 59),
        rgb(15, 23, 42),
        rgb(2, 6, 23),
        rgb(239, 68, 68),
        rgb(249, 115, 22),
        rgb(245, 158, 11),
        rgb(234, 179, 8),
        rgb(132, 204, 22),
        rgb(34, 197, 94),
        rgb(16, 185, 129),
        rgb(20, 184, 166),
        rgb(6, 182, 212),
        rgb(14, 165, 233),
        rgb(59, 130, 246),
        rgb(99, 102, 241),
        rgb(139, 92, 246),
        rgb(168, 85, 247),
        rgb(217, 70, 239),
        rgb(236, 72, 153),
        rgb(244, 63, 94),
        rgb(120, 53, 15),
    ]
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiColorPicker {
    pub button: UiButton,
    pub color: Color,
    pub popup_open: bool,
    pub popup_style: UiStyle,
    pub popup_size: [f32; 2],
    pub wheel_radius: f32,
    pub picker_mode: UiColorPickerMode,
    pub show_selector: bool,
    pub show_hex: bool,
    pub show_rgba: bool,
    pub show_hsl: bool,
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
            popup_size: [360.0, 344.0],
            wheel_radius: 88.0,
            picker_mode: UiColorPickerMode::SmoothWheel,
            show_selector: true,
            show_hex: true,
            show_rgba: true,
            show_hsl: true,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiDropdownDirection {
    #[default]
    Down,
    Up,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiDropdownOpenAnimation {
    #[default]
    Pop,
    Extend,
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
    /// Pixel size for popup. Zero axes use button width and option-list height.
    pub popup_size: [f32; 2],
    /// Pixel offset from direction-based popup placement.
    pub popup_offset: [f32; 2],
    pub popup_direction: UiDropdownDirection,
    pub open_animation: UiDropdownOpenAnimation,
    pub open_animation_duration: f32,
    pub internal_label: NodeID,
    pub internal_popup_panel: NodeID,
    pub internal_option_buttons: Vec<NodeID>,
    pub internal_option_labels: Vec<NodeID>,
    #[doc(hidden)]
    pub open_animation_progress: f32,
    #[doc(hidden)]
    pub was_open: bool,
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
            popup_size: [0.0, 0.0],
            popup_offset: [0.0, 0.0],
            popup_direction: UiDropdownDirection::Down,
            open_animation: UiDropdownOpenAnimation::Pop,
            open_animation_duration: 0.18,
            internal_label: NodeID::nil(),
            internal_popup_panel: NodeID::nil(),
            internal_option_buttons: Vec::new(),
            internal_option_labels: Vec::new(),
            open_animation_progress: 0.0,
            was_open: false,
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

impl Default for UiNineSliceButton {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiNineSliceButton {
    type Target = UiNode;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiNineSliceButton {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiNineSliceButton {
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.base
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
