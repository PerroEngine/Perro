use crate::prelude::*;
use perro_animation::{
    ANIMATION_TRANSFORM_MASK_POSITION, ANIMATION_TRANSFORM_MASK_ROTATION,
    ANIMATION_TRANSFORM_MASK_SCALE, AnimationBoneSelector, AnimationEase, AnimationEvent,
    AnimationEventScope, AnimationInterpolation, AnimationObjectTrack, AnimationParam,
    AnimationTrackValue,
};
use perro_nodes::animation_player::{
    AnimationObjectBinding, AnimationPlaybackType, AppliedAnimationTransform,
    AppliedAnimationTransformKind,
};
use perro_nodes::{
    AmbientLight3D, AnimationPlayer, Camera3D, MeshInstance3D, Node2D, Node3D, PointLight3D,
    RayLight3D, Skeleton2D, Skeleton3D, SpotLight3D, Sprite2D,
};
use perro_runtime_api::perro_structs::{
    Color, Quaternion, Transform2D, Transform3D, Vector2, Vector3,
};
use perro_runtime_api::perro_variant::Variant;
use perro_scene::{
    Camera3DField, Light3DField, MeshInstance3DField, Node2DField, Node3DField, NodeField,
    PointLight3DField, SpotLight3DField, Sprite2DField, resolve_node_field,
};
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

type SelfNodeType = AnimationPlayer;

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
    let Some(animation_id) = with_node!(ctx, SelfNodeType, id, |player| {
        if res.Animations().is_loaded(player.animation) {
            player.animation
        } else if res
            .Animations()
            .is_loaded(player.internal.last_applied_animation)
        {
            player.internal.last_applied_animation
        } else {
            AnimationID::nil()
        }
    }) else {
        return;
    };
    if animation_id.is_nil() {
        return;
    }

    let Some(clip) = res.Animations().get(animation_id) else {
        return;
    };
    let delta_seconds = delta_time!(ctx).max(0.0);
    let Some(step) = step_animation_player(ctx, id, animation_id, &clip, delta_seconds) else {
        return;
    };
    if !step.should_apply && step.event_frames.is_empty() {
        return;
    }

    let mut applied_transforms = Vec::new();
    if step.should_apply {
        let Some(previous_transforms) = with_node_mut!(ctx, SelfNodeType, id, |player| {
            std::mem::take(&mut player.internal.applied_transforms)
        })
        .warn_none_once(format_args!(
            "animation apply skip: node={} expect=AnimationPlayer missing",
            id.as_u64()
        )) else {
            return;
        };
        applied_transforms = previous_transforms;
        apply_clip_frame(
            ctx,
            res,
            &clip,
            step.frame,
            &step.bindings,
            &mut applied_transforms,
        );
    }
    for frame in step.event_frames.iter().copied() {
        apply_frame_events(ctx, &clip, frame, &step.bindings);
    }
    let _ = with_node_mut!(ctx, SelfNodeType, id, |player| {
        if step.should_apply {
            player.internal.applied_transforms = applied_transforms;
        }
        // Hand the scratch bindings buffer back for reuse next frame.
        player.internal.bindings_scratch = step.bindings;
        player.internal.event_frames_scratch = step.event_frames;
    });
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

mod tracks;
pub(super) use tracks::*;
mod playback;
pub(super) use playback::*;

#[cfg(test)]
mod tests {
    use super::*;
    use perro_animation::{AnimationKeyMode, AnimationObjectKey};
    use perro_nodes::Bone3D;
    use std::borrow::Cow;

    fn key(frame: u32, value: f32) -> AnimationObjectKey {
        AnimationObjectKey {
            frame,
            mode: AnimationKeyMode::Closed,
            interpolation: AnimationInterpolation::Linear,
            ease: AnimationEase::Linear,
            value: AnimationTrackValue::F32(value),
        }
    }

    fn track_with_keys(keys: Vec<AnimationObjectKey>) -> AnimationObjectTrack {
        AnimationObjectTrack {
            keys: Cow::Owned(keys),
            ..Default::default()
        }
    }

    fn f32_value(value: Option<AnimationTrackValue>) -> f32 {
        match value {
            Some(AnimationTrackValue::F32(v)) => v,
            other => panic!("expected F32 track value, got {other:?}"),
        }
    }

