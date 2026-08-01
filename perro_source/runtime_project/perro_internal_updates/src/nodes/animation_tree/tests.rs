use super::*;
use perro_animation::{
    AnimationEase, AnimationEvent, AnimationEventScope, AnimationFrameEvent,
    AnimationInterpolation, AnimationKeyMode, AnimationObjectKey, AnimationObjectTrack,
    AnimationTreeSlot,
};
use perro_runtime_api::perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

// ---------------------------------------------------------------------------
// counting allocator (evidence for the alloc-churn claim)
// ---------------------------------------------------------------------------

thread_local! {
    /// Per-thread so the count is unaffected by tests running in parallel.
    /// `const` init + `Cell<u64>` => no lazy alloc, no TLS destructor, so the
    /// allocator itself can never recurse into it.
    static ALLOC_COUNT: Cell<u64> = const { Cell::new(0) };
}

struct CountingAlloc;

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOC_COUNT.try_with(|c| c.set(c.get() + 1));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOC_COUNT.try_with(|c| c.set(c.get() + 1));
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let _ = ALLOC_COUNT.try_with(|c| c.set(c.get() + 1));
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL_ALLOC: CountingAlloc = CountingAlloc;

fn count_allocs(mut body: impl FnMut()) -> u64 {
    let start = ALLOC_COUNT.with(Cell::get);
    body();
    ALLOC_COUNT.with(Cell::get) - start
}

// ---------------------------------------------------------------------------
// reference implementation: the pre-interning `HashMap<PoseKey, PoseTrack>`
// pose path, kept verbatim so parity can be proven rather than asserted.
// Only the *containers* differ; every arithmetic helper is shared with the
// production path so this cannot drift on the math.
// ---------------------------------------------------------------------------

mod reference {
    use super::super::*;
    use std::collections::{HashMap, HashSet};

    #[derive(Clone, PartialEq, Eq)]
    pub struct RefKey {
        pub node: NodeID,
        pub object: Cow<'static, str>,
        pub field: NodeField,
        pub bone: Option<AnimationBoneTarget>,
    }

