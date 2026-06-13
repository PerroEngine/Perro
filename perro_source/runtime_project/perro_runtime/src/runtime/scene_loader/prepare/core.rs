// Scene prepare orchestration.
//
// Builds pending runtime nodes from scene docs and delegates node-family
// construction to `nodes/*` plus shared prepare helpers.

mod audio_nodes;
use audio_nodes::*;

use crate::{material_schema, runtime_project::StaticUiStyleLookup};
use perro_ids::{NodeID, string_to_u64};
use perro_io::load_asset;
use perro_nodes::{
    AmbientLight2D, Area2D, Area3D, AudioEffectZone2D, AudioEffectZone3D, AudioMask2D, AudioMask3D,
    AudioPortal2D, AudioPortal3D, BallJoint3D, Button2D, CameraStream, CameraStream2D,
    CameraStream3D, CollisionShape2D, CollisionShape3D, DistanceJoint2D, FixedJoint2D,
    FixedJoint3D, HingeJoint3D, ImageButton2D, NineSlice2D, NodeType, PhysicsForceEmitter2D,
    PhysicsForceEmitter3D, PhysicsForceProfile, PinJoint2D, PointLight2D, RayLight2D,
    RigidBody2D, RigidBody3D, SceneNode, SceneNodeData, Shape2D, Shape3D, SpotLight2D,
    StaticBody2D, StaticBody3D, Triangle2DKind, UiCameraStream, WaterBody2D, WaterBody3D,
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
    tilemap_2d::TileMap2D,
};
use perro_render_bridge::Material3D;
use perro_scene::{
    AnimatedSprite2DField, AnimationPlayerField, AnimationTreeField, Area2DField, Area3DField,
    BoneAttachment2DField, BoneAttachment3DField, BoneCollider2DField, BoneCollider3DField,
    Button2DField, Camera2DField, Camera3DField, CollisionShape2DField, CollisionShape3DField,
    DistanceJoint2DField, HingeJoint3DField, IKTarget2DField, IKTarget3DField, Joint2DField,
    Joint3DField, Light2DField, Light3DField, MeshInstance3DField, NodeField, Parser,
    ParticleEmitter2DField, ParticleEmitter3DField, PhysicsBoneChain2DField,
    PhysicsBoneChain3DField, PhysicsForceEmitterField, PointLight2DField, PointLight3DField,
    RayLight2DField, RayLight3DField, RigidBody2DField, RigidBody3DField, Scene, SceneFieldIterRef,
    SceneFieldName, SceneKey, SceneNodeData as SceneDefNodeData,
    SceneNodeEntry as SceneDefNodeEntry, SceneObjectField, SceneValue, Skeleton3DField, Sky3DField,
    SpotLight2DField, SpotLight3DField, Sprite2DField, StaticBody2DField, StaticBody3DField,
    TileMap2DField, UiAnimatedImageField, UiImageField, WaterBodyField, resolve_node_field,
    resolve_scene_node_field,
};
use perro_structs::{
    BitMask, Color, CustomPostParam, CustomPostParamValue, IKTargetSolver, PostProcessEffect,
    PostProcessSet, Quaternion, UVector2, Vector2, Vector3,
};
use perro_ui::{
    UiAnimatedImage, UiAnimatedImageFrameSet, UiBox, UiButton, UiCheckbox, UiColorPicker,
    UiGrid, UiHLayout, UiImage, UiImageButton, UiImageScaleMode, UiLabel, UiLayout,
    UiList, UiListIndent, UiMouseFilter, UiNineSlice, UiPanel, UiScrollContainer, UiShape,
    UiShapeKind, UiTextAlign, UiTextBlock, UiTextBox, UiVLayout,
};
use rayon::prelude::*;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
#[cfg(feature = "profile")]
use std::time::Duration;
#[cfg(feature = "profile")]
use std::time::Instant;

#[cfg(feature = "profile")]
pub(super) struct RuntimeSceneLoadStats {
    pub(super) source_load: Duration,
    pub(super) parse: Duration,
}

#[cfg(not(feature = "profile"))]
pub(super) struct RuntimeSceneLoadStats;

