use crate::node_2d::Node2D;
use perro_ids::NodeID;
use perro_structs::{AudioEffect, AudioMaterial};
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug)]
pub struct AudioMask2D {
    pub base: Node2D,
    pub enabled: bool,
    pub material: AudioMaterial,
}

impl AudioMask2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            enabled: true,
            material: AudioMaterial::new(),
        }
    }
}

impl Default for AudioMask2D {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for AudioMask2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for AudioMask2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct AudioEffectZone2D {
    pub base: Node2D,
    pub enabled: bool,
    pub audio_mask: perro_structs::BitMask,
    pub bounce: bool,
    pub effects: Vec<AudioEffect>,
}

impl AudioEffectZone2D {
    pub fn new() -> Self {
        Self {
            base: Node2D::new(),
            enabled: true,
            audio_mask: perro_structs::BitMask::NONE,
            bounce: false,
            effects: vec![AudioEffect::new()],
        }
    }
}

impl Default for AudioEffectZone2D {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for AudioEffectZone2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for AudioEffectZone2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct AudioPortal2D {
    pub base: Node2D,
    pub enabled: bool,
    pub targets: Vec<NodeID>,
    pub strength: f32,
}

impl AudioPortal2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            enabled: true,
            targets: Vec::new(),
            strength: 1.0,
        }
    }
}

impl Default for AudioPortal2D {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for AudioPortal2D {
    type Target = Node2D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for AudioPortal2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