    impl std::hash::Hash for RefKey {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.node.hash(state);
            self.object.hash(state);
            std::mem::discriminant(&self.field).hash(state);
            match self.bone.as_ref().map(|target| &target.selector) {
                None => state.write_u8(0),
                Some(AnimationBoneSelector::Index(index)) => {
                    state.write_u8(1);
                    index.hash(state);
                }
                Some(AnimationBoneSelector::Name(name)) => {
                    state.write_u8(2);
                    name.hash(state);
                }
            }
        }
    }

    #[derive(Clone, Default)]
    pub struct RefPose {
        pub tracks: HashMap<RefKey, PoseTrack>,
    }

    pub fn sample_clip_pose(
        clip: &AnimationClip,
        frame: u32,
        bindings: &[AnimationObjectBinding],
    ) -> RefPose {
        let mut pose = RefPose::default();
        for track in clip.object_tracks.iter() {
            let Some(value) = crate::nodes::animation_player::sample_track_value(track, frame)
            else {
                continue;
            };
            let Some(binding) = bindings
                .iter()
                .find(|binding| binding.object.as_ref() == track.object.as_ref())
            else {
                continue;
            };
            let key = RefKey {
                node: binding.node,
                object: track.object.clone(),
                field: track.field,
                bone: track.bone_target.clone(),
            };
            pose.tracks.insert(
                key,
                PoseTrack {
                    node: binding.node,
                    field: track.field,
                    transform2d_mask: track.transform2d_mask,
                    transform3d_mask: track.transform3d_mask,
                    value,
                },
            );
        }
        pose
    }

    pub fn blend_poses(poses: &[RefPose], weights: &[f32], mask: &AnimationTreeMask) -> RefPose {
        if poses.is_empty() {
            return RefPose::default();
        }
        let sum: f32 = weights.iter().copied().filter(|v| *v > 0.0).sum();
        if sum <= f32::EPSILON {
            return poses[0].clone();
        }
        let mut out = RefPose::default();
        let mut seen = HashSet::<&RefKey>::new();
        for pose in poses {
            for key in pose.tracks.keys() {
                if !seen.insert(key) {
                    continue;
                }
                let mut acc: Option<(PoseTrack, BlendWeights)> = None;
                for (idx, pose) in poses.iter().enumerate() {
                    let Some(track) = pose.tracks.get(key) else {
                        continue;
                    };
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
                if let Some((track, _)) = acc {
                    out.tracks.insert(key.clone(), track);
                }
            }
        }
        out
    }

    pub fn add_pose_delta(
        base: &mut RefPose,
        pose: &RefPose,
        weight: f32,
        mask: &AnimationTreeMask,
    ) {
        if weight == 0.0 {
            return;
        }
        for (key, track) in &pose.tracks {
            if !mask_allows(mask, key.object.as_ref(), key.bone.as_ref(), track.field) {
                continue;
            }
            if let Some(existing) = base.tracks.get_mut(key) {
                existing.value = add_value(&existing.value, &scale_value(&track.value, weight));
                existing.transform2d_mask |= track.transform2d_mask;
                existing.transform3d_mask |= track.transform3d_mask;
            } else {
                let mut next = track.clone();
                next.value = scale_value(&next.value, weight);
                base.tracks.insert(key.clone(), next);
            }
        }
    }

    pub fn invert_pose(pose: &mut RefPose, mask: &AnimationTreeMask) {
        let keys = pose.tracks.keys().cloned().collect::<Vec<_>>();
        for key in keys {
            let allowed = pose.tracks.get(&key).is_some_and(|track| {
                mask_allows(mask, key.object.as_ref(), key.bone.as_ref(), track.field)
            });
            if allowed && let Some(track) = pose.tracks.get_mut(&key) {
                track.value = scale_value(&track.value, -1.0);
            }
        }
    }

    pub fn eval_node(
        asset: &AnimationTreeAsset,
        runtime_weights: &[AnimationTreeRuntimeWeight],
        key: &str,
        visiting: &mut [bool],
        sample_slot: &mut dyn FnMut(&str) -> RefPose,
    ) -> Option<RefPose> {
        let Some(index) = asset.nodes.iter().position(|node| node.key.as_ref() == key) else {
            return Some(sample_slot(key));
        };
        if visiting[index] {
            return None;
        }
        visiting[index] = true;
        let node = &asset.nodes[index];
        let pose = match &node.kind {
            AnimationTreeNodeKind::Blend {
                inputs,
                weights,
                mask,
            } => {
                let mut poses = Vec::new();
                let mut raw_weights = Vec::new();
                for (idx, input) in inputs.iter().enumerate() {
                    if let Some(pose) = eval_node(
                        asset,
                        runtime_weights,
                        input.as_ref(),
                        visiting,
                        sample_slot,
                    ) {
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
                blend_poses(&poses, &raw_weights, mask)
            }
            AnimationTreeNodeKind::Add {
                base,
                inputs,
                weights,
                mask,
            } => {
                let base_pose =
                    eval_node(asset, runtime_weights, base.as_ref(), visiting, sample_slot)?;
                let mut out = base_pose;
                for (idx, input) in inputs.iter().enumerate() {
                    if let Some(pose) = eval_node(
                        asset,
                        runtime_weights,
                        input.as_ref(),
                        visiting,
                        sample_slot,
                    ) {
                        let weight =
                            runtime_weight(runtime_weights, key, input.as_ref(), weights, idx);
                        add_pose_delta(&mut out, &pose, weight, mask);
                    }
                }
                out
            }
            AnimationTreeNodeKind::Invert { input, mask } => {
                let mut pose = eval_node(
                    asset,
                    runtime_weights,
                    input.as_ref(),
                    visiting,
                    sample_slot,
                )?;
                invert_pose(&mut pose, mask);
                pose
            }
        };
        visiting[index] = false;
        Some(pose)
    }
}

// ---------------------------------------------------------------------------
// canonical, bit-exact pose comparison
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Canon {
    node: u64,
    object: String,
    field: String,
    bone: String,
    transform2d_mask: u8,
    transform3d_mask: u8,
    value: String,
}

fn f32_bits(value: f32) -> String {
    format!("{:08x}", value.to_bits())
}

fn value_repr(value: &AnimationTrackValue) -> String {
    match value {
        AnimationTrackValue::Bool(v) => format!("bool:{v}"),
        AnimationTrackValue::I32(v) => format!("i32:{v}"),
        AnimationTrackValue::U32(v) => format!("u32:{v}"),
        AnimationTrackValue::F32(v) => format!("f32:{}", f32_bits(*v)),
        AnimationTrackValue::Vec2(v) => {
            format!("v2:{},{}", f32_bits(v[0]), f32_bits(v[1]))
        }
        AnimationTrackValue::Vec3(v) => {
            format!(
                "v3:{},{},{}",
                f32_bits(v[0]),
                f32_bits(v[1]),
                f32_bits(v[2])
            )
        }
        AnimationTrackValue::Vec4(v) => format!(
            "v4:{},{},{},{}",
            f32_bits(v[0]),
            f32_bits(v[1]),
            f32_bits(v[2]),
            f32_bits(v[3])
        ),
        AnimationTrackValue::AssetPath(v) => format!("path:{v}"),
        AnimationTrackValue::Transform2D(t) => format!(
            "t2:{},{},{},{},{}",
            f32_bits(t.position.x),
            f32_bits(t.position.y),
            f32_bits(t.rotation),
            f32_bits(t.scale.x),
            f32_bits(t.scale.y)
        ),
        AnimationTrackValue::Transform3D(t) => format!(
            "t3:{},{},{},{},{},{},{},{},{},{}",
            f32_bits(t.position.x),
            f32_bits(t.position.y),
            f32_bits(t.position.z),
            f32_bits(t.rotation.x),
            f32_bits(t.rotation.y),
            f32_bits(t.rotation.z),
            f32_bits(t.rotation.w),
            f32_bits(t.scale.x),
            f32_bits(t.scale.y),
            f32_bits(t.scale.z)
        ),
    }
}

fn bone_repr(bone: Option<&AnimationBoneTarget>) -> String {
    match bone.map(|target| &target.selector) {
        None => "none".to_string(),
        Some(AnimationBoneSelector::Index(index)) => format!("idx:{index}"),
        Some(AnimationBoneSelector::Name(name)) => format!("name:{name}"),
    }
}

fn canon(
    node: NodeID,
    object: &str,
    bone: Option<&AnimationBoneTarget>,
    track: &PoseTrack,
) -> Canon {
    Canon {
        node: node.as_u64(),
        object: object.to_string(),
        field: format!("{:?}", track.field),
        bone: bone_repr(bone),
        transform2d_mask: track.transform2d_mask,
        transform3d_mask: track.transform3d_mask,
        value: value_repr(&track.value),
    }
}

// ---------------------------------------------------------------------------
// scenario harness
// ---------------------------------------------------------------------------

struct SlotState {
    name: String,
    clip: AnimationClip,
    bindings: Vec<AnimationObjectBinding>,
    frame: u32,
}

struct Scenario {
    asset: AnimationTreeAsset,
    weights: Vec<AnimationTreeRuntimeWeight>,
    slots: Vec<SlotState>,
}

/// Runs the production (interned) path and canonicalizes the applied output.
fn run_new(scenario: &Scenario) -> Vec<Canon> {
    EVAL_SCRATCH.with(|cell| {
        let mut scratch = cell.borrow_mut();
        scratch.begin_eval(scenario.asset.nodes.len());
        let mut sample = |name: &str, scratch: &mut EvalScratch| match scenario
            .slots
            .iter()
            .find(|slot| slot.name == name)
        {
            Some(slot) => sample_clip_pose(&slot.clip, slot.frame, &slot.bindings, scratch),
            None => scratch.take_pose(),
        };
        let pose = eval_node(
            &scenario.asset,
            &scenario.weights,
            scenario.asset.output.as_ref(),
            &mut scratch,
            &mut sample,
        )
        .unwrap_or_default();
        let mut out = Vec::new();
        for (local, slot) in pose.iter().enumerate() {
            if let Some(track) = slot.as_ref() {
                let key = scratch.key(local);
                out.push(canon(
                    track.node,
                    key.object.as_ref(),
                    key.bone.as_ref(),
                    track,
                ));
            }
        }
        scratch.give_pose(pose);
        out.sort();
        out
    })
}

/// Runs the pre-refactor `HashMap` path and canonicalizes the applied output.
fn run_reference(scenario: &Scenario) -> Vec<Canon> {
    let mut visiting = vec![false; scenario.asset.nodes.len()];
    let mut sample = |name: &str| match scenario.slots.iter().find(|slot| slot.name == name) {
        Some(slot) => reference::sample_clip_pose(&slot.clip, slot.frame, &slot.bindings),
        None => reference::RefPose::default(),
    };
    let pose = reference::eval_node(
        &scenario.asset,
        &scenario.weights,
        scenario.asset.output.as_ref(),
        &mut visiting,
        &mut sample,
    )
    .unwrap_or_default();
    let mut out = pose
        .tracks
        .iter()
        .map(|(key, track)| canon(track.node, key.object.as_ref(), key.bone.as_ref(), track))
        .collect::<Vec<_>>();
    out.sort();
    out
}

fn assert_parity(label: &str, scenario: &Scenario) {
    let expected = run_reference(scenario);
    let actual = run_new(scenario);
    assert_eq!(
        expected.len(),
        actual.len(),
        "{label}: track count differs\nreference={expected:#?}\nactual={actual:#?}"
    );
    for (index, (want, got)) in expected.iter().zip(actual.iter()).enumerate() {
        assert_eq!(want, got, "{label}: track {index} differs");
    }
}

// ---------------------------------------------------------------------------
// builders
// ---------------------------------------------------------------------------

fn key_at(frame: u32, value: AnimationTrackValue) -> AnimationObjectKey {
    AnimationObjectKey {
        frame,
        mode: AnimationKeyMode::Closed,
        interpolation: AnimationInterpolation::Linear,
        ease: AnimationEase::Linear,
        value,
    }
}

fn track(
    object: &str,
    field: NodeField,
    bone: Option<AnimationBoneTarget>,
    transform3d_mask: u8,
    values: Vec<(u32, AnimationTrackValue)>,
) -> AnimationObjectTrack {
    AnimationObjectTrack {
        object: Cow::Owned(object.to_string()),
        field,
        bone_target: bone,
        transform2d_mask: 0,
        transform3d_mask,
        interpolation: AnimationInterpolation::Linear,
        ease: AnimationEase::Linear,
        keys: Cow::Owned(
            values
                .into_iter()
                .map(|(frame, value)| key_at(frame, value))
                .collect(),
        ),
    }
}

fn f32_track(object: &str, field: NodeField, value: f32) -> AnimationObjectTrack {
    track(
        object,
        field,
        None,
        0,
        vec![(0, AnimationTrackValue::F32(value))],
    )
}

fn bone_track(
    object: &str,
    bone: AnimationBoneTarget,
    mask: u8,
    transform: Transform3D,
) -> AnimationObjectTrack {
    track(
        object,
        NodeField::Node3D(Node3DField::Position),
        Some(bone),
        mask,
        vec![(0, AnimationTrackValue::Transform3D(transform))],
    )
}

fn clip_of(tracks: Vec<AnimationObjectTrack>) -> AnimationClip {
    AnimationClip {
        name: Cow::Borrowed("clip"),
        fps: 30.0,
        total_frames: 8,
        objects: Cow::Borrowed(&[]),
        object_tracks: Cow::Owned(tracks),
        frame_events: Cow::Borrowed(&[]),
    }
}

fn binding(object: &str, node: u32) -> AnimationObjectBinding {
    AnimationObjectBinding {
        object: Cow::Owned(object.to_string()),
        node: NodeID::new(node),
    }
}

fn bone_index(index: u32) -> AnimationBoneTarget {
    AnimationBoneTarget {
        selector: AnimationBoneSelector::Index(index),
    }
}

fn bone_name(name: &str) -> AnimationBoneTarget {
    AnimationBoneTarget {
        selector: AnimationBoneSelector::Name(Cow::Owned(name.to_string())),
    }
}

fn transform3d(x: f32, y: f32, z: f32, yaw: f32, scale: f32) -> Transform3D {
    Transform3D::new(
        Vector3::new(x, y, z),
        Quaternion::from_euler_xyz(0.0, yaw, 0.0),
        Vector3::new(scale, scale, scale),
    )
}

fn asset_of(
    slots: &[&str],
    nodes: Vec<AnimationTreeGraphNode>,
    output: &str,
) -> AnimationTreeAsset {
    AnimationTreeAsset {
        name: Cow::Borrowed("tree"),
        slots: Cow::Owned(
            slots
                .iter()
                .map(|name| AnimationTreeSlot {
                    name: Cow::Owned(name.to_string()),
                })
                .collect(),
        ),
        nodes: Cow::Owned(nodes),
        output: Cow::Owned(output.to_string()),
    }
}

fn graph_node(key: &str, kind: AnimationTreeNodeKind) -> AnimationTreeGraphNode {
    AnimationTreeGraphNode {
        key: Cow::Owned(key.to_string()),
        kind,
    }
}

fn refs(names: &[&str]) -> Cow<'static, [Cow<'static, str>]> {
    Cow::Owned(
        names
            .iter()
            .map(|name| Cow::Owned(name.to_string()))
            .collect(),
    )
}

fn names(values: &[&str]) -> Cow<'static, [Cow<'static, str>]> {
    refs(values)
}

