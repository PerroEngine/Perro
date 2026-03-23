use crate::prelude::*;
use perro_animation::{
    AnimationChannel, AnimationInterpolation, AnimationObjectTrack, AnimationTrackValue,
    Camera3DChannel, Light3DChannel, Node2DChannel, Node3DChannel, PointLight3DChannel,
    SpotLight3DChannel,
};
use perro_nodes::animation_player::{AnimationObjectBinding, AnimationPlaybackType};
use perro_nodes::{
    AmbientLight3D, AnimationPlayer, Camera3D, Node2D, Node3D, PointLight3D, RayLight3D,
    SpotLight3D,
};
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
    if clip.object_tracks.is_empty() {
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
    bindings: Vec<AnimationObjectBinding>,
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
            let duration = clip.duration_seconds();
            player.current_time = advance_time(
                player.current_time,
                delta_seconds * player.speed,
                duration,
                player.playback_type,
            );
            let frame_count = clip.frame_count();
            player.current_frame = time_to_frame(
                player.current_time,
                clip.fps,
                frame_count,
                player.playback_type,
            );
        } else {
            let frame_count = clip.frame_count();
            player.current_frame =
                clamp_frame(player.current_frame, frame_count, player.playback_type);
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
    bindings: &[AnimationObjectBinding],
) where
    RT: RuntimeAPI + ?Sized,
{
    for track in clip.object_tracks.iter() {
        for binding in bindings
            .iter()
            .filter(|b| b.object.as_ref() == track.object.as_ref())
        {
            apply_track(ctx, binding.node, track, frame);
        }
    }
}

