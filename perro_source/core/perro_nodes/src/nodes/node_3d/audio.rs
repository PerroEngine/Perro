use crate::node_3d::Node3D;
use perro_ids::NodeID;
use perro_structs::{AudioEffect, AudioMaterial};
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug)]
pub struct AudioMask3D {
    pub base: Node3D,
    pub enabled: bool,
    pub material: AudioMaterial,
}

impl AudioMask3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            enabled: true,
            material: AudioMaterial::new(),
        }
    }
}

impl Default for AudioMask3D {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for AudioMask3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for AudioMask3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct AudioEffectZone3D {
    pub base: Node3D,
    pub enabled: bool,
    pub audio_mask: perro_structs::BitMask,
    pub bounce: bool,
    pub effects: Vec<AudioEffect>,
}

impl AudioEffectZone3D {
    pub fn new() -> Self {
        Self {
            base: Node3D::new(),
            enabled: true,
            audio_mask: perro_structs::BitMask::NONE,
            bounce: false,
            effects: vec![AudioEffect::new()],
        }
    }
}

impl Default for AudioEffectZone3D {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for AudioEffectZone3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for AudioEffectZone3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug)]
pub struct AudioPortal3D {
    pub base: Node3D,
    pub enabled: bool,
    pub targets: Vec<NodeID>,
    pub strength: f32,
}

impl AudioPortal3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            enabled: true,
            targets: Vec::new(),
            strength: 1.0,
        }
    }
}

impl Default for AudioPortal3D {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for AudioPortal3D {
    type Target = Node3D;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for AudioPortal3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
