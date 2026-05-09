use crate::prelude::*;
use perro_animation::{
    AnimationBoneSelector, AnimationClip, AnimationObjectKey, AnimationObjectTrack,
    AnimationTrackValue, AnimationTreeAsset, AnimationTreeGraphNode, AnimationTreeMask,
    AnimationTreeNodeKind,
};
use perro_nodes::AnimationTree;
use perro_nodes::animation_tree::AnimationTreeSlotPlayback;
use perro_scene::{Node3DField, NodeField};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

type SelfNodeType = AnimationTree;

#[derive(Clone)]
struct PoseTrack {
    node: NodeID,
    object: Cow<'static, str>,
    field: NodeField,
    bone_target: Option<perro_animation::AnimationBoneTarget>,
    value: AnimationTrackValue,
}

#[derive(Clone, Default)]
struct Pose {
    tracks: BTreeMap<String, PoseTrack>,
}

pub fn internal_update<RT, R, IP>(
    ctx: &mut RuntimeWindow<'_, RT>,
    res: &ResourceWindow<'_, R>,
    _ipt_w: &InputWindow<'_, IP>,
    id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
    let tree_id = with_node!(ctx, SelfNodeType, id, |tree| tree.tree);
    if tree_id.is_nil() {
        return;
    }
    let Some(asset) = res.AnimationTrees().get(tree_id) else {
        return;
    };
    sync_slots(ctx, id, &asset);
    step_slots(ctx, res, id);
    let pose = with_node!(ctx, SelfNodeType, id, |tree| eval_tree_pose(
        tree, res, &asset
    ));
    apply_pose(ctx, res, &pose);
    fire_slot_events(ctx, res, id);
}

pub fn internal_fixed_update<RT, R, IP>(
    _run: &mut RuntimeWindow<'_, RT>,
    _res_w: &ResourceWindow<'_, R>,
    _ipt_w: &InputWindow<'_, IP>,
    _id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
}

