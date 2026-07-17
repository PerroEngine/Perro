//! Runtime UI layout, retained command extraction, and text input handling.

use super::state::{DirtyState, UiButtonVisualState};
use super::{Runtime, RuntimeUiTiming};
use ahash::AHashMap;
use perro_ids::{NodeID, SignalID, TextureID};
#[cfg(test)]
use perro_input_api::GamepadAxis;
use perro_input_api::{GamepadButton, JoyConButton, KeyCode, MouseButton, PlayerBinding};
use perro_nodes::{SceneNode, SceneNodeData};
use perro_render_bridge::{
    CameraStreamCommand, CameraStreamSourceState, RenderCommand, ResourceCommand, UiCommand,
    UiCornerRadiiState, UiDepthEffectState, UiFillKindState, UiImageScaleState,
    UiLinearGradientState, UiRectState, UiTextAlignState,
};
use perro_runtime_render::{UiDirtyMask, UiExtractionOptions, ui_image_texture_request};
use perro_structs::{Color, UVector2, Vector2};
use perro_ui::{
    ComputedUiRect, UiAnchor, UiButton, UiDropdownDirection, UiDropdownOpenAnimation, UiFontSizing,
    UiHorizontalAlign, UiImageScaleMode, UiLayoutData, UiLayoutMode, UiLayoutSpacingMode, UiNode,
    UiPanel, UiSizeMode, UiStyle, UiTextBox, UiTextEdit, UiTransform, UiUnit, UiVector2,
    UiVerticalAlign,
};
use perro_variant::Variant;
use std::borrow::Cow;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

#[path = "ui/color_picker.rs"]
mod color_picker;
#[path = "ui/locale.rs"]
mod locale;

use color_picker::*;

const TEXT_EDIT_REPEAT_DELAY: f32 = 0.35;
const TEXT_EDIT_REPEAT_RATE: f32 = 0.035;
const UI_NAV_REPEAT_DELAY: f32 = 0.35;
const UI_NAV_REPEAT_RATE: f32 = 0.15;
const UI_NAV_STICK_ON: f32 = 0.55;
const UI_NAV_STICK_OFF: f32 = 0.35;

#[path = "ui/commands.rs"]
mod commands;
#[path = "ui/interaction.rs"]
mod interaction;

#[path = "ui/events.rs"]
mod events;
#[path = "ui/layout_core.rs"]
mod layout_core;
#[path = "ui/layout_rects.rs"]
mod layout_rects;
#[path = "ui/layout_size.rs"]
mod layout_size;

#[path = "ui/helpers.rs"]
mod helpers;

use helpers::*;

#[cfg(test)]
#[path = "ui/tests.rs"]
mod tests;
