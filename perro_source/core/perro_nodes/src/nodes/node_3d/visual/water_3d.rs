use crate::{Node3D, WaterSurfaceParams};
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug, Default)]
pub struct WaterBody3D {
    pub base: Node3D,
    pub water: WaterSurfaceParams,
}

impl WaterBody3D {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            water: WaterSurfaceParams {
                size: perro_structs::Vector2::new(128.0, 128.0),
                resolution: [128, 128],
                depth: 12.0,
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
                coastline: crate::CoastlineSettings::new(),
                debug: false,
            },
        }
    }
}

impl Deref for WaterBody3D {
    type Target = Node3D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for WaterBody3D {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}
