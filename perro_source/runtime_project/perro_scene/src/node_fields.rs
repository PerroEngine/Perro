use perro_nodes::NodeType;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeField {
    Node2D(Node2DField),
    Node3D(Node3DField),
    Camera2D(Camera2DField),
    Sprite2D(Sprite2DField),
    CollisionShape2D(CollisionShape2DField),
    StaticBody2D(StaticBody2DField),
    RigidBody2D(RigidBody2DField),
    Area2D(Area2DField),
    MeshInstance3D(MeshInstance3DField),
    Skeleton3D(Skeleton3DField),
    TerrainInstance3D(TerrainInstance3DField),
    Camera3D(Camera3DField),
    ParticleEmitter3D(ParticleEmitter3DField),
    AnimationPlayer(AnimationPlayerField),
    Light3D(Light3DField),
    Sky3D(Sky3DField),
    RayLight3D(RayLight3DField),
    PointLight3D(PointLight3DField),
    SpotLight3D(SpotLight3DField),
    CollisionShape3D(CollisionShape3DField),
    StaticBody3D(StaticBody3DField),
    RigidBody3D(RigidBody3DField),
    Area3D(Area3DField),
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollisionShape2DField {
    Shape,
    Sensor,
    Friction,
    Restitution,
    Density,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticBody2DField {
    Enabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigidBody2DField {
    Enabled,
    LinearVelocity,
    AngularVelocity,
    GravityScale,
    LinearDamping,
    AngularDamping,
    CanSleep,
    LockRotation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area2DField {
    Enabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshInstance3DField {
    Mesh,
    Material,
    Model,
    Skeleton,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skeleton3DField {
    Skeleton,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainInstance3DField {
    ShowDebugVertices,
    ShowDebugEdges,
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
pub enum Light3DField {
    Color,
    Intensity,
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
    Sensor,
    Friction,
    Restitution,
    Density,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticBody3DField {
    Enabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigidBody3DField {
    Enabled,
    LinearVelocity,
    AngularVelocity,
    GravityScale,
    LinearDamping,
    AngularDamping,
    CanSleep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area3DField {
    Enabled,
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
            _ => None,
        },
        NodeType::CollisionShape2D => match field {
            "shape" => Some(NodeField::CollisionShape2D(CollisionShape2DField::Shape)),
            "sensor" => Some(NodeField::CollisionShape2D(CollisionShape2DField::Sensor)),
            "friction" => Some(NodeField::CollisionShape2D(CollisionShape2DField::Friction)),
            "restitution" => Some(NodeField::CollisionShape2D(CollisionShape2DField::Restitution)),
            "density" => Some(NodeField::CollisionShape2D(CollisionShape2DField::Density)),
            _ => None,
        },
        NodeType::StaticBody2D => match field {
            "enabled" => Some(NodeField::StaticBody2D(StaticBody2DField::Enabled)),
            _ => None,
        },
        NodeType::RigidBody2D => match field {
            "enabled" => Some(NodeField::RigidBody2D(RigidBody2DField::Enabled)),
            "linear_velocity" | "velocity" => {
                Some(NodeField::RigidBody2D(RigidBody2DField::LinearVelocity))
            }
            "angular_velocity" => Some(NodeField::RigidBody2D(RigidBody2DField::AngularVelocity)),
            "gravity_scale" => Some(NodeField::RigidBody2D(RigidBody2DField::GravityScale)),
            "linear_damping" => Some(NodeField::RigidBody2D(RigidBody2DField::LinearDamping)),
            "angular_damping" => Some(NodeField::RigidBody2D(RigidBody2DField::AngularDamping)),
            "can_sleep" => Some(NodeField::RigidBody2D(RigidBody2DField::CanSleep)),
            "lock_rotation" => Some(NodeField::RigidBody2D(RigidBody2DField::LockRotation)),
            _ => None,
        },
        NodeType::Area2D => match field {
            "enabled" => Some(NodeField::Area2D(Area2DField::Enabled)),
            _ => None,
        },
        NodeType::MeshInstance3D => match field {
            "mesh" => Some(NodeField::MeshInstance3D(MeshInstance3DField::Mesh)),
            "material" => Some(NodeField::MeshInstance3D(MeshInstance3DField::Material)),
            "model" => Some(NodeField::MeshInstance3D(MeshInstance3DField::Model)),
            "skeleton" => Some(NodeField::MeshInstance3D(MeshInstance3DField::Skeleton)),
            _ => None,
        },
        NodeType::Skeleton3D => match field {
            "skeleton" => Some(NodeField::Skeleton3D(Skeleton3DField::Skeleton)),
            _ => None,
        },
        NodeType::TerrainInstance3D => match field {
            "show_debug_vertices" => Some(NodeField::TerrainInstance3D(
                TerrainInstance3DField::ShowDebugVertices,
            )),
            "show_debug_edges" => Some(NodeField::TerrainInstance3D(
                TerrainInstance3DField::ShowDebugEdges,
            )),
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
            "looping" => Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::Looping)),
            "prewarm" => Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::Prewarm)),
            "spawn_rate" => Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::SpawnRate)),
            "seed" => Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::Seed)),
            "params" => Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::Params)),
            "profile" => Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::Profile)),
            "sim_mode" => Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::SimMode)),
            "render_mode" => Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::RenderMode)),
            _ => None,
        },
        NodeType::AnimationPlayer => match field {
            "animation" => Some(NodeField::AnimationPlayer(AnimationPlayerField::Animation)),
            "bindings" => Some(NodeField::AnimationPlayer(AnimationPlayerField::Bindings)),
            "speed" => Some(NodeField::AnimationPlayer(AnimationPlayerField::Speed)),
            "paused" => Some(NodeField::AnimationPlayer(AnimationPlayerField::Paused)),
            "playback" => {
                Some(NodeField::AnimationPlayer(AnimationPlayerField::Playback))
            }
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
            "sensor" => Some(NodeField::CollisionShape3D(CollisionShape3DField::Sensor)),
            "friction" => Some(NodeField::CollisionShape3D(CollisionShape3DField::Friction)),
            "restitution" => Some(NodeField::CollisionShape3D(CollisionShape3DField::Restitution)),
            "density" => Some(NodeField::CollisionShape3D(CollisionShape3DField::Density)),
            _ => None,
        },
        NodeType::StaticBody3D => match field {
            "enabled" => Some(NodeField::StaticBody3D(StaticBody3DField::Enabled)),
            _ => None,
        },
        NodeType::RigidBody3D => match field {
            "enabled" => Some(NodeField::RigidBody3D(RigidBody3DField::Enabled)),
            "linear_velocity" | "velocity" => {
                Some(NodeField::RigidBody3D(RigidBody3DField::LinearVelocity))
            }
            "angular_velocity" => Some(NodeField::RigidBody3D(RigidBody3DField::AngularVelocity)),
            "gravity_scale" => Some(NodeField::RigidBody3D(RigidBody3DField::GravityScale)),
            "linear_damping" => Some(NodeField::RigidBody3D(RigidBody3DField::LinearDamping)),
            "angular_damping" => Some(NodeField::RigidBody3D(RigidBody3DField::AngularDamping)),
            "can_sleep" => Some(NodeField::RigidBody3D(RigidBody3DField::CanSleep)),
            _ => None,
        },
        NodeType::Area3D => match field {
            "enabled" => Some(NodeField::Area3D(Area3DField::Enabled)),
            _ => None,
        },
        _ => None,
    }
}

fn resolve_light3d_common(field: &str) -> Option<Light3DField> {
    match field {
        "color" => Some(Light3DField::Color),
        "intensity" => Some(Light3DField::Intensity),
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
        "cloud_variance" | "clouds_variance" | "clouds.variance" => {
            Some(Sky3DField::CloudVariance)
        }
        "wind_vector" | "cloud_wind" | "clouds_wind" | "clouds.wind" => {
            Some(Sky3DField::CloudWindVector)
        }
        "star_size" | "stars_size" | "stars.size" => Some(Sky3DField::StarSize),
        "star_scatter" | "stars_scatter" | "stars.scatter" => Some(Sky3DField::StarScatter),
        "star_gleam" | "stars_gleam" | "stars.gleam" => Some(Sky3DField::StarGleam),
        "moon_size" | "moon.size" => Some(Sky3DField::MoonSize),
        "sun_size" | "sun.size" => Some(Sky3DField::SunSize),
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
