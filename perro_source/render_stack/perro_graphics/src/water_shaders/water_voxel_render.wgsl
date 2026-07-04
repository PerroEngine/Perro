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

struct RayLightGpu {
    direction: vec4<f32>,
    color_intensity: vec4<f32>,
}

struct PointLightGpu {
    position_range: vec4<f32>,
    color_intensity: vec4<f32>,
}

struct SpotLightGpu {
    position_range: vec4<f32>,
    direction_outer_cos: vec4<f32>,
    color_intensity: vec4<f32>,
    inner_cos_pad: vec4<f32>,
}

struct Scene3D {
    view_proj: mat4x4<f32>,
    ambient_and_counts: vec4<f32>,
    camera_pos: vec4<f32>,
    ambient_color: vec4<f32>,
    ray_light: RayLightGpu,
    ray_lights: array<RayLightGpu, 3>,
    point_lights: array<PointLightGpu, 8>,
    spot_lights: array<SpotLightGpu, 8>,
    inv_view_proj: mat4x4<f32>,
}

@group(0) @binding(0) var<storage, read> waters: array<Water>;
@group(0) @binding(1) var<storage, read> voxel_waters: array<VoxelWater>;
@group(0) @binding(2) var<storage, read> density: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read> surface: array<vec4<f32>>;
@group(0) @binding(4) var<uniform> params: Params;
@group(1) @binding(0) var<uniform> scene: Scene3D;
@group(2) @binding(0) var scene_depth_tex: texture_depth_2d;

struct Out {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) local_pos: vec3<f32>,
    @location(2) @interpolate(flat) voxel_idx: u32,
    @location(3) @interpolate(flat) water_idx: u32,
}

fn cube_vertex(vertex_idx: u32) -> vec3<f32> {
    var p = array<vec3<f32>, 36>(
        vec3<f32>(-0.5,  0.0, -0.5), vec3<f32>( 0.5,  0.0, -0.5), vec3<f32>( 0.5,  0.0,  0.5),
        vec3<f32>(-0.5,  0.0, -0.5), vec3<f32>( 0.5,  0.0,  0.5), vec3<f32>(-0.5,  0.0,  0.5),
        vec3<f32>(-0.5, -1.0,  0.5), vec3<f32>( 0.5, -1.0,  0.5), vec3<f32>( 0.5, -1.0, -0.5),
        vec3<f32>(-0.5, -1.0,  0.5), vec3<f32>( 0.5, -1.0, -0.5), vec3<f32>(-0.5, -1.0, -0.5),
        vec3<f32>(-0.5, -1.0, -0.5), vec3<f32>( 0.5, -1.0, -0.5), vec3<f32>( 0.5,  0.0, -0.5),
        vec3<f32>(-0.5, -1.0, -0.5), vec3<f32>( 0.5,  0.0, -0.5), vec3<f32>(-0.5,  0.0, -0.5),
        vec3<f32>( 0.5, -1.0, -0.5), vec3<f32>( 0.5, -1.0,  0.5), vec3<f32>( 0.5,  0.0,  0.5),
        vec3<f32>( 0.5, -1.0, -0.5), vec3<f32>( 0.5,  0.0,  0.5), vec3<f32>( 0.5,  0.0, -0.5),
        vec3<f32>( 0.5, -1.0,  0.5), vec3<f32>(-0.5, -1.0,  0.5), vec3<f32>(-0.5,  0.0,  0.5),
        vec3<f32>( 0.5, -1.0,  0.5), vec3<f32>(-0.5,  0.0,  0.5), vec3<f32>( 0.5,  0.0,  0.5),
        vec3<f32>(-0.5, -1.0,  0.5), vec3<f32>(-0.5, -1.0, -0.5), vec3<f32>(-0.5,  0.0, -0.5),
        vec3<f32>(-0.5, -1.0,  0.5), vec3<f32>(-0.5,  0.0, -0.5), vec3<f32>(-0.5,  0.0,  0.5),
    );
    return p[vertex_idx];
}

fn water_model(w: Water) -> mat4x4<f32> {
    return mat4x4<f32>(
        vec4<f32>(w.model_x.xyz, 0.0),
        vec4<f32>(w.model_y.xyz, 0.0),
        vec4<f32>(w.model_z.xyz, 0.0),
        w.model_w,
    );
}

fn water_world_to_local(w: Water, world: vec3<f32>) -> vec3<f32> {
    let rel = world - w.model_w.xyz;
    let axis_x = w.model_x.xyz;
    let axis_y = w.model_y.xyz;
    let axis_z = w.model_z.xyz;
    let sx = max(dot(axis_x, axis_x), 1.0e-5);
    let sy = max(dot(axis_y, axis_y), 1.0e-5);
    let sz = max(dot(axis_z, axis_z), 1.0e-5);
    return vec3<f32>(dot(rel, axis_x) / sx, dot(rel, axis_y) / sy, dot(rel, axis_z) / sz);
}

