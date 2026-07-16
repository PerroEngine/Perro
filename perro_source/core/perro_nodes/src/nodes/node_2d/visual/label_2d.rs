use crate::node_2d::Node2D;
use perro_structs::{Color, Vector2};
use perro_ui::{UiFont, UiTextAlign};
use std::{
    borrow::Cow,
    ops::{Deref, DerefMut},
};

#[derive(Clone, Debug)]
pub struct Label2D {
    pub base: Node2D,
    pub size: Vector2,
    pub text: Cow<'static, str>,
    pub color: Color,
    pub font_size: f32,
    pub font: UiFont,
    pub h_align: UiTextAlign,
    pub v_align: UiTextAlign,
}

impl Label2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            size: Vector2::new(128.0, 32.0),
            text: Cow::Borrowed(""),
            color: Color::WHITE,
            font_size: 20.0,
            font: UiFont::Default,
            h_align: UiTextAlign::Center,
            v_align: UiTextAlign::Center,
        }
    }
}

impl Default for Label2D {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for Label2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Label2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
