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
    gravity_path: vec4<f32>,
    color_start: vec4<f32>,
    color_end: vec4<f32>,
    emissive_point: vec4<f32>,
    life_speed: vec4<f32>,
    size_spread_rate: vec4<f32>,
    time_path: vec4<f32>,
    counts_seed: vec4<u32>,
    flags: vec4<u32>,
    custom_ops_xy: vec4<u32>,
    custom_ops_zp: vec4<u32>,
}

struct ParticleParams {
    emitter_count: u32,
    particle_count: u32,
    _pad0: u32,
    _pad1: u32,
}

struct ComputedParticle {
    world_pos: vec4<f32>,
    color: vec4<f32>,
    emissive: vec4<f32>,
}

struct ExprOp {
    words: vec4<u32>, // opcode, arg_bits, reserved, reserved
}

@group(1) @binding(0)
var<storage, read> emitters: array<GpuEmitter>;

@group(1) @binding(1)
var<uniform> params: ParticleParams;

@group(1) @binding(2)
var<storage, read_write> particles: array<ComputedParticle>;

@group(1) @binding(3)
var<storage, read> expr_ops: array<ExprOp>;

@group(1) @binding(4)
var<storage, read> custom_params: array<f32>;

@group(1) @binding(5)
var<storage, read> particle_emitters: array<u32>;
@group(1) @binding(6)
var<storage, read> particle_spawn_origins: array<vec4<f32>>;

@group(1) @binding(0)
var<storage, read> particles_read: array<ComputedParticle>;

fn hash01(seed: u32) -> f32 {
    var x = seed * 747796405u + 2891336453u;
    x = (x >> ((x >> 28u) + 4u)) ^ x;
    x = x * 277803737u;
    x = (x >> 22u) ^ x;
    return f32(x) / 4294967295.0;
}

fn hash01f(v: f32) -> f32 {
    let n = sin(v * 12.9898 + 78.233) * 43758.547;
    return n - floor(n);
}

fn safe_normalize(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let len = length(v);
    if len > 1.0e-6 {
        return v / len;
    }
    return fallback;
}

