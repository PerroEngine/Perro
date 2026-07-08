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

#[derive(Clone, Debug)]
pub struct Camera2D {
    pub base: Node2D,
    pub zoom: f32,
    pub active: bool,
    pub render_mask: BitMask,
    pub post_processing: PostProcessSet,
    pub audio_options: AudioListenerOptions,
}

impl Default for Camera2D {
    fn default() -> Self {
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

impl Camera2D {
    #[deprecated(note = "use Camera2D::default()")]
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_camera_2d_masks_no_render_layers() {
        assert_eq!(Camera2D::default().render_mask, BitMask::NONE);
    }
}
