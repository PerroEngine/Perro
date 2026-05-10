use std::{
    borrow::Cow,
    ops::{Deref, DerefMut},
};

use crate::node_2d::Node2D;
use perro_ids::TextureID;

impl Deref for Sprite2D {
    type Target = Node2D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Sprite2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug, Default)]
pub struct Sprite2D {
    pub base: Node2D,
    pub texture: TextureID,
    pub texture_region: Option<[f32; 4]>,
}

impl Sprite2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            texture: TextureID::nil(),
            texture_region: None,
        }
    }
}

impl Deref for AnimatedSprite2D {
    type Target = Node2D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for AnimatedSprite2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct AnimatedSprite {
    pub name: Cow<'static, str>,
    pub start: [f32; 2],
    pub frame_size: [f32; 2],
    pub frame_count: u32,
    pub columns: u32,
    pub fps: f32,
}

impl Default for AnimatedSprite {
    fn default() -> Self {
        Self::new("default")
    }
}

impl AnimatedSprite {
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
        let columns = self.columns;
        let (column, row) = if columns > 0 {
            (frame % columns, frame / columns)
        } else {
            (frame, 0)
        };
        let [base_x, base_y] = self.start;

        Some([base_x + column as f32 * w, base_y + row as f32 * h, w, h])
    }
}

#[derive(Clone, Debug)]
pub struct AnimatedSprite2D {
    pub base: Node2D,
    pub texture: TextureID,
    pub animations: Vec<AnimatedSprite>,
    pub current_animation: Cow<'static, str>,
    pub current_frame: u32,
    pub fps_scale: f32,
    pub playing: bool,
    pub looping: bool,
    pub frame_accum: f32,
}

impl Default for AnimatedSprite2D {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimatedSprite2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            texture: TextureID::nil(),
            animations: Vec::new(),
            current_animation: Cow::Borrowed("default"),
            current_frame: 0,
            fps_scale: 1.0,
            playing: true,
            looping: true,
            frame_accum: 0.0,
        }
    }

    pub fn current_animation_data(&self) -> Option<&AnimatedSprite> {
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
