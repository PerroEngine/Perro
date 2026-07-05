use super::*;

// Pan is a direction cue, not a distance cue: rodio's ears sit at x = ±1, so an
// emitter must live near the unit sphere to produce audible channel separation.
// Dividing by `range` (the old mapping) collapsed everything to near-center.
const PAN_RADIUS: f32 = 0.85;
// Fade pan toward center for very close sounds so walking past an emitter
// sweeps smoothly instead of hard-flipping left/right.
const PAN_NEAR_FADE: f32 = 0.5;

pub(super) fn spatial_pan(local: [f32; 3]) -> [f32; 3] {
    let dist = (local[0] * local[0] + local[1] * local[1] + local[2] * local[2]).sqrt();
    if dist <= 0.0001 {
        return [0.0, 0.0, 0.0];
    }
    let scale = PAN_RADIUS * (dist / (dist + PAN_NEAR_FADE)) / dist;
    [local[0] * scale, local[1] * scale, local[2] * scale]
}

// Squared linear falloff: audibly louder up close, still exactly 0 at range.
pub(super) fn distance_attenuation(distance: f32, range: f32) -> f32 {
    let linear = 1.0 - (distance / range.max(0.0001)).clamp(0.0, 1.0);
    linear * linear
}

pub(super) fn rotate_vec2(v: Vector2, radians: f32) -> Vector2 {
    let (sin, cos) = radians.sin_cos();
    Vector2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}

pub(super) fn normalize_spatial_options(mut options: SpatialAudioOptions) -> SpatialAudioOptions {
    options.range = options.range.max(0.0001);
    options.direction_2d = normalize_direction_2d(options.direction_2d);
    options.direction_3d = normalize_direction_3d(options.direction_3d);
    options
}

fn normalize_direction_2d(direction: AudioDirection<Vector2>) -> AudioDirection<Vector2> {
    match direction {
        AudioDirection::Omni => AudioDirection::Omni,
        AudioDirection::Directional(v) => AudioDirection::Directional(normalized_or_zero_2d(v)),
        AudioDirection::InverseDirectional(v) => {
            AudioDirection::InverseDirectional(normalized_or_zero_2d(v))
        }
        AudioDirection::Bidirectional(v) => AudioDirection::Bidirectional(normalized_or_zero_2d(v)),
    }
}

fn normalize_direction_3d(direction: AudioDirection<Vector3>) -> AudioDirection<Vector3> {
    match direction {
        AudioDirection::Omni => AudioDirection::Omni,
        AudioDirection::Directional(v) => AudioDirection::Directional(normalized_or_zero_3d(v)),
        AudioDirection::InverseDirectional(v) => {
            AudioDirection::InverseDirectional(normalized_or_zero_3d(v))
        }
        AudioDirection::Bidirectional(v) => AudioDirection::Bidirectional(normalized_or_zero_3d(v)),
    }
}

fn normalized_or_zero_2d(v: Vector2) -> Vector2 {
    if v.length_squared() > 0.0001 {
        v.normalized()
    } else {
        Vector2::ZERO
    }
}

fn normalized_or_zero_3d(v: Vector3) -> Vector3 {
    if v.length_squared() > 0.0001 {
        v.normalized()
    } else {
        Vector3::ZERO
    }
}

pub(super) fn inverse_transform_point_2d(transform: Transform2D, point: Vector2) -> Vector2 {
    let local = rotate_vec2(point - transform.position, -transform.rotation);
    Vector2::new(
        local.x / transform.scale.x.abs().max(0.0001),
        local.y / transform.scale.y.abs().max(0.0001),
    )
}

pub(super) fn transform_point_2d(transform: Transform2D, point: Vector2) -> Vector2 {
    let scaled = Vector2::new(point.x * transform.scale.x, point.y * transform.scale.y);
    transform.position + rotate_vec2(scaled, transform.rotation)
}

