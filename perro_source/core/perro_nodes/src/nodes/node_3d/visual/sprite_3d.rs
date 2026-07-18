use crate::node_3d::Node3D;
use perro_ids::TextureID;
use perro_structs::{Color, Vector2};
use perro_ui::{UiCornerRadii, UiRect, UiTextAlign};
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

#[derive(Clone, Debug)]
pub struct Sprite3D {
    pub base: Node3D,
    pub texture: TextureID,
    pub size: Vector2,
    pub texture_region: Option<[f32; 4]>,
    pub flip_x: bool,
    pub flip_y: bool,
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
    pub lock_orientation: bool,
    pub backface_cull: bool,
    pub visible_through_objects: bool,
    pub backdrop_color: Color,
    pub corner_radii: UiCornerRadii,
    pub padding: UiRect,
    pub text: Arc<str>,
    pub color: Color,
    pub font_size: f32,
    pub font: perro_ui::UiFont,
    pub h_align: UiTextAlign,
    pub v_align: UiTextAlign,
}

impl Label3D {
    pub fn new() -> Self {
        Self {
            base: Node3D::new(),
            size: Vector2::new(1.0, 0.25),
            lock_orientation: false,
            backface_cull: true,
            visible_through_objects: false,
            backdrop_color: Color::TRANSPARENT,
            corner_radii: UiCornerRadii::zero(),
            padding: UiRect::ZERO,
            text: Arc::from(""),
            color: Color::WHITE,
            font_size: 20.0,
            font: perro_ui::UiFont::Default,
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
