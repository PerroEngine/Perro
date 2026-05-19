use super::{PendingScriptAttach, prepare::PreparedScene};
use crate::Runtime;
use perro_ids::{NodeID, ScriptMemberID};
use perro_nodes::animation_player::AnimationObjectBinding;
use perro_nodes::animation_tree::AnimationTreeAnimation;
use perro_nodes::{SceneNode, SceneNodeData};
use perro_resource_api::ResourceWindow;
use perro_scene::SceneValue;
use perro_structs::{Vector2, Vector3};
use perro_variant::Variant;
use std::sync::Arc;
use std::{borrow::Cow, collections::BTreeMap, collections::HashMap};

pub(super) struct MergePreparedSceneResult {
    pub(super) scene_root: NodeID,
    pub(super) script_nodes: Vec<PendingScriptAttach>,
}

type AnimationPlayerSceneBindings = Vec<(String, u32)>;
type AnimationTreeSlotSceneBinding = (usize, String, u32);
type AnimationTreeSceneBindings = Vec<AnimationTreeSlotSceneBinding>;

fn set_joint_body(
    node: &mut SceneNode,
    field: super::prepare::PendingJointBodyField,
    target: NodeID,
) {
    match &mut node.data {
        SceneNodeData::PinJoint2D(joint) => {
            set_joint_body_2d(&mut joint.body_a, &mut joint.body_b, field, target)
        }
        SceneNodeData::DistanceJoint2D(joint) => {
            set_joint_body_2d(&mut joint.body_a, &mut joint.body_b, field, target)
        }
        SceneNodeData::FixedJoint2D(joint) => {
            set_joint_body_2d(&mut joint.body_a, &mut joint.body_b, field, target)
        }
        SceneNodeData::BallJoint3D(joint) => {
            set_joint_body_3d(&mut joint.body_a, &mut joint.body_b, field, target)
        }
        SceneNodeData::HingeJoint3D(joint) => {
            set_joint_body_3d(&mut joint.body_a, &mut joint.body_b, field, target)
        }
        SceneNodeData::FixedJoint3D(joint) => {
            set_joint_body_3d(&mut joint.body_a, &mut joint.body_b, field, target)
        }
        _ => {}
    }
}

fn set_joint_body_2d(
    body_a: &mut NodeID,
    body_b: &mut NodeID,
    field: super::prepare::PendingJointBodyField,
    target: NodeID,
) {
    match field {
        super::prepare::PendingJointBodyField::BodyA => *body_a = target,
        super::prepare::PendingJointBodyField::BodyB => *body_b = target,
    }
}

