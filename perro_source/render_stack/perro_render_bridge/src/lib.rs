use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
pub use perro_particle_math::Op as ParticleExprOp3D;
use perro_structs::{ColorBlindFilter, DrawShape2D, PostProcessEffect, PostProcessSet};
use std::borrow::Cow;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderRequestID(pub u64);

impl RenderRequestID {
    #[inline]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

#[derive(Debug, Clone)]
pub struct Camera2DState {
    pub position: [f32; 2],
    pub rotation_radians: f32,
    pub zoom: f32,
    pub post_processing: Arc<[PostProcessEffect]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Camera3DState {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub projection: CameraProjectionState,
    pub post_processing: Arc<[PostProcessEffect]>,
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

#[derive(Debug, Clone, PartialEq)]
pub struct SkyTime3DState {
    pub time_of_day: f32,
    pub paused: bool,
    pub scale: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sky3DState {
    pub day_colors: Arc<[[f32; 3]]>,
    pub evening_colors: Arc<[[f32; 3]]>,
    pub night_colors: Arc<[[f32; 3]]>,
    pub sky_angle: f32,
    pub time: SkyTime3DState,
    pub cloud_size: f32,
    pub cloud_density: f32,
    pub cloud_variance: f32,
    pub cloud_wind_vector: [f32; 2],
    pub star_size: f32,
    pub star_scatter: f32,
    pub star_gleam: f32,
    pub moon_size: f32,
    pub sun_size: f32,
    pub style_blend: f32,
    pub sky_shader: Option<Cow<'static, str>>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleSimulationMode3D {
    Cpu,
    GpuVertex,
    GpuCompute,
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
    pub color_start: [f32; 4],
    pub color_end: [f32; 4],
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
            color_start: [1.0, 1.0, 1.0, 1.0],
            color_end: [1.0, 0.4, 0.1, 0.0],
            emissive: [0.0, 0.0, 0.0],
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
    pub color_start: [f32; 4],
    pub color_end: [f32; 4],
    pub emissive: [f32; 3],
    pub seed: u32,
    pub params: Vec<f32>,
    pub simulation_time: f32,
    pub simulation_delta: f32,
    pub profile: ParticleProfile3D,
    pub sim_mode: ParticleSimulationMode3D,
    pub render_mode: ParticleRenderMode3D,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StandardMaterial3D {
    pub base_color_factor: [f32; 4],
    pub roughness_factor: f32,
    pub metallic_factor: f32,
    pub occlusion_strength: f32,
    pub emissive_factor: [f32; 3],
    pub alpha_mode: u32, // 0=OPAQUE, 1=MASK, 2=BLEND
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
    pub alpha_mode: u32, // 0=OPAQUE, 1=MASK, 2=BLEND
    pub alpha_cutoff: f32,
    pub double_sided: bool,
    pub flat_shading: bool,
    // Texture slot indices (material-local index or NONE).
    pub base_color_texture: u32,
}

impl Default for UnlitMaterial3D {
    fn default() -> Self {
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
    pub alpha_mode: u32, // 0=OPAQUE, 1=MASK, 2=BLEND
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

#[derive(Debug, Clone, PartialEq)]
pub enum CustomMaterialParamValue3D {
    F32(f32),
    I32(i32),
    Bool(bool),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
}

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
pub struct CustomMaterial3D {
    pub shader_path: Cow<'static, str>,
    pub params: Cow<'static, [CustomMaterialParam3D]>,
}

impl CustomMaterial3D {
    #[inline]
    pub fn new(shader_path: impl Into<Cow<'static, str>>) -> Self {
        Self {
            shader_path: shader_path.into(),
            params: Cow::Borrowed(&[]),
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
        }
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
            Material3D::Custom(_) => StandardMaterial3D::default(),
        }
    }
}

impl Default for Camera3DState {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            projection: CameraProjectionState::default(),
            post_processing: Arc::from([]),
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

impl Default for Camera2DState {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            rotation_radians: 0.0,
            zoom: 1.0,
            post_processing: Arc::from([]),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Rect2DCommand {
    pub center: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
    pub z_index: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawShape2DCommand {
    pub shape: DrawShape2D,
    pub position: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprite2DCommand {
    pub texture: TextureID,
    pub model: [[f32; 3]; 3],
    pub tint: [f32; 4],
    pub z_index: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeMeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub joints: [u16; 4],
    pub weights: [f32; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeMeshData {
    pub vertices: Vec<RuntimeMeshVertex>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkeletonPalette {
    pub matrices: Arc<[[[f32; 4]; 4]]>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MaterialParamOverrideValue3D {
    F32(f32),
    I32(i32),
    Bool(bool),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialParamOverride3D {
    pub name: Cow<'static, str>,
    pub value: MaterialParamOverrideValue3D,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeshSurfaceBinding3D {
    pub material: Option<MaterialID>,
    pub overrides: Arc<[MaterialParamOverride3D]>,
    pub modulate: [f32; 4],
}

#[derive(Debug, Clone)]
pub enum ResourceCommand {
    CreateMesh {
        request: RenderRequestID,
        id: MeshID,
        source: String,
        reserved: bool,
    },
    CreateRuntimeMesh {
        request: RenderRequestID,
        id: MeshID,
        source: String,
        reserved: bool,
        mesh: RuntimeMeshData,
    },
    CreateTexture {
        request: RenderRequestID,
        id: TextureID,
        source: String,
        reserved: bool,
    },
    CreateMaterial {
        request: RenderRequestID,
        id: MaterialID,
        material: Material3D,
        source: Option<String>,
        reserved: bool,
    },
    SetMeshReserved {
        id: MeshID,
        reserved: bool,
    },
    SetTextureReserved {
        id: TextureID,
        reserved: bool,
    },
    SetMaterialReserved {
        id: MaterialID,
        reserved: bool,
    },
    DropMesh {
        id: MeshID,
    },
    DropTexture {
        id: TextureID,
    },
    DropMaterial {
        id: MaterialID,
    },
}

#[derive(Debug, Clone)]
pub enum Command2D {
    UpsertSprite {
        node: NodeID,
        sprite: Sprite2DCommand,
    },
    UpsertRect {
        node: NodeID,
        rect: Rect2DCommand,
    },
    RemoveNode {
        node: NodeID,
    },
    SetCamera {
        camera: Camera2DState,
    },
    DrawShape {
        draw: DrawShape2DCommand,
    },
}

#[derive(Debug, Clone)]
pub enum Command3D {
    Draw {
        mesh: MeshID,
        surfaces: Arc<[MeshSurfaceBinding3D]>,
        node: NodeID,
        model: [[f32; 4]; 4],
        skeleton: Option<SkeletonPalette>,
    },
    DrawDebugPoint3D {
        node: NodeID,
        position: [f32; 3],
        size: f32,
    },
    DrawDebugLine3D {
        node: NodeID,
        start: [f32; 3],
        end: [f32; 3],
        thickness: f32,
    },
    SetCamera {
        camera: Camera3DState,
    },
    SetAmbientLight {
        node: NodeID,
        light: AmbientLight3DState,
    },
    SetSky {
        node: NodeID,
        sky: Box<Sky3DState>,
    },
    SetRayLight {
        node: NodeID,
        light: RayLight3DState,
    },
    SetPointLight {
        node: NodeID,
        light: PointLight3DState,
    },
    SetSpotLight {
        node: NodeID,
        light: SpotLight3DState,
    },
    UpsertPointParticles {
        node: NodeID,
        particles: Box<PointParticles3DState>,
    },
    RemoveNode {
        node: NodeID,
    },
}

#[derive(Debug, Clone)]
pub enum RenderCommand {
    Resource(ResourceCommand),
    TwoD(Command2D),
    ThreeD(Box<Command3D>),
    PostProcessing(PostProcessingCommand),
    VisualAccessibility(VisualAccessibilityCommand),
}

#[derive(Debug, Clone)]
pub enum PostProcessingCommand {
    SetGlobal(PostProcessSet),
    AddGlobalNamed {
        name: Cow<'static, str>,
        effect: PostProcessEffect,
    },
    AddGlobalUnnamed(PostProcessEffect),
    RemoveGlobalByName(Cow<'static, str>),
    RemoveGlobalByIndex(usize),
    ClearGlobal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VisualAccessibilityCommand {
    EnableColorBlind {
        mode: ColorBlindFilter,
        strength: f32,
    },
    DisableColorBlind,
}

#[derive(Debug, Clone)]
pub enum RenderEvent {
    MeshCreated {
        request: RenderRequestID,
        id: MeshID,
    },
    TextureCreated {
        request: RenderRequestID,
        id: TextureID,
    },
    MaterialCreated {
        request: RenderRequestID,
        id: MaterialID,
    },
    Failed {
        request: RenderRequestID,
        reason: String,
    },
}

pub trait RenderBridge {
    fn submit(&mut self, command: RenderCommand);

    fn submit_many<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        for command in commands {
            self.submit(command);
        }
    }

    fn drain_events(&mut self, out: &mut Vec<RenderEvent>);
}
