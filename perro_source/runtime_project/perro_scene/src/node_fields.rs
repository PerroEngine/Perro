use perro_nodes::NodeType;
use std::str::FromStr;

use crate::SceneFieldName;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeField {
    Node2D(Node2DField),
    Node3D(Node3DField),
    Camera2D(Camera2DField),
    CameraStream(CameraStreamField),
    Sprite2D(Sprite2DField),
    AnimatedSprite2D(AnimatedSprite2DField),
    ParticleEmitter2D(ParticleEmitter2DField),
    WaterBody2D(WaterBodyField),
    Light2D(Light2DField),
    RayLight2D(RayLight2DField),
    PointLight2D(PointLight2DField),
    SpotLight2D(SpotLight2DField),
    TileMap2D(TileMap2DField),
    Skeleton2D(Skeleton2DField),
    BoneAttachment2D(BoneAttachment2DField),
    IKTarget2D(IKTarget2DField),
    PhysicsBoneChain2D(PhysicsBoneChain2DField),
    BoneCollider2D(BoneCollider2DField),
    CollisionShape2D(CollisionShape2DField),
    StaticBody2D(StaticBody2DField),
    RigidBody2D(RigidBody2DField),
    PhysicsForceEmitter2D(PhysicsForceEmitterField),
    Area2D(Area2DField),
    PinJoint2D(Joint2DField),
    DistanceJoint2D(DistanceJoint2DField),
    FixedJoint2D(Joint2DField),
    MeshInstance3D(MeshInstance3DField),
    Skeleton3D(Skeleton3DField),
    BoneAttachment3D(BoneAttachment3DField),
    IKTarget3D(IKTarget3DField),
    PhysicsBoneChain3D(PhysicsBoneChain3DField),
    BoneCollider3D(BoneCollider3DField),
    Camera3D(Camera3DField),
    ParticleEmitter3D(ParticleEmitter3DField),
    WaterBody3D(WaterBodyField),
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
    PhysicsForceEmitter3D(PhysicsForceEmitterField),
    Area3D(Area3DField),
    BallJoint3D(Joint3DField),
    HingeJoint3D(HingeJoint3DField),
    FixedJoint3D(Joint3DField),
    UiImage(UiImageField),
    UiAnimatedImage(UiAnimatedImageField),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraStreamField {
    Camera,
    Resolution,
    Width,
    Height,
    AspectRatio,
    AspectMode,
    PostProcessing,
    Enabled,
    Size,
    ZIndex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Node2DField {
    Position,
    Rotation,
    Scale,
    Visible,
    ZIndex,
    RenderLayers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Node3DField {
    Position,
    Rotation,
    Scale,
    Visible,
    RenderLayers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Camera2DField {
    Zoom,
    RenderMask,
    PostProcessing,
    AudioOptions,
    AudioMask,
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
pub enum WaterBodyField {
    Shape,
    Resolution,
    RenderResolution,
    VerticesPerMeter,
    SimCellsPerMeter,
    RenderVerticesPerMeter,
    Depth,
    Flow,
    Wind,
    IdleMode,
    WaveSpeed,
    WaveScale,
    WaveLength,
    WakeStrength,
    FoamStrength,
    Damping,
    Buoyancy,
    Drag,
    SampleReadbackRate,
    LodNearDistance,
    LodMidDistance,
    LodFarDistance,
    LodMinResolution,
    CollisionLayers,
    CollisionMask,
    LinkLayers,
    LinkMask,
    BlendWidth,
    WaveTransfer,
    FlowTransfer,
    DeepColor,
    ShallowColor,
    ShallowDepth,
    SkyBias,
    Optics,
    Material,
    Transparency,
    Reflectivity,
    Roughness,
    FresnelPower,
    NormalStrength,
    RippleScale,
    FoamColor,
    FoamAmount,
    CrestFoamThreshold,
    CausticStrength,
    RefractionStrength,
    ScatteringStrength,
    DistanceFogStrength,
    Coastline,
    Debug,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Light2DField {
    Color,
    Intensity,
    CastShadows,
    Active,
    RenderLayers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RayLight2DField {
    Visible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointLight2DField {
    Range,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpotLight2DField {
    Range,
    InnerAngleRadians,
    OuterAngleRadians,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileMap2DField {
    Tileset,
    Width,
    Height,
    EmptyTile,
    Tiles,
    CollisionEnabled,
    CollisionLayers,
    CollisionMask,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skeleton2DField {
    Skeleton,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollisionShape2DField {
    Shape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticBody2DField {
    Enabled,
    CollisionLayers,
    CollisionMask,
    Friction,
    Restitution,
    Density,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigidBody2DField {
    Enabled,
    CollisionLayers,
    CollisionMask,
    ContinuousCollisionDetection,
    Mass,
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
    CollisionLayers,
    CollisionMask,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicsForceEmitterField {
    Enabled,
    Profile,
    Radius,
    Strength,
    Duration,
    Pulse,
    Falloff,
    AffectBodies,
    AffectWater,
    CollisionLayers,
    CollisionMask,
    Vectors,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Joint2DField {
    BodyA,
    BodyB,
    AnchorA,
    AnchorB,
    Enabled,
    CollideConnected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DistanceJoint2DField {
    Common(Joint2DField),
    MinDistance,
    MaxDistance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshInstance3DField {
    Mesh,
    Material,
    Surfaces,
    Model,
    Skeleton,
    InstanceGrid,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skeleton3DField {
    Skeleton,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoneAttachment2DField {
    Skeleton,
    BoneIndex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IKTarget2DField {
    Skeleton,
    BoneIndex,
    ChainLength,
    Iterations,
    Tolerance,
    Weight,
    MatchRotation,
    Solver,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicsBoneChain2DField {
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
pub enum BoneCollider2DField {
    Enabled,
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
    Solver,
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
    RenderMask,
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
    AudioOptions,
    AudioMask,
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
    RenderLayers,
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
    RenderLayers,
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
    CollisionLayers,
    CollisionMask,
    Friction,
    Restitution,
    Density,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigidBody3DField {
    Enabled,
    CollisionLayers,
    CollisionMask,
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
    CollisionLayers,
    CollisionMask,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Joint3DField {
    BodyA,
    BodyB,
    AnchorA,
    AnchorB,
    Enabled,
    CollideConnected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HingeJoint3DField {
    Common(Joint3DField),
    Axis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiImageField {
    Texture,
    TextureRegion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiAnimatedImageField {
    Texture,
    Animations,
    CurrentAnimation,
    CurrentFrame,
    FpsScale,
    Playing,
    Looping,
    TextureRegion,
}

pub fn resolve_node_field(node_type_name: &str, field: &str) -> Option<NodeField> {
    let node_type = NodeType::from_str(node_type_name).ok()?;
    resolve_node_field_for_type(node_type, field)
}

pub fn resolve_scene_node_field(node_type_name: &str, field: &SceneFieldName) -> Option<NodeField> {
    let node_type = NodeType::from_str(node_type_name).ok()?;
    resolve_scene_node_field_for_type(node_type, field)
        .or_else(|| resolve_node_field_for_type(node_type, field.as_ref()))
}

fn resolve_scene_node_field_for_type(
    node_type: NodeType,
    field: &SceneFieldName,
) -> Option<NodeField> {
    if matches!(node_type, NodeType::Camera2D | NodeType::Camera3D)
        && matches!(field, SceneFieldName::RenderLayers)
    {
        return None;
    }

    if let Some(base) = resolve_base_scene_node_field(node_type, field) {
        return Some(base);
    }

    match node_type {
        NodeType::Camera2D => match field {
            SceneFieldName::Zoom => Some(NodeField::Camera2D(Camera2DField::Zoom)),
            SceneFieldName::RenderMask => Some(NodeField::Camera2D(Camera2DField::RenderMask)),
            SceneFieldName::PostProcessing => {
                Some(NodeField::Camera2D(Camera2DField::PostProcessing))
            }
            SceneFieldName::AudioOptions => Some(NodeField::Camera2D(Camera2DField::AudioOptions)),
            SceneFieldName::AudioMask => Some(NodeField::Camera2D(Camera2DField::AudioMask)),
            SceneFieldName::Active => Some(NodeField::Camera2D(Camera2DField::Active)),
            _ => None,
        },
        NodeType::CameraStream2D | NodeType::CameraStream3D | NodeType::UiCameraStream => {
            resolve_scene_camera_stream(field).map(NodeField::CameraStream)
        }
        NodeType::Camera3D => match field {
            SceneFieldName::Zoom => Some(NodeField::Camera3D(Camera3DField::Zoom)),
            SceneFieldName::RenderMask => Some(NodeField::Camera3D(Camera3DField::RenderMask)),
            SceneFieldName::Projection => Some(NodeField::Camera3D(Camera3DField::Projection)),
            SceneFieldName::PerspectiveFovYDegrees => {
                Some(NodeField::Camera3D(Camera3DField::PerspectiveFovYDegrees))
            }
            SceneFieldName::PerspectiveNear => {
                Some(NodeField::Camera3D(Camera3DField::PerspectiveNear))
            }
            SceneFieldName::PerspectiveFar => {
                Some(NodeField::Camera3D(Camera3DField::PerspectiveFar))
            }
            SceneFieldName::OrthographicSize => {
                Some(NodeField::Camera3D(Camera3DField::OrthographicSize))
            }
            SceneFieldName::OrthographicNear => {
                Some(NodeField::Camera3D(Camera3DField::OrthographicNear))
            }
            SceneFieldName::OrthographicFar => {
                Some(NodeField::Camera3D(Camera3DField::OrthographicFar))
            }
            SceneFieldName::FrustumLeft => Some(NodeField::Camera3D(Camera3DField::FrustumLeft)),
            SceneFieldName::FrustumRight => Some(NodeField::Camera3D(Camera3DField::FrustumRight)),
            SceneFieldName::FrustumBottom => {
                Some(NodeField::Camera3D(Camera3DField::FrustumBottom))
            }
            SceneFieldName::FrustumTop => Some(NodeField::Camera3D(Camera3DField::FrustumTop)),
            SceneFieldName::FrustumNear => Some(NodeField::Camera3D(Camera3DField::FrustumNear)),
            SceneFieldName::FrustumFar => Some(NodeField::Camera3D(Camera3DField::FrustumFar)),
            SceneFieldName::PostProcessing => {
                Some(NodeField::Camera3D(Camera3DField::PostProcessing))
            }
            SceneFieldName::AudioOptions => Some(NodeField::Camera3D(Camera3DField::AudioOptions)),
            SceneFieldName::AudioMask => Some(NodeField::Camera3D(Camera3DField::AudioMask)),
            SceneFieldName::Active => Some(NodeField::Camera3D(Camera3DField::Active)),
            _ => None,
        },
        NodeType::Sprite2D => match field {
            SceneFieldName::Texture => Some(NodeField::Sprite2D(Sprite2DField::Texture)),
            SceneFieldName::TextureRegion => {
                Some(NodeField::Sprite2D(Sprite2DField::TextureRegion))
            }
            _ => None,
        },
        NodeType::AnimatedSprite2D => match field {
            SceneFieldName::Texture => {
                Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::Texture))
            }
            SceneFieldName::Animations => Some(NodeField::AnimatedSprite2D(
                AnimatedSprite2DField::Animations,
            )),
            SceneFieldName::CurrentAnimation | SceneFieldName::Animation => Some(
                NodeField::AnimatedSprite2D(AnimatedSprite2DField::CurrentAnimation),
            ),
            SceneFieldName::CurrentFrame => Some(NodeField::AnimatedSprite2D(
                AnimatedSprite2DField::CurrentFrame,
            )),
            SceneFieldName::FpsScale => {
                Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::FpsScale))
            }
            SceneFieldName::Playing => {
                Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::Playing))
            }
            SceneFieldName::Looping => {
                Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::Looping))
            }
            _ => None,
        },
        NodeType::ParticleEmitter2D => match field {
            SceneFieldName::Active => {
                Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::Active))
            }
            SceneFieldName::Looping => Some(NodeField::ParticleEmitter2D(
                ParticleEmitter2DField::Looping,
            )),
            SceneFieldName::Prewarm => Some(NodeField::ParticleEmitter2D(
                ParticleEmitter2DField::Prewarm,
            )),
            SceneFieldName::SpawnRate => Some(NodeField::ParticleEmitter2D(
                ParticleEmitter2DField::SpawnRate,
            )),
            SceneFieldName::Seed => {
                Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::Seed))
            }
            SceneFieldName::Params => {
                Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::Params))
            }
            SceneFieldName::Profile => Some(NodeField::ParticleEmitter2D(
                ParticleEmitter2DField::Profile,
            )),
            SceneFieldName::SimMode => Some(NodeField::ParticleEmitter2D(
                ParticleEmitter2DField::SimMode,
            )),
            _ => None,
        },
        NodeType::AmbientLight2D => resolve_scene_light2d_common(field).map(NodeField::Light2D),
        NodeType::RayLight2D => match field {
            SceneFieldName::Visible => Some(NodeField::RayLight2D(RayLight2DField::Visible)),
            _ => resolve_scene_light2d_common(field).map(NodeField::Light2D),
        },
        NodeType::PointLight2D => match field {
            SceneFieldName::Range | SceneFieldName::Radius => {
                Some(NodeField::PointLight2D(PointLight2DField::Range))
            }
            _ => resolve_scene_light2d_common(field).map(NodeField::Light2D),
        },
        NodeType::SpotLight2D => match field {
            SceneFieldName::Range | SceneFieldName::Radius => {
                Some(NodeField::SpotLight2D(SpotLight2DField::Range))
            }
            SceneFieldName::InnerAngleRadians => {
                Some(NodeField::SpotLight2D(SpotLight2DField::InnerAngleRadians))
            }
            SceneFieldName::OuterAngleRadians => {
                Some(NodeField::SpotLight2D(SpotLight2DField::OuterAngleRadians))
            }
            _ => resolve_scene_light2d_common(field).map(NodeField::Light2D),
        },
        NodeType::TileMap2D => match field {
            SceneFieldName::Tileset => Some(NodeField::TileMap2D(TileMap2DField::Tileset)),
            SceneFieldName::Width => Some(NodeField::TileMap2D(TileMap2DField::Width)),
            SceneFieldName::Height => Some(NodeField::TileMap2D(TileMap2DField::Height)),
            SceneFieldName::EmptyTile => Some(NodeField::TileMap2D(TileMap2DField::EmptyTile)),
            SceneFieldName::Tiles => Some(NodeField::TileMap2D(TileMap2DField::Tiles)),
            SceneFieldName::CollisionEnabled => {
                Some(NodeField::TileMap2D(TileMap2DField::CollisionEnabled))
            }
            SceneFieldName::CollisionLayers => {
                Some(NodeField::TileMap2D(TileMap2DField::CollisionLayers))
            }
            SceneFieldName::CollisionMask => {
                Some(NodeField::TileMap2D(TileMap2DField::CollisionMask))
            }
            _ => None,
        },
        NodeType::WaterBody2D => resolve_scene_water_body(field).map(NodeField::WaterBody2D),
        NodeType::CollisionShape2D => match field {
            SceneFieldName::Shape => {
                Some(NodeField::CollisionShape2D(CollisionShape2DField::Shape))
            }
            _ => None,
        },
        NodeType::StaticBody2D => resolve_scene_static_body_2d(field).map(NodeField::StaticBody2D),
        NodeType::RigidBody2D => resolve_scene_rigid_body_2d(field).map(NodeField::RigidBody2D),
        NodeType::PhysicsForceEmitter2D => {
            resolve_scene_physics_force_emitter(field).map(NodeField::PhysicsForceEmitter2D)
        }
        NodeType::Area2D => resolve_scene_area_2d(field).map(NodeField::Area2D),
        NodeType::PinJoint2D => resolve_scene_joint2d_common(field).map(NodeField::PinJoint2D),
        NodeType::FixedJoint2D => resolve_scene_joint2d_common(field).map(NodeField::FixedJoint2D),
        NodeType::DistanceJoint2D => match field {
            SceneFieldName::MinDistance => Some(NodeField::DistanceJoint2D(
                DistanceJoint2DField::MinDistance,
            )),
            SceneFieldName::MaxDistance => Some(NodeField::DistanceJoint2D(
                DistanceJoint2DField::MaxDistance,
            )),
            _ => resolve_scene_joint2d_common(field)
                .map(DistanceJoint2DField::Common)
                .map(NodeField::DistanceJoint2D),
        },
        NodeType::MeshInstance3D | NodeType::MultiMeshInstance3D => match field {
            SceneFieldName::Mesh => Some(NodeField::MeshInstance3D(MeshInstance3DField::Mesh)),
            SceneFieldName::Material => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::Material))
            }
            SceneFieldName::Surfaces => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::Surfaces))
            }
            SceneFieldName::Model => Some(NodeField::MeshInstance3D(MeshInstance3DField::Model)),
            SceneFieldName::Skeleton => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::Skeleton))
            }
            SceneFieldName::Meshlets => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::Meshlets))
            }
            SceneFieldName::MinLod => Some(NodeField::MeshInstance3D(MeshInstance3DField::MinLod)),
            SceneFieldName::MaxLod => Some(NodeField::MeshInstance3D(MeshInstance3DField::MaxLod)),
            SceneFieldName::Blend => Some(NodeField::MeshInstance3D(MeshInstance3DField::Blend)),
            SceneFieldName::BlendEnabled => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::BlendEnabled))
            }
            SceneFieldName::BlendNormals => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::BlendNormals))
            }
            SceneFieldName::BlendLayers => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::BlendLayers))
            }
            SceneFieldName::BlendMask => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::BlendMask))
            }
            SceneFieldName::BlendDistance => Some(NodeField::MeshInstance3D(
                MeshInstance3DField::BlendDistance,
            )),
            SceneFieldName::BlendMinDistance => Some(NodeField::MeshInstance3D(
                MeshInstance3DField::BlendMinDistance,
            )),
            _ => None,
        },
        NodeType::Skeleton2D => match field {
            SceneFieldName::Skeleton => Some(NodeField::Skeleton2D(Skeleton2DField::Skeleton)),
            _ => None,
        },
        NodeType::Skeleton3D => match field {
            SceneFieldName::Skeleton => Some(NodeField::Skeleton3D(Skeleton3DField::Skeleton)),
            _ => None,
        },
        NodeType::BoneAttachment2D => {
            resolve_scene_bone_attachment_2d(field).map(NodeField::BoneAttachment2D)
        }
        NodeType::BoneAttachment3D => {
            resolve_scene_bone_attachment_3d(field).map(NodeField::BoneAttachment3D)
        }
        NodeType::IKTarget2D => resolve_scene_ik_target_2d(field).map(NodeField::IKTarget2D),
        NodeType::IKTarget3D => resolve_scene_ik_target_3d(field).map(NodeField::IKTarget3D),
        NodeType::PhysicsBoneChain2D => {
            resolve_scene_physics_bone_chain_2d(field).map(NodeField::PhysicsBoneChain2D)
        }
        NodeType::PhysicsBoneChain3D => {
            resolve_scene_physics_bone_chain_3d(field).map(NodeField::PhysicsBoneChain3D)
        }
        NodeType::BoneCollider2D => match field {
            SceneFieldName::Enabled => {
                Some(NodeField::BoneCollider2D(BoneCollider2DField::Enabled))
            }
            _ => None,
        },
        NodeType::BoneCollider3D => match field {
            SceneFieldName::Enabled => {
                Some(NodeField::BoneCollider3D(BoneCollider3DField::Enabled))
            }
            _ => None,
        },
        NodeType::ParticleEmitter3D => match field {
            SceneFieldName::Active => {
                Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::Active))
            }
            SceneFieldName::Looping => Some(NodeField::ParticleEmitter3D(
                ParticleEmitter3DField::Looping,
            )),
            SceneFieldName::Prewarm => Some(NodeField::ParticleEmitter3D(
                ParticleEmitter3DField::Prewarm,
            )),
            SceneFieldName::SpawnRate => Some(NodeField::ParticleEmitter3D(
                ParticleEmitter3DField::SpawnRate,
            )),
            SceneFieldName::Seed => {
                Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::Seed))
            }
            SceneFieldName::Params => {
                Some(NodeField::ParticleEmitter3D(ParticleEmitter3DField::Params))
            }
            SceneFieldName::Profile => Some(NodeField::ParticleEmitter3D(
                ParticleEmitter3DField::Profile,
            )),
            SceneFieldName::SimMode => Some(NodeField::ParticleEmitter3D(
                ParticleEmitter3DField::SimMode,
            )),
            SceneFieldName::RenderMode => Some(NodeField::ParticleEmitter3D(
                ParticleEmitter3DField::RenderMode,
            )),
            _ => None,
        },
        NodeType::WaterBody3D => resolve_scene_water_body(field).map(NodeField::WaterBody3D),
        NodeType::AnimationPlayer => match field {
            SceneFieldName::Animation => {
                Some(NodeField::AnimationPlayer(AnimationPlayerField::Animation))
            }
            SceneFieldName::Bindings => {
                Some(NodeField::AnimationPlayer(AnimationPlayerField::Bindings))
            }
            SceneFieldName::Speed => Some(NodeField::AnimationPlayer(AnimationPlayerField::Speed)),
            SceneFieldName::Paused => {
                Some(NodeField::AnimationPlayer(AnimationPlayerField::Paused))
            }
            SceneFieldName::Playback => {
                Some(NodeField::AnimationPlayer(AnimationPlayerField::Playback))
            }
            _ => None,
        },
        NodeType::AnimationTree => match field {
            SceneFieldName::Tree => Some(NodeField::AnimationTree(AnimationTreeField::Tree)),
            SceneFieldName::Animations => {
                Some(NodeField::AnimationTree(AnimationTreeField::Animations))
            }
            SceneFieldName::Bindings => {
                Some(NodeField::AnimationTree(AnimationTreeField::Bindings))
            }
            SceneFieldName::Speed => Some(NodeField::AnimationTree(AnimationTreeField::Speed)),
            SceneFieldName::Paused => Some(NodeField::AnimationTree(AnimationTreeField::Paused)),
            _ => None,
        },
        NodeType::AmbientLight3D => resolve_scene_light3d_common(field).map(NodeField::Light3D),
        NodeType::Sky3D => resolve_scene_sky3d_field(field).map(NodeField::Sky3D),
        NodeType::RayLight3D => match field {
            SceneFieldName::Visible => Some(NodeField::RayLight3D(RayLight3DField::Visible)),
            _ => resolve_scene_light3d_common(field).map(NodeField::Light3D),
        },
        NodeType::PointLight3D => match field {
            SceneFieldName::Range => Some(NodeField::PointLight3D(PointLight3DField::Range)),
            _ => resolve_scene_light3d_common(field).map(NodeField::Light3D),
        },
        NodeType::SpotLight3D => match field {
            SceneFieldName::Range => Some(NodeField::SpotLight3D(SpotLight3DField::Range)),
            SceneFieldName::InnerAngleRadians => {
                Some(NodeField::SpotLight3D(SpotLight3DField::InnerAngleRadians))
            }
            SceneFieldName::OuterAngleRadians => {
                Some(NodeField::SpotLight3D(SpotLight3DField::OuterAngleRadians))
            }
            _ => resolve_scene_light3d_common(field).map(NodeField::Light3D),
        },
        NodeType::CollisionShape3D => match field {
            SceneFieldName::Shape => {
                Some(NodeField::CollisionShape3D(CollisionShape3DField::Shape))
            }
            SceneFieldName::Trimesh => {
                Some(NodeField::CollisionShape3D(CollisionShape3DField::Trimesh))
            }
            SceneFieldName::Debug => {
                Some(NodeField::CollisionShape3D(CollisionShape3DField::Debug))
            }
            _ => None,
        },
        NodeType::StaticBody3D => resolve_scene_static_body_3d(field).map(NodeField::StaticBody3D),
        NodeType::RigidBody3D => resolve_scene_rigid_body_3d(field).map(NodeField::RigidBody3D),
        NodeType::PhysicsForceEmitter3D => {
            resolve_scene_physics_force_emitter(field).map(NodeField::PhysicsForceEmitter3D)
        }
        NodeType::Area3D => resolve_scene_area_3d(field).map(NodeField::Area3D),
        NodeType::BallJoint3D => resolve_scene_joint3d_common(field).map(NodeField::BallJoint3D),
        NodeType::FixedJoint3D => resolve_scene_joint3d_common(field).map(NodeField::FixedJoint3D),
        NodeType::HingeJoint3D => match field {
            SceneFieldName::Axis => Some(NodeField::HingeJoint3D(HingeJoint3DField::Axis)),
            _ => resolve_scene_joint3d_common(field)
                .map(HingeJoint3DField::Common)
                .map(NodeField::HingeJoint3D),
        },
        NodeType::UiImage => match field {
            SceneFieldName::Texture
            | SceneFieldName::Image
            | SceneFieldName::Source
            | SceneFieldName::Src => Some(NodeField::UiImage(UiImageField::Texture)),
            SceneFieldName::TextureRegion => Some(NodeField::UiImage(UiImageField::TextureRegion)),
            _ => None,
        },
        NodeType::UiAnimatedImage => match field {
            SceneFieldName::Texture
            | SceneFieldName::Image
            | SceneFieldName::Source
            | SceneFieldName::Src => {
                Some(NodeField::UiAnimatedImage(UiAnimatedImageField::Texture))
            }
            SceneFieldName::Animations => {
                Some(NodeField::UiAnimatedImage(UiAnimatedImageField::Animations))
            }
            SceneFieldName::CurrentAnimation | SceneFieldName::Animation => Some(
                NodeField::UiAnimatedImage(UiAnimatedImageField::CurrentAnimation),
            ),
            SceneFieldName::CurrentFrame => Some(NodeField::UiAnimatedImage(
                UiAnimatedImageField::CurrentFrame,
            )),
            SceneFieldName::FpsScale => {
                Some(NodeField::UiAnimatedImage(UiAnimatedImageField::FpsScale))
            }
            SceneFieldName::Playing => {
                Some(NodeField::UiAnimatedImage(UiAnimatedImageField::Playing))
            }
            SceneFieldName::Looping => {
                Some(NodeField::UiAnimatedImage(UiAnimatedImageField::Looping))
            }
            SceneFieldName::TextureRegion => Some(NodeField::UiAnimatedImage(
                UiAnimatedImageField::TextureRegion,
            )),
            _ => None,
        },
        _ => None,
    }
}

