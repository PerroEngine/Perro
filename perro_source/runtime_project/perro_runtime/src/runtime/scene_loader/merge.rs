use super::{PendingScriptAttach, prepare::PreparedScene};
use crate::Runtime;
use ahash::AHashMap;
use perro_ids::{NodeID, ScriptMemberID};
use perro_nodes::animation_player::AnimationObjectBinding;
use perro_nodes::animation_tree::AnimationTreeAnimation;
use perro_nodes::{SceneNode, SceneNodeData};
use perro_resource_api::ResourceWindow;
use perro_scene::SceneValue;
use perro_structs::{IVector2, IVector3, IVector4, UVector2, UVector3, UVector4, Vector2, Vector3};
use perro_variant::Variant;
use std::sync::Arc;
use std::{borrow::Cow, collections::BTreeMap};

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
    // Read the type from the owned node before it moves into the arena,
    // avoiding a post-insert arena lookup. The arena indexes tags on insert.
    let engine_root_type = engine_root.node_type();
    let engine_root = runtime.nodes.insert(engine_root);
    runtime.register_internal_node_schedules(engine_root, engine_root_type);

    // Scene keys are sparse `u32` (author-assigned keys plus generated ones from
    // root_of expansion / default light injection), so a `Vec` index is not safe.
    // ahash keeps the u32 lookups fast without siphash overhead.
    let mut key_to: AHashMap<u32, NodeID> = AHashMap::with_capacity(nodes.len());
    let mut key_name_to = if scripts.is_empty() {
        None
    } else {
        Some(AHashMap::with_capacity(nodes.len()))
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
    let mut camera_stream_links: Vec<(NodeID, u32)> = Vec::new();
    let mut joint_body_links: Vec<(NodeID, super::prepare::PendingJointBodyField, u32)> =
        Vec::new();
    let resource_api = runtime.resource_api.clone();
    // One resource window reused across the whole merge loop. It only holds a
    // shared borrow of `resource_api`, compatible with the direct
    // `resource_api.*` shared calls below.
    let res = ResourceWindow::new(resource_api.as_ref());

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
            decal_texture_sources,
            mesh_source,
            material_surfaces,
            skeleton_source,
            mesh_skeleton_target,
            bone_attachment_skeleton_target,
            ik_target_skeleton_target,
            physics_bone_chain_skeleton_target,
            camera_stream_target,
            joint_body_links: pending_joint_body_links,
            animation_bindings,
            locale_text_bindings,
        } = pending;

        if key_to.contains_key(&key) {
            return Err(format!("duplicate scene key `{}`", key));
        }

        // Compute type and the camera-active flag from the owned node before
        // it moves into the arena, eliminating post-insert lookups. The arena
        // indexes tags on insert.
        let node_type = node.node_type();
        let camera_3d_active =
            matches!(&node.data, SceneNodeData::Camera3D(camera) if camera.active);
        let node = runtime.nodes.insert(node);
        runtime.register_internal_node_schedules(node, node_type);
        if camera_3d_active {
            runtime.note_camera_3d_activated(node);
        }
        for binding in locale_text_bindings {
            runtime.add_locale_text_binding(node, binding.field, binding.key, binding.key_hash);
        }
        if !animation_bindings.is_empty() {
            animation_player_bindings.push((node, animation_bindings));
        }
        if let Some(source) = animation_source {
            let animation = res.Animations().load(&source);
            if let Some(node_data) = runtime.nodes.get_mut(node)
                && let SceneNodeData::AnimationPlayer(player) = &mut node_data.data
            {
                player.set_animation(animation);
            }
        }
        if let Some(source) = animation_tree_source {
            let tree = res.AnimationTrees().load(&source);
            if let Some(node_data) = runtime.nodes.get_mut(node)
                && let SceneNodeData::AnimationTree(anim_tree) = &mut node_data.data
            {
                anim_tree.set_tree(tree);
            }
        }
        if !animation_tree_animations.is_empty() {
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
        if decal_texture_sources.iter().any(Option::is_some)
            && let Some(node_data) = runtime.nodes.get_mut(node)
            && let SceneNodeData::Decal3D(decal) = &mut node_data.data
        {
            let [albedo, normal, emission] = decal_texture_sources;
            if let Some(source) = albedo {
                decal.albedo_texture = res.Textures().load(&source);
            }
            if let Some(source) = normal {
                decal.normal_texture = res.Textures().load(&source);
            }
            if let Some(source) = emission {
                decal.emission_texture = res.Textures().load(&source);
            }
        }
        if let Some(source) = mesh_source {
            let mesh = res.Meshes().load(&source);
            if let Some(node_data) = runtime.nodes.get_mut(node) {
                match &mut node_data.data {
                    SceneNodeData::MeshInstance3D(mesh_instance) => {
                        mesh_instance.mesh = mesh;
                    }
                    SceneNodeData::MultiMeshInstance3D(mesh_instance) => {
                        mesh_instance.mesh = mesh;
                    }
                    _ => {}
                }
            }
            runtime.render_3d.mesh_sources.insert(node, source);
        }
        if !material_surfaces.is_empty() {
            let mut sources = Vec::with_capacity(material_surfaces.len());
            let mut overrides = Vec::with_capacity(material_surfaces.len());
            for (surface_index, surface) in material_surfaces.into_iter().enumerate() {
                let material = surface
                    .source
                    .as_deref()
                    .map(|source| res.Materials().load(source))
                    .or_else(|| {
                        surface
                            .inline
                            .clone()
                            .map(|material| resource_api.shared_inline_material_id(material))
                    });
                if let Some(material) = material
                    && let Some(node_data) = runtime.nodes.get_mut(node)
                {
                    match &mut node_data.data {
                        SceneNodeData::MeshInstance3D(mesh_instance) => {
                            mesh_instance.set_surface_material(surface_index, Some(material));
                        }
                        SceneNodeData::MultiMeshInstance3D(mesh_instance) => {
                            mesh_instance.ensure_surface_mut(surface_index).material =
                                Some(material);
                        }
                        _ => {}
                    }
                }
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
        if let Some(source) = skeleton_source
            && let Some(node_data) = runtime.nodes.get_mut(node)
        {
            match &mut node_data.data {
                SceneNodeData::Skeleton2D(skeleton) => {
                    skeleton.bones = res.Skeletons().load_bones_2d(&source);
                    if resource_api.is_skeleton_2d_pending(&source) {
                        runtime
                            .pending_skeleton_sources_2d
                            .insert(node, source.clone());
                    }
                }
                SceneNodeData::Skeleton3D(skeleton) => {
                    skeleton.bones = res.Skeletons().load_bones_3d(&source);
                    skeleton.refresh_inv_bind_cache();
                    if resource_api.is_skeleton_3d_pending(&source) {
                        runtime
                            .pending_skeleton_sources_3d
                            .insert(node, source.clone());
                    }
                }
                _ => {}
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
        if let Some(target) = camera_stream_target {
            camera_stream_links.push((node, target));
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

        runtime.nodes.set_parent(child, parent);
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

    for (stream_node, target_key) in camera_stream_links {
        let target = *key_to
            .get(&target_key)
            .ok_or_else(|| format!("camera stream target `{target_key}` not found"))?;
        if let Some(node_data) = runtime.nodes.get_mut(stream_node) {
            match &mut node_data.data {
                SceneNodeData::CameraStream2D(stream) => stream.stream.camera = target,
                SceneNodeData::CameraStream3D(stream) => stream.stream.camera = target,
                SceneNodeData::UiCameraStream(stream) => stream.stream.camera = target,
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
        player.replace_bindings(resolved);
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
        runtime.nodes.set_parent(*root, engine_root);
        if let Some(engine_root_node) = runtime.nodes.get_mut(engine_root) {
            engine_root_node.add_child(*root);
        }
    }

    // Force initial frame extraction for freshly merged scene content, even with no scripts.
    runtime.mark_transform_dirty_recursive(engine_root);
    runtime.mark_needs_rerender(engine_root);
    for node_id in key_to.values().copied() {
        runtime.mark_needs_rerender(node_id);
        runtime.mark_ui_dirty(
            node_id,
            Runtime::UI_DIRTY_LAYOUT_SELF
                | Runtime::UI_DIRTY_LAYOUT_PARENT
                | Runtime::UI_DIRTY_COMMANDS,
        );
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
    key_to: &AHashMap<u32, NodeID>,
    key_name_to: Option<&AHashMap<String, NodeID>>,
) -> Variant {
    match value {
        SceneValue::Bool(v) => Variant::from(*v),
        SceneValue::I32(v) => Variant::from(*v),
        SceneValue::F32(v) => Variant::from(*v),
        SceneValue::Vec2 { x, y } => Variant::from(Vector2::new(*x, *y)),
        SceneValue::Vec3 { x, y, z } => Variant::from(Vector3::new(*x, *y, *z)),
        SceneValue::IVec2 { x, y } => Variant::from(IVector2::new(*x, *y)),
        SceneValue::IVec3 { x, y, z } => Variant::from(IVector3::new(*x, *y, *z)),
        SceneValue::IVec4 { x, y, z, w } => Variant::from(IVector4::new(*x, *y, *z, *w)),
        SceneValue::UVec2 { x, y } => Variant::from(UVector2::new(*x, *y)),
        SceneValue::UVec3 { x, y, z } => Variant::from(UVector3::new(*x, *y, *z)),
        SceneValue::UVec4 { x, y, z, w } => Variant::from(UVector4::new(*x, *y, *z, *w)),
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
