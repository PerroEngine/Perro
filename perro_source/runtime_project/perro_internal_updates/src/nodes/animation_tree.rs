use crate::prelude::*;
use perro_animation::{
    AnimationBoneSelector, AnimationBoneTarget, AnimationClip, AnimationTrackValue,
    AnimationTreeAsset, AnimationTreeGraphNode, AnimationTreeMask, AnimationTreeNodeKind,
};
use perro_nodes::AnimationTree;
use perro_nodes::animation_player::AnimationObjectBinding;
use perro_nodes::animation_tree::{AnimationTreeRuntimeWeight, AnimationTreeSlotPlayback};
use perro_scene::{Node3DField, NodeField};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hash, Hasher};

thread_local! {
    /// Per-thread eval scratch: cycle-guard stamps, the pose-key interner, the
    /// pose buffer pool + the binding sort order. Persisting the interner across
    /// frames is what makes key clones a one-time cost per distinct key instead
    /// of a per-frame cost (see `EvalScratch::intern`).
    static EVAL_SCRATCH: RefCell<EvalScratch> = RefCell::new(EvalScratch::default());
}

/// Interner reset threshold. Keys accumulate as scenes swap `NodeID`s, so drop
/// the whole table once it grows past a plausible live working set.
const MAX_INTERNED_KEYS: usize = 16_384;

/// Above this binding count the per-track `find` scan is replaced by a sorted
/// index + binary search. Below it the scan wins outright.
const BINDING_SORT_THRESHOLD: usize = 8;

// Compact pose-track identity. Replaces the old formatted `String` key so
// sampling/blending no longer allocs per track per frame.
// Owns `object` + `bone` outright: `PoseTrack` reads them thru the key instead
// of holding a second copy, halving the `Cow` clones per sampled track.
// Now lives only inside the interner: a `Pose` stores dense interned ids, so a
// key is cloned once ever (first sight) rather than once per blend/insert.
#[derive(Clone, PartialEq, Eq)]
struct PoseKey {
    node: NodeID,
    object: Cow<'static, str>,
    field: NodeField,
    bone: Option<AnimationBoneTarget>,
}

#[derive(Clone)]
struct PoseTrack {
    node: NodeID,
    field: NodeField,
    transform2d_mask: u8,
    transform3d_mask: u8,
    value: AnimationTrackValue,
}

/// Dense pose storage indexed by the interned *local* id of a `PoseKey`.
///
/// Two tracks collide in blend/add iff their interned ids match, and ids match
/// iff the `PoseKey`s are `Eq` — the identical collision rule the old
/// `HashMap<PoseKey, PoseTrack>` used. Everything downstream (blend, add,
/// invert, apply) becomes a dense `Vec` walk with no per-entry hash, alloc or
/// key clone.
type Pose = Vec<Option<PoseTrack>>;

/// FNV-1a, used to derive the interner bucket hash. Content matches the old
/// `Hash for PoseKey` impl (outer `NodeField` discriminant only); exactness
/// comes from the full `Eq` compare done on every bucket candidate.
struct Fnv1a(u64);

impl Fnv1a {
    #[inline]
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Hasher for Fnv1a {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= *byte as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

/// Bucket keys are already well-mixed FNV output, so re-hashing them with
/// SipHash is pure overhead.
#[derive(Default)]
struct IdentityHasher(u64);

impl Hasher for IdentityHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = self.0.rotate_left(8) ^ (*byte as u64);
        }
    }

    #[inline]
    fn write_u64(&mut self, value: u64) {
        self.0 = value;
    }
}

type BucketMap = HashMap<u64, Vec<u32>, BuildHasherDefault<IdentityHasher>>;

