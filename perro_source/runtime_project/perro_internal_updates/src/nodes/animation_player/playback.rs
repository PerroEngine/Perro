use super::*;

pub(in super::super) fn sample_track_value(
    track: &AnimationObjectTrack,
    frame: u32,
) -> Option<AnimationTrackValue> {
    if track.keys.is_empty() {
        return None;
    }
    debug_assert!(
        track.keys.windows(2).all(|w| w[0].frame <= w[1].frame),
        "animation track keys must be frame-sorted ascending"
    );

    // Keys are frame-sorted ascending (guaranteed at build time), so the
    // split point between "<= frame" and "> frame" keys can be found with
    // a binary search instead of a linear scan.
    let split = track.keys.partition_point(|key| key.frame <= frame);
    let prev_index = if split == 0 { 0 } else { split - 1 };
    let next_index = if split < track.keys.len() {
        Some(split)
    } else {
        None
    };

    let prev_key = &track.keys[prev_index];
    let prev = &prev_key.value;

    let interpolation = prev_key.interpolation;
    let ease = prev_key.ease;
    match interpolation {
        AnimationInterpolation::Step => Some(prev.clone()),
        AnimationInterpolation::Linear => {
            let Some(next_index) = next_index else {
                return Some(prev.clone());
            };
            let next_key = &track.keys[next_index];
            let frame_span = next_key.frame.saturating_sub(prev_key.frame);
            if frame_span == 0 {
                return Some(prev.clone());
            }

            let local = frame.saturating_sub(prev_key.frame);
            let t = (local as f32 / frame_span as f32).clamp(0.0, 1.0);
            let t = ease_sample(ease, t);
            Some(interpolate_values(prev, &next_key.value, t))
        }
    }
}

#[inline]
pub(in super::super) fn ease_sample(ease: AnimationEase, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match ease {
        AnimationEase::Linear => t,
        AnimationEase::EaseIn => t * t,
        AnimationEase::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
        AnimationEase::EaseInOut => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - ((-2.0 * t + 2.0) * (-2.0 * t + 2.0)) * 0.5
            }
        }
    }
}

pub(in super::super) fn interpolate_values(
    a: &AnimationTrackValue,
    b: &AnimationTrackValue,
    t: f32,
) -> AnimationTrackValue {
    match (a, b) {
        (AnimationTrackValue::F32(a), AnimationTrackValue::F32(b)) => {
            AnimationTrackValue::F32(lerp_f32(*a, *b, t))
        }
        (AnimationTrackValue::I32(a), AnimationTrackValue::I32(b)) => {
            AnimationTrackValue::I32(lerp_f32(*a as f32, *b as f32, t).round() as i32)
        }
        (AnimationTrackValue::U32(a), AnimationTrackValue::U32(b)) => {
            AnimationTrackValue::U32(lerp_f32(*a as f32, *b as f32, t).round().max(0.0) as u32)
        }
        (AnimationTrackValue::Vec2(a), AnimationTrackValue::Vec2(b)) => {
            AnimationTrackValue::Vec2([lerp_f32(a[0], b[0], t), lerp_f32(a[1], b[1], t)])
        }
        (AnimationTrackValue::Vec3(a), AnimationTrackValue::Vec3(b)) => {
            AnimationTrackValue::Vec3([
                lerp_f32(a[0], b[0], t),
                lerp_f32(a[1], b[1], t),
                lerp_f32(a[2], b[2], t),
            ])
        }
        (AnimationTrackValue::Vec4(a), AnimationTrackValue::Vec4(b)) => {
            AnimationTrackValue::Vec4([
                lerp_f32(a[0], b[0], t),
                lerp_f32(a[1], b[1], t),
                lerp_f32(a[2], b[2], t),
                lerp_f32(a[3], b[3], t),
            ])
        }
        (AnimationTrackValue::Transform2D(a), AnimationTrackValue::Transform2D(b)) => {
            let mut out = *a;
            out.position.x = lerp_f32(a.position.x, b.position.x, t);
            out.position.y = lerp_f32(a.position.y, b.position.y, t);
            out.rotation = lerp_f32(a.rotation, b.rotation, t);
            out.scale.x = lerp_f32(a.scale.x, b.scale.x, t);
            out.scale.y = lerp_f32(a.scale.y, b.scale.y, t);
            AnimationTrackValue::Transform2D(out)
        }
        (AnimationTrackValue::Transform3D(a), AnimationTrackValue::Transform3D(b)) => {
            let mut out = *a;
            out.position.x = lerp_f32(a.position.x, b.position.x, t);
            out.position.y = lerp_f32(a.position.y, b.position.y, t);
            out.position.z = lerp_f32(a.position.z, b.position.z, t);
            out.scale.x = lerp_f32(a.scale.x, b.scale.x, t);
            out.scale.y = lerp_f32(a.scale.y, b.scale.y, t);
            out.scale.z = lerp_f32(a.scale.z, b.scale.z, t);
            out.rotation = a.rotation.to_quat().slerp(b.rotation.to_quat(), t).into();
            AnimationTrackValue::Transform3D(out)
        }
        _ => a.clone(),
    }
}

