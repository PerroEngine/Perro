struct Camera3D {
    view_proj: mat4x4<f32>,
    inv_view_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera3D;

struct GpuEmitter {
    model_0: vec4<f32>,
    model_1: vec4<f32>,
    model_2: vec4<f32>,
    model_3: vec4<f32>,
    gravity_path: vec4<f32>,      // xyz gravity, w path_kind
    color_start: vec4<f32>,
    color_end: vec4<f32>,
    emissive_point: vec4<f32>,    // xyz emissive, w size
    life_speed: vec4<f32>,        // life_min, life_max, speed_min, speed_max
    size_spread_rate: vec4<f32>,  // size_min, size_max, spread_radians, emission_rate
    time_path: vec4<f32>,         // simulation_time, path_a, path_b, reserved
    counts_seed: vec4<u32>,       // start, count, max_alive_budget, seed
    flags: vec4<u32>,             // looping, prewarm, spin_bits, spawn_origin_base
    custom_ops_xy: vec4<u32>,     // x_off, x_len, y_off, y_len
    custom_ops_zp: vec4<u32>,     // z_off, z_len, params_off, params_len
}

struct ParticleParams {
    emitter_count: u32,
    particle_count: u32,
    _pad0: u32,
    _pad1: u32,
}

@group(1) @binding(0)
var<storage, read> emitters: array<GpuEmitter>;

@group(1) @binding(1)
var<uniform> params: ParticleParams;

@group(1) @binding(2)
var<storage, read> particle_emitters: array<u32>;
@group(1) @binding(3)
var<storage, read> particle_spawn_origins: array<vec4<f32>>;
@group(1) @binding(4)
var<storage, read> particle_spawn_rotations: array<vec4<f32>>;

struct ParticleOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) emissive: vec3<f32>,
}

fn hash01(seed: u32) -> f32 {
    var x = seed * 747796405u + 2891336453u;
    x = (x >> ((x >> 28u) + 4u)) ^ x;
    x = x * 277803737u;
    x = (x >> 22u) ^ x;
    return f32(x) / 4294967295.0;
}

fn safe_normalize(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let len_sq = dot(v, v);
    if len_sq > 1.0e-12 {
        return v * inverseSqrt(len_sq);
    }
    return fallback;
}

