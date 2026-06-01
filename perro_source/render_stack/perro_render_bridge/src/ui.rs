use super::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiRectState {
    pub center: [f32; 2],
    pub size: [f32; 2],
    pub pivot: [f32; 2],
    pub rotation_radians: f32,
    pub z_index: i32,
}

impl UiRectState {
    pub fn screen_min_max(self, viewport: [f32; 2]) -> ([f32; 2], [f32; 2]) {
        let screen_center = [viewport[0] * 0.5, viewport[1] * 0.5];
        let center = [
            screen_center[0] + self.center[0],
            screen_center[1] - self.center[1],
        ];
        let half = [self.size[0] * 0.5, self.size[1] * 0.5];
        (
            [center[0] - half[0], center[1] - half[1]],
            [center[0] + half[0], center[1] + half[1]],
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiDepthEffectState {
    pub color: Color,
    pub distance: f32,
    pub falloff: f32,
    pub vector: [f32; 2],
    pub size: f32,
}

impl UiDepthEffectState {
    pub const fn none() -> Self {
        Self {
            color: Color::TRANSPARENT,
            distance: 0.0,
            falloff: 0.0,
            vector: [0.0, -1.0],
            size: 1.0,
        }
    }
}

impl Default for UiDepthEffectState {
    fn default() -> Self {
        Self::none()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiCommand {
    UpsertPanel {
        node: NodeID,
        rect: UiRectState,
        clip_rect: [f32; 4],
        fill: [f32; 4],
        stroke: [f32; 4],
        stroke_width: f32,
        corner_radius: f32,
        shadow: UiDepthEffectState,
        highlight: UiDepthEffectState,
    },
    UpsertButton {
        node: NodeID,
        rect: UiRectState,
        clip_rect: [f32; 4],
        fill: [f32; 4],
        stroke: [f32; 4],
        stroke_width: f32,
        corner_radius: f32,
        shadow: UiDepthEffectState,
        highlight: UiDepthEffectState,
        disabled: bool,
    },
    UpsertLabel {
        node: NodeID,
        rect: UiRectState,
        clip_rect: [f32; 4],
        text: Cow<'static, str>,
        color: Color,
        font_size: f32,
        h_align: UiTextAlignState,
        v_align: UiTextAlignState,
    },
    UpsertImage {
        node: NodeID,
        rect: UiRectState,
        clip_rect: [f32; 4],
        texture: TextureID,
        tint: Color,
        uv_min: [f32; 2],
        uv_max: [f32; 2],
        scale_mode: UiImageScaleState,
        h_align: UiTextAlignState,
        v_align: UiTextAlignState,
        aspect_ratio: f32,
    },
    UpsertNineSlice {
        node: NodeID,
        rect: UiRectState,
        clip_rect: [f32; 4],
        texture: TextureID,
        tint: Color,
        uv_min: [f32; 2],
        uv_max: [f32; 2],
        margins: [f32; 4],
    },
    UpsertTextEdit {
        node: NodeID,
        rect: UiRectState,
        clip_rect: [f32; 4],
        fill: [f32; 4],
        stroke: [f32; 4],
        stroke_width: f32,
        corner_radius: f32,
        shadow: UiDepthEffectState,
        highlight: UiDepthEffectState,
        text: Cow<'static, str>,
        placeholder: Cow<'static, str>,
        color: Color,
        placeholder_color: Color,
        selection_color: Color,
        caret_color: Color,
        font_size: f32,
        padding: [f32; 4],
        scroll: [f32; 2],
        caret: usize,
        anchor: usize,
        focused: bool,
        multiline: bool,
    },
    RemoveNode {
        node: NodeID,
    },
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum UiTextAlignState {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum UiImageScaleState {
    #[default]
    Stretch,
    Fit,
    Cover,
}