fn apply_water_body_fields(node: &mut WaterSurfaceParams, ty: &str, fields: &[SceneObjectField]) {
    let mut sim_cells_per_meter = None;
    let mut render_vertices_per_meter = None;
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        let field = match resolve_node_field(ty, name) {
            Some(NodeField::WaterBody2D(field)) | Some(NodeField::WaterBody3D(field)) => field,
            _ => return,
        };
        match field {
            WaterBodyField::Shape => match ty {
                "WaterBody3D" => {
                    if let Some(shape) = as_shape_3d(value).and_then(water_shape_from_shape_3d) {
                        node.shape = shape;
                        node.depth = shape.depth(node.depth);
                        if let Some(density) = sim_cells_per_meter {
                            node.resolution = water_resolution_from_density(node.shape, density);
                        }
                        if let Some(density) = render_vertices_per_meter {
                            node.render_resolution =
                                water_resolution_from_density(node.shape, density);
                        }
                    }
                }
                _ => {
                    if let Some(shape) = as_shape_2d(value).and_then(water_shape_from_shape_2d) {
                        node.shape = shape;
                        if let Some(density) = sim_cells_per_meter {
                            node.resolution = water_resolution_from_density(node.shape, density);
                        }
                        if let Some(density) = render_vertices_per_meter {
                            node.render_resolution =
                                water_resolution_from_density(node.shape, density);
                        }
                    }
                }
            },
            WaterBodyField::Resolution => {
                if let Some(resolution) = water_resolution_value(value) {
                    node.resolution = resolution;
                }
            }
            WaterBodyField::RenderResolution => {
                if let Some(resolution) = water_resolution_value(value) {
                    node.render_resolution = resolution;
                }
            }
            WaterBodyField::VerticesPerMeter => {
                if let Some(v) = as_f32(value) {
                    let density = v.max(0.01);
                    sim_cells_per_meter = Some(density);
                    render_vertices_per_meter = Some(density);
                    node.resolution = water_resolution_from_density(node.shape, density);
                    node.render_resolution = water_resolution_from_density(node.shape, density);
                }
            }
            WaterBodyField::SimCellsPerMeter => {
                if let Some(v) = as_f32(value) {
                    let density = v.max(0.01);
                    sim_cells_per_meter = Some(density);
                    node.resolution = water_resolution_from_density(node.shape, density);
                }
            }
            WaterBodyField::RenderVerticesPerMeter => {
                if let Some(v) = as_f32(value) {
                    let density = v.max(0.01);
                    render_vertices_per_meter = Some(density);
                    node.render_resolution = water_resolution_from_density(node.shape, density);
                }
            }
            WaterBodyField::Depth => {
                if let Some(v) = as_f32(value) {
                    node.depth = v.max(0.0);
                    if let WaterShape::Box { size } = node.shape {
                        node.shape = WaterShape::box_volume(Vector3::new(
                            size.x,
                            node.depth.max(0.001),
                            size.z,
                        ));
                    }
                }
            }
            WaterBodyField::Flow => {
                if let Some(v) = as_vec2(value) {
                    node.flow = v;
                }
            }
            WaterBodyField::Wind => {
                if let Some(v) = as_vec2(value) {
                    node.wind = v;
                }
            }
            WaterBodyField::IdleMode => {
                if let Some(v) = as_water_idle_mode(value) {
                    node.idle_mode = v;
                }
            }
            WaterBodyField::WaveSpeed => {
                if let Some(v) = as_f32(value) {
                    node.wave.speed = v.max(0.0);
                }
            }
            WaterBodyField::WaveScale => {
                if let Some(v) = as_f32(value) {
                    node.wave.scale = v.max(0.0);
                }
            }
            WaterBodyField::WaveLength => {
                if let Some(v) = as_f32(value) {
                    node.wave.length = v.max(0.001);
                }
            }
            WaterBodyField::WakeStrength => {
                if let Some(v) = as_f32(value) {
                    node.physics.wake_strength = v.max(0.0);
                }
            }
            WaterBodyField::FoamStrength => {
                if let Some(v) = as_f32(value) {
                    node.physics.foam_strength = v.max(0.0);
                }
            }
            WaterBodyField::Damping => {
                if let Some(v) = as_f32(value) {
                    node.wave.damping = v.clamp(0.0, 1.0);
                }
            }
            WaterBodyField::Buoyancy => {
                if let Some(v) = as_f32(value) {
                    node.physics.buoyancy = v.max(0.0);
                }
            }
            WaterBodyField::Drag => {
                if let Some(v) = as_f32(value) {
                    node.physics.drag = v.max(0.0);
                }
            }
            WaterBodyField::SampleReadbackRate => {
                if let Some(v) = as_f32(value) {
                    node.physics.sample_readback_rate = v.max(0.0);
                }
            }
            WaterBodyField::LodNearDistance => {
                if let Some(v) = as_f32(value) {
                    node.lod.near_distance = v.max(0.0);
                }
            }
            WaterBodyField::LodMidDistance => {
                if let Some(v) = as_f32(value) {
                    node.lod.mid_distance = v.max(node.lod.near_distance);
                }
            }
            WaterBodyField::LodFarDistance => {
                if let Some(v) = as_f32(value) {
                    node.lod.far_distance = v.max(node.lod.mid_distance);
                }
            }
            WaterBodyField::LodMinResolution => {
                if let Some((x, y)) = value.as_vec2() {
                    node.lod.min_resolution = [
                        (x.max(1.0).round() as u32).clamp(1, 4096),
                        (y.max(1.0).round() as u32).clamp(1, 4096),
                    ];
                } else if let Some(v) = as_i32(value) {
                    let v = v.clamp(1, 4096) as u32;
                    node.lod.min_resolution = [v, v];
                }
            }
            WaterBodyField::CollisionLayers => {
                if let Some(v) = as_bitmask(value) {
                    node.collision_layers = v;
                }
            }
            WaterBodyField::CollisionMask => {
                if let Some(v) = as_bitmask(value) {
                    node.collision_mask = v;
                }
            }
            WaterBodyField::LinkLayers => {
                if let Some(v) = as_bitmask(value) {
                    node.link.link_layers = v;
                }
            }
            WaterBodyField::LinkMask => {
                if let Some(v) = as_bitmask(value) {
                    node.link.link_mask = v;
                }
            }
            WaterBodyField::BlendWidth => {
                if let Some(v) = as_f32(value) {
                    node.link.blend_width = v.max(0.0);
                }
            }
            WaterBodyField::WaveTransfer => {
                if let Some(v) = as_f32(value) {
                    node.link.wave_transfer = v.max(0.0);
                }
            }
            WaterBodyField::FlowTransfer => {
                if let Some(v) = as_f32(value) {
                    node.link.flow_transfer = v.max(0.0);
                }
            }
            WaterBodyField::DeepColor => {
                if let Some(v) = as_color(value) {
                    node.optics.deep_color = v;
                }
            }
            WaterBodyField::ShallowColor => {
                if let Some(v) = as_color(value) {
                    node.optics.shallow_color = v;
                }
            }
            WaterBodyField::ShallowDepth => {
                if let Some(v) = as_f32(value) {
                    node.optics.shallow_depth = v.max(-1.0);
                }
            }
            WaterBodyField::SkyBias => {
                if let Some(v) = as_water_sky_bias(value) {
                    node.optics.sky_bias = v;
                }
            }
            WaterBodyField::Optics => {
                apply_water_optics_settings(&mut node.optics, value);
            }
            WaterBodyField::Material => {
                apply_water_visual_params(&mut node.visual, value);
            }
            WaterBodyField::Transparency => {
                if let Some(v) = as_f32(value) {
                    node.visual.transparency = v.clamp(0.0, 1.0);
                }
            }
            WaterBodyField::Reflectivity => {
                if let Some(v) = as_f32(value) {
                    node.visual.reflectivity = v.clamp(0.0, 1.0);
                }
            }
            WaterBodyField::Roughness => {
                if let Some(v) = as_f32(value) {
                    node.visual.roughness = v.clamp(0.0, 1.0);
                }
            }
            WaterBodyField::FresnelPower => {
                if let Some(v) = as_f32(value) {
                    node.visual.fresnel_power = v.max(0.001);
                }
            }
            WaterBodyField::NormalStrength => {
                if let Some(v) = as_f32(value) {
                    node.visual.normal_strength = v.max(0.0);
                }
            }
            WaterBodyField::RippleScale => {
                if let Some(v) = as_f32(value) {
                    node.visual.ripple_scale = v.max(0.001);
                }
            }
            WaterBodyField::FoamColor => {
                if let Some(v) = as_color(value) {
                    node.visual.foam_color = v;
                }
            }
            WaterBodyField::FoamAmount => {
                if let Some(v) = as_f32(value) {
                    node.visual.foam_amount = v.max(0.0);
                }
            }
            WaterBodyField::CrestFoamThreshold => {
                if let Some(v) = as_f32(value) {
                    node.visual.crest_foam_threshold = v.max(0.0);
                }
            }
            WaterBodyField::CausticStrength => {
                if let Some(v) = as_f32(value) {
                    node.visual.caustic_strength = v.max(0.0);
                }
            }
            WaterBodyField::RefractionStrength => {
                if let Some(v) = as_f32(value) {
                    node.visual.refraction_strength = v.max(0.0);
                }
            }
            WaterBodyField::ScatteringStrength => {
                if let Some(v) = as_f32(value) {
                    node.visual.scattering_strength = v.max(0.0);
                }
            }
            WaterBodyField::DistanceFogStrength => {
                if let Some(v) = as_f32(value) {
                    node.visual.distance_fog_strength = v.max(0.0);
                }
            }
            WaterBodyField::Coastline => {
                apply_coastline_settings(&mut node.coastline, value);
            }
            WaterBodyField::Debug => {
                if let Some(v) = as_bool(value) {
                    node.debug = v;
                }
            }
        }
    });
}

fn water_resolution_value(value: &SceneValue) -> Option<[u32; 2]> {
    if let Some((x, y)) = value.as_vec2() {
        Some([
            (x.max(1.0).round() as u32).clamp(1, 4096),
            (y.max(1.0).round() as u32).clamp(1, 4096),
        ])
    } else {
        as_i32(value).map(|v| {
            let v = v.clamp(1, 4096) as u32;
            [v, v]
        })
    }
}

fn water_resolution_from_density(shape: WaterShape, vertices_per_meter: f32) -> [u32; 2] {
    let size = shape.surface_size();
    [
        ((size.x.abs() * vertices_per_meter).ceil() as u32 + 1).clamp(1, 4096),
        ((size.y.abs() * vertices_per_meter).ceil() as u32 + 1).clamp(1, 4096),
    ]
}

fn water_shape_from_shape_2d(shape: Shape2D) -> Option<WaterShape> {
    match shape {
        Shape2D::Quad { width, height } => Some(WaterShape::rect(Vector2::new(
            width.max(0.001),
            height.max(0.001),
        ))),
        Shape2D::Circle { radius } => Some(WaterShape::Circle {
            radius: radius.max(0.001),
        }),
        Shape2D::Triangle { .. } => None,
    }
}

fn water_shape_from_shape_3d(shape: Shape3D) -> Option<WaterShape> {
    match shape {
        Shape3D::Cube { size } => Some(WaterShape::box_volume(Vector3::new(
            size.x.max(0.001),
            size.y.max(0.001),
            size.z.max(0.001),
        ))),
        Shape3D::Cylinder {
            radius,
            half_height,
        } => Some(WaterShape::Cylinder {
            radius: radius.max(0.001),
            half_height: half_height.max(0.001),
        }),
        Shape3D::Sphere { radius } => Some(WaterShape::Cylinder {
            radius: radius.max(0.001),
            half_height: radius.max(0.001),
        }),
        _ => None,
    }
}

