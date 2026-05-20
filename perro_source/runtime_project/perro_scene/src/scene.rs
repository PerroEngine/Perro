use perro_structs::ConstParamValue;
use std::borrow::Cow;

pub type SceneObjectField = (SceneFieldName, SceneValue);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SceneFieldName {
    Position,
    Rotation,
    RotationDeg,
    Scale,
    Visible,
    ZIndex,
    RenderLayers,
    RenderMask,
    PostProcessing,
    Camera,
    Resolution,
    AspectRatio,
    AspectMode,
    Texture,
    TextureRegion,
    FlipX,
    FlipY,
    FlipZ,
    Zoom,
    Active,
    Enabled,
    Shape,
    Trimesh,
    Debug,
    Range,
    Radius,
    InnerAngleRadians,
    OuterAngleRadians,
    Intensity,
    CastShadows,
    Color,
    Mesh,
    Material,
    Materials,
    Model,
    Skeleton,
    BodyA,
    BodyB,
    AnchorA,
    AnchorB,
    CollideConnected,
    Animation,
    Animations,
    CurrentAnimation,
    CurrentFrame,
    Bindings,
    FpsScale,
    Playing,
    Looping,
    Prewarm,
    SpawnRate,
    Seed,
    Params,
    Profile,
    SimMode,
    RenderMode,
    Tree,
    Speed,
    Paused,
    Playback,
    Text,
    Placeholder,
    Hint,
    Style,
    Image,
    Source,
    Src,
    Audio,
    AudioMask,
    AudioOptions,
    CollisionLayers,
    CollisionMask,
    CollisionEnabled,
    Friction,
    Restitution,
    Density,
    Mass,
    ContinuousCollisionDetection,
    LinearVelocity,
    AngularVelocity,
    GravityScale,
    LinearDamping,
    AngularDamping,
    CanSleep,
    LockRotation,
    Anchor,
    PositionRatio,
    SizeRatio,
    MinSizeRatio,
    Size,
    Width,
    Height,
    EmptyTile,
    Tiles,
    Tileset,
    Surfaces,
    Meshlets,
    MinLod,
    MaxLod,
    Blend,
    BlendEnabled,
    BlendScreen,
    BlendNormals,
    BlendLayers,
    BlendMask,
    BlendDistance,
    BlendMinDistance,
    BoneIndex,
    ChainLength,
    Iterations,
    Tolerance,
    Weight,
    MatchRotation,
    Solver,
    Gravity,
    Damping,
    Stiffness,
    Collisions,
    MinDistance,
    MaxDistance,
    Axis,
    Projection,
    PerspectiveFovYDegrees,
    PerspectiveNear,
    PerspectiveFar,
    OrthographicSize,
    OrthographicNear,
    OrthographicFar,
    FrustumLeft,
    FrustumRight,
    FrustumBottom,
    FrustumTop,
    FrustumNear,
    FrustumFar,
    DayColors,
    EveningColors,
    NightColors,
    SkyAngle,
    Time,
    TimeOfDay,
    TimePaused,
    TimeScale,
    CloudSize,
    CloudDensity,
    CloudVariance,
    WindVector,
    StarSize,
    StarScatter,
    StarGleam,
    MoonSize,
    SunSize,
    SkyShader,
    Fill,
    Stroke,
    Custom(Cow<'static, str>),
}

impl SceneFieldName {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Position => "position",
            Self::Rotation => "rotation",
            Self::RotationDeg => "rotation_deg",
            Self::Scale => "scale",
            Self::Visible => "visible",
            Self::ZIndex => "z_index",
            Self::RenderLayers => "render_layers",
            Self::RenderMask => "render_mask",
            Self::PostProcessing => "post_processing",
            Self::Camera => "camera",
            Self::Resolution => "resolution",
            Self::AspectRatio => "aspect_ratio",
            Self::AspectMode => "aspect_mode",
            Self::Texture => "texture",
            Self::TextureRegion => "texture_region",
            Self::FlipX => "flip_x",
            Self::FlipY => "flip_y",
            Self::FlipZ => "flip_z",
            Self::Zoom => "zoom",
            Self::Active => "active",
            Self::Enabled => "enabled",
            Self::Shape => "shape",
            Self::Trimesh => "trimesh",
            Self::Debug => "debug",
            Self::Range => "range",
            Self::Radius => "radius",
            Self::InnerAngleRadians => "inner_angle_radians",
            Self::OuterAngleRadians => "outer_angle_radians",
            Self::Intensity => "intensity",
            Self::CastShadows => "cast_shadows",
            Self::Color => "color",
            Self::Mesh => "mesh",
            Self::Material => "material",
            Self::Materials => "materials",
            Self::Model => "model",
            Self::Skeleton => "skeleton",
            Self::BodyA => "body_a",
            Self::BodyB => "body_b",
            Self::AnchorA => "anchor_a",
            Self::AnchorB => "anchor_b",
            Self::CollideConnected => "collide_connected",
            Self::Animation => "animation",
            Self::Animations => "animations",
            Self::CurrentAnimation => "current_animation",
            Self::CurrentFrame => "current_frame",
            Self::Bindings => "bindings",
            Self::FpsScale => "fps_scale",
            Self::Playing => "playing",
            Self::Looping => "looping",
            Self::Prewarm => "prewarm",
            Self::SpawnRate => "spawn_rate",
            Self::Seed => "seed",
            Self::Params => "params",
            Self::Profile => "profile",
            Self::SimMode => "sim_mode",
            Self::RenderMode => "render_mode",
            Self::Tree => "tree",
            Self::Speed => "speed",
            Self::Paused => "paused",
            Self::Playback => "playback",
            Self::Text => "text",
            Self::Placeholder => "placeholder",
            Self::Hint => "hint",
            Self::Style => "style",
            Self::Image => "image",
            Self::Source => "source",
            Self::Src => "src",
            Self::Audio => "audio",
            Self::AudioMask => "audio_mask",
            Self::AudioOptions => "audio_options",
            Self::CollisionLayers => "collision_layers",
            Self::CollisionMask => "collision_mask",
            Self::CollisionEnabled => "collision_enabled",
            Self::Friction => "friction",
            Self::Restitution => "restitution",
            Self::Density => "density",
            Self::Mass => "mass",
            Self::ContinuousCollisionDetection => "continuous_collision_detection",
            Self::LinearVelocity => "linear_velocity",
            Self::AngularVelocity => "angular_velocity",
            Self::GravityScale => "gravity_scale",
            Self::LinearDamping => "linear_damping",
            Self::AngularDamping => "angular_damping",
            Self::CanSleep => "can_sleep",
            Self::LockRotation => "lock_rotation",
            Self::Anchor => "anchor",
            Self::PositionRatio => "position_ratio",
            Self::SizeRatio => "size_ratio",
            Self::MinSizeRatio => "min_size_ratio",
            Self::Size => "size",
            Self::Width => "width",
            Self::Height => "height",
            Self::EmptyTile => "empty_tile",
            Self::Tiles => "tiles",
            Self::Tileset => "tileset",
            Self::Surfaces => "surfaces",
            Self::Meshlets => "meshlets",
            Self::Blend => "blend",
            Self::BlendEnabled => "blend_enabled",
            Self::BlendScreen => "blend_screen",
            Self::BlendNormals => "blend_normals",
            Self::BlendLayers => "blend_layers",
            Self::BlendMask => "blend_mask",
            Self::BlendDistance => "blend_distance",
            Self::BlendMinDistance => "blend_min_distance",
            Self::MinLod => "min_lod",
            Self::MaxLod => "max_lod",
            Self::BoneIndex => "bone_index",
            Self::ChainLength => "chain_length",
            Self::Iterations => "iterations",
            Self::Tolerance => "tolerance",
            Self::Weight => "weight",
            Self::MatchRotation => "match_rotation",
            Self::Solver => "solver",
            Self::Gravity => "gravity",
            Self::Damping => "damping",
            Self::Stiffness => "stiffness",
            Self::Collisions => "collisions",
            Self::MinDistance => "min_distance",
            Self::MaxDistance => "max_distance",
            Self::Axis => "axis",
            Self::Projection => "projection",
            Self::PerspectiveFovYDegrees => "perspective_fov_y_degrees",
            Self::PerspectiveNear => "perspective_near",
            Self::PerspectiveFar => "perspective_far",
            Self::OrthographicSize => "orthographic_size",
            Self::OrthographicNear => "orthographic_near",
            Self::OrthographicFar => "orthographic_far",
            Self::FrustumLeft => "frustum_left",
            Self::FrustumRight => "frustum_right",
            Self::FrustumBottom => "frustum_bottom",
            Self::FrustumTop => "frustum_top",
            Self::FrustumNear => "frustum_near",
            Self::FrustumFar => "frustum_far",
            Self::DayColors => "day_colors",
            Self::EveningColors => "evening_colors",
            Self::NightColors => "night_colors",
            Self::SkyAngle => "sky_angle",
            Self::Time => "time",
            Self::TimeOfDay => "time_of_day",
            Self::TimePaused => "time_paused",
            Self::TimeScale => "time_scale",
            Self::CloudSize => "cloud_size",
            Self::CloudDensity => "cloud_density",
            Self::CloudVariance => "cloud_variance",
            Self::WindVector => "wind_vector",
            Self::StarSize => "star_size",
            Self::StarScatter => "star_scatter",
            Self::StarGleam => "star_gleam",
            Self::MoonSize => "moon_size",
            Self::SunSize => "sun_size",
            Self::SkyShader => "sky_shader",
            Self::Fill => "fill",
            Self::Stroke => "stroke",
            Self::Custom(v) => v.as_ref(),
        }
    }

    pub fn from_name(name: impl Into<Cow<'static, str>>) -> Self {
        let name = name.into();
        if let Some(field) = Self::from_borrowed(name.as_ref()) {
            return field;
        }
        Self::Custom(name)
    }

    pub fn from_borrowed(name: &str) -> Option<Self> {
        Some(match name {
            "position" => Self::Position,
            "rotation" => Self::Rotation,
            "rotation_deg" => Self::RotationDeg,
            "scale" => Self::Scale,
            "visible" => Self::Visible,
            "z_index" => Self::ZIndex,
            "render_layers" => Self::RenderLayers,
            "render_mask" => Self::RenderMask,
            "post_processing" => Self::PostProcessing,
            "camera" | "camera_id" | "source_camera" => Self::Camera,
            "resolution" => Self::Resolution,
            "aspect_ratio" => Self::AspectRatio,
            "aspect_mode" => Self::AspectMode,
            "texture" => Self::Texture,
            "texture_region" => Self::TextureRegion,
            "flip_x" | "flip_h" | "mirror_x" => Self::FlipX,
            "flip_y" | "flip_v" | "mirror_y" => Self::FlipY,
            "flip_z" | "mirror_z" => Self::FlipZ,
            "zoom" => Self::Zoom,
            "active" => Self::Active,
            "enabled" => Self::Enabled,
            "shape" => Self::Shape,
            "trimesh" => Self::Trimesh,
            "debug" => Self::Debug,
            "range" => Self::Range,
            "radius" => Self::Radius,
            "inner_angle_radians" => Self::InnerAngleRadians,
            "outer_angle_radians" => Self::OuterAngleRadians,
            "intensity" => Self::Intensity,
            "cast_shadows" => Self::CastShadows,
            "color" => Self::Color,
            "mesh" => Self::Mesh,
            "material" => Self::Material,
            "materials" => Self::Materials,
            "model" => Self::Model,
            "skeleton" => Self::Skeleton,
            "body_a" => Self::BodyA,
            "body_b" => Self::BodyB,
            "anchor_a" => Self::AnchorA,
            "anchor_b" => Self::AnchorB,
            "collide_connected" => Self::CollideConnected,
            "animation" => Self::Animation,
            "animations" => Self::Animations,
            "current_animation" => Self::CurrentAnimation,
            "current_frame" => Self::CurrentFrame,
            "bindings" => Self::Bindings,
            "fps_scale" => Self::FpsScale,
            "playing" => Self::Playing,
            "looping" => Self::Looping,
            "prewarm" => Self::Prewarm,
            "spawn_rate" => Self::SpawnRate,
            "seed" => Self::Seed,
            "params" => Self::Params,
            "profile" => Self::Profile,
            "sim_mode" => Self::SimMode,
            "render_mode" => Self::RenderMode,
            "tree" => Self::Tree,
            "speed" => Self::Speed,
            "paused" => Self::Paused,
            "playback" => Self::Playback,
            "text" => Self::Text,
            "placeholder" => Self::Placeholder,
            "hint" => Self::Hint,
            "style" => Self::Style,
            "image" => Self::Image,
            "source" => Self::Source,
            "src" => Self::Src,
            "audio" => Self::Audio,
            "audio_mask" => Self::AudioMask,
            "audio_options" => Self::AudioOptions,
            "collision_layers" => Self::CollisionLayers,
            "collision_mask" => Self::CollisionMask,
            "collision_enabled" => Self::CollisionEnabled,
            "friction" => Self::Friction,
            "restitution" => Self::Restitution,
            "density" => Self::Density,
            "mass" => Self::Mass,
            "continuous_collision_detection" => Self::ContinuousCollisionDetection,
            "linear_velocity" => Self::LinearVelocity,
            "angular_velocity" => Self::AngularVelocity,
            "gravity_scale" => Self::GravityScale,
            "linear_damping" => Self::LinearDamping,
            "angular_damping" => Self::AngularDamping,
            "can_sleep" => Self::CanSleep,
            "lock_rotation" => Self::LockRotation,
            "anchor" => Self::Anchor,
            "position_ratio" => Self::PositionRatio,
            "size_ratio" => Self::SizeRatio,
            "min_size_ratio" => Self::MinSizeRatio,
            "size" => Self::Size,
            "width" => Self::Width,
            "height" => Self::Height,
            "empty_tile" => Self::EmptyTile,
            "tiles" => Self::Tiles,
            "tileset" => Self::Tileset,
            "surfaces" => Self::Surfaces,
            "meshlets" => Self::Meshlets,
            "min_lod" => Self::MinLod,
            "max_lod" => Self::MaxLod,
            "blend" | "mesh_blend" | "blending" => Self::Blend,
            "blend_enabled" => Self::BlendEnabled,
            "blend_screen" => Self::BlendScreen,
            "blend_normals" => Self::BlendNormals,
            "blend_layers" => Self::BlendLayers,
            "blend_mask" => Self::BlendMask,
            "blend_distance" | "blend_size" => Self::BlendDistance,
            "blend_min_distance" | "blend_min_size" => Self::BlendMinDistance,
            "bone_index" => Self::BoneIndex,
            "chain_length" => Self::ChainLength,
            "iterations" => Self::Iterations,
            "tolerance" => Self::Tolerance,
            "weight" => Self::Weight,
            "match_rotation" => Self::MatchRotation,
            "solver" => Self::Solver,
            "gravity" => Self::Gravity,
            "damping" => Self::Damping,
            "stiffness" => Self::Stiffness,
            "collisions" => Self::Collisions,
            "min_distance" => Self::MinDistance,
            "max_distance" => Self::MaxDistance,
            "axis" => Self::Axis,
            "projection" => Self::Projection,
            "perspective_fov_y_degrees" => Self::PerspectiveFovYDegrees,
            "perspective_near" => Self::PerspectiveNear,
            "perspective_far" => Self::PerspectiveFar,
            "orthographic_size" => Self::OrthographicSize,
            "orthographic_near" => Self::OrthographicNear,
            "orthographic_far" => Self::OrthographicFar,
            "frustum_left" => Self::FrustumLeft,
            "frustum_right" => Self::FrustumRight,
            "frustum_bottom" => Self::FrustumBottom,
            "frustum_top" => Self::FrustumTop,
            "frustum_near" => Self::FrustumNear,
            "frustum_far" => Self::FrustumFar,
            "day_colors" => Self::DayColors,
            "evening_colors" => Self::EveningColors,
            "night_colors" => Self::NightColors,
            "sky_angle" => Self::SkyAngle,
            "time" => Self::Time,
            "time_of_day" => Self::TimeOfDay,
            "time_paused" => Self::TimePaused,
            "time_scale" => Self::TimeScale,
            "cloud_size" => Self::CloudSize,
            "cloud_density" => Self::CloudDensity,
            "cloud_variance" => Self::CloudVariance,
            "wind_vector" => Self::WindVector,
            "star_size" => Self::StarSize,
            "star_scatter" => Self::StarScatter,
            "star_gleam" => Self::StarGleam,
            "moon_size" => Self::MoonSize,
            "sun_size" => Self::SunSize,
            "sky_shader" => Self::SkyShader,
            "fill" => Self::Fill,
            "stroke" => Self::Stroke,
            _ => return None,
        })
    }
}

