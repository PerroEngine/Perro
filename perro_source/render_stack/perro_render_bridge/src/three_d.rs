use super::*;
use crate::two_d::{WaterBodyQueryState, WaterIdleModeState, WaterLinkState, WaterShapeState};
use perro_structs::{AudioListenerOptions, BitMask, Color, CustomPostParam};

#[derive(Debug, Clone, PartialEq)]
pub struct Camera3DState {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub projection: CameraProjectionState,
    pub render_mask: BitMask,
    pub post_processing: Arc<[PostProcessEffect]>,
    pub audio_options: AudioListenerOptions,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CameraProjectionState {
    Perspective {
        fov_y_degrees: f32,
        near: f32,
        far: f32,
    },
    Orthographic {
        size: f32,
        near: f32,
        far: f32,
    },
    Frustum {
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmbientLight3DState {
    pub color: [f32; 3],
    pub intensity: f32,
    pub cast_shadows: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayLight3DState {
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
    pub cast_shadows: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointLight3DState {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub cast_shadows: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpotLight3DState {
    pub position: [f32; 3],
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub inner_angle_radians: f32,
    pub outer_angle_radians: f32,
    pub cast_shadows: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterImpact3D {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub strength: f32,
    pub radius: f32,
    pub cavitation: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterContact3D {
    pub body: NodeID,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub radius: f32,
    pub foam_amount: f32,
    pub persist: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WaterCoastlineShape3D {
    Box {
        center: [f32; 3],
        half_extents: [f32; 3],
        axis_x: [f32; 2],
        axis_z: [f32; 2],
    },
    Sphere {
        center: [f32; 3],
        radius: f32,
    },
    Cylinder {
        center: [f32; 3],
        radius: f32,
        half_height: f32,
    },
    Triangle {
        points: [[f32; 3]; 3],
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Water3DState {
    pub model: [[f32; 4]; 4],
    pub paused: bool,
    pub simulation_time: f32,
    pub simulation_delta: f32,
    pub size: [f32; 2],
    pub shape: WaterShapeState,
    pub resolution: [u32; 2],
    pub render_resolution: [u32; 2],
    pub depth: f32,
    pub flow: [f32; 2],
    pub wind: [f32; 2],
    pub idle_mode: WaterIdleModeState,
    pub wave_speed: f32,
    pub wave_scale: f32,
    pub wave_length: f32,
    pub damping: f32,
    pub wake_strength: f32,
    pub foam_strength: f32,
    pub sample_readback_rate: f32,
    pub lod_near_distance: f32,
    pub lod_mid_distance: f32,
    pub lod_far_distance: f32,
    pub lod_min_resolution: [u32; 2],
    pub collision_layers: BitMask,
    pub collision_mask: BitMask,
    pub deep_color: Color,
    pub shallow_color: Color,
    pub shallow_depth: f32,
    pub sky_bias_ratio: f32,
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
    pub coastline_foam_color: Color,
    pub coastline_foam_strength: f32,
    pub coastline_foam_width: f32,
    pub coastline_cutoff_softness: f32,
    pub coastline_wave_reflection: f32,
    pub coastline_wave_damping: f32,
    pub coastline_edge_noise: f32,
    pub debug: bool,
    pub links: Arc<[WaterLinkState]>,
    pub queries: Arc<[WaterBodyQueryState]>,
    pub impacts: Arc<[WaterImpact3D]>,
    pub coastline_shapes: Arc<[WaterCoastlineShape3D]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkyTime3DState {
    pub time_of_day: f32,
    pub paused: bool,
    pub scale: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkyShaderPass3DState {
    pub path: Cow<'static, str>,
    pub params: Arc<[CustomPostParam]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sky3DState {
    pub day_colors: Arc<[[f32; 3]]>,
    pub evening_colors: Arc<[[f32; 3]]>,
    pub night_colors: Arc<[[f32; 3]]>,
    pub horizon_colors: Arc<[[f32; 3]]>,
    pub time: SkyTime3DState,
    pub shaders: Arc<[SkyShaderPass3DState]>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParticlePath3D {
    None,
    Ballistic,
    Spiral {
        angular_velocity: f32,
        radius: f32,
    },
    OrbitY {
        angular_velocity: f32,
        radius: f32,
    },
    NoiseDrift {
        amplitude: f32,
        frequency: f32,
    },
    FlatDisk {
        radius: f32,
    },
    Custom {
        expr_x: Cow<'static, str>,
        expr_y: Cow<'static, str>,
        expr_z: Cow<'static, str>,
    },
    CustomCompiled {
        expr_x_ops: Cow<'static, [ParticleExprOp3D]>,
        expr_y_ops: Cow<'static, [ParticleExprOp3D]>,
        expr_z_ops: Cow<'static, [ParticleExprOp3D]>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParticlePath2D {
    None,
    Ballistic,
    Spiral {
        angular_velocity: f32,
        radius: f32,
    },
    NoiseDrift {
        amplitude: f32,
        frequency: f32,
    },
    FlatDisk {
        radius: f32,
    },
    Custom {
        expr_x: Cow<'static, str>,
        expr_y: Cow<'static, str>,
    },
    CustomCompiled {
        expr_x_ops: Cow<'static, [ParticleExprOp2D]>,
        expr_y_ops: Cow<'static, [ParticleExprOp2D]>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleSimulationMode3D {
    Cpu,
    GpuVertex,
    GpuCompute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleSimulationMode2D {
    Cpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleRenderMode3D {
    Point,
    Billboard,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParticleProfile3D {
    pub path: ParticlePath3D,
    pub expr_x_ops: Option<Cow<'static, [ParticleExprOp3D]>>,
    pub expr_y_ops: Option<Cow<'static, [ParticleExprOp3D]>>,
    pub expr_z_ops: Option<Cow<'static, [ParticleExprOp3D]>>,
    pub lifetime_min: f32,
    pub lifetime_max: f32,
    pub speed_min: f32,
    pub speed_max: f32,
    pub spread_radians: f32,
    pub size: f32,
    pub size_min: f32,
    pub size_max: f32,
    pub force: [f32; 3],
    pub color_start: Color,
    pub color_end: Color,
    pub emissive: [f32; 3],
    pub spin_angular_velocity: f32,
}

impl Default for ParticleProfile3D {
    fn default() -> Self {
        Self {
            path: ParticlePath3D::None,
            expr_x_ops: None,
            expr_y_ops: None,
            expr_z_ops: None,
            lifetime_min: 0.6,
            lifetime_max: 1.4,
            speed_min: 1.0,
            speed_max: 3.0,
            spread_radians: std::f32::consts::FRAC_PI_3,
            size: 6.0,
            size_min: 0.65,
            size_max: 1.35,
            force: [0.0, 0.0, 0.0],
            color_start: Color::WHITE,
            color_end: Color::new(1.0, 0.4, 0.1, 0.0),
            emissive: [0.0, 0.0, 0.0],
            spin_angular_velocity: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParticleProfile2D {
    pub path: ParticlePath2D,
    pub expr_x_ops: Option<Cow<'static, [ParticleExprOp2D]>>,
    pub expr_y_ops: Option<Cow<'static, [ParticleExprOp2D]>>,
    pub lifetime_min: f32,
    pub lifetime_max: f32,
    pub speed_min: f32,
    pub speed_max: f32,
    pub spread_radians: f32,
    pub size: f32,
    pub size_min: f32,
    pub size_max: f32,
    pub force: [f32; 2],
    pub color_start: Color,
    pub color_end: Color,
    pub spin_angular_velocity: f32,
}

impl Default for ParticleProfile2D {
    fn default() -> Self {
        Self {
            path: ParticlePath2D::None,
            expr_x_ops: None,
            expr_y_ops: None,
            lifetime_min: 0.6,
            lifetime_max: 1.4,
            speed_min: 1.0,
            speed_max: 3.0,
            spread_radians: std::f32::consts::FRAC_PI_3,
            size: 6.0,
            size_min: 0.65,
            size_max: 1.35,
            force: [0.0, 0.0],
            color_start: Color::WHITE,
            color_end: Color::new(1.0, 0.4, 0.1, 0.0),
            spin_angular_velocity: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PointParticles3DState {
    pub model: [[f32; 4]; 4],
    pub active: bool,
    pub looping: bool,
    pub prewarm: bool,
    pub alive_budget: u32,
    pub emission_rate: f32,
    pub lifetime_min: f32,
    pub lifetime_max: f32,
    pub speed_min: f32,
    pub speed_max: f32,
    pub spread_radians: f32,
    pub size: f32,
    pub size_min: f32,
    pub size_max: f32,
    pub gravity: [f32; 3],
    pub color_start: Color,
    pub color_end: Color,
    pub emissive: [f32; 3],
    pub seed: u32,
    pub params: Vec<f32>,
    pub simulation_time: f32,
    pub simulation_delta: f32,
    pub profile: ParticleProfile3D,
    pub sim_mode: ParticleSimulationMode3D,
    pub render_mode: ParticleRenderMode3D,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PointParticles2DState {
    pub model: [[f32; 3]; 3],
    pub z_index: i32,
    pub active: bool,
    pub looping: bool,
    pub prewarm: bool,
    pub alive_budget: u32,
    pub emission_rate: f32,
    pub lifetime_min: f32,
    pub lifetime_max: f32,
    pub speed_min: f32,
    pub speed_max: f32,
    pub spread_radians: f32,
    pub size: f32,
    pub size_min: f32,
    pub size_max: f32,
    pub force: [f32; 2],
    pub color_start: Color,
    pub color_end: Color,
    pub seed: u32,
    pub params: Vec<f32>,
    pub simulation_time: f32,
    pub simulation_delta: f32,
    pub profile: ParticleProfile2D,
    pub sim_mode: ParticleSimulationMode2D,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StandardMaterial3D {
    pub base_color_factor: [f32; 4],
    pub roughness_factor: f32,
    pub metallic_factor: f32,
    pub occlusion_strength: f32,
    pub emissive_factor: [f32; 3],
    pub alpha_mode: u8, // 0=OPAQUE, 1=MASK, 2=BLEND
    pub alpha_cutoff: f32,
    pub double_sided: bool,
    pub flat_shading: bool,
    pub normal_scale: f32,
    // Texture slot indices (glTF material-local index or NONE).
    pub base_color_texture: u32,
    pub metallic_roughness_texture: u32,
    pub normal_texture: u32,
    pub occlusion_texture: u32,
    pub emissive_texture: u32,
}

impl Default for StandardMaterial3D {
    fn default() -> Self {
        Self::const_default()
    }
}

impl StandardMaterial3D {
    #[inline]
    pub const fn const_default() -> Self {
        Self {
            base_color_factor: [0.85, 0.85, 0.85, 1.0],
            roughness_factor: 0.5,
            metallic_factor: 0.0,
            occlusion_strength: 1.0,
            emissive_factor: [0.0, 0.0, 0.0],
            alpha_mode: 0,
            alpha_cutoff: 0.5,
            double_sided: false,
            flat_shading: false,
            normal_scale: 1.0,
            base_color_texture: MATERIAL_TEXTURE_NONE,
            metallic_roughness_texture: MATERIAL_TEXTURE_NONE,
            normal_texture: MATERIAL_TEXTURE_NONE,
            occlusion_texture: MATERIAL_TEXTURE_NONE,
            emissive_texture: MATERIAL_TEXTURE_NONE,
        }
    }
}

pub const MATERIAL_TEXTURE_NONE: u32 = u32::MAX;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnlitMaterial3D {
    pub base_color_factor: [f32; 4],
    pub emissive_factor: [f32; 3],
    pub alpha_mode: u8, // 0=OPAQUE, 1=MASK, 2=BLEND
    pub alpha_cutoff: f32,
    pub double_sided: bool,
    pub flat_shading: bool,
    // Texture slot indices (material-local index or NONE).
    pub base_color_texture: u32,
}

impl Default for UnlitMaterial3D {
    fn default() -> Self {
        Self::const_default()
    }
}

impl UnlitMaterial3D {
    #[inline]
    pub const fn const_default() -> Self {
        Self {
            base_color_factor: [1.0, 1.0, 1.0, 1.0],
            emissive_factor: [0.0, 0.0, 0.0],
            alpha_mode: 0,
            alpha_cutoff: 0.5,
            double_sided: false,
            flat_shading: false,
            base_color_texture: MATERIAL_TEXTURE_NONE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToonMaterial3D {
    pub base_color_factor: [f32; 4],
    pub emissive_factor: [f32; 3],
    pub alpha_mode: u8, // 0=OPAQUE, 1=MASK, 2=BLEND
    pub alpha_cutoff: f32,
    pub double_sided: bool,
    pub flat_shading: bool,
    pub band_count: u32,
    pub rim_strength: f32,
    pub outline_width: f32,
    // Texture slot indices (material-local index or NONE).
    pub base_color_texture: u32,
    pub ramp_texture: u32,
}

impl Default for ToonMaterial3D {
    fn default() -> Self {
        Self::const_default()
    }
}

impl ToonMaterial3D {
    #[inline]
    pub const fn const_default() -> Self {
        Self {
            base_color_factor: [1.0, 1.0, 1.0, 1.0],
            emissive_factor: [0.0, 0.0, 0.0],
            alpha_mode: 0,
            alpha_cutoff: 0.5,
            double_sided: false,
            flat_shading: false,
            band_count: 4,
            rim_strength: 0.0,
            outline_width: 0.0,
            base_color_texture: MATERIAL_TEXTURE_NONE,
            ramp_texture: MATERIAL_TEXTURE_NONE,
        }
    }
}

pub type CustomMaterialParamValue3D = perro_structs::ConstParamValue;

#[derive(Debug, Clone, PartialEq)]
pub struct CustomMaterialParam3D {
    pub name: Option<Cow<'static, str>>,
    pub value: CustomMaterialParamValue3D,
}

impl CustomMaterialParam3D {
    #[inline]
    pub fn named(name: impl Into<Cow<'static, str>>, value: CustomMaterialParamValue3D) -> Self {
        Self {
            name: Some(name.into()),
            value,
        }
    }

    #[inline]
    pub fn unnamed(value: CustomMaterialParamValue3D) -> Self {
        Self { name: None, value }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CustomMaterialImage3D {
    pub name: Option<Cow<'static, str>>,
    pub source: Cow<'static, str>,
}

impl CustomMaterialImage3D {
    #[inline]
    pub fn named(name: impl Into<Cow<'static, str>>, source: impl Into<Cow<'static, str>>) -> Self {
        Self {
            name: Some(name.into()),
            source: source.into(),
        }
    }

    #[inline]
    pub fn unnamed(source: impl Into<Cow<'static, str>>) -> Self {
        Self {
            name: None,
            source: source.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CustomMaterial3D {
    pub shader_path: Cow<'static, str>,
    pub params: Cow<'static, [CustomMaterialParam3D]>,
    pub images: Cow<'static, [CustomMaterialImage3D]>,
    pub lighting: CustomMaterialLighting3D,
    pub surface: StandardMaterial3D,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CustomMaterialLighting3D {
    Standard,
    Raw,
}

impl CustomMaterial3D {
    #[inline]
    pub fn new(shader_path: impl Into<Cow<'static, str>>) -> Self {
        Self {
            shader_path: shader_path.into(),
            params: Cow::Borrowed(&[]),
            images: Cow::Borrowed(&[]),
            lighting: CustomMaterialLighting3D::Standard,
            surface: StandardMaterial3D::default(),
        }
    }

    #[inline]
    pub fn with_params(
        shader_path: impl Into<Cow<'static, str>>,
        params: Vec<CustomMaterialParam3D>,
    ) -> Self {
        Self {
            shader_path: shader_path.into(),
            params: Cow::Owned(params),
            images: Cow::Borrowed(&[]),
            lighting: CustomMaterialLighting3D::Standard,
            surface: StandardMaterial3D::default(),
        }
    }

    #[inline]
    pub fn with_lighting(mut self, lighting: CustomMaterialLighting3D) -> Self {
        self.lighting = lighting;
        self
    }

    #[inline]
    pub fn with_images(mut self, images: Vec<CustomMaterialImage3D>) -> Self {
        self.images = Cow::Owned(images);
        self
    }

    #[inline]
    pub fn with_surface(mut self, surface: StandardMaterial3D) -> Self {
        self.surface = surface;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Material3D {
    Standard(StandardMaterial3D),
    Unlit(UnlitMaterial3D),
    Toon(ToonMaterial3D),
    Custom(CustomMaterial3D),
}

impl Default for Material3D {
    fn default() -> Self {
        Self::Standard(StandardMaterial3D::default())
    }
}

impl Material3D {
    #[inline]
    pub fn standard_params(&self) -> StandardMaterial3D {
        match self {
            Material3D::Standard(params) => *params,
            Material3D::Unlit(params) => StandardMaterial3D {
                base_color_factor: params.base_color_factor,
                emissive_factor: params.emissive_factor,
                alpha_mode: params.alpha_mode,
                alpha_cutoff: params.alpha_cutoff,
                double_sided: params.double_sided,
                flat_shading: params.flat_shading,
                base_color_texture: params.base_color_texture,
                metallic_roughness_texture: MATERIAL_TEXTURE_NONE,
                normal_texture: MATERIAL_TEXTURE_NONE,
                occlusion_texture: MATERIAL_TEXTURE_NONE,
                emissive_texture: MATERIAL_TEXTURE_NONE,
                roughness_factor: 1.0,
                metallic_factor: 0.0,
                occlusion_strength: 1.0,
                normal_scale: 1.0,
            },
            Material3D::Toon(params) => StandardMaterial3D {
                base_color_factor: params.base_color_factor,
                emissive_factor: params.emissive_factor,
                alpha_mode: params.alpha_mode,
                alpha_cutoff: params.alpha_cutoff,
                double_sided: params.double_sided,
                flat_shading: params.flat_shading,
                base_color_texture: params.base_color_texture,
                metallic_roughness_texture: MATERIAL_TEXTURE_NONE,
                normal_texture: MATERIAL_TEXTURE_NONE,
                occlusion_texture: MATERIAL_TEXTURE_NONE,
                emissive_texture: MATERIAL_TEXTURE_NONE,
                roughness_factor: 0.7,
                metallic_factor: 0.0,
                occlusion_strength: 1.0,
                normal_scale: 1.0,
            },
            Material3D::Custom(params) => params.surface,
        }
    }
}

impl Default for Camera3DState {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            projection: CameraProjectionState::default(),
            render_mask: BitMask::NONE,
            post_processing: Arc::from([]),
            audio_options: AudioListenerOptions::new(),
        }
    }
}

impl Default for CameraProjectionState {
    fn default() -> Self {
        Self::Perspective {
            fov_y_degrees: 60.0,
            near: 0.1,
            far: 1_000_000.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeMeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub joints: [u16; 4],
    pub weights: UnitVector4,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeMeshBlendShapeVertex {
    pub position_delta: [f32; 3],
    pub normal_delta: [f32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeMeshBlendShape {
    pub vertices: Vec<RuntimeMeshBlendShapeVertex>,
    pub has_normal_deltas: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeshSurfaceRange {
    pub index_start: u32,
    pub index_count: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mesh3D {
    pub vertices: Vec<RuntimeMeshVertex>,
    pub indices: Vec<u32>,
    pub surface_ranges: Vec<MeshSurfaceRange>,
    pub blend_shapes: Vec<RuntimeMeshBlendShape>,
}

pub type RuntimeMeshDataSnapshot = Mesh3D;
pub type RuntimeMeshData = Mesh3D;

#[derive(Debug, Clone, PartialEq)]
pub struct SkeletonPalette {
    pub matrices: Arc<[[[f32; 4]; 4]]>,
}

pub type MaterialParamOverrideValue3D = perro_structs::ConstParamValue;

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialParamOverride3D {
    pub name: Cow<'static, str>,
    pub value: MaterialParamOverrideValue3D,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeshSurfaceBinding3D {
    pub material: Option<MaterialID>,
    pub overrides: Arc<[MaterialParamOverride3D]>,
    pub modulate: Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshBlendOptions3D {
    pub enabled: bool,
    pub screen_blending: bool,
    pub normal_blending: bool,
    pub blend_layers: BitMask,
    pub blend_mask: BitMask,
    pub distance: f32,
    pub min_distance: f32,
    pub noise_factor: f32,
    pub noise_scale: f32,
}

impl MeshBlendOptions3D {
    pub const fn new() -> Self {
        Self {
            enabled: false,
            screen_blending: true,
            normal_blending: false,
            blend_layers: BitMask::ALL,
            blend_mask: BitMask::NONE,
            distance: 0.6,
            min_distance: 0.03,
            noise_factor: 0.35,
            noise_scale: 14.0,
        }
    }

    #[inline]
    pub const fn active(self) -> bool {
        self.enabled
            && self.blend_layers.bits() != 0
            && self.blend_mask.bits() != BitMask::ALL.bits()
    }
}

impl Default for MeshBlendOptions3D {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LODOptions3D {
    pub min_lod: u8,
    pub max_lod: u8,
}

impl LODOptions3D {
    pub const MIN: u8 = 0;
    pub const LOW: u8 = 1;
    pub const MEDIUM_LOW: u8 = 2;
    pub const MEDIUM: u8 = 3;
    pub const HIGH: u8 = 4;
    pub const MAX: u8 = 5;

    pub const fn new() -> Self {
        Self {
            min_lod: Self::MIN,
            max_lod: Self::MAX,
        }
    }
}

impl Default for LODOptions3D {
    fn default() -> Self {
        Self::new()
    }
}