@vertex
fn vs_main(@builtin(instance_index) particle_index: u32) -> ParticleOut {
    var out: ParticleOut;
    out.clip_pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    out.color = vec4<f32>(0.0);
    out.emissive = vec3<f32>(0.0);

    if particle_index >= params.particle_count {
        return out;
    }
    let emitter_idx = particle_emitters[particle_index];
    if emitter_idx >= params.emitter_count {
        return out;
    }

    let e = emitters[emitter_idx];
    let local_i = particle_index - e.counts_seed.x;
    let model = mat4x4<f32>(e.model_0, e.model_1, e.model_2, e.model_3);
    let up = safe_normalize((model * vec4<f32>(0.0, 1.0, 0.0, 0.0)).xyz, vec3<f32>(0.0, 1.0, 0.0));
    let time = max(e.time_path.x, 0.0);
    let life_min = max(e.life_speed.x, 0.001);
    let life_max = max(e.life_speed.y, life_min);
    let speed_min = max(e.life_speed.z, 0.0);
    let speed_max = max(e.life_speed.w, speed_min);
    let size_min = max(e.size_spread_rate.x, 0.01);
    let size_max = max(e.size_spread_rate.y, size_min);

    let prewarm_time = select(time, time + life_max, e.flags.x != 0u && e.flags.y != 0u);
    let rate = max(e.size_spread_rate.w, 1.0e-6);
    var total_spawned = u32(floor(prewarm_time * rate));
    if e.flags.x != 0u && e.flags.y != 0u {
        total_spawned = max(total_spawned, e.counts_seed.y - 1u);
    }
    var spawn_index = local_i;
    if e.flags.x != 0u {
        let back = (e.counts_seed.y - 1u) - local_i;
        spawn_index = total_spawned - back;
    }
    let spawn_slot = e.flags.w + (spawn_index % max(e.counts_seed.z, 1u));
    let origin = particle_spawn_origins[spawn_slot].xyz;
    let spawn_rot = particle_spawn_rotations[spawn_slot];
    let h0 = hash01(e.counts_seed.w ^ spawn_index);
    let h1 = hash01((e.counts_seed.w + 0x9E3779B9u) ^ (spawn_index * 3u));
    let h2 = hash01((e.counts_seed.w + 0x7F4A7C15u) ^ (spawn_index * 7u));
    let h3 = hash01((e.counts_seed.w + 0x94D049BBu) ^ (spawn_index * 11u));
    let life = life_min + (life_max - life_min) * h0;
    let spawn_t = f32(spawn_index) / rate;
    let local_t = prewarm_time - spawn_t;
    if local_t < 0.0 || local_t > life {
        return out;
    }

    let age = clamp(local_t / life, 0.0, 1.0);
    let speed = speed_min + (speed_max - speed_min) * h1;
    let spread = e.size_spread_rate.z * (h2 * 2.0 - 1.0);
    let yaw = h0 * 6.28318530718;
    let yaw_sin = sin(yaw);
    let yaw_cos = cos(yaw);
    let spread_sin = sin(spread);
    let spread_cos = cos(spread);
    let dir_y = spread_cos - yaw_cos * spread_sin;
    let dir_z = spread_sin + yaw_cos * spread_cos;
    let dir = safe_normalize(vec3<f32>(yaw_sin, dir_y, dir_z), vec3<f32>(0.0, 1.0, 0.0));
    var pos = origin;

    let path_kind = u32(max(e.gravity_path.w, 0.0));
    let path_a = e.time_path.y;
    let path_b = e.time_path.z;
    if path_kind == 1u {
        pos = pos + dir * speed * local_t;
    } else if path_kind == 2u {
        let theta = local_t * path_a + h0 * 6.28318530718;
        pos = pos + vec3<f32>(cos(theta) * path_b, 0.0, sin(theta) * path_b);
    } else if path_kind == 3u {
        let theta = local_t * path_a + h1 * 6.28318530718;
        pos = origin + vec3<f32>(cos(theta) * path_b, pos.y - origin.y, sin(theta) * path_b);
    } else if path_kind == 4u {
        let n = sin(local_t * path_b + h2 * 37.0);
        let m = cos(local_t * path_b * 1.37 + h1 * 17.0);
        pos = pos + vec3<f32>(n, m, n * m) * abs(path_a);
    } else if path_kind == 5u {
        let seq = (f32(local_i) + 0.5) / max(f32(max(e.counts_seed.y, 1u)), 1.0);
        let theta = f32(local_i) * 2.3999631 + h3 * 0.35;
        let radial = sqrt(seq);
        let r = path_b * radial * age;
        pos = pos + vec3<f32>(cos(theta) * r, 0.0, sin(theta) * r);
    }

    let force = e.gravity_path.xyz;
    pos = pos + 0.5 * force * local_t * local_t;

    let spin = bitcast<f32>(e.flags.z);
    if abs(spin) > 1.0e-6 {
        let rel = pos - origin;
        let theta = spin * local_t;
        let s = sin(theta);
        let c = cos(theta);
        pos = origin + vec3<f32>(rel.x * c - rel.z * s, rel.y, rel.x * s + rel.z * c);
    }
    pos = origin + rotate_by_quat(spawn_rot, pos - origin);

    let color = e.color_start + (e.color_end - e.color_start) * age;
    let size = e.emissive_point.w * (size_min + (size_max - size_min) * h2);

    out.clip_pos = camera.view_proj * vec4<f32>(pos, 1.0);
    out.color = color;
    out.color.a = clamp(color.a, 0.0, 1.0);
    out.emissive = e.emissive_point.xyz;
    return out;
}

