use crate::BitMask;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioMaterial {
    pub absorption: f32,
    pub reflection: f32,
    pub transmission: f32,
    pub diffusion: f32,
    pub low_pass_strength: f32,
    pub thickness_multiplier: f32,
    pub audio_mask: BitMask,
}

impl AudioMaterial {
    pub const fn new() -> Self {
        Self {
            absorption: 0.35,
            reflection: 0.35,
            transmission: 0.15,
            diffusion: 0.15,
            low_pass_strength: 0.5,
            thickness_multiplier: 1.0,
            audio_mask: BitMask::NONE,
        }
    }
}

impl Default for AudioMaterial {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioDiffusion {
    pub damping: f32,
    pub compression: f32,
    pub hardness: f32,
}

impl AudioDiffusion {
    pub const fn new() -> Self {
        Self {
            damping: 0.35,
            compression: 0.15,
            hardness: 0.5,
        }
    }
}

impl Default for AudioDiffusion {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioInteraction {
    pub material: AudioMaterial,
    pub diffusion: AudioDiffusion,
}

impl AudioInteraction {
    pub const fn new() -> Self {
        Self {
            material: AudioMaterial::new(),
            diffusion: AudioDiffusion::new(),
        }
    }
}

impl Default for AudioInteraction {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioEffect {
    pub reverb_send: f32,
    pub echo: f32,
    pub dampening: f32,
}

impl AudioEffect {
    pub const fn new() -> Self {
        Self {
            reverb_send: 0.35,
            echo: 0.0,
            dampening: 0.0,
        }
    }
}

impl Default for AudioEffect {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioListenerOptions {
    pub audio_mask: BitMask,
    pub effects: Vec<AudioEffect>,
}

impl AudioListenerOptions {
    pub const fn new() -> Self {
        Self {
            audio_mask: BitMask::NONE,
            effects: Vec::new(),
        }
    }
}

impl Default for AudioListenerOptions {
    fn default() -> Self {
        Self::new()
    }
}
