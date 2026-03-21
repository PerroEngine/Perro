use crate::material_schema;
use perro_ids::IntoTagID;
use perro_io::load_asset;
use perro_nodes::{
    CollisionShape2D, CollisionShape3D, RigidBody2D, RigidBody3D, Shape2D, Shape3D,
    SceneNode, SceneNodeData,
    ambient_light_3d::AmbientLight3D,
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
    Triangle2DKind, StaticBody2D, StaticBody3D,
};
use perro_render_bridge::Material3D;
use perro_scene::{
    Parser, RuntimeNodeData, RuntimeNodeEntry, RuntimeScene, RuntimeValue, StaticNodeData,
    StaticNodeEntry, StaticNodeType, StaticScene, StaticSceneValue,
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
    pub(super) texture_source: Option<String>,
    pub(super) mesh_source: Option<String>,
    pub(super) material_source: Option<String>,
    pub(super) material_inline: Option<Material3D>,
    pub(super) skeleton_source: Option<String>,
    pub(super) mesh_skeleton_target: Option<String>,
}

type SceneNodeExtraction = (
    SceneNode,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<Material3D>,
    Option<String>,
    Option<String>,
);

pub(super) fn load_runtime_scene_from_disk(
    path: &str,
) -> Result<(RuntimeScene, RuntimeSceneLoadStats), String> {
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

pub(super) fn prepare_static_scene(scene: &'static StaticScene) -> Result<PreparedScene, String> {
    let mut nodes = Vec::with_capacity(scene.nodes.len());
    let mut scripts = Vec::new();

    for static_node in scene.nodes {
        let (
            node,
            texture_source,
            mesh_source,
            material_source,
            material_inline,
            skeleton_source,
            mesh_skeleton_target,
        ) = scene_node_from_static_entry(static_node)?;
        if let Some(script) = static_node.script {
            scripts.push(PendingScript {
                node_key: static_node.key.0.to_string(),
                script_path: script.to_string(),
            });
        }
        nodes.push(PendingNode {
            key: static_node.key.0.to_string(),
            parent_key: static_node.parent.map(|k| k.0.to_string()),
            node,
            texture_source,
            mesh_source,
            material_source,
            material_inline,
            skeleton_source,
            mesh_skeleton_target,
        });
    }

    Ok(PreparedScene {
        root_key: scene.root.map(|k| k.0.to_string()),
        nodes,
        scripts,
    })
}

pub(super) fn prepare_runtime_scene(scene: RuntimeScene) -> Result<PreparedScene, String> {
    let RuntimeScene { nodes, root } = scene;
    let mut prepared_nodes = Vec::with_capacity(nodes.len());
    let mut scripts = Vec::new();

    for entry in nodes {
        let (
            node,
            texture_source,
            mesh_source,
            material_source,
            material_inline,
            skeleton_source,
            mesh_skeleton_target,
        ) = scene_node_from_runtime_entry(&entry)?;
        if let Some(script) = entry.script {
            scripts.push(PendingScript {
                node_key: entry.key.clone(),
                script_path: script,
            });
        }
        prepared_nodes.push(PendingNode {
            key: entry.key,
            parent_key: entry.parent,
            node,
            texture_source,
            mesh_source,
            material_source,
            material_inline,
            skeleton_source,
            mesh_skeleton_target,
        });
    }

    Ok(PreparedScene {
        root_key: root,
        nodes: prepared_nodes,
        scripts,
    })
}
fn scene_node_from_static_entry(entry: &StaticNodeEntry) -> Result<SceneNodeExtraction, String> {
    let mut node = SceneNode::new(scene_node_data_from_static(&entry.data)?);
    if let Some(name) = entry.name {
        node.name = Cow::Borrowed(name);
    }
    if !entry.tags.is_empty() {
        let tags = entry
            .tags
            .iter()
            .map(|tag| (*tag).into_tag_id())
            .collect::<Vec<_>>();
        node.set_tag_ids(Some(tags));
    }
    let texture_source = extract_texture_source_static(&entry.data);
    let mesh_source_explicit = extract_mesh_source_static(&entry.data);
    let material_source_explicit = extract_material_source_static(&entry.data);
    let material_inline = extract_material_inline_static(&entry.data);
    let skeleton_source = extract_skeleton_source_static(&entry.data);
    let mesh_skeleton_target = extract_mesh_skeleton_target_static(&entry.data);
    let model_source = extract_model_source_static(&entry.data);
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
        texture_source,
        mesh_source,
        material_source,
        material_inline,
        skeleton_source,
        mesh_skeleton_target,
    ))
}

fn scene_node_from_runtime_entry(entry: &RuntimeNodeEntry) -> Result<SceneNodeExtraction, String> {
    let mut node = SceneNode::new(scene_node_data_from_runtime(&entry.data)?);
    if let Some(name) = &entry.name {
        node.name = Cow::Owned(name.clone());
    }
    if !entry.tags.is_empty() {
        let tags = entry
            .tags
            .iter()
            .map(|tag| tag.as_str().into_tag_id())
            .collect::<Vec<_>>();
        node.set_tag_ids(Some(tags));
    }
    let texture_source = extract_texture_source(&entry.data);
    let mesh_source_explicit = extract_mesh_source(&entry.data);
    let material_source_explicit = extract_material_source(&entry.data);
    let material_inline = extract_material_inline(&entry.data);
    let skeleton_source = extract_skeleton_source(&entry.data);
    let mesh_skeleton_target = extract_mesh_skeleton_target(&entry.data);
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
        texture_source,
        mesh_source,
        material_source,
        material_inline,
        skeleton_source,
        mesh_skeleton_target,
    ))
}

fn scene_node_data_from_runtime(data: &RuntimeNodeData) -> Result<SceneNodeData, String> {
    match data.ty.as_str() {
        "Node" => Ok(SceneNodeData::Node),
        "Node2D" => Ok(SceneNodeData::Node2D(build_runtime_node_2d(data))),
        "Sprite2D" => Ok(SceneNodeData::Sprite2D(build_runtime_sprite_2d(data))),
        "Camera2D" => Ok(SceneNodeData::Camera2D(build_runtime_camera_2d(data))),
        "CollisionShape2D" => Ok(SceneNodeData::CollisionShape2D(
            build_runtime_collision_shape_2d(data),
        )),
        "StaticBody2D" => Ok(SceneNodeData::StaticBody2D(build_runtime_static_body_2d(
            data,
        ))),
        "RigidBody2D" => Ok(SceneNodeData::RigidBody2D(build_runtime_rigid_body_2d(
            data,
        ))),
        "Node3D" => Ok(SceneNodeData::Node3D(build_runtime_node_3d(data))),
        "MeshInstance3D" => Ok(SceneNodeData::MeshInstance3D(
            build_runtime_mesh_instance_3d(data),
        )),
        "CollisionShape3D" => Ok(SceneNodeData::CollisionShape3D(
            build_runtime_collision_shape_3d(data),
        )),
        "StaticBody3D" => Ok(SceneNodeData::StaticBody3D(build_runtime_static_body_3d(
            data,
        ))),
        "RigidBody3D" => Ok(SceneNodeData::RigidBody3D(build_runtime_rigid_body_3d(
            data,
        ))),
        "Skeleton3D" => Ok(SceneNodeData::Skeleton3D(build_runtime_skeleton_3d(data))),
        "TerrainInstance3D" => Ok(SceneNodeData::TerrainInstance3D(
            build_runtime_terrain_instance_3d(data),
        )),
        "Camera3D" => Ok(SceneNodeData::Camera3D(build_runtime_camera_3d(data))),
        "ParticleEmitter3D" => Ok(SceneNodeData::ParticleEmitter3D(
            build_runtime_particle_emitter_3d(data),
        )),
        "AmbientLight3D" => Ok(SceneNodeData::AmbientLight3D(
            build_runtime_ambient_light_3d(data),
        )),
        "RayLight3D" => Ok(SceneNodeData::RayLight3D(build_runtime_ray_light_3d(data))),
        "PointLight3D" => Ok(SceneNodeData::PointLight3D(build_runtime_point_light_3d(
            data,
        ))),
        "SpotLight3D" => Ok(SceneNodeData::SpotLight3D(build_runtime_spot_light_3d(
            data,
        ))),
        other => Err(format!("unsupported scene node type `{other}`")),
    }
}

fn scene_node_data_from_static(data: &StaticNodeData) -> Result<SceneNodeData, String> {
    match data.ty {
        StaticNodeType::Node => Ok(SceneNodeData::Node),
        StaticNodeType::Node2D => Ok(SceneNodeData::Node2D(build_static_node_2d(data))),
        StaticNodeType::Sprite2D => Ok(SceneNodeData::Sprite2D(build_static_sprite_2d(data))),
        StaticNodeType::Camera2D => Ok(SceneNodeData::Camera2D(build_static_camera_2d(data))),
        StaticNodeType::CollisionShape2D => Ok(SceneNodeData::CollisionShape2D(
            build_static_collision_shape_2d(data),
        )),
        StaticNodeType::StaticBody2D => Ok(SceneNodeData::StaticBody2D(
            build_static_static_body_2d(data),
        )),
        StaticNodeType::RigidBody2D => Ok(SceneNodeData::RigidBody2D(
            build_static_rigid_body_2d(data),
        )),
        StaticNodeType::Node3D => Ok(SceneNodeData::Node3D(build_static_node_3d(data))),
        StaticNodeType::MeshInstance3D => Ok(SceneNodeData::MeshInstance3D(
            build_static_mesh_instance_3d(data),
        )),
        StaticNodeType::CollisionShape3D => Ok(SceneNodeData::CollisionShape3D(
            build_static_collision_shape_3d(data),
        )),
        StaticNodeType::StaticBody3D => Ok(SceneNodeData::StaticBody3D(
            build_static_static_body_3d(data),
        )),
        StaticNodeType::RigidBody3D => Ok(SceneNodeData::RigidBody3D(
            build_static_rigid_body_3d(data),
        )),
        StaticNodeType::Skeleton3D => Ok(SceneNodeData::Skeleton3D(build_static_skeleton_3d(data))),
        StaticNodeType::TerrainInstance3D => Ok(SceneNodeData::TerrainInstance3D(
            build_static_terrain_instance_3d(data),
        )),
        StaticNodeType::Camera3D => Ok(SceneNodeData::Camera3D(build_static_camera_3d(data))),
        StaticNodeType::ParticleEmitter3D => Ok(SceneNodeData::ParticleEmitter3D(
            build_static_particle_emitter_3d(data),
        )),
        StaticNodeType::AmbientLight3D => Ok(SceneNodeData::AmbientLight3D(
            build_static_ambient_light_3d(data),
        )),
        StaticNodeType::RayLight3D => {
            Ok(SceneNodeData::RayLight3D(build_static_ray_light_3d(data)))
        }
        StaticNodeType::PointLight3D => Ok(SceneNodeData::PointLight3D(
            build_static_point_light_3d(data),
        )),
        StaticNodeType::SpotLight3D => {
            Ok(SceneNodeData::SpotLight3D(build_static_spot_light_3d(data)))
        }
    }
}

