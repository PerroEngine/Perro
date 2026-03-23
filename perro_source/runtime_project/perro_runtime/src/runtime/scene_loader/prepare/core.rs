use crate::material_schema;
use perro_ids::IntoTagID;
use perro_io::load_asset;
use perro_nodes::{
    ambient_light_3d::AmbientLight3D,
    animation_player::AnimationPlayer,
    camera_2d::Camera2D,
    camera_3d::{Camera3D, CameraProjection},
    mesh_instance_3d::MeshInstance3D,
    node_2d::Node2D,
    node_3d::Node3D,
    particle_emitter_3d::ParticleEmitter3D,
    particle_emitter_3d::{ParticleEmitterSimMode3D, ParticleType},
    point_light_3d::PointLight3D,
    ray_light_3d::RayLight3D,
    skeleton_3d::Skeleton3D,
    spot_light_3d::SpotLight3D,
    sprite_2d::Sprite2D,
    terrain_instance_3d::TerrainInstance3D,
    Area2D, Area3D, CollisionShape2D, CollisionShape3D, RigidBody2D, RigidBody3D, SceneNode,
    SceneNodeData, Shape2D, Shape3D, StaticBody2D, StaticBody3D, Triangle2DKind,
};
use perro_render_bridge::Material3D;
use perro_scene::{
    AnimationPlayerField, Area2DField, Area3DField, Camera2DField, Camera3DField,
    CollisionShape2DField, CollisionShape3DField, Light3DField, MeshInstance3DField, Node2DField,
    Node3DField, NodeField, Parser, ParticleEmitter3DField, PointLight3DField,
    RayLight3DField, RigidBody2DField, RigidBody3DField, Scene, SceneFieldIterRef,
    SceneNodeData as SceneDefNodeData, SceneNodeEntry as SceneDefNodeEntry, SceneObjectField,
    SceneValue, Skeleton3DField, SpotLight3DField, Sprite2DField, StaticBody2DField,
    StaticBody3DField, TerrainInstance3DField, resolve_node_field,
};
use perro_structs::{
    CustomPostParam, CustomPostParamValue, PostProcessEffect, PostProcessSet, Quaternion, Vector2,
    Vector3,
};
use std::borrow::Cow;
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

pub(super) struct PreparedScene {
    pub(super) root_key: Option<String>,
    pub(super) nodes: Vec<PendingNode>,
    pub(super) scripts: Vec<PendingScript>,
}

pub(super) struct PendingScript {
    pub(super) node_key: String,
    pub(super) script_path: String,
}

pub(super) struct PendingNode {
    pub(super) key: String,
    pub(super) parent_key: Option<String>,
    pub(super) node: SceneNode,
    pub(super) animation_source: Option<String>,
    pub(super) texture_source: Option<String>,
    pub(super) mesh_source: Option<String>,
    pub(super) material_source: Option<String>,
    pub(super) material_inline: Option<Material3D>,
    pub(super) skeleton_source: Option<String>,
    pub(super) mesh_skeleton_target: Option<String>,
    pub(super) animation_bindings: Vec<(String, String)>,
}

type SceneNodeExtraction = (
    SceneNode,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<Material3D>,
    Option<String>,
    Option<String>,
    Vec<(String, String)>,
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
    let scene = Parser::new(source).parse_scene();
    #[cfg(feature = "profile")]
    let parse = parse_start.elapsed();
    #[cfg(feature = "profile")]
    let stats = RuntimeSceneLoadStats { source_load, parse };
    #[cfg(not(feature = "profile"))]
    let stats = RuntimeSceneLoadStats;
    Ok((scene, stats))
}