fn apply_water_optics_settings(node: &mut perro_nodes::WaterOpticsSettings, value: &SceneValue) {
    let SceneValue::Object(fields) = value else {
        return;
    };
    for (name, value) in fields.iter() {
        match name.as_ref() {
            "deep_color" | "deep" => {
                if let Some(v) = as_color(value) {
                    node.deep_color = v;
                }
            }
            "shallow_color" | "shallow" => {
                if let Some(v) = as_color(value) {
                    node.shallow_color = v;
                }
            }
            "shallow_depth" | "shallow_cutoff" | "shallowness" | "shallowness_depth" => {
                if let Some(v) = as_f32(value) {
                    node.shallow_depth = v.max(-1.0);
                }
            }
            "sky_bias" | "sky_reflect" | "sky_reflection" => {
                if let Some(v) = as_water_sky_bias(value) {
                    node.sky_bias = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_water_visual_params(node: &mut perro_nodes::WaterVisualParams, value: &SceneValue) {
    let SceneValue::Object(fields) = value else {
        return;
    };
    for (name, value) in fields.iter() {
        match name.as_ref() {
            "transparency" => {
                if let Some(v) = as_f32(value) {
                    node.transparency = v.clamp(0.0, 1.0);
                }
            }
            "reflectivity" | "reflection_strength" => {
                if let Some(v) = as_f32(value) {
                    node.reflectivity = v.clamp(0.0, 1.0);
                }
            }
            "roughness" => {
                if let Some(v) = as_f32(value) {
                    node.roughness = v.clamp(0.0, 1.0);
                }
            }
            "fresnel_power" => {
                if let Some(v) = as_f32(value) {
                    node.fresnel_power = v.max(0.001);
                }
            }
            "normal_strength" => {
                if let Some(v) = as_f32(value) {
                    node.normal_strength = v.max(0.0);
                }
            }
            "ripple_scale" => {
                if let Some(v) = as_f32(value) {
                    node.ripple_scale = v.max(0.001);
                }
            }
            "foam_color" => {
                if let Some(v) = as_color(value) {
                    node.foam_color = v;
                }
            }
            "foam_amount" => {
                if let Some(v) = as_f32(value) {
                    node.foam_amount = v.max(0.0);
                }
            }
            "crest_foam_threshold" => {
                if let Some(v) = as_f32(value) {
                    node.crest_foam_threshold = v.max(0.0);
                }
            }
            "caustic_strength" => {
                if let Some(v) = as_f32(value) {
                    node.caustic_strength = v.max(0.0);
                }
            }
            "refraction_strength" => {
                if let Some(v) = as_f32(value) {
                    node.refraction_strength = v.max(0.0);
                }
            }
            "scattering_strength" => {
                if let Some(v) = as_f32(value) {
                    node.scattering_strength = v.max(0.0);
                }
            }
            "distance_fog_strength" => {
                if let Some(v) = as_f32(value) {
                    node.distance_fog_strength = v.max(0.0);
                }
            }
            _ => {}
        }
    }
}

fn apply_coastline_settings(node: &mut perro_nodes::CoastlineSettings, value: &SceneValue) {
    let SceneValue::Object(fields) = value else {
        return;
    };
    for (name, value) in fields.iter() {
        match name.as_ref() {
            "foam_color" => {
                if let Some(v) = as_color(value) {
                    node.foam_color = v;
                }
            }
            "foam_strength" => {
                if let Some(v) = as_f32(value) {
                    node.foam_strength = v.max(0.0);
                }
            }
            "foam_width" => {
                if let Some(v) = as_f32(value) {
                    node.foam_width = v.max(0.0);
                }
            }
            "cutoff_softness" => {
                if let Some(v) = as_f32(value) {
                    node.cutoff_softness = v.max(0.0);
                }
            }
            "wave_reflection" => {
                if let Some(v) = as_f32(value) {
                    node.wave_reflection = v.clamp(0.0, 1.0);
                }
            }
            "wave_damping" => {
                if let Some(v) = as_f32(value) {
                    node.wave_damping = v.clamp(0.0, 1.0);
                }
            }
            "edge_noise" => {
                if let Some(v) = as_f32(value) {
                    node.edge_noise = v.max(0.0);
                }
            }
            _ => {}
        }
    }
}

fn as_water_sky_bias(value: &SceneValue) -> Option<WaterSkyBias> {
    if let Some(v) = as_f32(value) {
        return if v <= 0.0 {
            Some(WaterSkyBias::None)
        } else {
            Some(WaterSkyBias::Active {
                ratio: v.clamp(0.0, 1.0),
            })
        };
    }
    if let Some(v) = as_str(value) {
        return match v.trim().to_ascii_lowercase().as_str() {
            "none" | "off" | "false" => Some(WaterSkyBias::None),
            "active" | "sky" | "on" | "true" => Some(WaterSkyBias::Active { ratio: 0.35 }),
            _ => None,
        };
    }
    let SceneValue::Object(fields) = value else {
        return None;
    };
    let mut ratio = 0.35;
    let mut active = true;
    for (name, value) in fields.iter() {
        match name.as_ref() {
            "ratio" | "strength" | "amount" => {
                if let Some(v) = as_f32(value) {
                    ratio = v.clamp(0.0, 1.0);
                }
            }
            "active" => {
                if let Some(v) = as_bool(value) {
                    active = v;
                }
            }
            "mode" | "type" => {
                if let Some(v) = as_str(value)
                    && matches!(v.trim().to_ascii_lowercase().as_str(), "none" | "off")
                {
                    active = false;
                }
            }
            _ => {}
        }
    }
    if active && ratio > 0.0 {
        Some(WaterSkyBias::Active { ratio })
    } else {
        Some(WaterSkyBias::None)
    }
}

fn as_color(value: &SceneValue) -> Option<Color> {
    match value {
        SceneValue::Vec4 { x, y, z, w } => Some(Color::new(*x, *y, *z, *w)),
        SceneValue::Vec3 { x, y, z } => Some(Color::rgb(*x, *y, *z)),
        SceneValue::Str(v) => Color::from_hex(v.as_ref()),
        SceneValue::Key(v) => Color::from_hex(v.as_ref()),
        _ => None,
    }
}

fn as_water_idle_mode(value: &SceneValue) -> Option<WaterIdleMode> {
    match as_str(value)?.trim().to_ascii_lowercase().as_str() {
        "calm" => Some(WaterIdleMode::Calm),
        "sine" => Some(WaterIdleMode::Sine),
        "chop" | "choppy" => Some(WaterIdleMode::Chop),
        "storm" => Some(WaterIdleMode::Storm),
        "river" => Some(WaterIdleMode::River),
        _ => None,
    }
}

fn as_force_profile(value: &SceneValue) -> Option<PhysicsForceProfile> {
    match as_str(value)?.trim().to_ascii_lowercase().as_str() {
        "lift" => Some(PhysicsForceProfile::Lift),
        "explosion" => Some(PhysicsForceProfile::Explosion),
        "current" => Some(PhysicsForceProfile::Current),
        "vortex" => Some(PhysicsForceProfile::Vortex),
        "custom" => Some(PhysicsForceProfile::Custom),
        _ => None,
    }
}

fn as_vec2_array(value: &SceneValue) -> Option<Vec<Vector2>> {
    let SceneValue::Array(items) = value else {
        return None;
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items.iter() {
        out.push(as_vec2(item)?);
    }
    Some(out)
}

fn as_vec3_array(value: &SceneValue) -> Option<Vec<Vector3>> {
    let SceneValue::Array(items) = value else {
        return None;
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items.iter() {
        out.push(as_vec3(item)?);
    }
    Some(out)
}

pub(super) struct PreparedScene {
    pub(super) root_key: Option<u32>,
    pub(super) nodes: Vec<PendingNode>,
    pub(super) scripts: Vec<PendingScript>,
}

pub(super) struct PendingScript {
    pub(super) node_key: u32,
    #[cfg(test)]
    pub(super) node_key_name: String,
    pub(super) script_path_hash: u64,
    pub(super) script_mount: Option<String>,
    pub(super) scene_injected_vars: Vec<(String, SceneValue)>,
}

pub(super) struct PendingNode {
    pub(super) key: u32,
    pub(super) key_name: String,
    pub(super) parent_key: Option<u32>,
    pub(super) node: SceneNode,
    pub(super) animation_source: Option<String>,
    pub(super) animation_tree_source: Option<String>,
    pub(super) animation_tree_animations: Vec<PendingAnimationTreeAnimation>,
    pub(super) texture_source: Option<String>,
    pub(super) mesh_source: Option<String>,
    pub(super) material_surfaces: Vec<PendingSurfaceMaterial>,
    pub(super) skeleton_source: Option<String>,
    pub(super) mesh_skeleton_target: Option<u32>,
    pub(super) bone_attachment_skeleton_target: Option<u32>,
    pub(super) ik_target_skeleton_target: Option<u32>,
    pub(super) physics_bone_chain_skeleton_target: Option<u32>,
    pub(super) joint_body_links: Vec<PendingJointBodyLink>,
    pub(super) animation_bindings: Vec<(String, u32)>,
    pub(super) locale_text_bindings: Vec<PendingLocaleTextBinding>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PendingJointBodyField {
    BodyA,
    BodyB,
}

pub(super) struct PendingJointBodyLink {
    pub(super) field: PendingJointBodyField,
    pub(super) target_key: u32,
}

#[derive(Clone, Debug)]
pub(super) struct PendingLocaleTextBinding {
    pub(super) field: crate::runtime::state::LocaleTextField,
    pub(super) key: String,
    pub(super) key_hash: u64,
}

pub(super) struct PendingAnimationTreeAnimation {
    pub(super) source: String,
    pub(super) bindings: Vec<(String, u32)>,
    pub(super) speed: f32,
    pub(super) paused: bool,
    pub(super) playback_type: perro_nodes::AnimationPlaybackType,
}

pub(super) struct PendingSurfaceMaterial {
    pub(super) source: Option<String>,
    pub(super) inline: Option<Material3D>,
}

type AnimationSceneBindings = Vec<(String, String)>;
type AnimationTreeAnimationEntry = (
    String,
    AnimationSceneBindings,
    f32,
    bool,
    perro_nodes::AnimationPlaybackType,
);
type AnimationTreeAnimationEntries = Vec<AnimationTreeAnimationEntry>;

type SceneNodeExtraction = (
    SceneNode,
    Option<String>,
    Option<String>,
    AnimationTreeAnimationEntries,
    Option<String>,
    Option<String>,
    Vec<PendingSurfaceMaterial>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Vec<(PendingJointBodyField, String)>,
    Vec<(String, String)>,
    Vec<PendingLocaleTextBinding>,
);

pub(super) fn load_runtime_scene_from_disk(
    path: &str,
) -> Result<(Scene, RuntimeSceneLoadStats), String> {
    #[cfg(feature = "profile")]
    let source_load_start = Instant::now();
    let bytes = load_asset(path).map_err(|err| format!("failed to load scene `{path}`: {err}"))?;
    #[cfg(feature = "profile")]
    let source_load = source_load_start.elapsed();

    let source = std::str::from_utf8(&bytes)
        .map_err(|err| format!("scene `{path}` is not valid UTF-8: {err}"))?;
    #[cfg(feature = "profile")]
    let parse_start = Instant::now();
    let mut scene = Parser::new(source)
        .try_parse_scene()
        .map_err(|err| format!("failed to parse scene `{path}`: {err}"))?;
    if let Some(mount_name) = parse_dlc_mount_name(path) {
        resolve_scene_dlc_self_paths(&mut scene, &mount_name);
    }
    #[cfg(feature = "profile")]
    let parse = parse_start.elapsed();
    #[cfg(feature = "profile")]
    let stats = RuntimeSceneLoadStats { source_load, parse };
    #[cfg(not(feature = "profile"))]
    let stats = RuntimeSceneLoadStats;
    Ok((scene, stats))
}

fn parse_dlc_mount_name(path: &str) -> Option<String> {
    let rest = path.strip_prefix("dlc://")?;
    let (mount, _) = rest.split_once('/').unwrap_or((rest, ""));
    if mount.eq_ignore_ascii_case("self") || mount.is_empty() {
        return None;
    }
    Some(mount.to_string())
}

fn resolve_scene_dlc_self_paths(scene: &mut Scene, mount_name: &str) {
    let prefix = "dlc://self/";
    let replacement = format!("dlc://{mount_name}/");
    let replacement_ref = replacement.as_str();
    for node in scene.nodes.to_mut() {
        if let Some(script) = node.script.as_ref()
            && script.starts_with(prefix)
        {
            let resolved = script.replacen(prefix, replacement_ref, 1);
            node.script = Some(Cow::Owned(resolved));
        }
        if let Some(root_of) = node.root_of.as_ref()
            && root_of.starts_with(prefix)
        {
            let resolved = root_of.replacen(prefix, replacement_ref, 1);
            node.root_of = Some(Cow::Owned(resolved));
        }
        resolve_scene_value_fields_dlc_self(node.script_vars.to_mut(), prefix, replacement_ref);
        resolve_scene_node_data_dlc_self(&mut node.data, prefix, replacement_ref);
    }
}

fn resolve_scene_node_data_dlc_self(data: &mut SceneDefNodeData, prefix: &str, replacement: &str) {
    resolve_scene_value_fields_dlc_self(data.fields.to_mut(), prefix, replacement);
    if let Some(base) = data.base.as_mut()
        && let perro_scene::SceneNodeDataBase::Owned(base_data) = base
    {
        resolve_scene_node_data_dlc_self(base_data.as_mut(), prefix, replacement);
    }
}

fn resolve_scene_value_fields_dlc_self(
    fields: &mut [SceneObjectField],
    prefix: &str,
    replacement: &str,
) {
    for (_, value) in fields {
        resolve_scene_value_dlc_self(value, prefix, replacement);
    }
}

fn resolve_scene_value_dlc_self(value: &mut SceneValue, prefix: &str, replacement: &str) {
    match value {
        SceneValue::Str(v) if v.as_ref().starts_with(prefix) => {
            *v = Cow::Owned(v.replacen(prefix, replacement, 1));
        }
        SceneValue::Object(fields) => {
            for (_, item) in fields.to_mut() {
                resolve_scene_value_dlc_self(item, prefix, replacement);
            }
        }
        SceneValue::Array(values) => {
            for item in values.to_mut() {
                resolve_scene_value_dlc_self(item, prefix, replacement);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
pub(super) fn prepare_scene_with_loader(
    scene: &Scene,
    load_scene: &dyn Fn(&str) -> Result<Arc<Scene>, String>,
) -> Result<PreparedScene, String> {
    prepare_scene_with_loader_and_styles(scene, load_scene, None)
}

pub(super) fn prepare_scene_with_loader_and_styles(
    scene: &Scene,
    load_scene: &dyn Fn(&str) -> Result<Arc<Scene>, String>,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) -> Result<PreparedScene, String> {
    let mut include_stack = HashSet::new();
    prepare_scene_with_stack(
        scene,
        &mut include_stack,
        load_scene,
        static_ui_style_lookup,
    )
}

fn prepare_scene_with_stack(
    scene: &Scene,
    include_stack: &mut HashSet<String>,
    load_scene: &dyn Fn(&str) -> Result<Arc<Scene>, String>,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) -> Result<PreparedScene, String> {
    if scene.nodes.iter().all(|entry| entry.root_of.is_none()) {
        let mut prepared = prepare_scene_parallel(scene, static_ui_style_lookup)?;
        ensure_default_ray_light_3d(&mut prepared);
        return Ok(prepared);
    }

    let mut prepared_nodes = Vec::with_capacity(scene.nodes.len());
    let mut scripts = Vec::new();
    let mut next_key = scene
        .nodes
        .iter()
        .map(|node| node.key.as_u32())
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let key_map = HashMap::new();

    let mut ctx = PrepareSceneCtx {
        prepared_nodes: &mut prepared_nodes,
        scripts: &mut scripts,
        next_key: &mut next_key,
        include_stack,
        load_scene,
        static_ui_style_lookup,
        scratch: ScenePrepareScratch::default(),
    };

    for entry in scene.nodes.as_ref() {
        push_entry_prepared(scene, entry, None, &key_map, &mut ctx)?;
    }

    let mut prepared = PreparedScene {
        root_key: scene.root.map(|key| key.as_u32()),
        nodes: prepared_nodes,
        scripts,
    };
    ensure_default_ray_light_3d(&mut prepared);
    Ok(prepared)
}

struct PreparedEntry {
    node: PendingNode,
    script: Option<PendingScript>,
}

fn prepare_scene_parallel(
    scene: &Scene,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) -> Result<PreparedScene, String> {
    if scene.nodes.iter().all(|entry| entry.script.is_none()) {
        let nodes = scene
            .nodes
            .as_ref()
            .par_iter()
            .with_min_len(256)
            .map_init(ScenePrepareScratch::default, |scratch, entry| {
                prepare_node_no_root(scene, entry, static_ui_style_lookup, scratch)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut prepared = PreparedScene {
            root_key: scene.root.map(|key| key.as_u32()),
            nodes,
            scripts: Vec::new(),
        };
        ensure_default_ray_light_3d(&mut prepared);
        return Ok(prepared);
    }

    let entries = scene
        .nodes
        .as_ref()
        .par_iter()
        .with_min_len(256)
        .map_init(ScenePrepareScratch::default, |scratch, entry| {
            prepare_entry_no_root(scene, entry, static_ui_style_lookup, scratch)
        })
        .collect::<Vec<_>>();

    let mut prepared_nodes = Vec::with_capacity(entries.len());
    let mut scripts = Vec::new();
    for entry in entries {
        let entry = entry?;
        if let Some(script) = entry.script {
            scripts.push(script);
        }
        prepared_nodes.push(entry.node);
    }

    let mut prepared = PreparedScene {
        root_key: scene.root.map(|key| key.as_u32()),
        nodes: prepared_nodes,
        scripts,
    };
    ensure_default_ray_light_3d(&mut prepared);
    Ok(prepared)
}

fn ensure_default_ray_light_3d(prepared: &mut PreparedScene) {
    if !prepared
        .nodes
        .iter()
        .any(|node| node.node.node_type().is_a(NodeType::Node3D))
    {
        return;
    }
    if prepared
        .nodes
        .iter()
        .any(|node| matches!(node.node.data, SceneNodeData::RayLight3D(_)))
    {
        return;
    }
    let key = prepared
        .nodes
        .iter()
        .map(|node| node.key)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let mut node = SceneNode::new(SceneNodeData::RayLight3D(RayLight3D::new()));
    node.name = Cow::Borrowed("__perro_default_ray_light");
    prepared.nodes.push(PendingNode {
        key,
        key_name: "__perro_default_ray_light".to_string(),
        parent_key: None,
        node,
        animation_source: None,
        animation_tree_source: None,
        animation_tree_animations: Vec::new(),
        texture_source: None,
        mesh_source: None,
        material_surfaces: Vec::new(),
        skeleton_source: None,
        mesh_skeleton_target: None,
        bone_attachment_skeleton_target: None,
        ik_target_skeleton_target: None,
        physics_bone_chain_skeleton_target: None,
        joint_body_links: Vec::new(),
        animation_bindings: Vec::new(),
        locale_text_bindings: Vec::new(),
    });
}

fn prepare_entry_no_root(
    scene: &Scene,
    entry: &SceneDefNodeEntry,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
    scratch: &mut ScenePrepareScratch,
) -> Result<PreparedEntry, String> {
    let node = prepare_node_no_root(scene, entry, static_ui_style_lookup, scratch)?;
    let key_map = HashMap::new();
    let script = entry.script.as_ref().map(|script| {
        let script_path_hash = string_to_u64(script.as_ref());
        let script_mount = parse_dlc_mount_name(script.as_ref());
        PendingScript {
            node_key: node.key,
            #[cfg(test)]
            node_key_name: node.key_name.clone(),
            script_path_hash,
            script_mount,
            scene_injected_vars: entry
                .script_vars
                .iter()
                .map(|(k, v)| (k.to_string(), remap_scene_value_keys(v, scene, &key_map)))
                .collect(),
        }
    });

    Ok(PreparedEntry { node, script })
}

fn prepare_node_no_root(
    scene: &Scene,
    entry: &SceneDefNodeEntry,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
    scratch: &mut ScenePrepareScratch,
) -> Result<PendingNode, String> {
    let key = entry.key.as_u32();
    let key_name = scene.key_name_or_id(entry.key).into_owned();
    let parent_key = entry.parent.map(|p| p.as_u32());

    let (
        node,
        animation_source,
        animation_tree_source,
        animation_tree_animations,
        texture_source,
        mesh_source,
        material_surfaces,
        skeleton_source,
        mesh_skeleton_target,
        bone_attachment_skeleton_target,
        ik_target_skeleton_target,
        physics_bone_chain_skeleton_target,
        joint_body_targets,
        animation_bindings,
        locale_text_bindings,
    ) = scene_node_from_entry(entry, static_ui_style_lookup, scratch)?;

    Ok(PendingNode {
        key,
        key_name,
        parent_key,
        node,
        animation_source,
        animation_tree_source,
        animation_tree_animations: animation_tree_animations
            .into_iter()
            .map(
                |(source, bindings, speed, paused, playback_type)| PendingAnimationTreeAnimation {
                    source,
                    bindings: bindings
                        .into_iter()
                        .filter_map(|(object, target)| {
                            scene_key_by_name(scene, target.as_str())
                                .map(|target| (object, target.as_u32()))
                        })
                        .collect(),
                    speed,
                    paused,
                    playback_type,
                },
            )
            .collect(),
        texture_source,
        mesh_source,
        material_surfaces,
        skeleton_source,
        mesh_skeleton_target: mesh_skeleton_target
            .and_then(|v| scene_key_by_name(scene, v.as_str()))
            .map(|target| target.as_u32()),
        bone_attachment_skeleton_target: bone_attachment_skeleton_target
            .and_then(|v| scene_key_by_name(scene, v.as_str()))
            .map(|target| target.as_u32()),
        ik_target_skeleton_target: ik_target_skeleton_target
            .and_then(|v| scene_key_by_name(scene, v.as_str()))
            .map(|target| target.as_u32()),
        physics_bone_chain_skeleton_target: physics_bone_chain_skeleton_target
            .and_then(|v| scene_key_by_name(scene, v.as_str()))
            .map(|target| target.as_u32()),
        joint_body_links: joint_body_targets
            .into_iter()
            .filter_map(|(field, target)| {
                scene_key_by_name(scene, target.as_str()).map(|target| PendingJointBodyLink {
                    field,
                    target_key: target.as_u32(),
                })
            })
            .collect(),
        animation_bindings: animation_bindings
            .into_iter()
            .filter_map(|(object, target)| {
                scene_key_by_name(scene, target.as_str()).map(|target| (object, target.as_u32()))
            })
            .collect(),
        locale_text_bindings,
    })
}

fn push_entry_prepared(
    scene: &Scene,
    entry: &SceneDefNodeEntry,
    key_override: Option<u32>,
    key_map: &HashMap<SceneKey, u32>,
    ctx: &mut PrepareSceneCtx<'_>,
) -> Result<(), String> {
    let key = key_override.unwrap_or_else(|| remap_key(entry.key, key_map));
    let key_name = scene.key_name_or_id(entry.key).into_owned();
    let parent_key = entry.parent.map(|p| remap_key(p, key_map));
    let mut merged_root_entry = None;

    let root_of_source = entry.root_of.as_ref().map(|v| v.as_ref().to_string());
    if let Some(root_of_path) = root_of_source.as_ref() {
        if ctx.include_stack.contains(root_of_path) {
            return Err(format!(
                "root_of cycle detected while loading `{}` for host `{}`",
                root_of_path, key_name
            ));
        }
        ctx.include_stack.insert(root_of_path.clone());
        let root_merge_result = (|| {
            let import_scene = (ctx.load_scene)(root_of_path.as_str())?;
            let import_root = import_scene
                .root
                .ok_or_else(|| format!("root_of scene `{}` has no $root", root_of_path))?;
            let import_root_node = import_scene
                .nodes
                .iter()
                .find(|node| node.key == import_root)
                .ok_or_else(|| {
                    format!(
                        "root_of scene `{}` root key `{}` was not found in node list",
                        root_of_path,
                        import_scene.key_name_or_id(import_root)
                    )
                })?;
            let merged = merge_root_host_entry(entry, import_root_node);
            expand_import_children_into_host(
                key,
                root_of_path.as_str(),
                import_scene.as_ref(),
                &import_root,
                ctx,
            )?;
            Ok::<SceneDefNodeEntry, String>(merged)
        })();
        ctx.include_stack.remove(root_of_path);
        merged_root_entry = Some(root_merge_result?);
    }

    let entry = merged_root_entry.as_ref().unwrap_or(entry);

    let (
        node,
        animation_source,
        animation_tree_source,
        animation_tree_animations,
        texture_source,
        mesh_source,
        material_surfaces,
        skeleton_source,
        mesh_skeleton_target,
        bone_attachment_skeleton_target,
        ik_target_skeleton_target,
        physics_bone_chain_skeleton_target,
        joint_body_targets,
        animation_bindings,
        locale_text_bindings,
    ) = scene_node_from_entry(entry, ctx.static_ui_style_lookup, &mut ctx.scratch)?;

    #[cfg(test)]
    let test_node_key_name = key_name.clone();

    ctx.prepared_nodes.push(PendingNode {
        key,
        key_name,
        parent_key,
        node,
        animation_source,
        animation_tree_source,
        animation_tree_animations: animation_tree_animations
            .into_iter()
            .map(
                |(source, bindings, speed, paused, playback_type)| PendingAnimationTreeAnimation {
                    source,
                    bindings: bindings
                        .into_iter()
                        .filter_map(|(object, target)| {
                            scene_key_by_name(scene, target.as_str())
                                .map(|target| (object, remap_key(target, key_map)))
                        })
                        .collect(),
                    speed,
                    paused,
                    playback_type,
                },
            )
            .collect(),
        texture_source,
        mesh_source,
        material_surfaces,
        skeleton_source,
        mesh_skeleton_target: mesh_skeleton_target
            .and_then(|v| scene_key_by_name(scene, v.as_str()))
            .map(|target| remap_key(target, key_map)),
        bone_attachment_skeleton_target: bone_attachment_skeleton_target
            .and_then(|v| scene_key_by_name(scene, v.as_str()))
            .map(|target| remap_key(target, key_map)),
        ik_target_skeleton_target: ik_target_skeleton_target
            .and_then(|v| scene_key_by_name(scene, v.as_str()))
            .map(|target| remap_key(target, key_map)),
        physics_bone_chain_skeleton_target: physics_bone_chain_skeleton_target
            .and_then(|v| scene_key_by_name(scene, v.as_str()))
            .map(|target| remap_key(target, key_map)),
        joint_body_links: joint_body_targets
            .into_iter()
            .filter_map(|(field, target)| {
                scene_key_by_name(scene, target.as_str()).map(|target| PendingJointBodyLink {
                    field,
                    target_key: remap_key(target, key_map),
                })
            })
            .collect(),
        animation_bindings: animation_bindings
            .into_iter()
            .filter_map(|(object, target)| {
                scene_key_by_name(scene, target.as_str())
                    .map(|target| (object, remap_key(target, key_map)))
            })
            .collect(),
        locale_text_bindings,
    });

    if let Some(script) = entry.script.as_ref() {
        let script_path_hash = string_to_u64(script.as_ref());
        let script_mount = entry
            .script
            .as_ref()
            .and_then(|path| parse_dlc_mount_name(path.as_ref()));
        ctx.scripts.push(PendingScript {
            node_key: key,
            #[cfg(test)]
            node_key_name: test_node_key_name,
            script_path_hash,
            script_mount,
            scene_injected_vars: entry
                .script_vars
                .iter()
                .map(|(k, v)| (k.to_string(), remap_scene_value_keys(v, scene, key_map)))
                .collect(),
        });
    }

    Ok(())
}

struct PrepareSceneCtx<'a> {
    prepared_nodes: &'a mut Vec<PendingNode>,
    scripts: &'a mut Vec<PendingScript>,
    next_key: &'a mut u32,
    include_stack: &'a mut HashSet<String>,
    load_scene: &'a dyn Fn(&str) -> Result<Arc<Scene>, String>,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
    scratch: ScenePrepareScratch,
}

#[derive(Default)]
struct ScenePrepareScratch {
    fields: Vec<SceneObjectField>,
}

fn expand_import_children_into_host(
    host_key: u32,
    path: &str,
    import_scene: &Scene,
    import_root: &SceneKey,
    ctx: &mut PrepareSceneCtx<'_>,
) -> Result<(), String> {
    let mut map = HashMap::<SceneKey, u32>::new();
    map.insert(*import_root, host_key);
    for node in import_scene.nodes.as_ref() {
        if node.key == *import_root {
            continue;
        }
        let next = *ctx.next_key;
        *ctx.next_key = ctx.next_key.saturating_add(1);
        map.insert(node.key, next);
    }

    for node in import_scene.nodes.as_ref() {
        if node.key == *import_root {
            continue;
        }
        let remapped_key = map.get(&node.key).copied().ok_or_else(|| {
            format!(
                "missing remap key for `{}` in root_of `{path}`",
                import_scene.key_name_or_id(node.key)
            )
        })?;
        push_entry_prepared(import_scene, node, Some(remapped_key), &map, ctx)?;
    }
    Ok(())
}

fn merge_root_host_entry(
    host: &SceneDefNodeEntry,
    base_root: &SceneDefNodeEntry,
) -> SceneDefNodeEntry {
    let mut merged = host.clone();
    merged.name = host.name.clone().or_else(|| base_root.name.clone());
    if host.tags.is_empty() {
        merged.tags = base_root.tags.clone();
    }
    if host.children.is_empty() {
        merged.children = base_root.children.clone();
    }
    merged.parent = host.parent.or(base_root.parent);
    if host.clear_script {
        merged.script = None;
    } else if host.script.is_some() {
        merged.script = host.script.clone();
    } else {
        merged.script = base_root.script.clone();
    }
    merged.clear_script = false;
    merged.script_vars = merge_scene_object_fields(&base_root.script_vars, &host.script_vars);
    merged.data = if host.has_data_override {
        merge_scene_node_data(&base_root.data, &host.data)
    } else {
        base_root.data.clone()
    };
    merged.has_data_override = true;
    merged
}

fn merge_scene_node_data(base: &SceneDefNodeData, local: &SceneDefNodeData) -> SceneDefNodeData {
    if base.node_type != local.node_type {
        return local.clone();
    }

    let base_fields = flatten_scene_node_fields(base);
    let local_fields = flatten_scene_node_fields(local);
    let merged_fields = merge_scene_object_fields(&base_fields, &local_fields);
    SceneDefNodeData {
        node_type: local.node_type,
        fields: merged_fields,
        base: None,
    }
}

fn flatten_scene_node_fields(data: &SceneDefNodeData) -> Vec<SceneObjectField> {
    let mut out = Vec::new();
    flatten_scene_node_fields_into(data, &mut out);
    out
}

fn flatten_scene_node_fields_into(data: &SceneDefNodeData, out: &mut Vec<SceneObjectField>) {
    if let Some(base) = data.base_ref() {
        flatten_scene_node_fields_into(base, out);
    }
    out.extend(data.fields.iter().cloned());
}

fn scratch_flatten_scene_node_fields<'a>(
    data: &SceneDefNodeData,
    scratch: &'a mut ScenePrepareScratch,
) -> &'a [SceneObjectField] {
    scratch.fields.clear();
    flatten_scene_node_fields_into(data, &mut scratch.fields);
    scratch.fields.as_slice()
}

fn merge_scene_object_fields(
    base: &[SceneObjectField],
    local: &[SceneObjectField],
) -> Cow<'static, [SceneObjectField]> {
    let mut merged: BTreeMap<SceneFieldName, SceneValue> = BTreeMap::new();
    for (name, value) in base {
        merged.insert(name.clone(), value.clone());
    }
    for (name, value) in local {
        if is_unset_marker(value) {
            merged.remove(name);
            continue;
        }

        let key = name.clone();
        let next_value = if let Some(prev) = merged.get(&key) {
            merge_scene_values(prev, value)
        } else {
            value.clone()
        };
        merged.insert(key, next_value);
    }

    Cow::Owned(merged.into_iter().collect())
}

fn merge_scene_values(base: &SceneValue, local: &SceneValue) -> SceneValue {
    match (base, local) {
        (SceneValue::Object(base_fields), SceneValue::Object(local_fields)) => {
            SceneValue::Object(merge_scene_object_fields(base_fields, local_fields))
        }
        _ => local.clone(),
    }
}

fn is_unset_marker(value: &SceneValue) -> bool {
    matches!(value, SceneValue::Key(key) if key.as_ref() == "__unset__")
        || matches!(value, SceneValue::Str(text) if text.as_ref() == "__unset__")
}

fn remap_key(key: SceneKey, key_map: &HashMap<SceneKey, u32>) -> u32 {
    key_map.get(&key).copied().unwrap_or_else(|| key.as_u32())
}

fn scene_key_by_name(scene: &Scene, name: &str) -> Option<SceneKey> {
    if let Some(raw) = name.strip_prefix('#') {
        return raw.parse::<u32>().ok().map(SceneKey::new);
    }
    let name = name.strip_prefix('@').unwrap_or(name);
    scene
        .key_names
        .iter()
        .position(|key_name| key_name.as_ref() == name)
        .and_then(|idx| u32::try_from(idx).ok())
        .map(SceneKey::new)
}

fn remap_scene_value_keys(
    value: &SceneValue,
    scene: &Scene,
    key_map: &HashMap<SceneKey, u32>,
) -> SceneValue {
    match value {
        SceneValue::Bool(v) => SceneValue::Bool(*v),
        SceneValue::I32(v) => SceneValue::I32(*v),
        SceneValue::F32(v) => SceneValue::F32(*v),
        SceneValue::Vec2 { x, y } => SceneValue::Vec2 { x: *x, y: *y },
        SceneValue::Vec3 { x, y, z } => SceneValue::Vec3 {
            x: *x,
            y: *y,
            z: *z,
        },
        SceneValue::Vec4 { x, y, z, w } => SceneValue::Vec4 {
            x: *x,
            y: *y,
            z: *z,
            w: *w,
        },
        SceneValue::Str(v) => SceneValue::Str(v.clone()),
        SceneValue::Hashed(v) => SceneValue::Hashed(*v),
        SceneValue::Key(v) => scene_key_by_name(scene, v.as_ref())
            .map(|key| SceneValue::Key(format!("#{}", remap_key(key, key_map)).into()))
            .unwrap_or_else(|| SceneValue::Key(v.clone())),
        SceneValue::Object(fields) => SceneValue::Object(Cow::Owned(
            fields
                .iter()
                .map(|(k, v)| (k.clone(), remap_scene_value_keys(v, scene, key_map)))
                .collect(),
        )),
        SceneValue::Array(items) => SceneValue::Array(Cow::Owned(
            items
                .iter()
                .map(|v| remap_scene_value_keys(v, scene, key_map))
                .collect(),
        )),
    }
}
fn scene_node_from_entry(
    entry: &SceneDefNodeEntry,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
    scratch: &mut ScenePrepareScratch,
) -> Result<SceneNodeExtraction, String> {
    let mut node = SceneNode::new(scene_node_data_from(&entry.data, static_ui_style_lookup)?);
    if let Some(name) = &entry.name {
        node.name = name.clone();
    }
    if !entry.tags.is_empty() {
        let tags = entry
            .tags
            .iter()
            .map(|tag| perro_ids::NodeTag::new(tag.clone()))
            .collect::<Vec<_>>();
        node.set_tags(Some(tags));
    }
    let texture_source = extract_texture_source(&entry.data);
    let animation_source = extract_animation_source(&entry.data);
    let animation_tree_source = extract_animation_tree_source(&entry.data);
    let animation_tree_animations = extract_animation_tree_animations(&entry.data);
    let mesh_source_explicit = extract_mesh_source(&entry.data);
    let material_surfaces_explicit = extract_material_surfaces(&entry.data);
    let skeleton_source = extract_skeleton_source(&entry.data);
    let mesh_skeleton_target = extract_mesh_skeleton_target(&entry.data)?;
    let bone_attachment_skeleton_target = extract_bone_attachment_skeleton_target(&entry.data)?;
    let ik_target_skeleton_target = extract_ik_target_skeleton_target(&entry.data)?;
    let physics_bone_chain_skeleton_target =
        extract_physics_bone_chain_skeleton_target(&entry.data)?;
    let joint_body_targets = extract_joint_body_targets(&entry.data, scratch);
    let animation_bindings = extract_animation_scene_bindings(&entry.data);
    let locale_text_bindings = extract_locale_text_bindings(&entry.data, scratch);
    let model_source = extract_model_source(&entry.data);
    let (mesh_source, material_surfaces) = if let Some(model) = model_source.as_ref() {
        (
            Some(format!("{model}:mesh[0]")),
            vec![PendingSurfaceMaterial {
                source: Some(format!("{model}:mat[0]")),
                inline: None,
            }],
        )
    } else {
        (mesh_source_explicit, material_surfaces_explicit)
    };
    Ok((
        node,
        animation_source,
        animation_tree_source,
        animation_tree_animations,
        texture_source,
        mesh_source,
        material_surfaces,
        skeleton_source,
        mesh_skeleton_target,
        bone_attachment_skeleton_target,
        ik_target_skeleton_target,
        physics_bone_chain_skeleton_target,
        joint_body_targets,
        animation_bindings,
        locale_text_bindings,
    ))
}

fn extract_locale_text_bindings(
    data: &SceneDefNodeData,
    scratch: &mut ScenePrepareScratch,
) -> Vec<PendingLocaleTextBinding> {
    let mut out = Vec::new();
    match data.node_type {
        NodeType::UiLabel => {
            let fields = scratch_flatten_scene_node_fields(data, scratch);
            push_locale_text_binding(
                &mut out,
                fields,
                "text",
                crate::runtime::state::LocaleTextField::LabelText,
            );
        }
        NodeType::UiTextBox | NodeType::UiTextBlock => {
            let fields = scratch_flatten_scene_node_fields(data, scratch);
            push_locale_text_binding(
                &mut out,
                fields,
                "text",
                crate::runtime::state::LocaleTextField::TextEditText,
            );
            push_locale_text_binding(
                &mut out,
                fields,
                "placeholder",
                crate::runtime::state::LocaleTextField::TextEditPlaceholder,
            );
            push_locale_text_binding(
                &mut out,
                fields,
                "hint",
                crate::runtime::state::LocaleTextField::TextEditPlaceholder,
            );
        }
        _ => {}
    }
    out
}

fn push_locale_text_binding(
    out: &mut Vec<PendingLocaleTextBinding>,
    fields: &[SceneObjectField],
    field_name: &str,
    field: crate::runtime::state::LocaleTextField,
) {
    for (name, value) in fields {
        if name.as_ref() != field_name {
            continue;
        }
        out.retain(|binding| binding.field != field);
        let Some(raw) = as_str(value) else {
            continue;
        };
        let Some(key) = parse_locale_text_key(raw) else {
            continue;
        };
        out.push(PendingLocaleTextBinding {
            key: key.to_string(),
            key_hash: string_to_u64(key),
            field,
        });
    }
}

fn extract_joint_body_targets(
    data: &SceneDefNodeData,
    scratch: &mut ScenePrepareScratch,
) -> Vec<(PendingJointBodyField, String)> {
    let mut out = Vec::new();
    let Some((body_a_field, body_b_field)) = joint_body_fields_for(data.node_type) else {
        return out;
    };
    let fields = scratch_flatten_scene_node_fields(data, scratch);
    for (name, value) in fields {
        let resolved = resolve_scene_node_field(data.type_name(), name);
        let field = if resolved == Some(body_a_field) {
            Some(PendingJointBodyField::BodyA)
        } else if resolved == Some(body_b_field) {
            Some(PendingJointBodyField::BodyB)
        } else {
            None
        };
        if let Some(field) = field
            && let Some(target) = as_str(value)
        {
            out.push((field, target.to_string()));
        }
    }
    out
}

fn joint_body_fields_for(ty: NodeType) -> Option<(NodeField, NodeField)> {
    match ty {
        NodeType::PinJoint2D => Some((
            NodeField::PinJoint2D(Joint2DField::BodyA),
            NodeField::PinJoint2D(Joint2DField::BodyB),
        )),
        NodeType::DistanceJoint2D => Some((
            NodeField::DistanceJoint2D(DistanceJoint2DField::Common(Joint2DField::BodyA)),
            NodeField::DistanceJoint2D(DistanceJoint2DField::Common(Joint2DField::BodyB)),
        )),
        NodeType::FixedJoint2D => Some((
            NodeField::FixedJoint2D(Joint2DField::BodyA),
            NodeField::FixedJoint2D(Joint2DField::BodyB),
        )),
        NodeType::BallJoint3D => Some((
            NodeField::BallJoint3D(Joint3DField::BodyA),
            NodeField::BallJoint3D(Joint3DField::BodyB),
        )),
        NodeType::HingeJoint3D => Some((
            NodeField::HingeJoint3D(HingeJoint3DField::Common(Joint3DField::BodyA)),
            NodeField::HingeJoint3D(HingeJoint3DField::Common(Joint3DField::BodyB)),
        )),
        NodeType::FixedJoint3D => Some((
            NodeField::FixedJoint3D(Joint3DField::BodyA),
            NodeField::FixedJoint3D(Joint3DField::BodyB),
        )),
        _ => None,
    }
}

fn scene_node_data_from(
    data: &SceneDefNodeData,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) -> Result<SceneNodeData, String> {
    match data.node_type {
        NodeType::Node => Ok(SceneNodeData::Node),
        NodeType::Node2D => Ok(SceneNodeData::Node2D(build_node_2d(data))),
        NodeType::CameraStream2D => {
            Ok(SceneNodeData::CameraStream2D(build_camera_stream_2d(data)))
        }
        NodeType::Button2D => Ok(SceneNodeData::Button2D(build_button_2d(data))),
        NodeType::ImageButton2D => Ok(SceneNodeData::ImageButton2D(build_image_button_2d(data))),
        NodeType::Sprite2D => Ok(SceneNodeData::Sprite2D(build_sprite_2d(data))),
        NodeType::NineSlice2D => Ok(SceneNodeData::NineSlice2D(build_nine_slice_2d(data))),
        NodeType::AnimatedSprite2D => Ok(SceneNodeData::AnimatedSprite2D(build_animated_sprite_2d(
            data,
        ))),
        NodeType::ParticleEmitter2D => Ok(SceneNodeData::ParticleEmitter2D(build_particle_emitter_2d(
            data,
        ))),
        NodeType::AmbientLight2D => Ok(SceneNodeData::AmbientLight2D(build_ambient_light_2d(data))),
        NodeType::RayLight2D => Ok(SceneNodeData::RayLight2D(build_ray_light_2d(data))),
        NodeType::PointLight2D => Ok(SceneNodeData::PointLight2D(build_point_light_2d(data))),
        NodeType::SpotLight2D => Ok(SceneNodeData::SpotLight2D(build_spot_light_2d(data))),
        NodeType::TileMap2D => Ok(SceneNodeData::TileMap2D(build_tilemap_2d(data))),
        NodeType::WaterBody2D => Ok(SceneNodeData::WaterBody2D(build_water_body_2d(data))),
        NodeType::Skeleton2D => Ok(SceneNodeData::Skeleton2D(build_skeleton_2d(data))),
        NodeType::BoneAttachment2D => Ok(SceneNodeData::BoneAttachment2D(build_bone_attachment_2d(
            data,
        ))),
        NodeType::IKTarget2D => Ok(SceneNodeData::IKTarget2D(build_ik_target_2d(data))),
        NodeType::PhysicsBoneChain2D => Ok(SceneNodeData::PhysicsBoneChain2D(
            build_physics_bone_chain_2d(data),
        )),
        NodeType::BoneCollider2D => Ok(SceneNodeData::BoneCollider2D(build_bone_collider_2d(data))),
        NodeType::Camera2D => Ok(SceneNodeData::Camera2D(build_camera_2d(data))),
        NodeType::CollisionShape2D => Ok(SceneNodeData::CollisionShape2D(build_collision_shape_2d(
            data,
        ))),
        NodeType::StaticBody2D => Ok(SceneNodeData::StaticBody2D(build_static_body_2d(data))),
        NodeType::Area2D => Ok(SceneNodeData::Area2D(build_area_2d(data))),
        NodeType::RigidBody2D => Ok(SceneNodeData::RigidBody2D(build_rigid_body_2d(data))),
        NodeType::PhysicsForceEmitter2D => Ok(SceneNodeData::PhysicsForceEmitter2D(
            build_physics_force_emitter_2d(data),
        )),
        NodeType::PinJoint2D => Ok(SceneNodeData::PinJoint2D(build_pin_joint_2d(data))),
        NodeType::DistanceJoint2D => Ok(SceneNodeData::DistanceJoint2D(build_distance_joint_2d(
            data,
        ))),
        NodeType::FixedJoint2D => Ok(SceneNodeData::FixedJoint2D(build_fixed_joint_2d(data))),
        NodeType::AudioMask2D => Ok(SceneNodeData::AudioMask2D(build_audio_mask_2d(data))),
        NodeType::AudioEffectZone2D => Ok(SceneNodeData::AudioEffectZone2D(
            build_audio_effect_zone_2d(data),
        )),
        NodeType::AudioPortal2D => Ok(SceneNodeData::AudioPortal2D(build_audio_portal_2d(data))),
        NodeType::Node3D => Ok(SceneNodeData::Node3D(build_node_3d(data))),
        NodeType::CameraStream3D => {
            Ok(SceneNodeData::CameraStream3D(build_camera_stream_3d(data)))
        }
        NodeType::MeshInstance3D => Ok(SceneNodeData::MeshInstance3D(build_mesh_instance_3d(data))),
        NodeType::MultiMeshInstance3D => Ok(SceneNodeData::MultiMeshInstance3D(
            build_multi_mesh_instance_3d(data),
        )),
        NodeType::CollisionShape3D => Ok(SceneNodeData::CollisionShape3D(build_collision_shape_3d(
            data,
        ))),
        NodeType::StaticBody3D => Ok(SceneNodeData::StaticBody3D(build_static_body_3d(data))),
        NodeType::Area3D => Ok(SceneNodeData::Area3D(build_area_3d(data))),
        NodeType::RigidBody3D => Ok(SceneNodeData::RigidBody3D(build_rigid_body_3d(data))),
        NodeType::PhysicsForceEmitter3D => Ok(SceneNodeData::PhysicsForceEmitter3D(
            build_physics_force_emitter_3d(data),
        )),
        NodeType::BallJoint3D => Ok(SceneNodeData::BallJoint3D(build_ball_joint_3d(data))),
        NodeType::HingeJoint3D => Ok(SceneNodeData::HingeJoint3D(build_hinge_joint_3d(data))),
        NodeType::FixedJoint3D => Ok(SceneNodeData::FixedJoint3D(build_fixed_joint_3d(data))),
        NodeType::AudioMask3D => Ok(SceneNodeData::AudioMask3D(build_audio_mask_3d(data))),
        NodeType::AudioEffectZone3D => Ok(SceneNodeData::AudioEffectZone3D(
            build_audio_effect_zone_3d(data),
        )),
        NodeType::AudioPortal3D => Ok(SceneNodeData::AudioPortal3D(build_audio_portal_3d(data))),
        NodeType::Skeleton3D => Ok(SceneNodeData::Skeleton3D(build_skeleton_3d(data))),
        NodeType::BoneAttachment3D => Ok(SceneNodeData::BoneAttachment3D(build_bone_attachment_3d(
            data,
        ))),
        NodeType::IKTarget3D => Ok(SceneNodeData::IKTarget3D(build_ik_target_3d(data))),
        NodeType::PhysicsBoneChain3D => Ok(SceneNodeData::PhysicsBoneChain3D(
            build_physics_bone_chain_3d(data),
        )),
        NodeType::BoneCollider3D => Ok(SceneNodeData::BoneCollider3D(build_bone_collider_3d(data))),
        NodeType::Camera3D => Ok(SceneNodeData::Camera3D(build_camera_3d(data))),
        NodeType::ParticleEmitter3D => Ok(SceneNodeData::ParticleEmitter3D(build_particle_emitter_3d(
            data,
        ))),
        NodeType::WaterBody3D => Ok(SceneNodeData::WaterBody3D(build_water_body_3d(data))),
        NodeType::AnimationPlayer => Ok(SceneNodeData::AnimationPlayer(build_animation_player(data))),
        NodeType::AnimationTree => Ok(SceneNodeData::AnimationTree(build_animation_tree(data))),
        NodeType::AmbientLight3D => Ok(SceneNodeData::AmbientLight3D(build_ambient_light_3d(data))),
        NodeType::Sky3D => Ok(SceneNodeData::Sky3D(build_sky_3d(data))),
        NodeType::RayLight3D => Ok(SceneNodeData::RayLight3D(build_ray_light_3d(data))),
        NodeType::PointLight3D => Ok(SceneNodeData::PointLight3D(build_point_light_3d(data))),
        NodeType::SpotLight3D => Ok(SceneNodeData::SpotLight3D(build_spot_light_3d(data))),
        NodeType::UiBox => Ok(SceneNodeData::UiBox(build_ui_box(data))),
        NodeType::UiPanel => Ok(SceneNodeData::UiPanel(build_ui_panel(
            data,
            static_ui_style_lookup,
        ))),
        NodeType::UiButton => Ok(SceneNodeData::UiButton(build_ui_button(
            data,
            static_ui_style_lookup,
        ))),
        NodeType::UiShape => Ok(SceneNodeData::UiShape(build_ui_shape(data))),
        NodeType::UiCheckbox => Ok(SceneNodeData::UiCheckbox(build_ui_checkbox(
            data,
            static_ui_style_lookup,
        ))),
        NodeType::UiColorPicker => Ok(SceneNodeData::UiColorPicker(build_ui_color_picker(
            data,
            static_ui_style_lookup,
        ))),
        NodeType::UiCameraStream => Ok(SceneNodeData::UiCameraStream(build_ui_camera_stream(data))),
        NodeType::UiImage => Ok(SceneNodeData::UiImage(build_ui_image(data))),
        NodeType::UiImageButton => Ok(SceneNodeData::UiImageButton(build_ui_image_button(data))),
        NodeType::UiNineSlice => Ok(SceneNodeData::UiNineSlice(build_ui_nine_slice(data))),
        NodeType::UiAnimatedImage => Ok(SceneNodeData::UiAnimatedImage(build_ui_animated_image(
            data,
        ))),
        NodeType::UiLabel => Ok(SceneNodeData::UiLabel(build_ui_label(data))),
        NodeType::UiTextBox => Ok(SceneNodeData::UiTextBox(build_ui_text_box(
            data,
            static_ui_style_lookup,
        ))),
        NodeType::UiTextBlock => Ok(SceneNodeData::UiTextBlock(build_ui_text_block(
            data,
            static_ui_style_lookup,
        ))),
        NodeType::UiScrollContainer => Ok(SceneNodeData::UiScrollContainer(
            build_ui_scroll_container(data),
        )),
        NodeType::UiLayout => Ok(SceneNodeData::UiLayout(build_ui_layout(data))),
        NodeType::UiHLayout => Ok(SceneNodeData::UiHLayout(build_ui_hlayout(data))),
        NodeType::UiVLayout => Ok(SceneNodeData::UiVLayout(build_ui_vlayout(data))),
        NodeType::UiGrid => Ok(SceneNodeData::UiGrid(build_ui_grid(data))),
        NodeType::UiList => Ok(SceneNodeData::UiList(build_ui_list(data))),
        NodeType::UiListIndent => Ok(SceneNodeData::UiListIndent(build_ui_list_indent(data))),
    }
}

fn apply_camera_stream_fields(stream: &mut CameraStream, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "camera" | "camera_id" | "source_camera" => {
            if let Some(v) = as_node_id(value) {
                stream.camera = v;
            }
        }
        "resolution" => {
            if let Some(v) = as_vec2(value) {
                stream.resolution = UVector2::new(v.x.max(1.0) as u32, v.y.max(1.0) as u32);
            }
        }
        "width" => {
            if let Some(v) = as_u32(value) {
                stream.resolution.x = v.max(1);
            }
        }
        "height" => {
            if let Some(v) = as_u32(value) {
                stream.resolution.y = v.max(1);
            }
        }
        "aspect_ratio" | "ratio" => {
            if let Some(v) = as_f32(value) {
                stream.aspect_ratio = v.max(0.0);
            }
        }
        "aspect_mode" | "scale_mode" | "image_scale" => {
            if let Some(v) = as_str(value) {
                stream.aspect_mode = match v {
                    "stretch" | "fill" => UiImageScaleMode::Stretch,
                    "cover" | "crop" => UiImageScaleMode::Cover,
                    _ => UiImageScaleMode::Fit,
                };
            }
        }
        "post_processing" => {
            if let Some(v) = as_post_processing(value) {
                stream.post_processing = v;
            }
        }
        "enabled" | "active" => {
            if let Some(v) = as_bool(value) {
                stream.enabled = v;
            }
        }
        _ => {}
    });
    stream.resolution.x = stream.resolution.x.clamp(1, 8192);
    stream.resolution.y = stream.resolution.y.clamp(1, 8192);
}