fn apply_track<RT>(
    ctx: &mut RuntimeContext<'_, RT>,
    node_id: NodeID,
    track: &AnimationObjectTrack,
    frame: u32,
) where
    RT: RuntimeAPI + ?Sized,
{
    let Some(value) = sample_track_value(track, frame) else {
        return;
    };

    match &track.channel {
        AnimationChannel::Node2D(channel) => match channel {
            Node2DChannel::Transform => {
                if let AnimationTrackValue::Transform2D(value) = value {
                    let _ = with_base_node_mut!(ctx, Node2D, node_id, |node| {
                        node.transform = *value;
                    });
                }
            }
            Node2DChannel::Visible => {
                if let AnimationTrackValue::Bool(value) = value {
                    let _ = with_base_node_mut!(ctx, Node2D, node_id, |node| {
                        node.visible = *value;
                    });
                }
            }
        },
        AnimationChannel::Node3D(channel) => match channel {
            Node3DChannel::Transform => {
                if let AnimationTrackValue::Transform3D(value) = value {
                    let _ = with_base_node_mut!(ctx, Node3D, node_id, |node| {
                        node.transform = *value;
                    });
                }
            }
            Node3DChannel::Visible => {
                if let AnimationTrackValue::Bool(value) = value {
                    let _ = with_base_node_mut!(ctx, Node3D, node_id, |node| {
                        node.visible = *value;
                    });
                }
            }
        },
        AnimationChannel::Camera3D(channel) => {
            if let Some(v) = as_f32_track(value) {
                let _ = with_base_node_mut!(ctx, Camera3D, node_id, |camera| match channel {
                    Camera3DChannel::Zoom => apply_camera_zoom(camera, v),
                    Camera3DChannel::PerspectiveFovYDegrees => {
                        if let perro_nodes::CameraProjection::Perspective {
                            fov_y_degrees, ..
                        } = &mut camera.projection
                        {
                            *fov_y_degrees = v.clamp(10.0, 120.0);
                        }
                    }
                    Camera3DChannel::PerspectiveNear => {
                        if let perro_nodes::CameraProjection::Perspective { near, far, .. } =
                            &mut camera.projection
                        {
                            *near = v.max(0.001);
                            if *far <= *near {
                                *far = *near + 0.001;
                            }
                        }
                    }
                    Camera3DChannel::PerspectiveFar => {
                        if let perro_nodes::CameraProjection::Perspective { near, far, .. } =
                            &mut camera.projection
                        {
                            *far = v.max(*near + 0.001);
                        }
                    }
                    Camera3DChannel::OrthographicSize => {
                        if let perro_nodes::CameraProjection::Orthographic { size, .. } =
                            &mut camera.projection
                        {
                            *size = v.abs().max(0.001);
                        }
                    }
                    Camera3DChannel::OrthographicNear => {
                        if let perro_nodes::CameraProjection::Orthographic { near, far, .. } =
                            &mut camera.projection
                        {
                            *near = v.max(0.001);
                            if *far <= *near {
                                *far = *near + 0.001;
                            }
                        }
                    }
                    Camera3DChannel::OrthographicFar => {
                        if let perro_nodes::CameraProjection::Orthographic { near, far, .. } =
                            &mut camera.projection
                        {
                            *far = v.max(*near + 0.001);
                        }
                    }
                    Camera3DChannel::FrustumLeft => {
                        if let perro_nodes::CameraProjection::Frustum { left, right, .. } =
                            &mut camera.projection
                        {
                            *left = v;
                            if *right <= *left {
                                *right = *left + 0.001;
                            }
                        }
                    }
                    Camera3DChannel::FrustumRight => {
                        if let perro_nodes::CameraProjection::Frustum { left, right, .. } =
                            &mut camera.projection
                        {
                            *right = v.max(*left + 0.001);
                        }
                    }
                    Camera3DChannel::FrustumBottom => {
                        if let perro_nodes::CameraProjection::Frustum { bottom, top, .. } =
                            &mut camera.projection
                        {
                            *bottom = v;
                            if *top <= *bottom {
                                *top = *bottom + 0.001;
                            }
                        }
                    }
                    Camera3DChannel::FrustumTop => {
                        if let perro_nodes::CameraProjection::Frustum { bottom, top, .. } =
                            &mut camera.projection
                        {
                            *top = v.max(*bottom + 0.001);
                        }
                    }
                    Camera3DChannel::FrustumNear => {
                        if let perro_nodes::CameraProjection::Frustum { near, far, .. } =
                            &mut camera.projection
                        {
                            *near = v.max(0.001);
                            if *far <= *near {
                                *far = *near + 0.001;
                            }
                        }
                    }
                    Camera3DChannel::FrustumFar => {
                        if let perro_nodes::CameraProjection::Frustum { near, far, .. } =
                            &mut camera.projection
                        {
                            *far = v.max(*near + 0.001);
                        }
                    }
                    Camera3DChannel::Active => {}
                });
            }
            if matches!(channel, Camera3DChannel::Active)
                && let AnimationTrackValue::Bool(active) = value
            {
                let _ = with_base_node_mut!(ctx, Camera3D, node_id, |camera| {
                    camera.active = *active;
                });
            }
        }
        AnimationChannel::Light3D(channel) => match channel {
            Light3DChannel::Color => {
                if let AnimationTrackValue::Vec3(color) = value {
                    let c = *color;
                    if with_base_node_mut!(ctx, RayLight3D, node_id, |n| n.color = c).is_none() {
                        if with_base_node_mut!(ctx, PointLight3D, node_id, |n| n.color = c)
                            .is_none()
                        {
                            if with_base_node_mut!(ctx, SpotLight3D, node_id, |n| n.color = c)
                                .is_none()
                            {
                                let _ = with_base_node_mut!(ctx, AmbientLight3D, node_id, |n| n
                                    .color =
                                    c);
                            }
                        }
                    }
                }
            }
            Light3DChannel::Intensity => {
                if let Some(v) = as_f32_track(value) {
                    if with_base_node_mut!(ctx, RayLight3D, node_id, |n| n.intensity = v).is_none()
                    {
                        if with_base_node_mut!(ctx, PointLight3D, node_id, |n| n.intensity = v)
                            .is_none()
                        {
                            if with_base_node_mut!(ctx, SpotLight3D, node_id, |n| n.intensity = v)
                                .is_none()
                            {
                                let _ = with_base_node_mut!(ctx, AmbientLight3D, node_id, |n| {
                                    n.intensity = v
                                });
                            }
                        }
                    }
                }
            }
            Light3DChannel::Active => {
                if let AnimationTrackValue::Bool(v) = value {
                    if with_base_node_mut!(ctx, RayLight3D, node_id, |n| n.active = *v).is_none() {
                        if with_base_node_mut!(ctx, PointLight3D, node_id, |n| n.active = *v)
                            .is_none()
                        {
                            if with_base_node_mut!(ctx, SpotLight3D, node_id, |n| n.active = *v)
                                .is_none()
                            {
                                let _ = with_base_node_mut!(ctx, AmbientLight3D, node_id, |n| {
                                    n.active = *v
                                });
                            }
                        }
                    }
                }
            }
        },
        AnimationChannel::PointLight3D(channel) => match channel {
            PointLight3DChannel::Range => {
                if let Some(v) = as_f32_track(value) {
                    let _ = with_base_node_mut!(ctx, PointLight3D, node_id, |node| {
                        node.range = v;
                    });
                }
            }
        },
        AnimationChannel::SpotLight3D(channel) => {
            if let Some(v) = as_f32_track(value) {
                let _ = with_base_node_mut!(ctx, SpotLight3D, node_id, |node| match channel {
                    SpotLight3DChannel::Range => node.range = v,
                    SpotLight3DChannel::InnerAngleRadians => node.inner_angle_radians = v,
                    SpotLight3DChannel::OuterAngleRadians => node.outer_angle_radians = v,
                });
            }
        }
        AnimationChannel::Custom(_) => {}
    }
}

