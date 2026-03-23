use crate::prelude::*;
use perro_animation::{AnimationChannel, AnimationTrack, AnimationTrackValues};
use perro_nodes::{AnimationPlayer, Node2D, Node3D};
use std::sync::Arc;

type SelfNodeType = AnimationPlayer;

pub fn internal_update<RT, R, IP>(
    ctx: &mut RuntimeContext<'_, RT>,
    res: &ResourceContext<'_, R>,
    _ipt: &InputContext<'_, IP>,
    self_id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
    let animation_id = with_node!(ctx, SelfNodeType, self_id, |player| player.animation);
    if animation_id.is_nil() {
        return;
    }

    let Some(clip) = animation_get!(res, animation_id) else {
        return;
    };
    if clip.tracks.is_empty() {
        return;
    }

    let delta_seconds = delta_time!(ctx).max(0.0);
    let Some(step) = step_animation_player(ctx, self_id, &clip, delta_seconds) else {
        return;
    };
    if !step.should_apply {
        return;
    }

    apply_clip_frame(ctx, &clip, step.frame, &step.bindings);
}

pub fn internal_fixed_update<RT, R, IP>(
    _ctx: &mut RuntimeContext<'_, RT>,
    _res: &ResourceContext<'_, R>,
    _ipt: &InputContext<'_, IP>,
    _self_id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
}

struct AnimationStep {
    frame: u32,
    bindings: Vec<perro_animation::AnimationNodeBinding>,
    should_apply: bool,
}

fn step_animation_player<RT>(
    ctx: &mut RuntimeContext<'_, RT>,
    self_id: NodeID,
    clip: &Arc<perro_animation::AnimationClip>,
    delta_seconds: f32,
) -> Option<AnimationStep>
where
    RT: RuntimeAPI + ?Sized,
{
    with_node_mut!(ctx, SelfNodeType, self_id, |player| {
        let previous_frame = player.current_frame;

        if !player.paused {
            player.current_time = advance_time(
                player.current_time,
                delta_seconds * player.speed,
                clip.duration,
                player.looping,
            );
            player.current_frame = time_to_frame(
                player.current_time,
                clip.fps,
                clip.frame_count,
                player.looping,
            );
        } else {
            player.current_frame =
                clamp_frame(player.current_frame, clip.frame_count, player.looping);
        }

        let binding_hash = hash_bindings(&player.bindings);
        let frame_changed = player.current_frame != previous_frame;
        let binding_changed = binding_hash != player.internal.last_binding_hash;
        let animation_changed = player.animation != player.internal.last_applied_animation;
        let frame_unapplied = player.current_frame != player.internal.last_applied_frame;
        let should_apply = animation_changed || frame_changed || binding_changed || frame_unapplied;

        if should_apply {
            player.internal.last_applied_animation = player.animation;
            player.internal.last_applied_frame = player.current_frame;
            player.internal.last_binding_hash = binding_hash;
        }

        AnimationStep {
            frame: player.current_frame,
            bindings: if should_apply {
                player.bindings.to_vec()
            } else {
                Vec::new()
            },
            should_apply,
        }
    })
}

fn apply_clip_frame<RT>(
    ctx: &mut RuntimeContext<'_, RT>,
    clip: &Arc<perro_animation::AnimationClip>,
    frame: u32,
    bindings: &[perro_animation::AnimationNodeBinding],
) where
    RT: RuntimeAPI + ?Sized,
{
    for track in clip.tracks.iter() {
        for binding in bindings
            .iter()
            .filter(|b| b.track.as_ref() == track.key.as_ref())
        {
            apply_track(ctx, binding.node, track, frame);
        }
    }
}

fn apply_track<RT>(
    ctx: &mut RuntimeContext<'_, RT>,
    node_id: NodeID,
    track: &AnimationTrack,
    frame: u32,
) where
    RT: RuntimeAPI + ?Sized,
{
    match track.channel {
        AnimationChannel::Transform2D => {
            if let Some(value) = sample_transform2d(&track.values, frame) {
                let _ = with_base_node_mut!(ctx, Node2D, node_id, |node| {
                    node.transform = value;
                });
            }
        }
        AnimationChannel::Transform3D => {
            if let Some(value) = sample_transform3d(&track.values, frame) {
                let _ = with_base_node_mut!(ctx, Node3D, node_id, |node| {
                    node.transform = value;
                });
            }
        }
        AnimationChannel::NodeVisible => {
            if let Some(value) = sample_bool(&track.values, frame) {
                if with_base_node_mut!(ctx, Node3D, node_id, |node| {
                    node.visible = value;
                })
                .is_none()
                {
                    let _ = with_base_node_mut!(ctx, Node2D, node_id, |node| {
                        node.visible = value;
                    });
                }
            }
        }
        AnimationChannel::Custom(_) => {}
    }
}

fn advance_time(current_time: f32, dt: f32, duration: f32, looping: bool) -> f32 {
    let next = current_time + dt;
    if duration > 0.0 {
        if looping {
            next.rem_euclid(duration)
        } else {
            next.clamp(0.0, duration)
        }
    } else {
        0.0
    }
}

fn time_to_frame(time: f32, fps: f32, frame_count: u32, looping: bool) -> u32 {
    if frame_count <= 1 {
        return 0;
    }
    if fps <= 0.0 {
        return clamp_frame(0, frame_count, looping);
    }
    let frame = (time.max(0.0) * fps).floor() as u32;
    clamp_frame(frame, frame_count, looping)
}

fn clamp_frame(frame: u32, frame_count: u32, looping: bool) -> u32 {
    if frame_count <= 1 {
        return 0;
    }
    if looping {
        frame % frame_count
    } else {
        frame.min(frame_count.saturating_sub(1))
    }
}

fn sample_transform2d(
    values: &AnimationTrackValues,
    frame: u32,
) -> Option<perro_runtime_context::perro_structs::Transform2D> {
    let AnimationTrackValues::Transform2D(values) = values else {
        return None;
    };
    sample(values.as_ref(), frame).copied()
}

fn sample_transform3d(
    values: &AnimationTrackValues,
    frame: u32,
) -> Option<perro_runtime_context::perro_structs::Transform3D> {
    let AnimationTrackValues::Transform3D(values) = values else {
        return None;
    };
    sample(values.as_ref(), frame).copied()
}

fn sample_bool(values: &AnimationTrackValues, frame: u32) -> Option<bool> {
    let AnimationTrackValues::Bool(values) = values else {
        return None;
    };
    sample(values.as_ref(), frame).copied()
}

fn sample<T>(values: &[T], frame: u32) -> Option<&T> {
    if values.is_empty() {
        return None;
    }
    let idx = usize::min(frame as usize, values.len() - 1);
    values.get(idx)
}

fn hash_bindings(bindings: &[perro_animation::AnimationNodeBinding]) -> u64 {
    let mut h = 0x9E37_79B9_7F4A_7C15u64 ^ bindings.len() as u64;
    for binding in bindings {
        h ^= binding.node.as_u64();
        h = h.rotate_left(7).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        for b in binding.track.as_bytes() {
            h ^= *b as u64;
            h = h.rotate_left(5).wrapping_mul(0x94D0_49BB_1331_11EB);
        }
    }
    h
}
