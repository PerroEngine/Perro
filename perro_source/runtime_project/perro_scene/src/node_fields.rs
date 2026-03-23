use perro_nodes::NodeType;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeField {
    // Node2D base
    Position2D,
    Rotation2D,
    Scale2D,
    Visible2D,
    ZIndex2D,
    // Node3D base
    Position3D,
    Rotation3D,
    Scale3D,
    Visible3D,
    // Camera2D
    Camera2DZoom,
    Camera2DPostProcessing,
    Camera2DActive,
    // Physics 2D
    CollisionShape2DShape,
    CollisionShape2DSensor,
    CollisionShape2DFriction,
    CollisionShape2DRestitution,
    CollisionShape2DDensity,
    StaticBody2DEnabled,
    RigidBody2DEnabled,
    RigidBody2DLinearVelocity,
    RigidBody2DAngularVelocity,
    RigidBody2DGravityScale,
    RigidBody2DLinearDamping,
    RigidBody2DAngularDamping,
    RigidBody2DCanSleep,
    RigidBody2DLockRotation,
    Area2DEnabled,
    // Mesh/Skeleton/Terrain
    MeshInstance3DMesh,
    MeshInstance3DMaterial,
    MeshInstance3DModel,
    MeshInstance3DSkeleton,
    Skeleton3DSkeleton,
    Terrain3DShowDebugVertices,
    Terrain3DShowDebugEdges,
    // Camera3D
    Camera3DZoom,
    Camera3DProjection,
    Camera3DPerspectiveFovYDegrees,
    Camera3DPerspectiveNear,
    Camera3DPerspectiveFar,
    Camera3DOrthographicSize,
    Camera3DOrthographicNear,
    Camera3DOrthographicFar,
    Camera3DFrustumLeft,
    Camera3DFrustumRight,
    Camera3DFrustumBottom,
    Camera3DFrustumTop,
    Camera3DFrustumNear,
    Camera3DFrustumFar,
    Camera3DPostProcessing,
    Camera3DActive,
    // ParticleEmitter3D
    ParticleEmitter3DActive,
    ParticleEmitter3DLooping,
    ParticleEmitter3DPrewarm,
    ParticleEmitter3DSpawnRate,
    ParticleEmitter3DSeed,
    ParticleEmitter3DParams,
    ParticleEmitter3DProfile,
    ParticleEmitter3DSimMode,
    ParticleEmitter3DRenderMode,
    // AnimationPlayer
    AnimationPlayerAnimation,
    AnimationPlayerBindings,
    AnimationPlayerSpeed,
    AnimationPlayerPlaying,
    AnimationPlayerPaused,
    AnimationPlayerPlayback,
    // Lights
    AmbientLight3DColor,
    AmbientLight3DIntensity,
    AmbientLight3DActive,
    RayLight3DColor,
    RayLight3DIntensity,
    RayLight3DActive,
    RayLight3DVisible,
    PointLight3DColor,
    PointLight3DIntensity,
    PointLight3DRange,
    PointLight3DActive,
    SpotLight3DColor,
    SpotLight3DIntensity,
    SpotLight3DRange,
    SpotLight3DInnerAngleRadians,
    SpotLight3DOuterAngleRadians,
    SpotLight3DActive,
    // Physics 3D
    CollisionShape3DShape,
    CollisionShape3DSensor,
    CollisionShape3DFriction,
    CollisionShape3DRestitution,
    CollisionShape3DDensity,
    StaticBody3DEnabled,
    RigidBody3DEnabled,
    RigidBody3DLinearVelocity,
    RigidBody3DAngularVelocity,
    RigidBody3DGravityScale,
    RigidBody3DLinearDamping,
    RigidBody3DAngularDamping,
    RigidBody3DCanSleep,
    Area3DEnabled,
}

