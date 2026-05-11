use perro_asset_formats::ptset::{MAGIC as TILESET2D_MAGIC, VERSION as TILESET2D_VERSION};
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
pub use perro_particle_math::Op as ParticleExprOp3D;
pub use perro_particle_math::Op as ParticleExprOp2D;
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

#[derive(Debug, Clone, PartialEq)]
pub struct Camera2DState {
    pub position: [f32; 2],
    pub rotation_radians: f32,
    pub zoom: f32,
    pub post_processing: Arc<[PostProcessEffect]>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiRectState {
    pub center: [f32; 2],
    pub size: [f32; 2],
    pub pivot: [f32; 2],
    pub rotation_radians: f32,
    pub z_index: i32,
}

impl UiRectState {
    pub fn screen_min_max(self, viewport: [f32; 2]) -> ([f32; 2], [f32; 2]) {
        let screen_center = [viewport[0] * 0.5, viewport[1] * 0.5];
        let center = [
            screen_center[0] + self.center[0],
            screen_center[1] - self.center[1],
        ];
        let half = [self.size[0] * 0.5, self.size[1] * 0.5];
        (
            [center[0] - half[0], center[1] - half[1]],
            [center[0] + half[0], center[1] + half[1]],
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiDepthEffectState {
    pub color: [f32; 4],
    pub distance: f32,
    pub falloff: f32,
    pub vector: [f32; 2],
    pub size: f32,
}

impl UiDepthEffectState {
    pub const fn none() -> Self {
        Self {
            color: [0.0, 0.0, 0.0, 0.0],
            distance: 0.0,
            falloff: 0.0,
            vector: [0.0, -1.0],
            size: 1.0,
        }
    }
}

impl Default for UiDepthEffectState {
    fn default() -> Self {
        Self::none()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiCommand {
    UpsertPanel {
        node: NodeID,
        rect: UiRectState,
        clip_rect: [f32; 4],
        fill: [f32; 4],
        stroke: [f32; 4],
        stroke_width: f32,
        corner_radius: f32,
        shadow: UiDepthEffectState,
        highlight: UiDepthEffectState,
    },
    UpsertButton {
        node: NodeID,
        rect: UiRectState,
        clip_rect: [f32; 4],
        fill: [f32; 4],
        stroke: [f32; 4],
        stroke_width: f32,
        corner_radius: f32,
        shadow: UiDepthEffectState,
        highlight: UiDepthEffectState,
        disabled: bool,
    },
    UpsertLabel {
        node: NodeID,
        rect: UiRectState,
        clip_rect: [f32; 4],
        text: Cow<'static, str>,
        color: [f32; 4],
        font_size: f32,
        h_align: UiTextAlignState,
        v_align: UiTextAlignState,
    },
    UpsertImage {
        node: NodeID,
        rect: UiRectState,
        clip_rect: [f32; 4],
        texture: TextureID,
        tint: [f32; 4],
        uv_min: [f32; 2],
        uv_max: [f32; 2],
        scale_mode: UiImageScaleState,
        h_align: UiTextAlignState,
        v_align: UiTextAlignState,
        aspect_ratio: f32,
    },
    UpsertTextEdit {
        node: NodeID,
        rect: UiRectState,
        clip_rect: [f32; 4],
        fill: [f32; 4],
        stroke: [f32; 4],
        stroke_width: f32,
        corner_radius: f32,
        shadow: UiDepthEffectState,
        highlight: UiDepthEffectState,
        text: Cow<'static, str>,
        placeholder: Cow<'static, str>,
        color: [f32; 4],
        placeholder_color: [f32; 4],
        selection_color: [f32; 4],
        caret_color: [f32; 4],
        font_size: f32,
        padding: [f32; 4],
        scroll: [f32; 2],
        caret: usize,
        anchor: usize,
        focused: bool,
        multiline: bool,
    },
    RemoveNode {
        node: NodeID,
    },
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum UiTextAlignState {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum UiImageScaleState {
    #[default]
    Stretch,
    Fit,
    Cover,
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
    pub color_start: [f32; 4],
    pub color_end: [f32; 4],
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
            color_start: [1.0, 1.0, 1.0, 1.0],
            color_end: [1.0, 0.4, 0.1, 0.0],
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
    pub color_start: [f32; 4],
    pub color_end: [f32; 4],
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

#[derive(Debug, Clone, PartialEq)]
pub struct DrawShape2DCommand {
    pub shape: DrawShape2D,
    pub position: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprite2DCommand {
    pub texture: TextureID,
    pub model: [[f32; 3]; 3],
    pub tint: [f32; 4],
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub size: [f32; 2],
    pub z_index: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointLight2DState {
    pub position: [f32; 2],
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub z_index: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmbientLight2DState {
    pub color: [f32; 3],
    pub intensity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayLight2DState {
    pub direction: [f32; 2],
    pub color: [f32; 3],
    pub intensity: f32,
    pub z_index: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpotLight2DState {
    pub position: [f32; 2],
    pub direction: [f32; 2],
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub inner_angle_radians: f32,
    pub outer_angle_radians: f32,
    pub z_index: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Light2DState {
    Ambient(AmbientLight2DState),
    Ray(RayLight2DState),
    Point(PointLight2DState),
    Spot(SpotLight2DState),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TileMap2DCommand {
    pub texture: TextureID,
    pub sprites: Arc<[Sprite2DCommand]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TileSet2D {
    pub texture: Cow<'static, str>,
    pub tile_size: [f32; 2],
    pub columns: u32,
    pub rows: u32,
    pub tiles: Cow<'static, [TileSetTile2D]>,
}

impl TileSet2D {
    pub const fn empty() -> Self {
        Self {
            texture: Cow::Borrowed(""),
            tile_size: [0.0, 0.0],
            columns: 0,
            rows: 0,
            tiles: Cow::Borrowed(&[]),
        }
    }

    pub fn tile(&self, id: i32) -> Option<&TileSetTile2D> {
        self.tiles.iter().find(|tile| tile.id == id)
    }

    pub fn is_empty(&self) -> bool {
        self.texture.is_empty() || self.tile_size[0] <= 0.0 || self.tile_size[1] <= 0.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TileSetTile2D {
    pub id: i32,
    pub atlas: [u32; 2],
    pub collision: bool,
    pub collision_shape: TileSetCollisionShape2D,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TileSetCollisionShape2D {
    Auto,
    Shape {
        shape: TileSetShape2D,
        offset: [f32; 2],
    },
    Polygon {
        points: Cow<'static, [perro_structs::Vector2]>,
        offset: [f32; 2],
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TileSetShape2D {
    Rect { width: f32, height: f32 },
    Circle { radius: f32 },
    Triangle { width: f32, height: f32 },
}

pub fn encode_tileset_2d_binary(tileset: &TileSet2D) -> Vec<u8> {
    let texture = tileset.texture.as_ref().as_bytes();
    let mut out = Vec::with_capacity(32 + texture.len() + tileset.tiles.len() * 32);
    out.extend_from_slice(TILESET2D_MAGIC);
    write_u32(&mut out, TILESET2D_VERSION);
    write_u32(&mut out, texture.len() as u32);
    out.extend_from_slice(texture);
    write_f32(&mut out, tileset.tile_size[0]);
    write_f32(&mut out, tileset.tile_size[1]);
    write_u32(&mut out, tileset.columns);
    write_u32(&mut out, tileset.rows);
    write_u32(&mut out, tileset.tiles.len() as u32);
    for tile in tileset.tiles.iter() {
        write_i32(&mut out, tile.id);
        write_u32(&mut out, tile.atlas[0]);
        write_u32(&mut out, tile.atlas[1]);
        out.push(u8::from(tile.collision));
        encode_tileset_collision_shape(&mut out, &tile.collision_shape);
    }
    out
}

pub fn decode_tileset_2d_binary(bytes: &[u8]) -> Option<TileSet2D> {
    let mut cursor = 0usize;
    if read_bytes(bytes, &mut cursor, TILESET2D_MAGIC.len())? != TILESET2D_MAGIC {
        return None;
    }
    let version = read_u32(bytes, &mut cursor)?;
    if version != TILESET2D_VERSION {
        return None;
    }
    let texture_len = read_u32(bytes, &mut cursor)? as usize;
    let texture = std::str::from_utf8(read_bytes(bytes, &mut cursor, texture_len)?)
        .ok()?
        .to_string();
    let tile_size = [read_f32(bytes, &mut cursor)?, read_f32(bytes, &mut cursor)?];
    let columns = read_u32(bytes, &mut cursor)?;
    let rows = read_u32(bytes, &mut cursor)?;
    let tile_count = read_u32(bytes, &mut cursor)? as usize;
    let mut tiles = Vec::with_capacity(tile_count);
    for _ in 0..tile_count {
        let id = read_i32(bytes, &mut cursor)?;
        let atlas = [read_u32(bytes, &mut cursor)?, read_u32(bytes, &mut cursor)?];
        let collision = read_u8(bytes, &mut cursor)? != 0;
        let collision_shape = decode_tileset_collision_shape(bytes, &mut cursor)?;
        tiles.push(TileSetTile2D {
            id,
            atlas,
            collision,
            collision_shape,
        });
    }
    if cursor != bytes.len() || texture.is_empty() || tile_size[0] <= 0.0 || tile_size[1] <= 0.0 {
        return None;
    }
    Some(TileSet2D {
        texture: Cow::Owned(texture),
        tile_size,
        columns,
        rows,
        tiles: Cow::Owned(tiles),
    })
}

fn encode_tileset_collision_shape(out: &mut Vec<u8>, shape: &TileSetCollisionShape2D) {
    match shape {
        TileSetCollisionShape2D::Auto => out.push(0),
        TileSetCollisionShape2D::Shape { shape, offset } => {
            out.push(1);
            match *shape {
                TileSetShape2D::Rect { width, height } => {
                    out.push(0);
                    write_f32(out, width);
                    write_f32(out, height);
                }
                TileSetShape2D::Circle { radius } => {
                    out.push(1);
                    write_f32(out, radius);
                }
                TileSetShape2D::Triangle { width, height } => {
                    out.push(2);
                    write_f32(out, width);
                    write_f32(out, height);
                }
            }
            write_f32(out, offset[0]);
            write_f32(out, offset[1]);
        }
        TileSetCollisionShape2D::Polygon { points, offset } => {
            out.push(2);
            write_u32(out, points.len() as u32);
            for point in points.iter() {
                write_f32(out, point.x);
                write_f32(out, point.y);
            }
            write_f32(out, offset[0]);
            write_f32(out, offset[1]);
        }
    }
}

fn decode_tileset_collision_shape(
    bytes: &[u8],
    cursor: &mut usize,
) -> Option<TileSetCollisionShape2D> {
    match read_u8(bytes, cursor)? {
        0 => Some(TileSetCollisionShape2D::Auto),
        1 => {
            let shape = match read_u8(bytes, cursor)? {
                0 => TileSetShape2D::Rect {
                    width: read_f32(bytes, cursor)?,
                    height: read_f32(bytes, cursor)?,
                },
                1 => TileSetShape2D::Circle {
                    radius: read_f32(bytes, cursor)?,
                },
                2 => TileSetShape2D::Triangle {
                    width: read_f32(bytes, cursor)?,
                    height: read_f32(bytes, cursor)?,
                },
                _ => return None,
            };
            let offset = [read_f32(bytes, cursor)?, read_f32(bytes, cursor)?];
            Some(TileSetCollisionShape2D::Shape { shape, offset })
        }
        2 => {
            let count = read_u32(bytes, cursor)? as usize;
            let mut points = Vec::with_capacity(count);
            for _ in 0..count {
                points.push(perro_structs::Vector2::new(
                    read_f32(bytes, cursor)?,
                    read_f32(bytes, cursor)?,
                ));
            }
            if points.len() < 3 {
                return None;
            }
            let offset = [read_f32(bytes, cursor)?, read_f32(bytes, cursor)?];
            Some(TileSetCollisionShape2D::Polygon {
                points: Cow::Owned(points),
                offset,
            })
        }
        _ => None,
    }
}

pub fn parse_ptileset_source(source: &str) -> Option<TileSet2D> {
    let mut texture = String::new();
    let mut tile_size = [0.0, 0.0];
    let mut columns = 0u32;
    let mut rows = 0u32;
    let mut tiles = Vec::new();
    let compact = source.replace('\n', " ");
    for raw in source.lines() {
        let line = raw.trim();
        if line.starts_with("texture") {
            texture = parse_quoted_value(line)?;
        } else if line.starts_with("tile_size") {
            tile_size = parse_vec2_u32(line).map(|v| [v[0] as f32, v[1] as f32])?;
        } else if line.starts_with("columns") {
            columns = parse_u32_after_eq(line)?;
        } else if line.starts_with("rows") {
            rows = parse_u32_after_eq(line)?;
        }
    }
    for object in extract_braced_objects(&compact) {
        let id = find_i32_field(object, "id")?;
        let atlas = find_vec2_field(object, "atlas")?;
        let collision = find_bool_field(object, "collision").unwrap_or(false);
        let collision_shape =
            find_collision_shape_field(object).unwrap_or(TileSetCollisionShape2D::Auto);
        tiles.push(TileSetTile2D {
            id,
            atlas,
            collision,
            collision_shape,
        });
    }
    if texture.is_empty() || tile_size[0] <= 0.0 || tile_size[1] <= 0.0 {
        return None;
    }
    tiles.sort_by_key(|tile| tile.id);
    Some(TileSet2D {
        texture: Cow::Owned(texture),
        tile_size,
        columns,
        rows,
        tiles: Cow::Owned(tiles),
    })
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_f32(out: &mut Vec<u8>, value: f32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn read_bytes<'a>(bytes: &'a [u8], cursor: &mut usize, len: usize) -> Option<&'a [u8]> {
    let end = cursor.checked_add(len)?;
    let out = bytes.get(*cursor..end)?;
    *cursor = end;
    Some(out)
}

fn read_u8(bytes: &[u8], cursor: &mut usize) -> Option<u8> {
    let value = *bytes.get(*cursor)?;
    *cursor += 1;
    Some(value)
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Option<u32> {
    let raw: [u8; 4] = read_bytes(bytes, cursor, 4)?.try_into().ok()?;
    Some(u32::from_le_bytes(raw))
}

fn read_i32(bytes: &[u8], cursor: &mut usize) -> Option<i32> {
    let raw: [u8; 4] = read_bytes(bytes, cursor, 4)?.try_into().ok()?;
    Some(i32::from_le_bytes(raw))
}

fn read_f32(bytes: &[u8], cursor: &mut usize) -> Option<f32> {
    let raw: [u8; 4] = read_bytes(bytes, cursor, 4)?.try_into().ok()?;
    Some(f32::from_le_bytes(raw))
}

fn extract_braced_objects(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = None;
    for (idx, ch) in text.char_indices() {
        match ch {
            '{' => {
                if depth == 0 {
                    start = Some(idx + ch.len_utf8());
                }
                depth += 1;
            }
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0
                    && let Some(start_idx) = start.take()
                {
                    out.push(&text[start_idx..idx]);
                }
            }
            _ => {}
        }
    }
    out
}

fn parse_quoted_value(line: &str) -> Option<String> {
    let (_, rest) = line.split_once('=')?;
    let rest = rest.trim();
    Some(rest.strip_prefix('"')?.split('"').next()?.to_string())
}

fn parse_u32_after_eq(line: &str) -> Option<u32> {
    line.split_once('=')?.1.trim().parse().ok()
}

fn parse_vec2_u32(line: &str) -> Option<[u32; 2]> {
    let (_, rest) = line.split_once('=')?;
    parse_vec2_inner(rest)
}

fn find_i32_field(text: &str, key: &str) -> Option<i32> {
    let rest = text.split(key).nth(1)?.split_once('=')?.1.trim();
    rest.split(|c: char| c == ',' || c.is_whitespace())
        .find(|v| !v.is_empty())?
        .parse()
        .ok()
}

fn find_bool_field(text: &str, key: &str) -> Option<bool> {
    let rest = text.split(key).nth(1)?.split_once('=')?.1.trim();
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn find_vec2_field(text: &str, key: &str) -> Option<[u32; 2]> {
    parse_vec2_inner(text.split(key).nth(1)?.split_once('=')?.1)
}

fn find_collision_shape_field(text: &str) -> Option<TileSetCollisionShape2D> {
    let rest = text
        .split("collision_shape")
        .nth(1)?
        .split_once('=')?
        .1
        .trim();
    if rest.starts_with("\"auto\"") || rest.starts_with("auto") {
        return Some(TileSetCollisionShape2D::Auto);
    }
    if let Some(rect) = rest.split("rect").nth(1) {
        let body = rect.split_once('{')?.1.rsplit_once('}')?.0;
        let size = find_vec2_f32_field(body, "size")?;
        let offset = find_vec2_f32_field(body, "offset").unwrap_or([0.0, 0.0]);
        return Some(TileSetCollisionShape2D::Shape {
            shape: TileSetShape2D::Rect {
                width: size[0],
                height: size[1],
            },
            offset,
        });
    }
    if let Some(circle) = rest.split("circle").nth(1) {
        let body = circle.split_once('{')?.1.rsplit_once('}')?.0;
        let radius = find_f32_field(body, "radius")?;
        let offset = find_vec2_f32_field(body, "offset").unwrap_or([0.0, 0.0]);
        return Some(TileSetCollisionShape2D::Shape {
            shape: TileSetShape2D::Circle { radius },
            offset,
        });
    }
    if let Some(triangle) = rest.split("triangle").nth(1) {
        let body = triangle.split_once('{')?.1.rsplit_once('}')?.0;
        let size = find_vec2_f32_field(body, "size").or_else(|| {
            Some([
                find_f32_field(body, "width")?,
                find_f32_field(body, "height")?,
            ])
        })?;
        let offset = find_vec2_f32_field(body, "offset").unwrap_or([0.0, 0.0]);
        return Some(TileSetCollisionShape2D::Shape {
            shape: TileSetShape2D::Triangle {
                width: size[0],
                height: size[1],
            },
            offset,
        });
    }
    if let Some(polygon) = rest.split("polygon").nth(1) {
        let body = polygon.split_once('{')?.1.rsplit_once('}')?.0;
        let points = find_vec2_f32_array_field(body, "points")?;
        let offset = find_vec2_f32_field(body, "offset").unwrap_or([0.0, 0.0]);
        return Some(TileSetCollisionShape2D::Polygon {
            points: Cow::Owned(points),
            offset,
        });
    }
    None
}

fn find_f32_field(text: &str, key: &str) -> Option<f32> {
    let rest = text.split(key).nth(1)?.split_once('=')?.1.trim();
    rest.split(|c: char| c == ',' || c.is_whitespace() || c == '}')
        .find(|v| !v.is_empty())?
        .parse()
        .ok()
}

fn find_vec2_f32_field(text: &str, key: &str) -> Option<[f32; 2]> {
    parse_vec2_f32_inner(text.split(key).nth(1)?.split_once('=')?.1)
}

fn find_vec2_f32_array_field(text: &str, key: &str) -> Option<Vec<perro_structs::Vector2>> {
    let rest = text.split(key).nth(1)?.split_once('=')?.1.trim();
    let inner = rest.strip_prefix('[')?.split_once(']')?.0;
    let mut points = Vec::new();
    for raw in inner.split(')').filter(|part| part.contains('(')) {
        let pair = raw.rsplit_once('(')?.1;
        let mut it = pair.split(',').map(|v| v.trim().parse::<f32>().ok());
        points.push(perro_structs::Vector2::new(it.next()??, it.next()??));
    }
    (points.len() >= 3).then_some(points)
}

fn parse_vec2_f32_inner(text: &str) -> Option<[f32; 2]> {
    let inner = text.trim().strip_prefix('(')?.split_once(')')?.0;
    let mut parts = inner.split(',').map(|v| v.trim().parse::<f32>().ok());
    Some([parts.next()??, parts.next()??])
}

fn parse_vec2_inner(text: &str) -> Option<[u32; 2]> {
    let inner = text.trim().strip_prefix('(')?.split_once(')')?.0;
    let mut parts = inner.split(',').map(|v| v.trim().parse::<u32>().ok());
    Some([parts.next()??, parts.next()??])
}

impl Default for Sprite2DCommand {
    fn default() -> Self {
        Self {
            texture: TextureID::nil(),
            model: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            tint: [1.0, 1.0, 1.0, 1.0],
            uv_min: [0.0, 0.0],
            uv_max: [0.0, 0.0],
            size: [0.0, 0.0],
            z_index: 0,
        }
    }
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
pub struct MeshSurfaceRange {
    pub index_start: u32,
    pub index_count: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mesh3D {
    pub vertices: Vec<RuntimeMeshVertex>,
    pub indices: Vec<u32>,
    pub surface_ranges: Vec<MeshSurfaceRange>,
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
        mesh: Mesh3D,
    },
    WriteMeshData {
        id: MeshID,
        mesh: Mesh3D,
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
    WriteMaterialData {
        id: MaterialID,
        material: Material3D,
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
    UpsertTileMap {
        node: NodeID,
        tilemap: TileMap2DCommand,
    },
    UpsertRect {
        node: NodeID,
        rect: Rect2DCommand,
    },
    UpsertPointParticles {
        node: NodeID,
        particles: Box<PointParticles2DState>,
    },
    SetAmbientLight {
        node: NodeID,
        light: AmbientLight2DState,
    },
    SetRayLight {
        node: NodeID,
        light: RayLight2DState,
    },
    SetPointLight {
        node: NodeID,
        light: PointLight2DState,
    },
    SetSpotLight {
        node: NodeID,
        light: SpotLight2DState,
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
        meshlet_override: Option<bool>,
    },
    DrawMulti {
        mesh: MeshID,
        surfaces: Arc<[MeshSurfaceBinding3D]>,
        node: NodeID,
        instance_mats: Arc<[[[f32; 4]; 4]]>,
        skeleton: Option<SkeletonPalette>,
        meshlet_override: Option<bool>,
    },
    DrawMultiDense {
        mesh: MeshID,
        surfaces: Arc<[MeshSurfaceBinding3D]>,
        node: NodeID,
        node_model: [[f32; 4]; 4],
        instance_scale: f32,
        instances: Arc<[DenseInstancePose3D]>,
        meshlet_override: Option<bool>,
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DenseInstancePose3D {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum RenderCommand {
    Resource(ResourceCommand),
    TwoD(Command2D),
    ThreeD(Box<Command3D>),
    Ui(UiCommand),
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
        mesh: Option<Mesh3D>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_rect_converts_center_origin_y_up_to_screen_rect() {
        let rect = UiRectState {
            center: [300.0, 0.0],
            size: [200.0, 100.0],
            pivot: [0.5, 0.5],
            rotation_radians: 0.0,
            z_index: 0,
        };

        let (min, max) = rect.screen_min_max([800.0, 600.0]);

        assert_eq!(min, [600.0, 250.0]);
        assert_eq!(max, [800.0, 350.0]);
    }

    #[test]
    fn tileset_binary_roundtrip_keeps_collision_shapes() {
        let tileset = parse_ptileset_source(
            r#"
            texture = "res://tiles/world.png"
            tile_size = (16, 16)
            columns = 2
            rows = 1
            tiles = [
                { id = 1 atlas = (0, 0) collision = true collision_shape = "auto" },
                { id = 2 atlas = (1, 0) collision = true collision_shape = { polygon = { points = [(0, 0), (16, 0), (8, 16)] offset = (1, -2) } } },
            ]
            "#,
        )
        .expect("tileset parses");

        let bytes = encode_tileset_2d_binary(&tileset);
        assert_eq!(&bytes[0..5], b"PTSET");
        assert_eq!(u32::from_le_bytes(bytes[5..9].try_into().unwrap()), 1);
        let decoded = decode_tileset_2d_binary(&bytes).expect("tileset decodes");

        assert_eq!(decoded, tileset);
    }
}
