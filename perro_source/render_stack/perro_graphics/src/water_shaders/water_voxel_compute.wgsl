struct Water {
    node: u32,
    kind: u32,
    idle_mode: u32,
    z_index: i32,
    size_depth_time: vec4<f32>,
    flow_wind: vec4<f32>,
    wave: vec4<f32>,
    flags: vec4<u32>,
    deep_color: vec4<f32>,
    shallow_color: vec4<f32>,
    sky_color_bias: vec4<f32>,
    foam_color: vec4<f32>,
    visual0: vec4<f32>,
    visual1: vec4<f32>,
    visual2: vec4<f32>,
    wave_profile: vec4<f32>,
    coastline_foam_color: vec4<f32>,
    coastline: vec4<f32>,
    shape: vec4<f32>,
    sim: vec4<u32>,
    model_x: vec4<f32>,
    model_y: vec4<f32>,
    model_z: vec4<f32>,
    model_w: vec4<f32>,
}

struct Params {
    water_count: u32,
    water_2d_count: u32,
    cell_count: u32,
    _pad: u32,
    time_seconds: f32,
    delta_seconds: f32,
    _pad1: vec2<f32>,
}

struct VoxelWater {
    water_idx: u32,
    cell_offset: u32,
    cell_count: u32,
    _pad0: u32,
    dims: vec4<u32>,
    size_depth_voxel: vec4<f32>,
    render: vec4<f32>,
}

@group(0) @binding(0) var<storage, read> waters: array<Water>;
@group(0) @binding(1) var<storage, read> voxel_waters: array<VoxelWater>;
@group(0) @binding(2) var<storage, read_write> density: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> velocity: array<vec4<f32>>;
@group(0) @binding(4) var<storage, read_write> velocity_next: array<vec4<f32>>;
@group(0) @binding(5) var<storage, read_write> pressure: array<vec4<f32>>;
@group(0) @binding(6) var<storage, read_write> surface: array<vec4<f32>>;
@group(0) @binding(7) var<storage, read> coastline: array<vec4<f32>>;
@group(0) @binding(8) var<uniform> params: Params;

fn voxel_idx(dims: vec3<u32>, p: vec3<u32>) -> u32 {
    return (p.y * dims.z + p.z) * dims.x + p.x;
}

fn voxel_coord(dims: vec3<u32>, idx: u32) -> vec3<u32> {
    let x = idx % dims.x;
    let z = (idx / dims.x) % dims.z;
    let y = idx / max(dims.x * dims.z, 1u);
    return vec3<u32>(x, y, z);
}

fn safe_cell(v: VoxelWater, p: vec3<i32>) -> u32 {
    let dims = vec3<u32>(v.dims.xyz);
    let q = vec3<u32>(
        u32(clamp(p.x, 0, i32(dims.x) - 1)),
        u32(clamp(p.y, 0, i32(dims.y) - 1)),
        u32(clamp(p.z, 0, i32(dims.z) - 1)),
    );
    return v.cell_offset + voxel_idx(dims, q);
}

fn wave_seed(w: Water, local: vec3<f32>, t: f32) -> f32 {
    let wind = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.zw, length(w.flow_wind.zw) > 0.0001));
    let p = local.xz / max(w.wave_profile.x, 0.001);
    let a = sin(dot(p, wind) * 6.2831853 + t * w.wave.x * 0.2);
    let b = sin((p.x * 1.7 - p.y * 0.8) * 6.2831853 - t * w.wave.x * 0.31);
    return (a * 0.72 + b * 0.28) * w.wave.y;
}

fn base_density(v: VoxelWater, w: Water, local: vec3<f32>, coord: vec3<u32>) -> f32 {
    let half = v.size_depth_voxel.xyz * 0.5;
    if abs(local.x) > half.x || abs(local.z) > half.z || local.y > 0.05 || local.y < -v.size_depth_voxel.y {
        return 0.0;
    }
    let wave = wave_seed(w, local, params.time_seconds);
    let fill = -local.y + wave;
    let surface_band = max(v.size_depth_voxel.w * 1.5, 0.03);
    let body = smoothstep(-surface_band, surface_band, fill);
    let top = 1.0 - smoothstep(surface_band, surface_band * 2.5, fill);
    return clamp(body * (0.78 + top * 0.22), 0.0, 1.0);
}

fn local_from_coord(v: VoxelWater, coord: vec3<u32>) -> vec3<f32> {
    let dims = vec3<f32>(max(v.dims.xyz, vec3<u32>(1u)));
    let uvw = (vec3<f32>(coord) + vec3<f32>(0.5)) / dims;
    return vec3<f32>(
        (uvw.x - 0.5) * v.size_depth_voxel.x,
        -uvw.y * v.size_depth_voxel.y,
        (uvw.z - 0.5) * v.size_depth_voxel.z,
    );
}