fn resolve_node_field_for_type(node_type: NodeType, field: &str) -> Option<NodeField> {
    match (node_type, field) {
        (NodeType::Camera2D, "render_mask") => {
            return Some(NodeField::Camera2D(Camera2DField::RenderMask));
        }
        (NodeType::Camera3D, "render_mask") => {
            return Some(NodeField::Camera3D(Camera3DField::RenderMask));
        }
        (NodeType::Camera2D | NodeType::Camera3D, "render_layers") => {
            return None;
        }
        _ => {}
    }

    if let Some(base) = resolve_base_node_field(node_type, field) {
        return Some(base);
    }

    match node_type {
        NodeType::Camera2D => match field {
            "zoom" => Some(NodeField::Camera2D(Camera2DField::Zoom)),
            "render_mask" => Some(NodeField::Camera2D(Camera2DField::RenderMask)),
            "post_processing" => Some(NodeField::Camera2D(Camera2DField::PostProcessing)),
            "audio_options" => Some(NodeField::Camera2D(Camera2DField::AudioOptions)),
            "audio_mask" => Some(NodeField::Camera2D(Camera2DField::AudioMask)),
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
        NodeType::AmbientLight2D => resolve_light2d_common(field).map(NodeField::Light2D),
        NodeType::RayLight2D => match field {
            "visible" => Some(NodeField::RayLight2D(RayLight2DField::Visible)),
            _ => resolve_light2d_common(field).map(NodeField::Light2D),
        },
        NodeType::PointLight2D => match field {
            "range" | "radius" => Some(NodeField::PointLight2D(PointLight2DField::Range)),
            _ => resolve_light2d_common(field).map(NodeField::Light2D),
        },
        NodeType::SpotLight2D => match field {
            "range" | "radius" => Some(NodeField::SpotLight2D(SpotLight2DField::Range)),
            "inner_angle_radians" => {
                Some(NodeField::SpotLight2D(SpotLight2DField::InnerAngleRadians))
            }
            "outer_angle_radians" => {
                Some(NodeField::SpotLight2D(SpotLight2DField::OuterAngleRadians))
            }
            _ => resolve_light2d_common(field).map(NodeField::Light2D),
        },
        NodeType::TileMap2D => match field {
            "tileset" => Some(NodeField::TileMap2D(TileMap2DField::Tileset)),
            "width" => Some(NodeField::TileMap2D(TileMap2DField::Width)),
            "height" => Some(NodeField::TileMap2D(TileMap2DField::Height)),
            "empty_tile" => Some(NodeField::TileMap2D(TileMap2DField::EmptyTile)),
            "tiles" => Some(NodeField::TileMap2D(TileMap2DField::Tiles)),
            "collision_enabled" | "collision" => {
                Some(NodeField::TileMap2D(TileMap2DField::CollisionEnabled))
            }
            "collision_layers" => Some(NodeField::TileMap2D(TileMap2DField::CollisionLayers)),
            "collision_mask" => Some(NodeField::TileMap2D(TileMap2DField::CollisionMask)),
            _ => None,
        },
        NodeType::WaterBody2D => resolve_water_body(field).map(NodeField::WaterBody2D),
        NodeType::CollisionShape2D => match field {
            "shape" => Some(NodeField::CollisionShape2D(CollisionShape2DField::Shape)),
            _ => None,
        },
        NodeType::StaticBody2D => match field {
            "enabled" => Some(NodeField::StaticBody2D(StaticBody2DField::Enabled)),
            "collision_layers" => Some(NodeField::StaticBody2D(StaticBody2DField::CollisionLayers)),
            "collision_mask" => Some(NodeField::StaticBody2D(StaticBody2DField::CollisionMask)),
            "friction" => Some(NodeField::StaticBody2D(StaticBody2DField::Friction)),
            "restitution" => Some(NodeField::StaticBody2D(StaticBody2DField::Restitution)),
            "density" => Some(NodeField::StaticBody2D(StaticBody2DField::Density)),
            _ => None,
        },
        NodeType::RigidBody2D => match field {
            "enabled" => Some(NodeField::RigidBody2D(RigidBody2DField::Enabled)),
            "collision_layers" => Some(NodeField::RigidBody2D(RigidBody2DField::CollisionLayers)),
            "collision_mask" => Some(NodeField::RigidBody2D(RigidBody2DField::CollisionMask)),
            "continuous_collision_detection" | "ccd" => Some(NodeField::RigidBody2D(
                RigidBody2DField::ContinuousCollisionDetection,
            )),
            "mass" => Some(NodeField::RigidBody2D(RigidBody2DField::Mass)),
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
        NodeType::PhysicsForceEmitter2D => {
            resolve_physics_force_emitter(field).map(NodeField::PhysicsForceEmitter2D)
        }
        NodeType::Area2D => match field {
            "enabled" => Some(NodeField::Area2D(Area2DField::Enabled)),
            "collision_layers" => Some(NodeField::Area2D(Area2DField::CollisionLayers)),
            "collision_mask" => Some(NodeField::Area2D(Area2DField::CollisionMask)),
            _ => None,
        },
        NodeType::PinJoint2D => resolve_joint2d_common(field).map(NodeField::PinJoint2D),
        NodeType::FixedJoint2D => resolve_joint2d_common(field).map(NodeField::FixedJoint2D),
        NodeType::DistanceJoint2D => match field {
            "min_distance" | "min" => Some(NodeField::DistanceJoint2D(
                DistanceJoint2DField::MinDistance,
            )),
            "max_distance" | "max" | "distance" => Some(NodeField::DistanceJoint2D(
                DistanceJoint2DField::MaxDistance,
            )),
            _ => resolve_joint2d_common(field)
                .map(DistanceJoint2DField::Common)
                .map(NodeField::DistanceJoint2D),
        },
        NodeType::Skeleton2D => match field {
            "skeleton" => Some(NodeField::Skeleton2D(Skeleton2DField::Skeleton)),
            _ => None,
        },
        NodeType::BoneAttachment2D => match field {
            "skeleton" => Some(NodeField::BoneAttachment2D(BoneAttachment2DField::Skeleton)),
            "bone" | "bone_index" => Some(NodeField::BoneAttachment2D(
                BoneAttachment2DField::BoneIndex,
            )),
            _ => None,
        },
        NodeType::IKTarget2D => match field {
            "skeleton" => Some(NodeField::IKTarget2D(IKTarget2DField::Skeleton)),
            "bone" | "bone_index" => Some(NodeField::IKTarget2D(IKTarget2DField::BoneIndex)),
            "chain_length" => Some(NodeField::IKTarget2D(IKTarget2DField::ChainLength)),
            "iterations" | "iters" => Some(NodeField::IKTarget2D(IKTarget2DField::Iterations)),
            "tolerance" => Some(NodeField::IKTarget2D(IKTarget2DField::Tolerance)),
            "weight" => Some(NodeField::IKTarget2D(IKTarget2DField::Weight)),
            "match_rotation" => Some(NodeField::IKTarget2D(IKTarget2DField::MatchRotation)),
            "solver" => Some(NodeField::IKTarget2D(IKTarget2DField::Solver)),
            _ => None,
        },
        NodeType::PhysicsBoneChain2D => match field {
            "skeleton" => Some(NodeField::PhysicsBoneChain2D(
                PhysicsBoneChain2DField::Skeleton,
            )),
            "bone" | "bone_index" => Some(NodeField::PhysicsBoneChain2D(
                PhysicsBoneChain2DField::BoneIndex,
            )),
            "chain_length" => Some(NodeField::PhysicsBoneChain2D(
                PhysicsBoneChain2DField::ChainLength,
            )),
            "enabled" => Some(NodeField::PhysicsBoneChain2D(
                PhysicsBoneChain2DField::Enabled,
            )),
            "gravity" => Some(NodeField::PhysicsBoneChain2D(
                PhysicsBoneChain2DField::Gravity,
            )),
            "damping" => Some(NodeField::PhysicsBoneChain2D(
                PhysicsBoneChain2DField::Damping,
            )),
            "stiffness" => Some(NodeField::PhysicsBoneChain2D(
                PhysicsBoneChain2DField::Stiffness,
            )),
            "radius" => Some(NodeField::PhysicsBoneChain2D(
                PhysicsBoneChain2DField::Radius,
            )),
            "collisions" | "collision" => Some(NodeField::PhysicsBoneChain2D(
                PhysicsBoneChain2DField::Collisions,
            )),
            "iterations" | "iters" => Some(NodeField::PhysicsBoneChain2D(
                PhysicsBoneChain2DField::Iterations,
            )),
            _ => None,
        },
        NodeType::BoneCollider2D => match field {
            "enabled" => Some(NodeField::BoneCollider2D(BoneCollider2DField::Enabled)),
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
            "min_lod" | "lod_min" => Some(NodeField::MeshInstance3D(MeshInstance3DField::MinLod)),
            "max_lod" | "lod_max" => Some(NodeField::MeshInstance3D(MeshInstance3DField::MaxLod)),
            "blend" | "mesh_blend" | "blending" => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::Blend))
            }
            "blend_enabled" => Some(NodeField::MeshInstance3D(MeshInstance3DField::BlendEnabled)),
            "blend_screen" => Some(NodeField::MeshInstance3D(MeshInstance3DField::BlendScreen)),
            "blend_normals" => Some(NodeField::MeshInstance3D(MeshInstance3DField::BlendNormals)),
            "blend_layers" => Some(NodeField::MeshInstance3D(MeshInstance3DField::BlendLayers)),
            "blend_mask" => Some(NodeField::MeshInstance3D(MeshInstance3DField::BlendMask)),
            "blend_distance" | "blend_size" => Some(NodeField::MeshInstance3D(
                MeshInstance3DField::BlendDistance,
            )),
            "blend_min_distance" | "blend_min_size" => Some(NodeField::MeshInstance3D(
                MeshInstance3DField::BlendMinDistance,
            )),
            _ => None,
        },
        NodeType::MultiMeshInstance3D => match field {
            "mesh" => Some(NodeField::MeshInstance3D(MeshInstance3DField::Mesh)),
            "material" => Some(NodeField::MeshInstance3D(MeshInstance3DField::Material)),
            "surfaces" => Some(NodeField::MeshInstance3D(MeshInstance3DField::Surfaces)),
            "model" => Some(NodeField::MeshInstance3D(MeshInstance3DField::Model)),
            "instance_grid" | "grid_instances" => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::InstanceGrid))
            }
            "meshlets" | "use_meshlets" => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::Meshlets))
            }
            "min_lod" | "lod_min" => Some(NodeField::MeshInstance3D(MeshInstance3DField::MinLod)),
            "max_lod" | "lod_max" => Some(NodeField::MeshInstance3D(MeshInstance3DField::MaxLod)),
            "blend" | "mesh_blend" | "blending" => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::Blend))
            }
            "blend_enabled" => Some(NodeField::MeshInstance3D(MeshInstance3DField::BlendEnabled)),
            "blend_screen" => Some(NodeField::MeshInstance3D(MeshInstance3DField::BlendScreen)),
            "blend_normals" => Some(NodeField::MeshInstance3D(MeshInstance3DField::BlendNormals)),
            "blend_layers" => Some(NodeField::MeshInstance3D(MeshInstance3DField::BlendLayers)),
            "blend_mask" => Some(NodeField::MeshInstance3D(MeshInstance3DField::BlendMask)),
            "blend_distance" | "blend_size" => Some(NodeField::MeshInstance3D(
                MeshInstance3DField::BlendDistance,
            )),
            "blend_min_distance" | "blend_min_size" => Some(NodeField::MeshInstance3D(
                MeshInstance3DField::BlendMinDistance,
            )),
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
            "iterations" | "iters" => Some(NodeField::IKTarget3D(IKTarget3DField::Iterations)),
            "tolerance" => Some(NodeField::IKTarget3D(IKTarget3DField::Tolerance)),
            "weight" => Some(NodeField::IKTarget3D(IKTarget3DField::Weight)),
            "match_rotation" => Some(NodeField::IKTarget3D(IKTarget3DField::MatchRotation)),
            "solver" => Some(NodeField::IKTarget3D(IKTarget3DField::Solver)),
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
            "iterations" | "iters" => Some(NodeField::PhysicsBoneChain3D(
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
            "render_mask" => Some(NodeField::Camera3D(Camera3DField::RenderMask)),
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
            "audio_options" => Some(NodeField::Camera3D(Camera3DField::AudioOptions)),
            "audio_mask" => Some(NodeField::Camera3D(Camera3DField::AudioMask)),
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
        NodeType::WaterBody3D => resolve_water_body(field).map(NodeField::WaterBody3D),
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
            "collision_layers" => Some(NodeField::StaticBody3D(StaticBody3DField::CollisionLayers)),
            "collision_mask" => Some(NodeField::StaticBody3D(StaticBody3DField::CollisionMask)),
            "friction" => Some(NodeField::StaticBody3D(StaticBody3DField::Friction)),
            "restitution" => Some(NodeField::StaticBody3D(StaticBody3DField::Restitution)),
            "density" => Some(NodeField::StaticBody3D(StaticBody3DField::Density)),
            _ => None,
        },
        NodeType::RigidBody3D => match field {
            "enabled" => Some(NodeField::RigidBody3D(RigidBody3DField::Enabled)),
            "collision_layers" => Some(NodeField::RigidBody3D(RigidBody3DField::CollisionLayers)),
            "collision_mask" => Some(NodeField::RigidBody3D(RigidBody3DField::CollisionMask)),
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
        NodeType::PhysicsForceEmitter3D => {
            resolve_physics_force_emitter(field).map(NodeField::PhysicsForceEmitter3D)
        }
        NodeType::Area3D => match field {
            "enabled" => Some(NodeField::Area3D(Area3DField::Enabled)),
            "collision_layers" => Some(NodeField::Area3D(Area3DField::CollisionLayers)),
            "collision_mask" => Some(NodeField::Area3D(Area3DField::CollisionMask)),
            _ => None,
        },
        NodeType::BallJoint3D => resolve_joint3d_common(field).map(NodeField::BallJoint3D),
        NodeType::FixedJoint3D => resolve_joint3d_common(field).map(NodeField::FixedJoint3D),
        NodeType::HingeJoint3D => match field {
            "axis" => Some(NodeField::HingeJoint3D(HingeJoint3DField::Axis)),
            _ => resolve_joint3d_common(field)
                .map(HingeJoint3DField::Common)
                .map(NodeField::HingeJoint3D),
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
        NodeType::UiAnimatedImage => match field {
            "texture" | "image" | "source" | "src" => {
                Some(NodeField::UiAnimatedImage(UiAnimatedImageField::Texture))
            }
            "animations" | "sprites" => {
                Some(NodeField::UiAnimatedImage(UiAnimatedImageField::Animations))
            }
            "current_animation" | "animation" | "clip" => Some(NodeField::UiAnimatedImage(
                UiAnimatedImageField::CurrentAnimation,
            )),
            "current_frame" | "frame" => Some(NodeField::UiAnimatedImage(
                UiAnimatedImageField::CurrentFrame,
            )),
            "fps_scale" | "speed" => {
                Some(NodeField::UiAnimatedImage(UiAnimatedImageField::FpsScale))
            }
            "playing" | "play" => Some(NodeField::UiAnimatedImage(UiAnimatedImageField::Playing)),
            "looping" | "loop" => Some(NodeField::UiAnimatedImage(UiAnimatedImageField::Looping)),
            "texture_region" | "region" | "atlas_region" => Some(NodeField::UiAnimatedImage(
                UiAnimatedImageField::TextureRegion,
            )),
            _ => None,
        },
        _ => None,
    }
}

fn resolve_scene_joint2d_common(field: &SceneFieldName) -> Option<Joint2DField> {
    match field {
        SceneFieldName::BodyA => Some(Joint2DField::BodyA),
        SceneFieldName::BodyB => Some(Joint2DField::BodyB),
        SceneFieldName::AnchorA => Some(Joint2DField::AnchorA),
        SceneFieldName::AnchorB => Some(Joint2DField::AnchorB),
        SceneFieldName::Enabled => Some(Joint2DField::Enabled),
        SceneFieldName::CollideConnected => Some(Joint2DField::CollideConnected),
        _ => None,
    }
}

fn resolve_scene_joint3d_common(field: &SceneFieldName) -> Option<Joint3DField> {
    match field {
        SceneFieldName::BodyA => Some(Joint3DField::BodyA),
        SceneFieldName::BodyB => Some(Joint3DField::BodyB),
        SceneFieldName::AnchorA => Some(Joint3DField::AnchorA),
        SceneFieldName::AnchorB => Some(Joint3DField::AnchorB),
        SceneFieldName::Enabled => Some(Joint3DField::Enabled),
        SceneFieldName::CollideConnected => Some(Joint3DField::CollideConnected),
        _ => None,
    }
}

fn resolve_scene_light2d_common(field: &SceneFieldName) -> Option<Light2DField> {
    match field {
        SceneFieldName::Color => Some(Light2DField::Color),
        SceneFieldName::Intensity => Some(Light2DField::Intensity),
        SceneFieldName::CastShadows => Some(Light2DField::CastShadows),
        SceneFieldName::Active => Some(Light2DField::Active),
        SceneFieldName::RenderLayers => Some(Light2DField::RenderLayers),
        _ => None,
    }
}

fn resolve_scene_water_body(field: &SceneFieldName) -> Option<WaterBodyField> {
    resolve_water_body(field.as_ref())
}

fn resolve_water_body(field: &str) -> Option<WaterBodyField> {
    match field {
        "shape" => Some(WaterBodyField::Shape),
        "resolution" | "sim_resolution" => Some(WaterBodyField::Resolution),
        "render_resolution" | "mesh_resolution" => Some(WaterBodyField::RenderResolution),
        "vertices_per_meter"
        | "verts_per_meter"
        | "vpm"
        | "resolution_per_meter"
        | "sim_vertices_per_meter" => Some(WaterBodyField::VerticesPerMeter),
        "sim_cells_per_meter" | "simulation_cells_per_meter" => {
            Some(WaterBodyField::SimCellsPerMeter)
        }
        "render_vertices_per_meter" | "render_verts_per_meter" | "mesh_vertices_per_meter" => {
            Some(WaterBodyField::RenderVerticesPerMeter)
        }
        "depth" => Some(WaterBodyField::Depth),
        "flow" => Some(WaterBodyField::Flow),
        "wind" => Some(WaterBodyField::Wind),
        "idle_mode" | "idle" => Some(WaterBodyField::IdleMode),
        "wave_speed" => Some(WaterBodyField::WaveSpeed),
        "wave_scale" => Some(WaterBodyField::WaveScale),
        "wave_length" | "wavelength" | "wave_size" => Some(WaterBodyField::WaveLength),
        "wake_strength" => Some(WaterBodyField::WakeStrength),
        "foam_strength" => Some(WaterBodyField::FoamStrength),
        "damping" => Some(WaterBodyField::Damping),
        "buoyancy" => Some(WaterBodyField::Buoyancy),
        "drag" => Some(WaterBodyField::Drag),
        "sample_readback_rate" | "readback_rate" => Some(WaterBodyField::SampleReadbackRate),
        "lod_near_distance" | "lod_near" => Some(WaterBodyField::LodNearDistance),
        "lod_mid_distance" | "lod_mid" => Some(WaterBodyField::LodMidDistance),
        "lod_far_distance" | "lod_far" => Some(WaterBodyField::LodFarDistance),
        "lod_min_resolution" | "min_resolution" => Some(WaterBodyField::LodMinResolution),
        "collision_layers" => Some(WaterBodyField::CollisionLayers),
        "collision_mask" => Some(WaterBodyField::CollisionMask),
        "link_layers" | "water_link_layers" => Some(WaterBodyField::LinkLayers),
        "link_mask" | "water_link_mask" => Some(WaterBodyField::LinkMask),
        "blend_width" | "link_blend_width" => Some(WaterBodyField::BlendWidth),
        "wave_transfer" | "link_wave_transfer" => Some(WaterBodyField::WaveTransfer),
        "flow_transfer" | "link_flow_transfer" => Some(WaterBodyField::FlowTransfer),
        "deep_color" | "deep_water_color" => Some(WaterBodyField::DeepColor),
        "shallow_color" | "shallow_water_color" => Some(WaterBodyField::ShallowColor),
        "shallow_depth" | "shallow_cutoff" | "shallowness" | "shallowness_depth" => {
            Some(WaterBodyField::ShallowDepth)
        }
        "sky_bias" | "sky_reflect" | "sky_reflection" => Some(WaterBodyField::SkyBias),
        "optics" | "water_colors" | "colors" => Some(WaterBodyField::Optics),
        "material" | "visual" | "water_material" => Some(WaterBodyField::Material),
        "transparency" => Some(WaterBodyField::Transparency),
        "reflectivity" | "reflection_strength" => Some(WaterBodyField::Reflectivity),
        "roughness" => Some(WaterBodyField::Roughness),
        "fresnel_power" => Some(WaterBodyField::FresnelPower),
        "normal_strength" => Some(WaterBodyField::NormalStrength),
        "ripple_scale" => Some(WaterBodyField::RippleScale),
        "foam_color" => Some(WaterBodyField::FoamColor),
        "foam_amount" => Some(WaterBodyField::FoamAmount),
        "crest_foam_threshold" => Some(WaterBodyField::CrestFoamThreshold),
        "caustic_strength" => Some(WaterBodyField::CausticStrength),
        "refraction_strength" => Some(WaterBodyField::RefractionStrength),
        "scattering_strength" => Some(WaterBodyField::ScatteringStrength),
        "distance_fog_strength" => Some(WaterBodyField::DistanceFogStrength),
        "coastline" => Some(WaterBodyField::Coastline),
        "debug" => Some(WaterBodyField::Debug),
        _ => None,
    }
}

fn resolve_scene_light3d_common(field: &SceneFieldName) -> Option<Light3DField> {
    match field {
        SceneFieldName::Color => Some(Light3DField::Color),
        SceneFieldName::Intensity => Some(Light3DField::Intensity),
        SceneFieldName::CastShadows => Some(Light3DField::CastShadows),
        SceneFieldName::Active => Some(Light3DField::Active),
        SceneFieldName::RenderLayers => Some(Light3DField::RenderLayers),
        _ => None,
    }
}

fn resolve_scene_static_body_2d(field: &SceneFieldName) -> Option<StaticBody2DField> {
    match field {
        SceneFieldName::Enabled => Some(StaticBody2DField::Enabled),
        SceneFieldName::CollisionLayers => Some(StaticBody2DField::CollisionLayers),
        SceneFieldName::CollisionMask => Some(StaticBody2DField::CollisionMask),
        SceneFieldName::Friction => Some(StaticBody2DField::Friction),
        SceneFieldName::Restitution => Some(StaticBody2DField::Restitution),
        SceneFieldName::Density => Some(StaticBody2DField::Density),
        _ => None,
    }
}

fn resolve_scene_static_body_3d(field: &SceneFieldName) -> Option<StaticBody3DField> {
    match field {
        SceneFieldName::Enabled => Some(StaticBody3DField::Enabled),
        SceneFieldName::CollisionLayers => Some(StaticBody3DField::CollisionLayers),
        SceneFieldName::CollisionMask => Some(StaticBody3DField::CollisionMask),
        SceneFieldName::Friction => Some(StaticBody3DField::Friction),
        SceneFieldName::Restitution => Some(StaticBody3DField::Restitution),
        SceneFieldName::Density => Some(StaticBody3DField::Density),
        _ => None,
    }
}

fn resolve_scene_rigid_body_2d(field: &SceneFieldName) -> Option<RigidBody2DField> {
    match field {
        SceneFieldName::Enabled => Some(RigidBody2DField::Enabled),
        SceneFieldName::CollisionLayers => Some(RigidBody2DField::CollisionLayers),
        SceneFieldName::CollisionMask => Some(RigidBody2DField::CollisionMask),
        SceneFieldName::ContinuousCollisionDetection => {
            Some(RigidBody2DField::ContinuousCollisionDetection)
        }
        SceneFieldName::Mass => Some(RigidBody2DField::Mass),
        SceneFieldName::LinearVelocity => Some(RigidBody2DField::LinearVelocity),
        SceneFieldName::AngularVelocity => Some(RigidBody2DField::AngularVelocity),
        SceneFieldName::GravityScale => Some(RigidBody2DField::GravityScale),
        SceneFieldName::LinearDamping => Some(RigidBody2DField::LinearDamping),
        SceneFieldName::AngularDamping => Some(RigidBody2DField::AngularDamping),
        SceneFieldName::CanSleep => Some(RigidBody2DField::CanSleep),
        SceneFieldName::LockRotation => Some(RigidBody2DField::LockRotation),
        SceneFieldName::Friction => Some(RigidBody2DField::Friction),
        SceneFieldName::Restitution => Some(RigidBody2DField::Restitution),
        SceneFieldName::Density => Some(RigidBody2DField::Density),
        _ => None,
    }
}

fn resolve_scene_rigid_body_3d(field: &SceneFieldName) -> Option<RigidBody3DField> {
    match field {
        SceneFieldName::Enabled => Some(RigidBody3DField::Enabled),
        SceneFieldName::CollisionLayers => Some(RigidBody3DField::CollisionLayers),
        SceneFieldName::CollisionMask => Some(RigidBody3DField::CollisionMask),
        SceneFieldName::ContinuousCollisionDetection => {
            Some(RigidBody3DField::ContinuousCollisionDetection)
        }
        SceneFieldName::Mass => Some(RigidBody3DField::Mass),
        SceneFieldName::LinearVelocity => Some(RigidBody3DField::LinearVelocity),
        SceneFieldName::AngularVelocity => Some(RigidBody3DField::AngularVelocity),
        SceneFieldName::GravityScale => Some(RigidBody3DField::GravityScale),
        SceneFieldName::LinearDamping => Some(RigidBody3DField::LinearDamping),
        SceneFieldName::AngularDamping => Some(RigidBody3DField::AngularDamping),
        SceneFieldName::CanSleep => Some(RigidBody3DField::CanSleep),
        SceneFieldName::Friction => Some(RigidBody3DField::Friction),
        SceneFieldName::Restitution => Some(RigidBody3DField::Restitution),
        SceneFieldName::Density => Some(RigidBody3DField::Density),
        _ => None,
    }
}

fn resolve_scene_area_2d(field: &SceneFieldName) -> Option<Area2DField> {
    match field {
        SceneFieldName::Enabled => Some(Area2DField::Enabled),
        SceneFieldName::CollisionLayers => Some(Area2DField::CollisionLayers),
        SceneFieldName::CollisionMask => Some(Area2DField::CollisionMask),
        _ => None,
    }
}

fn resolve_scene_area_3d(field: &SceneFieldName) -> Option<Area3DField> {
    match field {
        SceneFieldName::Enabled => Some(Area3DField::Enabled),
        SceneFieldName::CollisionLayers => Some(Area3DField::CollisionLayers),
        SceneFieldName::CollisionMask => Some(Area3DField::CollisionMask),
        _ => None,
    }
}

fn resolve_scene_physics_force_emitter(field: &SceneFieldName) -> Option<PhysicsForceEmitterField> {
    resolve_physics_force_emitter(field.as_ref())
}

fn resolve_physics_force_emitter(field: &str) -> Option<PhysicsForceEmitterField> {
    match field {
        "enabled" => Some(PhysicsForceEmitterField::Enabled),
        "profile" => Some(PhysicsForceEmitterField::Profile),
        "radius" | "range" => Some(PhysicsForceEmitterField::Radius),
        "strength" | "intensity" => Some(PhysicsForceEmitterField::Strength),
        "duration" => Some(PhysicsForceEmitterField::Duration),
        "pulse" => Some(PhysicsForceEmitterField::Pulse),
        "falloff" => Some(PhysicsForceEmitterField::Falloff),
        "affect_bodies" | "bodies" => Some(PhysicsForceEmitterField::AffectBodies),
        "affect_water" | "water" => Some(PhysicsForceEmitterField::AffectWater),
        "collision_layers" => Some(PhysicsForceEmitterField::CollisionLayers),
        "collision_mask" => Some(PhysicsForceEmitterField::CollisionMask),
        "vectors" | "forces" => Some(PhysicsForceEmitterField::Vectors),
        _ => None,
    }
}

fn resolve_scene_bone_attachment_2d(field: &SceneFieldName) -> Option<BoneAttachment2DField> {
    match field {
        SceneFieldName::Skeleton => Some(BoneAttachment2DField::Skeleton),
        SceneFieldName::BoneIndex => Some(BoneAttachment2DField::BoneIndex),
        _ => None,
    }
}

fn resolve_scene_bone_attachment_3d(field: &SceneFieldName) -> Option<BoneAttachment3DField> {
    match field {
        SceneFieldName::Skeleton => Some(BoneAttachment3DField::Skeleton),
        SceneFieldName::BoneIndex => Some(BoneAttachment3DField::BoneIndex),
        _ => None,
    }
}

fn resolve_scene_ik_target_2d(field: &SceneFieldName) -> Option<IKTarget2DField> {
    match field {
        SceneFieldName::Skeleton => Some(IKTarget2DField::Skeleton),
        SceneFieldName::BoneIndex => Some(IKTarget2DField::BoneIndex),
        SceneFieldName::ChainLength => Some(IKTarget2DField::ChainLength),
        SceneFieldName::Iterations => Some(IKTarget2DField::Iterations),
        SceneFieldName::Tolerance => Some(IKTarget2DField::Tolerance),
        SceneFieldName::Weight => Some(IKTarget2DField::Weight),
        SceneFieldName::MatchRotation => Some(IKTarget2DField::MatchRotation),
        SceneFieldName::Solver => Some(IKTarget2DField::Solver),
        _ => None,
    }
}

fn resolve_scene_ik_target_3d(field: &SceneFieldName) -> Option<IKTarget3DField> {
    match field {
        SceneFieldName::Skeleton => Some(IKTarget3DField::Skeleton),
        SceneFieldName::BoneIndex => Some(IKTarget3DField::BoneIndex),
        SceneFieldName::ChainLength => Some(IKTarget3DField::ChainLength),
        SceneFieldName::Iterations => Some(IKTarget3DField::Iterations),
        SceneFieldName::Tolerance => Some(IKTarget3DField::Tolerance),
        SceneFieldName::Weight => Some(IKTarget3DField::Weight),
        SceneFieldName::MatchRotation => Some(IKTarget3DField::MatchRotation),
        SceneFieldName::Solver => Some(IKTarget3DField::Solver),
        _ => None,
    }
}

fn resolve_scene_physics_bone_chain_2d(field: &SceneFieldName) -> Option<PhysicsBoneChain2DField> {
    match field {
        SceneFieldName::Skeleton => Some(PhysicsBoneChain2DField::Skeleton),
        SceneFieldName::BoneIndex => Some(PhysicsBoneChain2DField::BoneIndex),
        SceneFieldName::ChainLength => Some(PhysicsBoneChain2DField::ChainLength),
        SceneFieldName::Enabled => Some(PhysicsBoneChain2DField::Enabled),
        SceneFieldName::Gravity => Some(PhysicsBoneChain2DField::Gravity),
        SceneFieldName::Damping => Some(PhysicsBoneChain2DField::Damping),
        SceneFieldName::Stiffness => Some(PhysicsBoneChain2DField::Stiffness),
        SceneFieldName::Radius => Some(PhysicsBoneChain2DField::Radius),
        SceneFieldName::Collisions => Some(PhysicsBoneChain2DField::Collisions),
        SceneFieldName::Iterations => Some(PhysicsBoneChain2DField::Iterations),
        _ => None,
    }
}

fn resolve_scene_physics_bone_chain_3d(field: &SceneFieldName) -> Option<PhysicsBoneChain3DField> {
    match field {
        SceneFieldName::Skeleton => Some(PhysicsBoneChain3DField::Skeleton),
        SceneFieldName::BoneIndex => Some(PhysicsBoneChain3DField::BoneIndex),
        SceneFieldName::ChainLength => Some(PhysicsBoneChain3DField::ChainLength),
        SceneFieldName::Enabled => Some(PhysicsBoneChain3DField::Enabled),
        SceneFieldName::Gravity => Some(PhysicsBoneChain3DField::Gravity),
        SceneFieldName::Damping => Some(PhysicsBoneChain3DField::Damping),
        SceneFieldName::Stiffness => Some(PhysicsBoneChain3DField::Stiffness),
        SceneFieldName::Radius => Some(PhysicsBoneChain3DField::Radius),
        SceneFieldName::Collisions => Some(PhysicsBoneChain3DField::Collisions),
        SceneFieldName::Iterations => Some(PhysicsBoneChain3DField::Iterations),
        _ => None,
    }
}

fn resolve_scene_sky3d_field(field: &SceneFieldName) -> Option<Sky3DField> {
    match field {
        SceneFieldName::DayColors => Some(Sky3DField::DayColors),
        SceneFieldName::EveningColors => Some(Sky3DField::EveningColors),
        SceneFieldName::NightColors => Some(Sky3DField::NightColors),
        SceneFieldName::SkyAngle => Some(Sky3DField::SkyAngle),
        SceneFieldName::Time => Some(Sky3DField::Time),
        SceneFieldName::TimeOfDay => Some(Sky3DField::TimeOfDay),
        SceneFieldName::TimePaused => Some(Sky3DField::TimePaused),
        SceneFieldName::TimeScale => Some(Sky3DField::TimeScale),
        SceneFieldName::CloudSize => Some(Sky3DField::CloudSize),
        SceneFieldName::CloudDensity => Some(Sky3DField::CloudDensity),
        SceneFieldName::CloudVariance => Some(Sky3DField::CloudVariance),
        SceneFieldName::WindVector => Some(Sky3DField::CloudWindVector),
        SceneFieldName::StarSize => Some(Sky3DField::StarSize),
        SceneFieldName::StarScatter => Some(Sky3DField::StarScatter),
        SceneFieldName::StarGleam => Some(Sky3DField::StarGleam),
        SceneFieldName::MoonSize => Some(Sky3DField::MoonSize),
        SceneFieldName::SunSize => Some(Sky3DField::SunSize),
        SceneFieldName::Style => Some(Sky3DField::Style),
        SceneFieldName::SkyShader => Some(Sky3DField::SkyShader),
        SceneFieldName::Active => Some(Sky3DField::Active),
        SceneFieldName::RenderLayers => Some(Sky3DField::RenderLayers),
        _ => None,
    }
}

fn resolve_joint2d_common(field: &str) -> Option<Joint2DField> {
    match field {
        "body_a" | "a" => Some(Joint2DField::BodyA),
        "body_b" | "b" => Some(Joint2DField::BodyB),
        "anchor_a" => Some(Joint2DField::AnchorA),
        "anchor_b" => Some(Joint2DField::AnchorB),
        "enabled" => Some(Joint2DField::Enabled),
        "collide_connected" | "collision" => Some(Joint2DField::CollideConnected),
        _ => None,
    }
}

fn resolve_joint3d_common(field: &str) -> Option<Joint3DField> {
    match field {
        "body_a" | "a" => Some(Joint3DField::BodyA),
        "body_b" | "b" => Some(Joint3DField::BodyB),
        "anchor_a" => Some(Joint3DField::AnchorA),
        "anchor_b" => Some(Joint3DField::AnchorB),
        "enabled" => Some(Joint3DField::Enabled),
        "collide_connected" | "collision" => Some(Joint3DField::CollideConnected),
        _ => None,
    }
}

fn resolve_light3d_common(field: &str) -> Option<Light3DField> {
    match field {
        "color" => Some(Light3DField::Color),
        "intensity" => Some(Light3DField::Intensity),
        "cast_shadows" | "casts_shadows" => Some(Light3DField::CastShadows),
        "active" => Some(Light3DField::Active),
        "render_layers" => Some(Light3DField::RenderLayers),
        _ => None,
    }
}

fn resolve_light2d_common(field: &str) -> Option<Light2DField> {
    match field {
        "color" => Some(Light2DField::Color),
        "intensity" => Some(Light2DField::Intensity),
        "cast_shadows" | "casts_shadows" => Some(Light2DField::CastShadows),
        "active" => Some(Light2DField::Active),
        "render_layers" => Some(Light2DField::RenderLayers),
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
        "render_layers" => Some(Sky3DField::RenderLayers),
        _ => None,
    }
}

fn resolve_base_node_field(node_type: NodeType, field: &str) -> Option<NodeField> {
    if node_type.is_a(NodeType::Node2D) {
        return match field {
            "position" => Some(NodeField::Node2D(Node2DField::Position)),
            "rotation" | "rotation_deg" => Some(NodeField::Node2D(Node2DField::Rotation)),
            "scale" => Some(NodeField::Node2D(Node2DField::Scale)),
            "visible" => Some(NodeField::Node2D(Node2DField::Visible)),
            "z_index" => Some(NodeField::Node2D(Node2DField::ZIndex)),
            "render_layers" => Some(NodeField::Node2D(Node2DField::RenderLayers)),
            _ => None,
        };
    }

    if node_type.is_a(NodeType::Node3D) {
        return match field {
            "position" => Some(NodeField::Node3D(Node3DField::Position)),
            "rotation" | "rotation_deg" => Some(NodeField::Node3D(Node3DField::Rotation)),
            "scale" => Some(NodeField::Node3D(Node3DField::Scale)),
            "visible" => Some(NodeField::Node3D(Node3DField::Visible)),
            "render_layers" => Some(NodeField::Node3D(Node3DField::RenderLayers)),
            _ => None,
        };
    }

    None
}

fn resolve_base_scene_node_field(node_type: NodeType, field: &SceneFieldName) -> Option<NodeField> {
    if node_type.is_a(NodeType::Node2D) {
        return match field {
            SceneFieldName::Position => Some(NodeField::Node2D(Node2DField::Position)),
            SceneFieldName::Rotation => Some(NodeField::Node2D(Node2DField::Rotation)),
            SceneFieldName::Scale => Some(NodeField::Node2D(Node2DField::Scale)),
            SceneFieldName::Visible => Some(NodeField::Node2D(Node2DField::Visible)),
            SceneFieldName::ZIndex => Some(NodeField::Node2D(Node2DField::ZIndex)),
            SceneFieldName::RenderLayers => Some(NodeField::Node2D(Node2DField::RenderLayers)),
            _ => None,
        };
    }

    if node_type.is_a(NodeType::Node3D) {
        return match field {
            SceneFieldName::Position => Some(NodeField::Node3D(Node3DField::Position)),
            SceneFieldName::Rotation => Some(NodeField::Node3D(Node3DField::Rotation)),
            SceneFieldName::Scale => Some(NodeField::Node3D(Node3DField::Scale)),
            SceneFieldName::Visible => Some(NodeField::Node3D(Node3DField::Visible)),
            SceneFieldName::RenderLayers => Some(NodeField::Node3D(Node3DField::RenderLayers)),
            _ => None,
        };
    }

    None
}

fn resolve_scene_camera_stream(field: &SceneFieldName) -> Option<CameraStreamField> {
    match field {
        SceneFieldName::Camera => Some(CameraStreamField::Camera),
        SceneFieldName::Resolution => Some(CameraStreamField::Resolution),
        SceneFieldName::Width => Some(CameraStreamField::Width),
        SceneFieldName::Height => Some(CameraStreamField::Height),
        SceneFieldName::AspectRatio => Some(CameraStreamField::AspectRatio),
        SceneFieldName::AspectMode => Some(CameraStreamField::AspectMode),
        SceneFieldName::PostProcessing => Some(CameraStreamField::PostProcessing),
        SceneFieldName::Enabled | SceneFieldName::Active => Some(CameraStreamField::Enabled),
        SceneFieldName::Size => Some(CameraStreamField::Size),
        SceneFieldName::ZIndex => Some(CameraStreamField::ZIndex),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collision_layer_fields_use_layers_and_mask_names() {
        assert_eq!(
            resolve_node_field("StaticBody2D", "collision_layers"),
            Some(NodeField::StaticBody2D(StaticBody2DField::CollisionLayers))
        );
        assert_eq!(
            resolve_node_field("StaticBody2D", "collision_mask"),
            Some(NodeField::StaticBody2D(StaticBody2DField::CollisionMask))
        );
        for field in [
            "collision_layer",
            "collision_mask_layers",
            "layer",
            "layers",
            "mask",
            "masks",
        ] {
            assert_eq!(resolve_node_field("StaticBody2D", field), None);
        }
    }

    #[test]
    fn render_fields_use_camera_mask_and_node_layers_only() {
        assert_eq!(
            resolve_node_field("Camera2D", "render_mask"),
            Some(NodeField::Camera2D(Camera2DField::RenderMask))
        );
        assert_eq!(resolve_node_field("Camera2D", "render_layers"), None);
        assert_eq!(
            resolve_node_field("Sprite2D", "render_layers"),
            Some(NodeField::Node2D(Node2DField::RenderLayers))
        );
        assert_eq!(resolve_node_field("Sprite2D", "render_mask"), None);
        assert_eq!(
            resolve_node_field("MeshInstance3D", "render_layers"),
            Some(NodeField::Node3D(Node3DField::RenderLayers))
        );
        assert_eq!(resolve_node_field("MeshInstance3D", "render_mask"), None);
    }

    #[test]
    fn mesh_blend_fields_use_layers_and_mask_names() {
        assert_eq!(
            resolve_node_field("MeshInstance3D", "blend_layers"),
            Some(NodeField::MeshInstance3D(MeshInstance3DField::BlendLayers))
        );
        assert_eq!(
            resolve_node_field("MeshInstance3D", "blend_mask"),
            Some(NodeField::MeshInstance3D(MeshInstance3DField::BlendMask))
        );
        assert_eq!(
            resolve_node_field("MultiMeshInstance3D", "blend_layers"),
            Some(NodeField::MeshInstance3D(MeshInstance3DField::BlendLayers))
        );
        assert_eq!(
            resolve_node_field("MultiMeshInstance3D", "blend_mask"),
            Some(NodeField::MeshInstance3D(MeshInstance3DField::BlendMask))
        );
        assert_eq!(
            resolve_node_field("MultiMeshInstance3D", "instance_grid"),
            Some(NodeField::MeshInstance3D(MeshInstance3DField::InstanceGrid))
        );
        assert_eq!(resolve_node_field("MeshInstance3D", "blend_layer"), None);
    }

    #[test]
    fn scene_field_enum_resolver_matches_string_resolver_for_canonical_fields() {
        for (node_type, field) in [
            ("Node2D", "position"),
            ("Node2D", "rotation"),
            ("Node2D", "render_layers"),
            ("Camera2D", "render_mask"),
            ("Camera2D", "audio_options"),
            ("Sprite2D", "texture_region"),
            ("StaticBody2D", "collision_layers"),
            ("StaticBody2D", "collision_mask"),
            ("RigidBody2D", "continuous_collision_detection"),
            ("RigidBody3D", "mass"),
            ("DistanceJoint2D", "body_a"),
            ("MeshInstance3D", "mesh"),
            ("MeshInstance3D", "min_lod"),
            ("Camera3D", "perspective_fov_y_degrees"),
            ("SpotLight2D", "inner_angle_radians"),
            ("SpotLight3D", "outer_angle_radians"),
            ("AnimationTree", "bindings"),
            ("Sky3D", "cloud_density"),
            ("CollisionShape3D", "trimesh"),
            ("UiImage", "image"),
            ("UiAnimatedImage", "current_frame"),
        ] {
            let scene_field = SceneFieldName::from_name(field.to_string());
            assert_eq!(
                resolve_scene_node_field(node_type, &scene_field),
                resolve_node_field(node_type, field),
                "{node_type}.{field}"
            );
        }
    }
}