#[inline]
pub(in super::super) fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub(in super::super) fn advance_playback_frame(
    current_frame: f32,
    delta_frames: f32,
    frame_count: u32,
    playback_type: AnimationPlaybackType,
    boomerang_direction: &mut f32,
) -> f32 {
    if frame_count <= 1 {
        *boomerang_direction = 1.0;
        return 0.0;
    }

    match playback_type {
        AnimationPlaybackType::Boomerang => advance_boomerang_frame(
            current_frame,
            delta_frames,
            frame_count,
            boomerang_direction,
        ),
        _ => {
            *boomerang_direction = 1.0;
            let next = current_frame + delta_frames;
            normalize_playback_frame(next, frame_count, playback_type)
        }
    }
}

pub(in super::super) fn crossed_animation_frames(
    current_frame: f32,
    delta_frames: f32,
    frame_count: u32,
    playback_type: AnimationPlaybackType,
    boomerang_direction: f32,
    events: &[perro_animation::AnimationFrameEvent],
    out: &mut Vec<u32>,
) {
    if events.is_empty()
        || frame_count <= 1
        || !current_frame.is_finite()
        || !delta_frames.is_finite()
        || delta_frames == 0.0
    {
        return;
    }

    let last = frame_count.saturating_sub(1) as f64;
    let (start, end, period, boomerang) = match playback_type {
        AnimationPlaybackType::Once => {
            let start = (current_frame as f64).clamp(0.0, last);
            let end = (start + delta_frames as f64).clamp(0.0, last);
            (start, end, 0_i64, false)
        }
        AnimationPlaybackType::Loop => {
            let period = frame_count as f64;
            let start = (current_frame as f64).rem_euclid(period);
            (
                start,
                start + delta_frames as f64,
                frame_count as i64,
                false,
            )
        }
        AnimationPlaybackType::Boomerang => {
            let period = last * 2.0;
            let frame = (current_frame as f64).clamp(0.0, last);
            let start = if boomerang_direction.is_sign_negative() {
                period - frame
            } else {
                frame
            };
            (start, start + delta_frames as f64, period as i64, true)
        }
    };

    let mut push_boundary = |boundary: i64| {
        let frame = if period == 0 {
            boundary.clamp(0, last as i64) as u32
        } else {
            let phase = boundary.rem_euclid(period);
            if boomerang && phase > last as i64 {
                (period - phase) as u32
            } else {
                phase as u32
            }
        };
        if frame_has_event(events, frame) {
            out.push(frame);
        }
    };

    if end > start {
        let mut boundary = start.floor() as i64 + 1;
        while boundary as f64 <= end {
            push_boundary(boundary);
            boundary += 1;
        }
    } else if end < start {
        let mut boundary = start.ceil() as i64 - 1;
        while boundary as f64 >= end {
            push_boundary(boundary);
            boundary -= 1;
        }
    }
}

pub(in super::super) fn advance_boomerang_frame(
    current_frame: f32,
    delta_frames: f32,
    frame_count: u32,
    boomerang_direction: &mut f32,
) -> f32 {
    if frame_count <= 1 {
        *boomerang_direction = 1.0;
        return 0.0;
    }

    let last = frame_count.saturating_sub(1) as f32;
    if !current_frame.is_finite() || !delta_frames.is_finite() {
        *boomerang_direction = 1.0;
        return if current_frame.is_finite() {
            current_frame.clamp(0.0, last)
        } else {
            0.0
        };
    }

    let period = last * 2.0;
    let phase = if boomerang_direction.is_sign_negative() {
        period - current_frame.clamp(0.0, last)
    } else {
        current_frame.clamp(0.0, last)
    };
    let phase = (phase + delta_frames).rem_euclid(period);
    if phase < last {
        *boomerang_direction = 1.0;
        phase
    } else {
        *boomerang_direction = -1.0;
        period - phase
    }
}

pub(in super::super) fn playback_frame_to_frame(
    frame: f32,
    frame_count: u32,
    playback_type: AnimationPlaybackType,
) -> u32 {
    if frame_count <= 1 {
        return 0;
    }
    let normalized = normalize_playback_frame(frame, frame_count, playback_type);
    let discrete = normalized.max(0.0).floor() as u32;
    clamp_frame(discrete, frame_count, playback_type)
}

pub(in super::super) fn clamp_frame(
    frame: u32,
    frame_count: u32,
    playback_type: AnimationPlaybackType,
) -> u32 {
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

pub(in super::super) fn normalize_playback_frame(
    frame: f32,
    frame_count: u32,
    playback_type: AnimationPlaybackType,
) -> f32 {
    if frame_count <= 1 {
        return 0.0;
    }
    let last = frame_count.saturating_sub(1) as f32;
    match playback_type {
        AnimationPlaybackType::Once => frame.clamp(0.0, last),
        AnimationPlaybackType::Loop => frame.rem_euclid(frame_count as f32),
        AnimationPlaybackType::Boomerang => {
            let period = last * 2.0;
            if period <= 0.0 {
                return 0.0;
            }
            let wrapped = frame.rem_euclid(period);
            if wrapped <= last {
                wrapped
            } else {
                period - wrapped
            }
        }
    }
}