fn rotate_by_quat(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

@vertex
fn vs_billboard(
    @builtin(instance_index) particle_index: u32,
    @builtin(vertex_index) vertex_index: u32,
) -> ParticleOut {
    var out: ParticleOut;
    out.clip_pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    out.color = vec4<f32>(0.0);
    out.emissive = vec3<f32>(0.0);

    if particle_index >= params.particle_count {
        return out;
    }
    let emitter_idx = particle_emitters[particle_index];
    if emitter_idx >= params.emitter_count {
        return out;
    }

    let e = emitters[emitter_idx];
    let local_i = particle_index - e.counts_seed.x;
    let model = mat4x4<f32>(e.model_0, e.model_1, e.model_2, e.model_3);
    let time = max(e.time_path.x, 0.0);
    let life_min = max(e.life_speed.x, 0.001);
    let life_max = max(e.life_speed.y, life_min);
    let speed_min = max(e.life_speed.z, 0.0);
    let speed_max = max(e.life_speed.w, speed_min);
    let size_min = max(e.size_spread_rate.x, 0.01);
    let size_max = max(e.size_spread_rate.y, size_min);

    let prewarm_time = select(time, time + life_max, e.flags.x != 0u && e.flags.y != 0u);
    let rate = max(e.size_spread_rate.w, 1.0e-6);
    var total_spawned = u32(floor(prewarm_time * rate));
    if e.flags.x != 0u && e.flags.y != 0u {
        total_spawned = max(total_spawned, e.counts_seed.y - 1u);
    }
    var spawn_index = local_i;
    if e.flags.x != 0u {
        let back = (e.counts_seed.y - 1u) - local_i;
        spawn_index = total_spawned - back;
    }
    let spawn_slot = e.flags.w + (spawn_index % max(e.counts_seed.z, 1u));
    let origin = particle_spawn_origins[spawn_slot].xyz;
    let spawn_rot = particle_spawn_rotations[spawn_slot];
    let h0 = hash01(e.counts_seed.w ^ spawn_index);
    let h1 = hash01((e.counts_seed.w + 0x9E3779B9u) ^ (spawn_index * 3u));
    let h2 = hash01((e.counts_seed.w + 0x7F4A7C15u) ^ (spawn_index * 7u));
    let h3 = hash01((e.counts_seed.w + 0x94D049BBu) ^ (spawn_index * 11u));
    let life = life_min + (life_max - life_min) * h0;
    let spawn_t = f32(spawn_index) / rate;
    let local_t = prewarm_time - spawn_t;
    if local_t < 0.0 || local_t > life {
        return out;
    }

    let age = clamp(local_t / life, 0.0, 1.0);
    let speed = speed_min + (speed_max - speed_min) * h1;
    let spread = e.size_spread_rate.z * (h2 * 2.0 - 1.0);
    let yaw = h0 * 6.28318530718;
    let yaw_sin = sin(yaw);
    let yaw_cos = cos(yaw);
    let spread_sin = sin(spread);
    let spread_cos = cos(spread);
    let dir_y = spread_cos - yaw_cos * spread_sin;
    let dir_z = spread_sin + yaw_cos * spread_cos;
    let dir = safe_normalize(vec3<f32>(yaw_sin, dir_y, dir_z), vec3<f32>(0.0, 1.0, 0.0));
    var pos = origin;

    let path_kind = u32(max(e.gravity_path.w, 0.0));
    let path_a = e.time_path.y;
    let path_b = e.time_path.z;
    if path_kind == 1u {
        pos = pos + dir * speed * local_t;
    } else if path_kind == 2u {
        let theta = local_t * path_a + h0 * 6.28318530718;
        pos = pos + vec3<f32>(cos(theta) * path_b, 0.0, sin(theta) * path_b);
    } else if path_kind == 3u {
        let theta = local_t * path_a + h1 * 6.28318530718;
        pos = origin + vec3<f32>(cos(theta) * path_b, pos.y - origin.y, sin(theta) * path_b);
    } else if path_kind == 4u {
        let n = sin(local_t * path_b + h2 * 37.0);
        let m = cos(local_t * path_b * 1.37 + h1 * 17.0);
        pos = pos + vec3<f32>(n, m, n * m) * abs(path_a);
    } else if path_kind == 5u {
        let seq = (f32(local_i) + 0.5) / max(f32(max(e.counts_seed.y, 1u)), 1.0);
        let theta = f32(local_i) * 2.3999631 + h3 * 0.35;
        let radial = sqrt(seq);
        let r = path_b * radial * age;
        pos = pos + vec3<f32>(cos(theta) * r, 0.0, sin(theta) * r);
    }

    let force = e.gravity_path.xyz;
    pos = pos + 0.5 * force * local_t * local_t;
    let spin = bitcast<f32>(e.flags.z);
    if abs(spin) > 1.0e-6 {
        let rel = pos - origin;
        let theta = spin * local_t;
        let s = sin(theta);
        let c = cos(theta);
        pos = origin + vec3<f32>(rel.x * c - rel.z * s, rel.y, rel.x * s + rel.z * c);
    }
    pos = origin + rotate_by_quat(spawn_rot, pos - origin);

    let color = e.color_start + (e.color_end - e.color_start) * age;
    let size = e.emissive_point.w * (size_min + (size_max - size_min) * h2);
    let center_clip = camera.view_proj * vec4<f32>(pos, 1.0);
    let corners = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, 1.0),
    );
    let half_size = max(size * 0.5, 1.0);
    let ndc_offset = corners[vertex_index] * half_size * camera.inv_view_size * 2.0;
    out.clip_pos = center_clip + vec4<f32>(ndc_offset * center_clip.w, 0.0, 0.0);
    out.color = color;
    out.color.a = clamp(color.a, 0.0, 1.0);
    out.emissive = e.emissive_point.xyz;
    return out;
}

@fragment
fn fs_main(in: ParticleOut) -> @location(0) vec4<f32> {
    let rgb = in.color.rgb + in.emissive;
    return vec4<f32>(rgb, in.color.a);
}


