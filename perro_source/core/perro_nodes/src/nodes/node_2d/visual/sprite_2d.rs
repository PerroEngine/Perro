use std::{
    borrow::Cow,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicUsize, Ordering},
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
    pub flip_x: bool,
    pub flip_y: bool,
}

#[derive(Clone, Debug)]
pub struct NineSlice2D {
    pub base: Node2D,
    pub size: perro_structs::Vector2,
    pub texture: TextureID,
    pub texture_region: Option<[f32; 4]>,
    pub margins: [f32; 4],
    pub tint: perro_structs::Color,
}

impl NineSlice2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            size: perro_structs::Vector2::new(128.0, 48.0),
            texture: TextureID::nil(),
            texture_region: None,
            margins: [8.0, 8.0, 8.0, 8.0],
            tint: perro_structs::Color::WHITE,
        }
    }
}

impl Default for NineSlice2D {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for NineSlice2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for NineSlice2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl Sprite2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            texture: TextureID::nil(),
            texture_region: None,
            flip_x: false,
            flip_y: false,
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

#[derive(Debug)]
pub struct AnimatedSprite2D {
    pub base: Node2D,
    pub texture: TextureID,
    pub animations: Vec<AnimatedSprite>,
    pub flip_x: bool,
    pub flip_y: bool,
    pub current_animation: Cow<'static, str>,
    current_animation_index: AtomicUsize,
    pub current_frame: u32,
    pub fps_scale: f32,
    pub playing: bool,
    pub looping: bool,
    pub frame_accum: f32,
}

impl Clone for AnimatedSprite2D {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
            texture: self.texture,
            animations: self.animations.clone(),
            flip_x: self.flip_x,
            flip_y: self.flip_y,
            current_animation: self.current_animation.clone(),
            current_animation_index: AtomicUsize::new(
                self.current_animation_index.load(Ordering::Relaxed),
            ),
            current_frame: self.current_frame,
            fps_scale: self.fps_scale,
            playing: self.playing,
            looping: self.looping,
            frame_accum: self.frame_accum,
        }
    }
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
            flip_x: false,
            flip_y: false,
            current_animation: Cow::Borrowed("default"),
            current_animation_index: AtomicUsize::new(usize::MAX),
            current_frame: 0,
            fps_scale: 1.0,
            playing: true,
            looping: true,
            frame_accum: 0.0,
        }
    }

    pub fn current_animation_data(&self) -> Option<&AnimatedSprite> {
        let current_name = self.current_animation.as_ref();
        let cached = self.current_animation_index.load(Ordering::Relaxed);
        if cached != usize::MAX
            && let Some(animation) = self.animations.get(cached)
            && animation.name.as_ref() == current_name
        {
            return Some(animation);
        }
        if let Some(index) = self
            .animations
            .iter()
            .position(|animation| animation.name.as_ref() == current_name)
        {
            self.current_animation_index.store(index, Ordering::Relaxed);
            return self.animations.get(index);
        }
        if let Some(animation) = self.animations.first() {
            self.current_animation_index.store(0, Ordering::Relaxed);
            return Some(animation);
        }
        self.current_animation_index
            .store(usize::MAX, Ordering::Relaxed);
        None
    }

    pub fn current_texture_region(&self) -> Option<[f32; 4]> {
        self.current_animation_data()
            .and_then(|animation| animation.texture_region_for_frame(self.current_frame))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_animation_cache_refresh_on_name_chg() {
        let mut sprite = AnimatedSprite2D::new();
        sprite.animations.push(AnimatedSprite::new("idle"));
        sprite.animations.push(AnimatedSprite::new("run"));

        assert_eq!(
            sprite
                .current_animation_data()
                .map(|anim| anim.name.as_ref()),
            Some("idle")
        );

        sprite.current_animation = Cow::Borrowed("run");

        assert_eq!(
            sprite
                .current_animation_data()
                .map(|anim| anim.name.as_ref()),
            Some("run")
        );
    }
}
