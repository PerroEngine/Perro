// Pan is a direction cue: rodio's ears sit at x = ±1, so the emitter must live
// near the unit sphere for audible channel separation. Near-fade keeps close
// fly-bys from hard-flipping left/right.
const PAN_RADIUS: f32 = 0.85;
const PAN_NEAR_FADE: f32 = 0.15;
// Ears only resolve the horizontal axis; boost x and damp elevation before
// projecting onto the pan sphere so left/right stays legible at oblique angles.
const PAN_LATERAL_BOOST: f32 = 1.9;
const PAN_VERTICAL_SCALE: f32 = 0.35;

pub(crate) fn spatial_pan(local: [f32; 3]) -> [f32; 3] {
    let dist = (local[0] * local[0] + local[1] * local[1] + local[2] * local[2]).sqrt();
    if dist <= 0.0001 {
        return [0.0, 0.0, 0.0];
    }
    let x = local[0] * PAN_LATERAL_BOOST;
    let y = local[1] * PAN_VERTICAL_SCALE;
    let z = local[2];
    let warped = (x * x + y * y + z * z).sqrt();
    if warped <= 0.0001 {
        return [0.0, 0.0, 0.0];
    }
    let scale = PAN_RADIUS * (dist / (dist + PAN_NEAR_FADE)) / warped;
    [x * scale, y * scale, z * scale]
}

// Squared linear falloff: audibly louder up close, exactly 0 at range.
pub(crate) fn distance_attenuation(distance: f32, range: f32) -> f32 {
    let linear = 1.0 - (distance / range.max(0.0001)).clamp(0.0, 1.0);
    linear * linear
}

pub(crate) fn inverse_rotate_vec3(rotation: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let [x, y, z, w] = normalized_quat(rotation);
    rotate_vec3([-x, -y, -z, w], v)
}

fn normalized_quat(rotation: [f32; 4]) -> [f32; 4] {
    let [x, y, z, w] = rotation;
    let len_sq = x * x + y * y + z * z + w * w;
    if !len_sq.is_finite() || len_sq <= 1.0e-6 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let inv = len_sq.sqrt().recip();
    [x * inv, y * inv, z * inv, w * inv]
}

fn rotate_vec3(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let [qx, qy, qz, qw] = q;
    let [vx, vy, vz] = v;
    let tx = 2.0 * (qy * vz - qz * vy);
    let ty = 2.0 * (qz * vx - qx * vz);
    let tz = 2.0 * (qx * vy - qy * vx);
    [
        vx + qw * tx + (qy * tz - qz * ty),
        vy + qw * ty + (qz * tx - qx * tz),
        vz + qw * tz + (qx * ty - qy * tx),
    ]
}