fn mask_of(objects: &[&str], fields: &[&str], bones: &[&str]) -> AnimationTreeMask {
    AnimationTreeMask {
        objects: names(objects),
        fields: names(fields),
        bones: names(bones),
    }
}

// ---------------------------------------------------------------------------
// scenarios
// ---------------------------------------------------------------------------

/// Two slots, partly overlapping bindings, blended under an object+field mask.
/// `Hips` is bound in both slots to the same node (collides), `Tail` only in
/// slot B (no collision), `Head` is bound to *different* nodes per slot, which
/// must stay two distinct pose entries.
fn scenario_blend_mask() -> Scenario {
    let idle = clip_of(vec![
        f32_track("Hips", NodeField::Node3D(Node3DField::Position), 1.0),
        f32_track("Hips", NodeField::Node3D(Node3DField::Rotation), 2.0),
        f32_track("Head", NodeField::Node3D(Node3DField::Position), 3.0),
        bone_track(
            "Hips",
            bone_index(4),
            perro_animation::ANIMATION_TRANSFORM_MASK_POSITION
                | perro_animation::ANIMATION_TRANSFORM_MASK_ROTATION,
            transform3d(1.0, 0.0, 0.0, 0.0, 1.0),
        ),
    ]);
    let run = clip_of(vec![
        f32_track("Hips", NodeField::Node3D(Node3DField::Position), 9.0),
        f32_track("Tail", NodeField::Node3D(Node3DField::Position), 5.0),
        f32_track("Head", NodeField::Node3D(Node3DField::Position), 7.0),
        bone_track(
            "Hips",
            bone_index(4),
            perro_animation::ANIMATION_TRANSFORM_MASK_ROTATION
                | perro_animation::ANIMATION_TRANSFORM_MASK_SCALE,
            transform3d(0.0, 3.0, 0.0, 1.2, 2.0),
        ),
        bone_track(
            "Hips",
            bone_name("Spine"),
            perro_animation::ANIMATION_TRANSFORM_MASK_POSITION,
            transform3d(0.0, 0.0, 4.0, 0.0, 1.0),
        ),
    ]);
    Scenario {
        asset: asset_of(
            &["Idle", "Run"],
            vec![graph_node(
                "Move",
                AnimationTreeNodeKind::Blend {
                    inputs: refs(&["Idle", "Run"]),
                    weights: Cow::Owned(vec![0.25, 0.75]),
                    mask: mask_of(
                        &["Hips", "Head"],
                        &["position", "rotation"],
                        &["4", "Spine"],
                    ),
                },
            )],
            "Move",
        ),
        weights: Vec::new(),
        slots: vec![
            SlotState {
                name: "Idle".to_string(),
                clip: idle,
                bindings: vec![binding("Hips", 10), binding("Head", 11)],
                frame: 0,
            },
            SlotState {
                name: "Run".to_string(),
                clip: run,
                // `Head` binds to a *different* node here -> distinct pose key.
                bindings: vec![
                    binding("Hips", 10),
                    binding("Tail", 12),
                    binding("Head", 13),
                ],
                frame: 0,
            },
        ],
    }
}