// Runtime node builders (grouped by spatial domain)
fn build_runtime_node_2d(data: &RuntimeNodeData) -> Node2D {
    let mut node = Node2D::new();
    apply_node_2d_data(&mut node, data);
    node
}

fn build_runtime_sprite_2d(data: &RuntimeNodeData) -> Sprite2D {
    let mut node = Sprite2D::new();
    if let Some(base) = &data.base {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_sprite_2d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_camera_2d(data: &RuntimeNodeData) -> Camera2D {
    let mut node = Camera2D::new();
    if let Some(base) = &data.base {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_camera_2d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_collision_shape_2d(data: &RuntimeNodeData) -> CollisionShape2D {
    let mut node = CollisionShape2D::new();
    if let Some(base) = &data.base {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_collision_shape_2d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_static_body_2d(data: &RuntimeNodeData) -> StaticBody2D {
    let mut node = StaticBody2D::new();
    if let Some(base) = &data.base {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_static_body_2d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_rigid_body_2d(data: &RuntimeNodeData) -> RigidBody2D {
    let mut node = RigidBody2D::new();
    if let Some(base) = &data.base {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_rigid_body_2d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_node_3d(data: &RuntimeNodeData) -> Node3D {
    let mut node = Node3D::new();
    apply_node_3d_data(&mut node, data);
    node
}

fn build_runtime_mesh_instance_3d(data: &RuntimeNodeData) -> MeshInstance3D {
    let mut node = MeshInstance3D::new();
    if let Some(base) = &data.base {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_mesh_instance_3d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_skeleton_3d(data: &RuntimeNodeData) -> Skeleton3D {
    let mut node = Skeleton3D::new();
    if let Some(base) = &data.base {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_skeleton_3d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_terrain_instance_3d(data: &RuntimeNodeData) -> TerrainInstance3D {
    let mut node = TerrainInstance3D::new();
    if let Some(base) = &data.base {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_terrain_instance_3d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_camera_3d(data: &RuntimeNodeData) -> Camera3D {
    let mut node = Camera3D::new();
    if let Some(base) = &data.base {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_camera_3d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_particle_emitter_3d(data: &RuntimeNodeData) -> ParticleEmitter3D {
    let mut node = ParticleEmitter3D::new();
    if let Some(base) = &data.base {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_particle_emitter_3d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_collision_shape_3d(data: &RuntimeNodeData) -> CollisionShape3D {
    let mut node = CollisionShape3D::new();
    if let Some(base) = &data.base {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_collision_shape_3d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_static_body_3d(data: &RuntimeNodeData) -> StaticBody3D {
    let mut node = StaticBody3D::new();
    if let Some(base) = &data.base {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_static_body_3d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_rigid_body_3d(data: &RuntimeNodeData) -> RigidBody3D {
    let mut node = RigidBody3D::new();
    if let Some(base) = &data.base {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_rigid_body_3d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_ray_light_3d(data: &RuntimeNodeData) -> RayLight3D {
    let mut node = RayLight3D::new();
    if let Some(base) = &data.base {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_ray_light_3d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_ambient_light_3d(data: &RuntimeNodeData) -> AmbientLight3D {
    let mut node = AmbientLight3D::new();
    apply_ambient_light_3d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_point_light_3d(data: &RuntimeNodeData) -> PointLight3D {
    let mut node = PointLight3D::new();
    if let Some(base) = &data.base {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_point_light_3d_fields(&mut node, &data.fields);
    node
}

fn build_runtime_spot_light_3d(data: &RuntimeNodeData) -> SpotLight3D {
    let mut node = SpotLight3D::new();
    if let Some(base) = &data.base {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_spot_light_3d_fields(&mut node, &data.fields);
    node
}

// Static node builders (grouped by spatial domain)
fn build_static_node_2d(data: &StaticNodeData) -> Node2D {
    let mut node = Node2D::new();
    apply_node_2d_data_static(&mut node, data);
    node
}

fn build_static_sprite_2d(data: &StaticNodeData) -> Sprite2D {
    let mut node = Sprite2D::new();
    if let Some(base) = data.base {
        apply_node_2d_data_static(&mut node, base);
    }
    apply_node_2d_fields_static(&mut node, data.fields);
    apply_sprite_2d_fields_static(&mut node, data.fields);
    node
}

fn build_static_camera_2d(data: &StaticNodeData) -> Camera2D {
    let mut node = Camera2D::new();
    if let Some(base) = data.base {
        apply_node_2d_data_static(&mut node, base);
    }
    apply_node_2d_fields_static(&mut node, data.fields);
    apply_camera_2d_fields_static(&mut node, data.fields);
    node
}

fn build_static_collision_shape_2d(data: &StaticNodeData) -> CollisionShape2D {
    let mut node = CollisionShape2D::new();
    if let Some(base) = data.base {
        apply_node_2d_data_static(&mut node, base);
    }
    apply_node_2d_fields_static(&mut node, data.fields);
    apply_collision_shape_2d_fields_static(&mut node, data.fields);
    node
}

fn build_static_static_body_2d(data: &StaticNodeData) -> StaticBody2D {
    let mut node = StaticBody2D::new();
    if let Some(base) = data.base {
        apply_node_2d_data_static(&mut node, base);
    }
    apply_node_2d_fields_static(&mut node, data.fields);
    apply_static_body_2d_fields_static(&mut node, data.fields);
    node
}

fn build_static_rigid_body_2d(data: &StaticNodeData) -> RigidBody2D {
    let mut node = RigidBody2D::new();
    if let Some(base) = data.base {
        apply_node_2d_data_static(&mut node, base);
    }
    apply_node_2d_fields_static(&mut node, data.fields);
    apply_rigid_body_2d_fields_static(&mut node, data.fields);
    node
}

fn build_static_node_3d(data: &StaticNodeData) -> Node3D {
    let mut node = Node3D::new();
    apply_node_3d_data_static(&mut node, data);
    node
}

fn build_static_mesh_instance_3d(data: &StaticNodeData) -> MeshInstance3D {
    let mut node = MeshInstance3D::new();
    if let Some(base) = data.base {
        apply_node_3d_data_static(&mut node, base);
    }
    apply_node_3d_fields_static(&mut node, data.fields);
    apply_mesh_instance_3d_fields_static(&mut node, data.fields);
    node
}

fn build_static_skeleton_3d(data: &StaticNodeData) -> Skeleton3D {
    let mut node = Skeleton3D::new();
    if let Some(base) = data.base {
        apply_node_3d_data_static(&mut node, base);
    }
    apply_node_3d_fields_static(&mut node, data.fields);
    apply_skeleton_3d_fields_static(&mut node, data.fields);
    node
}

fn build_static_terrain_instance_3d(data: &StaticNodeData) -> TerrainInstance3D {
    let mut node = TerrainInstance3D::new();
    if let Some(base) = data.base {
        apply_node_3d_data_static(&mut node, base);
    }
    apply_node_3d_fields_static(&mut node, data.fields);
    apply_terrain_instance_3d_fields_static(&mut node, data.fields);
    node
}

fn build_static_camera_3d(data: &StaticNodeData) -> Camera3D {
    let mut node = Camera3D::new();
    if let Some(base) = data.base {
        apply_node_3d_data_static(&mut node, base);
    }
    apply_node_3d_fields_static(&mut node, data.fields);
    apply_camera_3d_fields_static(&mut node, data.fields);
    node
}

fn build_static_particle_emitter_3d(data: &StaticNodeData) -> ParticleEmitter3D {
    let mut node = ParticleEmitter3D::new();
    if let Some(base) = data.base {
        apply_node_3d_data_static(&mut node, base);
    }
    apply_node_3d_fields_static(&mut node, data.fields);
    apply_particle_emitter_3d_fields_static(&mut node, data.fields);
    node
}

fn build_static_collision_shape_3d(data: &StaticNodeData) -> CollisionShape3D {
    let mut node = CollisionShape3D::new();
    if let Some(base) = data.base {
        apply_node_3d_data_static(&mut node, base);
    }
    apply_node_3d_fields_static(&mut node, data.fields);
    apply_collision_shape_3d_fields_static(&mut node, data.fields);
    node
}

fn build_static_static_body_3d(data: &StaticNodeData) -> StaticBody3D {
    let mut node = StaticBody3D::new();
    if let Some(base) = data.base {
        apply_node_3d_data_static(&mut node, base);
    }
    apply_node_3d_fields_static(&mut node, data.fields);
    apply_static_body_3d_fields_static(&mut node, data.fields);
    node
}

fn build_static_rigid_body_3d(data: &StaticNodeData) -> RigidBody3D {
    let mut node = RigidBody3D::new();
    if let Some(base) = data.base {
        apply_node_3d_data_static(&mut node, base);
    }
    apply_node_3d_fields_static(&mut node, data.fields);
    apply_rigid_body_3d_fields_static(&mut node, data.fields);
    node
}

fn build_static_ray_light_3d(data: &StaticNodeData) -> RayLight3D {
    let mut node = RayLight3D::new();
    if let Some(base) = data.base {
        apply_node_3d_data_static(&mut node, base);
    }
    apply_node_3d_fields_static(&mut node, data.fields);
    apply_ray_light_3d_fields_static(&mut node, data.fields);
    node
}

fn build_static_ambient_light_3d(data: &StaticNodeData) -> AmbientLight3D {
    let mut node = AmbientLight3D::new();
    apply_ambient_light_3d_fields_static(&mut node, data.fields);
    node
}

fn build_static_point_light_3d(data: &StaticNodeData) -> PointLight3D {
    let mut node = PointLight3D::new();
    if let Some(base) = data.base {
        apply_node_3d_data_static(&mut node, base);
    }
    apply_node_3d_fields_static(&mut node, data.fields);
    apply_point_light_3d_fields_static(&mut node, data.fields);
    node
}

fn build_static_spot_light_3d(data: &StaticNodeData) -> SpotLight3D {
    let mut node = SpotLight3D::new();
    if let Some(base) = data.base {
        apply_node_3d_data_static(&mut node, base);
    }
    apply_node_3d_fields_static(&mut node, data.fields);
    apply_spot_light_3d_fields_static(&mut node, data.fields);
    node
}

fn apply_node_2d_data_static(target: &mut Node2D, data: &StaticNodeData) {
    if let Some(base) = data.base {
        apply_node_2d_data_static(target, base);
    }
    apply_node_2d_fields_static(target, data.fields);
}

fn apply_node_3d_data_static(target: &mut Node3D, data: &StaticNodeData) {
    if let Some(base) = data.base {
        apply_node_3d_data_static(target, base);
    }
    apply_node_3d_fields_static(target, data.fields);
}

fn apply_node_2d_data(target: &mut Node2D, data: &RuntimeNodeData) {
    if let Some(base) = &data.base {
        apply_node_2d_data(target, base);
    }
    apply_node_2d_fields(target, &data.fields);
}

fn apply_node_3d_data(target: &mut Node3D, data: &RuntimeNodeData) {
    if let Some(base) = &data.base {
        apply_node_3d_data(target, base);
    }
    apply_node_3d_fields(target, &data.fields);
}

// Runtime field application: 2D
fn apply_node_2d_fields(node: &mut Node2D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        match name.as_str() {
            "position" => {
                if let Some(v) = as_vec2(value) {
                    node.transform.position = v;
                }
            }
            "scale" => {
                if let Some(v) = as_vec2(value) {
                    node.transform.scale = v;
                }
            }
            "rotation" => {
                if let Some(v) = as_f32(value) {
                    node.transform.rotation = v;
                }
            }
            "z_index" => {
                if let Some(v) = as_i32(value) {
                    node.z_index = v;
                }
            }
            "visible" => {
                if let Some(v) = as_bool(value) {
                    node.visible = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_sprite_2d_fields(_node: &mut Sprite2D, _fields: &[(String, RuntimeValue)]) {}

fn apply_camera_2d_fields(node: &mut Camera2D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        match name.as_str() {
            "zoom" => {
                if let Some(v) = as_f32(value) {
                    node.zoom = v;
                }
            }
            "post_processing" => {
                if let Some(v) = as_post_processing(value) {
                    node.post_processing = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    }
}

// Runtime field application: 3D
fn apply_node_3d_fields(node: &mut Node3D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        match name.as_str() {
            "position" => {
                if let Some(v) = as_vec3(value) {
                    node.transform.position = v;
                }
            }
            "scale" => {
                if let Some(v) = as_vec3(value) {
                    node.transform.scale = v;
                }
            }
            "rotation" => {
                if let Some(v) = as_quat(value) {
                    node.transform.rotation = v;
                }
            }
            "visible" => {
                if let Some(v) = as_bool(value) {
                    node.visible = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_mesh_instance_3d_fields(_node: &mut MeshInstance3D, _fields: &[(String, RuntimeValue)]) {}

fn apply_skeleton_3d_fields(_node: &mut Skeleton3D, _fields: &[(String, RuntimeValue)]) {}

fn apply_terrain_instance_3d_fields(
    node: &mut TerrainInstance3D,
    fields: &[(String, RuntimeValue)],
) {
    for (name, value) in fields {
        match name.as_str() {
            "show_debug_vertices" => {
                if let Some(v) = as_bool(value) {
                    node.show_debug_vertices = v;
                }
            }
            "show_debug_edges" => {
                if let Some(v) = as_bool(value) {
                    node.show_debug_edges = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_camera_3d_fields(node: &mut Camera3D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        match name.as_str() {
            "zoom" => {
                if let Some(v) = as_f32(value) {
                    apply_zoom_compat_projection(node, v);
                }
            }
            "projection" => {
                if let Some(v) = as_str(value) {
                    set_projection_mode(node, v);
                }
            }
            "perspective_fov_y_degrees" => {
                if let Some(v) = as_f32(value) {
                    set_projection_fov(node, v);
                }
            }
            "perspective_near" => {
                if let Some(v) = as_f32(value) {
                    set_projection_perspective_near(node, v);
                }
            }
            "perspective_far" => {
                if let Some(v) = as_f32(value) {
                    set_projection_perspective_far(node, v);
                }
            }
            "orthographic_size" => {
                if let Some(v) = as_f32(value) {
                    set_projection_ortho_size(node, v);
                }
            }
            "orthographic_near" => {
                if let Some(v) = as_f32(value) {
                    set_projection_ortho_near(node, v);
                }
            }
            "orthographic_far" => {
                if let Some(v) = as_f32(value) {
                    set_projection_ortho_far(node, v);
                }
            }
            "frustum_left" => {
                if let Some(v) = as_f32(value) {
                    set_projection_frustum_left(node, v);
                }
            }
            "frustum_right" => {
                if let Some(v) = as_f32(value) {
                    set_projection_frustum_right(node, v);
                }
            }
            "frustum_bottom" => {
                if let Some(v) = as_f32(value) {
                    set_projection_frustum_bottom(node, v);
                }
            }
            "frustum_top" => {
                if let Some(v) = as_f32(value) {
                    set_projection_frustum_top(node, v);
                }
            }
            "frustum_near" => {
                if let Some(v) = as_f32(value) {
                    set_projection_frustum_near(node, v);
                }
            }
            "frustum_far" => {
                if let Some(v) = as_f32(value) {
                    set_projection_frustum_far(node, v);
                }
            }
            "post_processing" => {
                if let Some(v) = as_post_processing(value) {
                    node.post_processing = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_particle_emitter_3d_fields(
    node: &mut ParticleEmitter3D,
    fields: &[(String, RuntimeValue)],
) {
    for (name, value) in fields {
        match name.as_str() {
            "active" => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            "looping" => {
                if let Some(v) = as_bool(value) {
                    node.looping = v;
                }
            }
            "prewarm" => {
                if let Some(v) = as_bool(value) {
                    node.prewarm = v;
                }
            }
            "spawn_rate" => {
                if let Some(v) = as_f32(value) {
                    node.spawn_rate = v.max(0.0);
                }
            }
            "seed" => {
                if let Some(v) = as_i32(value) {
                    node.seed = v.max(0) as u32;
                }
            }
            "params" => {
                if let Some(v) = as_particle_params(value) {
                    node.params = v;
                }
            }
            "profile" => {
                if let Some(v) = as_asset_source(value) {
                    node.profile = v;
                } else if let RuntimeValue::Object(entries) = value {
                    node.profile = inline_pparticle_from_runtime(entries);
                }
            }
            "sim_mode" => {
                if let Some(v) = as_particle_sim_mode(value) {
                    node.sim_mode = v;
                }
            }
            "render_mode" => {
                if let Some(v) = as_particle_render_mode(value) {
                    node.render_mode = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_ray_light_3d_fields(node: &mut RayLight3D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        match name.as_str() {
            "color" => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            "visible" => {
                if let Some(v) = as_bool(value) {
                    node.visible = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_ambient_light_3d_fields(node: &mut AmbientLight3D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        match name.as_str() {
            "color" => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_point_light_3d_fields(node: &mut PointLight3D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        match name.as_str() {
            "color" => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            "range" => {
                if let Some(v) = as_f32(value) {
                    node.range = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_spot_light_3d_fields(node: &mut SpotLight3D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        match name.as_str() {
            "color" => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32(value) {
                    node.intensity = v;
                }
            }
            "range" => {
                if let Some(v) = as_f32(value) {
                    node.range = v;
                }
            }
            "inner_angle_radians" => {
                if let Some(v) = as_f32(value) {
                    node.inner_angle_radians = v;
                }
            }
            "outer_angle_radians" => {
                if let Some(v) = as_f32(value) {
                    node.outer_angle_radians = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    }
}

// Static field application: 2D
fn apply_node_2d_fields_static(node: &mut Node2D, fields: &[(&str, StaticSceneValue)]) {
    for (name, value) in fields {
        match *name {
            "position" => {
                if let Some(v) = as_vec2_static(value) {
                    node.transform.position = v;
                }
            }
            "scale" => {
                if let Some(v) = as_vec2_static(value) {
                    node.transform.scale = v;
                }
            }
            "rotation" => {
                if let Some(v) = as_f32_static(value) {
                    node.transform.rotation = v;
                }
            }
            "z_index" => {
                if let Some(v) = as_i32_static(value) {
                    node.z_index = v;
                }
            }
            "visible" => {
                if let Some(v) = as_bool_static(value) {
                    node.visible = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_sprite_2d_fields_static(_node: &mut Sprite2D, _fields: &[(&str, StaticSceneValue)]) {}

fn apply_camera_2d_fields_static(node: &mut Camera2D, fields: &[(&str, StaticSceneValue)]) {
    for (name, value) in fields {
        match *name {
            "zoom" => {
                if let Some(v) = as_f32_static(value) {
                    node.zoom = v;
                }
            }
            "post_processing" => {
                if let Some(v) = as_post_processing_static(value) {
                    node.post_processing = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool_static(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    }
}

// Static field application: 3D
fn apply_node_3d_fields_static(node: &mut Node3D, fields: &[(&str, StaticSceneValue)]) {
    for (name, value) in fields {
        match *name {
            "position" => {
                if let Some(v) = as_vec3_static(value) {
                    node.transform.position = v;
                }
            }
            "scale" => {
                if let Some(v) = as_vec3_static(value) {
                    node.transform.scale = v;
                }
            }
            "rotation" => {
                if let Some(v) = as_quat_static(value) {
                    node.transform.rotation = v;
                }
            }
            "visible" => {
                if let Some(v) = as_bool_static(value) {
                    node.visible = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_mesh_instance_3d_fields_static(
    _node: &mut MeshInstance3D,
    _fields: &[(&str, StaticSceneValue)],
) {
}

fn apply_skeleton_3d_fields_static(_node: &mut Skeleton3D, _fields: &[(&str, StaticSceneValue)]) {}

fn apply_terrain_instance_3d_fields_static(
    node: &mut TerrainInstance3D,
    fields: &[(&str, StaticSceneValue)],
) {
    for (name, value) in fields {
        match *name {
            "show_debug_vertices" => {
                if let Some(v) = as_bool_static(value) {
                    node.show_debug_vertices = v;
                }
            }
            "show_debug_edges" => {
                if let Some(v) = as_bool_static(value) {
                    node.show_debug_edges = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_camera_3d_fields_static(node: &mut Camera3D, fields: &[(&str, StaticSceneValue)]) {
    for (name, value) in fields {
        match *name {
            "zoom" => {
                if let Some(v) = as_f32_static(value) {
                    apply_zoom_compat_projection(node, v);
                }
            }
            "projection" => {
                if let Some(v) = as_str_static(value) {
                    set_projection_mode(node, v);
                }
            }
            "perspective_fov_y_degrees" => {
                if let Some(v) = as_f32_static(value) {
                    set_projection_fov(node, v);
                }
            }
            "perspective_near" => {
                if let Some(v) = as_f32_static(value) {
                    set_projection_perspective_near(node, v);
                }
            }
            "perspective_far" => {
                if let Some(v) = as_f32_static(value) {
                    set_projection_perspective_far(node, v);
                }
            }
            "orthographic_size" => {
                if let Some(v) = as_f32_static(value) {
                    set_projection_ortho_size(node, v);
                }
            }
            "orthographic_near" => {
                if let Some(v) = as_f32_static(value) {
                    set_projection_ortho_near(node, v);
                }
            }
            "orthographic_far" => {
                if let Some(v) = as_f32_static(value) {
                    set_projection_ortho_far(node, v);
                }
            }
            "frustum_left" => {
                if let Some(v) = as_f32_static(value) {
                    set_projection_frustum_left(node, v);
                }
            }
            "frustum_right" => {
                if let Some(v) = as_f32_static(value) {
                    set_projection_frustum_right(node, v);
                }
            }
            "frustum_bottom" => {
                if let Some(v) = as_f32_static(value) {
                    set_projection_frustum_bottom(node, v);
                }
            }
            "frustum_top" => {
                if let Some(v) = as_f32_static(value) {
                    set_projection_frustum_top(node, v);
                }
            }
            "frustum_near" => {
                if let Some(v) = as_f32_static(value) {
                    set_projection_frustum_near(node, v);
                }
            }
            "frustum_far" => {
                if let Some(v) = as_f32_static(value) {
                    set_projection_frustum_far(node, v);
                }
            }
            "post_processing" => {
                if let Some(v) = as_post_processing_static(value) {
                    node.post_processing = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool_static(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_particle_emitter_3d_fields_static(
    node: &mut ParticleEmitter3D,
    fields: &[(&str, StaticSceneValue)],
) {
    for (name, value) in fields {
        match *name {
            "active" => {
                if let Some(v) = as_bool_static(value) {
                    node.active = v;
                }
            }
            "looping" => {
                if let Some(v) = as_bool_static(value) {
                    node.looping = v;
                }
            }
            "prewarm" => {
                if let Some(v) = as_bool_static(value) {
                    node.prewarm = v;
                }
            }
            "spawn_rate" => {
                if let Some(v) = as_f32_static(value) {
                    node.spawn_rate = v.max(0.0);
                }
            }
            "seed" => {
                if let Some(v) = as_i32_static(value) {
                    node.seed = v.max(0) as u32;
                }
            }
            "params" => {
                if let Some(v) = as_particle_params_static(value) {
                    node.params = v;
                }
            }
            "profile" => {
                if let Some(v) = as_asset_source_static(value) {
                    node.profile = v;
                } else if let StaticSceneValue::Object(entries) = value {
                    node.profile = inline_pparticle_from_static(entries);
                }
            }
            "sim_mode" => {
                if let Some(v) = as_particle_sim_mode_static(value) {
                    node.sim_mode = v;
                }
            }
            "render_mode" => {
                if let Some(v) = as_particle_render_mode_static(value) {
                    node.render_mode = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_ray_light_3d_fields_static(node: &mut RayLight3D, fields: &[(&str, StaticSceneValue)]) {
    for (name, value) in fields {
        match *name {
            "color" => {
                if let Some(v) = as_vec3_static(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32_static(value) {
                    node.intensity = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool_static(value) {
                    node.active = v;
                }
            }
            "visible" => {
                if let Some(v) = as_bool_static(value) {
                    node.visible = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_ambient_light_3d_fields_static(
    node: &mut AmbientLight3D,
    fields: &[(&str, StaticSceneValue)],
) {
    for (name, value) in fields {
        match *name {
            "color" => {
                if let Some(v) = as_vec3_static(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32_static(value) {
                    node.intensity = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool_static(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_point_light_3d_fields_static(
    node: &mut PointLight3D,
    fields: &[(&str, StaticSceneValue)],
) {
    for (name, value) in fields {
        match *name {
            "color" => {
                if let Some(v) = as_vec3_static(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32_static(value) {
                    node.intensity = v;
                }
            }
            "range" => {
                if let Some(v) = as_f32_static(value) {
                    node.range = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool_static(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    }
}

fn apply_spot_light_3d_fields_static(node: &mut SpotLight3D, fields: &[(&str, StaticSceneValue)]) {
    for (name, value) in fields {
        match *name {
            "color" => {
                if let Some(v) = as_vec3_static(value) {
                    node.color = [v.x, v.y, v.z];
                }
            }
            "intensity" => {
                if let Some(v) = as_f32_static(value) {
                    node.intensity = v;
                }
            }
            "range" => {
                if let Some(v) = as_f32_static(value) {
                    node.range = v;
                }
            }
            "inner_angle_radians" => {
                if let Some(v) = as_f32_static(value) {
                    node.inner_angle_radians = v;
                }
            }
            "outer_angle_radians" => {
                if let Some(v) = as_f32_static(value) {
                    node.outer_angle_radians = v;
                }
            }
            "active" => {
                if let Some(v) = as_bool_static(value) {
                    node.active = v;
                }
            }
            _ => {}
        }
    }
}

// Runtime value parsers
fn as_bool(value: &RuntimeValue) -> Option<bool> {
    match value {
        RuntimeValue::Bool(v) => Some(*v),
        _ => None,
    }
}

fn as_i32(value: &RuntimeValue) -> Option<i32> {
    match value {
        RuntimeValue::I32(v) => Some(*v),
        RuntimeValue::F32(v) => Some(*v as i32),
        _ => None,
    }
}

fn as_f32(value: &RuntimeValue) -> Option<f32> {
    match value {
        RuntimeValue::F32(v) => Some(*v),
        RuntimeValue::I32(v) => Some(*v as f32),
        _ => None,
    }
}

fn as_vec2(value: &RuntimeValue) -> Option<Vector2> {
    match value {
        RuntimeValue::Vec2 { x, y } => Some(Vector2::new(*x, *y)),
        _ => None,
    }
}

fn as_vec3(value: &RuntimeValue) -> Option<Vector3> {
    match value {
        RuntimeValue::Vec3 { x, y, z } => Some(Vector3::new(*x, *y, *z)),
        _ => None,
    }
}

fn as_quat(value: &RuntimeValue) -> Option<Quaternion> {
    match value {
        RuntimeValue::Vec4 { x, y, z, w } => Some(Quaternion::new(*x, *y, *z, *w)),
        _ => None,
    }
}

fn as_str(value: &RuntimeValue) -> Option<&str> {
    match value {
        RuntimeValue::Str(v) => Some(v.as_str()),
        RuntimeValue::Key(v) => Some(v.as_str()),
        _ => None,
    }
}

fn as_asset_source(value: &RuntimeValue) -> Option<String> {
    match value {
        RuntimeValue::Str(v) => Some(v.clone()),
        RuntimeValue::Key(v) => Some(v.clone()),
        _ => None,
    }
}

fn extract_texture_source(data: &RuntimeNodeData) -> Option<String> {
    if data.ty != "Sprite2D" {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (name == "texture")
            .then(|| as_asset_source(value))
            .flatten()
    })
}

fn extract_mesh_source(data: &RuntimeNodeData) -> Option<String> {
    if data.ty != "MeshInstance3D" {
        return None;
    }
    data.fields
        .iter()
        .find_map(|(name, value)| (name == "mesh").then(|| as_asset_source(value)).flatten())
}

fn extract_material_source(data: &RuntimeNodeData) -> Option<String> {
    if data.ty != "MeshInstance3D" {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (name == "material")
            .then(|| as_asset_source(value))
            .flatten()
    })
}

fn extract_material_inline(data: &RuntimeNodeData) -> Option<Material3D> {
    if data.ty != "MeshInstance3D" {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        if name != "material" {
            return None;
        }
        match value {
            RuntimeValue::Object(entries) => material_schema::from_runtime_object(entries),
            _ => None,
        }
    })
}

fn extract_model_source(data: &RuntimeNodeData) -> Option<String> {
    if data.ty != "MeshInstance3D" {
        return None;
    }
    data.fields
        .iter()
        .find_map(|(name, value)| (name == "model").then(|| as_asset_source(value)).flatten())
}

fn extract_skeleton_source(data: &RuntimeNodeData) -> Option<String> {
    if data.ty != "Skeleton3D" {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (name == "skeleton")
            .then(|| as_asset_source(value))
            .flatten()
    })
}

fn extract_mesh_skeleton_target(data: &RuntimeNodeData) -> Option<String> {
    if data.ty != "MeshInstance3D" {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (name == "skeleton")
            .then(|| as_asset_source(value))
            .flatten()
    })
}

// Static value parsers
fn as_bool_static(value: &StaticSceneValue) -> Option<bool> {
    match value {
        StaticSceneValue::Bool(v) => Some(*v),
        _ => None,
    }
}

fn as_i32_static(value: &StaticSceneValue) -> Option<i32> {
    match value {
        StaticSceneValue::I32(v) => Some(*v),
        StaticSceneValue::F32(v) => Some(*v as i32),
        _ => None,
    }
}

fn as_f32_static(value: &StaticSceneValue) -> Option<f32> {
    match value {
        StaticSceneValue::F32(v) => Some(*v),
        StaticSceneValue::I32(v) => Some(*v as f32),
        _ => None,
    }
}

fn as_vec2_static(value: &StaticSceneValue) -> Option<Vector2> {
    match value {
        StaticSceneValue::Vec2 { x, y } => Some(Vector2::new(*x, *y)),
        _ => None,
    }
}

fn as_vec3_static(value: &StaticSceneValue) -> Option<Vector3> {
    match value {
        StaticSceneValue::Vec3 { x, y, z } => Some(Vector3::new(*x, *y, *z)),
        _ => None,
    }
}

fn as_quat_static(value: &StaticSceneValue) -> Option<Quaternion> {
    match value {
        StaticSceneValue::Vec4 { x, y, z, w } => Some(Quaternion::new(*x, *y, *z, *w)),
        _ => None,
    }
}

fn as_str_static(value: &StaticSceneValue) -> Option<&str> {
    match value {
        StaticSceneValue::Str(v) => Some(*v),
        StaticSceneValue::Key(v) => Some(v.0),
        _ => None,
    }
}

fn inline_pparticle_from_runtime(entries: &[(String, RuntimeValue)]) -> String {
    let mut out = String::from("inline://");
    for (key, value) in entries {
        if let Some(encoded) = encode_runtime_value_for_pparticle(value) {
            out.push_str(key);
            out.push_str(" = ");
            out.push_str(&encoded);
            out.push('\n');
        }
    }
    out
}

fn inline_pparticle_from_static(entries: &[(&str, StaticSceneValue)]) -> String {
    let mut out = String::from("inline://");
    for (key, value) in entries {
        if let Some(encoded) = encode_static_value_for_pparticle(value) {
            out.push_str(key);
            out.push_str(" = ");
            out.push_str(&encoded);
            out.push('\n');
        }
    }
    out
}

fn encode_runtime_value_for_pparticle(value: &RuntimeValue) -> Option<String> {
    match value {
        RuntimeValue::Bool(v) => Some(if *v { "true" } else { "false" }.to_string()),
        RuntimeValue::I32(v) => Some(v.to_string()),
        RuntimeValue::F32(v) => Some(v.to_string()),
        RuntimeValue::Vec2 { x, y } => Some(format!("({x}, {y})")),
        RuntimeValue::Vec3 { x, y, z } => Some(format!("({x}, {y}, {z})")),
        RuntimeValue::Vec4 { x, y, z, w } => Some(format!("({x}, {y}, {z}, {w})")),
        RuntimeValue::Str(v) | RuntimeValue::Key(v) => Some(v.clone()),
        RuntimeValue::Object(_) | RuntimeValue::Array(_) => None,
    }
}

fn encode_static_value_for_pparticle(value: &StaticSceneValue) -> Option<String> {
    match value {
        StaticSceneValue::Bool(v) => Some(if *v { "true" } else { "false" }.to_string()),
        StaticSceneValue::I32(v) => Some(v.to_string()),
        StaticSceneValue::F32(v) => Some(v.to_string()),
        StaticSceneValue::Vec2 { x, y } => Some(format!("({x}, {y})")),
        StaticSceneValue::Vec3 { x, y, z } => Some(format!("({x}, {y}, {z})")),
        StaticSceneValue::Vec4 { x, y, z, w } => Some(format!("({x}, {y}, {z}, {w})")),
        StaticSceneValue::Str(v) => Some((*v).to_string()),
        StaticSceneValue::Key(v) => Some(v.0.to_string()),
        StaticSceneValue::Object(_) | StaticSceneValue::Array(_) => None,
    }
}

fn as_particle_sim_mode(value: &RuntimeValue) -> Option<ParticleEmitterSimMode3D> {
    let raw = as_str(value)?.trim().to_ascii_lowercase();
    match raw.as_str() {
        "default" => Some(ParticleEmitterSimMode3D::Default),
        "cpu" => Some(ParticleEmitterSimMode3D::Cpu),
        "hybrid" => Some(ParticleEmitterSimMode3D::GpuVertex),
        "gpu" => Some(ParticleEmitterSimMode3D::GpuCompute),
        _ => None,
    }
}

fn as_particle_render_mode(value: &RuntimeValue) -> Option<ParticleType> {
    let raw = as_str(value)?.trim().to_ascii_lowercase();
    match raw.as_str() {
        "point" => Some(ParticleType::Point),
        "billboard" => Some(ParticleType::Billboard),
        _ => None,
    }
}

fn as_particle_params(value: &RuntimeValue) -> Option<Vec<f32>> {
    match value {
        RuntimeValue::Vec2 { x, y } => Some(vec![*x, *y]),
        RuntimeValue::Vec3 { x, y, z } => Some(vec![*x, *y, *z]),
        RuntimeValue::Vec4 { x, y, z, w } => Some(vec![*x, *y, *z, *w]),
        RuntimeValue::Object(entries) => {
            let mut indexed = Vec::<(usize, f32)>::new();
            for (k, v) in entries {
                let idx = parse_param_key_index(k)?;
                let val = match v {
                    RuntimeValue::F32(n) => *n,
                    RuntimeValue::I32(n) => *n as f32,
                    _ => return None,
                };
                indexed.push((idx, val));
            }
            if indexed.is_empty() {
                return Some(Vec::new());
            }
            indexed.sort_unstable_by_key(|(i, _)| *i);
            let max = indexed.last().map(|(i, _)| *i).unwrap_or(0);
            let mut out = vec![0.0; max + 1];
            for (i, v) in indexed {
                out[i] = v;
            }
            Some(out)
        }
        _ => None,
    }
}

fn as_post_processing(value: &RuntimeValue) -> Option<PostProcessSet> {
    match value {
        RuntimeValue::Array(items) => {
            let mut effects = Vec::new();
            let mut names = Vec::new();
            for item in items {
                let (name, effect) = post_effect_from_runtime(item)?;
                effects.push(effect);
                names.push(name);
            }
            Some(PostProcessSet::from_pairs(effects, names))
        }
        RuntimeValue::Object(entries) => {
            let all_indexed = entries
                .iter()
                .all(|(k, _)| parse_param_key_index(k).is_some());
            if all_indexed {
                let mut indexed =
                    Vec::<(usize, Option<Cow<'static, str>>, PostProcessEffect)>::new();
                for (k, v) in entries {
                    let idx = parse_param_key_index(k)?;
                    let (name, effect) = post_effect_from_runtime(v)?;
                    indexed.push((idx, name, effect));
                }
                if indexed.is_empty() {
                    return Some(PostProcessSet::new());
                }
                indexed.sort_unstable_by_key(|(i, _, _)| *i);
                let mut effects = Vec::with_capacity(indexed.len());
                let mut names = Vec::with_capacity(indexed.len());
                for (_, name, effect) in indexed {
                    effects.push(effect);
                    names.push(name);
                }
                Some(PostProcessSet::from_pairs(effects, names))
            } else {
                let mut effects = Vec::with_capacity(entries.len());
                let mut names = Vec::with_capacity(entries.len());
                for (k, v) in entries {
                    let (mut name, effect) = post_effect_from_runtime(v)?;
                    if name.is_none() {
                        name = Some(Cow::Owned(k.clone()));
                    }
                    effects.push(effect);
                    names.push(name);
                }
                Some(PostProcessSet::from_pairs(effects, names))
            }
        }
        _ => None,
    }
}

fn as_post_processing_static(value: &StaticSceneValue) -> Option<PostProcessSet> {
    match value {
        StaticSceneValue::Array(items) => {
            let mut effects = Vec::new();
            let mut names = Vec::new();
            for item in items.iter() {
                let (name, effect) = post_effect_from_static(item)?;
                effects.push(effect);
                names.push(name);
            }
            Some(PostProcessSet::from_pairs(effects, names))
        }
        StaticSceneValue::Object(entries) => {
            let all_indexed = entries
                .iter()
                .all(|(k, _)| parse_param_key_index(k).is_some());
            if all_indexed {
                let mut indexed =
                    Vec::<(usize, Option<Cow<'static, str>>, PostProcessEffect)>::new();
                for (k, v) in entries.iter() {
                    let idx = parse_param_key_index(k)?;
                    let (name, effect) = post_effect_from_static(v)?;
                    indexed.push((idx, name, effect));
                }
                if indexed.is_empty() {
                    return Some(PostProcessSet::new());
                }
                indexed.sort_unstable_by_key(|(i, _, _)| *i);
                let mut effects = Vec::with_capacity(indexed.len());
                let mut names = Vec::with_capacity(indexed.len());
                for (_, name, effect) in indexed {
                    effects.push(effect);
                    names.push(name);
                }
                Some(PostProcessSet::from_pairs(effects, names))
            } else {
                let mut effects = Vec::with_capacity(entries.len());
                let mut names = Vec::with_capacity(entries.len());
                for (k, v) in entries.iter() {
                    let (mut name, effect) = post_effect_from_static(v)?;
                    if name.is_none() {
                        name = Some(Cow::Borrowed(*k));
                    }
                    effects.push(effect);
                    names.push(name);
                }
                Some(PostProcessSet::from_pairs(effects, names))
            }
        }
        _ => None,
    }
}

fn post_effect_from_runtime(
    value: &RuntimeValue,
) -> Option<(Option<Cow<'static, str>>, PostProcessEffect)> {
    let RuntimeValue::Object(entries) = value else {
        return None;
    };
    let mut name: Option<Cow<'static, str>> = None;
    let mut ty: Option<String> = None;
    let mut strength: Option<f32> = None;
    let mut size: Option<f32> = None;
    let mut waves: Option<f32> = None;
    let mut radius: Option<f32> = None;
    let mut softness: Option<f32> = None;
    let mut scanline_strength: Option<f32> = None;
    let mut curvature: Option<f32> = None;
    let mut chromatic: Option<f32> = None;
    let mut vignette: Option<f32> = None;
    let mut color: Option<[f32; 3]> = None;
    let mut threshold: Option<f32> = None;
    let mut amount: Option<f32> = None;
    let mut shader_path: Option<String> = None;
    let mut params: Option<Vec<CustomPostParam>> = None;

    for (k, v) in entries {
        match k.as_str() {
            "name" | "id" | "key" => {
                if let Some(s) = as_str(v) {
                    let s = s.trim();
                    if !s.is_empty() {
                        name = Some(Cow::Owned(s.to_string()));
                    }
                }
            }
            "type" | "effect" => {
                if let Some(s) = as_str(v) {
                    ty = Some(s.trim().to_ascii_lowercase());
                }
            }
            "strength" => strength = as_f32(v),
            "size" => size = as_f32(v),
            "waves" => waves = as_f32(v),
            "radius" => radius = as_f32(v),
            "softness" | "feather" => softness = as_f32(v),
            "scanlines" | "scanline_strength" => scanline_strength = as_f32(v),
            "curvature" => curvature = as_f32(v),
            "chromatic" | "chromatic_aberration" => chromatic = as_f32(v),
            "vignette" => vignette = as_f32(v),
            "color" | "tint" => {
                if let Some(c) = as_vec3(v) {
                    color = Some([c.x, c.y, c.z]);
                }
            }
            "threshold" => threshold = as_f32(v),
            "amount" => amount = as_f32(v),
            "shader" | "shader_path" => {
                if let Some(s) = as_str(v) {
                    shader_path = Some(s.to_string());
                }
            }
            "params" => params = as_post_params(v),
            _ => {}
        }
    }

    match ty.as_deref()? {
        "blur" => Some((
            name,
            PostProcessEffect::Blur {
                strength: strength.unwrap_or(1.0),
            },
        )),
        "pixel" | "pixelate" => Some((
            name,
            PostProcessEffect::Pixelate {
                size: size.unwrap_or(1.0),
            },
        )),
        "warp" => Some((
            name,
            PostProcessEffect::Warp {
                waves: waves.unwrap_or(1.0),
                strength: strength.unwrap_or(1.0),
            },
        )),
        "vignette" => Some((
            name,
            PostProcessEffect::Vignette {
                strength: strength.unwrap_or(0.6),
                radius: radius.unwrap_or(0.55),
                softness: softness.unwrap_or(0.25),
            },
        )),
        "crt" => Some((
            name,
            PostProcessEffect::Crt {
                scanline_strength: scanline_strength.unwrap_or(0.35),
                curvature: curvature.unwrap_or(0.15),
                chromatic: chromatic.unwrap_or(1.0),
                vignette: vignette.unwrap_or(0.25),
            },
        )),
        "colorfilter" | "color_filter" | "filter" => Some((
            name,
            PostProcessEffect::ColorFilter {
                color: color.unwrap_or([1.0, 1.0, 1.0]),
                strength: strength.unwrap_or(1.0),
            },
        )),
        "reversefilter" | "reverse_filter" | "reverse" => Some((
            name,
            PostProcessEffect::ReverseFilter {
                color: color.unwrap_or([1.0, 1.0, 1.0]),
                strength: strength.unwrap_or(1.0),
                softness: softness.unwrap_or(0.2),
            },
        )),
        "bloom" => Some((
            name,
            PostProcessEffect::Bloom {
                strength: strength.unwrap_or(0.6),
                threshold: threshold.unwrap_or(0.7),
                radius: radius.unwrap_or(1.25),
            },
        )),
        "saturate" | "saturation" => Some((
            name,
            PostProcessEffect::Saturate {
                amount: amount.or(strength).unwrap_or(1.2),
            },
        )),
        "black_white" | "blackwhite" | "bw" | "grayscale" => Some((
            name,
            PostProcessEffect::BlackWhite {
                amount: amount.or(strength).unwrap_or(1.0),
            },
        )),
        "custom" => {
            let shader_path = shader_path?;
            let params = params.unwrap_or_default();
            Some((
                name,
                PostProcessEffect::Custom {
                    shader_path: Cow::Owned(shader_path),
                    params: Cow::Owned(params),
                },
            ))
        }
        _ => None,
    }
}

fn post_effect_from_static(
    value: &StaticSceneValue,
) -> Option<(Option<Cow<'static, str>>, PostProcessEffect)> {
    let StaticSceneValue::Object(entries) = value else {
        return None;
    };
    let mut name: Option<Cow<'static, str>> = None;
    let mut ty: Option<String> = None;
    let mut strength: Option<f32> = None;
    let mut size: Option<f32> = None;
    let mut waves: Option<f32> = None;
    let mut radius: Option<f32> = None;
    let mut softness: Option<f32> = None;
    let mut scanline_strength: Option<f32> = None;
    let mut curvature: Option<f32> = None;
    let mut chromatic: Option<f32> = None;
    let mut vignette: Option<f32> = None;
    let mut color: Option<[f32; 3]> = None;
    let mut threshold: Option<f32> = None;
    let mut amount: Option<f32> = None;
    let mut shader_path: Option<String> = None;
    let mut params: Option<Vec<CustomPostParam>> = None;

    for (k, v) in entries.iter() {
        match *k {
            "name" | "id" | "key" => {
                if let Some(s) = as_str_static(v) {
                    let s = s.trim();
                    if !s.is_empty() {
                        name = Some(Cow::Borrowed(s));
                    }
                }
            }
            "type" | "effect" => {
                if let Some(s) = as_str_static(v) {
                    ty = Some(s.trim().to_ascii_lowercase());
                }
            }
            "strength" => strength = as_f32_static(v),
            "size" => size = as_f32_static(v),
            "waves" => waves = as_f32_static(v),
            "radius" => radius = as_f32_static(v),
            "softness" | "feather" => softness = as_f32_static(v),
            "scanlines" | "scanline_strength" => scanline_strength = as_f32_static(v),
            "curvature" => curvature = as_f32_static(v),
            "chromatic" | "chromatic_aberration" => chromatic = as_f32_static(v),
            "vignette" => vignette = as_f32_static(v),
            "color" | "tint" => {
                if let Some(c) = as_vec3_static(v) {
                    color = Some([c.x, c.y, c.z]);
                }
            }
            "threshold" => threshold = as_f32_static(v),
            "amount" => amount = as_f32_static(v),
            "shader" | "shader_path" => {
                if let Some(s) = as_str_static(v) {
                    shader_path = Some(s.to_string());
                }
            }
            "params" => params = as_post_params_static(v),
            _ => {}
        }
    }

    match ty.as_deref()? {
        "blur" => Some((
            name,
            PostProcessEffect::Blur {
                strength: strength.unwrap_or(1.0),
            },
        )),
        "pixel" | "pixelate" => Some((
            name,
            PostProcessEffect::Pixelate {
                size: size.unwrap_or(1.0),
            },
        )),
        "warp" => Some((
            name,
            PostProcessEffect::Warp {
                waves: waves.unwrap_or(1.0),
                strength: strength.unwrap_or(1.0),
            },
        )),
        "vignette" => Some((
            name,
            PostProcessEffect::Vignette {
                strength: strength.unwrap_or(0.6),
                radius: radius.unwrap_or(0.55),
                softness: softness.unwrap_or(0.25),
            },
        )),
        "crt" => Some((
            name,
            PostProcessEffect::Crt {
                scanline_strength: scanline_strength.unwrap_or(0.35),
                curvature: curvature.unwrap_or(0.15),
                chromatic: chromatic.unwrap_or(1.0),
                vignette: vignette.unwrap_or(0.25),
            },
        )),
        "colorfilter" | "color_filter" | "filter" => Some((
            name,
            PostProcessEffect::ColorFilter {
                color: color.unwrap_or([1.0, 1.0, 1.0]),
                strength: strength.unwrap_or(1.0),
            },
        )),
        "reversefilter" | "reverse_filter" | "reverse" => Some((
            name,
            PostProcessEffect::ReverseFilter {
                color: color.unwrap_or([1.0, 1.0, 1.0]),
                strength: strength.unwrap_or(1.0),
                softness: softness.unwrap_or(0.2),
            },
        )),
        "bloom" => Some((
            name,
            PostProcessEffect::Bloom {
                strength: strength.unwrap_or(0.6),
                threshold: threshold.unwrap_or(0.7),
                radius: radius.unwrap_or(1.25),
            },
        )),
        "saturate" | "saturation" => Some((
            name,
            PostProcessEffect::Saturate {
                amount: amount.or(strength).unwrap_or(1.2),
            },
        )),
        "black_white" | "blackwhite" | "bw" | "grayscale" => Some((
            name,
            PostProcessEffect::BlackWhite {
                amount: amount.or(strength).unwrap_or(1.0),
            },
        )),
        "custom" => {
            let shader_path = shader_path?;
            let params = params.unwrap_or_default();
            Some((
                name,
                PostProcessEffect::Custom {
                    shader_path: Cow::Owned(shader_path),
                    params: Cow::Owned(params),
                },
            ))
        }
        _ => None,
    }
}

fn as_post_params(value: &RuntimeValue) -> Option<Vec<CustomPostParam>> {
    match value {
        RuntimeValue::Array(items) => {
            let mut out = Vec::new();
            for item in items {
                out.push(CustomPostParam::unnamed(post_param_value_from_runtime(
                    item,
                )?));
            }
            Some(out)
        }
        RuntimeValue::Object(entries) => {
            let mut indexed = Vec::<(usize, CustomPostParam)>::new();
            for (k, v) in entries {
                let idx = parse_param_key_index(k)?;
                let value = post_param_value_from_runtime(v)?;
                indexed.push((idx, CustomPostParam::unnamed(value)));
            }
            if indexed.is_empty() {
                return Some(Vec::new());
            }
            indexed.sort_unstable_by_key(|(i, _)| *i);
            Some(indexed.into_iter().map(|(_, v)| v).collect())
        }
        _ => None,
    }
}

fn as_post_params_static(value: &StaticSceneValue) -> Option<Vec<CustomPostParam>> {
    match value {
        StaticSceneValue::Array(items) => {
            let mut out = Vec::new();
            for item in items.iter() {
                out.push(CustomPostParam::unnamed(post_param_value_from_static(
                    item,
                )?));
            }
            Some(out)
        }
        StaticSceneValue::Object(entries) => {
            let mut indexed = Vec::<(usize, CustomPostParam)>::new();
            for (k, v) in entries.iter() {
                let idx = parse_param_key_index(k)?;
                let value = post_param_value_from_static(v)?;
                indexed.push((idx, CustomPostParam::unnamed(value)));
            }
            if indexed.is_empty() {
                return Some(Vec::new());
            }
            indexed.sort_unstable_by_key(|(i, _)| *i);
            Some(indexed.into_iter().map(|(_, v)| v).collect())
        }
        _ => None,
    }
}

fn post_param_value_from_runtime(value: &RuntimeValue) -> Option<CustomPostParamValue> {
    match value {
        RuntimeValue::Bool(v) => Some(CustomPostParamValue::Bool(*v)),
        RuntimeValue::I32(v) => Some(CustomPostParamValue::I32(*v)),
        RuntimeValue::F32(v) => Some(CustomPostParamValue::F32(*v)),
        RuntimeValue::Vec2 { x, y } => Some(CustomPostParamValue::Vec2([*x, *y])),
        RuntimeValue::Vec3 { x, y, z } => Some(CustomPostParamValue::Vec3([*x, *y, *z])),
        RuntimeValue::Vec4 { x, y, z, w } => Some(CustomPostParamValue::Vec4([*x, *y, *z, *w])),
        _ => None,
    }
}

fn post_param_value_from_static(value: &StaticSceneValue) -> Option<CustomPostParamValue> {
    match value {
        StaticSceneValue::Bool(v) => Some(CustomPostParamValue::Bool(*v)),
        StaticSceneValue::I32(v) => Some(CustomPostParamValue::I32(*v)),
        StaticSceneValue::F32(v) => Some(CustomPostParamValue::F32(*v)),
        StaticSceneValue::Vec2 { x, y } => Some(CustomPostParamValue::Vec2([*x, *y])),
        StaticSceneValue::Vec3 { x, y, z } => Some(CustomPostParamValue::Vec3([*x, *y, *z])),
        StaticSceneValue::Vec4 { x, y, z, w } => Some(CustomPostParamValue::Vec4([*x, *y, *z, *w])),
        _ => None,
    }
}

fn as_particle_sim_mode_static(value: &StaticSceneValue) -> Option<ParticleEmitterSimMode3D> {
    let raw = as_str_static(value)?.trim().to_ascii_lowercase();
    match raw.as_str() {
        "default" => Some(ParticleEmitterSimMode3D::Default),
        "cpu" => Some(ParticleEmitterSimMode3D::Cpu),
        "hybrid" => Some(ParticleEmitterSimMode3D::GpuVertex),
        "gpu" => Some(ParticleEmitterSimMode3D::GpuCompute),
        _ => None,
    }
}

fn as_particle_render_mode_static(value: &StaticSceneValue) -> Option<ParticleType> {
    let raw = as_str_static(value)?.trim().to_ascii_lowercase();
    match raw.as_str() {
        "point" => Some(ParticleType::Point),
        "billboard" => Some(ParticleType::Billboard),
        _ => None,
    }
}

fn as_particle_params_static(value: &StaticSceneValue) -> Option<Vec<f32>> {
    match value {
        StaticSceneValue::Vec2 { x, y } => Some(vec![*x, *y]),
        StaticSceneValue::Vec3 { x, y, z } => Some(vec![*x, *y, *z]),
        StaticSceneValue::Vec4 { x, y, z, w } => Some(vec![*x, *y, *z, *w]),
        StaticSceneValue::Object(entries) => {
            let mut indexed = Vec::<(usize, f32)>::new();
            for (k, v) in *entries {
                let idx = parse_param_key_index(k)?;
                let val = match v {
                    StaticSceneValue::F32(n) => *n,
                    StaticSceneValue::I32(n) => *n as f32,
                    _ => return None,
                };
                indexed.push((idx, val));
            }
            if indexed.is_empty() {
                return Some(Vec::new());
            }
            indexed.sort_unstable_by_key(|(i, _)| *i);
            let max = indexed.last().map(|(i, _)| *i).unwrap_or(0);
            let mut out = vec![0.0; max + 1];
            for (i, v) in indexed {
                out[i] = v;
            }
            Some(out)
        }
        _ => None,
    }
}

fn parse_param_key_index(key: &str) -> Option<usize> {
    let key = key.trim();
    if let Ok(i) = key.parse::<usize>() {
        return Some(i);
    }
    if let Some(rest) = key.strip_prefix('p')
        && let Ok(i) = rest.parse::<usize>()
    {
        return Some(i);
    }
    None
}

fn apply_zoom_compat_projection(node: &mut Camera3D, zoom: f32) {
    let zoom = if zoom.is_finite() && zoom > 0.0 {
        zoom
    } else {
        1.0
    };
    let fov_y_degrees = (60.0 / zoom).clamp(10.0, 120.0);
    if let CameraProjection::Perspective {
        fov_y_degrees: fov, ..
    } = &mut node.projection
    {
        *fov = fov_y_degrees;
    }
}

fn set_projection_mode(node: &mut Camera3D, mode: &str) {
    match mode {
        "perspective" => {
            node.projection = CameraProjection::Perspective {
                fov_y_degrees: 60.0,
                near: 0.1,
                far: 1000.0,
            };
        }
        "orthographic" => {
            node.projection = CameraProjection::Orthographic {
                size: 10.0,
                near: 0.1,
                far: 1000.0,
            };
        }
        "frustum" => {
            node.projection = CameraProjection::Frustum {
                left: -1.0,
                right: 1.0,
                bottom: -1.0,
                top: 1.0,
                near: 0.1,
                far: 1000.0,
            };
        }
        _ => {}
    }
}

fn set_projection_fov(node: &mut Camera3D, value: f32) {
    let fov = value.clamp(10.0, 120.0);
    if let CameraProjection::Perspective { fov_y_degrees, .. } = &mut node.projection {
        *fov_y_degrees = fov;
    }
}

fn set_projection_perspective_near(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Perspective { near, far, .. } = &mut node.projection {
        *near = value.max(0.001);
        if *far <= *near {
            *far = *near + 0.001;
        }
    }
}

fn set_projection_perspective_far(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Perspective { near, far, .. } = &mut node.projection {
        *far = value.max(*near + 0.001);
    }
}

fn set_projection_ortho_size(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Orthographic { size, .. } = &mut node.projection {
        *size = value.abs().max(0.001);
    }
}

fn set_projection_ortho_near(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Orthographic { near, far, .. } = &mut node.projection {
        *near = value.max(0.001);
        if *far <= *near {
            *far = *near + 0.001;
        }
    }
}

fn set_projection_ortho_far(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Orthographic { near, far, .. } = &mut node.projection {
        *far = value.max(*near + 0.001);
    }
}

fn set_projection_frustum_left(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Frustum { left, right, .. } = &mut node.projection {
        *left = value;
        if *right <= *left {
            *right = *left + 0.001;
        }
    }
}

fn set_projection_frustum_right(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Frustum { left, right, .. } = &mut node.projection {
        *right = value.max(*left + 0.001);
    }
}

fn set_projection_frustum_bottom(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Frustum { bottom, top, .. } = &mut node.projection {
        *bottom = value;
        if *top <= *bottom {
            *top = *bottom + 0.001;
        }
    }
}

fn set_projection_frustum_top(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Frustum { bottom, top, .. } = &mut node.projection {
        *top = value.max(*bottom + 0.001);
    }
}

fn set_projection_frustum_near(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Frustum { near, far, .. } = &mut node.projection {
        *near = value.max(0.001);
        if *far <= *near {
            *far = *near + 0.001;
        }
    }
}

fn set_projection_frustum_far(node: &mut Camera3D, value: f32) {
    if let CameraProjection::Frustum { near, far, .. } = &mut node.projection {
        *far = value.max(*near + 0.001);
    }
}

fn apply_collision_shape_2d_fields(
    node: &mut CollisionShape2D,
    fields: &[(String, RuntimeValue)],
) {
    for (name, value) in fields {
        match name.as_str() {
            "shape" => {
                if let Some(shape) = as_shape_2d(value) {
                    node.shape = shape;
                }
            }
            "sensor" => {
                if let Some(sensor) = as_bool(value) {
                    node.sensor = sensor;
                }
            }
            "friction" => {
                if let Some(friction) = as_f32(value) {
                    node.friction = friction;
                }
            }
            "restitution" => {
                if let Some(restitution) = as_f32(value) {
                    node.restitution = restitution;
                }
            }
            "density" => {
                if let Some(density) = as_f32(value) {
                    node.density = density;
                }
            }
            _ => {}
        }
    }
}

fn apply_static_body_2d_fields(node: &mut StaticBody2D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        if name == "enabled" && let Some(enabled) = as_bool(value) {
            node.enabled = enabled;
        }
    }
}

fn apply_rigid_body_2d_fields(node: &mut RigidBody2D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        match name.as_str() {
            "enabled" => {
                if let Some(enabled) = as_bool(value) {
                    node.enabled = enabled;
                }
            }
            "linear_velocity" | "velocity" => {
                if let Some(velocity) = as_vec2(value) {
                    node.linear_velocity = velocity;
                }
            }
            "angular_velocity" => {
                if let Some(angular_velocity) = as_f32(value) {
                    node.angular_velocity = angular_velocity;
                }
            }
            "gravity_scale" => {
                if let Some(gravity_scale) = as_f32(value) {
                    node.gravity_scale = gravity_scale;
                }
            }
            "linear_damping" => {
                if let Some(linear_damping) = as_f32(value) {
                    node.linear_damping = linear_damping;
                }
            }
            "angular_damping" => {
                if let Some(angular_damping) = as_f32(value) {
                    node.angular_damping = angular_damping;
                }
            }
            "can_sleep" => {
                if let Some(can_sleep) = as_bool(value) {
                    node.can_sleep = can_sleep;
                }
            }
            "lock_rotation" => {
                if let Some(lock_rotation) = as_bool(value) {
                    node.lock_rotation = lock_rotation;
                }
            }
            _ => {}
        }
    }
}

fn apply_collision_shape_3d_fields(
    node: &mut CollisionShape3D,
    fields: &[(String, RuntimeValue)],
) {
    for (name, value) in fields {
        match name.as_str() {
            "shape" => {
                if let Some(shape) = as_shape_3d(value) {
                    node.shape = shape;
                }
            }
            "sensor" => {
                if let Some(sensor) = as_bool(value) {
                    node.sensor = sensor;
                }
            }
            "friction" => {
                if let Some(friction) = as_f32(value) {
                    node.friction = friction;
                }
            }
            "restitution" => {
                if let Some(restitution) = as_f32(value) {
                    node.restitution = restitution;
                }
            }
            "density" => {
                if let Some(density) = as_f32(value) {
                    node.density = density;
                }
            }
            _ => {}
        }
    }
}

fn apply_static_body_3d_fields(node: &mut StaticBody3D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        if name == "enabled" && let Some(enabled) = as_bool(value) {
            node.enabled = enabled;
        }
    }
}

fn apply_rigid_body_3d_fields(node: &mut RigidBody3D, fields: &[(String, RuntimeValue)]) {
    for (name, value) in fields {
        match name.as_str() {
            "enabled" => {
                if let Some(enabled) = as_bool(value) {
                    node.enabled = enabled;
                }
            }
            "linear_velocity" | "velocity" => {
                if let Some(velocity) = as_vec3(value) {
                    node.linear_velocity = velocity;
                }
            }
            "angular_velocity" => {
                if let Some(angular_velocity) = as_vec3(value) {
                    node.angular_velocity = angular_velocity;
                }
            }
            "gravity_scale" => {
                if let Some(gravity_scale) = as_f32(value) {
                    node.gravity_scale = gravity_scale;
                }
            }
            "linear_damping" => {
                if let Some(linear_damping) = as_f32(value) {
                    node.linear_damping = linear_damping;
                }
            }
            "angular_damping" => {
                if let Some(angular_damping) = as_f32(value) {
                    node.angular_damping = angular_damping;
                }
            }
            "can_sleep" => {
                if let Some(can_sleep) = as_bool(value) {
                    node.can_sleep = can_sleep;
                }
            }
            _ => {}
        }
    }
}

fn apply_collision_shape_2d_fields_static(
    node: &mut CollisionShape2D,
    fields: &[(&str, StaticSceneValue)],
) {
    for (name, value) in fields {
        match *name {
            "shape" => {
                if let Some(shape) = as_shape_2d_static(value) {
                    node.shape = shape;
                }
            }
            "sensor" => {
                if let Some(sensor) = as_bool_static(value) {
                    node.sensor = sensor;
                }
            }
            "friction" => {
                if let Some(friction) = as_f32_static(value) {
                    node.friction = friction;
                }
            }
            "restitution" => {
                if let Some(restitution) = as_f32_static(value) {
                    node.restitution = restitution;
                }
            }
            "density" => {
                if let Some(density) = as_f32_static(value) {
                    node.density = density;
                }
            }
            _ => {}
        }
    }
}

fn apply_static_body_2d_fields_static(node: &mut StaticBody2D, fields: &[(&str, StaticSceneValue)]) {
    for (name, value) in fields {
        if *name == "enabled" && let Some(enabled) = as_bool_static(value) {
            node.enabled = enabled;
        }
    }
}

fn apply_rigid_body_2d_fields_static(node: &mut RigidBody2D, fields: &[(&str, StaticSceneValue)]) {
    for (name, value) in fields {
        match *name {
            "enabled" => {
                if let Some(enabled) = as_bool_static(value) {
                    node.enabled = enabled;
                }
            }
            "linear_velocity" | "velocity" => {
                if let Some(velocity) = as_vec2_static(value) {
                    node.linear_velocity = velocity;
                }
            }
            "angular_velocity" => {
                if let Some(angular_velocity) = as_f32_static(value) {
                    node.angular_velocity = angular_velocity;
                }
            }
            "gravity_scale" => {
                if let Some(gravity_scale) = as_f32_static(value) {
                    node.gravity_scale = gravity_scale;
                }
            }
            "linear_damping" => {
                if let Some(linear_damping) = as_f32_static(value) {
                    node.linear_damping = linear_damping;
                }
            }
            "angular_damping" => {
                if let Some(angular_damping) = as_f32_static(value) {
                    node.angular_damping = angular_damping;
                }
            }
            "can_sleep" => {
                if let Some(can_sleep) = as_bool_static(value) {
                    node.can_sleep = can_sleep;
                }
            }
            "lock_rotation" => {
                if let Some(lock_rotation) = as_bool_static(value) {
                    node.lock_rotation = lock_rotation;
                }
            }
            _ => {}
        }
    }
}

fn apply_collision_shape_3d_fields_static(
    node: &mut CollisionShape3D,
    fields: &[(&str, StaticSceneValue)],
) {
    for (name, value) in fields {
        match *name {
            "shape" => {
                if let Some(shape) = as_shape_3d_static(value) {
                    node.shape = shape;
                }
            }
            "sensor" => {
                if let Some(sensor) = as_bool_static(value) {
                    node.sensor = sensor;
                }
            }
            "friction" => {
                if let Some(friction) = as_f32_static(value) {
                    node.friction = friction;
                }
            }
            "restitution" => {
                if let Some(restitution) = as_f32_static(value) {
                    node.restitution = restitution;
                }
            }
            "density" => {
                if let Some(density) = as_f32_static(value) {
                    node.density = density;
                }
            }
            _ => {}
        }
    }
}

fn apply_static_body_3d_fields_static(node: &mut StaticBody3D, fields: &[(&str, StaticSceneValue)]) {
    for (name, value) in fields {
        if *name == "enabled" && let Some(enabled) = as_bool_static(value) {
            node.enabled = enabled;
        }
    }
}

fn apply_rigid_body_3d_fields_static(node: &mut RigidBody3D, fields: &[(&str, StaticSceneValue)]) {
    for (name, value) in fields {
        match *name {
            "enabled" => {
                if let Some(enabled) = as_bool_static(value) {
                    node.enabled = enabled;
                }
            }
            "linear_velocity" | "velocity" => {
                if let Some(velocity) = as_vec3_static(value) {
                    node.linear_velocity = velocity;
                }
            }
            "angular_velocity" => {
                if let Some(angular_velocity) = as_vec3_static(value) {
                    node.angular_velocity = angular_velocity;
                }
            }
            "gravity_scale" => {
                if let Some(gravity_scale) = as_f32_static(value) {
                    node.gravity_scale = gravity_scale;
                }
            }
            "linear_damping" => {
                if let Some(linear_damping) = as_f32_static(value) {
                    node.linear_damping = linear_damping;
                }
            }
            "angular_damping" => {
                if let Some(angular_damping) = as_f32_static(value) {
                    node.angular_damping = angular_damping;
                }
            }
            "can_sleep" => {
                if let Some(can_sleep) = as_bool_static(value) {
                    node.can_sleep = can_sleep;
                }
            }
            _ => {}
        }
    }
}

fn as_shape_2d(value: &RuntimeValue) -> Option<Shape2D> {
    let RuntimeValue::Object(entries) = value else {
        return None;
    };
    let ty = entries.iter().find_map(|(k, v)| match k.as_str() {
        "type" | "kind" => as_str(v).map(|s| s.to_ascii_lowercase()),
        _ => None,
    })?;
    let width = entries
        .iter()
        .find_map(|(k, v)| (k == "width").then(|| as_f32(v)).flatten())
        .unwrap_or(1.0);
    let height = entries
        .iter()
        .find_map(|(k, v)| (k == "height").then(|| as_f32(v)).flatten())
        .unwrap_or(width);
    let radius = entries
        .iter()
        .find_map(|(k, v)| (k == "radius").then(|| as_f32(v)).flatten())
        .unwrap_or(0.5);

    match ty.as_str() {
        "quad" | "rect" | "rectangle" => Some(Shape2D::Quad { width, height }),
        "circle" => Some(Shape2D::Circle { radius }),
        "tri" | "triangle" => {
            let tri_kind = entries
                .iter()
                .find_map(|(k, v)| (k == "triangle").then(|| as_str(v)).flatten())
                .or_else(|| {
                    entries
                        .iter()
                        .find_map(|(k, v)| (k == "variant").then(|| as_str(v)).flatten())
                })
                .map(|raw| match raw.to_ascii_lowercase().as_str() {
                    "right" => Triangle2DKind::Right,
                    "isosceles" => Triangle2DKind::Isosceles,
                    _ => Triangle2DKind::Equilateral,
                })
                .unwrap_or(Triangle2DKind::Equilateral);
            Some(Shape2D::Triangle {
                kind: tri_kind,
                width,
                height,
            })
        }
        _ => None,
    }
}

fn as_shape_2d_static(value: &StaticSceneValue) -> Option<Shape2D> {
    let StaticSceneValue::Object(entries) = value else {
        return None;
    };
    let ty = entries.iter().find_map(|(k, v)| match *k {
        "type" | "kind" => as_str_static(v).map(|s| s.to_ascii_lowercase()),
        _ => None,
    })?;
    let width = entries
        .iter()
        .find_map(|(k, v)| (*k == "width").then(|| as_f32_static(v)).flatten())
        .unwrap_or(1.0);
    let height = entries
        .iter()
        .find_map(|(k, v)| (*k == "height").then(|| as_f32_static(v)).flatten())
        .unwrap_or(width);
    let radius = entries
        .iter()
        .find_map(|(k, v)| (*k == "radius").then(|| as_f32_static(v)).flatten())
        .unwrap_or(0.5);

    match ty.as_str() {
        "quad" | "rect" | "rectangle" => Some(Shape2D::Quad { width, height }),
        "circle" => Some(Shape2D::Circle { radius }),
        "tri" | "triangle" => {
            let tri_kind = entries
                .iter()
                .find_map(|(k, v)| (*k == "triangle").then(|| as_str_static(v)).flatten())
                .or_else(|| {
                    entries
                        .iter()
                        .find_map(|(k, v)| (*k == "variant").then(|| as_str_static(v)).flatten())
                })
                .map(|raw| match raw.to_ascii_lowercase().as_str() {
                    "right" => Triangle2DKind::Right,
                    "isosceles" => Triangle2DKind::Isosceles,
                    _ => Triangle2DKind::Equilateral,
                })
                .unwrap_or(Triangle2DKind::Equilateral);
            Some(Shape2D::Triangle {
                kind: tri_kind,
                width,
                height,
            })
        }
        _ => None,
    }
}

fn as_shape_3d(value: &RuntimeValue) -> Option<Shape3D> {
    let RuntimeValue::Object(entries) = value else {
        return None;
    };
    let ty = entries.iter().find_map(|(k, v)| match k.as_str() {
        "type" | "kind" => as_str(v).map(|s| s.to_ascii_lowercase()),
        _ => None,
    })?;

    let size = entries
        .iter()
        .find_map(|(k, v)| (k == "size").then(|| as_vec3(v)).flatten())
        .unwrap_or(Vector3::ONE);
    let radius = entries
        .iter()
        .find_map(|(k, v)| (k == "radius").then(|| as_f32(v)).flatten())
        .unwrap_or(0.5);
    let half_height = entries
        .iter()
        .find_map(|(k, v)| (k == "half_height").then(|| as_f32(v)).flatten())
        .or_else(|| {
            entries
                .iter()
                .find_map(|(k, v)| (k == "height").then(|| as_f32(v).map(|h| h * 0.5)).flatten())
        })
        .unwrap_or(0.5);

    match ty.as_str() {
        "cube" => Some(Shape3D::Cube { size }),
        "sphere" => Some(Shape3D::Sphere { radius }),
        "capsule" => Some(Shape3D::Capsule {
            radius,
            half_height,
        }),
        "cylinder" => Some(Shape3D::Cylinder {
            radius,
            half_height,
        }),
        "cone" => Some(Shape3D::Cone {
            radius,
            half_height,
        }),
        "tri_prism" | "triprism" => Some(Shape3D::TriPrism { size }),
        "triangular_pyramid" | "tri_pyr" => Some(Shape3D::TriangularPyramid { size }),
        "square_pyramid" | "sq_pyr" => Some(Shape3D::SquarePyramid { size }),
        _ => None,
    }
}

fn as_shape_3d_static(value: &StaticSceneValue) -> Option<Shape3D> {
    let StaticSceneValue::Object(entries) = value else {
        return None;
    };
    let ty = entries.iter().find_map(|(k, v)| match *k {
        "type" | "kind" => as_str_static(v).map(|s| s.to_ascii_lowercase()),
        _ => None,
    })?;

    let size = entries
        .iter()
        .find_map(|(k, v)| (*k == "size").then(|| as_vec3_static(v)).flatten())
        .unwrap_or(Vector3::ONE);
    let radius = entries
        .iter()
        .find_map(|(k, v)| (*k == "radius").then(|| as_f32_static(v)).flatten())
        .unwrap_or(0.5);
    let half_height = entries
        .iter()
        .find_map(|(k, v)| (*k == "half_height").then(|| as_f32_static(v)).flatten())
        .or_else(|| {
            entries.iter().find_map(|(k, v)| {
                (*k == "height")
                    .then(|| as_f32_static(v).map(|h| h * 0.5))
                    .flatten()
            })
        })
        .unwrap_or(0.5);

    match ty.as_str() {
        "cube" => Some(Shape3D::Cube { size }),
        "sphere" => Some(Shape3D::Sphere { radius }),
        "capsule" => Some(Shape3D::Capsule {
            radius,
            half_height,
        }),
        "cylinder" => Some(Shape3D::Cylinder {
            radius,
            half_height,
        }),
        "cone" => Some(Shape3D::Cone {
            radius,
            half_height,
        }),
        "tri_prism" | "triprism" => Some(Shape3D::TriPrism { size }),
        "triangular_pyramid" | "tri_pyr" => Some(Shape3D::TriangularPyramid { size }),
        "square_pyramid" | "sq_pyr" => Some(Shape3D::SquarePyramid { size }),
        _ => None,
    }
}

fn as_asset_source_static(value: &StaticSceneValue) -> Option<String> {
    match value {
        StaticSceneValue::Str(v) => Some((*v).to_string()),
        StaticSceneValue::Key(v) => Some(v.0.to_string()),
        _ => None,
    }
}

fn extract_texture_source_static(data: &StaticNodeData) -> Option<String> {
    if data.ty != StaticNodeType::Sprite2D {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (*name == "texture")
            .then(|| as_asset_source_static(value))
            .flatten()
    })
}

fn extract_mesh_source_static(data: &StaticNodeData) -> Option<String> {
    if data.ty != StaticNodeType::MeshInstance3D {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (*name == "mesh")
            .then(|| as_asset_source_static(value))
            .flatten()
    })
}

fn extract_material_source_static(data: &StaticNodeData) -> Option<String> {
    if data.ty != StaticNodeType::MeshInstance3D {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (*name == "material")
            .then(|| as_asset_source_static(value))
            .flatten()
    })
}

fn extract_material_inline_static(data: &StaticNodeData) -> Option<Material3D> {
    if data.ty != StaticNodeType::MeshInstance3D {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        if *name != "material" {
            return None;
        }
        match value {
            StaticSceneValue::Object(entries) => material_schema::from_static_object(entries),
            _ => None,
        }
    })
}

fn extract_model_source_static(data: &StaticNodeData) -> Option<String> {
    if data.ty != StaticNodeType::MeshInstance3D {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (*name == "model")
            .then(|| as_asset_source_static(value))
            .flatten()
    })
}

fn extract_skeleton_source_static(data: &StaticNodeData) -> Option<String> {
    if data.ty != StaticNodeType::Skeleton3D {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (*name == "skeleton")
            .then(|| as_asset_source_static(value))
            .flatten()
    })
}

fn extract_mesh_skeleton_target_static(data: &StaticNodeData) -> Option<String> {
    if data.ty != StaticNodeType::MeshInstance3D {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (*name == "skeleton")
            .then(|| as_asset_source_static(value))
            .flatten()
    })
}