fn pose_key_hash(
    node: NodeID,
    object: &str,
    field: NodeField,
    bone: Option<&AnimationBoneTarget>,
) -> u64 {
    let mut hasher = Fnv1a::new();
    node.hash(&mut hasher);
    object.hash(&mut hasher);
    std::mem::discriminant(&field).hash(&mut hasher);
    match bone.map(|target| &target.selector) {
        None => hasher.write_u8(0),
        Some(AnimationBoneSelector::Index(index)) => {
            hasher.write_u8(1);
            index.hash(&mut hasher);
        }
        Some(AnimationBoneSelector::Name(name)) => {
            hasher.write_u8(2);
            name.hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// Per-key eval stamp. `generation` marks the eval a key was last seen in;
/// `local` is its dense slot inside that eval.
#[derive(Clone, Copy, Default)]
struct InternSlot {
    generation: u64,
    local: u32,
}

#[derive(Default)]
struct EvalScratch {
    /// Global id -> key. Persists across evals so `Cow` clones amortize away.
    keys: Vec<PoseKey>,
    /// Global id -> eval stamp. Parallel to `keys`.
    slots: Vec<InternSlot>,
    /// Key hash -> candidate global ids.
    buckets: BucketMap,
    /// Local id -> global id, rebuilt per eval by generation stamping.
    live: Vec<u32>,
    generation: u64,
    /// Recycled pose buffers, so blend/add/sample never allocate steady-state.
    pool: Vec<Pose>,
    /// Cycle guard, indexed by graph-node index.
    visiting: Vec<bool>,
    /// Binding indices sorted by object name (ties by original index).
    binding_order: Vec<u32>,
}

impl EvalScratch {
    fn begin_eval(&mut self, node_count: usize) {
        self.generation += 1;
        self.live.clear();
        self.visiting.clear();
        self.visiting.resize(node_count, false);
        if self.keys.len() > MAX_INTERNED_KEYS {
            self.keys.clear();
            self.slots.clear();
            self.buckets.clear();
        }
    }

    /// One hash lookup per (track, eval) — the same count the old `HashMap`
    /// insert did — in exchange for dense ids everywhere downstream.
    // `&Cow` not `&str` on purpose: on a miss the key is built by *cloning* the
    // caller's `Cow`, which is free for the `Borrowed` case. Taking `&str` would
    // force a `String` alloc on every new key.
    #[allow(clippy::ptr_arg)]
    fn intern(
        &mut self,
        node: NodeID,
        object: &Cow<'static, str>,
        field: NodeField,
        bone: &Option<AnimationBoneTarget>,
    ) -> usize {
        let hash = pose_key_hash(node, object.as_ref(), field, bone.as_ref());
        let Self {
            keys,
            slots,
            buckets,
            live,
            generation,
            ..
        } = self;
        let bucket = buckets.entry(hash).or_default();
        let mut global = u32::MAX;
        for candidate in bucket.iter().copied() {
            let key = &keys[candidate as usize];
            if key.node == node
                && key.field == field
                && key.object.as_ref() == object.as_ref()
                && key.bone.as_ref() == bone.as_ref()
            {
                global = candidate;
                break;
            }
        }
        if global == u32::MAX {
            global = keys.len() as u32;
            keys.push(PoseKey {
                node,
                object: object.clone(),
                field,
                bone: bone.clone(),
            });
            slots.push(InternSlot::default());
            bucket.push(global);
        }
        let slot = &mut slots[global as usize];
        if slot.generation != *generation {
            slot.generation = *generation;
            slot.local = live.len() as u32;
            live.push(global);
        }
        slot.local as usize
    }

    #[inline]
    fn key(&self, local: usize) -> &PoseKey {
        &self.keys[self.live[local] as usize]
    }

    #[inline]
    fn take_pose(&mut self) -> Pose {
        match self.pool.pop() {
            Some(mut pose) => {
                pose.clear();
                pose
            }
            None => Pose::new(),
        }
    }

    #[inline]
    fn give_pose(&mut self, mut pose: Pose) {
        if self.pool.len() < 32 {
            pose.clear();
            self.pool.push(pose);
        }
    }

    /// Replaces the O(tracks x bindings) `bindings.iter().find` string scan.
    /// Ties sort by original index so a lookup still resolves to the *first*
    /// matching binding, exactly like `find`.
    fn prepare_bindings(&mut self, bindings: &[AnimationObjectBinding]) {
        self.binding_order.clear();
        if bindings.len() <= BINDING_SORT_THRESHOLD {
            return;
        }
        self.binding_order.extend(0..bindings.len() as u32);
        self.binding_order.sort_unstable_by(|a, b| {
            bindings[*a as usize]
                .object
                .as_ref()
                .cmp(bindings[*b as usize].object.as_ref())
                .then(a.cmp(b))
        });
    }

    fn resolve_binding(&self, bindings: &[AnimationObjectBinding], object: &str) -> Option<NodeID> {
        if self.binding_order.is_empty() {
            return bindings
                .iter()
                .find(|binding| binding.object.as_ref() == object)
                .map(|binding| binding.node);
        }
        let at = self
            .binding_order
            .partition_point(|index| bindings[*index as usize].object.as_ref() < object);
        let binding = &bindings[*self.binding_order.get(at)? as usize];
        (binding.object.as_ref() == object).then_some(binding.node)
    }
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
    let Some(tree_id) = with_node!(ctx, AnimationTree, id, |tree| tree.tree) else {
        return;
    };
    if tree_id.is_nil() {
        return;
    }
    let Some(asset) = res.AnimationTrees().get(tree_id) else {
        return;
    };
    sync_slots(ctx, id, &asset);
    step_slots(ctx, res, id);
    let Some(pose) = with_node!(ctx, AnimationTree, id, |tree| eval_tree_pose(
        tree, res, &asset
    ))
    .warn_none_once(format_args!(
        "animation tree pose skip: node={} expect=AnimationTree missing",
        id.as_u64()
    )) else {
        return;
    };
    let Some(mut applied_transforms) = with_node_mut!(ctx, AnimationTree, id, |tree| {
        std::mem::take(&mut tree.internal.applied_transforms)
    })
    .warn_none_once(format_args!(
        "animation tree apply skip: node={} expect=AnimationTree missing",
        id.as_u64()
    )) else {
        return;
    };
    apply_pose(ctx, res, &pose, &mut applied_transforms);
    release_pose(pose);
    let _ = with_node_mut!(ctx, AnimationTree, id, |tree| {
        tree.internal.applied_transforms = applied_transforms;
    });
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

fn sync_slots<RT>(ctx: &mut RuntimeWindow<'_, RT>, id: NodeID, asset: &AnimationTreeAsset)
where
    RT: RuntimeAPI + ?Sized,
{
    let _ = with_node_mut!(ctx, AnimationTree, id, |tree| {
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
                    last_animation: AnimationID::nil(),
                    current_frame: 0,
                    playback_frame: 0.0,
                    boomerang_direction: 1.0,
                    paused: false,
                    last_event_animation: AnimationID::nil(),
                    last_event_frame: u32::MAX,
                    pending_event_frames: Vec::new(),
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
    let _ = with_node_mut!(ctx, AnimationTree, id, |tree| {
        for idx in 0..tree.internal.slots.len() {
            // Borrow: `.cloned()` here deep-copied the whole slot entry incl its
            // bindings `Vec` every frame. Only POD fields are read below, and
            // `animations` / `internal.slots` are disjoint fields so the shared
            // borrow lives alongside the `&mut slot`.
            let entry = tree.animations.get(idx);
            let entry_animation = entry.map(|e| e.animation).unwrap_or_else(AnimationID::nil);
            let entry_speed = entry.map(|e| e.speed).unwrap_or(1.0);
            let entry_paused = entry.is_some_and(|e| e.paused);
            let entry_playback_type = entry.map(|e| e.playback_type).unwrap_or_default();
            let slot = &mut tree.internal.slots[idx];
            slot.pending_event_frames.clear();
            let animation = if res.Animations().is_loaded(entry_animation) {
                slot.last_animation = entry_animation;
                entry_animation
            } else if res.Animations().is_loaded(slot.last_animation) {
                slot.last_animation
            } else {
                AnimationID::nil()
            };
            if animation.is_nil() {
                continue;
            }
            let Some(clip) = res.Animations().get(animation) else {
                continue;
            };
            let frame_count = clip.frame_count();
            let previous_playback_frame = slot.playback_frame;
            let previous_direction = slot.boomerang_direction;
            if frame_count <= 1 {
                slot.current_frame = 0;
                slot.playback_frame = 0.0;
                slot.boomerang_direction = 1.0;
            } else if !(tree.paused || slot.paused || entry_paused) {
                let delta_frames = delta_seconds * clip.fps.max(0.0) * tree.speed * entry_speed;
                slot.playback_frame = super::animation_player::advance_playback_frame(
                    slot.playback_frame,
                    delta_frames,
                    frame_count,
                    entry_playback_type,
                    &mut slot.boomerang_direction,
                );
                super::animation_player::crossed_animation_frames(
                    previous_playback_frame,
                    delta_frames,
                    frame_count,
                    entry_playback_type,
                    previous_direction,
                    &clip.frame_events,
                    &mut slot.pending_event_frames,
                );
                slot.current_frame = super::animation_player::playback_frame_to_frame(
                    slot.playback_frame,
                    frame_count,
                    entry_playback_type,
                );
            }
            queue_current_slot_event_once(slot, animation, &clip.frame_events);
        }
    });
}

fn queue_current_slot_event_once(
    slot: &mut AnimationTreeSlotPlayback,
    animation: AnimationID,
    events: &[perro_animation::AnimationFrameEvent],
) {
    let cursor_changed =
        slot.last_event_animation != animation || slot.last_event_frame != slot.current_frame;
    if cursor_changed
        && slot.pending_event_frames.last().copied() != Some(slot.current_frame)
        && super::animation_player::frame_has_event(events, slot.current_frame)
    {
        slot.pending_event_frames.push(slot.current_frame);
    }
    slot.last_event_animation = animation;
    slot.last_event_frame = slot.current_frame;
}

fn eval_tree_pose<R>(
    tree: &AnimationTree,
    res: &ResourceWindow<'_, R>,
    asset: &AnimationTreeAsset,
) -> Pose
where
    R: ResourceAPI + ?Sized,
{
    // Was: fresh name->node `HashMap` per tree per frame + a `String` per visit.
    // Graph node counts are tiny, so a linear key scan + an index-keyed stamp
    // beat both. Only graph keys ever stay in `visiting` across recursion (slot
    // keys were inserted + removed in the same step), so index stamps match the
    // old string-set semantics exactly.
    EVAL_SCRATCH.with(|cell| {
        let mut scratch = cell.borrow_mut();
        scratch.begin_eval(asset.nodes.len());
        let mut sample_slot =
            |name: &str, scratch: &mut EvalScratch| eval_slot_pose(tree, res, name, scratch);
        eval_node(
            asset,
            &tree.internal.weights,
            asset.output.as_ref(),
            &mut scratch,
            &mut sample_slot,
        )
        .unwrap_or_default()
    })
}

fn eval_node(
    asset: &AnimationTreeAsset,
    runtime_weights: &[AnimationTreeRuntimeWeight],
    key: &str,
    scratch: &mut EvalScratch,
    sample_slot: &mut dyn FnMut(&str, &mut EvalScratch) -> Pose,
) -> Option<Pose> {
    let Some(index) = asset.nodes.iter().position(|node| node.key.as_ref() == key) else {
        return Some(sample_slot(key, scratch));
    };
    if scratch.visiting[index] {
        return None;
    }
    scratch.visiting[index] = true;
    let node: &AnimationTreeGraphNode = &asset.nodes[index];
    let pose = match &node.kind {
        AnimationTreeNodeKind::Blend {
            inputs,
            weights,
            mask,
        } => {
            let mut poses = Vec::with_capacity(inputs.len());
            let mut raw_weights = Vec::with_capacity(inputs.len());
            for (idx, input) in inputs.iter().enumerate() {
                if let Some(pose) =
                    eval_node(asset, runtime_weights, input.as_ref(), scratch, sample_slot)
                {
                    poses.push(pose);
                    raw_weights.push(runtime_weight(
                        runtime_weights,
                        key,
                        input.as_ref(),
                        weights,
                        idx,
                    ));
                }
            }
            let out = blend_poses(&poses, &raw_weights, mask, scratch);
            for pose in poses {
                scratch.give_pose(pose);
            }
            out
        }
        AnimationTreeNodeKind::Add {
            base,
            inputs,
            weights,
            mask,
        } => {
            let mut out = eval_node(asset, runtime_weights, base.as_ref(), scratch, sample_slot)?;
            for (idx, input) in inputs.iter().enumerate() {
                if let Some(pose) =
                    eval_node(asset, runtime_weights, input.as_ref(), scratch, sample_slot)
                {
                    let weight = runtime_weight(runtime_weights, key, input.as_ref(), weights, idx);
                    add_pose_delta(&mut out, &pose, weight, mask, scratch);
                    scratch.give_pose(pose);
                }
            }
            out
        }
        AnimationTreeNodeKind::Invert { input, mask } => {
            let mut pose = eval_node(asset, runtime_weights, input.as_ref(), scratch, sample_slot)?;
            invert_pose(&mut pose, mask, scratch);
            pose
        }
    };
    scratch.visiting[index] = false;
    Some(pose)
}

fn eval_slot_pose<R>(
    tree: &AnimationTree,
    res: &ResourceWindow<'_, R>,
    slot_name: &str,
    scratch: &mut EvalScratch,
) -> Pose
where
    R: ResourceAPI + ?Sized,
{
    let Some(slot_index) = tree
        .internal
        .slots
        .iter()
        .position(|s| s.name.as_ref() == slot_name)
    else {
        return scratch.take_pose();
    };
    let Some(slot) = tree.internal.slots.get(slot_index) else {
        return scratch.take_pose();
    };
    let Some(animation) = tree.animations.get(slot_index) else {
        return scratch.take_pose();
    };
    let animation_id = if res.Animations().is_loaded(animation.animation) {
        animation.animation
    } else {
        slot.last_animation
    };
    let Some(clip) = res.Animations().get(animation_id) else {
        return scratch.take_pose();
    };
    sample_clip_pose(&clip, slot.current_frame, &animation.bindings, scratch)
}

fn sample_clip_pose(
    clip: &AnimationClip,
    frame: u32,
    bindings: &[AnimationObjectBinding],
    scratch: &mut EvalScratch,
) -> Pose {
    let mut pose = scratch.take_pose();
    scratch.prepare_bindings(bindings);
    for track in clip.object_tracks.iter() {
        let Some(value) = super::animation_player::sample_track_value(track, frame) else {
            continue;
        };
        let Some(node) = scratch.resolve_binding(bindings, track.object.as_ref()) else {
            continue;
        };
        let local = scratch.intern(node, &track.object, track.field, &track.bone_target);
        if pose.len() <= local {
            pose.resize(local + 1, None);
        }
        // Last writer wins, matching the old `HashMap::insert`.
        pose[local] = Some(PoseTrack {
            node,
            field: track.field,
            transform2d_mask: track.transform2d_mask,
            transform3d_mask: track.transform3d_mask,
            value,
        });
    }
    pose
}

fn blend_poses(
    poses: &[Pose],
    weights: &[f32],
    mask: &AnimationTreeMask,
    scratch: &mut EvalScratch,
) -> Pose {
    if poses.is_empty() {
        return scratch.take_pose();
    }
    let sum: f32 = weights.iter().copied().filter(|v| *v > 0.0).sum();
    let mut out = scratch.take_pose();
    if sum <= f32::EPSILON {
        out.extend_from_slice(&poses[0]);
        return out;
    }
    let len = poses.iter().map(|pose| pose.len()).max().unwrap_or(0);
    out.resize(len, None);
    // Dense walk over the union of interned ids: same key set the old
    // `HashSet<&PoseKey>` union produced, minus the set + its hashing.
    for (local, out_slot) in out.iter_mut().enumerate() {
        let mut acc: Option<(PoseTrack, BlendWeights)> = None;
        for (idx, pose) in poses.iter().enumerate() {
            let Some(track) = pose.get(local).and_then(|slot| slot.as_ref()) else {
                continue;
            };
            let key = scratch.key(local);
            if !mask_allows(mask, key.object.as_ref(), key.bone.as_ref(), track.field) {
                continue;
            }
            let w = weights.get(idx).copied().unwrap_or(0.0).max(0.0) / sum;
            if w <= 0.0 {
                continue;
            }
            acc = Some(if let Some((mut prev, mut blended_weights)) = acc {
                blend_track(&mut prev, &mut blended_weights, track, w);
                (prev, blended_weights)
            } else {
                (track.clone(), BlendWeights::new(track, w))
            });
        }
        *out_slot = acc.map(|(track, _)| track);
    }
    out
}

#[derive(Clone, Copy, Default)]
struct BlendWeights {
    value: f32,
    position: f32,
    rotation: f32,
    scale: f32,
}

impl BlendWeights {
    fn new(track: &PoseTrack, weight: f32) -> Self {
        let transform_mask = if matches!(track.value, AnimationTrackValue::Transform2D(_)) {
            track.transform2d_mask
        } else {
            track.transform3d_mask
        };
        Self {
            value: weight,
            position: if transform_mask & perro_animation::ANIMATION_TRANSFORM_MASK_POSITION != 0 {
                weight
            } else {
                0.0
            },
            rotation: if transform_mask & perro_animation::ANIMATION_TRANSFORM_MASK_ROTATION != 0 {
                weight
            } else {
                0.0
            },
            scale: if transform_mask & perro_animation::ANIMATION_TRANSFORM_MASK_SCALE != 0 {
                weight
            } else {
                0.0
            },
        }
    }
}

fn blend_track(out: &mut PoseTrack, weights: &mut BlendWeights, next: &PoseTrack, weight: f32) {
    match (&mut out.value, &next.value) {
        (AnimationTrackValue::Transform2D(a), AnimationTrackValue::Transform2D(b)) => {
            blend_transform2d_channel(a, b, next.transform2d_mask, weights, weight);
        }
        (AnimationTrackValue::Transform3D(a), AnimationTrackValue::Transform3D(b)) => {
            blend_transform3d_channel(a, b, next.transform3d_mask, weights, weight);
        }
        (a, b) => {
            let total = weights.value + weight;
            *a = blend_value(a, b, weight / total);
            weights.value = total;
        }
    }
    out.transform2d_mask |= next.transform2d_mask;
    out.transform3d_mask |= next.transform3d_mask;
}

fn blend_transform2d_channel(
    out: &mut perro_runtime_api::perro_structs::Transform2D,
    next: &perro_runtime_api::perro_structs::Transform2D,
    mask: u8,
    weights: &mut BlendWeights,
    weight: f32,
) {
    if mask & perro_animation::ANIMATION_TRANSFORM_MASK_POSITION != 0 {
        let t = weight / (weights.position + weight);
        out.position = out.position.lerped(next.position, t);
        weights.position += weight;
    }
    if mask & perro_animation::ANIMATION_TRANSFORM_MASK_ROTATION != 0 {
        let t = weight / (weights.rotation + weight);
        out.rotation = lerp_angle(out.rotation, next.rotation, t);
        weights.rotation += weight;
    }
    if mask & perro_animation::ANIMATION_TRANSFORM_MASK_SCALE != 0 {
        let t = weight / (weights.scale + weight);
        out.scale = out.scale.lerped(next.scale, t);
        weights.scale += weight;
    }
}

fn blend_transform3d_channel(
    out: &mut perro_runtime_api::perro_structs::Transform3D,
    next: &perro_runtime_api::perro_structs::Transform3D,
    mask: u8,
    weights: &mut BlendWeights,
    weight: f32,
) {
    if mask & perro_animation::ANIMATION_TRANSFORM_MASK_POSITION != 0 {
        let t = weight / (weights.position + weight);
        out.position = out.position.lerped(next.position, t);
        weights.position += weight;
    }
    if mask & perro_animation::ANIMATION_TRANSFORM_MASK_ROTATION != 0 {
        let t = weight / (weights.rotation + weight);
        out.rotation = out.rotation.slerped(next.rotation, t);
        weights.rotation += weight;
    }
    if mask & perro_animation::ANIMATION_TRANSFORM_MASK_SCALE != 0 {
        let t = weight / (weights.scale + weight);
        out.scale = out.scale.lerped(next.scale, t);
        weights.scale += weight;
    }
}

fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    let delta =
        (b - a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    a + delta * t
}

fn blend_value(a: &AnimationTrackValue, b: &AnimationTrackValue, t: f32) -> AnimationTrackValue {
    match (a, b) {
        (AnimationTrackValue::F32(a), AnimationTrackValue::F32(b)) => {
            AnimationTrackValue::F32(a + (b - a) * t)
        }
        (AnimationTrackValue::Vec2(a), AnimationTrackValue::Vec2(b)) => {
            AnimationTrackValue::Vec2([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t])
        }
        (AnimationTrackValue::Vec3(a), AnimationTrackValue::Vec3(b)) => {
            AnimationTrackValue::Vec3([
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ])
        }
        _ if t >= 0.5 => b.clone(),
        _ => a.clone(),
    }
}

fn add_pose_delta(
    base: &mut Pose,
    pose: &Pose,
    weight: f32,
    mask: &AnimationTreeMask,
    scratch: &EvalScratch,
) {
    if weight == 0.0 {
        return;
    }
    if pose.len() > base.len() {
        base.resize(pose.len(), None);
    }
    for (local, slot) in pose.iter().enumerate() {
        let Some(track) = slot.as_ref() else {
            continue;
        };
        let key = scratch.key(local);
        if !mask_allows(mask, key.object.as_ref(), key.bone.as_ref(), track.field) {
            continue;
        }
        match base[local].as_mut() {
            Some(existing) => {
                existing.value = add_value(&existing.value, &scale_value(&track.value, weight));
                existing.transform2d_mask |= track.transform2d_mask;
                existing.transform3d_mask |= track.transform3d_mask;
            }
            None => {
                let mut next = track.clone();
                next.value = scale_value(&next.value, weight);
                base[local] = Some(next);
            }
        }
    }
}

fn invert_pose(pose: &mut Pose, mask: &AnimationTreeMask, scratch: &EvalScratch) {
    for (local, slot) in pose.iter_mut().enumerate() {
        let Some(track) = slot.as_mut() else {
            continue;
        };
        let key = scratch.key(local);
        if mask_allows(mask, key.object.as_ref(), key.bone.as_ref(), track.field) {
            track.value = scale_value(&track.value, -1.0);
        }
    }
}

/// Applies in ascending interned-id order, i.e. the order tracks were first
/// sampled this eval. Deterministic, unlike the old `HashMap` iteration order.
///
/// Borrows the interner to recover each track's bone target. Safe because
/// `apply_track_value` only writes node/bone state — it cannot re-enter an
/// animation-tree eval — and `pose` was interned in the eval that ran
/// immediately before this call.
fn apply_pose<RT, R>(
    ctx: &mut RuntimeWindow<'_, RT>,
    res: &ResourceWindow<'_, R>,
    pose: &Pose,
    applied_transforms: &mut Vec<perro_nodes::animation_player::AppliedAnimationTransform>,
) where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
{
    EVAL_SCRATCH.with(|cell| {
        let scratch = cell.borrow();
        for (local, slot) in pose.iter().enumerate() {
            let Some(track) = slot.as_ref() else {
                continue;
            };
            super::animation_player::apply_track_value(
                ctx,
                res,
                track.node,
                track.field,
                scratch.key(local).bone.as_ref(),
                track.transform2d_mask,
                track.transform3d_mask,
                &track.value,
                applied_transforms,
            );
        }
    });
}

fn release_pose(pose: Pose) {
    EVAL_SCRATCH.with(|cell| cell.borrow_mut().give_pose(pose));
}

fn fire_slot_events<RT, R>(ctx: &mut RuntimeWindow<'_, RT>, res: &ResourceWindow<'_, R>, id: NodeID)
where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
{
    let Some(entries) = with_node_mut!(ctx, AnimationTree, id, |tree| {
        tree.internal
            .slots
            .iter_mut()
            .enumerate()
            .filter_map(|(idx, slot)| {
                if slot.pending_event_frames.is_empty() {
                    return None;
                }
                tree.animations.get(idx).cloned().map(|animation| {
                    (
                        idx,
                        animation,
                        slot.last_event_animation,
                        std::mem::take(&mut slot.pending_event_frames),
                    )
                })
            })
            .collect::<Vec<_>>()
    })
    .warn_none_once(format_args!(
        "animation tree events skip: node={} expect=AnimationTree missing",
        id.as_u64()
    )) else {
        return;
    };
    for (idx, animation, animation_id, mut frames) in entries {
        let Some(clip) = res.Animations().get(animation_id) else {
            continue;
        };
        for frame in frames.iter().copied() {
            super::animation_player::apply_frame_events(ctx, &clip, frame, &animation.bindings);
        }
        frames.clear();
        let _ = with_node_mut!(ctx, AnimationTree, id, |tree| {
            if let Some(slot) = tree.internal.slots.get_mut(idx)
                && slot.pending_event_frames.is_empty()
            {
                slot.pending_event_frames = frames;
            }
        });
    }
}

fn runtime_weight(
    runtime_weights: &[AnimationTreeRuntimeWeight],
    node: &str,
    input: &str,
    weights: &[f32],
    index: usize,
) -> f32 {
    runtime_weights
        .iter()
        .find(|w| w.node.as_ref() == node && w.input.as_ref() == input)
        .map(|w| w.weight)
        .unwrap_or_else(|| weights.get(index).copied().unwrap_or(1.0))
}

fn mask_allows(
    mask: &AnimationTreeMask,
    object: &str,
    bone: Option<&AnimationBoneTarget>,
    field: NodeField,
) -> bool {
    if mask.is_empty() {
        return true;
    }
    let object_ok = mask.objects.is_empty() || mask.objects.iter().any(|v| v.as_ref() == object);
    let field_name = field_mask_name(field);
    let field_ok = mask.fields.is_empty()
        || mask
            .fields
            .iter()
            .any(|v| v.as_ref().eq_ignore_ascii_case(field_name));
    let bone_ok = if let Some(target) = bone {
        mask.bones.is_empty()
            || mask.bones.iter().any(|v| match &target.selector {
                AnimationBoneSelector::Index(index) => bone_index_str_eq(v.as_ref(), *index),
                AnimationBoneSelector::Name(name) => v.as_ref() == name.as_ref(),
            })
    } else {
        mask.bones.is_empty()
    };
    object_ok && field_ok && bone_ok
}

/// `text == index.to_string()` w/o the per-test `String` alloc. Formats into a
/// stack buffer so leading-zero / sign forms still miss, like the old compare.
fn bone_index_str_eq(text: &str, index: u32) -> bool {
    let mut buf = [0u8; 10];
    let mut cursor = buf.len();
    let mut rest = index;
    loop {
        cursor -= 1;
        buf[cursor] = b'0' + (rest % 10) as u8;
        rest /= 10;
        if rest == 0 {
            break;
        }
    }
    text.as_bytes() == &buf[cursor..]
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

#[cfg(test)]
mod tests;
