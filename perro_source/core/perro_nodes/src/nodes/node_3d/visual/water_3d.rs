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
                shape: crate::WaterShape::box_volume(perro_structs::Vector3::new(
                    500.0, 35.0, 500.0,
                )),
                resolution: [4096, 4096],
                render_resolution: [4096, 4096],
                depth: 35.0,
                flow: perro_structs::Vector2::new(0.10, 0.03),
                wind: perro_structs::Vector2::new(0.8, 0.2),
                idle_mode: crate::WaterIdleMode::Chop,
                wave: crate::WaterWaveProfile {
                    speed: 0.38,
                    scale: 0.55,
                    length: 22.0,
                    damping: 0.988,
                },
                physics: crate::WaterPhysicsParams {
                    buoyancy: 1.1,
                    drag: 0.36,
                    wake_strength: 1.20,
                    foam_strength: 0.72,
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
                optics: crate::WaterOpticsSettings {
                    deep_color: perro_structs::Color::new(0.005, 0.035, 0.15, 0.99),
                    shallow_color: perro_structs::Color::new(0.04, 0.25, 0.34, 0.95),
                    shallow_depth: -1.0,
                    sky_bias: crate::WaterSkyBias::None,
                },
                visual: crate::WaterVisualParams::new(),
                coastline: crate::CoastlineSettings {
                    foam_color: perro_structs::Color::new(0.68, 0.86, 0.92, 0.72),
                    foam_strength: 0.52,
                    foam_width: 1.65,
                    cutoff_softness: 0.45,
                    wave_reflection: 0.42,
                    wave_damping: 0.48,
                    edge_noise: 0.06,
                },
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
