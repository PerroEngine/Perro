use std::{
    borrow::Cow,
    ops::{Deref, DerefMut},
};

use crate::node_2d::Node2D;
use perro_ids::{SignalID, TextureID};
use perro_structs::{Color, Vector2};
use perro_ui::{CursorIcon, UiButtonWebAction, UiInputMask, UiMouseFilter, UiStyle};

#[derive(Clone, Debug)]
pub struct Button2D {
    pub base: Node2D,
    pub size: Vector2,
    pub style: UiStyle,
    pub hover_style: UiStyle,
    pub pressed_style: UiStyle,
    pub input_mask: UiInputMask,
    pub mouse_filter: UiMouseFilter,
    pub cursor_icon: CursorIcon,
    pub input_enabled: bool,
    pub clicked_signals: Vec<SignalID>,
    pub hover_signals: Vec<SignalID>,
    pub hover_exit_signals: Vec<SignalID>,
    pub pressed_signals: Vec<SignalID>,
    pub released_signals: Vec<SignalID>,
    pub web: Option<UiButtonWebAction>,
}

impl Button2D {
    #[deprecated(note = "use Button2D::default()")]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for Button2D {
    fn default() -> Self {
        Self {
            base: Node2D::new(),
            size: Vector2::new(128.0, 48.0),
            style: UiStyle::button(),
            hover_style: UiStyle {
                fill: Color::new(0.24, 0.27, 0.32, 1.0),
                ..UiStyle::button()
            },
            pressed_style: UiStyle {
                fill: Color::new(0.12, 0.14, 0.18, 1.0),
                ..UiStyle::button()
            },
            input_mask: UiInputMask::default(),
            mouse_filter: UiMouseFilter::Stop,
            cursor_icon: CursorIcon::Pointer,
            input_enabled: true,
            clicked_signals: Vec::new(),
            hover_signals: Vec::new(),
            hover_exit_signals: Vec::new(),
            pressed_signals: Vec::new(),
            released_signals: Vec::new(),
            web: None,
        }
    }
}

impl Deref for Button2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Button2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct ImageButton2D {
    pub base: Node2D,
    pub size: Vector2,
    pub texture: TextureID,
    pub texture_region: Option<[f32; 4]>,
    pub tint: Color,
    pub hover_tint: Color,
    pub pressed_tint: Color,
    pub input_mask: UiInputMask,
    pub mouse_filter: UiMouseFilter,
    pub cursor_icon: CursorIcon,
    pub input_enabled: bool,
    pub clicked_signals: Vec<SignalID>,
    pub hover_signals: Vec<SignalID>,
    pub hover_exit_signals: Vec<SignalID>,
    pub pressed_signals: Vec<SignalID>,
    pub released_signals: Vec<SignalID>,
    pub web: Option<UiButtonWebAction>,
    pub label: Cow<'static, str>,
}

#[derive(Clone, Debug)]
pub struct NineSliceButton2D {
    pub base: Node2D,
    pub size: Vector2,
    pub texture: TextureID,
    pub texture_region: Option<[f32; 4]>,
    pub margins: [f32; 4],
    pub tint: Color,
    pub hover_tint: Color,
    pub pressed_tint: Color,
    pub input_mask: UiInputMask,
    pub mouse_filter: UiMouseFilter,
    pub cursor_icon: CursorIcon,
    pub input_enabled: bool,
    pub clicked_signals: Vec<SignalID>,
    pub hover_signals: Vec<SignalID>,
    pub hover_exit_signals: Vec<SignalID>,
    pub pressed_signals: Vec<SignalID>,
    pub released_signals: Vec<SignalID>,
    pub web: Option<UiButtonWebAction>,
    pub label: Cow<'static, str>,
}

impl ImageButton2D {
    #[deprecated(note = "use ImageButton2D::default()")]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for ImageButton2D {
    fn default() -> Self {
        Self {
            base: Node2D::new(),
            size: Vector2::new(64.0, 64.0),
            texture: TextureID::nil(),
            texture_region: None,
            tint: Color::WHITE,
            hover_tint: Color::new(1.0, 1.0, 1.0, 0.9),
            pressed_tint: Color::new(0.8, 0.8, 0.8, 1.0),
            input_mask: UiInputMask::default(),
            mouse_filter: UiMouseFilter::Stop,
            cursor_icon: CursorIcon::Pointer,
            input_enabled: true,
            clicked_signals: Vec::new(),
            hover_signals: Vec::new(),
            hover_exit_signals: Vec::new(),
            pressed_signals: Vec::new(),
            released_signals: Vec::new(),
            web: None,
            label: Cow::Borrowed(""),
        }
    }
}

impl Default for NineSliceButton2D {
    fn default() -> Self {
        Self {
            base: Node2D::new(),
            size: Vector2::new(128.0, 48.0),
            texture: TextureID::nil(),
            texture_region: None,
            margins: [8.0, 8.0, 8.0, 8.0],
            tint: Color::WHITE,
            hover_tint: Color::new(1.0, 1.0, 1.0, 0.9),
            pressed_tint: Color::new(0.8, 0.8, 0.8, 1.0),
            input_mask: UiInputMask::default(),
            mouse_filter: UiMouseFilter::Stop,
            cursor_icon: CursorIcon::Pointer,
            input_enabled: true,
            clicked_signals: Vec::new(),
            hover_signals: Vec::new(),
            hover_exit_signals: Vec::new(),
            pressed_signals: Vec::new(),
            released_signals: Vec::new(),
            web: None,
            label: Cow::Borrowed(""),
        }
    }
}

impl Deref for ImageButton2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for ImageButton2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl Deref for NineSliceButton2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for NineSliceButton2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_ui::CursorIcon;

    #[test]
    fn button_2d_defaults_to_pointer_cursor() {
        assert_eq!(Button2D::default().cursor_icon, CursorIcon::Pointer);
    }

    #[test]
    fn image_button_2d_defaults_to_pointer_cursor() {
        assert_eq!(ImageButton2D::default().cursor_icon, CursorIcon::Pointer);
    }
}