impl AsRef<str> for SceneFieldName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for SceneFieldName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::ops::Deref for SceneFieldName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl PartialEq<str> for SceneFieldName {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for SceneFieldName {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl From<&'static str> for SceneFieldName {
    fn from(value: &'static str) -> Self {
        Self::from_name(Cow::Borrowed(value))
    }
}

impl From<String> for SceneFieldName {
    fn from(value: String) -> Self {
        Self::from_name(Cow::Owned(value))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SceneValueKey(pub Cow<'static, str>);

impl AsRef<str> for SceneValueKey {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl std::fmt::Display for SceneValueKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_ref())
    }
}

impl From<&'static str> for SceneValueKey {
    fn from(value: &'static str) -> Self {
        Self(Cow::Borrowed(value))
    }
}

impl From<String> for SceneValueKey {
    fn from(value: String) -> Self {
        Self(Cow::Owned(value))
    }
}

#[derive(Clone, Debug)]
pub enum SceneValue {
    Bool(bool),
    I32(i32),
    F32(f32),
    Vec2 { x: f32, y: f32 },
    Vec3 { x: f32, y: f32, z: f32 },
    Vec4 { x: f32, y: f32, z: f32, w: f32 },
    Str(Cow<'static, str>),
    Hashed(u64),
    Key(SceneValueKey),
    Object(Cow<'static, [SceneObjectField]>),
    Array(Cow<'static, [SceneValue]>),
}

impl SceneValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Self::I32(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Self::F32(v) => Some(*v),
            Self::I32(v) => Some(*v as f32),
            _ => None,
        }
    }

    pub fn as_vec2(&self) -> Option<(f32, f32)> {
        match self {
            Self::Vec2 { x, y } => Some((*x, *y)),
            _ => None,
        }
    }

    pub fn as_vec3(&self) -> Option<(f32, f32, f32)> {
        match self {
            Self::Vec3 { x, y, z } => Some((*x, *y, *z)),
            _ => None,
        }
    }

    pub fn as_vec4(&self) -> Option<(f32, f32, f32, f32)> {
        match self {
            Self::Vec4 { x, y, z, w } => Some((*x, *y, *z, *w)),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(v) => Some(v.as_ref()),
            _ => None,
        }
    }

    pub fn as_hashed(&self) -> Option<u64> {
        match self {
            Self::Hashed(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_key(&self) -> Option<&str> {
        match self {
            Self::Key(v) => Some(v.as_ref()),
            _ => None,
        }
    }

    #[inline]
    pub fn as_const_param(&self) -> Option<ConstParamValue> {
        ConstParamValue::try_from(self).ok()
    }
}

impl TryFrom<&SceneValue> for ConstParamValue {
    type Error = ();

    fn try_from(value: &SceneValue) -> Result<Self, Self::Error> {
        match value {
            SceneValue::Bool(v) => Ok(Self::Bool(*v)),
            SceneValue::I32(v) => Ok(Self::I32(*v)),
            SceneValue::F32(v) => Ok(Self::F32(*v)),
            SceneValue::Vec2 { x, y } => Ok(Self::Vec2([*x, *y])),
            SceneValue::Vec3 { x, y, z } => Ok(Self::Vec3([*x, *y, *z])),
            SceneValue::Vec4 { x, y, z, w } => Ok(Self::Vec4([*x, *y, *z, *w])),
            _ => Err(()),
        }
    }
}

impl From<ConstParamValue> for SceneValue {
    fn from(value: ConstParamValue) -> Self {
        match value {
            ConstParamValue::Bool(v) => Self::Bool(v),
            ConstParamValue::I32(v) => Self::I32(v),
            ConstParamValue::F32(v) => Self::F32(v),
            ConstParamValue::Vec2(v) => Self::Vec2 { x: v[0], y: v[1] },
            ConstParamValue::Vec3(v) => Self::Vec3 {
                x: v[0],
                y: v[1],
                z: v[2],
            },
            ConstParamValue::Vec4(v) => Self::Vec4 {
                x: v[0],
                y: v[1],
                z: v[2],
                w: v[3],
            },
        }
    }
}

#[derive(Clone, Copy)]
pub struct SceneFieldIterRef<'a> {
    fields: &'a [SceneObjectField],
}

impl<'a> SceneFieldIterRef<'a> {
    pub fn new(fields: &'a [SceneObjectField]) -> Self {
        Self { fields }
    }

    pub fn for_each(self, mut f: impl FnMut(&str, &'a SceneValue)) {
        for (name, value) in self.fields {
            f(name.as_ref(), value);
        }
    }

    pub fn for_each_field(self, mut f: impl FnMut(&'a SceneFieldName, &'a SceneValue)) {
        for (name, value) in self.fields {
            f(name, value);
        }
    }
}

#[derive(Debug, Clone)]
pub struct Scene {
    pub nodes: Cow<'static, [SceneNodeEntry]>,
    pub root: Option<SceneKey>,
    pub key_names: Cow<'static, [Cow<'static, str>]>,
}

impl Scene {
    pub fn key_name(&self, key: SceneKey) -> Option<&str> {
        self.key_names.get(key.as_usize()).map(|v| v.as_ref())
    }

    pub fn key_name_or_id(&self, key: SceneKey) -> Cow<'_, str> {
        self.key_name(key)
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(key.as_u32().to_string()))
    }
}

#[derive(Debug, Clone)]
pub struct SceneNodeEntry {
    pub data: SceneNodeData,
    pub has_data_override: bool,
    pub key: SceneKey,
    pub name: Option<Cow<'static, str>>,
    pub tags: Cow<'static, [Cow<'static, str>]>,
    pub children: Cow<'static, [SceneKey]>,
    pub parent: Option<SceneKey>,
    pub script: Option<Cow<'static, str>>,
    pub clear_script: bool,
    pub root_of: Option<Cow<'static, str>>,
    pub script_vars: Cow<'static, [SceneObjectField]>,
}

#[derive(Debug, Clone)]
pub struct SceneNodeData {
    pub ty: Cow<'static, str>,
    pub fields: Cow<'static, [SceneObjectField]>,
    pub base: Option<SceneNodeDataBase>,
}

#[derive(Debug, Clone)]
pub enum SceneNodeDataBase {
    Borrowed(&'static SceneNodeData),
    Owned(Box<SceneNodeData>),
}

impl SceneNodeData {
    pub fn base_ref(&self) -> Option<&SceneNodeData> {
        match &self.base {
            Some(SceneNodeDataBase::Borrowed(v)) => Some(*v),
            Some(SceneNodeDataBase::Owned(v)) => Some(v.as_ref()),
            None => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SceneKey(pub u32);

impl SceneKey {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }

    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}
