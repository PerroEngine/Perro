// Scene prepare orchestration.
//
// Builds pending runtime nodes from scene docs and delegates node-family
// construction to `nodes/*` plus shared prepare helpers.

use crate::{material_schema, runtime_project::StaticUiStyleLookup};
use perro_ids::{NodeID, string_to_u64};
use perro_io::load_asset;
use perro_nodes::{
    AmbientLight2D, Area2D, Area3D, AudioEffectZone2D, AudioEffectZone3D, AudioMask2D, AudioMask3D,
    AudioPortal2D, AudioPortal3D, BallJoint3D, Button2D, CameraStream, CameraStream2D,
    CameraStream3D, CharacterBody2D, CharacterBody3D, CollisionShape2D, CollisionShape3D,
    Decal3D, DistanceJoint2D, FixedJoint2D,
    FixedJoint3D, HingeJoint3D, ImageButton2D, Label2D, Label3D, NineSlice2D, NineSliceButton2D, NodeType, PhysicsForceEmitter2D,
    PhysicsForceEmitter3D, PhysicsForceProfile, PinJoint2D, PointLight2D, RayLight2D,
    RigidBody2D, RigidBody3D, SceneNode, SceneNodeData, Shape2D, Shape3D, SpotLight2D,
    StaticBody2D, StaticBody3D, Triangle2DKind, UiCameraStream, UiVideoPlayer,
    UiViewport,
    VideoPlayer, VideoPlayer2D, VideoPlayer3D, WaterBody2D, WaterBody3D, Webcam,
    WaterIdleMode, WaterShape, WaterSkyBias, WaterSurfaceParams,
    ambient_light_3d::AmbientLight3D,
    animation_player::AnimationPlayer,
    animation_tree::AnimationTree,
    bone_attachment_3d::BoneAttachment3D,
    bone_collider_3d::BoneCollider3D,
    camera_2d::Camera2D,
    camera_3d::{Camera3D, CameraProjection},
    ik_target_3d::IKTarget3D,
    mesh_instance_3d::{
        LODOptions, MaterialParamOverride, MaterialParamOverrideValue, MeshInstance3D,
        MeshSurfaceBinding,
    },
    multi_mesh_instance_3d::MultiMeshInstance3D,
    node_2d::Node2D,
    node_3d::Node3D,
    particle_emitter_2d::ParticleEmitter2D,
    particle_emitter_2d::ParticleEmitterSimMode2D,
    particle_emitter_3d::ParticleEmitter3D,
    particle_emitter_3d::{ParticleEmitterSimMode3D, ParticleType},
    physics_bone_chain_3d::PhysicsBoneChain3D,
    point_light_3d::PointLight3D,
    ray_light_3d::RayLight3D,
    skeleton_2d::{BoneAttachment2D, BoneCollider2D, IKTarget2D, PhysicsBoneChain2D, Skeleton2D},
    skeleton_3d::Skeleton3D,
    sky_3d::{Sky3D, SkyShaderPass},
    spot_light_3d::SpotLight3D,
    sprite_2d::{AnimatedSprite, AnimatedSprite2D, Sprite2D},
    sprite_3d::Sprite3D,
    tilemap_2d::TileMap2D,
};
use perro_render_bridge::Material3D;
use perro_scene::{
    AnimatedSprite2DField, AnimationPlayerField, AnimationTreeField, Area2DField, Area3DField,
    BoneAttachment2DField, BoneAttachment3DField, BoneCollider2DField, BoneCollider3DField,
    Camera2DField, Camera3DField, CharacterBodyField, CollisionShape2DField,
    CollisionShape3DField,
    DistanceJoint2DField, HingeJoint3DField, IKTarget2DField, IKTarget3DField, Joint2DField,
    Joint3DField, Light2DField, Light3DField, MeshInstance3DField, NodeField, Parser,
    ParticleEmitter2DField, ParticleEmitter3DField, PhysicsBoneChain2DField,
    PhysicsBoneChain3DField, PhysicsForceEmitterField, PointLight2DField, PointLight3DField,
    NodeFieldType, RayLight2DField, RayLight3DField, RigidBody2DField, RigidBody3DField, Scene,
    SceneAssetKind, SceneFieldIterRef, SceneFieldName, SceneKey,
    SceneFieldKey,
    SceneNodeData as SceneDefNodeData,
    SceneNodeEntry as SceneDefNodeEntry, SceneObjectField, SceneValue, Skeleton3DField, Sky3DField,
    SpotLight2DField, SpotLight3DField, StaticBody2DField, StaticBody3DField, TileMap2DField,
    UiAnimatedImageField, WaterBodyField, resolve_node_field,
    audio_effect_zone_fields, audio_mask_fields, audio_portal_fields, resolve_scene_node_field,
    scene_node_field,
};
use perro_structs::{
    BitMask, Color, CustomPostParam, CustomPostParamValue, IKTargetSolver, PostProcessEffect,
    PostProcessSet, Quaternion, UVector2, Vector2, Vector3,
};
use perro_ui::{
    UiAnimatedImage, UiAnimatedImageFrameSet, UiNode, UiButton, UiCheckbox, UiColorPicker,
    UiDropdown, UiGrid, UiHLayout, UiImage, UiImageButton, UiImageScaleMode, UiLabel, UiLayout,
    UiMouseFilter, UiNineSlice, UiNineSliceButton, UiPanel, UiProgressBar, UiScrollContainer, UiShape, UiShapeKind, UiTextAlign,
    UiTextBlock, UiTextBox, UiTreeList, UiTreeListItem, UiVLayout,
};
use rayon::prelude::*;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
#[cfg(feature = "profile")]
use std::time::Duration;
#[cfg(feature = "profile")]
use std::time::Instant;