/// Add node: base blend + two masked deltas, one of which introduces keys the
/// base does not have (the `else` insert branch) and one that only overlaps.
fn scenario_add_delta() -> Scenario {
    let base = clip_of(vec![
        f32_track("Hips", NodeField::Node3D(Node3DField::Position), 1.0),
        f32_track("Arm", NodeField::Node3D(Node3DField::Position), 2.0),
        bone_track(
            "Hips",
            bone_index(0),
            perro_animation::ANIMATION_TRANSFORM_MASK_POSITION,
            transform3d(1.0, 1.0, 1.0, 0.0, 1.0),
        ),
    ]);
    let delta_a = clip_of(vec![
        f32_track("Hips", NodeField::Node3D(Node3DField::Position), 10.0),
        f32_track("Leg", NodeField::Node3D(Node3DField::Position), 20.0),
    ]);
    let delta_b = clip_of(vec![
        bone_track(
            "Hips",
            bone_index(0),
            perro_animation::ANIMATION_TRANSFORM_MASK_POSITION,
            transform3d(5.0, 0.0, 0.0, 0.0, 1.0),
        ),
        f32_track("Arm", NodeField::Node3D(Node3DField::Scale), 3.0),
    ]);
    Scenario {
        asset: asset_of(
            &["Base", "DeltaA", "DeltaB"],
            vec![graph_node(
                "Additive",
                AnimationTreeNodeKind::Add {
                    base: Cow::Borrowed("Base"),
                    inputs: refs(&["DeltaA", "DeltaB"]),
                    weights: Cow::Owned(vec![0.5, -0.25]),
                    mask: mask_of(&[], &["position"], &[]),
                },
            )],
            "Additive",
        ),
        weights: Vec::new(),
        slots: vec![
            SlotState {
                name: "Base".to_string(),
                clip: base,
                bindings: vec![binding("Hips", 1), binding("Arm", 2)],
                frame: 0,
            },
            SlotState {
                name: "DeltaA".to_string(),
                clip: delta_a,
                bindings: vec![binding("Hips", 1), binding("Leg", 3)],
                frame: 0,
            },
            SlotState {
                name: "DeltaB".to_string(),
                clip: delta_b,
                bindings: vec![binding("Hips", 1), binding("Arm", 2)],
                frame: 0,
            },
        ],
    }
}