#[inline]
fn as_f32_track(value: &AnimationTrackValue) -> Option<f32> {
    match value {
        AnimationTrackValue::F32(v) => Some(*v),
        AnimationTrackValue::I32(v) => Some(*v as f32),
        AnimationTrackValue::U32(v) => Some(*v as f32),
        _ => None,
    }
}

fn apply_camera_zoom(camera: &mut Camera3D, zoom: f32) {
    let zoom = if zoom.is_finite() && zoom > 0.0 {
        zoom
    } else {
        1.0
    };
    let fov_y_degrees = (60.0 / zoom).clamp(10.0, 120.0);
    if let perro_nodes::CameraProjection::Perspective {
        fov_y_degrees: fov, ..
    } = &mut camera.projection
    {
        *fov = fov_y_degrees;
    }
}

fn sample_track_value(track: &AnimationObjectTrack, frame: u32) -> Option<&AnimationTrackValue> {
    if track.keys.is_empty() {
        return None;
    }

    let mut prev_index = None::<usize>;
    let mut next_index = None::<usize>;
    for (index, key) in track.keys.iter().enumerate() {
        if key.frame <= frame {
            prev_index = Some(index);
        } else {
            next_index = Some(index);
            break;
        }
    }

    let prev_index = prev_index.or(Some(0))?;
    let prev = &track.keys[prev_index].value;

    match track.interpolation {
        AnimationInterpolation::Step => Some(prev),
        AnimationInterpolation::Linear => {
            // Linear interpolation for typed payloads will be added as parser/runtime finalization.
            let _ = next_index;
            Some(prev)
        }
    }
}

fn advance_time(
    current_time: f32,
    dt: f32,
    duration: f32,
    playback_type: AnimationPlaybackType,
) -> f32 {
    let next = current_time + dt;
    normalize_playback_time(next, duration, playback_type)
}

fn time_to_frame(
    time: f32,
    fps: f32,
    frame_count: u32,
    playback_type: AnimationPlaybackType,
) -> u32 {
    if frame_count <= 1 {
        return 0;
    }
    if fps <= 0.0 {
        return clamp_frame(0, frame_count, playback_type);
    }
    let duration = frame_count.saturating_sub(1) as f32 / fps;
    let normalized = normalize_playback_time(time, duration, playback_type);
    let frame = (normalized.max(0.0) * fps).floor() as u32;
    clamp_frame(frame, frame_count, playback_type)
}

fn clamp_frame(frame: u32, frame_count: u32, playback_type: AnimationPlaybackType) -> u32 {
    if frame_count <= 1 {
        return 0;
    }
    let last = frame_count.saturating_sub(1);
    match playback_type {
        AnimationPlaybackType::Once => frame.min(last),
        AnimationPlaybackType::Loop => frame % frame_count,
        AnimationPlaybackType::Boomerang => {
            let period = last.saturating_mul(2);
            if period == 0 {
                return 0;
            }
            let pos = frame % period;
            if pos <= last { pos } else { period - pos }
        }
    }
}

fn normalize_playback_time(time: f32, duration: f32, playback_type: AnimationPlaybackType) -> f32 {
    if duration <= 0.0 {
        return 0.0;
    }
    match playback_type {
        AnimationPlaybackType::Once => time.clamp(0.0, duration),
        AnimationPlaybackType::Loop => time.rem_euclid(duration),
        AnimationPlaybackType::Boomerang => {
            let period = duration * 2.0;
            let wrapped = time.rem_euclid(period);
            if wrapped <= duration {
                wrapped
            } else {
                period - wrapped
            }
        }
    }
}

fn hash_bindings(bindings: &[AnimationObjectBinding]) -> u64 {
    let mut h = 0x9E37_79B9_7F4A_7C15u64 ^ bindings.len() as u64;
    for binding in bindings {
        h ^= binding.node.as_u64();
        h = h.rotate_left(7).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        for b in binding.object.as_bytes() {
            h ^= *b as u64;
            h = h.rotate_left(5).wrapping_mul(0x94D0_49BB_1331_11EB);
        }
    }
    h
}