pub(super) fn prepare_scene(scene: &Scene) -> Result<PreparedScene, String> {
    let mut prepared_nodes = Vec::with_capacity(scene.nodes.len());
    let mut scripts = Vec::new();

    for entry in scene.nodes.as_ref() {
        let (
            node,
            animation_source,
            texture_source,
            mesh_source,
            material_source,
            material_inline,
            skeleton_source,
            mesh_skeleton_target,
            animation_bindings,
        ) = scene_node_from_entry(entry)?;
        if let Some(script) = &entry.script {
            scripts.push(PendingScript {
                node_key: entry.key.as_ref().to_string(),
                script_path: script.to_string(),
            });
        }
        prepared_nodes.push(PendingNode {
            key: entry.key.as_ref().to_string(),
            parent_key: entry.parent.as_ref().map(|p| p.as_ref().to_string()),
            node,
            animation_source,
            texture_source,
            mesh_source,
            material_source,
            material_inline,
            skeleton_source,
            mesh_skeleton_target,
            animation_bindings,
        });
    }

    Ok(PreparedScene {
        root_key: scene.root.as_ref().map(|k| k.as_ref().to_string()),
        nodes: prepared_nodes,
        scripts,
    })
}
fn scene_node_from_entry(entry: &SceneDefNodeEntry) -> Result<SceneNodeExtraction, String> {
    let mut node = SceneNode::new(scene_node_data_from(&entry.data)?);
    if let Some(name) = &entry.name {
        node.name = name.clone();
    }
    if !entry.tags.is_empty() {
        let tags = entry
            .tags
            .iter()
            .map(|tag| tag.as_ref().into_tag_id())
            .collect::<Vec<_>>();
        node.set_tag_ids(Some(tags));
    }
    let texture_source = extract_texture_source(&entry.data);
    let animation_source = extract_animation_source(&entry.data);
    let mesh_source_explicit = extract_mesh_source(&entry.data);
    let material_source_explicit = extract_material_source(&entry.data);
    let material_inline = extract_material_inline(&entry.data);
    let skeleton_source = extract_skeleton_source(&entry.data);
    let mesh_skeleton_target = extract_mesh_skeleton_target(&entry.data);
    let animation_bindings = extract_animation_scene_bindings(&entry.data);
    let model_source = extract_model_source(&entry.data);
    let (mesh_source, material_source, material_inline) = if let Some(model) = model_source.as_ref()
    {
        (
            Some(format!("{model}:mesh[0]")),
            Some(format!("{model}:mat[0]")),
            None,
        )
    } else {
        (
            mesh_source_explicit,
            material_source_explicit,
            material_inline,
        )
    };
    Ok((
        node,
        animation_source,
        texture_source,
        mesh_source,
        material_source,
        material_inline,
        skeleton_source,
        mesh_skeleton_target,
        animation_bindings,
    ))
}

fn scene_node_data_from(data: &SceneDefNodeData) -> Result<SceneNodeData, String> {
    match data.ty.as_ref() {
        "Node" => Ok(SceneNodeData::Node),
        "Node2D" => Ok(SceneNodeData::Node2D(build_node_2d(data))),
        "Sprite2D" => Ok(SceneNodeData::Sprite2D(build_sprite_2d(data))),
        "Camera2D" => Ok(SceneNodeData::Camera2D(build_camera_2d(data))),
        "CollisionShape2D" => Ok(SceneNodeData::CollisionShape2D(build_collision_shape_2d(
            data,
        ))),
        "StaticBody2D" => Ok(SceneNodeData::StaticBody2D(build_static_body_2d(data))),
        "Area2D" => Ok(SceneNodeData::Area2D(build_area_2d(data))),
        "RigidBody2D" => Ok(SceneNodeData::RigidBody2D(build_rigid_body_2d(data))),
        "Node3D" => Ok(SceneNodeData::Node3D(build_node_3d(data))),
        "MeshInstance3D" => Ok(SceneNodeData::MeshInstance3D(build_mesh_instance_3d(data))),
        "CollisionShape3D" => Ok(SceneNodeData::CollisionShape3D(build_collision_shape_3d(
            data,
        ))),
        "StaticBody3D" => Ok(SceneNodeData::StaticBody3D(build_static_body_3d(data))),
        "Area3D" => Ok(SceneNodeData::Area3D(build_area_3d(data))),
        "RigidBody3D" => Ok(SceneNodeData::RigidBody3D(build_rigid_body_3d(data))),
        "Skeleton3D" => Ok(SceneNodeData::Skeleton3D(build_skeleton_3d(data))),
        "TerrainInstance3D" => Ok(SceneNodeData::TerrainInstance3D(build_terrain_instance_3d(
            data,
        ))),
        "Camera3D" => Ok(SceneNodeData::Camera3D(build_camera_3d(data))),
        "ParticleEmitter3D" => Ok(SceneNodeData::ParticleEmitter3D(build_particle_emitter_3d(
            data,
        ))),
        "AnimationPlayer" => Ok(SceneNodeData::AnimationPlayer(build_animation_player(data))),
        "AmbientLight3D" => Ok(SceneNodeData::AmbientLight3D(build_ambient_light_3d(data))),
        "RayLight3D" => Ok(SceneNodeData::RayLight3D(build_ray_light_3d(data))),
        "PointLight3D" => Ok(SceneNodeData::PointLight3D(build_point_light_3d(data))),
        "SpotLight3D" => Ok(SceneNodeData::SpotLight3D(build_spot_light_3d(data))),
        other => Err(format!("unsupported scene node type `{other}`")),
    }
}