fn eval_expr(
    ops_offset: u32,
    ops_len: u32,
    t: f32,
    life: f32,
    lifetime: f32,
    spawn_time: f32,
    emitter_time: f32,
    speed: f32,
    particle_id: f32,
    dir_x: f32,
    dir_y: f32,
    dir_z: f32,
    vel_x: f32,
    vel_y: f32,
    vel_z: f32,
    rand0: f32,
    rand1: f32,
    rand2: f32,
    seed: f32,
    ring_u: f32,
    index01: f32,
    emitter_x: f32,
    emitter_y: f32,
    emitter_z: f32,
    prev_x: f32,
    prev_y: f32,
    prev_z: f32,
    params_offset: u32,
    params_len: u32,
) -> f32 {
    var stack: array<f32, 64>;
    var sp: u32 = 0u;
    for (var i: u32 = 0u; i < ops_len; i = i + 1u) {
        let op = expr_ops[ops_offset + i].words;
        let code = op.x;
        if code == 0u {
            if sp >= 64u { return 0.0; }
            stack[sp] = bitcast<f32>(op.y);
            sp = sp + 1u;
        } else if code == 1u {
            if sp >= 64u { return 0.0; }
            stack[sp] = t;
            sp = sp + 1u;
        } else if code == 2u {
            if sp >= 64u { return 0.0; }
            stack[sp] = life;
            sp = sp + 1u;
        } else if code == 3u {
            if sp >= 64u { return 0.0; }
            stack[sp] = particle_id;
            sp = sp + 1u;
        } else if code == 4u {
            if sp >= 64u { return 0.0; }
            stack[sp] = rand0;
            sp = sp + 1u;
        } else if code == 5u {
            if sp >= 64u { return 0.0; }
            stack[sp] = rand1;
            sp = sp + 1u;
        } else if code == 6u {
            if sp >= 64u { return 0.0; }
            stack[sp] = rand2;
            sp = sp + 1u;
        } else if code == 7u {
            if sp < 1u { return 0.0; }
            sp = sp - 1u;
            let idx_raw = floor(stack[sp]);
            var value: f32 = 0.0;
            if idx_raw >= 0.0 {
                let idx = u32(idx_raw);
                if idx < params_len {
                    value = custom_params[params_offset + idx];
                }
            }
            stack[sp] = value;
            sp = sp + 1u;
        } else if code == 8u { // add
            if sp < 2u { return 0.0; }
            sp = sp - 1u; let b = stack[sp];
            sp = sp - 1u; let a = stack[sp];
            stack[sp] = a + b; sp = sp + 1u;
        } else if code == 9u { // sub
            if sp < 2u { return 0.0; }
            sp = sp - 1u; let b = stack[sp];
            sp = sp - 1u; let a = stack[sp];
            stack[sp] = a - b; sp = sp + 1u;
        } else if code == 10u { // mul
            if sp < 2u { return 0.0; }
            sp = sp - 1u; let b = stack[sp];
            sp = sp - 1u; let a = stack[sp];
            stack[sp] = a * b; sp = sp + 1u;
        } else if code == 11u { // div
            if sp < 2u { return 0.0; }
            sp = sp - 1u; let b = stack[sp];
            sp = sp - 1u; let a = stack[sp];
            stack[sp] = a / b; sp = sp + 1u;
        } else if code == 12u { // pow
            if sp < 2u { return 0.0; }
            sp = sp - 1u; let b = stack[sp];
            sp = sp - 1u; let a = stack[sp];
            stack[sp] = pow(a, b); sp = sp + 1u;
        } else if code == 13u { // neg
            if sp < 1u { return 0.0; }
            sp = sp - 1u; let a = stack[sp];
            stack[sp] = -a; sp = sp + 1u;
        } else if code == 14u { // sin
            if sp < 1u { return 0.0; }
            sp = sp - 1u; let a = stack[sp];
            stack[sp] = sin(a); sp = sp + 1u;
        } else if code == 15u { // cos
            if sp < 1u { return 0.0; }
            sp = sp - 1u; let a = stack[sp];
            stack[sp] = cos(a); sp = sp + 1u;
        } else if code == 16u { // tan
            if sp < 1u { return 0.0; }
            sp = sp - 1u; let a = stack[sp];
            stack[sp] = tan(a); sp = sp + 1u;
        } else if code == 17u { // abs
            if sp < 1u { return 0.0; }
            sp = sp - 1u; let a = stack[sp];
            stack[sp] = abs(a); sp = sp + 1u;
        } else if code == 18u { // sqrt
            if sp < 1u { return 0.0; }
            sp = sp - 1u; let a = stack[sp];
            stack[sp] = sqrt(max(a, 0.0)); sp = sp + 1u;
        } else if code == 19u { // min
            if sp < 2u { return 0.0; }
            sp = sp - 1u; let b = stack[sp];
            sp = sp - 1u; let a = stack[sp];
            stack[sp] = min(a, b); sp = sp + 1u;
        } else if code == 20u { // max
            if sp < 2u { return 0.0; }
            sp = sp - 1u; let b = stack[sp];
            sp = sp - 1u; let a = stack[sp];
            stack[sp] = max(a, b); sp = sp + 1u;
        } else if code == 21u { // clamp
            if sp < 3u { return 0.0; }
            sp = sp - 1u; let hi = stack[sp];
            sp = sp - 1u; let lo = stack[sp];
            sp = sp - 1u; let x = stack[sp];
            stack[sp] = clamp(x, lo, hi); sp = sp + 1u;
        } else if code == 22u { // speed
            if sp >= 64u { return 0.0; }
            stack[sp] = speed;
            sp = sp + 1u;
        } else if code == 23u { // lifetime
            if sp >= 64u { return 0.0; }
            stack[sp] = lifetime;
            sp = sp + 1u;
        } else if code == 24u { // age_left
            if sp >= 64u { return 0.0; }
            stack[sp] = max(lifetime - life, 0.0);
            sp = sp + 1u;
        } else if code == 25u { // age01
            if sp >= 64u { return 0.0; }
            stack[sp] = t;
            sp = sp + 1u;
        } else if code == 26u { // spawn_time
            if sp >= 64u { return 0.0; }
            stack[sp] = spawn_time;
            sp = sp + 1u;
        } else if code == 27u { // emitter_time
            if sp >= 64u { return 0.0; }
            stack[sp] = emitter_time;
            sp = sp + 1u;
        } else if code == 28u { // dir_x
            if sp >= 64u { return 0.0; }
            stack[sp] = dir_x;
            sp = sp + 1u;
        } else if code == 29u { // dir_y
            if sp >= 64u { return 0.0; }
            stack[sp] = dir_y;
            sp = sp + 1u;
        } else if code == 30u { // dir_z
            if sp >= 64u { return 0.0; }
            stack[sp] = dir_z;
            sp = sp + 1u;
        } else if code == 31u { // vel_x
            if sp >= 64u { return 0.0; }
            stack[sp] = vel_x;
            sp = sp + 1u;
        } else if code == 32u { // vel_y
            if sp >= 64u { return 0.0; }
            stack[sp] = vel_y;
            sp = sp + 1u;
        } else if code == 33u { // vel_z
            if sp >= 64u { return 0.0; }
            stack[sp] = vel_z;
            sp = sp + 1u;
        } else if code == 34u { // seed
            if sp >= 64u { return 0.0; }
            stack[sp] = seed;
            sp = sp + 1u;
        } else if code == 35u { // ring_u
            if sp >= 64u { return 0.0; }
            stack[sp] = ring_u;
            sp = sp + 1u;
        } else if code == 36u { // index01
            if sp >= 64u { return 0.0; }
            stack[sp] = index01;
            sp = sp + 1u;
        } else if code == 37u { // emitter_x
            if sp >= 64u { return 0.0; }
            stack[sp] = emitter_x;
            sp = sp + 1u;
        } else if code == 38u { // emitter_y
            if sp >= 64u { return 0.0; }
            stack[sp] = emitter_y;
            sp = sp + 1u;
        } else if code == 39u { // emitter_z
            if sp >= 64u { return 0.0; }
            stack[sp] = emitter_z;
            sp = sp + 1u;
        } else if code == 40u { // prev_x
            if sp >= 64u { return 0.0; }
            stack[sp] = prev_x;
            sp = sp + 1u;
        } else if code == 41u { // prev_y
            if sp >= 64u { return 0.0; }
            stack[sp] = prev_y;
            sp = sp + 1u;
        } else if code == 42u { // prev_z
            if sp >= 64u { return 0.0; }
            stack[sp] = prev_z;
            sp = sp + 1u;
        } else if code == 43u { // hash
            if sp < 1u { return 0.0; }
            sp = sp - 1u; let a = stack[sp];
            stack[sp] = hash01f(a); sp = sp + 1u;
        } else {
            return 0.0;
        }
    }
    if sp == 1u {
        return stack[0u];
    }
    return 0.0;
}