pub fn resolve_node_field(node_type_name: &str, field: &str) -> Option<NodeField> {
    let node_type = NodeType::from_str(node_type_name).ok()?;

    if let Some(base) = resolve_base_node_field(node_type, field) {
        return Some(base);
    }

    match node_type {
        NodeType::Camera2D => match field {
            "zoom" => Some(NodeField::Camera2DZoom),
            "post_processing" => Some(NodeField::Camera2DPostProcessing),
            "active" => Some(NodeField::Camera2DActive),
            _ => None,
        },
        NodeType::CollisionShape2D => match field {
            "shape" => Some(NodeField::CollisionShape2DShape),
            "sensor" => Some(NodeField::CollisionShape2DSensor),
            "friction" => Some(NodeField::CollisionShape2DFriction),
            "restitution" => Some(NodeField::CollisionShape2DRestitution),
            "density" => Some(NodeField::CollisionShape2DDensity),
            _ => None,
        },
        NodeType::StaticBody2D => match field {
            "enabled" => Some(NodeField::StaticBody2DEnabled),
            _ => None,
        },
        NodeType::RigidBody2D => match field {
            "enabled" => Some(NodeField::RigidBody2DEnabled),
            "linear_velocity" | "velocity" => Some(NodeField::RigidBody2DLinearVelocity),
            "angular_velocity" => Some(NodeField::RigidBody2DAngularVelocity),
            "gravity_scale" => Some(NodeField::RigidBody2DGravityScale),
            "linear_damping" => Some(NodeField::RigidBody2DLinearDamping),
            "angular_damping" => Some(NodeField::RigidBody2DAngularDamping),
            "can_sleep" => Some(NodeField::RigidBody2DCanSleep),
            "lock_rotation" => Some(NodeField::RigidBody2DLockRotation),
            _ => None,
        },
        NodeType::Area2D => match field {
            "enabled" => Some(NodeField::Area2DEnabled),
            _ => None,
        },
        NodeType::MeshInstance3D => match field {
            "mesh" => Some(NodeField::MeshInstance3DMesh),
            "material" => Some(NodeField::MeshInstance3DMaterial),
            "model" => Some(NodeField::MeshInstance3DModel),
            "skeleton" => Some(NodeField::MeshInstance3DSkeleton),
            _ => None,
        },
        NodeType::Skeleton3D => match field {
            "skeleton" => Some(NodeField::Skeleton3DSkeleton),
            _ => None,
        },
        NodeType::TerrainInstance3D => match field {
            "show_debug_vertices" => Some(NodeField::Terrain3DShowDebugVertices),
            "show_debug_edges" => Some(NodeField::Terrain3DShowDebugEdges),
            _ => None,
        },
        NodeType::Camera3D => match field {
            "zoom" => Some(NodeField::Camera3DZoom),
            "projection" => Some(NodeField::Camera3DProjection),
            "perspective_fov_y_degrees" => Some(NodeField::Camera3DPerspectiveFovYDegrees),
            "perspective_near" => Some(NodeField::Camera3DPerspectiveNear),
            "perspective_far" => Some(NodeField::Camera3DPerspectiveFar),
            "orthographic_size" => Some(NodeField::Camera3DOrthographicSize),
            "orthographic_near" => Some(NodeField::Camera3DOrthographicNear),
            "orthographic_far" => Some(NodeField::Camera3DOrthographicFar),
            "frustum_left" => Some(NodeField::Camera3DFrustumLeft),
            "frustum_right" => Some(NodeField::Camera3DFrustumRight),
            "frustum_bottom" => Some(NodeField::Camera3DFrustumBottom),
            "frustum_top" => Some(NodeField::Camera3DFrustumTop),
            "frustum_near" => Some(NodeField::Camera3DFrustumNear),
            "frustum_far" => Some(NodeField::Camera3DFrustumFar),
            "post_processing" => Some(NodeField::Camera3DPostProcessing),
            "active" => Some(NodeField::Camera3DActive),
            _ => None,
        },
        NodeType::ParticleEmitter3D => match field {
            "active" => Some(NodeField::ParticleEmitter3DActive),
            "looping" => Some(NodeField::ParticleEmitter3DLooping),
            "prewarm" => Some(NodeField::ParticleEmitter3DPrewarm),
            "spawn_rate" => Some(NodeField::ParticleEmitter3DSpawnRate),
            "seed" => Some(NodeField::ParticleEmitter3DSeed),
            "params" => Some(NodeField::ParticleEmitter3DParams),
            "profile" => Some(NodeField::ParticleEmitter3DProfile),
            "sim_mode" => Some(NodeField::ParticleEmitter3DSimMode),
            "render_mode" => Some(NodeField::ParticleEmitter3DRenderMode),
            _ => None,
        },
        NodeType::AnimationPlayer => match field {
            "animation" => Some(NodeField::AnimationPlayerAnimation),
            "bindings" => Some(NodeField::AnimationPlayerBindings),
            "speed" => Some(NodeField::AnimationPlayerSpeed),
            "playing" => Some(NodeField::AnimationPlayerPlaying),
            "paused" => Some(NodeField::AnimationPlayerPaused),
            "playback" | "loop" | "looping" => Some(NodeField::AnimationPlayerPlayback),
            _ => None,
        },
        NodeType::AmbientLight3D => match field {
            "color" => Some(NodeField::AmbientLight3DColor),
            "intensity" => Some(NodeField::AmbientLight3DIntensity),
            "active" => Some(NodeField::AmbientLight3DActive),
            _ => None,
        },
        NodeType::RayLight3D => match field {
            "color" => Some(NodeField::RayLight3DColor),
            "intensity" => Some(NodeField::RayLight3DIntensity),
            "active" => Some(NodeField::RayLight3DActive),
            "visible" => Some(NodeField::RayLight3DVisible),
            _ => None,
        },
        NodeType::PointLight3D => match field {
            "color" => Some(NodeField::PointLight3DColor),
            "intensity" => Some(NodeField::PointLight3DIntensity),
            "range" => Some(NodeField::PointLight3DRange),
            "active" => Some(NodeField::PointLight3DActive),
            _ => None,
        },
        NodeType::SpotLight3D => match field {
            "color" => Some(NodeField::SpotLight3DColor),
            "intensity" => Some(NodeField::SpotLight3DIntensity),
            "range" => Some(NodeField::SpotLight3DRange),
            "inner_angle_radians" => Some(NodeField::SpotLight3DInnerAngleRadians),
            "outer_angle_radians" => Some(NodeField::SpotLight3DOuterAngleRadians),
            "active" => Some(NodeField::SpotLight3DActive),
            _ => None,
        },
        NodeType::CollisionShape3D => match field {
            "shape" => Some(NodeField::CollisionShape3DShape),
            "sensor" => Some(NodeField::CollisionShape3DSensor),
            "friction" => Some(NodeField::CollisionShape3DFriction),
            "restitution" => Some(NodeField::CollisionShape3DRestitution),
            "density" => Some(NodeField::CollisionShape3DDensity),
            _ => None,
        },
        NodeType::StaticBody3D => match field {
            "enabled" => Some(NodeField::StaticBody3DEnabled),
            _ => None,
        },
        NodeType::RigidBody3D => match field {
            "enabled" => Some(NodeField::RigidBody3DEnabled),
            "linear_velocity" | "velocity" => Some(NodeField::RigidBody3DLinearVelocity),
            "angular_velocity" => Some(NodeField::RigidBody3DAngularVelocity),
            "gravity_scale" => Some(NodeField::RigidBody3DGravityScale),
            "linear_damping" => Some(NodeField::RigidBody3DLinearDamping),
            "angular_damping" => Some(NodeField::RigidBody3DAngularDamping),
            "can_sleep" => Some(NodeField::RigidBody3DCanSleep),
            _ => None,
        },
        NodeType::Area3D => match field {
            "enabled" => Some(NodeField::Area3DEnabled),
            _ => None,
        },
        _ => None,
    }
}

fn resolve_base_node_field(node_type: NodeType, field: &str) -> Option<NodeField> {
    if node_type.is_a(NodeType::Node2D) {
        return match field {
            "position" => Some(NodeField::Position2D),
            "rotation" => Some(NodeField::Rotation2D),
            "scale" => Some(NodeField::Scale2D),
            "visible" => Some(NodeField::Visible2D),
            "z_index" => Some(NodeField::ZIndex2D),
            _ => None,
        };
    }

    if node_type.is_a(NodeType::Node3D) {
        return match field {
            "position" => Some(NodeField::Position3D),
            "rotation" => Some(NodeField::Rotation3D),
            "scale" => Some(NodeField::Scale3D),
            "visible" => Some(NodeField::Visible3D),
            _ => None,
        };
    }

    None
}
