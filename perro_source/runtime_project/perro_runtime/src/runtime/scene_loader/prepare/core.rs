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
    ambient_light_3d::AmbientLight3D,
    animation_player::AnimationPlayer,
    animation_tree::AnimationTree,
    bone_attachment_3d::BoneAttachment3D,
    bone_collider_3d::BoneCollider3D,
    camera_2d::Camera2D,
    camera_3d::{Camera3D, CameraProjection},
    ik_target_3d::IKTarget3D,
    mesh_instance_3d::{MaterialParamOverride, MaterialParamOverrideValue, MeshInstance3D, MeshSurfaceBinding},
    multi_mesh_instance_3d::MultiMeshInstance3D,
    node_2d::Node2D,
    node_3d::Node3D,
    particle_emitter_2d::ParticleEmitter2D,
    particle_emitter_2d::ParticleEmitterSimMode2D,
    tilemap_2d::TileMap2D,
    particle_emitter_3d::ParticleEmitter3D,
    particle_emitter_3d::{ParticleEmitterSimMode3D, ParticleType},
    physics_bone_chain_3d::PhysicsBoneChain3D,
    point_light_3d::PointLight3D,
    ray_light_3d::RayLight3D,
    skeleton_2d::{
        BoneAttachment2D, BoneCollider2D, IKTarget2D, PhysicsBoneChain2D, Skeleton2D,
    },
    skeleton_3d::Skeleton3D,
    sky_3d::{Sky3D, SkyStyle},
    spot_light_3d::SpotLight3D,
    sprite_2d::{AnimatedSprite, AnimatedSprite2D, Sprite2D},
    AmbientLight2D, Area2D, Area3D, AudioMask2D, AudioMask3D, AudioPortal2D, AudioPortal3D,
    AudioZone2D, AudioZone3D, BallJoint3D, CollisionShape2D, CollisionShape3D, DistanceJoint2D,
    FixedJoint2D, FixedJoint3D, HingeJoint3D, PinJoint2D, PointLight2D, RayLight2D, RigidBody2D,
    RigidBody3D, SceneNode, SceneNodeData, Shape2D, Shape3D, SpotLight2D, StaticBody2D,
    StaticBody3D, Triangle2DKind,
};
use perro_render_bridge::Material3D;
use perro_scene::{
    AnimatedSprite2DField, AnimationPlayerField, AnimationTreeField, Area2DField, Area3DField, BoneAttachment2DField, BoneAttachment3DField,
    BoneCollider2DField, BoneCollider3DField, Camera2DField, Camera3DField, CollisionShape2DField, CollisionShape3DField, DistanceJoint2DField, HingeJoint3DField, IKTarget2DField, IKTarget3DField, Joint2DField, Joint3DField, Light3DField,
    Light2DField, MeshInstance3DField, Node2DField, Node3DField, NodeField, Parser,
    ParticleEmitter2DField, PointLight2DField, RayLight2DField, SpotLight2DField,
    ParticleEmitter3DField, TileMap2DField,
    PhysicsBoneChain2DField, PhysicsBoneChain3DField, PointLight3DField, RayLight3DField, RigidBody2DField, RigidBody3DField, Scene,
    SceneFieldIterRef, SceneKey, SceneNodeData as SceneDefNodeData,
    SceneNodeEntry as SceneDefNodeEntry, SceneObjectField, SceneValue, Skeleton3DField,
    Sky3DField, SpotLight3DField, Sprite2DField, StaticBody2DField, StaticBody3DField,
    UiAnimatedImageField, UiImageField, resolve_node_field,
};
use perro_structs::{
    Color, CustomPostParam, CustomPostParamValue, IKTargetSolver, PostProcessEffect,
    PostProcessSet, Quaternion, Vector2, Vector3,
};
use perro_ui::{
    UiAnimatedImage, UiAnimatedImageFrameSet, UiBox, UiButton, UiGrid, UiHLayout, UiImage,
    UiImageScaleMode, UiLabel, UiLayout, UiMouseFilter, UiPanel, UiScrollContainer, UiTextAlign,
    UiTextBlock, UiTextBox, UiTreeList, UiVLayout,
};
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
type AnimationTreeAnimationEntry = (String, AnimationSceneBindings, f32, bool, perro_nodes::AnimationPlaybackType);
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
    let mut scene = Parser::new(source).parse_scene();
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
    prepare_scene_with_stack(scene, &mut include_stack, load_scene, static_ui_style_lookup)
}

