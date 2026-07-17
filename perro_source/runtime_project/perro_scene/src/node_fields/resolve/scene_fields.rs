use super::super::*;

use super::*;

pub(in super::super) fn resolve_scene_node_field_for_type(
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
        NodeType::Webcam => resolve_scene_webcam(field).map(NodeField::Webcam),
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
            SceneFieldName::FlipX => Some(NodeField::Sprite2D(Sprite2DField::FlipX)),
            SceneFieldName::FlipY => Some(NodeField::Sprite2D(Sprite2DField::FlipY)),
            _ => None,
        },
        NodeType::Button2D => match field {
            SceneFieldName::Size => Some(NodeField::Button2D(Button2DField::Size)),
            _ => None,
        },
        NodeType::ImageButton2D => match field {
            SceneFieldName::Size => Some(NodeField::ImageButton2D(Button2DField::Size)),
            SceneFieldName::Texture => Some(NodeField::ImageButton2D(Button2DField::Texture)),
            SceneFieldName::TextureRegion => {
                Some(NodeField::ImageButton2D(Button2DField::TextureRegion))
            }
            _ => None,
        },
        NodeType::NineSliceButton2D => match field {
            SceneFieldName::Size => Some(NodeField::NineSliceButton2D(Button2DField::Size)),
            SceneFieldName::Texture => Some(NodeField::NineSliceButton2D(Button2DField::Texture)),
            SceneFieldName::TextureRegion => {
                Some(NodeField::NineSliceButton2D(Button2DField::TextureRegion))
            }
            _ => None,
        },
        NodeType::NineSlice2D => match field {
            SceneFieldName::Size => Some(NodeField::NineSlice2D(Button2DField::Size)),
            SceneFieldName::Texture => Some(NodeField::NineSlice2D(Button2DField::Texture)),
            SceneFieldName::TextureRegion => {
                Some(NodeField::NineSlice2D(Button2DField::TextureRegion))
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
            SceneFieldName::FlipX => {
                Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::FlipX))
            }
            SceneFieldName::FlipY => {
                Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::FlipY))
            }
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
        NodeType::CharacterBody2D => {
            resolve_scene_character_body(field).map(NodeField::CharacterBody2D)
        }
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
            SceneFieldName::BlendShapeWeights => Some(NodeField::MeshInstance3D(
                MeshInstance3DField::BlendShapeWeights,
            )),
            SceneFieldName::FlipX => Some(NodeField::MeshInstance3D(MeshInstance3DField::FlipX)),
            SceneFieldName::FlipY => Some(NodeField::MeshInstance3D(MeshInstance3DField::FlipY)),
            SceneFieldName::FlipZ => Some(NodeField::MeshInstance3D(MeshInstance3DField::FlipZ)),
            SceneFieldName::Meshlets => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::Meshlets))
            }
            SceneFieldName::MinLod => Some(NodeField::MeshInstance3D(MeshInstance3DField::MinLod)),
            SceneFieldName::MaxLod => Some(NodeField::MeshInstance3D(MeshInstance3DField::MaxLod)),
            SceneFieldName::CastShadows => {
                Some(NodeField::MeshInstance3D(MeshInstance3DField::CastShadows))
            }
            SceneFieldName::ReceiveShadows => Some(NodeField::MeshInstance3D(
                MeshInstance3DField::ReceiveShadows,
            )),
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
        NodeType::AmbientLight3D => match field {
            SceneFieldName::Visible => Some(NodeField::RayLight3D(RayLight3DField::Visible)),
            _ => resolve_scene_light3d_common(field).map(NodeField::Light3D),
        },
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
            SceneFieldName::FlipX => {
                Some(NodeField::CollisionShape3D(CollisionShape3DField::FlipX))
            }
            SceneFieldName::FlipY => {
                Some(NodeField::CollisionShape3D(CollisionShape3DField::FlipY))
            }
            SceneFieldName::FlipZ => {
                Some(NodeField::CollisionShape3D(CollisionShape3DField::FlipZ))
            }
            SceneFieldName::Debug => {
                Some(NodeField::CollisionShape3D(CollisionShape3DField::Debug))
            }
            _ => None,
        },
        NodeType::StaticBody3D => resolve_scene_static_body_3d(field).map(NodeField::StaticBody3D),
        NodeType::RigidBody3D => resolve_scene_rigid_body_3d(field).map(NodeField::RigidBody3D),
        NodeType::CharacterBody3D => {
            resolve_scene_character_body(field).map(NodeField::CharacterBody3D)
        }
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
        NodeType::UiImage
        | NodeType::UiImageButton
        | NodeType::UiNineSliceButton
        | NodeType::UiNineSlice => match field {
            SceneFieldName::Texture
            | SceneFieldName::Image
            | SceneFieldName::Source
            | SceneFieldName::Src => Some(if matches!(node_type, NodeType::UiImageButton) {
                NodeField::UiImageButton(UiImageField::Texture)
            } else if matches!(node_type, NodeType::UiNineSliceButton) {
                NodeField::UiNineSliceButton(UiImageField::Texture)
            } else if matches!(node_type, NodeType::UiNineSlice) {
                NodeField::UiNineSlice(UiImageField::Texture)
            } else {
                NodeField::UiImage(UiImageField::Texture)
            }),
            SceneFieldName::TextureRegion => {
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