pub(super) fn inverse_transform_dir_2d(transform: Transform2D, dir: Vector2) -> Vector2 {
    let local = rotate_vec2(dir, -transform.rotation);
    Vector2::new(
        local.x / transform.scale.x.abs().max(0.0001),
        local.y / transform.scale.y.abs().max(0.0001),
    )
    .normalized()
}

pub(super) fn transform_dir_2d(transform: Transform2D, dir: Vector2) -> Vector2 {
    let scaled = Vector2::new(dir.x * transform.scale.x, dir.y * transform.scale.y);
    rotate_vec2(scaled, transform.rotation).normalized()
}

pub(super) fn inverse_transform_point_3d(transform: Transform3D, point: Vector3) -> Vector3 {
    let local = transform
        .rotation
        .inverse()
        .rotate_vector3(point - transform.position);
    Vector3::new(
        local.x / transform.scale.x.abs().max(0.0001),
        local.y / transform.scale.y.abs().max(0.0001),
        local.z / transform.scale.z.abs().max(0.0001),
    )
}

pub(super) fn transform_point_3d(transform: Transform3D, point: Vector3) -> Vector3 {
    let scaled = Vector3::new(
        point.x * transform.scale.x,
        point.y * transform.scale.y,
        point.z * transform.scale.z,
    );
    transform.position + transform.rotation.rotate_vector3(scaled)
}

pub(super) fn inverse_transform_dir_3d(transform: Transform3D, dir: Vector3) -> Vector3 {
    let local = transform.rotation.inverse().rotate_vector3(dir);
    Vector3::new(
        local.x / transform.scale.x.abs().max(0.0001),
        local.y / transform.scale.y.abs().max(0.0001),
        local.z / transform.scale.z.abs().max(0.0001),
    )
    .normalized()
}

pub(super) fn transform_dir_3d(transform: Transform3D, dir: Vector3) -> Vector3 {
    let scaled = Vector3::new(
        dir.x * transform.scale.x,
        dir.y * transform.scale.y,
        dir.z * transform.scale.z,
    );
    transform.rotation.rotate_vector3(scaled).normalized()
}

pub(super) fn segment_aabb(
    from: Vector2,
    delta: Vector2,
    center: Vector2,
    half_w: f32,
    half_h: f32,
) -> Option<(f32, Vector2)> {
    let min = Vector2::new(center.x - half_w, center.y - half_h);
    let max = Vector2::new(center.x + half_w, center.y + half_h);
    let mut t_min = 0.0f32;
    let mut t_max = 1.0f32;
    let mut normal = Vector2::ZERO;
    for axis in 0..2 {
        let origin = if axis == 0 { from.x } else { from.y };
        let dir = if axis == 0 { delta.x } else { delta.y };
        let lo = if axis == 0 { min.x } else { min.y };
        let hi = if axis == 0 { max.x } else { max.y };
        if dir.abs() <= 0.000001 {
            if origin < lo || origin > hi {
                return None;
            }
            continue;
        }
        let inv = 1.0 / dir;
        let mut t1 = (lo - origin) * inv;
        let mut t2 = (hi - origin) * inv;
        let mut n = if axis == 0 {
            Vector2::new(-1.0, 0.0)
        } else {
            Vector2::new(0.0, -1.0)
        };
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
            n = -n;
        }
        if t1 > t_min {
            t_min = t1;
            normal = n;
        }
        t_max = t_max.min(t2);
        if t_min > t_max {
            return None;
        }
    }
    (0.0..=1.0).contains(&t_min).then_some((t_min, normal))
}

pub(super) fn reflect_2d(direction: Vector2, normal: Vector2) -> Option<Vector2> {
    let n = normal.normalized();
    if n.length_squared() <= 0.0001 {
        return None;
    }
    let reflected = direction - n * (2.0 * direction.dot(n));
    (reflected.length_squared() > 0.0001).then_some(reflected.normalized())
}