fn set_joint_body_3d(
    body_a: &mut NodeID,
    body_b: &mut NodeID,
    field: super::prepare::PendingJointBodyField,
    target: NodeID,
) {
    match field {
        super::prepare::PendingJointBodyField::BodyA => *body_a = target,
        super::prepare::PendingJointBodyField::BodyB => *body_b = target,
    }
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
    runtime.nodes.reserve(nodes.len().saturating_add(1));
    let engine_root = runtime.nodes.insert(engine_root);
    if let Some((ty, tags)) = runtime
        .nodes
        .get(engine_root)
        .map(|node| (node.node_type(), node.get_tag_ids()))
    {
        runtime.register_internal_node_schedules(engine_root, ty);
        for tag in tags {
            runtime
                .node_index
                .node_tag_index
                .entry(tag)
                .or_default()
                .insert(engine_root);
        }
    }

    let mut key_to: HashMap<u32, NodeID> = HashMap::with_capacity(nodes.len());
    let mut key_name_to = if scripts.is_empty() {
        None
    } else {
        Some(HashMap::with_capacity(nodes.len()))
    };
    let mut key_order: Vec<u32> = Vec::with_capacity(nodes.len());
    let mut parent_pairs = Vec::with_capacity(nodes.len());
    let mut animation_player_bindings: Vec<(NodeID, AnimationPlayerSceneBindings)> = Vec::new();
    let mut animation_tree_animation_bindings: Vec<(NodeID, AnimationTreeSceneBindings)> =
        Vec::new();
    let mut mesh_skeleton_links: Vec<(NodeID, u32)> = Vec::new();
    let mut bone_attachment_skeleton_links: Vec<(NodeID, u32)> = Vec::new();
    let mut ik_target_skeleton_links: Vec<(NodeID, u32)> = Vec::new();
    let mut physics_bone_chain_skeleton_links: Vec<(NodeID, u32)> = Vec::new();
    let mut joint_body_links: Vec<(NodeID, super::prepare::PendingJointBodyField, u32)> =
        Vec::new();
    let resource_api = runtime.resource_api.clone();

    for pending in nodes {
        let super::prepare::PendingNode {
            key,
            key_name,
            parent_key,
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
            joint_body_links: pending_joint_body_links,
            animation_bindings,
            locale_text_bindings,
        } = pending;

        if key_to.contains_key(&key) {
            return Err(format!("duplicate scene key `{}`", key));
        }

        let node = runtime.nodes.insert(node);
        if let Some((ty, tags)) = runtime
            .nodes
            .get(node)
            .map(|inserted| (inserted.node_type(), inserted.get_tag_ids()))
        {
            runtime.register_internal_node_schedules(node, ty);
            for tag in tags {
                runtime
                    .node_index
                    .node_tag_index
                    .entry(tag)
                    .or_default()
                    .insert(node);
            }
        }
        for binding in locale_text_bindings {
            runtime.add_locale_text_binding(node, binding.field, binding.key, binding.key_hash);
        }
        if !animation_bindings.is_empty() {
            animation_player_bindings.push((node, animation_bindings));
        }
        if let Some(source) = animation_source {
            let res = ResourceWindow::new(resource_api.as_ref());
            let animation = res.Animations().load(&source);
            if let Some(node_data) = runtime.nodes.get_mut(node)
                && let SceneNodeData::AnimationPlayer(player) = &mut node_data.data
            {
                player.set_animation(animation);
            }
        }
        if let Some(source) = animation_tree_source {
            let res = ResourceWindow::new(resource_api.as_ref());
            let tree = res.AnimationTrees().load(&source);
            if let Some(node_data) = runtime.nodes.get_mut(node)
                && let SceneNodeData::AnimationTree(anim_tree) = &mut node_data.data
            {
                anim_tree.set_tree(tree);
            }
        }
        if !animation_tree_animations.is_empty() {
            let res = ResourceWindow::new(resource_api.as_ref());
            let animations = animation_tree_animations
                .iter()
                .map(|entry| AnimationTreeAnimation {
                    animation: res.Animations().load(&entry.source),
                    bindings: Vec::new(),
                    speed: entry.speed,
                    paused: entry.paused,
                    playback_type: entry.playback_type,
                })
                .collect::<Vec<_>>();
            let pending_bindings = animation_tree_animations
                .iter()
                .enumerate()
                .flat_map(|(slot, entry)| {
                    entry
                        .bindings
                        .iter()
                        .map(move |(object, node_key)| (slot, object.clone(), *node_key))
                })
                .collect::<Vec<_>>();
            if !pending_bindings.is_empty() {
                animation_tree_animation_bindings.push((node, pending_bindings));
            }
            if let Some(node_data) = runtime.nodes.get_mut(node)
                && let SceneNodeData::AnimationTree(anim_tree) = &mut node_data.data
            {
                anim_tree.animations = animations;
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
            let res = ResourceWindow::new(resource_api.as_ref());
            if let Some(node_data) = runtime.nodes.get_mut(node) {
                match &mut node_data.data {
                    SceneNodeData::Skeleton2D(skeleton) => {
                        skeleton.bones = res.Skeletons().load_bones_2d(&source);
                    }
                    SceneNodeData::Skeleton3D(skeleton) => {
                        skeleton.bones = res.Skeletons().load_bones_3d(&source);
                    }
                    _ => {}
                }
            }
        }
        if let Some(target) = mesh_skeleton_target {
            mesh_skeleton_links.push((node, target));
        }
        if let Some(target) = bone_attachment_skeleton_target {
            bone_attachment_skeleton_links.push((node, target));
        }
        if let Some(target) = ik_target_skeleton_target {
            ik_target_skeleton_links.push((node, target));
        }
        if let Some(target) = physics_bone_chain_skeleton_target {
            physics_bone_chain_skeleton_links.push((node, target));
        }
        for link in pending_joint_body_links {
            joint_body_links.push((node, link.field, link.target_key));
        }
        if let Some(parent_key) = parent_key {
            parent_pairs.push((key, parent_key));
        }
        key_order.push(key);
        key_to.insert(key, node);
        if let Some(key_name_to) = key_name_to.as_mut() {
            key_name_to.insert(key_name, node);
        }
    }

    if let Some(root_key) = root_key
        && !key_to.contains_key(&root_key)
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
        if let Some(node_data) = runtime.nodes.get_mut(mesh_node)
            && let SceneNodeData::MeshInstance3D(mesh) = &mut node_data.data
        {
            mesh.skeleton = target;
        }
    }

    for (attachment_node, target_key) in bone_attachment_skeleton_links {
        let target = *key_to
            .get(&target_key)
            .ok_or_else(|| format!("bone attachment skeleton target `{target_key}` not found"))?;
        if let Some(node_data) = runtime.nodes.get_mut(attachment_node) {
            match &mut node_data.data {
                SceneNodeData::BoneAttachment2D(attachment) => attachment.skeleton = target,
                SceneNodeData::BoneAttachment3D(attachment) => attachment.skeleton = target,
                _ => {}
            }
        }
    }

    for (ik_target_node, target_key) in ik_target_skeleton_links {
        let target = *key_to
            .get(&target_key)
            .ok_or_else(|| format!("ik target skeleton target `{target_key}` not found"))?;
        if let Some(node_data) = runtime.nodes.get_mut(ik_target_node) {
            match &mut node_data.data {
                SceneNodeData::IKTarget2D(ik_target) => ik_target.params.skeleton = target,
                SceneNodeData::IKTarget3D(ik_target) => ik_target.params.skeleton = target,
                _ => {}
            }
        }
    }

    for (chain_node, target_key) in physics_bone_chain_skeleton_links {
        let target = *key_to.get(&target_key).ok_or_else(|| {
            format!("physics bone chain skeleton target `{target_key}` not found")
        })?;
        if let Some(node_data) = runtime.nodes.get_mut(chain_node) {
            match &mut node_data.data {
                SceneNodeData::PhysicsBoneChain2D(chain) => chain.skeleton = target,
                SceneNodeData::PhysicsBoneChain3D(chain) => chain.skeleton = target,
                _ => {}
            }
        }
    }

    for (joint_node, field, target_key) in joint_body_links {
        let target = *key_to
            .get(&target_key)
            .ok_or_else(|| format!("joint body target `{target_key}` not found"))?;
        if let Some(node_data) = runtime.nodes.get_mut(joint_node) {
            set_joint_body(node_data, field, target);
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
            let Some(target_id) = key_to.get(&node_key).copied() else {
                continue;
            };
            resolved.push(AnimationObjectBinding {
                object: object.into(),
                node: target_id,
            });
        }
        player.bindings = resolved;
    }

    for (tree_id, scene_bindings) in animation_tree_animation_bindings {
        let Some(node_data) = runtime.nodes.get_mut(tree_id) else {
            continue;
        };
        let SceneNodeData::AnimationTree(tree) = &mut node_data.data else {
            continue;
        };
        for (slot, object, node_key) in scene_bindings {
            let Some(target_id) = key_to.get(&node_key).copied() else {
                continue;
            };
            tree.set_slot_binding(slot, &object, target_id);
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

    let primary_root = if let Some(root_key) = root_key {
        *key_to
            .get(&root_key)
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
                    scene_value_to_variant(value, &key_to, key_name_to.as_ref()),
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        script_nodes.push(PendingScriptAttach {
            node_id: id,
            script_path_hash: pending_script.script_path_hash,
            script_mount: pending_script.script_mount.clone(),
            scene_injected_vars,
        });
    }

    Ok(MergePreparedSceneResult {
        scene_root: primary_root,
        script_nodes,
    })
}

fn scene_value_to_variant(
    value: &SceneValue,
    key_to: &HashMap<u32, NodeID>,
    key_name_to: Option<&HashMap<String, NodeID>>,
) -> Variant {
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
            if let Some(raw) = v.as_ref().strip_prefix('#')
                && let Ok(key) = raw.parse::<u32>()
                && let Some(id) = key_to.get(&key)
            {
                Variant::from(*id)
            } else if let Some(id) = key_name_to.and_then(|key_name_to| key_name_to.get(v.as_ref()))
            {
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
                    scene_value_to_variant(v, key_to, key_name_to),
                );
            }
            Variant::Object(out)
        }
        SceneValue::Array(items) => Variant::Array(
            items
                .iter()
                .map(|v| scene_value_to_variant(v, key_to, key_name_to))
                .collect(),
        ),
    }
}
