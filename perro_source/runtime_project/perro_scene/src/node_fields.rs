use perro_nodes::NodeType;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeField {
    Node2D(Node2DField),
    Node3D(Node3DField),
    Camera2D(Camera2DField),
    Sprite2D(Sprite2DField),
    AnimatedSprite2D(AnimatedSprite2DField),
    ParticleEmitter2D(ParticleEmitter2DField),
    CollisionShape2D(CollisionShape2DField),
    StaticBody2D(StaticBody2DField),
    RigidBody2D(RigidBody2DField),
    Area2D(Area2DField),
    MeshInstance3D(MeshInstance3DField),
    Skeleton3D(Skeleton3DField),
    BoneAttachment3D(BoneAttachment3DField),
    IKTarget3D(IKTarget3DField),
    PhysicsBoneChain3D(PhysicsBoneChain3DField),
    BoneCollider3D(BoneCollider3DField),
    Camera3D(Camera3DField),
    ParticleEmitter3D(ParticleEmitter3DField),
    AnimationPlayer(AnimationPlayerField),
    AnimationTree(AnimationTreeField),
    Light3D(Light3DField),
    Sky3D(Sky3DField),
    RayLight3D(RayLight3DField),
    PointLight3D(PointLight3DField),
    SpotLight3D(SpotLight3DField),
    CollisionShape3D(CollisionShape3DField),
    StaticBody3D(StaticBody3DField),
    RigidBody3D(RigidBody3DField),
    Area3D(Area3DField),
    UiImage(UiImageField),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Node2DField {
    Position,
    Rotation,
    Scale,
    Visible,
    ZIndex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Node3DField {
    Position,
    Rotation,
    Scale,
    Visible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Camera2DField {
    Zoom,
    PostProcessing,
    Active,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sprite2DField {
    Texture,
    TextureRegion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimatedSprite2DField {
    Texture,
    Animations,
    CurrentAnimation,
    CurrentFrame,
    FpsScale,
    Playing,
    Looping,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticleEmitter2DField {
    Active,
    Looping,
    Prewarm,
    SpawnRate,
    Seed,
    Params,
    Profile,
    SimMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollisionShape2DField {
    Shape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticBody2DField {
    Enabled,
    Friction,
    Restitution,
    Density,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigidBody2DField {
    Enabled,
    ContinuousCollisionDetection,
    LinearVelocity,
    AngularVelocity,
    GravityScale,
    LinearDamping,
    AngularDamping,
    CanSleep,
    LockRotation,
    Friction,
    Restitution,
    Density,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area2DField {
    Enabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshInstance3DField {
    Mesh,
    Material,
    Surfaces,
    Model,
    Skeleton,
    Meshlets,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skeleton3DField {
    Skeleton,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoneAttachment3DField {
    Skeleton,
    BoneIndex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IKTarget3DField {
    Skeleton,
    BoneIndex,
    ChainLength,
    Iterations,
    Tolerance,
    Weight,
    MatchRotation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicsBoneChain3DField {
    Skeleton,
    BoneIndex,
    ChainLength,
    Enabled,
    Gravity,
    Damping,
    Stiffness,
    Radius,
    Collisions,
    Iterations,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoneCollider3DField {
    Enabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Camera3DField {
    Zoom,
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
    PostProcessing,
    Active,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticleEmitter3DField {
    Active,
    Looping,
    Prewarm,
    SpawnRate,
    Seed,
    Params,
    Profile,
    SimMode,
    RenderMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationPlayerField {
    Animation,
    Bindings,
    Speed,
    Paused,
    Playback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationTreeField {
    Tree,
    Animations,
    Bindings,
    Speed,
    Paused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Light3DField {
    Color,
    Intensity,
    CastShadows,
    Active,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RayLight3DField {
    Visible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sky3DField {
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
    CloudWindVector,
    StarSize,
    StarScatter,
    StarGleam,
    MoonSize,
    SunSize,
    Style,
    SkyShader,
    Active,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointLight3DField {
    Range,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpotLight3DField {
    Range,
    InnerAngleRadians,
    OuterAngleRadians,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollisionShape3DField {
    Shape,
    Trimesh,
    Debug,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticBody3DField {
    Enabled,
    Friction,
    Restitution,
    Density,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigidBody3DField {
    Enabled,
    ContinuousCollisionDetection,
    Mass,
    LinearVelocity,
    AngularVelocity,
    GravityScale,
    LinearDamping,
    AngularDamping,
    CanSleep,
    Friction,
    Restitution,
    Density,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area3DField {
    Enabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiImageField {
    Texture,
    TextureRegion,
}

pub fn resolve_node_field(node_type_name: &str, field: &str) -> Option<NodeField> {
    let node_type = NodeType::from_str(node_type_name).ok()?;

    if let Some(base) = resolve_base_node_field(node_type, field) {
        return Some(base);
    }

    match node_type {
        NodeType::Camera2D => match field {
            "zoom" => Some(NodeField::Camera2D(Camera2DField::Zoom)),
            "post_processing" => Some(NodeField::Camera2D(Camera2DField::PostProcessing)),
            "active" => Some(NodeField::Camera2D(Camera2DField::Active)),
            _ => None,
        },
        NodeType::Sprite2D => match field {
            "texture" => Some(NodeField::Sprite2D(Sprite2DField::Texture)),
            "texture_region" | "region" | "atlas_region" => {
                Some(NodeField::Sprite2D(Sprite2DField::TextureRegion))
            }
            _ => None,
        },
        NodeType::AnimatedSprite2D => match field {
            "texture" => Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::Texture)),
            "animations" | "sprites" => Some(NodeField::AnimatedSprite2D(
                AnimatedSprite2DField::Animations,
            )),
            "current_animation" | "animation" | "clip" => Some(NodeField::AnimatedSprite2D(
                AnimatedSprite2DField::CurrentAnimation,
            )),
            "current_frame" | "frame" => Some(NodeField::AnimatedSprite2D(
                AnimatedSprite2DField::CurrentFrame,
            )),
            "fps_scale" | "speed" => {
                Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::FpsScale))
            }
            "playing" | "play" => Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::Playing)),
            "looping" | "loop" => Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::Looping)),
            _ => None,
        },
        NodeType::ParticleEmitter2D => match field {
            "active" => Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::Active)),
            "looping" => Some(NodeField::ParticleEmitter2D(
                ParticleEmitter2DField::Looping,
            )),
            "prewarm" => Some(NodeField::ParticleEmitter2D(
                ParticleEmitter2DField::Prewarm,
            )),
            "spawn_rate" => Some(NodeField::ParticleEmitter2D(
                ParticleEmitter2DField::SpawnRate,
            )),
            "seed" => Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::Seed)),
            "params" => Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::Params)),
            "profile" => Some(NodeField::ParticleEmitter2D(
                ParticleEmitter2DField::Profile,
            )),
            "sim_mode" => Some(NodeField::ParticleEmitter2D(
                ParticleEmitter2DField::SimMode,
            )),
            _ => None,
        },
        NodeType::CollisionShape2D => match field {
            "shape" => Some(NodeField::CollisionShape2D(CollisionShape2DField::Shape)),
            _ => None,
        },
        NodeType::StaticBody2D => match field {
            "enabled" => Some(NodeField::StaticBody2D(StaticBody2DField::Enabled)),
            "friction" => Some(NodeField::StaticBody2D(StaticBody2DField::Friction)),
            "restitution" => Some(NodeField::StaticBody2D(StaticBody2DField::Restitution)),
            "density" => Some(NodeField::StaticBody2D(StaticBody2DField::Density)),
            _ => None,
        },
        NodeType::RigidBody2D => match field {
            "enabled" => Some(NodeField::RigidBody2D(RigidBody2DField::Enabled)),
            "continuous_collision_detection" | "ccd" => Some(NodeField::RigidBody2D(
                RigidBody2DField::ContinuousCollisionDetection,
            )),
            "linear_velocity" | "velocity" => {
                Some(NodeField::RigidBody2D(RigidBody2DField::LinearVelocity))
            }
            "angular_velocity" => Some(NodeField::RigidBody2D(RigidBody2DField::AngularVelocity)),
            "gravity_scale" => Some(NodeField::RigidBody2D(RigidBody2DField::GravityScale)),
            "linear_damping" => Some(NodeField::RigidBody2D(RigidBody2DField::LinearDamping)),
            "angular_damping" => Some(NodeField::RigidBody2D(RigidBody2DField::AngularDamping)),
            "can_sleep" => Some(NodeField::RigidBody2D(RigidBody2DField::CanSleep)),
            "lock_rotation" => Some(NodeField::RigidBody2D(RigidBody2DField::LockRotation)),
            "friction" => Some(NodeField::RigidBody2D(RigidBody2DField::Friction)),
            "restitution" => Some(NodeField::RigidBody2D(RigidBody2DField::Restitution)),
            "density" => Some(NodeField::RigidBody2D(RigidBody2DField::Density)),
            _ => None,
        },
        NodeType::Area2D => match field {
            "enabled" => Some(NodeField::Area2D(Area2DField::Enabled)),
            _ => None,
        },
        NodeType::MeshInstance3D => match field {
            "mesh" => Some(NodeField::MeshInstance3D(MeshInstance3DField::Mesh)),
            "material" => Some(NodeField::MeshInstance3D(MeshInstance3DField::Material)),
            "surfaces" => Some(NodeField::MeshInstance3D(MeshInstance3DField::Surfaces)),
            "model" => Some(NodeField::MeshInstance3D(MeshInstance3DField::Model)),
            "skeleton" => Some(NodeField::MeshInstance3D(MeshInstance3DField::Skeleton)),
            "meshlets" | "use_meshlets" => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::Meshlets))
            }
            _ => None,
        },
        NodeType::MultiMeshInstance3D => match field {
            "mesh" => Some(NodeField::MeshInstance3D(MeshInstance3DField::Mesh)),
            "material" => Some(NodeField::MeshInstance3D(MeshInstance3DField::Material)),
            "surfaces" => Some(NodeField::MeshInstance3D(MeshInstance3DField::Surfaces)),
            "model" => Some(NodeField::MeshInstance3D(MeshInstance3DField::Model)),
            "meshlets" | "use_meshlets" => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::Meshlets))
            }
            _ => None,
        },
        NodeType::Skeleton3D => match field {
            "skeleton" => Some(NodeField::Skeleton3D(Skeleton3DField::Skeleton)),
            _ => None,
        },
        NodeType::BoneAttachment3D => match field {
            "skeleton" => Some(NodeField::BoneAttachment3D(BoneAttachment3DField::Skeleton)),
            "bone" | "bone_index" => Some(NodeField::BoneAttachment3D(
                BoneAttachment3DField::BoneIndex,
            )),
            _ => None,
        },
        NodeType::IKTarget3D => match field {
            "skeleton" => Some(NodeField::IKTarget3D(IKTarget3DField::Skeleton)),
            "bone" | "bone_index" => Some(NodeField::IKTarget3D(IKTarget3DField::BoneIndex)),
            "chain_length" => Some(NodeField::IKTarget3D(IKTarget3DField::ChainLength)),
            "iterations" => Some(NodeField::IKTarget3D(IKTarget3DField::Iterations)),
            "tolerance" => Some(NodeField::IKTarget3D(IKTarget3DField::Tolerance)),
            "weight" => Some(NodeField::IKTarget3D(IKTarget3DField::Weight)),
            "match_rotation" => Some(NodeField::IKTarget3D(IKTarget3DField::MatchRotation)),
            _ => None,
        },
        NodeType::PhysicsBoneChain3D => match field {
            "skeleton" => Some(NodeField::PhysicsBoneChain3D(
                PhysicsBoneChain3DField::Skeleton,
            )),
            "bone" | "bone_index" => Some(NodeField::PhysicsBoneChain3D(
                PhysicsBoneChain3DField::BoneIndex,
            )),
            "chain_length" => Some(NodeField::PhysicsBoneChain3D(
                PhysicsBoneChain3DField::ChainLength,
            )),
            "enabled" => Some(NodeField::PhysicsBoneChain3D(
                PhysicsBoneChain3DField::Enabled,
            )),
            "gravity" => Some(NodeField::PhysicsBoneChain3D(
                PhysicsBoneChain3DField::Gravity,
            )),
            "damping" => Some(NodeField::PhysicsBoneChain3D(
                PhysicsBoneChain3DField::Damping,
            )),
            "stiffness" => Some(NodeField::PhysicsBoneChain3D(
                PhysicsBoneChain3DField::Stiffness,
            )),
            "radius" => Some(NodeField::PhysicsBoneChain3D(
                PhysicsBoneChain3DField::Radius,
            )),
            "collisions" | "collision" => Some(NodeField::PhysicsBoneChain3D(
                PhysicsBoneChain3DField::Collisions,
            )),
            "iterations" => Some(NodeField::PhysicsBoneChain3D(
                PhysicsBoneChain3DField::Iterations,
            )),
            _ => None,
        },
        NodeType::BoneCollider3D => match field {
            "enabled" => Some(NodeField::BoneCollider3D(BoneCollider3DField::Enabled)),
            _ => None,
        },
        NodeType::Camera3D => match field {
            "zoom" => Some(NodeField::Camera3D(Camera3DField::Zoom)),
            "projection" => Some(NodeField::Camera3D(Camera3DField::Projection)),
            "perspective_fov_y_degrees" => {
                Some(NodeField::Camera3D(Camera3DField::PerspectiveFovYDegrees))
            }
            "perspective_near" => Some(NodeField::Camera3D(Camera3DField::PerspectiveNear)),
            "perspective_far" => Some(NodeField::Camera3D(Camera3DField::PerspectiveFar)),
            "orthographic_size" => Some(NodeField::Camera3D(Camera3DField::OrthographicSize)),
            "orthographic_near" => Some(NodeField::Camera3D(Camera3DField::OrthographicNear)),
            "orthographic_far" => Some(NodeField::Camera3D(Camera3DField::OrthographicFar)),
            "frustum_left" => Some(NodeField::Camera3D(Camera3DField::FrustumLeft)),
            "frustum_right" => Some(NodeField::Camera3D(Camera3DField::FrustumRight)),
            "frustum_bottom" => Some(NodeField::Camera3D(Camera3DField::FrustumBottom)),
            "frustum_top" => Some(NodeField::Camera3D(Camera3DField::FrustumTop)),
            "frustum_near" => Some(NodeField::Camera3D(Camera3DField::FrustumNear)),
            "frustum_far" => Some(NodeField::Camera3D(Camera3DField::FrustumFar)),
            "post_processing" => Some(NodeField::Camera3D(Camera3DField::PostProcessing)),
            "active" => Some(NodeField::Camera3D(Camera3DField::Active)),
            _ => None,
        },
        NodeType::ParticleEmitter3D => match field {
            "active" => Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::Active)),
            "looping" => Some(NodeField::ParticleEmitter3D(
                ParticleEmitter3DField::Looping,
            )),
            "prewarm" => Some(NodeField::ParticleEmitter3D(
                ParticleEmitter3DField::Prewarm,
            )),
            "spawn_rate" => Some(NodeField::ParticleEmitter3D(
                ParticleEmitter3DField::SpawnRate,
            )),
            "seed" => Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::Seed)),
            "params" => Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::Params)),
            "profile" => Some(NodeField::ParticleEmitter3D(
                ParticleEmitter3DField::Profile,
            )),
            "sim_mode" => Some(NodeField::ParticleEmitter3D(
                ParticleEmitter3DField::SimMode,
            )),
            "render_mode" => Some(NodeField::ParticleEmitter3D(
                ParticleEmitter3DField::RenderMode,
            )),
            _ => None,
        },
        NodeType::AnimationPlayer => match field {
            "animation" => Some(NodeField::AnimationPlayer(AnimationPlayerField::Animation)),
            "bindings" => Some(NodeField::AnimationPlayer(AnimationPlayerField::Bindings)),
            "speed" => Some(NodeField::AnimationPlayer(AnimationPlayerField::Speed)),
            "paused" => Some(NodeField::AnimationPlayer(AnimationPlayerField::Paused)),
            "playback" => Some(NodeField::AnimationPlayer(AnimationPlayerField::Playback)),
            _ => None,
        },
        NodeType::AnimationTree => match field {
            "tree" => Some(NodeField::AnimationTree(AnimationTreeField::Tree)),
            "animations" => Some(NodeField::AnimationTree(AnimationTreeField::Animations)),
            "bindings" => Some(NodeField::AnimationTree(AnimationTreeField::Bindings)),
            "speed" => Some(NodeField::AnimationTree(AnimationTreeField::Speed)),
            "paused" => Some(NodeField::AnimationTree(AnimationTreeField::Paused)),
            _ => None,
        },
        NodeType::AmbientLight3D => resolve_light3d_common(field).map(NodeField::Light3D),
        NodeType::Sky3D => resolve_sky3d_field(field).map(NodeField::Sky3D),
        NodeType::RayLight3D => match field {
            "visible" => Some(NodeField::RayLight3D(RayLight3DField::Visible)),
            _ => resolve_light3d_common(field).map(NodeField::Light3D),
        },
        NodeType::PointLight3D => match field {
            "range" => Some(NodeField::PointLight3D(PointLight3DField::Range)),
            _ => resolve_light3d_common(field).map(NodeField::Light3D),
        },
        NodeType::SpotLight3D => match field {
            "range" => Some(NodeField::SpotLight3D(SpotLight3DField::Range)),
            "inner_angle_radians" => {
                Some(NodeField::SpotLight3D(SpotLight3DField::InnerAngleRadians))
            }
            "outer_angle_radians" => {
                Some(NodeField::SpotLight3D(SpotLight3DField::OuterAngleRadians))
            }
            _ => resolve_light3d_common(field).map(NodeField::Light3D),
        },
        NodeType::CollisionShape3D => match field {
            "shape" => Some(NodeField::CollisionShape3D(CollisionShape3DField::Shape)),
            "trimesh" | "tri_mesh" => {
                Some(NodeField::CollisionShape3D(CollisionShape3DField::Trimesh))
            }
            "debug" => Some(NodeField::CollisionShape3D(CollisionShape3DField::Debug)),
            _ => None,
        },
        NodeType::StaticBody3D => match field {
            "enabled" => Some(NodeField::StaticBody3D(StaticBody3DField::Enabled)),
            "friction" => Some(NodeField::StaticBody3D(StaticBody3DField::Friction)),
            "restitution" => Some(NodeField::StaticBody3D(StaticBody3DField::Restitution)),
            "density" => Some(NodeField::StaticBody3D(StaticBody3DField::Density)),
            _ => None,
        },
        NodeType::RigidBody3D => match field {
            "enabled" => Some(NodeField::RigidBody3D(RigidBody3DField::Enabled)),
            "continuous_collision_detection" | "ccd" => Some(NodeField::RigidBody3D(
                RigidBody3DField::ContinuousCollisionDetection,
            )),
            "mass" => Some(NodeField::RigidBody3D(RigidBody3DField::Mass)),
            "linear_velocity" | "velocity" => {
                Some(NodeField::RigidBody3D(RigidBody3DField::LinearVelocity))
            }
            "angular_velocity" => Some(NodeField::RigidBody3D(RigidBody3DField::AngularVelocity)),
            "gravity_scale" => Some(NodeField::RigidBody3D(RigidBody3DField::GravityScale)),
            "linear_damping" => Some(NodeField::RigidBody3D(RigidBody3DField::LinearDamping)),
            "angular_damping" => Some(NodeField::RigidBody3D(RigidBody3DField::AngularDamping)),
            "can_sleep" => Some(NodeField::RigidBody3D(RigidBody3DField::CanSleep)),
            "friction" => Some(NodeField::RigidBody3D(RigidBody3DField::Friction)),
            "restitution" => Some(NodeField::RigidBody3D(RigidBody3DField::Restitution)),
            "density" => Some(NodeField::RigidBody3D(RigidBody3DField::Density)),
            _ => None,
        },
        NodeType::Area3D => match field {
            "enabled" => Some(NodeField::Area3D(Area3DField::Enabled)),
            _ => None,
        },
        NodeType::UiImage => match field {
            "texture" | "image" | "source" | "src" => {
                Some(NodeField::UiImage(UiImageField::Texture))
            }
            "texture_region" | "region" | "atlas_region" => {
                Some(NodeField::UiImage(UiImageField::TextureRegion))
            }
            _ => None,
        },
        _ => None,
    }
}

fn resolve_light3d_common(field: &str) -> Option<Light3DField> {
    match field {
        "color" => Some(Light3DField::Color),
        "intensity" => Some(Light3DField::Intensity),
        "cast_shadows" | "casts_shadows" => Some(Light3DField::CastShadows),
        "active" => Some(Light3DField::Active),
        _ => None,
    }
}

fn resolve_sky3d_field(field: &str) -> Option<Sky3DField> {
    match field {
        "sky_colors" | "colors" | "day_colors" => Some(Sky3DField::DayColors),
        "evening_colors" | "sunset_colors" | "dusk_colors" => Some(Sky3DField::EveningColors),
        "night_colors" => Some(Sky3DField::NightColors),
        "sky_angle" | "angle" => Some(Sky3DField::SkyAngle),
        "time" => Some(Sky3DField::Time),
        "time_of_day" | "time.time_of_day" => Some(Sky3DField::TimeOfDay),
        "time_paused" | "pause_time" | "time.paused" => Some(Sky3DField::TimePaused),
        "time_scale" | "time_speed" | "time.scale" => Some(Sky3DField::TimeScale),
        "cloud_size" | "clouds_size" | "clouds.size" => Some(Sky3DField::CloudSize),
        "cloud_density" | "clouds_density" | "clouds.density" => Some(Sky3DField::CloudDensity),
        "cloud_variance" | "clouds_variance" | "clouds.variance" => Some(Sky3DField::CloudVariance),
        "wind_vector" | "cloud_wind" | "clouds_wind" | "clouds.wind" => {
            Some(Sky3DField::CloudWindVector)
        }
        "star_size" | "stars_size" | "stars.size" => Some(Sky3DField::StarSize),
        "star_scatter" | "stars_scatter" | "stars.scatter" => Some(Sky3DField::StarScatter),
        "star_gleam" | "stars_gleam" | "stars.gleam" => Some(Sky3DField::StarGleam),
        "moon_size" | "moon.size" => Some(Sky3DField::MoonSize),
        "sun_size" | "sun.size" => Some(Sky3DField::SunSize),
        "style" | "sky_style" | "sampler" => Some(Sky3DField::Style),
        "sky_shader" | "shader" => Some(Sky3DField::SkyShader),
        "active" => Some(Sky3DField::Active),
        _ => None,
    }
}

fn resolve_base_node_field(node_type: NodeType, field: &str) -> Option<NodeField> {
    if node_type.is_a(NodeType::Node2D) {
        return match field {
            "position" => Some(NodeField::Node2D(Node2DField::Position)),
            "rotation" => Some(NodeField::Node2D(Node2DField::Rotation)),
            "scale" => Some(NodeField::Node2D(Node2DField::Scale)),
            "visible" => Some(NodeField::Node2D(Node2DField::Visible)),
            "z_index" => Some(NodeField::Node2D(Node2DField::ZIndex)),
            _ => None,
        };
    }

    if node_type.is_a(NodeType::Node3D) {
        return match field {
            "position" => Some(NodeField::Node3D(Node3DField::Position)),
            "rotation" => Some(NodeField::Node3D(Node3DField::Rotation)),
            "scale" => Some(NodeField::Node3D(Node3DField::Scale)),
            "visible" => Some(NodeField::Node3D(Node3DField::Visible)),
            _ => None,
        };
    }

    None
}
