use perro_structs::Vector2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WaterIdleMode {
    #[default]
    Calm,
    Sine,
    Chop,
    Storm,
    River,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterWaveProfile {
    pub speed: f32,
    pub scale: f32,
    pub damping: f32,
}

impl Default for WaterWaveProfile {
    fn default() -> Self {
        Self {
            speed: 1.0,
            scale: 1.0,
            damping: 0.985,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterPhysicsParams {
    pub buoyancy: f32,
    pub drag: f32,
    pub wake_strength: f32,
    pub foam_strength: f32,
    pub sample_readback_rate: f32,
}

impl Default for WaterPhysicsParams {
    fn default() -> Self {
        Self {
            buoyancy: 1.0,
            drag: 0.35,
            wake_strength: 1.0,
            foam_strength: 0.65,
            sample_readback_rate: 30.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterSurfaceParams {
    pub size: Vector2,
    pub resolution: [u32; 2],
    pub depth: f32,
    pub flow: Vector2,
    pub wind: Vector2,
    pub idle_mode: WaterIdleMode,
    pub wave: WaterWaveProfile,
    pub physics: WaterPhysicsParams,
    pub shoreline_mask: bool,
    pub static_body_wakes: bool,
    pub debug: bool,
}

impl Default for WaterSurfaceParams {
    fn default() -> Self {
        Self {
            size: Vector2::new(32.0, 32.0),
            resolution: [128, 128],
            depth: 4.0,
            flow: Vector2::ZERO,
            wind: Vector2::new(1.0, 0.0),
            idle_mode: WaterIdleMode::Calm,
            wave: WaterWaveProfile::default(),
            physics: WaterPhysicsParams::default(),
            shoreline_mask: false,
            static_body_wakes: true,
            debug: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterPhysicsSample {
    pub height: f32,
    pub velocity: Vector2,
    pub foam: f32,
}

impl Default for WaterPhysicsSample {
    fn default() -> Self {
        Self {
            height: 0.0,
            velocity: Vector2::ZERO,
            foam: 0.0,
        }
    }
}

#[inline]
pub fn water_impact_strength(mass: f32, velocity: Vector2, wake_strength: f32) -> f32 {
    let mass = mass.max(0.0);
    let speed = velocity.length();
    (mass.sqrt() * speed * wake_strength.max(0.0)).min(256.0)
}

#[inline]
pub fn analytic_idle_water_height(
    surface: &WaterSurfaceParams,
    local: Vector2,
    time_seconds: f32,
) -> f32 {
    let phase = time_seconds * surface.wave.speed;
    match surface.idle_mode {
        WaterIdleMode::Calm => 0.0,
        WaterIdleMode::Sine => {
            let wind = surface.wind.normalized();
            (local.dot(wind) * 0.125 + phase).sin() * surface.wave.scale
        }
        WaterIdleMode::Chop => {
            let a = (local.x * 0.17 + phase).sin();
            let b = (local.y * 0.11 + phase * 1.37).cos();
            (a * 0.7 + b * 0.3) * surface.wave.scale
        }
        WaterIdleMode::Storm => {
            let a = (local.x * 0.23 + phase * 1.8).sin();
            let b = ((local.x + local.y) * 0.07 - phase * 1.2).cos();
            (a * 0.55 + b * 0.45) * surface.wave.scale * 1.8
        }
        WaterIdleMode::River => {
            let flow = surface.flow.normalized();
            (local.dot(flow) * 0.2 - phase * 1.5).sin() * surface.wave.scale * 0.45
        }
    }
}

#[inline]
pub fn water_physics_sample_or_idle(
    surface: &WaterSurfaceParams,
    local: Vector2,
    time_seconds: f32,
    gpu_sample: Option<WaterPhysicsSample>,
) -> WaterPhysicsSample {
    gpu_sample.unwrap_or(WaterPhysicsSample {
        height: analytic_idle_water_height(surface, local, time_seconds),
        velocity: surface.flow,
        foam: 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impact_strength_uses_mass_velocity_and_wake() {
        let strength = water_impact_strength(9.0, Vector2::new(3.0, 4.0), 2.0);
        assert_eq!(strength, 30.0);
        assert_eq!(
            water_impact_strength(-1.0, Vector2::new(3.0, 4.0), 2.0),
            0.0
        );
        assert_eq!(
            water_impact_strength(9.0, Vector2::new(3.0, 4.0), -2.0),
            0.0
        );
    }

    #[test]
    fn sample_prefers_gpu_cache_and_falls_back_to_idle() {
        let mut surface = WaterSurfaceParams::default();
        surface.idle_mode = WaterIdleMode::Sine;
        surface.wave.scale = 2.0;
        surface.flow = Vector2::new(1.0, 0.0);

        let cached = WaterPhysicsSample {
            height: 4.0,
            velocity: Vector2::new(0.5, 0.25),
            foam: 0.75,
        };
        assert_eq!(
            water_physics_sample_or_idle(&surface, Vector2::new(2.0, 0.0), 1.0, Some(cached)),
            cached
        );

        let fallback = water_physics_sample_or_idle(&surface, Vector2::new(2.0, 0.0), 1.0, None);
        assert_ne!(fallback.height, 4.0);
        assert_eq!(fallback.velocity, surface.flow);
        assert_eq!(fallback.foam, 0.0);
    }
}