fn sync_slots<RT>(ctx: &mut RuntimeWindow<'_, RT>, id: NodeID, asset: &Arc<AnimationTreeAsset>)
where
    RT: RuntimeAPI + ?Sized,
{
    let _ = with_node_mut!(ctx, SelfNodeType, id, |tree| {
        let needs_rebuild = tree.internal.slots.len() != asset.slots.len()
            || tree
                .internal
                .slots
                .iter()
                .zip(asset.slots.iter())
                .any(|(a, b)| a.name.as_ref() != b.name.as_ref());
        if needs_rebuild {
            tree.internal.slots = asset
                .slots
                .iter()
                .map(|slot| AnimationTreeSlotPlayback {
                    name: Cow::Owned(slot.name.to_string()),
                    current_frame: 0,
                    playback_frame: 0.0,
                    boomerang_direction: 1.0,
                    paused: false,
                })
                .collect();
        }
    });
}

fn step_slots<RT, R>(ctx: &mut RuntimeWindow<'_, RT>, res: &ResourceWindow<'_, R>, id: NodeID)
where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
{
    let delta_seconds = delta_time!(ctx).max(0.0);
    let _ = with_node_mut!(ctx, SelfNodeType, id, |tree| {
        if tree.paused {
            return;
        }
        for idx in 0..tree.internal.slots.len() {
            let entry = tree.animations.get(idx).cloned().unwrap_or_default();
            let slot = &mut tree.internal.slots[idx];
            if slot.paused || entry.paused || entry.animation.is_nil() {
                continue;
            }
            let Some(clip) = res.Animations().get(entry.animation) else {
                continue;
            };
            let frame_count = clip.frame_count();
            if frame_count <= 1 {
                slot.current_frame = 0;
                slot.playback_frame = 0.0;
                slot.boomerang_direction = 1.0;
                continue;
            }
            slot.playback_frame = super::animation_player::advance_playback_frame(
                slot.playback_frame,
                delta_seconds * clip.fps.max(0.0) * tree.speed * entry.speed,
                frame_count,
                entry.playback_type,
                &mut slot.boomerang_direction,
            );
            slot.current_frame = super::animation_player::playback_frame_to_frame(
                slot.playback_frame,
                frame_count,
                entry.playback_type,
            );
        }
    });
}

fn eval_tree_pose<R>(
    tree: &AnimationTree,
    res: &ResourceWindow<'_, R>,
    asset: &AnimationTreeAsset,
) -> Pose
where
    R: ResourceAPI + ?Sized,
{
    let nodes = asset
        .nodes
        .iter()
        .map(|node| (node.key.as_ref(), node))
        .collect::<HashMap<_, _>>();
    let mut visiting = HashSet::new();
    eval_node(tree, res, &nodes, asset.output.as_ref(), &mut visiting).unwrap_or_default()
}

fn eval_node<R>(
    tree: &AnimationTree,
    res: &ResourceWindow<'_, R>,
    nodes: &HashMap<&str, &AnimationTreeGraphNode>,
    key: &str,
    visiting: &mut HashSet<String>,
) -> Option<Pose>
where
    R: ResourceAPI + ?Sized,
{
    if !visiting.insert(key.to_string()) {
        return None;
    }
    let Some(node) = nodes.get(key).copied() else {
        visiting.remove(key);
        return Some(eval_slot_pose(tree, res, key));
    };
    let pose = match &node.kind {
        AnimationTreeNodeKind::Blend {
            inputs,
            weights,
            mask,
        } => {
            let mut poses = Vec::new();
            let mut raw_weights = Vec::new();
            for (idx, input) in inputs.iter().enumerate() {
                if let Some(pose) = eval_node(tree, res, nodes, input.as_ref(), visiting) {
                    poses.push(pose);
                    raw_weights.push(runtime_weight(tree, key, input.as_ref(), weights, idx));
                }
            }
            blend_poses(&poses, &raw_weights, mask)
        }
        AnimationTreeNodeKind::Add {
            base,
            inputs,
            weights,
            mask,
        } => {
            let base_pose = eval_node(tree, res, nodes, base.as_ref(), visiting)?;
            let mut out = base_pose.clone();
            for (idx, input) in inputs.iter().enumerate() {
                if let Some(pose) = eval_node(tree, res, nodes, input.as_ref(), visiting) {
                    let weight = runtime_weight(tree, key, input.as_ref(), weights, idx);
                    add_pose_delta(&mut out, &pose, weight, mask);
                }
            }
            out
        }
        AnimationTreeNodeKind::Invert { input, mask } => {
            let mut pose = eval_node(tree, res, nodes, input.as_ref(), visiting)?;
            invert_pose(&mut pose, mask);
            pose
        }
    };
    visiting.remove(key);
    Some(pose)
}

fn eval_slot_pose<R>(tree: &AnimationTree, res: &ResourceWindow<'_, R>, slot_name: &str) -> Pose
where
    R: ResourceAPI + ?Sized,
{
    let Some(slot_index) = tree
        .internal
        .slots
        .iter()
        .position(|s| s.name.as_ref() == slot_name)
    else {
        return Pose::default();
    };
    let Some(slot) = tree.internal.slots.get(slot_index) else {
        return Pose::default();
    };
    let Some(animation) = tree.animations.get(slot_index) else {
        return Pose::default();
    };
    let Some(clip) = res.Animations().get(animation.animation) else {
        return Pose::default();
    };
    sample_clip_pose(&clip, slot.current_frame, &animation.bindings)
}

fn sample_clip_pose(
    clip: &Arc<AnimationClip>,
    frame: u32,
    bindings: &[perro_nodes::animation_player::AnimationObjectBinding],
) -> Pose {
    let mut pose = Pose::default();
    for track in clip.object_tracks.iter() {
        let Some(value) = super::animation_player::sample_track_value(track, frame) else {
            continue;
        };
        let Some(binding) = bindings
            .iter()
            .find(|binding| binding.object.as_ref() == track.object.as_ref())
        else {
            continue;
        };
        let key = pose_key(
            binding.node,
            track.object.as_ref(),
            track.field,
            &track.bone_target,
        );
        pose.tracks.insert(
            key,
            PoseTrack {
                node: binding.node,
                object: track.object.clone(),
                field: track.field,
                bone_target: track.bone_target.clone(),
                value,
            },
        );
    }
    pose
}

fn blend_poses(poses: &[Pose], weights: &[f32], mask: &AnimationTreeMask) -> Pose {
    if poses.is_empty() {
        return Pose::default();
    }
    let sum: f32 = weights.iter().copied().filter(|v| *v > 0.0).sum();
    if sum <= f32::EPSILON {
        return poses[0].clone();
    }
    let mut out = Pose::default();
    let mut keys = BTreeMap::<String, ()>::new();
    for pose in poses {
        for key in pose.tracks.keys() {
            keys.insert(key.clone(), ());
        }
    }
    for key in keys.keys() {
        let mut acc: Option<PoseTrack> = None;
        for (idx, pose) in poses.iter().enumerate() {
            let Some(track) = pose.tracks.get(key) else {
                continue;
            };
            if !mask_allows(mask, track) {
                continue;
            }
            let w = weights.get(idx).copied().unwrap_or(0.0).max(0.0) / sum;
            if w <= 0.0 {
                continue;
            }
            acc = Some(if let Some(mut prev) = acc {
                prev.value = add_value(&prev.value, &scale_value(&track.value, w));
                prev
            } else {
                let mut first = track.clone();
                first.value = scale_value(&first.value, w);
                first
            });
        }
        if let Some(track) = acc {
            out.tracks.insert(key.clone(), track);
        }
    }
    out
}

fn add_pose_delta(base: &mut Pose, pose: &Pose, weight: f32, mask: &AnimationTreeMask) {
    if weight == 0.0 {
        return;
    }
    for (key, track) in &pose.tracks {
        if !mask_allows(mask, track) {
            continue;
        }
        if let Some(existing) = base.tracks.get_mut(key) {
            existing.value = add_value(&existing.value, &scale_value(&track.value, weight));
        } else {
            let mut next = track.clone();
            next.value = scale_value(&next.value, weight);
            base.tracks.insert(key.clone(), next);
        }
    }
}

fn invert_pose(pose: &mut Pose, mask: &AnimationTreeMask) {
    for track in pose.tracks.values_mut() {
        if mask_allows(mask, track) {
            track.value = scale_value(&track.value, -1.0);
        }
    }
}

fn apply_pose<RT, R>(ctx: &mut RuntimeWindow<'_, RT>, res: &ResourceWindow<'_, R>, pose: &Pose)
where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
{
    for track in pose.tracks.values() {
        let key = AnimationObjectKey {
            frame: 0,
            mode: perro_animation::AnimationKeyMode::Closed,
            interpolation: perro_animation::AnimationInterpolation::Step,
            ease: perro_animation::AnimationEase::Linear,
            value: track.value.clone(),
        };
        let anim_track = AnimationObjectTrack {
            object: track.object.clone(),
            field: track.field,
            bone_target: track.bone_target.clone(),
            interpolation: perro_animation::AnimationInterpolation::Step,
            ease: perro_animation::AnimationEase::Linear,
            keys: Cow::Owned(vec![key]),
        };
        super::animation_player::apply_track(ctx, res, track.node, &anim_track, 0);
    }
}

fn fire_slot_events<RT, R>(ctx: &mut RuntimeWindow<'_, RT>, res: &ResourceWindow<'_, R>, id: NodeID)
where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
{
    let entries = with_node!(ctx, SelfNodeType, id, |tree| {
        tree.internal
            .slots
            .iter()
            .enumerate()
            .filter_map(|(idx, slot)| {
                tree.animations
                    .get(idx)
                    .cloned()
                    .map(|animation| (animation, slot.current_frame))
            })
            .collect::<Vec<_>>()
    });
    for (animation, frame) in entries {
        let Some(clip) = res.Animations().get(animation.animation) else {
            continue;
        };
        super::animation_player::apply_frame_events(ctx, &clip, frame, &animation.bindings);
    }
}

fn runtime_weight(
    tree: &AnimationTree,
    node: &str,
    input: &str,
    weights: &[f32],
    index: usize,
) -> f32 {
    tree.internal
        .weights
        .iter()
        .find(|w| w.node.as_ref() == node && w.input.as_ref() == input)
        .map(|w| w.weight)
        .unwrap_or_else(|| weights.get(index).copied().unwrap_or(1.0))
}

fn mask_allows(mask: &AnimationTreeMask, track: &PoseTrack) -> bool {
    if mask.is_empty() {
        return true;
    }
    let object_ok = mask.objects.is_empty()
        || mask
            .objects
            .iter()
            .any(|v| v.as_ref() == track.object.as_ref());
    let field_name = field_mask_name(track.field);
    let field_ok = mask.fields.is_empty()
        || mask
            .fields
            .iter()
            .any(|v| v.as_ref().eq_ignore_ascii_case(field_name));
    let bone_ok = if let Some(target) = &track.bone_target {
        mask.bones.is_empty()
            || mask.bones.iter().any(|v| match &target.selector {
                AnimationBoneSelector::Index(index) => v.as_ref() == index.to_string(),
                AnimationBoneSelector::Name(name) => v.as_ref() == name.as_ref(),
            })
    } else {
        mask.bones.is_empty()
    };
    object_ok && field_ok && bone_ok
}

fn field_mask_name(field: NodeField) -> &'static str {
    match field {
        NodeField::Node3D(Node3DField::Position) => "position",
        NodeField::Node3D(Node3DField::Rotation) => "rotation",
        NodeField::Node3D(Node3DField::Scale) => "scale",
        NodeField::Node3D(Node3DField::Visible) => "visible",
        _ => "",
    }
}

fn pose_key(
    node: NodeID,
    object: &str,
    field: NodeField,
    bone_target: &Option<perro_animation::AnimationBoneTarget>,
) -> String {
    let bone = match bone_target {
        Some(target) => match &target.selector {
            AnimationBoneSelector::Index(index) => format!("i{index}"),
            AnimationBoneSelector::Name(name) => format!("n{}", name.as_ref()),
        },
        None => String::new(),
    };
    format!("{}:{object}:{field:?}:{bone}", node.as_u64())
}

fn scale_value(value: &AnimationTrackValue, weight: f32) -> AnimationTrackValue {
    match value {
        AnimationTrackValue::F32(v) => AnimationTrackValue::F32(v * weight),
        AnimationTrackValue::Vec2(v) => AnimationTrackValue::Vec2([v[0] * weight, v[1] * weight]),
        AnimationTrackValue::Vec3(v) => {
            AnimationTrackValue::Vec3([v[0] * weight, v[1] * weight, v[2] * weight])
        }
        AnimationTrackValue::Transform3D(v) => {
            let mut out = *v;
            out.position.x *= weight;
            out.position.y *= weight;
            out.position.z *= weight;
            AnimationTrackValue::Transform3D(out)
        }
        _ => value.clone(),
    }
}

fn add_value(a: &AnimationTrackValue, b: &AnimationTrackValue) -> AnimationTrackValue {
    match (a, b) {
        (AnimationTrackValue::F32(a), AnimationTrackValue::F32(b)) => {
            AnimationTrackValue::F32(a + b)
        }
        (AnimationTrackValue::Vec2(a), AnimationTrackValue::Vec2(b)) => {
            AnimationTrackValue::Vec2([a[0] + b[0], a[1] + b[1]])
        }
        (AnimationTrackValue::Vec3(a), AnimationTrackValue::Vec3(b)) => {
            AnimationTrackValue::Vec3([a[0] + b[0], a[1] + b[1], a[2] + b[2]])
        }
        (AnimationTrackValue::Transform3D(a), AnimationTrackValue::Transform3D(b)) => {
            let mut out = *a;
            out.position.x += b.position.x;
            out.position.y += b.position.y;
            out.position.z += b.position.z;
            AnimationTrackValue::Transform3D(out)
        }
        _ => a.clone(),
    }
}
