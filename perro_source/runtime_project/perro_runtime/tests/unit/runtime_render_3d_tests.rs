use super::Runtime;
use perro_animation::{
    AnimationClip, AnimationObject, AnimationObjectKey, AnimationObjectTrack, AnimationTrackValue,
};
use perro_ids::{MaterialID, MeshID, TextureID};
use perro_nodes::{
    AnimationPlayer, CameraProjection, CollisionShape3D, Label3D, SceneNode, SceneNodeData,
    StaticBody3D, WaterBody3D,
    ambient_light_3d::AmbientLight3D,
    camera_3d::Camera3D,
    mesh_instance_3d::MeshInstance3D,
    mesh_instance_3d::MeshSurfaceBinding,
    multi_mesh_instance_3d::MultiMeshInstance3D,
    node_3d::Node3D,
    physics_3d::RigidBody3D,
    physics_3d::Shape3D,
    ray_light_3d::RayLight3D,
    skeleton_3d::{Bone3D, Skeleton3D},
    sky_3d::Sky3D,
    sprite_3d::Sprite3D,
};
use perro_render_bridge::{
    CameraProjectionState, Command3D, Material3D, Mesh3D, RenderCommand, RenderEvent,
    ResourceCommand, StandardMaterial3D, UiCommand,
};
use perro_resource_api::sub_apis::{MaterialAPI, MeshAPI, TextureAPI};
use perro_runtime_api::sub_apis::{AnimPlayerAPI, NodeAPI};
use perro_scene::{Node3DField, NodeField, NodeType};
use perro_structs::Transform3D;
use perro_structs::{BitMask, Color, Quaternion, Vector3};
use std::borrow::Cow;
use std::sync::Arc;

fn collect_commands(runtime: &mut Runtime) -> Vec<RenderCommand> {
    let mut out = Vec::new();
    runtime.drain_render_commands(&mut out);
    out
}

fn collect_resource_texture_request(
    runtime: &mut Runtime,
    texture: TextureID,
) -> perro_render_bridge::RenderRequestID {
    let mut commands = Vec::new();
    runtime.drain_render_commands(&mut commands);
    commands
        .into_iter()
        .find_map(|command| match command {
            RenderCommand::Resource(ResourceCommand::CreateTexture { request, id, .. })
                if id == texture =>
            {
                Some(request)
            }
            _ => None,
        })
        .expect("expected texture create command")
}

fn water_3d_command(
    commands: &[RenderCommand],
    node_id: perro_ids::NodeID,
) -> &perro_render_bridge::Water3DState {
    commands
        .iter()
        .find_map(|command| match command {
            RenderCommand::ThreeD(command) => match command.as_ref() {
                Command3D::UpsertWater { node, water } if *node == node_id => Some(water.as_ref()),
                _ => None,
            },
            _ => None,
        })
        .expect("water command should exist")
}

fn set_primary_material(mesh: &mut MeshInstance3D, material: MaterialID) {
    if mesh.surfaces.is_empty() {
        mesh.surfaces.push(MeshSurfaceBinding::default());
    }
    mesh.surfaces[0].material = Some(material);
}

fn set_primary_material_multi(mesh: &mut MultiMeshInstance3D, material: MaterialID) {
    if mesh.surfaces.is_empty() {
        mesh.surfaces.push(MeshSurfaceBinding::default());
    }
    mesh.surfaces[0].material = Some(material);
}

fn node3d_position_clip(xs: &[(u32, f32)]) -> AnimationClip {
    AnimationClip {
        name: Cow::Borrowed("tool"),
        fps: 1.0,
        total_frames: xs.last().map(|(frame, _)| frame + 1).unwrap_or(0),
        objects: Cow::Owned(vec![AnimationObject {
            name: Cow::Borrowed("Tool"),
            node_type: NodeType::Node3D,
        }]),
        object_tracks: Cow::Owned(vec![AnimationObjectTrack {
            object: Cow::Borrowed("Tool"),
            field: NodeField::Node3D(Node3DField::Position),
            bone_target: None,
            transform2d_mask: 0,
            transform3d_mask: perro_animation::ANIMATION_TRANSFORM_MASK_POSITION,
            interpolation: perro_animation::AnimationInterpolation::Step,
            ease: perro_animation::AnimationEase::Linear,
            keys: Cow::Owned(
                xs.iter()
                    .map(|(frame, x)| AnimationObjectKey {
                        frame: *frame,
                        mode: perro_animation::AnimationKeyMode::Closed,
                        interpolation: perro_animation::AnimationInterpolation::Step,
                        ease: perro_animation::AnimationEase::Linear,
                        value: AnimationTrackValue::Transform3D(Transform3D::new(
                            Vector3::new(*x, 0.0, 0.0),
                            Quaternion::IDENTITY,
                            Vector3::ONE,
                        )),
                    })
                    .collect(),
            ),
        }]),
        frame_events: Cow::Borrowed(&[]),
    }
}

include!("runtime_render_3d_tests/water_overlays.rs");
include!("runtime_render_3d_tests/meshes.rs");
include!("runtime_render_3d_tests/materials.rs");
include!("runtime_render_3d_tests/animation.rs");