fn eval_particle(particle_index: u32) -> ComputedParticle {
    var out: ComputedParticle;
    out.world_pos = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    out.color = vec4<f32>(0.0);
    out.emissive = vec4<f32>(0.0);

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
    let origin = particle_spawn_origins[particle_index].xyz;
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
    let h0 = hash01(e.counts_seed.w ^ spawn_index);
    let h1 = hash01((e.counts_seed.w + 0x9E3779B9u) ^ (spawn_index * 3u));
    let h2 = hash01((e.counts_seed.w + 0x7F4A7C15u) ^ (spawn_index * 7u));
    let h3 = hash01((e.counts_seed.w + 0x94D049BBu) ^ (spawn_index * 11u));
    let lifetime = life_min + (life_max - life_min) * h0;
    let spawn_t = f32(spawn_index) / rate;
    let local_t = prewarm_time - spawn_t;
    if local_t < 0.0 || local_t > lifetime {
        return out;
    }

    let age = clamp(local_t / lifetime, 0.0, 1.0);
    let prev_local_t = max(local_t - max(e.time_path.w, 0.0), 0.0);
    let prev_age = clamp(prev_local_t / lifetime, 0.0, 1.0);
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
    let vel = dir * speed;
    let ring_u = fract(f32(spawn_index) * 0.61803395 + h3 * 0.1234567);
    var index01: f32 = 0.0;
    if e.counts_seed.y > 1u {
        index01 = f32(local_i) / f32(e.counts_seed.y - 1u);
    }
    var pos = origin;
    var prev_pos = origin;

    let path_kind = u32(max(e.gravity_path.w, 0.0));
    let path_a = e.time_path.y;
    let path_b = e.time_path.z;
    if path_kind == 1u {
        pos = pos + dir * speed * local_t;
        prev_pos = prev_pos + dir * speed * prev_local_t;
    } else if path_kind == 2u {
        let theta = local_t * path_a + h0 * 6.28318530718;
        pos = pos + vec3<f32>(cos(theta) * path_b, 0.0, sin(theta) * path_b);
        let prev_theta = prev_local_t * path_a + h0 * 6.28318530718;
        prev_pos = prev_pos + vec3<f32>(cos(prev_theta) * path_b, 0.0, sin(prev_theta) * path_b);
    } else if path_kind == 3u {
        let theta = local_t * path_a + h1 * 6.28318530718;
        pos = origin + vec3<f32>(cos(theta) * path_b, pos.y - origin.y, sin(theta) * path_b);
        let prev_theta = prev_local_t * path_a + h1 * 6.28318530718;
        prev_pos = origin + vec3<f32>(cos(prev_theta) * path_b, prev_pos.y - origin.y, sin(prev_theta) * path_b);
    } else if path_kind == 4u {
        let n = sin(local_t * path_b + h2 * 37.0);
        let m = cos(local_t * path_b * 1.37 + h1 * 17.0);
        pos = pos + vec3<f32>(n, m, n * m) * abs(path_a);
        let prev_n = sin(prev_local_t * path_b + h2 * 37.0);
        let prev_m = cos(prev_local_t * path_b * 1.37 + h1 * 17.0);
        prev_pos = prev_pos + vec3<f32>(prev_n, prev_m, prev_n * prev_m) * abs(path_a);
    } else if path_kind == 5u {
        let seq = (f32(local_i) + 0.5) / max(f32(max(e.counts_seed.y, 1u)), 1.0);
        let theta = f32(local_i) * 2.3999631 + h3 * 0.35;
        let radial = sqrt(seq);
        let r = path_b * radial * age;
        pos = pos + vec3<f32>(cos(theta) * r, 0.0, sin(theta) * r);
        let prev_r = path_b * radial * prev_age;
        prev_pos = prev_pos + vec3<f32>(cos(theta) * prev_r, 0.0, sin(theta) * prev_r);
    }

    let force = e.gravity_path.xyz;
    pos = pos + 0.5 * force * local_t * local_t;
    prev_pos = prev_pos + 0.5 * force * prev_local_t * prev_local_t;

    if e.custom_ops_xy.y > 0u && e.custom_ops_xy.w > 0u && e.custom_ops_zp.y > 0u {
        let dx = eval_expr(
            e.custom_ops_xy.x,
            e.custom_ops_xy.y,
            age,
            local_t,
            lifetime,
            spawn_t,
            time,
            speed,
            f32(spawn_index),
            dir.x,
            dir.y,
            dir.z,
            vel.x,
            vel.y,
            vel.z,
            h0,
            h1,
            h2,
            f32(spawn_index),
            ring_u,
            index01,
            origin.x,
            origin.y,
            origin.z,
            prev_pos.x,
            prev_pos.y,
            prev_pos.z,
            e.custom_ops_zp.z,
            e.custom_ops_zp.w,
        );
        let dy = eval_expr(
            e.custom_ops_xy.z,
            e.custom_ops_xy.w,
            age,
            local_t,
            lifetime,
            spawn_t,
            time,
            speed,
            f32(spawn_index),
            dir.x,
            dir.y,
            dir.z,
            vel.x,
            vel.y,
            vel.z,
            h0,
            h1,
            h2,
            f32(spawn_index),
            ring_u,
            index01,
            origin.x,
            origin.y,
            origin.z,
            prev_pos.x,
            prev_pos.y,
            prev_pos.z,
            e.custom_ops_zp.z,
            e.custom_ops_zp.w,
        );
        let dz = eval_expr(
            e.custom_ops_zp.x,
            e.custom_ops_zp.y,
            age,
            local_t,
            lifetime,
            spawn_t,
            time,
            speed,
            f32(spawn_index),
            dir.x,
            dir.y,
            dir.z,
            vel.x,
            vel.y,
            vel.z,
            h0,
            h1,
            h2,
            f32(spawn_index),
            ring_u,
            index01,
            origin.x,
            origin.y,
            origin.z,
            prev_pos.x,
            prev_pos.y,
            prev_pos.z,
            e.custom_ops_zp.z,
            e.custom_ops_zp.w,
        );
        pos = pos + vec3<f32>(dx, dy, dz);
    }

    let spin = bitcast<f32>(e.flags.z);
    if abs(spin) > 1.0e-6 {
        let rel = pos - origin;
        let theta = spin * local_t;
        let s = sin(theta);
        let c = cos(theta);
        pos = origin + vec3<f32>(rel.x * c - rel.z * s, rel.y, rel.x * s + rel.z * c);
    }

    let color = e.color_start + (e.color_end - e.color_start) * age;
    let size = e.emissive_point.w * (size_min + (size_max - size_min) * h2);
    out.world_pos = vec4<f32>(pos, size);
    out.color = vec4<f32>(color.rgb, clamp(color.a, 0.0, 1.0));
    out.emissive = vec4<f32>(e.emissive_point.xyz, f32(e.flags.w));
    return out;
}

