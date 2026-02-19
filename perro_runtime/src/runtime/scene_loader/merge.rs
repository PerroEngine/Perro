use super::prepare::PreparedScene;
use crate::Runtime;
use perro_core::{SceneNode, SceneNodeData};
use perro_ids::NodeID;
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
    engine_root.name = Cow::Borrowed("Root");
    let engine_root_id = runtime.nodes.insert(engine_root);

    let mut key_to_id: HashMap<String, NodeID> = HashMap::with_capacity(nodes.len());
    let mut key_order: Vec<String> = Vec::with_capacity(nodes.len());
    let mut parent_pairs = Vec::with_capacity(nodes.len());

    for pending in nodes {
        let super::prepare::PendingNode {
            key,
            parent_key,
            node,
            texture_source,
            mesh_source,
        } = pending;

        if key_to_id.contains_key(&key) {
            return Err(format!("duplicate scene key `{}`", key));
        }

        let node_id = runtime.nodes.insert(node);
        if let Some(source) = texture_source {
            runtime.render_2d.texture_sources.insert(node_id, source);
        }
        if let Some(source) = mesh_source {
            runtime.render_3d.mesh_sources.insert(node_id, source);
        }
        if let Some(parent_key) = parent_key {
            parent_pairs.push((key.clone(), parent_key));
        }
        key_order.push(key.clone());
        key_to_id.insert(key, node_id);
    }

    if let Some(root_key) = root_key.as_deref() {
        if !key_to_id.contains_key(root_key) {
            return Err(format!("scene root `{root_key}` not found in node list"));
        }
    }

    let mut edges = Vec::with_capacity(parent_pairs.len());
    for (child_key, parent_key) in parent_pairs {
        let child_id = *key_to_id
            .get(&child_key)
            .ok_or_else(|| format!("child node key `{child_key}` not found"))?;
        let parent_id = *key_to_id.get(&parent_key).ok_or_else(|| {
            format!(
                "parent node key `{parent_key}` not found while linking child `{child_key}`"
            )
        })?;

        if let Some(child) = runtime.nodes.get_mut(child_id) {
            child.parent = parent_id;
        }
        edges.push((parent_id, child_id));
    }

    for (parent_id, child_id) in edges {
        if let Some(parent) = runtime.nodes.get_mut(parent_id) {
            parent.add_child(child_id);
        }
    }

    let mut top_level_roots: Vec<NodeID> = Vec::new();
    for key in &key_order {
        let Some(&id) = key_to_id.get(key) else {
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
        *key_to_id
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

    for root_id in &attach_order {
        if let Some(root_node) = runtime.nodes.get_mut(*root_id) {
            root_node.parent = engine_root_id;
        }
        if let Some(engine_root_node) = runtime.nodes.get_mut(engine_root_id) {
            engine_root_node.add_child(*root_id);
        }
    }

    let mut script_nodes = Vec::with_capacity(scripts.len());
    for pending_script in scripts {
        let id = *key_to_id.get(&pending_script.node_key).ok_or_else(|| {
            format!(
                "script node key `{}` not found in node list",
                pending_script.node_key
            )
        })?;
        script_nodes.push((id, pending_script.script_path));
    }

    Ok(script_nodes)
}
