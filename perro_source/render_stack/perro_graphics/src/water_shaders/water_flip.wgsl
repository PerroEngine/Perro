struct FlipWater {
    particle_offset_count: vec4<u32>,
    grid_offset_dims_x: vec4<u32>,
    dims_yz_pad: vec4<u32>,
    size_depth_cell: vec4<f32>,
    flow_splash: vec4<f32>,
    splash_pos_radius: vec4<f32>,
    deep_color: vec4<f32>,
    shallow_color: vec4<f32>,
    model_x: vec4<f32>,
    model_y: vec4<f32>,
    model_z: vec4<f32>,
    model_w: vec4<f32>,
}

struct Particle {
    position: vec4<f32>,
    velocity: vec4<f32>,
    affine_x: vec4<f32>,
    affine_y: vec4<f32>,
}

struct GridAccum {
    vx: atomic<i32>,
    vy: atomic<i32>,
    vz: atomic<i32>,
    weight: atomic<i32>,
}

struct GridVelocity {
    current: vec4<f32>,
    previous: vec4<f32>,
}

struct Params {
    water_count: u32,
    particle_count: u32,
    grid_count: u32,
    frame_seed: u32,
    delta_seconds: f32,
    gravity: f32,
    flip_ratio: f32,
    _pad: f32,
}

@group(0) @binding(0) var<storage, read> waters: array<FlipWater>;
@group(0) @binding(1) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(2) var<storage, read_write> accum: array<GridAccum>;
@group(0) @binding(3) var<storage, read_write> grid_velocity: array<GridVelocity>;
@group(0) @binding(4) var<uniform> params: Params;

const FIXED_SCALE: f32 = 4096.0;

fn hash(v: u32) -> u32 {
    var x = v * 747796405u + 2891336453u;
    x = ((x >> ((x >> 28u) + 4u)) ^ x) * 277803737u;
    return (x >> 22u) ^ x;
}

fn dims(w: FlipWater) -> vec3<u32> {
    return vec3<u32>(w.grid_offset_dims_x.y, w.dims_yz_pad.x, w.dims_yz_pad.y);
}

fn grid_coord(w: FlipWater, local: vec3<f32>) -> vec3<u32> {
    let d = dims(w);
    let uvw = clamp(vec3<f32>(
        local.x / w.size_depth_cell.x + 0.5,
        -local.y / w.size_depth_cell.y,
        local.z / w.size_depth_cell.z + 0.5,
    ), vec3<f32>(0.0), vec3<f32>(0.99999));
    return min(vec3<u32>(uvw * vec3<f32>(d)), d - vec3<u32>(1u));
}

fn grid_index(w: FlipWater, c: vec3<u32>) -> u32 {
    let d = dims(w);
    return w.grid_offset_dims_x.x + (c.y * d.z + c.z) * d.x + c.x;
}

fn local_surface(w: FlipWater, p: vec3<f32>) -> f32 {
    let flow = vec2<f32>(w.flow_splash.xy);
    return sin((p.x + flow.x * f32(params.frame_seed) * params.delta_seconds) * 0.7
        + p.z * 0.43) * w.size_depth_cell.w * 0.32;
}

fn initial_particle(w: FlipWater, idx: u32, water_index: u32) -> Particle {
    let local_idx = idx - w.particle_offset_count.x;
    let d = dims(w);
    let layer_size = max(d.x * d.z, 1u);
    let layer = local_idx / layer_size;
    let cell = local_idx % layer_size;
    let x = cell % d.x;
    let z = (cell / d.x) % d.z;
    let jitter = vec2<f32>(f32(hash(idx) & 255u), f32(hash(idx + 17u) & 255u)) / 255.0 - 0.5;
    let pos = vec3<f32>(
        (f32(x) + 0.5 + jitter.x * 0.5) / f32(d.x) * w.size_depth_cell.x - w.size_depth_cell.x * 0.5,
        -(f32(layer) + 0.55) * w.size_depth_cell.w * 0.42,
        (f32(z) + 0.5 + jitter.y * 0.5) / f32(d.z) * w.size_depth_cell.z - w.size_depth_cell.z * 0.5,
    );
    return Particle(vec4<f32>(pos, 1.0), vec4<f32>(w.flow_splash.x, 0.0, w.flow_splash.y, 0.0), vec4<f32>(0.0, 0.0, 0.0, f32(water_index)), vec4<f32>(0.0));
}

