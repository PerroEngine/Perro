use std::ops::{Deref, DerefMut};

use crate::Node2D;
use perro_structs::{AudioListenerOptions, BitMask, PostProcessSet};

impl Deref for Camera2D {
    type Target = Node2D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for Camera2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug, Default)]
pub struct Camera2D {
    pub base: Node2D,
    pub zoom: f32,
    pub active: bool,
    pub render_mask: BitMask,
    pub post_processing: PostProcessSet,
    pub audio_options: AudioListenerOptions,
}

impl Camera2D {
    pub fn new() -> Self {
        Self {
            base: Node2D::new(),
            zoom: 0f32,
            active: false,
            render_mask: BitMask::NONE,
            post_processing: PostProcessSet::new(),
            audio_options: AudioListenerOptions::new(),
        }
    }
}
