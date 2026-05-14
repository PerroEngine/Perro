use perro_structs::{BitMask, Color, Vector2, Vector3};

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
            wake_strength: 1.35,
            foam_strength: 0.9,
            sample_readback_rate: 30.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterLinkParams {
    pub link_layers: BitMask,
    pub link_mask: BitMask,
    pub blend_width: f32,
    pub wave_transfer: f32,
    pub flow_transfer: f32,
}

impl Default for WaterLinkParams {
    fn default() -> Self {
        Self {
            link_layers: BitMask::ALL,
            link_mask: BitMask::NONE,
            blend_width: 0.0,
            wave_transfer: 1.0,
            flow_transfer: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterLodParams {
    pub near_distance: f32,
    pub mid_distance: f32,
    pub far_distance: f32,
    pub min_resolution: [u32; 2],
}

impl Default for WaterLodParams {
    fn default() -> Self {
        Self {
            near_distance: 128.0,
            mid_distance: 384.0,
            far_distance: 896.0,
            min_resolution: [32, 32],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoastlineSettings {
    pub foam_color: Color,
    pub foam_strength: f32,
    pub foam_width: f32,
    pub cutoff_softness: f32,
    pub wave_reflection: f32,
    pub wave_damping: f32,
    pub edge_noise: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum WaterSkyBias {
    #[default]
    None,
    Active {
        ratio: f32,
    },
}

impl WaterSkyBias {
    pub const fn ratio(self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Active { ratio } => ratio,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterOpticsSettings {
    pub deep_color: Color,
    pub shallow_color: Color,
    pub shallow_depth: f32,
    pub sky_bias: WaterSkyBias,
}

impl WaterOpticsSettings {
    pub const fn new() -> Self {
        Self {
            deep_color: Color::new(0.02, 0.16, 0.28, 0.94),
            shallow_color: Color::new(0.08, 0.46, 0.62, 0.74),
            shallow_depth: -1.0,
            sky_bias: WaterSkyBias::None,
        }
    }
}

impl Default for WaterOpticsSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl CoastlineSettings {
    pub const fn new() -> Self {
        Self {
            foam_color: Color::new(0.9, 0.97, 1.0, 1.0),
            foam_strength: 0.75,
            foam_width: 1.5,
            cutoff_softness: 0.25,
            wave_reflection: 0.45,
            wave_damping: 0.35,
            edge_noise: 0.2,
        }
    }
}

impl Default for CoastlineSettings {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WaterShape {
    Rect { size: Vector2 },
    Circle { radius: f32 },
    Box { size: Vector3 },
    Cylinder { radius: f32, half_height: f32 },
}

impl WaterShape {
    pub const fn rect(size: Vector2) -> Self {
        Self::Rect { size }
    }

    pub const fn box_volume(size: Vector3) -> Self {
        Self::Box { size }
    }

    pub fn surface_size(self) -> Vector2 {
        match self {
            Self::Rect { size } => size,
            Self::Circle { radius } => Vector2::new(radius * 2.0, radius * 2.0),
            Self::Box { size } => Vector2::new(size.x, size.z),
            Self::Cylinder { radius, .. } => Vector2::new(radius * 2.0, radius * 2.0),
        }
    }

    pub fn depth(self, fallback: f32) -> f32 {
        match self {
            Self::Box { size } => size.y.max(0.0),
            Self::Cylinder { half_height, .. } => (half_height * 2.0).max(0.0),
            _ => fallback,
        }
    }

    pub fn contains_surface(self, local: Vector2) -> bool {
        match self {
            Self::Rect { size } => {
                let half = size * 0.5;
                local.x.abs() <= half.x && local.y.abs() <= half.y
            }
            Self::Box { size } => {
                let half = Vector2::new(size.x, size.z) * 0.5;
                local.x.abs() <= half.x && local.y.abs() <= half.y
            }
            Self::Circle { radius } | Self::Cylinder { radius, .. } => {
                local.x * local.x + local.y * local.y <= radius * radius
            }
        }
    }
}

impl Default for WaterShape {
    fn default() -> Self {
        Self::Rect {
            size: Vector2::new(32.0, 32.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterSurfaceParams {
    pub shape: WaterShape,
    pub resolution: [u32; 2],
    pub depth: f32,
    pub flow: Vector2,
    pub wind: Vector2,
    pub idle_mode: WaterIdleMode,
    pub wave: WaterWaveProfile,
    pub physics: WaterPhysicsParams,
    pub lod: WaterLodParams,
    pub collision_layers: BitMask,
    pub collision_mask: BitMask,
    pub link: WaterLinkParams,
    pub optics: WaterOpticsSettings,
    pub coastline: CoastlineSettings,
    pub debug: bool,
}

impl Default for WaterSurfaceParams {
    fn default() -> Self {
        Self {
            shape: WaterShape::rect(Vector2::new(32.0, 32.0)),
            resolution: [801, 801],
            depth: 4.0,
            flow: Vector2::ZERO,
            wind: Vector2::new(1.0, 0.0),
            idle_mode: WaterIdleMode::Calm,
            wave: WaterWaveProfile::default(),
            physics: WaterPhysicsParams::default(),
            lod: WaterLodParams::default(),
            collision_layers: BitMask::ALL,
            collision_mask: BitMask::NONE,
            link: WaterLinkParams::default(),
            optics: WaterOpticsSettings::default(),
            coastline: CoastlineSettings::default(),
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
pub const fn water_idle_speed_scale() -> f32 {
    0.2
}

#[inline]
pub fn analytic_idle_water_height(
    surface: &WaterSurfaceParams,
    local: Vector2,
    time_seconds: f32,
) -> f32 {
    let phase = time_seconds * surface.wave.speed * water_idle_speed_scale();
    let size = surface.shape.surface_size();
    let wave_coord = Vector2::new(
        local.x / size.x.abs().max(0.001),
        local.y / size.y.abs().max(0.001),
    );
    let tau = std::f32::consts::TAU;
    match surface.idle_mode {
        WaterIdleMode::Calm => 0.0,
        WaterIdleMode::Sine => {
            let wind = surface.wind.normalized();
            (wave_coord.dot(wind) * tau + phase).sin() * surface.wave.scale
        }
        WaterIdleMode::Chop => {
            let wind = surface.wind.normalized();
            let cross = Vector2::new(-wind.y, wind.x);
            let a = (wave_coord.dot(wind) * tau * 1.4 + phase).sin();
            let b = (wave_coord.dot(cross) * tau * 0.9 + phase * 1.37).cos();
            let c = ((wave_coord.x * 0.7 + wave_coord.y * 1.3) * tau * 2.1 - phase * 0.72).sin();
            (a * 0.48 + b * 0.32 + c * 0.2) * surface.wave.scale * 3.05
        }
        WaterIdleMode::Storm => {
            let wind = surface.wind.normalized();
            let cross = Vector2::new(-wind.y, wind.x);
            let a = (wave_coord.dot(wind) * tau * 2.1 + phase * 1.1).sin();
            let b = (wave_coord.dot(cross) * tau * 1.3 - phase * 0.83).cos();
            let c = ((wave_coord.x * 1.6 + wave_coord.y * 0.55) * tau * 2.8 + phase * 1.47).sin();
            let d = ((wave_coord.x * -0.35 + wave_coord.y * 1.2) * tau * 3.7 - phase * 1.91).cos();
            let swell_a = (wave_coord.dot(wind) * tau * 1.15 + phase * 0.9)
                .sin()
                .max(0.0)
                .powf(5.0);
            let swell_b = (wave_coord.dot(cross) * tau * 0.95 - phase * 1.25 + 1.7)
                .sin()
                .max(0.0)
                .powf(4.0);
            (a * 0.22 + b * 0.18 + c * 0.16 + d * 0.12 + swell_a * 0.72 + swell_b * 0.46)
                * surface.wave.scale
                * 3.05
        }
        WaterIdleMode::River => {
            let flow = surface.flow.normalized();
            (wave_coord.dot(flow) * tau * 1.6 - phase * 1.5).sin() * surface.wave.scale * 0.45
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
    fn link_params_default_to_auto_link() {
        let params = WaterSurfaceParams::default();
        assert_eq!(params.link.link_layers, BitMask::ALL);
        assert_eq!(params.link.link_mask, BitMask::NONE);
        assert_eq!(params.link.blend_width, 0.0);
        assert_eq!(params.link.wave_transfer, 1.0);
        assert_eq!(params.link.flow_transfer, 1.0);
    }

    #[test]
    fn sample_prefers_gpu_cache_and_falls_back_to_idle() {
        let mut surface = WaterSurfaceParams {
            idle_mode: WaterIdleMode::Sine,
            flow: Vector2::new(1.0, 0.0),
            ..Default::default()
        };
        surface.wave.scale = 2.0;

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

    #[test]
    fn idle_waves_scale_with_surface_size() {
        let mut small = WaterSurfaceParams {
            idle_mode: WaterIdleMode::Storm,
            shape: WaterShape::rect(Vector2::new(20.0, 20.0)),
            ..Default::default()
        };
        small.wave.scale = 1.0;

        let mut large = small;
        large.shape = WaterShape::rect(Vector2::new(200.0, 200.0));

        let small_height = analytic_idle_water_height(&small, Vector2::new(4.0, -3.0), 0.5);
        let large_height = analytic_idle_water_height(&large, Vector2::new(40.0, -30.0), 0.5);

        assert!((small_height - large_height).abs() < 0.0001);
    }

    #[test]
    fn idle_wave_speed_uses_slow_authoring_scale() {
        let mut surface = WaterSurfaceParams {
            idle_mode: WaterIdleMode::Sine,
            shape: WaterShape::rect(Vector2::new(20.0, 20.0)),
            wind: Vector2::new(1.0, 0.0),
            ..Default::default()
        };
        surface.wave.speed = 1.0;

        let scaled = analytic_idle_water_height(&surface, Vector2::ZERO, 1.0);
        let expected = (water_idle_speed_scale()).sin() * surface.wave.scale;

        assert!((scaled - expected).abs() < 0.0001);
    }

    #[test]
    fn storm_idle_allows_large_stacked_swell_waves() {
        let mut surface = WaterSurfaceParams {
            idle_mode: WaterIdleMode::Storm,
            shape: WaterShape::rect(Vector2::new(20.0, 20.0)),
            wind: Vector2::new(0.8, 0.2),
            ..Default::default()
        };
        surface.wave.scale = 1.0;

        for x in -4..=4 {
            for y in -4..=4 {
                let height = analytic_idle_water_height(
                    &surface,
                    Vector2::new(x as f32 * 2.0, y as f32 * 2.0),
                    1.25,
                );
                assert!(height.abs() <= 5.68);
            }
        }
    }
}
