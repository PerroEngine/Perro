use crate::{Node2D, WaterSurfaceParams};
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug, Default)]
pub struct WaterBody2D {
    pub base: Node2D,
    pub water: WaterSurfaceParams,
}

impl WaterBody2D {
    pub const fn new() -> Self {
        Self {
            base: Node2D::new(),
            water: WaterSurfaceParams {
                shape: crate::WaterShape::rect(perro_structs::Vector2::new(32.0, 32.0)),
                quality: perro_structs::WaterQuality::Low,
                depth: 4.0,
                flow: perro_structs::Vector2::ZERO,
                wind: perro_structs::Vector2::new(1.0, 0.0),
                idle_mode: crate::WaterIdleMode::Calm,
                wave: crate::WaterWaveProfile {
                    speed: 1.0,
                    scale: 1.0,
                    length: 18.0,
                    damping: 0.985,
                },
                physics: crate::WaterPhysicsParams::for_quality(perro_structs::WaterQuality::Low),
                collision_layers: crate::BitMask::ALL,
                collision_mask: crate::BitMask::NONE,
                link: crate::WaterLinkParams {
                    link_layers: crate::BitMask::ALL,
                    link_mask: crate::BitMask::NONE,
                    blend_width: 0.0,
                    wave_transfer: 1.0,
                    flow_transfer: 1.0,
                },
                optics: crate::WaterOpticsSettings::new(),
                visual: crate::WaterVisualParams::new(),
                coastline: crate::CoastlineSettings::new(),
                debug: false,
            },
        }
    }
}

impl Deref for WaterBody2D {
    type Target = Node2D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for WaterBody2D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
