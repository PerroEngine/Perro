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
