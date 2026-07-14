const LOCALIZATION_CSV_CANDIDATES: &[&str] =
    &["localization.csv", "locale.csv", "translations.csv"];

pub const MAX_AUDIO_PROPAGATION_BOUNCES: u32 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcclusionCulling {
    Cpu,
    Gpu,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SsaoQuality {
    Off,
    Low,
    #[default]
    Medium,
    High,
    Ultra,
}

impl SsaoQuality {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Ultra => "ultra",
        }
    }
}

impl OcclusionCulling {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
            Self::Off => "off",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleSimDefault {
    Cpu,
    GpuVertex,
    GpuCompute,
}

impl ParticleSimDefault {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::GpuVertex => "hybrid",
            Self::GpuCompute => "gpu",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FrameRateCap {
    #[default]
    Unlimited,
    Fps(f32),
    RefreshRate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizationConfig {
    pub source_csv: String,
    pub key_column: String,
    pub default_locale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SteamInputMode {
    #[default]
    Off,
    Metadata,
    Actions,
}

impl SteamInputMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Metadata => "metadata",
            Self::Actions => "actions",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SteamConfig {
    pub enabled: bool,
    pub app_id: Option<u32>,
    pub input_mode: SteamInputMode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioPropagationConfig {
    pub max_bounces: u32,
    pub rays_per_tick: u32,
    pub max_ray_distance: f32,
}

impl Default for AudioPropagationConfig {
    fn default() -> Self {
        Self {
            max_bounces: 4,
            rays_per_tick: 64,
            max_ray_distance: 500.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioConfig {
    pub listener_max_distance: f32,
    pub propagation_tick_hz: f32,
    pub energy_cutoff: f32,
    pub debug_rays: bool,
    pub propagation_2d: AudioPropagationConfig,
    pub propagation_3d: AudioPropagationConfig,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            listener_max_distance: 500.0,
            propagation_tick_hz: 20.0,
            energy_cutoff: 0.02,
            debug_rays: false,
            propagation_2d: AudioPropagationConfig::default(),
            propagation_3d: AudioPropagationConfig {
                max_bounces: 4,
                rays_per_tick: 128,
                max_ray_distance: 500.0,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProjectMetadata {
    pub description: Option<String>,
    pub company: Option<String>,
    pub version: Option<String>,
    pub copyright: Option<String>,
    pub trademark: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProjectWebConfig {
    pub title: Option<String>,
    pub description: Option<String>,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderUiConfig {
    pub pixel_snapping: bool,
}

impl Default for RenderUiConfig {
    fn default() -> Self {
        Self {
            pixel_snapping: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RenderingConfig {
    pub ui: RenderUiConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRoute {
    pub href: String,
    pub name: String,
    pub scene: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProjectRoutesConfig {
    pub routes: Vec<ProjectRoute>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StaticProjectConfig {
    pub name: &'static str,
    pub metadata_description: Option<&'static str>,
    pub metadata_company: Option<&'static str>,
    pub metadata_version: Option<&'static str>,
    pub metadata_copyright: Option<&'static str>,
    pub metadata_trademark: Option<&'static str>,
    pub main_scene_hash: u64,
    pub icon_hash: u64,
    pub startup_splash_hash: u64,
    pub virtual_width: u32,
    pub virtual_height: u32,
    pub vsync: bool,
    pub frame_rate_cap: FrameRateCap,
    pub target_fixed_update: Option<f32>,
    pub physics_gravity: f32,
    pub physics_coef: f32,
    pub msaa: bool,
    pub ssao: SsaoQuality,
    pub meshlets: bool,
    pub dev_meshlets: bool,
    pub release_meshlets: bool,
    pub meshlet_debug_view: bool,
    pub occlusion_culling: OcclusionCulling,
    pub particle_sim_default: ParticleSimDefault,
    pub texture_filter: perro_structs::TextureFilterMode,
    pub rendering_ui_pixel_snapping: bool,
    pub audio_listener_max_distance: f32,
    pub audio_propagation_tick_hz: f32,
    pub audio_energy_cutoff: f32,
    pub audio_debug_rays: bool,
    pub audio_2d_max_bounces: u32,
    pub audio_2d_rays_per_tick: u32,
    pub audio_2d_max_ray_distance: f32,
    pub audio_3d_max_bounces: u32,
    pub audio_3d_rays_per_tick: u32,
    pub audio_3d_max_ray_distance: f32,
    pub localization_default_locale: &'static str,
    pub steam_enabled: bool,
    pub steam_app_id: Option<u32>,
    pub steam_input_mode: SteamInputMode,
}

impl StaticProjectConfig {
    pub const fn new(
        name: &'static str,
        main_scene_hash: u64,
        icon_hash: u64,
        startup_splash_hash: u64,
        virtual_width: u32,
        virtual_height: u32,
    ) -> Self {
        Self {
            name,
            metadata_description: None,
            metadata_company: None,
            metadata_version: None,
            metadata_copyright: None,
            metadata_trademark: None,
            main_scene_hash,
            icon_hash,
            startup_splash_hash,
            virtual_width,
            virtual_height,
            vsync: false,
            frame_rate_cap: FrameRateCap::Unlimited,
            target_fixed_update: Some(60.0),
            physics_gravity: -9.81,
            physics_coef: 1.0,
            msaa: true,
            ssao: SsaoQuality::Medium,
            meshlets: false,
            dev_meshlets: false,
            release_meshlets: true,
            meshlet_debug_view: false,
            occlusion_culling: OcclusionCulling::Gpu,
            particle_sim_default: ParticleSimDefault::Cpu,
            texture_filter: perro_structs::TextureFilterMode::LinearMipmap,
            rendering_ui_pixel_snapping: true,
            audio_listener_max_distance: 500.0,
            audio_propagation_tick_hz: 20.0,
            audio_energy_cutoff: 0.02,
            audio_debug_rays: false,
            audio_2d_max_bounces: 4,
            audio_2d_rays_per_tick: 64,
            audio_2d_max_ray_distance: 500.0,
            audio_3d_max_bounces: 4,
            audio_3d_rays_per_tick: 128,
            audio_3d_max_ray_distance: 500.0,
            localization_default_locale: "en",
            steam_enabled: false,
            steam_app_id: None,
            steam_input_mode: SteamInputMode::Off,
        }
    }

    pub const fn with_vsync(mut self, enabled: bool) -> Self {
        self.vsync = enabled;
        self
    }

    pub const fn with_frame_rate_cap(mut self, cap: FrameRateCap) -> Self {
        self.frame_rate_cap = cap;
        self
    }

    pub const fn with_target_fixed_update(mut self, target_fixed_update: Option<f32>) -> Self {
        self.target_fixed_update = target_fixed_update;
        self
    }

    pub const fn with_physics_gravity(mut self, gravity: f32) -> Self {
        self.physics_gravity = gravity;
        self
    }

    pub const fn with_physics_coef(mut self, coef: f32) -> Self {
        self.physics_coef = coef;
        self
    }

    pub const fn with_msaa(mut self, enabled: bool) -> Self {
        self.msaa = enabled;
        self
    }

    pub const fn with_ssao(mut self, quality: SsaoQuality) -> Self {
        self.ssao = quality;
        self
    }

    pub const fn with_dev_meshlets(mut self, enabled: bool) -> Self {
        self.dev_meshlets = enabled;
        self
    }

    pub const fn with_meshlets(mut self, enabled: bool) -> Self {
        self.meshlets = enabled;
        self
    }

    pub const fn with_release_meshlets(mut self, enabled: bool) -> Self {
        self.release_meshlets = enabled;
        self
    }

    pub const fn with_meshlet_debug_view(mut self, enabled: bool) -> Self {
        self.meshlet_debug_view = enabled;
        self
    }

    pub const fn with_occlusion_culling(mut self, mode: OcclusionCulling) -> Self {
        self.occlusion_culling = mode;
        self
    }

    pub const fn with_particle_sim_default(mut self, mode: ParticleSimDefault) -> Self {
        self.particle_sim_default = mode;
        self
    }

    pub const fn with_texture_filter(mut self, mode: perro_structs::TextureFilterMode) -> Self {
        self.texture_filter = mode;
        self
    }

    pub const fn with_ui_pixel_snapping(mut self, enabled: bool) -> Self {
        self.rendering_ui_pixel_snapping = enabled;
        self
    }

    pub const fn with_audio_config(mut self, config: AudioConfig) -> Self {
        self.audio_listener_max_distance = config.listener_max_distance;
        self.audio_propagation_tick_hz = config.propagation_tick_hz;
        self.audio_energy_cutoff = config.energy_cutoff;
        self.audio_debug_rays = config.debug_rays;
        self.audio_2d_max_bounces = config.propagation_2d.max_bounces;
        self.audio_2d_rays_per_tick = config.propagation_2d.rays_per_tick;
        self.audio_2d_max_ray_distance = config.propagation_2d.max_ray_distance;
        self.audio_3d_max_bounces = config.propagation_3d.max_bounces;
        self.audio_3d_rays_per_tick = config.propagation_3d.rays_per_tick;
        self.audio_3d_max_ray_distance = config.propagation_3d.max_ray_distance;
        self
    }

    pub const fn with_metadata(
        mut self,
        description: Option<&'static str>,
        company: Option<&'static str>,
        version: Option<&'static str>,
        copyright: Option<&'static str>,
        trademark: Option<&'static str>,
    ) -> Self {
        self.metadata_description = description;
        self.metadata_company = company;
        self.metadata_version = version;
        self.metadata_copyright = copyright;
        self.metadata_trademark = trademark;
        self
    }

    pub const fn with_localization(mut self, default_locale: &'static str) -> Self {
        self.localization_default_locale = default_locale;
        self
    }

    pub const fn with_steam(mut self, enabled: bool, app_id: Option<u32>) -> Self {
        self.steam_enabled = enabled;
        self.steam_app_id = app_id;
        self
    }

    pub const fn with_steam_input_mode(mut self, input_mode: SteamInputMode) -> Self {
        self.steam_input_mode = input_mode;
        self
    }

    pub fn to_runtime(self) -> ProjectConfig {
        ProjectConfig {
            name: self.name.to_string(),
            metadata: ProjectMetadata {
                description: self.metadata_description.map(str::to_string),
                company: self.metadata_company.map(str::to_string),
                version: self.metadata_version.map(str::to_string),
                copyright: self.metadata_copyright.map(str::to_string),
                trademark: self.metadata_trademark.map(str::to_string),
            },
            web: ProjectWebConfig::default(),
            main_scene: self.main_scene_hash.to_string(),
            main_scene_hash: Some(self.main_scene_hash),
            icon: self.icon_hash.to_string(),
            icon_hash: Some(self.icon_hash),
            startup_splash: self.startup_splash_hash.to_string(),
            startup_splash_hash: Some(self.startup_splash_hash),
            virtual_width: self.virtual_width,
            virtual_height: self.virtual_height,
            vsync: self.vsync,
            frame_rate_cap: self.frame_rate_cap,
            target_fixed_update: self.target_fixed_update,
            physics_gravity: self.physics_gravity,
            physics_coef: self.physics_coef,
            msaa: self.msaa,
            ssao: self.ssao,
            meshlets: self.meshlets,
            dev_meshlets: self.dev_meshlets,
            release_meshlets: self.release_meshlets,
            meshlet_debug_view: self.meshlet_debug_view,
            occlusion_culling: self.occlusion_culling,
            particle_sim_default: self.particle_sim_default,
            texture_filter: self.texture_filter,
            rendering: RenderingConfig {
                ui: RenderUiConfig {
                    pixel_snapping: self.rendering_ui_pixel_snapping,
                },
            },
            audio: AudioConfig {
                listener_max_distance: self.audio_listener_max_distance,
                propagation_tick_hz: self.audio_propagation_tick_hz,
                energy_cutoff: self.audio_energy_cutoff,
                debug_rays: self.audio_debug_rays,
                propagation_2d: AudioPropagationConfig {
                    max_bounces: self.audio_2d_max_bounces,
                    rays_per_tick: self.audio_2d_rays_per_tick,
                    max_ray_distance: self.audio_2d_max_ray_distance,
                },
                propagation_3d: AudioPropagationConfig {
                    max_bounces: self.audio_3d_max_bounces,
                    rays_per_tick: self.audio_3d_rays_per_tick,
                    max_ray_distance: self.audio_3d_max_ray_distance,
                },
            },
            localization: Some(LocalizationConfig {
                source_csv: String::new(),
                key_column: "key".to_string(),
                default_locale: self.localization_default_locale.to_string(),
            }),
            input_map: perro_input_api::InputMap::new(),
            steam: SteamConfig {
                enabled: self.steam_enabled,
                app_id: self.steam_app_id,
                input_mode: self.steam_input_mode,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectConfig {
    pub name: String,
    pub metadata: ProjectMetadata,
    pub web: ProjectWebConfig,
    pub main_scene: String,
    pub main_scene_hash: Option<u64>,
    pub icon: String,
    pub icon_hash: Option<u64>,
    pub startup_splash: String,
    pub startup_splash_hash: Option<u64>,
    pub virtual_width: u32,
    pub virtual_height: u32,
    pub vsync: bool,
    pub frame_rate_cap: FrameRateCap,
    pub target_fixed_update: Option<f32>,
    pub physics_gravity: f32,
    pub physics_coef: f32,
    pub msaa: bool,
    pub ssao: SsaoQuality,
    pub meshlets: bool,
    pub dev_meshlets: bool,
    pub release_meshlets: bool,
    pub meshlet_debug_view: bool,
    pub occlusion_culling: OcclusionCulling,
    pub particle_sim_default: ParticleSimDefault,
    pub texture_filter: perro_structs::TextureFilterMode,
    pub rendering: RenderingConfig,
    pub audio: AudioConfig,
    pub localization: Option<LocalizationConfig>,
    pub input_map: perro_input_api::InputMap,
    pub steam: SteamConfig,
}

impl ProjectConfig {
    pub fn default_for_name(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            metadata: ProjectMetadata::default(),
            web: ProjectWebConfig::default(),
            main_scene: "res://main.scn".to_string(),
            main_scene_hash: None,
            icon: "res://icon.png".to_string(),
            icon_hash: None,
            startup_splash: "res://icon.png".to_string(),
            startup_splash_hash: None,
            virtual_width: 1920,
            virtual_height: 1080,
            vsync: false,
            frame_rate_cap: FrameRateCap::Unlimited,
            target_fixed_update: Some(60.0),
            physics_gravity: -9.81,
            physics_coef: 1.0,
            msaa: true,
            ssao: SsaoQuality::Medium,
            meshlets: false,
            dev_meshlets: false,
            release_meshlets: true,
            meshlet_debug_view: false,
            occlusion_culling: OcclusionCulling::Gpu,
            particle_sim_default: ParticleSimDefault::Cpu,
            texture_filter: perro_structs::TextureFilterMode::LinearMipmap,
            rendering: RenderingConfig::default(),
            audio: AudioConfig::default(),
            localization: None,
            input_map: perro_input_api::InputMap::new(),
            steam: SteamConfig::default(),
        }
    }
}

impl ProjectRoutesConfig {
    pub fn scene_for_href(&self, href: &str) -> Option<&str> {
        self.routes
            .iter()
            .find(|route| route.href == href)
            .map(|route| route.scene.as_str())
    }
}