/// Nested graph: Add( Blend(Idle, Invert(Run)), Additive ), runtime weight
/// overrides, >8 bindings (exercises the sorted binding-resolution path) and a
/// duplicated binding object (must resolve to the *first* entry, like `find`).
fn scenario_nested_graph() -> Scenario {
    let mut wide = Vec::new();
    for index in 0..12u32 {
        wide.push(bone_track(
            "Rig",
            bone_index(index),
            perro_animation::ANIMATION_TRANSFORM_MASK_POSITION
                | perro_animation::ANIMATION_TRANSFORM_MASK_ROTATION,
            transform3d(index as f32, 0.5, -1.0, index as f32 * 0.1, 1.0),
        ));
    }
    wide.push(f32_track(
        "Prop",
        NodeField::Node3D(Node3DField::Visible),
        1.0,
    ));
    let idle = clip_of(wide);
    let run = clip_of(vec![
        bone_track(
            "Rig",
            bone_index(0),
            perro_animation::ANIMATION_TRANSFORM_MASK_POSITION,
            transform3d(-3.0, 0.0, 0.0, 0.0, 1.0),
        ),
        bone_track(
            "Rig",
            bone_name("Head"),
            perro_animation::ANIMATION_TRANSFORM_MASK_ROTATION,
            transform3d(0.0, 0.0, 0.0, 0.8, 1.0),
        ),
        f32_track("Prop", NodeField::Node3D(Node3DField::Visible), 0.0),
    ]);
    let additive = clip_of(vec![
        bone_track(
            "Rig",
            bone_index(3),
            perro_animation::ANIMATION_TRANSFORM_MASK_POSITION,
            transform3d(0.25, 0.25, 0.25, 0.0, 1.0),
        ),
        f32_track("Extra", NodeField::Node3D(Node3DField::Position), 6.0),
    ]);
    // > BINDING_SORT_THRESHOLD entries, plus a duplicate `Rig` binding.
    let wide_bindings = vec![
        binding("Rig", 100),
        binding("Prop", 101),
        binding("Zeta", 102),
        binding("Alpha", 103),
        binding("Rig", 199), // duplicate: `find` keeps node 100
        binding("Mid", 104),
        binding("Beta", 105),
        binding("Omega", 106),
        binding("Kilo", 107),
        binding("Extra", 108),
    ];
    Scenario {
        asset: asset_of(
            &["Idle", "Run", "Additive"],
            vec![
                graph_node(
                    "RunInv",
                    AnimationTreeNodeKind::Invert {
                        input: Cow::Borrowed("Run"),
                        mask: mask_of(&["Rig"], &[], &["0"]),
                    },
                ),
                graph_node(
                    "Move",
                    AnimationTreeNodeKind::Blend {
                        inputs: refs(&["Idle", "RunInv"]),
                        weights: Cow::Owned(vec![1.0, 1.0]),
                        mask: AnimationTreeMask::default(),
                    },
                ),
                graph_node(
                    "Root",
                    AnimationTreeNodeKind::Add {
                        base: Cow::Borrowed("Move"),
                        inputs: refs(&["Additive"]),
                        weights: Cow::Owned(vec![1.0]),
                        mask: mask_of(&[], &[], &["3"]),
                    },
                ),
            ],
            "Root",
        ),
        weights: vec![AnimationTreeRuntimeWeight {
            node: Cow::Borrowed("Move"),
            input: Cow::Borrowed("RunInv"),
            weight: 0.35,
        }],
        slots: vec![
            SlotState {
                name: "Idle".to_string(),
                clip: idle,
                bindings: wide_bindings.clone(),
                frame: 0,
            },
            SlotState {
                name: "Run".to_string(),
                clip: run,
                bindings: wide_bindings.clone(),
                frame: 0,
            },
            SlotState {
                name: "Additive".to_string(),
                clip: additive,
                bindings: wide_bindings,
                frame: 0,
            },
        ],
    }
}

