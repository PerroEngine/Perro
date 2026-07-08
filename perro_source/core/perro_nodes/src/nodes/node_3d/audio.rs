use crate::node_3d::Node3D;
use perro_ids::NodeID;
use perro_structs::{AudioEffect, AudioMaterial};
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug)]
pub struct AudioMask3D {
    pub base: Node3D,
    pub active: bool,
    pub material: AudioMaterial,
}

impl AudioMask3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            active: true,
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
    pub active: bool,
    pub audio_mask: perro_structs::BitMask,
    pub bounce: bool,
    pub effects: Vec<AudioEffect>,
}

impl AudioEffectZone3D {
    #[deprecated(note = "use AudioEffectZone3D::default()")]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for AudioEffectZone3D {
    fn default() -> Self {
        Self {
            base: Node3D::new(),
            active: true,
            audio_mask: perro_structs::BitMask::NONE,
            bounce: false,
            effects: vec![AudioEffect::new()],
        }
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
    pub active: bool,
    pub targets: Vec<NodeID>,
    pub strength: f32,
}

impl AudioPortal3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            active: true,
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