    #[test]
    fn sample_track_value_empty_track_returns_none() {
        let track = track_with_keys(vec![]);
        assert!(sample_track_value(&track, 0).is_none());
    }

    #[test]
    fn sample_track_value_frame_before_first_key_clamps_to_first() {
        let track = track_with_keys(vec![key(5, 10.0), key(10, 20.0)]);
        assert_eq!(f32_value(sample_track_value(&track, 0)), 10.0);
        assert_eq!(f32_value(sample_track_value(&track, 4)), 10.0);
    }

    #[test]
    fn sample_track_value_frame_after_last_key_clamps_to_last() {
        let track = track_with_keys(vec![key(5, 10.0), key(10, 20.0)]);
        assert_eq!(f32_value(sample_track_value(&track, 10)), 20.0);
        assert_eq!(f32_value(sample_track_value(&track, 100)), 20.0);
    }

    #[test]
    fn sample_track_value_exactly_on_key_returns_key_value() {
        let track = track_with_keys(vec![key(0, 1.0), key(5, 2.0), key(10, 3.0)]);
        assert_eq!(f32_value(sample_track_value(&track, 0)), 1.0);
        assert_eq!(f32_value(sample_track_value(&track, 5)), 2.0);
        assert_eq!(f32_value(sample_track_value(&track, 10)), 3.0);
    }

    #[test]
    fn sample_track_value_between_keys_interpolates_linearly() {
        let track = track_with_keys(vec![key(0, 0.0), key(10, 10.0)]);
        assert_eq!(f32_value(sample_track_value(&track, 5)), 5.0);
        assert_eq!(f32_value(sample_track_value(&track, 2)), 2.0);
        assert_eq!(f32_value(sample_track_value(&track, 8)), 8.0);
    }

    #[test]
    fn sample_track_value_single_key_returns_constant() {
        let track = track_with_keys(vec![key(3, 42.0)]);
        assert_eq!(f32_value(sample_track_value(&track, 0)), 42.0);
        assert_eq!(f32_value(sample_track_value(&track, 3)), 42.0);
        assert_eq!(f32_value(sample_track_value(&track, 999)), 42.0);
    }

    #[test]
    fn sample_track_value_many_keys_matches_linear_scan_reference() {
        // Cross-check the binary-search based lookup against a naive
        // linear scan (the pre-optimization algorithm) across every frame
        // in range, including boundaries, to guard against off-by-one
        // errors in the `partition_point` split.
        fn linear_scan_reference(track: &AnimationObjectTrack, frame: u32) -> Option<f32> {
            let mut prev_index = None::<usize>;
            let mut next_index = None::<usize>;
            for (index, k) in track.keys.iter().enumerate() {
                if k.frame <= frame {
                    prev_index = Some(index);
                } else {
                    next_index = Some(index);
                    break;
                }
            }
            let prev_index = prev_index.or(Some(0))?;
            let prev_key = &track.keys[prev_index];
            let AnimationTrackValue::F32(prev) = prev_key.value else {
                unreachable!()
            };
            match prev_key.interpolation {
                AnimationInterpolation::Step => Some(prev),
                AnimationInterpolation::Linear => {
                    let Some(next_index) = next_index else {
                        return Some(prev);
                    };
                    let next_key = &track.keys[next_index];
                    let AnimationTrackValue::F32(next) = next_key.value else {
                        unreachable!()
                    };
                    let frame_span = next_key.frame.saturating_sub(prev_key.frame);
                    if frame_span == 0 {
                        return Some(prev);
                    }
                    let local = frame.saturating_sub(prev_key.frame);
                    let t = (local as f32 / frame_span as f32).clamp(0.0, 1.0);
                    Some(prev + (next - prev) * t)
                }
            }
        }

        let keys = vec![
            key(0, 0.0),
            key(3, 30.0),
            key(3, 33.0),
            key(7, 70.0),
            key(20, 200.0),
        ];
        let track = track_with_keys(keys);

        for frame in 0..25u32 {
            let expected = linear_scan_reference(&track, frame);
            let actual = f32_value(sample_track_value(&track, frame));
            assert_eq!(
                Some(actual),
                expected,
                "mismatch at frame {frame}: binary search vs linear scan"
            );
        }
    }

