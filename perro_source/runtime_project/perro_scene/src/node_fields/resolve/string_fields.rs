use super::super::*;
use super::*;

pub(in super::super) fn resolve_node_field_for_type(
    node_type: NodeType,
    field: &str,
) -> Option<NodeField> {
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
        NodeType::CameraStream2D | NodeType::CameraStream3D | NodeType::UiCameraStream => {
            match field {
                "camera" | "source" | "webcam" => {
                    Some(NodeField::CameraStream(CameraStreamField::Camera))
                }
                "resolution" => Some(NodeField::CameraStream(CameraStreamField::Resolution)),
                "width" => Some(NodeField::CameraStream(CameraStreamField::Width)),
                "height" => Some(NodeField::CameraStream(CameraStreamField::Height)),
                "aspect_ratio" => Some(NodeField::CameraStream(CameraStreamField::AspectRatio)),
                "aspect_mode" => Some(NodeField::CameraStream(CameraStreamField::AspectMode)),
                "post_processing" => {
                    Some(NodeField::CameraStream(CameraStreamField::PostProcessing))
                }
                "enabled" | "active" => Some(NodeField::CameraStream(CameraStreamField::Enabled)),
                "size" => Some(NodeField::CameraStream(CameraStreamField::Size)),
                "z_index" => Some(NodeField::CameraStream(CameraStreamField::ZIndex)),
                _ => None,
            }
        }
        NodeType::Webcam => match field {
            "slot" | "device" | "device_id" | "name" | "source" | "src" => {
                Some(NodeField::Webcam(WebcamField::Device))
            }
            "resolution" => Some(NodeField::Webcam(WebcamField::Resolution)),
            "width" => Some(NodeField::Webcam(WebcamField::Width)),
            "height" => Some(NodeField::Webcam(WebcamField::Height)),
            "fps" | "frame_rate" | "fps_scale" => Some(NodeField::Webcam(WebcamField::Fps)),
            "mirror" | "flip_x" => Some(NodeField::Webcam(WebcamField::Mirror)),
            "cpu_frames" | "cpu_frame" | "readback" => {
                Some(NodeField::Webcam(WebcamField::CpuFrames))
            }
            "enabled" | "active" => Some(NodeField::Webcam(WebcamField::Enabled)),
            _ => None,
        },
        NodeType::Sprite2D => match field {
            "texture" => Some(NodeField::Sprite2D(Sprite2DField::Texture)),
            "texture_region" | "region" | "atlas_region" => {
                Some(NodeField::Sprite2D(Sprite2DField::TextureRegion))
            }
            "flip_x" | "flip_h" | "mirror_x" => Some(NodeField::Sprite2D(Sprite2DField::FlipX)),
            "flip_y" | "flip_v" | "mirror_y" => Some(NodeField::Sprite2D(Sprite2DField::FlipY)),
            _ => None,
        },
        NodeType::Sprite3D => match field {
            "texture" => Some(NodeField::Sprite3D(Sprite2DField::Texture)),
            "texture_region" | "region" | "atlas_region" => {
                Some(NodeField::Sprite3D(Sprite2DField::TextureRegion))
            }
            "flip_x" | "flip_h" | "mirror_x" => Some(NodeField::Sprite3D(Sprite2DField::FlipX)),
            "flip_y" | "flip_v" | "mirror_y" => Some(NodeField::Sprite3D(Sprite2DField::FlipY)),
            _ => None,
        },
        NodeType::Button2D => match field {
            "size" => Some(NodeField::Button2D(Button2DField::Size)),
            _ => None,
        },
        NodeType::ImageButton2D => match field {
            "size" => Some(NodeField::ImageButton2D(Button2DField::Size)),
            "texture" => Some(NodeField::ImageButton2D(Button2DField::Texture)),
            "texture_region" | "region" | "atlas_region" => {
                Some(NodeField::ImageButton2D(Button2DField::TextureRegion))
            }
            _ => None,
        },
        NodeType::NineSliceButton2D => match field {
            "size" => Some(NodeField::NineSliceButton2D(Button2DField::Size)),
            "texture" => Some(NodeField::NineSliceButton2D(Button2DField::Texture)),
            "texture_region" | "region" | "atlas_region" => {
                Some(NodeField::NineSliceButton2D(Button2DField::TextureRegion))
            }
            _ => None,
        },
        NodeType::NineSlice2D => match field {
            "size" => Some(NodeField::NineSlice2D(Button2DField::Size)),
            "texture" => Some(NodeField::NineSlice2D(Button2DField::Texture)),
            "texture_region" | "region" | "atlas_region" => {
                Some(NodeField::NineSlice2D(Button2DField::TextureRegion))
            }
            _ => None,
        },
        NodeType::AnimatedSprite2D => match field {
            "texture" => Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::Texture)),
            "animations" | "sprites" => Some(NodeField::AnimatedSprite2D(
                AnimatedSprite2DField::Animations,
            )),
            "flip_x" | "flip_h" | "mirror_x" => {
                Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::FlipX))
            }
            "flip_y" | "flip_v" | "mirror_y" => {
                Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::FlipY))
            }
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
        NodeType::CharacterBody2D => resolve_character_body(field).map(NodeField::CharacterBody2D),
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
            "blend_shape_weights" | "shape_key_weights" | "morph_weights" => Some(
                NodeField::MeshInstance3D(MeshInstance3DField::BlendShapeWeights),
            ),
            "flip_x" | "mirror_x" => Some(NodeField::MeshInstance3D(MeshInstance3DField::FlipX)),
            "flip_y" | "mirror_y" => Some(NodeField::MeshInstance3D(MeshInstance3DField::FlipY)),
            "flip_z" | "mirror_z" => Some(NodeField::MeshInstance3D(MeshInstance3DField::FlipZ)),
            "meshlets" | "use_meshlets" => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::Meshlets))
            }
            "min_lod" | "lod_min" => Some(NodeField::MeshInstance3D(MeshInstance3DField::MinLod)),
            "max_lod" | "lod_max" => Some(NodeField::MeshInstance3D(MeshInstance3DField::MaxLod)),
            "cast_shadows" | "casts_shadows" => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::CastShadows))
            }
            "receive_shadows" | "receives_shadows" => Some(NodeField::MeshInstance3D(
                MeshInstance3DField::ReceiveShadows,
            )),
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
            "blend_shape_weights" | "shape_key_weights" | "morph_weights" => Some(
                NodeField::MeshInstance3D(MeshInstance3DField::BlendShapeWeights),
            ),
            "flip_x" | "mirror_x" => Some(NodeField::MeshInstance3D(MeshInstance3DField::FlipX)),
            "flip_y" | "mirror_y" => Some(NodeField::MeshInstance3D(MeshInstance3DField::FlipY)),
            "flip_z" | "mirror_z" => Some(NodeField::MeshInstance3D(MeshInstance3DField::FlipZ)),
            "instance_grid" | "grid_instances" => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::InstanceGrid))
            }
            "meshlets" | "use_meshlets" => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::Meshlets))
            }
            "min_lod" | "lod_min" => Some(NodeField::MeshInstance3D(MeshInstance3DField::MinLod)),
            "max_lod" | "lod_max" => Some(NodeField::MeshInstance3D(MeshInstance3DField::MaxLod)),
            "cast_shadows" | "casts_shadows" => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::CastShadows))
            }
            "receive_shadows" | "receives_shadows" => Some(NodeField::MeshInstance3D(
                MeshInstance3DField::ReceiveShadows,
            )),
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
        NodeType::AmbientLight3D => match field {
            "visible" => Some(NodeField::RayLight3D(RayLight3DField::Visible)),
            _ => resolve_light3d_common(field).map(NodeField::Light3D),
        },
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
            "flip_x" | "mirror_x" => {
                Some(NodeField::CollisionShape3D(CollisionShape3DField::FlipX))
            }
            "flip_y" | "mirror_y" => {
                Some(NodeField::CollisionShape3D(CollisionShape3DField::FlipY))
            }
            "flip_z" | "mirror_z" => {
                Some(NodeField::CollisionShape3D(CollisionShape3DField::FlipZ))
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
        NodeType::CharacterBody3D => resolve_character_body(field).map(NodeField::CharacterBody3D),
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
        NodeType::UiImage
        | NodeType::UiImageButton
        | NodeType::UiNineSliceButton
        | NodeType::UiNineSlice => match field {
            "texture" | "image" | "source" | "src" => {
                Some(if matches!(node_type, NodeType::UiImageButton) {
                    NodeField::UiImageButton(UiImageField::Texture)
                } else if matches!(node_type, NodeType::UiNineSliceButton) {
                    NodeField::UiNineSliceButton(UiImageField::Texture)
                } else if matches!(node_type, NodeType::UiNineSlice) {
                    NodeField::UiNineSlice(UiImageField::Texture)
                } else {
                    NodeField::UiImage(UiImageField::Texture)
                })
            }
            "texture_region" | "region" | "atlas_region" => {
                Some(if matches!(node_type, NodeType::UiImageButton) {
                    NodeField::UiImageButton(UiImageField::TextureRegion)
                } else if matches!(node_type, NodeType::UiNineSliceButton) {
                    NodeField::UiNineSliceButton(UiImageField::TextureRegion)
                } else if matches!(node_type, NodeType::UiNineSlice) {
                    NodeField::UiNineSlice(UiImageField::TextureRegion)
                } else {
                    NodeField::UiImage(UiImageField::TextureRegion)
                })
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