/// Generate common scene-definition -> runtime-node constructors.
///
/// Node-specific decode stays in small apply hooks. The macro owns ctor,
/// inherited-data order, base-field application, and return plumbing.
macro_rules! define_scene_node_builder {
    (
        $vis:vis fn $name:ident -> $ty:ty = $init:expr;
        base $base:ident;
        $(data_apply [$($data_apply:ident),* $(,)?];)?
        apply [$($apply:ident),* $(,)?];
        $(custom |$node:ident, $fields:ident| $custom:block)?
    ) => {
        $vis fn $name(data: &SceneDefNodeData) -> $ty {
            let mut node = $init;
            define_scene_node_builder!(@base $base, node, data);
            $($($data_apply(&mut node, data);)*)?
            $($apply(&mut node, &data.fields);)*
            $(
                let $node = &mut node;
                let $fields = data.fields.as_ref();
                $custom
            )?
            node
        }
    };

    (@base none, $node:ident, $data:ident) => {};
    (@base node_2d, $node:ident, $data:ident) => {
        if let Some(base) = $data.base_ref() {
            apply_node_2d_data(&mut $node, base);
        }
        apply_node_2d_fields(&mut $node, &$data.fields);
    };
    (@base node_3d, $node:ident, $data:ident) => {
        if let Some(base) = $data.base_ref() {
            apply_node_3d_data(&mut $node, base);
        }
        apply_node_3d_fields(&mut $node, &$data.fields);
    };
    (@base embedded_node_2d, $node:ident, $data:ident) => {
        apply_node_2d_data(&mut $node.base, $data);
    };
    (@base embedded_node_3d, $node:ident, $data:ident) => {
        apply_node_3d_data(&mut $node.base, $data);
    };
    (@base ui, $node:ident, $data:ident) => {
        if let Some(base) = $data.base_ref() {
            apply_ui_node_data(&mut $node, base);
        }
        apply_ui_node_fields(&mut $node, &$data.fields);
    };
}

trait DecodeSceneFieldValue: Sized {
    fn decode_scene_field_value(value: &SceneValue) -> Option<Self>;
}

impl DecodeSceneFieldValue for bool {
    fn decode_scene_field_value(value: &SceneValue) -> Option<Self> {
        as_bool(value)
    }
}

impl DecodeSceneFieldValue for f32 {
    fn decode_scene_field_value(value: &SceneValue) -> Option<Self> {
        as_f32(value)
    }
}

impl DecodeSceneFieldValue for i32 {
    fn decode_scene_field_value(value: &SceneValue) -> Option<Self> {
        as_i32(value)
    }
}

impl DecodeSceneFieldValue for u32 {
    fn decode_scene_field_value(value: &SceneValue) -> Option<Self> {
        as_u32(value)
    }
}

impl DecodeSceneFieldValue for Vector2 {
    fn decode_scene_field_value(value: &SceneValue) -> Option<Self> {
        as_vec2(value)
    }
}

impl DecodeSceneFieldValue for Vector3 {
    fn decode_scene_field_value(value: &SceneValue) -> Option<Self> {
        as_vec3(value)
    }
}

impl DecodeSceneFieldValue for Quaternion {
    fn decode_scene_field_value(value: &SceneValue) -> Option<Self> {
        as_quat(value)
    }
}

impl DecodeSceneFieldValue for Color {
    fn decode_scene_field_value(value: &SceneValue) -> Option<Self> {
        as_scene_color(value)
    }
}

impl DecodeSceneFieldValue for String {
    fn decode_scene_field_value(value: &SceneValue) -> Option<Self> {
        as_str(value).map(str::to_owned)
    }
}

impl DecodeSceneFieldValue for BitMask {
    fn decode_scene_field_value(value: &SceneValue) -> Option<Self> {
        as_bitmask(value)
    }
}

impl DecodeSceneFieldValue for SceneValue {
    fn decode_scene_field_value(value: &SceneValue) -> Option<Self> {
        Some(value.clone())
    }
}

fn decode_scene_field<T: DecodeSceneFieldValue>(
    _key: SceneFieldKey<T>,
    value: &SceneValue,
) -> Option<T> {
    T::decode_scene_field_value(value)
}

/// Apply typed scene fields. Keys own names, aliases, and decode types.
macro_rules! apply_scene_fields {
    ($data:expr, { $($key:path => |$value:ident| $body:block),* $(,)? }) => {{
        for (name, raw_value) in flatten_scene_node_fields($data) {
            let mut matched = false;
            $(
                if !matched && $key.matches(name.as_ref()) {
                    matched = true;
                    if let Some($value) = decode_scene_field($key, &raw_value) {
                        $body
                    }
                }
            )*
            let _ = matched;
        }
    }};
}

mod audio_nodes;
use audio_nodes::*;

#[cfg(feature = "profile")]
pub(super) struct RuntimeSceneLoadStats {
    pub(super) source_load: Duration,
    pub(super) parse: Duration,
}

#[cfg(not(feature = "profile"))]
pub(super) struct RuntimeSceneLoadStats;

#[path = "core/water.rs"]
mod water;
use water::*;
#[path = "core/scene_data.rs"]
mod scene_data;
pub(crate) use scene_data::*;
#[path = "core/load.rs"]
mod load;
pub(super) use load::*;
#[path = "core/merge.rs"]
mod merge;
use merge::*;
