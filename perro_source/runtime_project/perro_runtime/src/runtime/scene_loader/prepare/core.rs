use crate::material_schema;
use perro_ids::{IntoTagID, string_to_u64};
use perro_io::load_asset;
use perro_nodes::{
    ambient_light_3d::AmbientLight3D,
    animation_player::AnimationPlayer,
    camera_2d::Camera2D,
    camera_3d::{Camera3D, CameraProjection},
    mesh_instance_3d::{MaterialParamOverride, MaterialParamOverrideValue, MeshInstance3D, MeshSurfaceBinding},
    multi_mesh_instance_3d::MultiMeshInstance3D,
    node_2d::Node2D,
    node_3d::Node3D,
    particle_emitter_3d::ParticleEmitter3D,
    particle_emitter_3d::{ParticleEmitterSimMode3D, ParticleType},
    point_light_3d::PointLight3D,
    ray_light_3d::RayLight3D,
    skeleton_3d::Skeleton3D,
    sky_3d::{Sky3D, SkyStyle},
    spot_light_3d::SpotLight3D,
    sprite_2d::Sprite2D,
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
    SceneValue, Skeleton3DField, Sky3DField, SpotLight3DField, Sprite2DField, StaticBody2DField,
    StaticBody3DField, resolve_node_field,
};
use perro_structs::{
    CustomPostParam, CustomPostParamValue, PostProcessEffect, PostProcessSet, Quaternion, Vector2,
    Vector3,
};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
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
    pub(super) script_path_hash: u64,
    pub(super) scene_injected_vars: Vec<(String, SceneValue)>,
}

pub(super) struct PendingNode {
    pub(super) key: String,
    pub(super) parent_key: Option<String>,
    pub(super) node: SceneNode,
    pub(super) animation_source: Option<String>,
    pub(super) texture_source: Option<String>,
    pub(super) mesh_source: Option<String>,
    pub(super) material_surfaces: Vec<PendingSurfaceMaterial>,
    pub(super) skeleton_source: Option<String>,
    pub(super) mesh_skeleton_target: Option<String>,
    pub(super) animation_bindings: Vec<(String, String)>,
}

pub(super) struct PendingSurfaceMaterial {
    pub(super) source: Option<String>,
    pub(super) inline: Option<Material3D>,
}

