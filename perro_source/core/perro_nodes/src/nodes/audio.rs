use crate::{node_2d::Node2D, node_3d::Node3D};
use perro_ids::NodeID;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioMaterial {
    pub absorption: f32,
    pub reflection: f32,
    pub transmission: f32,
    pub diffusion: f32,
    pub low_pass_strength: f32,
    pub thickness_multiplier: f32,
    pub occlusion_mask: u32,
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
            occlusion_mask: u32::MAX,
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
pub struct AudioZoneEffect {
    pub reverb_send: f32,
    pub echo: f32,
    pub dampening: f32,
}

impl AudioZoneEffect {
    pub const fn new() -> Self {
        Self {
            reverb_send: 0.35,
            echo: 0.0,
            dampening: 0.0,
        }
    }
}

impl Default for AudioZoneEffect {
    fn default() -> Self {
        Self::new()
    }
}

macro_rules! define_audio_node_2d {
    ($name:ident { $($field:ident : $ty:ty = $value:expr),* $(,)? }) => {
        #[derive(Clone, Debug)]
        pub struct $name {
            pub base: Node2D,
            $(pub $field: $ty,)*
        }

        impl $name {
            pub const fn new() -> Self {
                Self {
                    base: Node2D::new(),
                    $($field: $value,)*
                }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl Deref for $name {
            type Target = Node2D;

            fn deref(&self) -> &Self::Target {
                &self.base
            }
        }

        impl DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.base
            }
        }
    };
}

macro_rules! define_audio_node_3d {
    ($name:ident { $($field:ident : $ty:ty = $value:expr),* $(,)? }) => {
        #[derive(Clone, Debug)]
        pub struct $name {
            pub base: Node3D,
            $(pub $field: $ty,)*
        }

        impl $name {
            pub const fn new() -> Self {
                Self {
                    base: Node3D::new(),
                    $($field: $value,)*
                }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl Deref for $name {
            type Target = Node3D;

            fn deref(&self) -> &Self::Target {
                &self.base
            }
        }

        impl DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.base
            }
        }
    };
}

define_audio_node_2d!(AudioMask2D {
    enabled: bool = true,
    material: AudioMaterial = AudioMaterial::new(),
});

define_audio_node_3d!(AudioMask3D {
    enabled: bool = true,
    material: AudioMaterial = AudioMaterial::new(),
});

define_audio_node_2d!(AudioZone2D {
    enabled: bool = true,
    effect: AudioZoneEffect = AudioZoneEffect::new(),
    affect_listener: bool = true,
    affect_emitters: bool = true,
    affect_path: bool = true,
});

define_audio_node_3d!(AudioZone3D {
    enabled: bool = true,
    effect: AudioZoneEffect = AudioZoneEffect::new(),
    affect_listener: bool = true,
    affect_emitters: bool = true,
    affect_path: bool = true,
});

define_audio_node_2d!(AudioPortal2D {
    enabled: bool = true,
    target: NodeID = NodeID::nil(),
    strength: f32 = 1.0,
});

define_audio_node_3d!(AudioPortal3D {
    enabled: bool = true,
    target: NodeID = NodeID::nil(),
    strength: f32 = 1.0,
});
