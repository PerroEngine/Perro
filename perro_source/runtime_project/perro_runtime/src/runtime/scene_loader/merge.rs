use super::{PendingScriptAttach, prepare::PreparedScene};
use crate::Runtime;
use perro_ids::{NodeID, ScriptMemberID};
use perro_nodes::animation_player::AnimationObjectBinding;
use perro_nodes::{SceneNode, SceneNodeData};
use perro_resource_context::ResourceContext;
use perro_scene::SceneValue;
use perro_structs::{Vector2, Vector3};
use perro_variant::Variant;
use std::sync::Arc;
use std::{borrow::Cow, collections::BTreeMap, collections::HashMap};

pub(super) struct MergePreparedSceneResult {
    pub(super) scene_root: NodeID,
    pub(super) script_nodes: Vec<PendingScriptAttach>,
}

pub(super) fn merge_prepared_scene(
    runtime: &mut Runtime,
    prepared: PreparedScene,
) -> Result<MergePreparedSceneResult, String> {
    let PreparedScene {
        root_key,
        nodes,
        scripts,
    } = prepared;

    let mut engine_root = SceneNode::new(SceneNodeData::Node);
    engine_root.name = Cow::Borrowed("Game Root");
    let engine_root = runtime.nodes.insert(engine_root);
    if let Some(node) = runtime.nodes.get(engine_root) {
        runtime.register_internal_node_schedules(engine_root, node.node_type());
    }

    let mut key_to: HashMap<String, NodeID> = HashMap::with_capacity(nodes.len());
    let mut key_order: Vec<String> = Vec::with_capacity(nodes.len());
    let mut parent_pairs = Vec::with_capacity(nodes.len());
    let mut animation_player_bindings: Vec<(NodeID, Vec<(String, String)>)> = Vec::new();
    let mut mesh_skeleton_links: Vec<(NodeID, String)> = Vec::new();
    let resource_api = runtime.resource_api.clone();

    for pending in nodes {
        let super::prepare::PendingNode {
            key,
            parent_key,
            node,
            animation_source,
            texture_source,
            mesh_source,
            material_surfaces,
            skeleton_source,
            mesh_skeleton_target,
            animation_bindings,
        } = pending;

        if key_to.contains_key(&key) {
            return Err(format!("duplicate scene key `{}`", key));
        }

        let node = runtime.nodes.insert(node);
        if let Some(inserted) = runtime.nodes.get(node) {
            let ty = inserted.node_type();
            runtime.register_internal_node_schedules(node, ty);
        }
        if !animation_bindings.is_empty() {
            animation_player_bindings.push((node, animation_bindings));
        }
        if let Some(source) = animation_source {
            let res = ResourceContext::new(resource_api.as_ref());
            let animation = res.Animations().load(&source);
            if let Some(node_data) = runtime.nodes.get_mut(node)
                && let SceneNodeData::AnimationPlayer(player) = &mut node_data.data
            {
                player.set_animation(animation);
            }
        }
        if let Some(source) = texture_source {
            runtime.render_2d.texture_sources.insert(node, source);
        }
        if let Some(source) = mesh_source {
            runtime.render_3d.mesh_sources.insert(node, source);
        }
        if !material_surfaces.is_empty() {
            let mut sources = Vec::with_capacity(material_surfaces.len());
            let mut overrides = Vec::with_capacity(material_surfaces.len());
            for surface in material_surfaces {
                sources.push(surface.source);
                overrides.push(surface.inline);
            }
            runtime
                .render_3d
                .material_surface_sources
                .insert(node, sources);
            runtime
                .render_3d
                .material_surface_overrides
                .insert(node, overrides);
        }
        if let Some(source) = skeleton_source {
            let res = ResourceContext::new(resource_api.as_ref());
            let bones = res.Skeletons().load_bones(&source);
            if let Some(node_data) = runtime.nodes.get_mut(node)
                && let SceneNodeData::Skeleton3D(skeleton) = &mut node_data.data
            {
                skeleton.bones = bones;
            }
        }
        if let Some(target) = mesh_skeleton_target {
            mesh_skeleton_links.push((node, target));
        }
        let key_for_map = key.clone();
        if let Some(parent_key) = parent_key {
            parent_pairs.push((key.clone(), parent_key));
        }
        key_order.push(key);
        key_to.insert(key_for_map, node);
    }

    if let Some(root_key) = root_key.as_deref()
        && !key_to.contains_key(root_key)
    {
        return Err(format!("scene root `{root_key}` not found in node list"));
    }

    let mut edges = Vec::with_capacity(parent_pairs.len());
    for (child_key, parent_key) in parent_pairs {
        let child = *key_to
            .get(&child_key)
            .ok_or_else(|| format!("child node key `{child_key}` not found"))?;
        let parent = *key_to.get(&parent_key).ok_or_else(|| {
            format!("parent node key `{parent_key}` not found while linking child `{child_key}`")
        })?;

        if let Some(child) = runtime.nodes.get_mut(child) {
            child.parent = parent;
        }
        edges.push((parent, child));
    }

    for (parent, child) in edges {
        if let Some(parent) = runtime.nodes.get_mut(parent) {
            parent.add_child(child);
        }
    }

    for (mesh_node, target_key) in mesh_skeleton_links {
        let target = *key_to
            .get(&target_key)
            .ok_or_else(|| format!("mesh skeleton target `{target_key}` not found"))?;
        if let Some(node_data) = runtime.nodes.get_mut(mesh_node) {
            match &mut node_data.data {
                SceneNodeData::MeshInstance3D(mesh) => mesh.skeleton = target,
                _ => {}
            }
        }
    }

    for (player_id, scene_bindings) in animation_player_bindings {
        let Some(node_data) = runtime.nodes.get_mut(player_id) else {
            continue;
        };
        let SceneNodeData::AnimationPlayer(player) = &mut node_data.data else {
            continue;
        };
        let mut resolved = Vec::with_capacity(scene_bindings.len());
        for (object, node_key) in scene_bindings {
            let Some(target_id) = key_to.get(node_key.as_str()).copied() else {
                continue;
            };
            resolved.push(AnimationObjectBinding {
                object: object.into(),
                node: target_id,
            });
        }
        player.bindings = Cow::Owned(resolved);
    }

    let mut top_level_roots: Vec<NodeID> = Vec::new();
    for key in &key_order {
        let Some(&id) = key_to.get(key) else {
            continue;
        };
        let Some(node) = runtime.nodes.get(id) else {
            continue;
        };
        if node.parent.is_nil() {
            top_level_roots.push(id);
        }
    }

    if top_level_roots.is_empty() {
        return Err("boot scene produced no top-level root nodes".to_string());
    }

    let primary_root = if let Some(root_key) = root_key.as_deref() {
        *key_to
            .get(root_key)
            .ok_or_else(|| format!("scene root `{root_key}` not found in node list"))?
    } else {
        top_level_roots[0]
    };

    let mut attach_order = Vec::with_capacity(top_level_roots.len());
    attach_order.push(primary_root);
    for id in top_level_roots {
        if id != primary_root {
            attach_order.push(id);
        }
    }

    for root in &attach_order {
        if let Some(root_node) = runtime.nodes.get_mut(*root) {
            root_node.parent = engine_root;
        }
        if let Some(engine_root_node) = runtime.nodes.get_mut(engine_root) {
            engine_root_node.add_child(*root);
        }
    }

    // Force initial frame extraction for freshly merged scene content, even with no scripts.
    runtime.mark_transform_dirty_recursive(engine_root);
    runtime.mark_needs_rerender(engine_root);
    for node_id in key_to.values().copied() {
        runtime.mark_needs_rerender(node_id);
    }

    let mut script_nodes = Vec::with_capacity(scripts.len());
    for pending_script in scripts {
        let id = *key_to.get(&pending_script.node_key).ok_or_else(|| {
            format!(
                "script node key `{}` not found in node list",
                pending_script.node_key
            )
        })?;
        let scene_injected_vars = pending_script
            .scene_injected_vars
            .iter()
            .map(|(name, value)| {
                Ok((
                    ScriptMemberID::from_string(name.as_str()),
                    scene_value_to_variant(value, &key_to),
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        script_nodes.push(PendingScriptAttach {
            node_id: id,
            script_path_hash: pending_script.script_path_hash,
            scene_injected_vars,
        });
    }

    Ok(MergePreparedSceneResult {
        scene_root: primary_root,
        script_nodes,
    })
}

fn scene_value_to_variant(value: &SceneValue, key_to: &HashMap<String, NodeID>) -> Variant {
    match value {
        SceneValue::Bool(v) => Variant::from(*v),
        SceneValue::I32(v) => Variant::from(*v),
        SceneValue::F32(v) => Variant::from(*v),
        SceneValue::Vec2 { x, y } => Variant::from(Vector2::new(*x, *y)),
        SceneValue::Vec3 { x, y, z } => Variant::from(Vector3::new(*x, *y, *z)),
        SceneValue::Vec4 { x, y, z, w } => Variant::Array(vec![
            Variant::from(*x),
            Variant::from(*y),
            Variant::from(*z),
            Variant::from(*w),
        ]),
        SceneValue::Str(v) => Variant::from(v.to_string()),
        SceneValue::Hashed(v) => Variant::from(*v),
        SceneValue::Key(v) => {
            if let Some(id) = key_to.get(v.as_ref()) {
                Variant::from(*id)
            } else {
                Variant::from(v.to_string())
            }
        }
        SceneValue::Object(entries) => {
            let mut out = BTreeMap::new();
            for (k, v) in entries.iter() {
                out.insert(
                    Arc::<str>::from(k.as_ref()),
                    scene_value_to_variant(v, key_to),
                );
            }
            Variant::Object(out)
        }
        SceneValue::Array(items) => Variant::Array(
            items
                .iter()
                .map(|v| scene_value_to_variant(v, key_to))
                .collect(),
        ),
    }
}