@compute @workgroup_size(64)
fn cs_clear_grid(@builtin(global_invocation_id) gid: vec3<u32>) {
    let w = waters[gid.y];
    let local_idx = gid.x;
    let cells = w.grid_offset_dims_x.y * w.dims_yz_pad.x * w.dims_yz_pad.y;
    if local_idx >= cells { return; }
    let idx = w.grid_offset_dims_x.x + local_idx;
    atomicStore(&accum[idx].vx, 0);
    atomicStore(&accum[idx].vy, 0);
    atomicStore(&accum[idx].vz, 0);
    atomicStore(&accum[idx].weight, 0);
}

@compute @workgroup_size(64)
fn cs_p2g(@builtin(global_invocation_id) gid: vec3<u32>) {
    let wi = gid.y;
    let w = waters[wi];
    if gid.x >= w.particle_offset_count.y { return; }
    let idx = w.particle_offset_count.x + gid.x;
    var p = particles[idx];
    if p.position.w < 0.5 { p = initial_particle(w, idx, wi); particles[idx] = p; }
    let cell = grid_index(w, grid_coord(w, p.position.xyz));
    atomicAdd(&accum[cell].vx, i32(clamp(p.velocity.x * FIXED_SCALE, -2147480000.0, 2147480000.0)));
    atomicAdd(&accum[cell].vy, i32(clamp(p.velocity.y * FIXED_SCALE, -2147480000.0, 2147480000.0)));
    atomicAdd(&accum[cell].vz, i32(clamp(p.velocity.z * FIXED_SCALE, -2147480000.0, 2147480000.0)));
    atomicAdd(&accum[cell].weight, i32(FIXED_SCALE));
}

@compute @workgroup_size(64)
fn cs_grid(@builtin(global_invocation_id) gid: vec3<u32>) {
    let wi = gid.y;
    let w = waters[wi];
    let cells = w.grid_offset_dims_x.y * w.dims_yz_pad.x * w.dims_yz_pad.y;
    if gid.x >= cells { return; }
    let idx = w.grid_offset_dims_x.x + gid.x;
    let old = grid_velocity[idx].current;
    let weight = f32(atomicLoad(&accum[idx].weight)) / FIXED_SCALE;
    var velocity = old.xyz * 0.98;
    if weight > 0.0 {
        velocity = vec3<f32>(
            f32(atomicLoad(&accum[idx].vx)),
            f32(atomicLoad(&accum[idx].vy)),
            f32(atomicLoad(&accum[idx].vz)),
        ) / (FIXED_SCALE * weight);
    }
    velocity = velocity + vec3<f32>(
        w.flow_splash.x * params.delta_seconds,
        -params.gravity * params.delta_seconds,
        w.flow_splash.y * params.delta_seconds,
    );
    velocity *= 1.0 - w.flow_splash.w * params.delta_seconds;
    grid_velocity[idx].previous = old;
    grid_velocity[idx].current = vec4<f32>(velocity, weight);
}