/// 3 slots x 60 Transform3D bone tracks over 20 bound objects, blended then
/// added — the humanoid-ish shape used for the bench numbers.
fn scenario_humanoid() -> Scenario {
    fn humanoid_clip(seed: f32) -> AnimationClip {
        let mut tracks = Vec::with_capacity(60);
        for object in 0..20u32 {
            for bone in 0..3u32 {
                tracks.push(bone_track(
                    &format!("Obj{object:02}"),
                    bone_index(object * 3 + bone),
                    perro_animation::ANIMATION_TRANSFORM_MASK_POSITION
                        | perro_animation::ANIMATION_TRANSFORM_MASK_ROTATION
                        | perro_animation::ANIMATION_TRANSFORM_MASK_SCALE,
                    transform3d(
                        seed + object as f32,
                        bone as f32,
                        seed * 0.5,
                        seed * 0.1 + bone as f32,
                        1.0 + seed * 0.01,
                    ),
                ));
            }
        }
        clip_of(tracks)
    }
    let bindings = (0..20u32)
        .map(|object| binding(&format!("Obj{object:02}"), 500 + object))
        .collect::<Vec<_>>();
    Scenario {
        asset: asset_of(
            &["Idle", "Run", "Additive"],
            vec![
                graph_node(
                    "Move",
                    AnimationTreeNodeKind::Blend {
                        inputs: refs(&["Idle", "Run"]),
                        weights: Cow::Owned(vec![0.4, 0.6]),
                        mask: AnimationTreeMask::default(),
                    },
                ),
                graph_node(
                    "Root",
                    AnimationTreeNodeKind::Add {
                        base: Cow::Borrowed("Move"),
                        inputs: refs(&["Additive"]),
                        weights: Cow::Owned(vec![0.5]),
                        mask: AnimationTreeMask::default(),
                    },
                ),
            ],
            "Root",
        ),
        weights: Vec::new(),
        slots: vec![
            SlotState {
                name: "Idle".to_string(),
                clip: humanoid_clip(1.0),
                bindings: bindings.clone(),
                frame: 0,
            },
            SlotState {
                name: "Run".to_string(),
                clip: humanoid_clip(2.0),
                bindings: bindings.clone(),
                frame: 0,
            },
            SlotState {
                name: "Additive".to_string(),
                clip: humanoid_clip(3.0),
                bindings,
                frame: 0,
            },
        ],
    }
}

// ---------------------------------------------------------------------------
// parity tests
// ---------------------------------------------------------------------------

#[test]
fn blend_with_mask_matches_hashmap_pose_path() {
    assert_parity("blend+mask", &scenario_blend_mask());
}

#[test]
fn add_delta_matches_hashmap_pose_path() {
    assert_parity("add+delta", &scenario_add_delta());
}

#[test]
fn nested_graph_matches_hashmap_pose_path() {
    assert_parity("nested graph", &scenario_nested_graph());
}

#[test]
fn humanoid_tree_matches_hashmap_pose_path() {
    assert_parity("humanoid", &scenario_humanoid());
}

#[test]
fn duplicate_binding_resolves_to_first_entry() {
    // `find` semantics: first binding for an object wins, even after sorting.
    let scenario = scenario_nested_graph();
    let out = run_new(&scenario);
    assert!(
        out.iter().any(|c| c.node == 100),
        "expected first `Rig` binding (node 100) to win"
    );
    assert!(
        !out.iter().any(|c| c.node == 199),
        "duplicate `Rig` binding (node 199) must never resolve"
    );
}