fn prepare_scene_with_stack(
    scene: &Scene,
    include_stack: &mut HashSet<String>,
    load_scene: &dyn Fn(&str) -> Result<Arc<Scene>, String>,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) -> Result<PreparedScene, String> {
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
    };

    for entry in scene.nodes.as_ref() {
        push_entry_prepared(scene, entry, None, &key_map, &mut ctx)?;
    }

    Ok(PreparedScene {
        root_key: scene.root.map(|key| key.as_u32()),
        nodes: prepared_nodes,
        scripts,
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
    ) = scene_node_from_entry(entry, ctx.static_ui_style_lookup)?;

    ctx.prepared_nodes.push(PendingNode {
        key,
        key_name: key_name.clone(),
        parent_key,
        node,
        animation_source,
        animation_tree_source,
        animation_tree_animations: animation_tree_animations
            .into_iter()
            .map(|(source, bindings, speed, paused, playback_type)| PendingAnimationTreeAnimation {
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
            })
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
            node_key_name: key_name.clone(),
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
        let remapped_key = map
            .get(&node.key)
            .copied()
            .ok_or_else(|| format!("missing remap key for `{}` in root_of `{path}`", import_scene.key_name_or_id(node.key)))?;
        push_entry_prepared(
            import_scene,
            node,
            Some(remapped_key),
            &map,
            ctx,
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
    let joint_body_targets = extract_joint_body_targets(&entry.data);
    let animation_bindings = extract_animation_scene_bindings(&entry.data);
    let locale_text_bindings = extract_locale_text_bindings(&entry.data);
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

fn extract_locale_text_bindings(data: &SceneDefNodeData) -> Vec<PendingLocaleTextBinding> {
    let mut out = Vec::new();
    let fields = flatten_scene_node_fields(data);
    match data.ty.as_ref() {
        "UiLabel" => {
            push_locale_text_binding(
                &mut out,
                &fields,
                "text",
                crate::runtime::state::LocaleTextField::LabelText,
            );
        }
        "UiTextBox" | "UiTextBlock" => {
            push_locale_text_binding(
                &mut out,
                &fields,
                "text",
                crate::runtime::state::LocaleTextField::TextEditText,
            );
            push_locale_text_binding(
                &mut out,
                &fields,
                "placeholder",
                crate::runtime::state::LocaleTextField::TextEditPlaceholder,
            );
            push_locale_text_binding(
                &mut out,
                &fields,
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

fn extract_joint_body_targets(data: &SceneDefNodeData) -> Vec<(PendingJointBodyField, String)> {
    let fields = flatten_scene_node_fields(data);
    let mut out = Vec::new();
    let Some((body_a_field, body_b_field)) = joint_body_fields_for(data.ty.as_ref()) else {
        return out;
    };
    for (name, value) in fields {
        let resolved = resolve_node_field(data.ty.as_ref(), name.as_ref());
        let field = if resolved == Some(body_a_field) {
            Some(PendingJointBodyField::BodyA)
        } else if resolved == Some(body_b_field) {
            Some(PendingJointBodyField::BodyB)
        } else {
            None
        };
        if let Some(field) = field
            && let Some(target) = as_str(&value)
        {
            out.push((field, target.to_string()));
        }
    }
    out
}

fn joint_body_fields_for(ty: &str) -> Option<(NodeField, NodeField)> {
    match ty {
        "PinJoint2D" => Some((
            NodeField::PinJoint2D(Joint2DField::BodyA),
            NodeField::PinJoint2D(Joint2DField::BodyB),
        )),
        "DistanceJoint2D" => Some((
            NodeField::DistanceJoint2D(DistanceJoint2DField::Common(Joint2DField::BodyA)),
            NodeField::DistanceJoint2D(DistanceJoint2DField::Common(Joint2DField::BodyB)),
        )),
        "FixedJoint2D" => Some((
            NodeField::FixedJoint2D(Joint2DField::BodyA),
            NodeField::FixedJoint2D(Joint2DField::BodyB),
        )),
        "BallJoint3D" => Some((
            NodeField::BallJoint3D(Joint3DField::BodyA),
            NodeField::BallJoint3D(Joint3DField::BodyB),
        )),
        "HingeJoint3D" => Some((
            NodeField::HingeJoint3D(HingeJoint3DField::Common(Joint3DField::BodyA)),
            NodeField::HingeJoint3D(HingeJoint3DField::Common(Joint3DField::BodyB)),
        )),
        "FixedJoint3D" => Some((
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
    match data.ty.as_ref() {
        "Node" => Ok(SceneNodeData::Node),
        "Node2D" => Ok(SceneNodeData::Node2D(build_node_2d(data))),
        "Sprite2D" => Ok(SceneNodeData::Sprite2D(build_sprite_2d(data))),
        "AnimatedSprite2D" => Ok(SceneNodeData::AnimatedSprite2D(build_animated_sprite_2d(data))),
        "ParticleEmitter2D" => Ok(SceneNodeData::ParticleEmitter2D(build_particle_emitter_2d(
            data,
        ))),
        "AmbientLight2D" => Ok(SceneNodeData::AmbientLight2D(build_ambient_light_2d(data))),
        "RayLight2D" => Ok(SceneNodeData::RayLight2D(build_ray_light_2d(data))),
        "PointLight2D" => Ok(SceneNodeData::PointLight2D(build_point_light_2d(data))),
        "SpotLight2D" => Ok(SceneNodeData::SpotLight2D(build_spot_light_2d(data))),
        "TileMap2D" => Ok(SceneNodeData::TileMap2D(build_tilemap_2d(data))),
        "Skeleton2D" => Ok(SceneNodeData::Skeleton2D(build_skeleton_2d(data))),
        "Bone2D" => Err("unsupported scene node type `Bone2D`; use Skeleton2D.bones from .pskel2d".to_string()),
        "BoneAttachment2D" => Ok(SceneNodeData::BoneAttachment2D(
            build_bone_attachment_2d(data),
        )),
        "IKTarget2D" => Ok(SceneNodeData::IKTarget2D(build_ik_target_2d(data))),
        "PhysicsBoneChain2D" => Ok(SceneNodeData::PhysicsBoneChain2D(
            build_physics_bone_chain_2d(data),
        )),
        "BoneCollider2D" => Ok(SceneNodeData::BoneCollider2D(build_bone_collider_2d(data))),
        "Camera2D" => Ok(SceneNodeData::Camera2D(build_camera_2d(data))),
        "CollisionShape2D" => Ok(SceneNodeData::CollisionShape2D(build_collision_shape_2d(
            data,
        ))),
        "StaticBody2D" => Ok(SceneNodeData::StaticBody2D(build_static_body_2d(data))),
        "Area2D" => Ok(SceneNodeData::Area2D(build_area_2d(data))),
        "RigidBody2D" => Ok(SceneNodeData::RigidBody2D(build_rigid_body_2d(data))),
        "PinJoint2D" => Ok(SceneNodeData::PinJoint2D(build_pin_joint_2d(data))),
        "DistanceJoint2D" => Ok(SceneNodeData::DistanceJoint2D(build_distance_joint_2d(data))),
        "FixedJoint2D" => Ok(SceneNodeData::FixedJoint2D(build_fixed_joint_2d(data))),
        "AudioMask2D" => Ok(SceneNodeData::AudioMask2D(build_audio_mask_2d(data))),
        "AudioZone2D" => Ok(SceneNodeData::AudioZone2D(build_audio_zone_2d(data))),
        "AudioPortal2D" => Ok(SceneNodeData::AudioPortal2D(build_audio_portal_2d(data))),
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
        "BallJoint3D" => Ok(SceneNodeData::BallJoint3D(build_ball_joint_3d(data))),
        "HingeJoint3D" => Ok(SceneNodeData::HingeJoint3D(build_hinge_joint_3d(data))),
        "FixedJoint3D" => Ok(SceneNodeData::FixedJoint3D(build_fixed_joint_3d(data))),
        "AudioMask3D" => Ok(SceneNodeData::AudioMask3D(build_audio_mask_3d(data))),
        "AudioZone3D" => Ok(SceneNodeData::AudioZone3D(build_audio_zone_3d(data))),
        "AudioPortal3D" => Ok(SceneNodeData::AudioPortal3D(build_audio_portal_3d(data))),
        "Skeleton3D" => Ok(SceneNodeData::Skeleton3D(build_skeleton_3d(data))),
        "BoneAttachment3D" => Ok(SceneNodeData::BoneAttachment3D(
            build_bone_attachment_3d(data),
        )),
        "IKTarget3D" => Ok(SceneNodeData::IKTarget3D(build_ik_target_3d(data))),
        "PhysicsBoneChain3D" => Ok(SceneNodeData::PhysicsBoneChain3D(
            build_physics_bone_chain_3d(data),
        )),
        "BoneCollider3D" => Ok(SceneNodeData::BoneCollider3D(build_bone_collider_3d(data))),
        "Camera3D" => Ok(SceneNodeData::Camera3D(build_camera_3d(data))),
        "ParticleEmitter3D" => Ok(SceneNodeData::ParticleEmitter3D(build_particle_emitter_3d(
            data,
        ))),
        "AnimationPlayer" => Ok(SceneNodeData::AnimationPlayer(build_animation_player(data))),
        "AnimationTree" => Ok(SceneNodeData::AnimationTree(build_animation_tree(data))),
        "AmbientLight3D" => Ok(SceneNodeData::AmbientLight3D(build_ambient_light_3d(data))),
        "Sky3D" => Ok(SceneNodeData::Sky3D(build_sky_3d(data))),
        "RayLight3D" => Ok(SceneNodeData::RayLight3D(build_ray_light_3d(data))),
        "PointLight3D" => Ok(SceneNodeData::PointLight3D(build_point_light_3d(data))),
        "SpotLight3D" => Ok(SceneNodeData::SpotLight3D(build_spot_light_3d(data))),
        "UiBox" => Ok(SceneNodeData::UiBox(build_ui_box(data))),
        "UiPanel" => Ok(SceneNodeData::UiPanel(build_ui_panel(data, static_ui_style_lookup))),
        "UiButton" => Ok(SceneNodeData::UiButton(build_ui_button(data, static_ui_style_lookup))),
        "UiImage" => Ok(SceneNodeData::UiImage(build_ui_image(data))),
        "UiAnimatedImage" => Ok(SceneNodeData::UiAnimatedImage(build_ui_animated_image(data))),
        "UiLabel" => Ok(SceneNodeData::UiLabel(build_ui_label(data))),
        "UiTextBox" => Ok(SceneNodeData::UiTextBox(build_ui_text_box(
            data,
            static_ui_style_lookup,
        ))),
        "UiTextBlock" => Ok(SceneNodeData::UiTextBlock(build_ui_text_block(
            data,
            static_ui_style_lookup,
        ))),
        "UiScrollContainer" | "UiScroll" => {
            Ok(SceneNodeData::UiScrollContainer(build_ui_scroll_container(data)))
        }
        "UiLayout" => Ok(SceneNodeData::UiLayout(build_ui_layout(data))),
        "UiHLayout" | "UiHBox" => Ok(SceneNodeData::UiHLayout(build_ui_hlayout(data))),
        "UiVLayout" | "UiVBox" => Ok(SceneNodeData::UiVLayout(build_ui_vlayout(data))),
        "UiGrid" => Ok(SceneNodeData::UiGrid(build_ui_grid(data))),
        "UiTreeList" => Ok(SceneNodeData::UiTreeList(build_ui_tree_list(data))),
        other => Err(format!("unsupported scene node type `{other}`")),
    }
}