@compute @workgroup_size(64)
fn cs_g2p(@builtin(global_invocation_id) gid: vec3<u32>) {
    let wi = gid.y;
    let w = waters[wi];
    if gid.x >= w.particle_offset_count.y { return; }
    let idx = w.particle_offset_count.x + gid.x;
    var p = particles[idx];
    let cell = grid_index(w, grid_coord(w, p.position.xyz));
    let gv = grid_velocity[cell];
    let pic = gv.current.xyz;
    let flip = p.velocity.xyz + gv.current.xyz - gv.previous.xyz;
    var velocity = mix(pic, flip, params.flip_ratio);
    // velocity.w is an explicit droplet state.  Do not infer splash visibility
    // from a pool particle drifting above the approximate render surface.
    var droplet = p.velocity.w > 0.5;
    let splash_delta = p.position.xyz - w.splash_pos_radius.xyz;
    let splash_r = w.splash_pos_radius.w;
    let in_splash = splash_r > 0.0 && w.flow_splash.z > 0.05 && length(splash_delta.xz) < splash_r;
    let impact_epoch = w.particle_offset_count.z;
    let unseen_impact = impact_epoch != 0u && u32(p.affine_y.x) != impact_epoch;
    // Every particle consumes each impact epoch once. This prevents a retained
    // physics contact from launching a fresh batch every rendered frame.
    p.affine_y.x = f32(impact_epoch);
    let surface = local_surface(w, p.position.xyz);
    if unseen_impact && in_splash && abs(p.position.y - surface) < w.size_depth_cell.w * 0.75 && (hash(idx + params.frame_seed) & 31u) == 0u {
        let radial = normalize(vec3<f32>(splash_delta.x, 0.2, splash_delta.z) + vec3<f32>(0.001, 0.0, 0.0));
        let impulse = min(w.flow_splash.z, max(2.0, w.size_depth_cell.y * 0.75));
        velocity += radial * impulse * 0.35 + vec3<f32>(0.0, impulse * 0.55, 0.0);
        droplet = true;
    }
    p.position = vec4<f32>(p.position.xyz + velocity * params.delta_seconds, p.position.w);
    let new_surface = local_surface(w, p.position.xyz);
    // Droplet -> surface: conserve horizontal momentum and rejoin FLIP pool.
    if droplet && p.position.y <= new_surface && velocity.y < 0.0 {
        p.position = vec4<f32>(p.position.x, min(new_surface - w.size_depth_cell.w * 0.15, -0.001), p.position.z, p.position.w);
        velocity = mix(velocity, pic, 0.72);
        velocity = vec3<f32>(velocity.x, max(velocity.y * -0.08, -0.05), velocity.z);
        droplet = false;
    } else if !droplet && p.position.y > new_surface {
        // FLIP liquid stays in the volume.  Only an impact may detach it.
        p.position.y = new_surface - w.size_depth_cell.w * 0.15;
        velocity.y = min(velocity.y, 0.0);
    }
    let half_size = w.size_depth_cell.xz * 0.5;
    if abs(p.position.x) > half_size.x || abs(p.position.z) > half_size.y || p.position.y < -w.size_depth_cell.y {
        p = initial_particle(w, idx, wi);
    } else {
        p.velocity = vec4<f32>(velocity, select(0.0, 1.0, droplet));
    }
    p.affine_x.w = f32(wi);
    particles[idx] = p;
}

struct Camera3D { view_proj: mat4x4<f32> }
@group(1) @binding(0) var<uniform> camera: Camera3D;

struct SplashOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
}

fn model(w: FlipWater) -> mat4x4<f32> {
    return mat4x4<f32>(w.model_x, w.model_y, w.model_z, w.model_w);
}

@vertex
fn vs_splash(@builtin(instance_index) idx: u32, @builtin(vertex_index) vertex: u32) -> SplashOut {
    let wi = u32(particles[idx].affine_x.w);
    let w = waters[wi];
    let p = particles[idx];
    let world = model(w) * vec4<f32>(p.position.xyz, 1.0);
    let center = camera.view_proj * world;
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let uv = corners[vertex];
    let visible = select(0.0, 1.0, p.velocity.w > 0.5);
    let radius = w.size_depth_cell.w * 0.9 * visible;
    var out: SplashOut;
    let projected_radius = radius * vec2<f32>(camera.view_proj[0][0], camera.view_proj[1][1]);
    out.clip = center + vec4<f32>(uv * projected_radius, 0.0, 0.0);
    out.color = mix(w.deep_color, w.shallow_color, 0.8);
    out.color.a *= visible;
    out.uv = uv;
    return out;
}

@fragment
fn fs_splash(in: SplashOut) -> @location(0) vec4<f32> {
    let edge = smoothstep(1.0, 0.55, length(in.uv));
    return vec4<f32>(in.color.rgb, in.color.a * edge);
}