@vertex
fn vs_voxel_water(
    @builtin(vertex_index) vertex_idx: u32,
    @builtin(instance_index) instance_idx: u32,
) -> Out {
    let v = voxel_waters[instance_idx];
    let w = waters[v.water_idx];
    let unit = cube_vertex(vertex_idx);
    let local = vec3<f32>(
        unit.x * v.size_depth_voxel.x,
        unit.y * v.size_depth_voxel.y,
        unit.z * v.size_depth_voxel.z,
    );
    let world = (water_model(w) * vec4<f32>(local, 1.0)).xyz;
    return Out(scene.view_proj * vec4<f32>(world, 1.0), world, local, instance_idx, v.water_idx);
}

fn voxel_idx(dims: vec3<u32>, p: vec3<u32>) -> u32 {
    return (p.y * dims.z + p.z) * dims.x + p.x;
}

fn clamp_coord(dims: vec3<u32>, p: vec3<i32>) -> vec3<u32> {
    return vec3<u32>(
        u32(clamp(p.x, 0, i32(dims.x) - 1)),
        u32(clamp(p.y, 0, i32(dims.y) - 1)),
        u32(clamp(p.z, 0, i32(dims.z) - 1)),
    );
}

fn sample_buffer_cell(buf_idx: u32, dims: vec3<u32>, p: vec3<i32>, use_surface: bool) -> vec4<f32> {
    let q = clamp_coord(dims, p);
    let idx = voxel_waters[buf_idx].cell_offset + voxel_idx(dims, q);
    if use_surface {
        return surface[idx];
    }
    return density[idx];
}

fn trilinear_sample(buf_idx: u32, uvw: vec3<f32>, use_surface: bool) -> vec4<f32> {
    let dims = voxel_waters[buf_idx].dims.xyz;
    let grid = clamp(uvw, vec3<f32>(0.0), vec3<f32>(0.999999)) * vec3<f32>(max(dims - vec3<u32>(1u), vec3<u32>(1u)));
    let base = vec3<i32>(floor(grid));
    let f = fract(grid);
    let c000 = sample_buffer_cell(buf_idx, dims, base + vec3<i32>(0, 0, 0), use_surface);
    let c100 = sample_buffer_cell(buf_idx, dims, base + vec3<i32>(1, 0, 0), use_surface);
    let c010 = sample_buffer_cell(buf_idx, dims, base + vec3<i32>(0, 1, 0), use_surface);
    let c110 = sample_buffer_cell(buf_idx, dims, base + vec3<i32>(1, 1, 0), use_surface);
    let c001 = sample_buffer_cell(buf_idx, dims, base + vec3<i32>(0, 0, 1), use_surface);
    let c101 = sample_buffer_cell(buf_idx, dims, base + vec3<i32>(1, 0, 1), use_surface);
    let c011 = sample_buffer_cell(buf_idx, dims, base + vec3<i32>(0, 1, 1), use_surface);
    let c111 = sample_buffer_cell(buf_idx, dims, base + vec3<i32>(1, 1, 1), use_surface);
    let x00 = mix(c000, c100, f.x);
    let x10 = mix(c010, c110, f.x);
    let x01 = mix(c001, c101, f.x);
    let x11 = mix(c011, c111, f.x);
    return mix(mix(x00, x10, f.y), mix(x01, x11, f.y), f.z);
}

fn uvw_from_local(v: VoxelWater, local: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        local.x / max(v.size_depth_voxel.x, 0.001) + 0.5,
        clamp(-local.y / max(v.size_depth_voxel.y, 0.001), 0.0, 1.0),
        local.z / max(v.size_depth_voxel.z, 0.001) + 0.5,
    );
}

fn ray_box(origin: vec3<f32>, dir: vec3<f32>, bmin: vec3<f32>, bmax: vec3<f32>) -> vec2<f32> {
    let inv = 1.0 / select(dir, vec3<f32>(1.0e-5), abs(dir) < vec3<f32>(1.0e-5));
    let t0 = (bmin - origin) * inv;
    let t1 = (bmax - origin) * inv;
    let mn = min(t0, t1);
    let mx = max(t0, t1);
    return vec2<f32>(max(max(mn.x, mn.y), mn.z), min(min(mx.x, mx.y), mx.z));
}

fn scene_world_from_depth(coord: vec2<i32>, dims_u: vec2<u32>, depth: f32) -> vec3<f32> {
    let uv = (vec2<f32>(coord) + vec2<f32>(0.5)) / vec2<f32>(dims_u);
    let ndc_xy = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let ndc = vec4<f32>(ndc_xy, depth, 1.0);
    let world_h = scene.inv_view_proj * ndc;
    return world_h.xyz / max(abs(world_h.w), 1.0e-5);
}