pub(super) fn segment_aabb_3d(
    from: Vector3,
    delta: Vector3,
    center: Vector3,
    half: Vector3,
) -> Option<f32> {
    let min = center - half;
    let max = center + half;
    let mut t_min = 0.0f32;
    let mut t_max = 1.0f32;
    for axis in 0..3 {
        let origin = match axis {
            0 => from.x,
            1 => from.y,
            _ => from.z,
        };
        let dir = match axis {
            0 => delta.x,
            1 => delta.y,
            _ => delta.z,
        };
        let lo = match axis {
            0 => min.x,
            1 => min.y,
            _ => min.z,
        };
        let hi = match axis {
            0 => max.x,
            1 => max.y,
            _ => max.z,
        };
        if dir.abs() <= 0.000001 {
            if origin < lo || origin > hi {
                return None;
            }
            continue;
        }
        let inv = 1.0 / dir;
        let mut t1 = (lo - origin) * inv;
        let mut t2 = (hi - origin) * inv;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        t_min = t_min.max(t1);
        t_max = t_max.min(t2);
        if t_min > t_max {
            return None;
        }
    }
    (0.0..=1.0).contains(&t_min).then_some(t_min)
}

pub(super) fn segment_aabb_3d_with_normal(
    from: Vector3,
    delta: Vector3,
    center: Vector3,
    half: Vector3,
) -> Option<(f32, Vector3)> {
    let min = center - half;
    let max = center + half;
    let mut t_min = 0.0f32;
    let mut t_max = 1.0f32;
    let mut normal = Vector3::new(0.0, 0.0, 0.0);
    for axis in 0..3 {
        let origin = match axis {
            0 => from.x,
            1 => from.y,
            _ => from.z,
        };
        let dir = match axis {
            0 => delta.x,
            1 => delta.y,
            _ => delta.z,
        };
        let lo = match axis {
            0 => min.x,
            1 => min.y,
            _ => min.z,
        };
        let hi = match axis {
            0 => max.x,
            1 => max.y,
            _ => max.z,
        };
        if dir.abs() <= 0.000001 {
            if origin < lo || origin > hi {
                return None;
            }
            continue;
        }
        let inv = 1.0 / dir;
        let mut t1 = (lo - origin) * inv;
        let mut t2 = (hi - origin) * inv;
        let mut axis_normal = match axis {
            0 => Vector3::new(if dir > 0.0 { -1.0 } else { 1.0 }, 0.0, 0.0),
            1 => Vector3::new(0.0, if dir > 0.0 { -1.0 } else { 1.0 }, 0.0),
            _ => Vector3::new(0.0, 0.0, if dir > 0.0 { -1.0 } else { 1.0 }),
        };
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
            axis_normal *= -1.0;
        }
        if t1 > t_min {
            t_min = t1;
            normal = axis_normal;
        }
        t_max = t_max.min(t2);
        if t_min > t_max {
            return None;
        }
    }
    (0.0..=1.0).contains(&t_min).then_some((t_min, normal))
}

pub(super) fn reflect_3d(direction: Vector3, normal: Vector3) -> Option<Vector3> {
    let n = normal.normalized();
    if n.length_squared() <= 0.0001 {
        return None;
    }
    let reflected = direction - n * (2.0 * direction.dot(n));
    (reflected.length_squared() > 0.0001).then_some(reflected.normalized())
}

pub(super) fn inverse_rotate_vec3(rotation: [f32; 4], v: Vector3) -> Vector3 {
    let [x, y, z, w] = normalized_quat(rotation);
    rotate_vec3([-x, -y, -z, w], v)
}

pub(super) fn normalized_quat(rotation: [f32; 4]) -> [f32; 4] {
    let [x, y, z, w] = rotation;
    let len_sq = x * x + y * y + z * z + w * w;
    if len_sq <= 0.000_001 || !len_sq.is_finite() {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let inv = len_sq.sqrt().recip();
    [x * inv, y * inv, z * inv, w * inv]
}

pub(super) fn rotate_vec3(rotation: [f32; 4], v: Vector3) -> Vector3 {
    let [x, y, z, w] = normalized_quat(rotation);
    let qv = Vector3::new(x, y, z);
    let t = qv.cross(v) * 2.0;
    v + t * w + qv.cross(t)
}