    fn signal_event(frame: u32) -> perro_animation::AnimationFrameEvent {
        perro_animation::AnimationFrameEvent {
            frame,
            scope: AnimationEventScope::Global,
            event: AnimationEvent::EmitSignal {
                name: Cow::Borrowed("noop"),
                params: Cow::Owned(vec![]),
            },
        }
    }

    #[test]
    fn frame_has_event_empty_list_is_false() {
        assert!(!frame_has_event(&[], 0));
    }

    #[test]
    fn frame_has_event_true_only_on_matching_frame() {
        let events = vec![signal_event(2), signal_event(7)];
        assert!(frame_has_event(&events, 2));
        assert!(frame_has_event(&events, 7));
        assert!(!frame_has_event(&events, 5));
        assert!(!frame_has_event(&events, 0));
    }

    #[test]
    fn frame_has_event_handles_loop_wraparound_frame_zero() {
        // Loop playback can land back on frame 0 after wrapping; an event
        // authored at frame 0 must still be detected on every pass.
        let events = vec![signal_event(0), signal_event(15)];
        assert!(frame_has_event(&events, 0));
        assert!(frame_has_event(&events, 15));
        assert!(!frame_has_event(&events, 1));
    }

    #[test]
    fn loop_large_step_lists_crossed_event_frames_in_order() {
        let events = (0..5).map(signal_event).collect::<Vec<_>>();
        let mut crossed = Vec::new();

        crossed_animation_frames(
            0.25,
            8.0,
            5,
            AnimationPlaybackType::Loop,
            1.0,
            &events,
            &mut crossed,
        );

        assert_eq!(crossed, [1, 2, 3, 4, 0, 1, 2, 3]);
    }

    #[test]
    fn reverse_loop_lists_crossed_event_frames_in_order() {
        let events = (0..5).map(signal_event).collect::<Vec<_>>();
        let mut crossed = Vec::new();

        crossed_animation_frames(
            3.75,
            -5.0,
            5,
            AnimationPlaybackType::Loop,
            1.0,
            &events,
            &mut crossed,
        );

        assert_eq!(crossed, [3, 2, 1, 0, 4]);
    }

    #[test]
    fn boomerang_large_step_lists_turnaround_events_once() {
        let events = (0..4).map(signal_event).collect::<Vec<_>>();
        let mut crossed = Vec::new();

        crossed_animation_frames(
            0.0,
            8.0,
            4,
            AnimationPlaybackType::Boomerang,
            1.0,
            &events,
            &mut crossed,
        );

        assert_eq!(crossed, [1, 2, 3, 2, 1, 0, 1, 2]);
    }

    #[test]
    fn binding_fingerprint_detects_public_vec_mutation() {
        let mut bindings = vec![AnimationObjectBinding {
            object: Cow::Borrowed("body"),
            node: NodeID::new(1),
        }];
        let before = bindings_fingerprint(&bindings);

        bindings[0].node = NodeID::new(2);

        assert_ne!(before, bindings_fingerprint(&bindings));
    }

    #[test]
    fn boomerang_keeps_moving_after_turnaround() {
        let mut dir = 1.0_f32;
        let frame_count = 6_u32; // 0..5
        let mut frame = 0.0_f32;
        let mut sampled = Vec::new();

        for _ in 0..20 {
            frame = advance_playback_frame(
                frame,
                1.0,
                frame_count,
                AnimationPlaybackType::Boomerang,
                &mut dir,
            );
            sampled.push(playback_frame_to_frame(
                frame,
                frame_count,
                AnimationPlaybackType::Boomerang,
            ));
        }

        assert_eq!(
            sampled,
            vec![1, 2, 3, 4, 5, 4, 3, 2, 1, 0, 1, 2, 3, 4, 5, 4, 3, 2, 1, 0]
        );
    }

    #[test]
    fn boomerang_large_step_matches_small_steps() {
        let mut large_dir = 1.0;
        let large = advance_boomerang_frame(0.0, 25.0, 6, &mut large_dir);

        let mut small_dir = 1.0;
        let mut small = 0.0;
        for _ in 0..25 {
            small = advance_boomerang_frame(small, 1.0, 6, &mut small_dir);
        }

        assert_eq!(large, small);
        assert_eq!(large_dir, small_dir);
        assert!((0.0..=5.0).contains(&large));
    }