/// Sweeps weights, mask contents + slot frames across the three shapes so the
/// parity claim covers zero/negative/normalizing weight paths and every mask
/// axis, not just the hand-picked values above.
#[test]
fn randomized_sweep_matches_hashmap_pose_path() {
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mask_objects: [&[&str]; 4] = [&[], &["Hips"], &["Rig", "Prop"], &["Missing"]];
    let mask_fields: [&[&str]; 4] = [&[], &["position"], &["POSITION", "rotation"], &["visible"]];
    let mask_bones: [&[&str]; 4] = [&[], &["0"], &["3", "Head"], &["Spine"]];
    let weight_choices = [0.0f32, -1.0, 0.25, 1.0, 4.0, f32::EPSILON * 0.5];

    for case in 0..240u32 {
        let mut scenario = match case % 3 {
            0 => scenario_blend_mask(),
            1 => scenario_add_delta(),
            _ => scenario_nested_graph(),
        };
        let mask = mask_of(
            mask_objects[(next() % 4) as usize],
            mask_fields[(next() % 4) as usize],
            mask_bones[(next() % 4) as usize],
        );
        let mut nodes = scenario.asset.nodes.to_vec();
        for node in nodes.iter_mut() {
            let new_weights = |len: usize, next: &mut dyn FnMut() -> u64| {
                Cow::Owned(
                    (0..len)
                        .map(|_| weight_choices[(next() % weight_choices.len() as u64) as usize])
                        .collect::<Vec<f32>>(),
                )
            };
            match &mut node.kind {
                AnimationTreeNodeKind::Blend {
                    inputs,
                    weights,
                    mask: node_mask,
                } => {
                    *weights = new_weights(inputs.len(), &mut next);
                    *node_mask = mask.clone();
                }
                AnimationTreeNodeKind::Add {
                    inputs,
                    weights,
                    mask: node_mask,
                    ..
                } => {
                    *weights = new_weights(inputs.len(), &mut next);
                    *node_mask = mask.clone();
                }
                AnimationTreeNodeKind::Invert {
                    mask: node_mask, ..
                } => {
                    *node_mask = mask.clone();
                }
            }
        }
        scenario.asset.nodes = Cow::Owned(nodes);
        for slot in scenario.slots.iter_mut() {
            slot.frame = (next() % 8) as u32;
        }
        assert_parity(&format!("random case {case}"), &scenario);
    }
}

#[test]
fn interned_ids_collide_exactly_when_keys_are_equal() {
    EVAL_SCRATCH.with(|cell| {
        let mut scratch = cell.borrow_mut();
        scratch.begin_eval(0);
        let node_a = NodeID::new(7);
        let node_b = NodeID::new(8);
        let hips: Cow<'static, str> = Cow::Borrowed("Hips");
        let head: Cow<'static, str> = Cow::Borrowed("Head");
        let pos = NodeField::Node3D(Node3DField::Position);
        let rot = NodeField::Node3D(Node3DField::Rotation);
        let bone0 = Some(bone_index(0));
        let bone_named = Some(bone_name("0"));

        let base = scratch.intern(node_a, &hips, pos, &None);
        assert_eq!(base, scratch.intern(node_a, &hips, pos, &None), "same key");
        assert_ne!(base, scratch.intern(node_b, &hips, pos, &None), "node");
        assert_ne!(base, scratch.intern(node_a, &head, pos, &None), "object");
        assert_ne!(base, scratch.intern(node_a, &hips, rot, &None), "field");
        assert_ne!(base, scratch.intern(node_a, &hips, pos, &bone0), "bone");
        assert_ne!(
            scratch.intern(node_a, &hips, pos, &bone0),
            scratch.intern(node_a, &hips, pos, &bone_named),
            "index vs name bone selector"
        );
    });
}

#[test]
fn interner_reuses_ids_across_evals_without_recloning() {
    let scenario = scenario_humanoid();
    let _ = run_new(&scenario);
    let interned_after_first = EVAL_SCRATCH.with(|cell| cell.borrow().keys.len());
    for _ in 0..8 {
        let _ = run_new(&scenario);
    }
    let interned_after_many = EVAL_SCRATCH.with(|cell| cell.borrow().keys.len());
    assert_eq!(
        interned_after_first, interned_after_many,
        "steady-state evals must not intern new keys"
    );
}

// ---------------------------------------------------------------------------
// bench / evidence
// ---------------------------------------------------------------------------

fn eval_new_only(scenario: &Scenario) {
    EVAL_SCRATCH.with(|cell| {
        let mut scratch = cell.borrow_mut();
        scratch.begin_eval(scenario.asset.nodes.len());
        let mut sample = |name: &str, scratch: &mut EvalScratch| match scenario
            .slots
            .iter()
            .find(|slot| slot.name == name)
        {
            Some(slot) => sample_clip_pose(&slot.clip, slot.frame, &slot.bindings, scratch),
            None => scratch.take_pose(),
        };
        let pose = eval_node(
            &scenario.asset,
            &scenario.weights,
            scenario.asset.output.as_ref(),
            &mut scratch,
            &mut sample,
        )
        .unwrap_or_default();
        std::hint::black_box(pose.len());
        scratch.give_pose(pose);
    });
}

fn eval_reference_only(scenario: &Scenario) {
    let mut visiting = vec![false; scenario.asset.nodes.len()];
    let mut sample = |name: &str| match scenario.slots.iter().find(|slot| slot.name == name) {
        Some(slot) => reference::sample_clip_pose(&slot.clip, slot.frame, &slot.bindings),
        None => reference::RefPose::default(),
    };
    let pose = reference::eval_node(
        &scenario.asset,
        &scenario.weights,
        scenario.asset.output.as_ref(),
        &mut visiting,
        &mut sample,
    )
    .unwrap_or_default();
    std::hint::black_box(pose.tracks.len());
}

