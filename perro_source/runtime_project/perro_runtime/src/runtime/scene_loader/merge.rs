use super::prepare::PreparedScene;
use crate::Runtime;
use perro_ids::NodeID;
use perro_nodes::{SceneNode, SceneNodeData};
use perro_resource_context::ResourceContext;
use std::{borrow::Cow, collections::HashMap};

pub(super) fn merge_prepared_scene(
    runtime: &mut Runtime,
    prepared: PreparedScene,
) -> Result<Vec<(NodeID, String)>, String> {
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
    let mut mesh_skeleton_links: Vec<(NodeID, String)> = Vec::new();
    let resource_api = runtime.resource_api.clone();

    for pending in nodes {
        let super::prepare::PendingNode {
            key,
            parent_key,
            node,
            texture_source,
            mesh_source,
            material_source,
            material_inline,
            skeleton_source,
            mesh_skeleton_target,
        } = pending;

        if key_to.contains_key(&key) {
            return Err(format!("duplicate scene key `{}`", key));
        }

        let node = runtime.nodes.insert(node);
        if let Some(inserted) = runtime.nodes.get(node) {
            runtime.register_internal_node_schedules(node, inserted.node_type());
        }
        let _ = runtime.ensure_terrain_instance_data(node);
        if let Some(source) = texture_source {
            runtime.render_2d.texture_sources.insert(node, source);
        }
        if let Some(source) = mesh_source {
            runtime.render_3d.mesh_sources.insert(node, source);
        }
        if let Some(source) = material_source {
            runtime.render_3d.material_sources.insert(node, source);
        }
        if let Some(material) = material_inline {
            runtime.render_3d.material_overrides.insert(node, material);
        }
        if let Some(source) = skeleton_source {
            let res = ResourceContext::new(resource_api.as_ref());
            let bones = res.Skeletons().load_bones(&source);
            if let Some(node_data) = runtime.nodes.get_mut(node) {
                if let SceneNodeData::Skeleton3D(skeleton) = &mut node_data.data {
                    skeleton.bones = bones;
                }
            }
        }
        if let Some(target) = mesh_skeleton_target {
            mesh_skeleton_links.push((node, target));
        }
        if let Some(parent_key) = parent_key {
            parent_pairs.push((key.clone(), parent_key));
        }
        key_order.push(key.clone());
        key_to.insert(key, node);
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
            if let SceneNodeData::MeshInstance3D(mesh) = &mut node_data.data {
                mesh.skeleton = target;
            }
        }
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

    let mut script_nodes = Vec::with_capacity(scripts.len());
    for pending_script in scripts {
        let id = *key_to.get(&pending_script.node_key).ok_or_else(|| {
            format!(
                "script node key `{}` not found in node list",
                pending_script.node_key
            )
        })?;
        script_nodes.push((id, pending_script.script_path));
    }

    Ok(script_nodes)
}