    #[test]
    fn boomerang_rejects_non_finite_state() {
        let mut dir = -1.0;
        let frame = advance_boomerang_frame(f32::INFINITY, 1.0, 6, &mut dir);
        assert_eq!(frame, 0.0);
        assert_eq!(dir, 1.0);

        let frame = advance_boomerang_frame(3.0, f32::INFINITY, 6, &mut dir);
        assert_eq!(frame, 3.0);
        assert_eq!(dir, 1.0);
    }

    #[test]
    fn bone_track_writes_pose_not_rest() {
        let rest = Transform3D::new(
            Vector3::new(1.0, 2.0, 3.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        );
        let pose = Transform3D::new(
            Vector3::new(4.0, 5.0, 6.0),
            Quaternion::new(0.0, 0.0, 0.70710677, 0.70710677),
            Vector3::ONE,
        );
        let mut skeleton = Skeleton3D::default();
        skeleton.bones.push(Bone3D {
            rest,
            pose: rest,
            ..Bone3D::new()
        });
        let target = perro_animation::AnimationBoneTarget {
            selector: AnimationBoneSelector::Index(0),
        };

        assert!(apply_bone_pose_3d(
            &mut skeleton,
            &target,
            pose,
            ANIMATION_TRANSFORM_MASK_POSITION
                | ANIMATION_TRANSFORM_MASK_ROTATION
                | ANIMATION_TRANSFORM_MASK_SCALE
        ));
        assert_eq!(skeleton.bones[0].rest, rest);
        assert_eq!(skeleton.bones[0].pose.position, pose.position);
        assert_eq!(skeleton.bones[0].pose.scale, pose.scale);
        assert_eq!(
            skeleton.bones[0].pose.rotation,
            (rest.rotation * pose.rotation).normalized()
        );
    }

    #[test]
    fn bone_track_uses_rest_for_unauthored_channels_and_delta_rotation() {
        let rest = Transform3D::new(
            Vector3::new(1.0, 2.0, 3.0),
            Quaternion::new(0.0, 0.0, 0.38268343, 0.9238795),
            Vector3::new(2.0, 2.0, 2.0),
        );
        let delta = Quaternion::new(0.0, 0.0, 0.300706, 0.953717);
        let pose = Transform3D::new(Vector3::ZERO, delta, Vector3::ZERO);
        let mut skeleton = Skeleton3D::default();
        skeleton.bones.push(Bone3D {
            rest,
            pose: rest,
            ..Bone3D::new()
        });
        let target = perro_animation::AnimationBoneTarget {
            selector: AnimationBoneSelector::Index(0),
        };

        assert!(apply_bone_pose_3d(
            &mut skeleton,
            &target,
            pose,
            ANIMATION_TRANSFORM_MASK_ROTATION
        ));

        assert_eq!(skeleton.bones[0].pose.position, rest.position);
        assert_eq!(skeleton.bones[0].pose.scale, rest.scale);
        assert_eq!(
            skeleton.bones[0].pose.rotation,
            (rest.rotation * delta).normalized()
        );
    }

    #[test]
    fn node3d_animation_position_applies_as_local_offset() {
        let current = Transform3D::new(
            Vector3::new(0.0, 1.0, 0.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        );
        let previous = Transform3D::IDENTITY;
        let next = Transform3D::new(
            Vector3::new(1.0, 1.0, 1.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        );

        let out =
            apply_transform_offset_3d(current, previous, next, ANIMATION_TRANSFORM_MASK_POSITION);

        assert_eq!(out.position, Vector3::new(1.0, 2.0, 1.0));
    }

    #[test]
    fn node3d_animation_position_replaces_previous_offset_not_base() {
        let current = Transform3D::new(
            Vector3::new(1.0, 2.0, 1.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        );
        let previous = Transform3D::new(
            Vector3::new(1.0, 1.0, 1.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        );
        let next = Transform3D::new(
            Vector3::new(2.0, 0.0, 0.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        );

        let out =
            apply_transform_offset_3d(current, previous, next, ANIMATION_TRANSFORM_MASK_POSITION);

        assert_eq!(out.position, Vector3::new(2.0, 1.0, 0.0));
    }
}
