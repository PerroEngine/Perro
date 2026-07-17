use perro_nodes::{
    Area2D, Area3D, Camera2D, Camera3D, CharacterBody3D, MeshBlendOptions, MeshInstance3D, Node2D,
    Node3D, NodeType, PhysicsForceEmitter2D, PhysicsForceEmitter3D, RigidBody2D, RigidBody3D,
    StaticBody2D, StaticBody3D,
};
use perro_structs::{BitMask, Color, Quaternion, Vector2, Vector3};
use perro_ui::{UiNode, UiUnit, UiVector2};
use std::str::FromStr;

use crate::{SceneFieldName, SceneValue};

mod types;
pub use types::*;
mod defaults;
pub use defaults::*;
mod resolve;
use resolve::*;

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
    fn camera_stream_accepts_webcam_alias_and_webcam_fields() {
        assert_eq!(
            resolve_node_field("UiCameraStream", "webcam"),
            Some(NodeField::CameraStream(CameraStreamField::Camera))
        );
        assert_eq!(
            resolve_node_field("CameraStream2D", "source"),
            Some(NodeField::CameraStream(CameraStreamField::Camera))
        );
        assert_eq!(
            resolve_node_field("Webcam", "slot"),
            Some(NodeField::Webcam(WebcamField::Device))
        );
        assert_eq!(
            resolve_node_field("Webcam", "cpu_frames"),
            Some(NodeField::Webcam(WebcamField::CpuFrames))
        );
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
    fn flip_fields_resolve_for_sprites_and_meshes() {
        assert_eq!(
            resolve_node_field("Sprite2D", "flip_x"),
            Some(NodeField::Sprite2D(Sprite2DField::FlipX))
        );
        assert_eq!(
            resolve_node_field("Sprite3D", "flip_y"),
            Some(NodeField::Sprite3D(Sprite2DField::FlipY))
        );
        assert_eq!(
            resolve_node_field("AnimatedSprite2D", "flip_y"),
            Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::FlipY))
        );
        assert_eq!(
            resolve_node_field("MeshInstance3D", "flip_z"),
            Some(NodeField::MeshInstance3D(MeshInstance3DField::FlipZ))
        );
        assert_eq!(
            resolve_node_field("MultiMeshInstance3D", "mirror_x"),
            Some(NodeField::MeshInstance3D(MeshInstance3DField::FlipX))
        );
        assert_eq!(
            resolve_node_field("CollisionShape3D", "flip_x"),
            Some(NodeField::CollisionShape3D(CollisionShape3DField::FlipX))
        );
        assert_eq!(
            resolve_node_field("CollisionShape3D", "mirror_z"),
            Some(NodeField::CollisionShape3D(CollisionShape3DField::FlipZ))
        );
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
            ("Sprite3D", "texture_region"),
            ("Label2D", "render_layers"),
            ("Label3D", "render_layers"),
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
            ("Sky3D", "horizon_colors"),
            ("Sky3D", "shaders"),
            ("CollisionShape3D", "trimesh"),
            ("UiImage", "image"),
            ("UiImageButton", "image"),
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

    #[test]
    fn scene_field_defaults_cover_masks_and_mesh_surface_state() {
        assert_eq!(
            default_scene_field_value_by_name(NodeType::MeshInstance3D, "render_layers"),
            Some(SceneValue::I32(BitMask::ALL.bits() as i32))
        );
        assert_eq!(
            default_scene_field_value_by_name(NodeType::Camera3D, "render_mask"),
            Some(SceneValue::I32(BitMask::NONE.bits() as i32))
        );
        assert_eq!(
            default_scene_field_value_by_name(NodeType::RigidBody3D, "collision_layers"),
            Some(SceneValue::I32(BitMask::ALL.bits() as i32))
        );
        assert_eq!(
            default_scene_field_value_by_name(NodeType::RigidBody3D, "collision_mask"),
            Some(SceneValue::I32(BitMask::NONE.bits() as i32))
        );
        assert_eq!(
            default_scene_field_value_by_name(NodeType::MeshInstance3D, "surfaces"),
            Some(SceneValue::Array(Default::default()))
        );
    }
}