fn coast_cell(w: Water, local_xz: vec2<f32>) -> vec4<f32> {
    let width = max(w.sim.z, 1u);
    let height = max(w.sim.w, 1u);
    let count = max(w.sim.y, 1u);
    let u = clamp(local_xz.x / max(w.size_depth_time.x, 0.001) + 0.5, 0.0, 0.999999);
    let v = clamp(local_xz.y / max(w.size_depth_time.y, 0.001) + 0.5, 0.0, 0.999999);
    let x = min(u32(floor(u * f32(width))), width - 1u);
    let y = min(u32(floor(v * f32(height))), height - 1u);
    let idx = min(y * width + x, count - 1u);
    return coastline[w.sim.x + idx];
}

fn coast_gradient(w: Water, local_xz: vec2<f32>) -> vec2<f32> {
    let texel = vec2<f32>(
        w.size_depth_time.x / f32(max(w.sim.z, 1u)),
        w.size_depth_time.y / f32(max(w.sim.w, 1u)),
    );
    let sx = coast_cell(w, local_xz + vec2<f32>(texel.x, 0.0)).x
        - coast_cell(w, local_xz - vec2<f32>(texel.x, 0.0)).x;
    let sz = coast_cell(w, local_xz + vec2<f32>(0.0, texel.y)).x
        - coast_cell(w, local_xz - vec2<f32>(0.0, texel.y)).x;
    return vec2<f32>(sx, sz);
}

@compute @workgroup_size(64)
fn cs_inject(@builtin(global_invocation_id) gid: vec3<u32>) {
    let vi = gid.y;
    if vi >= arrayLength(&voxel_waters) {
        return;
    }
    let v = voxel_waters[vi];
    let idx = gid.x;
    if idx >= v.cell_count {
        return;
    }
    let w = waters[v.water_idx];
    let coord = voxel_coord(v.dims.xyz, idx);
    let local = local_from_coord(v, coord);
    let base = base_density(v, w, local, coord);
    let aid = coast_cell(w, local.xz);
    let solid = clamp(aid.x, 0.0, 1.0);
    let wake = clamp(aid.z + aid.w * 0.35, 0.0, 1.0);
    let carved = base * (1.0 - solid);
    let old = density[v.cell_offset + idx].x;
    let injected = mix(old, carved, 0.16 + clamp(params.delta_seconds * 4.0, 0.0, 0.30));
    density[v.cell_offset + idx] = vec4<f32>(clamp(injected + wake * 0.18, 0.0, 1.0), carved, wake, 1.0);
    let flow = vec3<f32>(w.flow_wind.x, 0.0, w.flow_wind.y);
    let coast_push = coast_gradient(w, local.xz);
    let outward = normalize(vec3<f32>(-coast_push.x, 0.0, -coast_push.y) + vec3<f32>(0.0001, 0.0, 0.0));
    let splash = wake * (0.85 + w.wave.y * 0.25);
    let wall_slide = 1.0 - solid * 0.85;
    velocity[v.cell_offset + idx] = vec4<f32>(
        (flow * 0.08 + outward * (aid.y * 0.18 + splash * 0.55) + vec3<f32>(0.0, splash * 0.35 - 0.02, 0.0)) * wall_slide,
        0.0,
    );
}

@compute @workgroup_size(64)
fn cs_advect(@builtin(global_invocation_id) gid: vec3<u32>) {
    let vi = gid.y;
    if vi >= arrayLength(&voxel_waters) {
        return;
    }
    let v = voxel_waters[vi];
    let idx = gid.x;
    if idx >= v.cell_count {
        return;
    }
    let dims = v.dims.xyz;
    let coord = voxel_coord(dims, idx);
    let cur = v.cell_offset + idx;
    let vel = velocity[cur].xyz;
    let back = vec3<i32>(coord) - vec3<i32>(round(vel * params.delta_seconds * 8.0));
    let src = safe_cell(v, back);
    surface[cur] = mix(density[cur], density[src], 0.55);
    velocity_next[cur] = mix(velocity[cur], velocity[src], 0.55);
}

@compute @workgroup_size(64)
fn cs_divergence(@builtin(global_invocation_id) gid: vec3<u32>) {
    let vi = gid.y;
    if vi >= arrayLength(&voxel_waters) {
        return;
    }
    let v = voxel_waters[vi];
    let idx = gid.x;
    if idx >= v.cell_count {
        return;
    }
    let c = vec3<i32>(voxel_coord(v.dims.xyz, idx));
    let cur = v.cell_offset + idx;
    density[cur] = surface[cur];
    velocity[cur] = velocity_next[cur];
    let local = local_from_coord(v, voxel_coord(v.dims.xyz, idx));
    let solid = clamp(coast_cell(waters[v.water_idx], local.xz).x, 0.0, 1.0);
    let vx = velocity[safe_cell(v, c + vec3<i32>(1, 0, 0))].x - velocity[safe_cell(v, c - vec3<i32>(1, 0, 0))].x;
    let vy = velocity[safe_cell(v, c + vec3<i32>(0, 1, 0))].y - velocity[safe_cell(v, c - vec3<i32>(0, 1, 0))].y;
    let vz = velocity[safe_cell(v, c + vec3<i32>(0, 0, 1))].z - velocity[safe_cell(v, c - vec3<i32>(0, 0, 1))].z;
    velocity_next[cur] = vec4<f32>(velocity[cur].xyz, (vx + vy + vz) * 0.5 * (1.0 - solid));
    pressure[cur] = vec4<f32>(0.0);
}

