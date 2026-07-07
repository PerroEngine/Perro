use crate::node_3d::Node3D;
use perro_ids::TextureID;
use perro_structs::{Color, Vector2};
use perro_ui::UiTextAlign;
use std::{
    borrow::Cow,
    ops::{Deref, DerefMut},
};

#[derive(Clone, Debug)]
pub struct Sprite3D {
    pub base: Node3D,
    pub texture: TextureID,
    pub size: Vector2,
    pub texture_region: Option<[f32; 4]>,
    pub flip_x: bool,
    pub flip_y: bool,
    pub tint: Color,
}

impl Sprite3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            texture: TextureID::nil(),
            size: Vector2::ONE,
            texture_region: None,
            flip_x: false,
            flip_y: false,
            tint: Color::WHITE,
        }
    }
}

impl Default for Sprite3D {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for Sprite3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Sprite3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct Label3D {
    pub base: Node3D,
    pub size: Vector2,
    pub text: Cow<'static, str>,
    pub color: Color,
    pub font_size: f32,
    pub h_align: UiTextAlign,
    pub v_align: UiTextAlign,
}

impl Label3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            size: Vector2::new(1.0, 0.25),
            text: Cow::Borrowed(""),
            color: Color::WHITE,
            font_size: 20.0,
            h_align: UiTextAlign::Center,
            v_align: UiTextAlign::Center,
        }
    }
}

impl Default for Label3D {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for Label3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Label3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
