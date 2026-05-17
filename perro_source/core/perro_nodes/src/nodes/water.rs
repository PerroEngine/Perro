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
    pub length: f32,
    pub damping: f32,
}

impl Default for WaterWaveProfile {
    fn default() -> Self {
        Self {
            speed: 1.0,
            scale: 1.0,
            length: 18.0,
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
            buoyancy: 2.0,
            drag: 0.75,
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
pub struct WaterVisualParams {
    pub transparency: f32,
    pub reflectivity: f32,
    pub roughness: f32,
    pub fresnel_power: f32,
    pub normal_strength: f32,
    pub ripple_scale: f32,
    pub foam_color: Color,
    pub foam_amount: f32,
    pub crest_foam_threshold: f32,
    pub caustic_strength: f32,
    pub refraction_strength: f32,
    pub scattering_strength: f32,
    pub distance_fog_strength: f32,
}

impl WaterVisualParams {
    pub const fn new() -> Self {
        Self {
            transparency: 0.20,
            reflectivity: 0.58,
            roughness: 0.14,
            fresnel_power: 4.2,
            normal_strength: 1.28,
            ripple_scale: 1.0,
            foam_color: Color::new(0.86, 0.96, 1.0, 1.0),
            foam_amount: 0.86,
            crest_foam_threshold: 0.50,
            caustic_strength: 0.26,
            refraction_strength: 0.16,
            scattering_strength: 0.24,
            distance_fog_strength: 0.28,
        }
    }
}

impl Default for WaterVisualParams {
    fn default() -> Self {
        Self::new()
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
            foam_strength: 1.05,
            foam_width: 2.25,
            cutoff_softness: 0.34,
            wave_reflection: 0.45,
            wave_damping: 0.42,
            edge_noise: 0.24,
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
    pub render_resolution: [u32; 2],
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
    pub visual: WaterVisualParams,
    pub coastline: CoastlineSettings,
    pub debug: bool,
}

impl Default for WaterSurfaceParams {
    fn default() -> Self {
        Self {
            shape: WaterShape::rect(Vector2::new(32.0, 32.0)),
            resolution: [801, 801],
            render_resolution: [801, 801],
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
            visual: WaterVisualParams::default(),
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
    let wave_coord = local * (1.0 / surface.wave.length.max(0.001));
    let tau = std::f32::consts::TAU;
    match surface.idle_mode {
        WaterIdleMode::Calm => 0.0,
        WaterIdleMode::Sine => {
            let wind = normalized_or_x(surface.wind);
            let cross = Vector2::new(-wind.y, wind.x);
            let a = (wave_coord.dot(wind) * tau + phase).sin();
            let b = (wave_coord.dot(cross) * tau * 1.73 - phase * 0.61).sin();
            let c = ((wave_coord.x * 0.37 + wave_coord.y * 0.91) * tau * 2.37 + phase * 1.41).sin();
            (crest_wave(a) * 0.52 + b * 0.24 + crest_wave(c) * 0.24) * surface.wave.scale
        }
        WaterIdleMode::Chop => {
            let wind = normalized_or_x(surface.wind);
            let cross = Vector2::new(-wind.y, wind.x);
            let a = (wave_coord.dot(wind) * tau * 0.72 + phase * 0.84).sin();
            let b = (wave_coord.dot(cross) * tau * 1.21 - phase * 1.17).cos();
            let c = ((wave_coord.x * 0.74 + wave_coord.y * 1.36) * tau * 1.83 + phase * 1.46).sin();
            let d =
                ((wave_coord.x * -1.19 + wave_coord.y * 0.48) * tau * 2.79 - phase * 2.08).cos();
            (crest_wave(a) * 0.42 + b * 0.20 + crest_wave(c) * 0.25 + d * 0.13)
                * surface.wave.scale
                * 1.45
        }
        WaterIdleMode::Storm => {
            let wind = normalized_or_x(surface.wind);
            let cross = Vector2::new(-wind.y, wind.x);
            let a = (wave_coord.dot(wind) * tau * 0.58 + phase * 0.77).sin();
            let b = (wave_coord.dot(cross) * tau * 1.02 - phase * 0.91).cos();
            let c = ((wave_coord.x * 1.43 + wave_coord.y * 0.61) * tau * 1.74 + phase * 1.37).sin();
            let d =
                ((wave_coord.x * -0.51 + wave_coord.y * 1.18) * tau * 2.52 - phase * 1.91).cos();
            let swell_a = pow5(
                (wave_coord.dot(wind) * tau * 0.39 + phase * 0.63)
                    .sin()
                    .max(0.0),
            );
            let swell_b = pow4(
                (wave_coord.dot(cross) * tau * 0.64 - phase * 1.09 + 1.7)
                    .sin()
                    .max(0.0),
            );
            (crest_wave(a) * 0.30
                + b * 0.12
                + crest_wave(c) * 0.14
                + d * 0.10
                + swell_a * 0.82
                + swell_b * 0.56)
                * surface.wave.scale
                * 1.65
        }
        WaterIdleMode::River => {
            let flow = normalized_or(surface.flow, normalized_or_x(surface.wind));
            let cross = Vector2::new(-flow.y, flow.x);
            let downstream = wave_coord.dot(flow);
            let across = wave_coord.dot(cross);
            let rush = (downstream * tau * 2.6 - phase * 4.2).sin();
            let train = (downstream * tau * 5.1 - phase * 7.4 + across * 1.15).sin();
            let shear = (across * tau * 1.35 + downstream * 0.9 - phase * 1.1).sin();
            (crest_wave(rush) * 0.58 + train * 0.28 + shear * 0.14)
                * surface.wave.scale
                * 0.52
        }
    }
}

#[inline]
fn crest_wave(v: f32) -> f32 {
    let pos = v.max(0.0);
    let neg = (-v).max(0.0);
    pow3(pos) - fast_pow_1_35(neg) * 0.30
}

#[inline]
fn normalized_or_x(v: Vector2) -> Vector2 {
    normalized_or(v, Vector2::new(1.0, 0.0))
}

#[inline]
fn normalized_or(v: Vector2, fallback: Vector2) -> Vector2 {
    let len_sq = v.x * v.x + v.y * v.y;
    if len_sq <= 1.0e-12 {
        fallback
    } else if (len_sq - 1.0).abs() <= 1.0e-4 {
        v
    } else {
        v / len_sq.sqrt()
    }
}

#[inline]
fn pow3(v: f32) -> f32 {
    v * v * v
}

#[inline]
fn pow4(v: f32) -> f32 {
    let v2 = v * v;
    v2 * v2
}

#[inline]
fn pow5(v: f32) -> f32 {
    pow4(v) * v
}

#[inline]
fn fast_pow_1_35(v: f32) -> f32 {
    if v <= 0.0 {
        0.0
    } else {
        let sqrt = v.sqrt();
        v * (sqrt * 0.60 + v * 0.40)
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
    fn idle_waves_use_world_space_profile_length() {
        let mut small = WaterSurfaceParams {
            idle_mode: WaterIdleMode::Storm,
            shape: WaterShape::rect(Vector2::new(20.0, 20.0)),
            ..Default::default()
        };
        small.wave.scale = 1.0;

        let mut large = small;
        large.shape = WaterShape::rect(Vector2::new(200.0, 200.0));

        let small_height = analytic_idle_water_height(&small, Vector2::new(4.0, -3.0), 0.5);
        let same_world_height = analytic_idle_water_height(&large, Vector2::new(4.0, -3.0), 0.5);
        let scaled_world_height =
            analytic_idle_water_height(&large, Vector2::new(40.0, -30.0), 0.5);

        assert!((small_height - same_world_height).abs() < 0.0001);
        assert!((small_height - scaled_world_height).abs() > 0.01);
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
        assert!(scaled.abs() > 0.0001);
        assert!(scaled.abs() <= surface.wave.scale);
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

    #[test]
    fn river_idle_uses_flow_as_rush_direction() {
        let x_flow = WaterSurfaceParams {
            idle_mode: WaterIdleMode::River,
            shape: WaterShape::rect(Vector2::new(20.0, 20.0)),
            flow: Vector2::new(1.0, 0.0),
            wind: Vector2::new(0.0, 1.0),
            wave: WaterWaveProfile {
                speed: 1.0,
                scale: 1.0,
                length: 10.0,
                damping: 0.985,
            },
            ..Default::default()
        };
        let y_flow = WaterSurfaceParams {
            flow: Vector2::new(0.0, 1.0),
            ..x_flow
        };
        let sample = Vector2::new(3.0, 0.0);

        let x_height = analytic_idle_water_height(&x_flow, sample, 0.7);
        let y_height = analytic_idle_water_height(&y_flow, sample, 0.7);

        assert!((x_height - y_height).abs() > 0.05);
    }
}