@compute @workgroup_size(64)
fn cs_pressure(@builtin(global_invocation_id) gid: vec3<u32>) {
    let vi = gid.y;
    if vi >= arrayLength(&voxel_waters) {
        return;
    }
    let v = voxel_waters[vi];
    let idx = gid.x;
    if idx >= v.cell_count {
        return;
    }
    let c = vec3<i32>(voxel_coord(v.dims.xyz, idx));
    let cur = v.cell_offset + idx;
    let p = pressure[safe_cell(v, c + vec3<i32>(1, 0, 0))].x
        + pressure[safe_cell(v, c - vec3<i32>(1, 0, 0))].x
        + pressure[safe_cell(v, c + vec3<i32>(0, 1, 0))].x
        + pressure[safe_cell(v, c - vec3<i32>(0, 1, 0))].x
        + pressure[safe_cell(v, c + vec3<i32>(0, 0, 1))].x
        + pressure[safe_cell(v, c - vec3<i32>(0, 0, 1))].x;
    pressure[cur] = vec4<f32>((p - velocity_next[cur].w) / 6.0, 0.0, 0.0, 0.0);
}

@compute @workgroup_size(64)
fn cs_project(@builtin(global_invocation_id) gid: vec3<u32>) {
    let vi = gid.y;
    if vi >= arrayLength(&voxel_waters) {
        return;
    }
    let v = voxel_waters[vi];
    let idx = gid.x;
    if idx >= v.cell_count {
        return;
    }
    let c = vec3<i32>(voxel_coord(v.dims.xyz, idx));
    let cur = v.cell_offset + idx;
    let grad = vec3<f32>(
        pressure[safe_cell(v, c + vec3<i32>(1, 0, 0))].x - pressure[safe_cell(v, c - vec3<i32>(1, 0, 0))].x,
        pressure[safe_cell(v, c + vec3<i32>(0, 1, 0))].x - pressure[safe_cell(v, c - vec3<i32>(0, 1, 0))].x,
        pressure[safe_cell(v, c + vec3<i32>(0, 0, 1))].x - pressure[safe_cell(v, c - vec3<i32>(0, 0, 1))].x,
    ) * 0.5;
    let local = local_from_coord(v, voxel_coord(v.dims.xyz, idx));
    let aid = coast_cell(waters[v.water_idx], local.xz);
    let solid = clamp(aid.x, 0.0, 1.0);
    let push = coast_gradient(waters[v.water_idx], local.xz);
    let wall_normal = normalize(vec3<f32>(-push.x, 0.0, -push.y) + vec3<f32>(0.0001, 0.0, 0.0));
    let projected = velocity[cur].xyz - grad;
    velocity[cur] = vec4<f32>((projected + wall_normal * aid.y * 0.04) * (1.0 - solid * 0.9) * 0.992, 0.0);
}

@compute @workgroup_size(64)
fn cs_surface(@builtin(global_invocation_id) gid: vec3<u32>) {
    let vi = gid.y;
    if vi >= arrayLength(&voxel_waters) {
        return;
    }
    let v = voxel_waters[vi];
    let idx = gid.x;
    if idx >= v.cell_count {
        return;
    }
    let c = vec3<i32>(voxel_coord(v.dims.xyz, idx));
    let cur = v.cell_offset + idx;
    let d = density[cur].x;
    let local = local_from_coord(v, voxel_coord(v.dims.xyz, idx));
    let aid = coast_cell(waters[v.water_idx], local.xz);
    let grad = vec3<f32>(
        density[safe_cell(v, c + vec3<i32>(1, 0, 0))].x - density[safe_cell(v, c - vec3<i32>(1, 0, 0))].x,
        density[safe_cell(v, c + vec3<i32>(0, 1, 0))].x - density[safe_cell(v, c - vec3<i32>(0, 1, 0))].x,
        density[safe_cell(v, c + vec3<i32>(0, 0, 1))].x - density[safe_cell(v, c - vec3<i32>(0, 0, 1))].x,
    );
    let top_shell = 1.0 - smoothstep(v.size_depth_voxel.w * 1.5, v.size_depth_voxel.w * 5.0, -local.y);
    let fluid_edge = smoothstep(0.08, 0.32, length(grad)) * smoothstep(0.08, 0.55, d) * (1.0 - smoothstep(0.92, 1.0, d));
    let contact_edge = clamp(aid.y * 0.32 + aid.z * 0.18, 0.0, 1.0);
    let edge = max(
        fluid_edge * top_shell,
        contact_edge * top_shell,
    );
    surface[cur] = vec4<f32>(edge, normalize(-grad + vec3<f32>(0.0, 0.15, 0.0)));
}