fn screen_contact(world_pos: vec3<f32>, blend: f32) -> vec2<f32> {
    let dims_u = textureDimensions(scene_depth_tex);
    let dims = vec2<i32>(i32(dims_u.x), i32(dims_u.y));
    let clip = scene.view_proj * vec4<f32>(world_pos, 1.0);
    let ndc = clip.xyz / max(abs(clip.w), 1.0e-5);
    let coord = vec2<i32>(vec2<f32>((ndc.x * 0.5 + 0.5) * f32(dims.x), (0.5 - ndc.y * 0.5) * f32(dims.y)));
    if any(coord < vec2<i32>(0)) || any(coord >= dims) {
        return vec2<f32>(0.0);
    }
    var edge = 0.0;
    var core = 0.0;
    let water_dist = distance(world_pos, scene.camera_pos.xyz);
    for (var oy = -3; oy <= 3; oy = oy + 1) {
        for (var ox = -3; ox <= 3; ox = ox + 1) {
            let sc = clamp(coord + vec2<i32>(ox, oy), vec2<i32>(0), dims - vec2<i32>(1));
            let d = textureLoad(scene_depth_tex, sc, 0);
            if d >= 0.999999 {
                continue;
            }
            let scene_world = scene_world_from_depth(sc, dims_u, d);
            let gap = abs(distance(scene_world, scene.camera_pos.xyz) - water_dist);
            let px = length(vec2<f32>(f32(ox), f32(oy)));
            let fade = 1.0 - smoothstep(0.0, 3.8, px);
            edge = max(edge, (1.0 - smoothstep(0.02, 0.72, gap)) * fade);
            core = max(core, (1.0 - smoothstep(0.005, 0.14, gap)) * fade);
        }
    }
    return vec2<f32>(edge, core) * blend;
}

@fragment
fn fs_voxel_water(in: Out) -> @location(0) vec4<f32> {
    let v = voxel_waters[in.voxel_idx];
    let w = waters[in.water_idx];
    let cam_local = water_world_to_local(w, scene.camera_pos.xyz);
    let ray_dir = normalize(in.local_pos - cam_local);
    let bmin = vec3<f32>(-v.size_depth_voxel.x * 0.5, -v.size_depth_voxel.y, -v.size_depth_voxel.z * 0.5);
    let bmax = vec3<f32>( v.size_depth_voxel.x * 0.5,  v.size_depth_voxel.w * 0.08,  v.size_depth_voxel.z * 0.5);
    let hit = ray_box(cam_local, ray_dir, bmin, bmax);
    if hit.y <= max(hit.x, 0.0) {
        discard;
    }

    var t = max(hit.x, 0.0);
    let ray_len = hit.y - t;
    let steps = 64u;
    let step_len = ray_len / f32(steps);
    let jitter = fract(sin(dot(in.clip_pos.xy, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    t += step_len * jitter;

    var trans = 1.0;
    var rgb = vec3<f32>(0.0);
    var foam_acc = 0.0;
    var normal_acc = vec3<f32>(0.0, 1.0, 0.0);
    var hit_world = in.world_pos;
    for (var i = 0u; i < steps; i = i + 1u) {
        if t > hit.y || trans < 0.03 {
            break;
        }
        let p = cam_local + ray_dir * t;
        let uvw = uvw_from_local(v, p);
        if all(uvw >= vec3<f32>(0.0)) && all(uvw <= vec3<f32>(1.0)) {
            let d = trilinear_sample(in.voxel_idx, uvw, false).x;
            let s = trilinear_sample(in.voxel_idx, uvw, true);
            let top_fade = 1.0 - smoothstep(0.10, 0.36, uvw.y);
            let fluid = smoothstep(0.16, 0.72, d) * top_fade;
            let surface_density = smoothstep(0.10, 0.55, s.x) * fluid;
            let alpha = clamp((fluid * 0.10 + surface_density * 0.18) * step_len / max(v.size_depth_voxel.w, 0.02), 0.0, 0.22);
            let depth_t = clamp(uvw.y, 0.0, 1.0);
            let base = mix(w.shallow_color.rgb, w.deep_color.rgb, depth_t);
            rgb += base * alpha * trans;
            foam_acc = max(foam_acc, surface_density * w.visual1.z * 0.38);
            normal_acc = normalize(normal_acc + s.yzw * surface_density);
            hit_world = (water_model(w) * vec4<f32>(p, 1.0)).xyz;
            trans *= 1.0 - alpha;
        }
        t += step_len;
    }

    let alpha_out = clamp(1.0 - trans, 0.0, 1.0);
    if alpha_out <= 0.01 {
        discard;
    }
    var color = rgb / max(alpha_out, 0.001);
    let contact = screen_contact(hit_world, v.render.x);
    let view_dir = normalize(scene.camera_pos.xyz - hit_world);
    let fresnel = pow(1.0 - clamp(dot(normalize(normal_acc), view_dir), 0.0, 1.0), max(w.visual0.w, 1.0));
    let foam = clamp(foam_acc + contact.x * w.coastline.x * 0.42 + contact.y * 0.18, 0.0, 1.0);
    let foam_rgb = mix(w.coastline_foam_color.rgb, w.foam_color.rgb, w.foam_color.a);
    color = mix(color, foam_rgb, smoothstep(0.18, 0.86, foam) * 0.36);
    color = mix(color, w.sky_color_bias.rgb, fresnel * w.visual0.y * 0.22);
    return vec4<f32>(color, clamp(alpha_out + foam * 0.12 + fresnel * 0.08, 0.0, 1.0));
}