type SceneNodeExtraction = (
    SceneNode,
    Option<String>,
    Option<String>,
    Option<String>,
    Vec<PendingSurfaceMaterial>,
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

pub(super) fn prepare_scene_with_loader(
    scene: &Scene,
    load_scene: &dyn Fn(&str) -> Result<Scene, String>,
) -> Result<PreparedScene, String> {
    let mut include_stack = HashSet::new();
    prepare_scene_with_stack(scene, &mut include_stack, load_scene)
}

fn prepare_scene_with_stack(
    scene: &Scene,
    include_stack: &mut HashSet<String>,
    load_scene: &dyn Fn(&str) -> Result<Scene, String>,
) -> Result<PreparedScene, String> {
    let mut prepared_nodes = Vec::with_capacity(scene.nodes.len());
    let mut scripts = Vec::new();

    for entry in scene.nodes.as_ref() {
        push_entry_prepared(
            entry,
            None,
            &HashMap::new(),
            &mut prepared_nodes,
            &mut scripts,
            include_stack,
            load_scene,
        )?;
    }

    Ok(PreparedScene {
        root_key: scene.root.as_ref().map(|k| k.as_ref().to_string()),
        nodes: prepared_nodes,
        scripts,
    })
}

fn push_entry_prepared(
    entry: &SceneDefNodeEntry,
    key_override: Option<&str>,
    key_map: &HashMap<String, String>,
    prepared_nodes: &mut Vec<PendingNode>,
    scripts: &mut Vec<PendingScript>,
    include_stack: &mut HashSet<String>,
    load_scene: &dyn Fn(&str) -> Result<Scene, String>,
) -> Result<(), String> {
    let key = key_override
        .map(|v| v.to_string())
        .unwrap_or_else(|| remap_key(entry.key.as_ref(), key_map));
    let parent_key = entry
        .parent
        .as_ref()
        .map(|p| remap_key(p.as_ref(), key_map));
    let mut merged_root_entry = None;

    if let Some(root_of_path) = &entry.root_of {
        if include_stack.contains(root_of_path.as_ref()) {
            return Err(format!(
                "root_of cycle detected while loading `{}` for host `{}`",
                root_of_path.as_ref(),
                key
            ));
        }
        include_stack.insert(root_of_path.to_string());
        let root_merge_result = (|| {
            let import_scene = load_scene(root_of_path.as_ref())?;
            let import_root = import_scene
                .root
                .as_ref()
                .map(|v| v.as_ref().to_string())
                .ok_or_else(|| format!("root_of scene `{}` has no @root", root_of_path.as_ref()))?;
            let import_root_node = import_scene
                .nodes
                .iter()
                .find(|node| node.key.as_ref() == import_root)
                .ok_or_else(|| {
                    format!(
                        "root_of scene `{}` root key `{import_root}` was not found in node list",
                        root_of_path.as_ref()
                    )
                })?;
            let merged = merge_root_host_entry(entry, import_root_node);
            expand_import_children_into_host(
                key.as_str(),
                root_of_path.as_ref(),
                &import_scene,
                &import_root,
                prepared_nodes,
                scripts,
                include_stack,
                load_scene,
            )?;
            Ok::<SceneDefNodeEntry, String>(merged)
        })();
        include_stack.remove(root_of_path.as_ref());
        merged_root_entry = Some(root_merge_result?);
    }

    let entry = merged_root_entry.as_ref().unwrap_or(entry);

    let (
        node,
        animation_source,
        texture_source,
        mesh_source,
        material_surfaces,
        skeleton_source,
        mesh_skeleton_target,
        animation_bindings,
    ) = scene_node_from_entry(entry)?;

    prepared_nodes.push(PendingNode {
        key: key.clone(),
        parent_key,
        node,
        animation_source,
        texture_source,
        mesh_source,
        material_surfaces,
        skeleton_source,
        mesh_skeleton_target: mesh_skeleton_target.map(|v| remap_key(v.as_str(), key_map)),
        animation_bindings: animation_bindings
            .into_iter()
            .map(|(object, target)| (object, remap_key(target.as_str(), key_map)))
            .collect(),
    });

    let script_path_hash = entry
        .script_hash
        .or_else(|| entry.script.as_ref().map(|script| string_to_u64(script.as_ref())));
    if let Some(script_path_hash) = script_path_hash {
        scripts.push(PendingScript {
            node_key: key.clone(),
            script_path_hash,
            scene_injected_vars: entry
                .script_vars
                .iter()
                .map(|(k, v)| (k.to_string(), remap_scene_value_keys(v, key_map)))
                .collect(),
        });
    }

    Ok(())
}

fn expand_import_children_into_host(
    host_key: &str,
    path: &str,
    import_scene: &Scene,
    import_root: &str,
    prepared_nodes: &mut Vec<PendingNode>,
    scripts: &mut Vec<PendingScript>,
    include_stack: &mut HashSet<String>,
    load_scene: &dyn Fn(&str) -> Result<Scene, String>,
) -> Result<(), String> {
    let mut map = HashMap::<String, String>::new();
    map.insert(import_root.to_string(), host_key.to_string());
    for node in import_scene.nodes.as_ref() {
        let source_key = node.key.as_ref().to_string();
        if source_key == import_root {
            continue;
        }
        map.insert(source_key.clone(), format!("{host_key}::{source_key}"));
    }

    for node in import_scene.nodes.as_ref() {
        if node.key.as_ref() == import_root {
            continue;
        }
        let remapped_key = map
            .get(node.key.as_ref())
            .cloned()
            .ok_or_else(|| format!("missing remap key for `{}` in root_of `{path}`", node.key.as_ref()))?;
        push_entry_prepared(
            node,
            Some(remapped_key.as_str()),
            &map,
            prepared_nodes,
            scripts,
            include_stack,
            load_scene,
        )?;
    }
    Ok(())
}

fn merge_root_host_entry(host: &SceneDefNodeEntry, base_root: &SceneDefNodeEntry) -> SceneDefNodeEntry {
    let mut merged = host.clone();
    merged.name = host.name.clone().or_else(|| base_root.name.clone());
    if host.tags.is_empty() {
        merged.tags = base_root.tags.clone();
    }
    if host.children.is_empty() {
        merged.children = base_root.children.clone();
    }
    merged.parent = host.parent.clone().or_else(|| base_root.parent.clone());
    if host.clear_script {
        merged.script = None;
        merged.script_hash = None;
    } else if host.script.is_some() || host.script_hash.is_some() {
        merged.script = host.script.clone();
        merged.script_hash = host
            .script_hash
            .or_else(|| host.script.as_ref().map(|path| string_to_u64(path.as_ref())));
    } else {
        merged.script = base_root.script.clone();
        merged.script_hash = base_root
            .script_hash
            .or_else(|| base_root.script.as_ref().map(|path| string_to_u64(path.as_ref())));
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
    if base.ty != local.ty {
        return local.clone();
    }

    let base_fields = flatten_scene_node_fields(base);
    let local_fields = flatten_scene_node_fields(local);
    let merged_fields = merge_scene_object_fields(&base_fields, &local_fields);
    SceneDefNodeData {
        ty: local.ty.clone(),
        fields: merged_fields,
        base: None,
    }
}

fn flatten_scene_node_fields(data: &SceneDefNodeData) -> Vec<SceneObjectField> {
    let mut out = Vec::new();
    if let Some(base) = data.base_ref() {
        out.extend(flatten_scene_node_fields(base));
    }
    out.extend(data.fields.iter().cloned());
    out
}

fn merge_scene_object_fields(
    base: &[SceneObjectField],
    local: &[SceneObjectField],
) -> Cow<'static, [SceneObjectField]> {
    let mut merged: BTreeMap<String, SceneValue> = BTreeMap::new();
    for (name, value) in base {
        merged.insert(name.to_string(), value.clone());
    }
    for (name, value) in local {
        if is_unset_marker(value) {
            merged.remove(name.as_ref());
            continue;
        }

        let key = name.to_string();
        let next_value = if let Some(prev) = merged.get(&key) {
            merge_scene_values(prev, value)
        } else {
            value.clone()
        };
        merged.insert(key, next_value);
    }

    Cow::Owned(
        merged
            .into_iter()
            .map(|(name, value)| (Cow::Owned(name), value))
            .collect(),
    )
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

fn remap_key(key: &str, key_map: &HashMap<String, String>) -> String {
    key_map
        .get(key)
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

fn remap_scene_value_keys(value: &SceneValue, key_map: &HashMap<String, String>) -> SceneValue {
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
        SceneValue::Key(v) => {
            let next = key_map
                .get(v.as_ref())
                .cloned()
                .unwrap_or_else(|| v.as_ref().to_string());
            SceneValue::Key(next.into())
        }
        SceneValue::Object(fields) => SceneValue::Object(Cow::Owned(
            fields
                .iter()
                .map(|(k, v)| (k.clone(), remap_scene_value_keys(v, key_map)))
                .collect(),
        )),
        SceneValue::Array(items) => SceneValue::Array(Cow::Owned(
            items
                .iter()
                .map(|v| remap_scene_value_keys(v, key_map))
                .collect(),
        )),
    }
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
    let material_surfaces_explicit = extract_material_surfaces(&entry.data);
    let skeleton_source = extract_skeleton_source(&entry.data);
    let mesh_skeleton_target = extract_mesh_skeleton_target(&entry.data);
    let animation_bindings = extract_animation_scene_bindings(&entry.data);
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
        texture_source,
        mesh_source,
        material_surfaces,
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
        "MultiMeshInstance3D" => Ok(SceneNodeData::MultiMeshInstance3D(
            build_multi_mesh_instance_3d(data),
        )),
        "CollisionShape3D" => Ok(SceneNodeData::CollisionShape3D(build_collision_shape_3d(
            data,
        ))),
        "StaticBody3D" => Ok(SceneNodeData::StaticBody3D(build_static_body_3d(data))),
        "Area3D" => Ok(SceneNodeData::Area3D(build_area_3d(data))),
        "RigidBody3D" => Ok(SceneNodeData::RigidBody3D(build_rigid_body_3d(data))),
        "Skeleton3D" => Ok(SceneNodeData::Skeleton3D(build_skeleton_3d(data))),
        "Camera3D" => Ok(SceneNodeData::Camera3D(build_camera_3d(data))),
        "ParticleEmitter3D" => Ok(SceneNodeData::ParticleEmitter3D(build_particle_emitter_3d(
            data,
        ))),
        "AnimationPlayer" => Ok(SceneNodeData::AnimationPlayer(build_animation_player(data))),
        "AmbientLight3D" => Ok(SceneNodeData::AmbientLight3D(build_ambient_light_3d(data))),
        "Sky3D" => Ok(SceneNodeData::Sky3D(build_sky_3d(data))),
        "RayLight3D" => Ok(SceneNodeData::RayLight3D(build_ray_light_3d(data))),
        "PointLight3D" => Ok(SceneNodeData::PointLight3D(build_point_light_3d(data))),
        "SpotLight3D" => Ok(SceneNodeData::SpotLight3D(build_spot_light_3d(data))),
        other => Err(format!("unsupported scene node type `{other}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_nodes::SceneNodeData;
    use perro_scene::Parser;

    #[test]
    fn root_of_merges_root_defaults_overrides_and_children() {
        let host = Parser::new(
            r#"
            @root = host
            [host]
            root_of = "res://base.scn"
            script_vars = {
                keep: 5,
                remove_me: __unset__,
                nested: { b: 20, c: 30 },
                added: true
            }
            [Node2D]
                rotation = 3.0
            [/Node2D]
            [/host]

            [local_child]
            parent = host
            [Node]
            [/Node]
            [/local_child]
            "#,
        )
        .parse_scene();

        let base = Parser::new(
            r#"
            @root = base_root
            [base_root]
            script = "res://base_script.rs"
            script_vars = {
                keep: 1,
                remove_me: 2,
                nested: { a: 10, b: 11 },
                old_only: 9
            }
            [Node2D]
                position = (1, 2)
                rotation = 1.0
            [/Node2D]
            [/base_root]

            [base_child]
            parent = base_root
            [Node]
            [/Node]
            [/base_child]
            "#,
        )
        .parse_scene();

        let prepared = prepare_scene_with_loader(&host, &|path| match path {
            "res://base.scn" => Ok(base.clone()),
            _ => Err(format!("unknown scene path `{path}`")),
        })
        .expect("prepare scene");

        let host_script = prepared
            .scripts
            .iter()
            .find(|pending| pending.node_key == "host")
            .expect("host script");
        assert_eq!(
            host_script.script_path_hash,
            string_to_u64("res://base_script.rs")
        );

        let mut vars = BTreeMap::new();
        for (name, value) in &host_script.scene_injected_vars {
            vars.insert(name.as_str(), value);
        }
        assert!(vars.contains_key("keep"));
        assert!(vars.contains_key("added"));
        assert!(vars.contains_key("nested"));
        assert!(vars.contains_key("old_only"));
        assert!(!vars.contains_key("remove_me"));

        match vars.get("nested").expect("nested var") {
            SceneValue::Object(fields) => {
                assert!(fields.iter().any(|(k, _)| k.as_ref() == "a"));
                assert!(fields.iter().any(|(k, _)| k.as_ref() == "b"));
                assert!(fields.iter().any(|(k, _)| k.as_ref() == "c"));
            }
            other => panic!("expected nested object, got {other:?}"),
        }

        let host_node = prepared
            .nodes
            .iter()
            .find(|pending| pending.key == "host")
            .expect("host node");
        match &host_node.node.data {
            SceneNodeData::Node2D(node_2d) => {
                assert_eq!(node_2d.position.x, 1.0);
                assert_eq!(node_2d.position.y, 2.0);
                assert_eq!(node_2d.rotation, 3.0);
            }
            other => panic!("expected Node2D host node, got {other:?}"),
        }

        assert!(prepared.nodes.iter().any(|pending| pending.key == "host::base_child"));
        assert!(prepared.nodes.iter().any(|pending| pending.key == "local_child"));
    }

    #[test]
    fn root_of_script_clear_prevents_inherited_script() {
        let host = Parser::new(
            r#"
            @root = host
            [host]
            root_of = "res://base.scn"
            script = null
            [Node]
            [/Node]
            [/host]
            "#,
        )
        .parse_scene();

        let base = Parser::new(
            r#"
            @root = base_root
            [base_root]
            script = "res://base_script.rs"
            [Node]
            [/Node]
            [/base_root]
            "#,
        )
        .parse_scene();

        let prepared = prepare_scene_with_loader(&host, &|path| match path {
            "res://base.scn" => Ok(base.clone()),
            _ => Err(format!("unknown scene path `{path}`")),
        })
        .expect("prepare scene");

        assert!(!prepared.scripts.iter().any(|pending| pending.node_key == "host"));
    }

    #[test]
    fn root_of_without_host_type_block_inherits_template_root_data() {
        let host = Parser::new(
            r#"
            @root = host
            [host]
            root_of = "res://base.scn"
            [/host]
            "#,
        )
        .parse_scene();

        let base = Parser::new(
            r#"
            @root = base_root
            [base_root]
            [Node2D]
                position = (7, 8)
                rotation = 1.25
            [/Node2D]
            [/base_root]
            "#,
        )
        .parse_scene();

        let prepared = prepare_scene_with_loader(&host, &|path| match path {
            "res://base.scn" => Ok(base.clone()),
            _ => Err(format!("unknown scene path `{path}`")),
        })
        .expect("prepare scene");

        let host_node = prepared
            .nodes
            .iter()
            .find(|pending| pending.key == "host")
            .expect("host node");
        match &host_node.node.data {
            SceneNodeData::Node2D(node_2d) => {
                assert_eq!(node_2d.position.x, 7.0);
                assert_eq!(node_2d.position.y, 8.0);
                assert_eq!(node_2d.rotation, 1.25);
            }
            other => panic!("expected inherited Node2D host node, got {other:?}"),
        }
    }
}