#[test]
fn interned_pose_path_allocates_far_less() {
    let scenario = scenario_humanoid();
    // Warm both paths (interner fill + pose pool fill are one-time costs).
    for _ in 0..4 {
        eval_new_only(&scenario);
        eval_reference_only(&scenario);
    }
    const EVALS: u64 = 100;
    let new_allocs = count_allocs(|| {
        for _ in 0..EVALS {
            eval_new_only(&scenario);
        }
    });
    let reference_allocs = count_allocs(|| {
        for _ in 0..EVALS {
            eval_reference_only(&scenario);
        }
    });
    println!(
        "alloc/eval  interned={:.1}  hashmap={:.1}  ratio={:.2}x",
        new_allocs as f64 / EVALS as f64,
        reference_allocs as f64 / EVALS as f64,
        reference_allocs as f64 / new_allocs.max(1) as f64
    );
    assert!(
        new_allocs * 4 < reference_allocs,
        "expected >4x fewer allocs: interned={new_allocs} hashmap={reference_allocs}"
    );
}

/// `cargo test -p perro_internal_updates -- --ignored --nocapture animation_tree`
#[test]
#[ignore = "bench-style timing test; run with --ignored --nocapture"]
fn bench_eval_tree_pose() {
    let scenario = scenario_humanoid();
    const EVALS: u32 = 20_000;
    for _ in 0..1_000 {
        eval_new_only(&scenario);
        eval_reference_only(&scenario);
    }
    let start = std::time::Instant::now();
    for _ in 0..EVALS {
        eval_reference_only(&scenario);
    }
    let reference = start.elapsed();
    let start = std::time::Instant::now();
    for _ in 0..EVALS {
        eval_new_only(&scenario);
    }
    let interned = start.elapsed();
    println!(
        "eval 180 tracks / 3 slots: hashmap={:.2}us  interned={:.2}us  speedup={:.2}x",
        reference.as_secs_f64() * 1e6 / EVALS as f64,
        interned.as_secs_f64() * 1e6 / EVALS as f64,
        reference.as_secs_f64() / interned.as_secs_f64()
    );
}

// ---------------------------------------------------------------------------
// pre-existing unit tests
// ---------------------------------------------------------------------------

fn event(frame: u32) -> AnimationFrameEvent {
    AnimationFrameEvent {
        frame,
        scope: AnimationEventScope::Global,
        event: AnimationEvent::EmitSignal {
            name: Cow::Borrowed("test"),
            params: Cow::Borrowed(&[]),
        },
    }
}

#[test]
fn paused_slot_queues_current_event_once() {
    let animation = AnimationID::new(1);
    let events = [event(2)];
    let mut slot = AnimationTreeSlotPlayback {
        current_frame: 2,
        last_event_frame: u32::MAX,
        ..Default::default()
    };

    queue_current_slot_event_once(&mut slot, animation, &events);
    assert_eq!(slot.pending_event_frames, [2]);
    slot.pending_event_frames.clear();
    queue_current_slot_event_once(&mut slot, animation, &events);

    assert!(slot.pending_event_frames.is_empty());
}

#[test]
fn changed_current_frame_queues_event_once() {
    let animation = AnimationID::new(1);
    let events = [event(1), event(3)];
    let mut slot = AnimationTreeSlotPlayback {
        current_frame: 1,
        last_event_frame: u32::MAX,
        ..Default::default()
    };
    queue_current_slot_event_once(&mut slot, animation, &events);
    slot.pending_event_frames.clear();

    slot.current_frame = 3;
    queue_current_slot_event_once(&mut slot, animation, &events);

    assert_eq!(slot.pending_event_frames, [3]);
}

#[test]
fn bone_index_str_eq_matches_to_string_exactly() {
    for index in [0u32, 7, 10, 99, 1234, u32::MAX] {
        assert!(bone_index_str_eq(&index.to_string(), index));
    }
    // Same forms the old `to_string()` compare rejected.
    assert!(!bone_index_str_eq("07", 7));
    assert!(!bone_index_str_eq(" 7", 7));
    assert!(!bone_index_str_eq("", 0));
    assert!(!bone_index_str_eq("7x", 7));
    assert!(!bone_index_str_eq("8", 7));
}

#[test]
fn transform2d_blend_uses_short_rotation_arc_and_scale() {
    let mut out = Transform2D::new(Vector2::ZERO, 350.0_f32.to_radians(), Vector2::ONE);
    let next = Transform2D::new(Vector2::ZERO, 10.0_f32.to_radians(), Vector2::new(3.0, 5.0));
    let mut weights = BlendWeights {
        rotation: 1.0,
        scale: 1.0,
        ..Default::default()
    };

    blend_transform2d_channel(
        &mut out,
        &next,
        perro_animation::ANIMATION_TRANSFORM_MASK_ROTATION
            | perro_animation::ANIMATION_TRANSFORM_MASK_SCALE,
        &mut weights,
        1.0,
    );

    assert!(out.rotation.sin().abs() < 1e-5);
    assert_eq!(out.scale, Vector2::new(2.0, 3.0));
}

#[test]
fn transform3d_mask_does_not_dilute_only_authored_rotation() {
    let mut out = Transform3D::IDENTITY;
    let next_rotation = Quaternion::from_euler_xyz(0.0, 0.0, std::f32::consts::FRAC_PI_2);
    let next = Transform3D::new(Vector3::ZERO, next_rotation, Vector3::ONE);
    let mut weights = BlendWeights::default();

    blend_transform3d_channel(
        &mut out,
        &next,
        perro_animation::ANIMATION_TRANSFORM_MASK_ROTATION,
        &mut weights,
        1.0,
    );

    assert!(out.rotation.dot(next_rotation).abs() > 0.9999);
}
