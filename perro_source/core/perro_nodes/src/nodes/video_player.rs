use std::{
    borrow::Cow,
    ops::{Deref, DerefMut},
};

use crate::{Node2D, Node3D};
use perro_ids::TextureID;
use perro_structs::{Color, Vector2};
use perro_ui::{UiImageScaleMode, UiNode, UiNodeBase};

#[derive(Clone, Debug, PartialEq)]
pub struct VideoPlayer {
    pub source: Cow<'static, str>,
    pub texture: TextureID,
    pub playing: bool,
    pub looping: bool,
    pub fps_scale: f32,
    pub volume: f32,
}

impl VideoPlayer {
    pub const fn new() -> Self {
        Self {
            source: Cow::Borrowed(""),
            texture: TextureID::nil(),
            playing: true,
            looping: true,
            fps_scale: 1.0,
            volume: 1.0,
        }
    }
}

impl Default for VideoPlayer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct VideoPlayer2D {
    pub base: Node2D,
    pub video: VideoPlayer,
    pub size: Vector2,
    pub tint: Color,
    pub flip_x: bool,
    pub flip_y: bool,
}

impl VideoPlayer2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            video: VideoPlayer::new(),
            size: Vector2::ONE,
            tint: Color::WHITE,
            flip_x: false,
            flip_y: false,
        }
    }
}

impl Default for VideoPlayer2D {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for VideoPlayer2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for VideoPlayer2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct VideoPlayer3D {
    pub base: Node3D,
    pub video: VideoPlayer,
    pub size: Vector2,
    pub tint: Color,
    pub flip_x: bool,
    pub flip_y: bool,
}

impl VideoPlayer3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            video: VideoPlayer::new(),
            size: Vector2::ONE,
            tint: Color::WHITE,
            flip_x: false,
            flip_y: false,
        }
    }
}

impl Default for VideoPlayer3D {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for VideoPlayer3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for VideoPlayer3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct UiVideoPlayer {
    pub base: UiNode,
    pub video: VideoPlayer,
    pub tint: Color,
    pub scale_mode: UiImageScaleMode,
    pub aspect_ratio: f32,
    pub corner_radius: f32,
}

impl UiVideoPlayer {
    pub const fn new() -> Self {
        Self {
            base: UiNode::new(),
            video: VideoPlayer::new(),
            tint: Color::WHITE,
            scale_mode: UiImageScaleMode::Fit,
            aspect_ratio: 1.0,
            corner_radius: 0.0,
        }
    }
}

impl Default for UiVideoPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiVideoPlayer {
    type Target = UiNode;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiVideoPlayer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiVideoPlayer {
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.base
    }
}