@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if idx >= params.particle_count {
        return;
    }
    particles[idx] = eval_particle(idx);
}

struct ParticleOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) emissive: vec3<f32>,
}

@vertex
fn vs_main(@builtin(instance_index) particle_index: u32) -> ParticleOut {
    var out: ParticleOut;
    let p = particles_read[particle_index];
    out.clip_pos = camera.view_proj * vec4<f32>(p.world_pos.xyz, 1.0);
    out.color = p.color;
    out.emissive = p.emissive.xyz;
    return out;
}

@vertex
fn vs_billboard(
    @builtin(instance_index) particle_index: u32,
    @builtin(vertex_index) vertex_index: u32,
) -> ParticleOut {
    var out: ParticleOut;
    let p = particles_read[particle_index];
    let center_clip = camera.view_proj * vec4<f32>(p.world_pos.xyz, 1.0);
    let corners = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, 1.0),
    );
    let half_size = max(p.world_pos.w * 0.5, 1.0);
    let ndc_offset = corners[vertex_index] * half_size * camera.inv_view_size * 2.0;
    out.clip_pos = center_clip + vec4<f32>(ndc_offset * center_clip.w, 0.0, 0.0);
    out.color = p.color;
    out.emissive = p.emissive.xyz;
    return out;
}

@fragment
fn fs_main(in: ParticleOut) -> @location(0) vec4<f32> {
    let rgb = in.color.rgb + in.emissive;
    return vec4<f32>(rgb, in.color.a);
}


