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
                size: perro_structs::Vector2::new(32.0, 32.0),
                shape: crate::WaterShape::rect(perro_structs::Vector2::new(32.0, 32.0)),
                resolution: [128, 128],
                depth: 4.0,
                flow: perro_structs::Vector2::ZERO,
                wind: perro_structs::Vector2::new(1.0, 0.0),
                idle_mode: crate::WaterIdleMode::Calm,
                wave: crate::WaterWaveProfile {
                    speed: 1.0,
                    scale: 1.0,
                    damping: 0.985,
                },
                physics: crate::WaterPhysicsParams {
                    buoyancy: 1.0,
                    drag: 0.35,
                    wake_strength: 1.0,
                    foam_strength: 0.65,
                    sample_readback_rate: 30.0,
                },
                lod: crate::WaterLodParams {
                    near_distance: 128.0,
                    mid_distance: 384.0,
                    far_distance: 896.0,
                    min_resolution: [32, 32],
                },
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
