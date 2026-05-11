use super::*;

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
