
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

@group(0) @binding(0)
var<storage, read> waters: array<Water>;
@group(0) @binding(1)
var<storage, read> cells: array<vec4<f32>>;
@group(0) @binding(2)
var<uniform> params: Params;
@group(0) @binding(3)
var<storage, read> coastline_cells: array<vec4<f32>>;
struct WaterRenderChunk {
    water_idx: u32,
    render_width: u32,
    render_height: u32,
    flags: u32,
    uv_origin: vec2<f32>,
    uv_scale: vec2<f32>,
}
@group(0) @binding(4)
var<storage, read> render_chunks: array<WaterRenderChunk>;
@group(1) @binding(0)
var<uniform> scene: Scene3D;
@group(2) @binding(0)
var scene_depth_tex: texture_depth_2d;

struct Water3DVertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) water_idx: u32,
    @location(2) world_pos: vec3<f32>,
    @location(3) side_t: f32,
}

fn water_shape_alpha(w: Water, uv: vec2<f32>) -> f32 {
    let local = (uv - vec2<f32>(0.5, 0.5)) * w.size_depth_time.xy;
    if w.shape.x < 0.5 {
        return 1.0;
    }
    let r = w.shape.y;
    if dot(local, local) <= r * r {
        return 1.0;
    }
    return 0.0;
}

fn water_cell(w: Water, uv: vec2<f32>) -> vec4<f32> {
    if w.sim.y == 0u {
        return vec4<f32>(0.0);
    }
    let width = max(w.sim.z, 1u);
    let height = max(w.sim.w, 1u);
    let p = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * vec2<f32>(f32(max(width - 1u, 1u)), f32(max(height - 1u, 1u)));
    let x0 = u32(floor(p.x));
    let y0 = u32(floor(p.y));
    let x1 = min(x0 + 1u, width - 1u);
    let y1 = min(y0 + 1u, height - 1u);
    let f = fract(p);
    let tx = f.x * f.x * (3.0 - 2.0 * f.x);
    let ty = f.y * f.y * (3.0 - 2.0 * f.y);
    let i00 = min(y0 * width + x0, w.sim.y - 1u);
    let i10 = min(y0 * width + x1, w.sim.y - 1u);
    let i01 = min(y1 * width + x0, w.sim.y - 1u);
    let i11 = min(y1 * width + x1, w.sim.y - 1u);
    let a = mix(cells[w.sim.x + i00], cells[w.sim.x + i10], tx);
    let b = mix(cells[w.sim.x + i01], cells[w.sim.x + i11], tx);
    return mix(a, b, ty) * w.model_x.w;
}

fn water_coast_solid(w: Water, uv: vec2<f32>) -> bool {
    if w.sim.y == 0u {
        return false;
    }
    let width = max(w.sim.z, 1u);
    let height = max(w.sim.w, 1u);
    let p = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * vec2<f32>(f32(max(width - 1u, 1u)), f32(max(height - 1u, 1u)));
    let x0 = u32(floor(p.x));
    let y0 = u32(floor(p.y));
    let x1 = min(x0 + 1u, width - 1u);
    let y1 = min(y0 + 1u, height - 1u);
    let tx = fract(p.x);
    let ty = fract(p.y);
    let i00 = min(y0 * width + x0, w.sim.y - 1u);
    let i10 = min(y0 * width + x1, w.sim.y - 1u);
    let i01 = min(y1 * width + x0, w.sim.y - 1u);
    let i11 = min(y1 * width + x1, w.sim.y - 1u);
    let a = mix(coastline_cells[w.sim.x + i00].x, coastline_cells[w.sim.x + i10].x, tx);
    let b = mix(coastline_cells[w.sim.x + i01].x, coastline_cells[w.sim.x + i11].x, tx);
    return mix(a, b, ty) > 0.985;
}

fn water_coast_sample(w: Water, uv: vec2<f32>) -> vec4<f32> {
    if w.sim.y == 0u {
        return vec4<f32>(0.0);
    }
    let width = max(w.sim.z, 1u);
    let height = max(w.sim.w, 1u);
    let p = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * vec2<f32>(f32(max(width - 1u, 1u)), f32(max(height - 1u, 1u)));
    let x0 = u32(floor(p.x));
    let y0 = u32(floor(p.y));
    let x1 = min(x0 + 1u, width - 1u);
    let y1 = min(y0 + 1u, height - 1u);
    let tx = fract(p.x);
    let ty = fract(p.y);
    let i00 = min(y0 * width + x0, w.sim.y - 1u);
    let i10 = min(y0 * width + x1, w.sim.y - 1u);
    let i01 = min(y1 * width + x0, w.sim.y - 1u);
    let i11 = min(y1 * width + x1, w.sim.y - 1u);
    let a = mix(coastline_cells[w.sim.x + i00], coastline_cells[w.sim.x + i10], tx);
    let b = mix(coastline_cells[w.sim.x + i01], coastline_cells[w.sim.x + i11], tx);
    return mix(a, b, ty);
}

fn water_surface_height(w: Water, uv: vec2<f32>) -> f32 {
    let ripple = water_cell(w, uv);
    return ripple.x + ripple.y * 0.72;
}

fn water_ridge_wave(v: f32) -> f32 {
    let s = sin(v);
    return pow(max(s, 0.0), 8.0) - pow(max(-s, 0.0), 0.78) * 0.18;
}

fn water_visual_wave_height(w: Water, uv: vec2<f32>, t: f32) -> f32 {
    let local = (uv - vec2<f32>(0.5, 0.5)) * w.size_depth_time.xy;
    let wind = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.zw, length(w.flow_wind.zw) > 0.0001));
    let cross = vec2<f32>(-wind.y, wind.x);
    let ripple_scale = max(w.visual1.y, 0.001);
    let p = local / max(ripple_scale, 0.001);
    let break_n = water_fbm(local * 0.16 + vec2<f32>(t * 0.08, -t * 0.05));
    let shard = water_fbm(local * 0.075 + vec2<f32>(3.7, 9.2)) * 6.2831853;
    let break_dir = normalize(vec2<f32>(
        cos(shard + water_fbm(local * 0.11 + vec2<f32>(11.0, 2.0)) * 1.2),
        sin(shard + water_fbm(local * 0.09 + vec2<f32>(5.0, 19.0)) * 1.2),
    ));
    let a = water_ridge_wave(dot(p, wind) * 0.42 + t * w.wave.x * 1.9 + break_n * 1.6);
    let b = water_ridge_wave(dot(p, cross) * 0.86 - t * w.wave.x * 2.6 + 1.4 + dot(local, break_dir) * 0.15);
    let c = water_ridge_wave((p.x * 0.58 + p.y * 0.35) + t * w.wave.x * 3.6 + shard * 0.44);
    let d = water_ridge_wave(dot(p, break_dir) * 1.42 - t * w.wave.x * 3.2 + break_n * 2.8);
    let diag_a = water_ridge_wave(dot(p, normalize(vec2<f32>(0.72, 0.69))) * 1.05 + t * w.wave.x * 2.9 + break_n);
    let diag_b = water_ridge_wave(dot(p, normalize(vec2<f32>(-0.64, 0.77))) * 1.24 - t * w.wave.x * 2.4 + shard * 0.27);
    let cut_n = water_ridged_fbm(local * 0.28 + vec2<f32>(t * 0.04, -t * 0.03));
    let fracture = (break_n - 0.5) * 0.14 + (cut_n - 0.5) * 0.10;
    let ridge = a * 0.32 + b * 0.20 + c * 0.14 + d * 0.15 + diag_a * 0.12 + diag_b * 0.11 + fracture;
    let snap = sign(ridge) * pow(abs(ridge), 1.46);
    return snap * w.wave.y * 0.24 * clamp(w.visual1.x, 0.0, 1.4);
}

fn water_height_visual(w: Water, uv: vec2<f32>, t: f32) -> f32 {
    return water_surface_height(w, uv) + water_visual_wave_height(w, uv, t);
}

fn water_height_geometry(w: Water, uv: vec2<f32>, t: f32) -> f32 {
    let du = vec2<f32>(1.0 / max(f32(w.flags.x), 2.0), 0.0);
    let dv = vec2<f32>(0.0, 1.0 / max(f32(w.flags.y), 2.0));
    let center = water_surface_height(w, uv);
    let neighbor_avg = (
        water_surface_height(w, uv - du)
        + water_surface_height(w, uv + du)
        + water_surface_height(w, uv - dv)
        + water_surface_height(w, uv + dv)
    ) * 0.25;
    let smoothed = mix(center, neighbor_avg, 0.86);
    return smoothed * 0.35 + water_analytic_wave(w, uv, t, false).x * 0.22;
}

fn water_visual_normal(w: Water, uv: vec2<f32>, t: f32) -> vec3<f32> {
    let du = vec2<f32>(1.0 / max(f32(w.flags.x), 2.0), 0.0);
    let dv = vec2<f32>(0.0, 1.0 / max(f32(w.flags.y), 2.0));
    let h_l = water_height_visual(w, uv - du, t);
    let h_r = water_height_visual(w, uv + du, t);
    let h_d = water_height_visual(w, uv - dv, t);
    let h_u = water_height_visual(w, uv + dv, t);
    let sx = max(w.size_depth_time.x * du.x * 2.0, 0.001);
    let sz = max(w.size_depth_time.y * dv.y * 2.0, 0.001);
    let normal_scale = clamp(w.visual1.x, 0.0, 1.35) * 0.62;
    return normalize(vec3<f32>((h_l - h_r) * normal_scale, (sx + sz) * 1.18, (h_d - h_u) * normal_scale));
}

fn water_visual_normal_fast(w: Water, uv: vec2<f32>) -> vec3<f32> {
    let du = vec2<f32>(1.0 / max(f32(w.flags.x), 2.0), 0.0);
    let dv = vec2<f32>(0.0, 1.0 / max(f32(w.flags.y), 2.0));
    let h_l = water_surface_height(w, uv - du);
    let h_r = water_surface_height(w, uv + du);
    let h_d = water_surface_height(w, uv - dv);
    let h_u = water_surface_height(w, uv + dv);
    let sx = max(w.size_depth_time.x * du.x * 2.0, 0.001);
    let sz = max(w.size_depth_time.y * dv.y * 2.0, 0.001);
    let normal_scale = clamp(w.visual1.x, 0.0, 1.1) * 0.42;
    return normalize(vec3<f32>((h_l - h_r) * normal_scale, (sx + sz) * 1.12, (h_d - h_u) * normal_scale));
}

fn water_hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn water_hash2(p: vec2<f32>) -> vec2<f32> {
    let x = fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
    let y = fract(sin(dot(p, vec2<f32>(269.5, 183.3))) * 43758.5453);
    return vec2<f32>(x, y);
}

fn water_grad2(p: vec2<f32>) -> vec2<f32> {
    let h = water_hash2(p) * 2.0 - 1.0;
    let len2 = max(dot(h, h), 1.0e-4);
    return h * inverseSqrt(len2);
}

fn water_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let a = water_hash(i);
    let b = water_hash(i + vec2<f32>(1.0, 0.0));
    let c = water_hash(i + vec2<f32>(0.0, 1.0));
    let d = water_hash(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn water_perlin_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let g00 = water_grad2(i);
    let g10 = water_grad2(i + vec2<f32>(1.0, 0.0));
    let g01 = water_grad2(i + vec2<f32>(0.0, 1.0));
    let g11 = water_grad2(i + vec2<f32>(1.0, 1.0));
    let a = dot(g00, f - vec2<f32>(0.0, 0.0));
    let b = dot(g10, f - vec2<f32>(1.0, 0.0));
    let c = dot(g01, f - vec2<f32>(0.0, 1.0));
    let d = dot(g11, f - vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y) * 0.5 + 0.5;
}

fn water_fbm(p: vec2<f32>) -> f32 {
    var q = p;
    var amp = 0.52;
    var sum = 0.0;
    var norm = 0.0;
    for (var i = 0u; i < 4u; i = i + 1u) {
        sum += water_noise(q) * amp;
        norm += amp;
        q = vec2<f32>(q.x * 1.72 + q.y * 1.11, q.x * -1.04 + q.y * 1.83) + vec2<f32>(17.0, 9.0);
        amp *= 0.52;
    }
    return sum / max(norm, 0.001);
}

fn water_perlin_fbm(p: vec2<f32>) -> f32 {
    var q = p;
    var amp = 0.54;
    var sum = 0.0;
    var norm = 0.0;
    for (var i = 0u; i < 5u; i = i + 1u) {
        sum += water_perlin_noise(q) * amp;
        norm += amp;
        q = vec2<f32>(q.x * 1.84 - q.y * 0.74, q.x * 0.92 + q.y * 1.67) + vec2<f32>(7.0, 19.0);
        amp *= 0.53;
    }
    return sum / max(norm, 0.001);
}

fn water_ridged_fbm(p: vec2<f32>) -> f32 {
    var q = p;
    var amp = 0.56;
    var sum = 0.0;
    var norm = 0.0;
    for (var i = 0u; i < 4u; i = i + 1u) {
        let n = abs(water_noise(q) * 2.0 - 1.0);
        sum += (1.0 - n) * amp;
        norm += amp;
        q = vec2<f32>(q.x * 1.91 - q.y * 0.73, q.x * 0.86 + q.y * 1.68) + vec2<f32>(4.0, 23.0);
        amp *= 0.50;
    }
    return sum / max(norm, 0.001);
}

fn water_hex_noise(p: vec2<f32>) -> f32 {
    let rot_a = vec2<f32>(p.x * 0.5 - p.y * 0.8660254, p.x * 0.8660254 + p.y * 0.5);
    let rot_b = vec2<f32>(p.x * 0.5 + p.y * 0.8660254, -p.x * 0.8660254 + p.y * 0.5);
    let a = water_noise(p);
    let b = water_noise(rot_a + vec2<f32>(11.0, 7.0));
    let c = water_noise(rot_b + vec2<f32>(23.0, 17.0));
    let cell = water_noise(p * 0.19 + vec2<f32>(3.0, 5.0));
    return mix((a + b + c) * 0.33333334, max(a, max(b, c)), 0.22 + cell * 0.18);
}

fn water_hex_ridged_fbm(p: vec2<f32>) -> f32 {
    var q = p;
    var amp = 0.58;
    var sum = 0.0;
    var norm = 0.0;
    for (var i = 0u; i < 4u; i = i + 1u) {
        let n = abs(water_hex_noise(q) * 2.0 - 1.0);
        sum += (1.0 - n) * amp;
        norm += amp;
        q = vec2<f32>(q.x * 1.73 - q.y * 0.94, q.x * 0.88 + q.y * 1.61) + vec2<f32>(13.0, 29.0);
        amp *= 0.52;
    }
    return sum / max(norm, 0.001);
}

fn water_line_layer(p: vec2<f32>, dir: vec2<f32>, t: f32, scale: f32) -> vec3<f32> {
    let n = water_hex_noise(p * 0.21 + dir * t * 0.08);
    let q = dot(p, dir) * scale + n * 1.35 + t * 0.18;
    let band = abs(fract(q) - 0.5) * 2.0;
    let line = 1.0 - smoothstep(0.035, 0.13, band);
    let broken =
        smoothstep(0.30, 0.74, water_hex_noise(p * 0.53 + vec2<f32>(t * 0.05, -t * 0.04)));
    let dark = smoothstep(0.62, 0.95, band)
        * smoothstep(0.42, 0.88, water_hex_noise(p * 0.12 - dir * t * 0.03));
    return vec3<f32>(line * broken, dark, n);
}

fn water_schlick_fresnel(cos_theta: f32, power: f32) -> f32 {
    let f0 = 0.028;
    let grazing = 1.0 - clamp(cos_theta, 0.0, 1.0);
    let edge = pow(grazing, max(power * 0.72, 0.001));
    let shoulder = smoothstep(0.12, 0.78, grazing);
    return f0 + (1.0 - f0) * (edge * 0.88 + shoulder * 0.26);
}

fn water_scene_world_from_depth(coord: vec2<i32>, dims_u: vec2<u32>, depth: f32) -> vec3<f32> {
    let uv = (vec2<f32>(coord) + vec2<f32>(0.5)) / vec2<f32>(dims_u);
    let ndc_xy = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let ndc = vec4<f32>(ndc_xy, depth * 2.0 - 1.0, 1.0);
    let world_h = scene.inv_view_proj * ndc;
    return world_h.xyz / max(abs(world_h.w), 1.0e-5);
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

fn water_uv_from_local(w: Water, local: vec3<f32>) -> vec2<f32> {
    return local.xz / max(w.size_depth_time.xy, vec2<f32>(0.001)) + vec2<f32>(0.5, 0.5);
}

fn water_wave_layer(local: vec2<f32>, dir: vec2<f32>, wavelength: f32, amp: f32, speed: f32, steep: f32, t: f32) -> vec4<f32> {
    let d = normalize(select(vec2<f32>(1.0, 0.0), dir, length(dir) > 0.0001));
    let k = 6.2831853 / max(wavelength, 0.001);
    let phase = dot(local, d) * k + t * speed;
    let s = sin(phase);
    let c = cos(phase);
    let sharp = sign(s) * pow(abs(s), mix(1.0, 1.75, clamp(steep, 0.0, 1.0)));
    let height = mix(s, sharp, clamp(steep, 0.0, 1.0)) * amp;
    let deriv = c * amp * k;
    return vec4<f32>(height, deriv * d.x, deriv * d.y, abs(sharp));
}

fn water_analytic_wave(w: Water, uv: vec2<f32>, t: f32, detail: bool) -> vec4<f32> {
    if w.idle_mode == 0u {
        return vec4<f32>(0.0);
    }
    let local = (uv - vec2<f32>(0.5, 0.5)) * w.size_depth_time.xy;
    let wind = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.zw, length(w.flow_wind.zw) > 0.0001));
    let flow = normalize(select(wind, w.flow_wind.xy, length(w.flow_wind.xy) > 0.0001));
    let primary = select(wind, flow, w.idle_mode == 4u);
    let cross = vec2<f32>(-primary.y, primary.x);
    let len = max(w.wave_profile.x, 0.25);
    let scale = w.wave.y;
    let speed = w.wave.x * 0.22;
    var h = vec4<f32>(0.0);
    if w.idle_mode == 1u {
        h += water_wave_layer(local, wind, len, scale * 0.52, speed * 1.15, 0.12, t);
        h += water_wave_layer(local, cross, len * 0.58, scale * 0.18, -speed * 0.82, 0.05, t);
    } else if w.idle_mode == 2u {
        h += water_wave_layer(local, wind, len * 0.92, scale * 0.58, speed * 1.18, 0.36, t);
        h += water_wave_layer(local, cross, len * 0.46, scale * 0.22, -speed * 1.44, 0.22, t);
        h += water_wave_layer(local, normalize(wind + cross * 0.55), len * 0.34, scale * 0.13, speed * 1.92, 0.30, t);
    } else if w.idle_mode == 3u {
        h += water_wave_layer(local, wind, len * 1.42, scale * 0.86, speed * 0.88, 0.48, t);
        h += water_wave_layer(local, cross, len * 0.72, scale * 0.34, -speed * 1.25, 0.34, t);
        h += water_wave_layer(local, normalize(wind * 0.45 - cross), len * 0.38, scale * 0.22, speed * 1.82, 0.42, t);
    } else {
        h += water_wave_layer(local, flow, len * 0.38, scale * 0.44, -speed * 4.4, 0.30, t);
        h += water_wave_layer(local, flow, len * 0.20, scale * 0.18, -speed * 7.8, 0.18, t);
        h += water_wave_layer(local, cross, len * 0.72, scale * 0.08, -speed * 1.2, 0.06, t);
    }
    if detail {
        let p = local / max(w.visual1.y, 0.001);
        let n0 = water_perlin_fbm(p * 0.18 + primary * t * select(0.025, 0.085, w.idle_mode == 4u));
        let n1 = water_ridged_fbm(p * 0.42 - cross * t * 0.040 - primary * t * select(0.0, 0.11, w.idle_mode == 4u));
        let micro = (n0 - 0.5) * scale * 0.075 + (n1 - 0.5) * scale * 0.045;
        h.x += micro * clamp(w.visual1.x, 0.0, 1.6);
        h.w += abs(micro);
    }
    return h;
}

fn water_analytic_normal(w: Water, uv: vec2<f32>, t: f32) -> vec3<f32> {
    let wave = water_analytic_wave(w, uv, t, true);
    let ripple = water_cell(w, uv).x;
    let du = vec2<f32>(1.0 / max(f32(w.sim.z), 2.0), 0.0);
    let dv = vec2<f32>(0.0, 1.0 / max(f32(w.sim.w), 2.0));
    let ripple_dx = (water_cell(w, uv + du).x - water_cell(w, uv - du).x)
        / max(w.size_depth_time.x * du.x * 2.0, 0.001);
    let ripple_dz = (water_cell(w, uv + dv).x - water_cell(w, uv - dv).x)
        / max(w.size_depth_time.y * dv.y * 2.0, 0.001);
    let strength = clamp(w.visual1.x, 0.0, 1.6);
    return normalize(vec3<f32>(
        -(wave.y + ripple_dx * 0.10) * strength,
        1.0 + ripple * 0.02,
        -(wave.z + ripple_dz * 0.10) * strength,
    ));
}

fn water_depth_thickness(in: Water3DVertexOut, normal: vec3<f32>) -> f32 {
    let dims_u = textureDimensions(scene_depth_tex);
    let dims = vec2<i32>(i32(dims_u.x), i32(dims_u.y));
    let offset = vec2<i32>(round(normal.xz * clamp(waters[in.water_idx].visual2.y * 18.0, 0.0, 32.0)));
    let coord = clamp(vec2<i32>(floor(in.clip_pos.xy)) + offset, vec2<i32>(0), dims - vec2<i32>(1));
    let scene_depth = textureLoad(scene_depth_tex, coord, 0);
    if scene_depth >= 0.999999 {
        return waters[in.water_idx].size_depth_time.z;
    }
    let scene_world = water_scene_world_from_depth(coord, dims_u, scene_depth);
    let view_water = distance(in.world_pos, scene.camera_pos.xyz);
    let view_scene = distance(scene_world, scene.camera_pos.xyz);
    return max(view_scene - view_water, 0.0);
}

fn water_surface_contact_foam(in: Water3DVertexOut, normal: vec3<f32>, t: f32) -> f32 {
    let dims_u = textureDimensions(scene_depth_tex);
    let dims = vec2<i32>(i32(dims_u.x), i32(dims_u.y));
    let coord = vec2<i32>(floor(in.clip_pos.xy));
    if any(coord < vec2<i32>(0)) || any(coord >= dims) {
        return 0.0;
    }
    let w = waters[in.water_idx];
    let current_local = water_world_to_local(w, in.world_pos);
    let surface_y = water_height_geometry(w, in.uv, t);
    let band = max(0.12, min(w.size_depth_time.z * 0.040, 0.34));
    let cell_world = length(w.size_depth_time.xy / max(vec2<f32>(f32(max(w.flags.x - 1u, 1u)), f32(max(w.flags.y - 1u, 1u))), vec2<f32>(1.0)));
    let radius_world = max(cell_world * 0.85, 0.22);
    var contact = 0.0;
    for (var oy = -2; oy <= 2; oy = oy + 1) {
        for (var ox = -2; ox <= 2; ox = ox + 1) {
            let sample_coord = clamp(coord + vec2<i32>(ox, oy), vec2<i32>(0), dims - vec2<i32>(1));
            let scene_depth = textureLoad(scene_depth_tex, sample_coord, 0);
            if scene_depth >= 0.999999 {
                continue;
            }
            let scene_world = water_scene_world_from_depth(sample_coord, dims_u, scene_depth);
            let scene_local = water_world_to_local(w, scene_world);
            let scene_uv = water_uv_from_local(w, scene_local);
            if water_shape_alpha(w, scene_uv) <= 0.0 {
                continue;
            }
            let signed_y = scene_local.y - surface_y;
            let y_delta = abs(signed_y);
            if y_delta > band {
                continue;
            }
            let xz_delta = distance(scene_local.xz, current_local.xz);
            if xz_delta > radius_world {
                continue;
            }
            let height_lock = 1.0 - smoothstep(band * 0.18, band, y_delta);
            let width_lock = 1.0 - smoothstep(radius_world * 0.15, radius_world, xz_delta);
            let pixel_lock = 1.0 - smoothstep(0.0, 2.45, length(vec2<f32>(f32(ox), f32(oy))));
            contact = max(contact, height_lock * width_lock * (0.68 + pixel_lock * 0.32));
        }
    }
    let local = current_local.xz;
    let flow = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.xy + w.flow_wind.zw * 0.35, length(w.flow_wind.xy + w.flow_wind.zw * 0.35) > 0.0001));
    let lace = water_ridged_fbm(local * 2.7 + flow * t * 0.32 + normal.xz * 1.6);
    let holes = water_perlin_fbm(local * 6.4 - flow * t * 0.54 + vec2<f32>(7.0, 13.0));
    let broken = smoothstep(0.18, 0.92, lace) * (1.0 - smoothstep(0.72, 0.98, holes));
    return clamp(contact * (0.58 + broken * 0.92), 0.0, 1.0);
}

struct WaterVertexLocal {
    position: vec3<f32>,
    uv: vec2<f32>,
    side_t: f32,
    valid: bool,
}

fn water_surface_vertex(w: Water, vertex_idx: u32) -> WaterVertexLocal {
    let width = max(w.flags.x, 1u);
    let height = max(w.flags.y, 1u);
    let quad_width = width - 1u;
    let quad_height = height - 1u;
    let quad_count = quad_width * quad_height;
    let cell = vertex_idx / 6u;
    if w.sim.y == 0u || quad_count == 0u || cell >= quad_count {
        return WaterVertexLocal(vec3<f32>(0.0), vec2<f32>(0.0), 0.0, false);
    }
    var corner = array<vec2<u32>, 6>(
        vec2<u32>(0u, 0u),
        vec2<u32>(1u, 1u),
        vec2<u32>(1u, 0u),
        vec2<u32>(0u, 0u),
        vec2<u32>(0u, 1u),
        vec2<u32>(1u, 1u),
    );
    let cx = cell % quad_width;
    let cy = cell / quad_width;
    let c = corner[vertex_idx % 6u];
    let uv = vec2<f32>(f32(cx + c.x) / f32(quad_width), f32(cy + c.y) / f32(quad_height));
    let pos = vec3<f32>(
        (uv.x - 0.5) * w.size_depth_time.x,
        water_height_geometry(w, uv, params.time_seconds),
        (uv.y - 0.5) * w.size_depth_time.y,
    );
    return WaterVertexLocal(pos, uv, 0.0, true);
}

fn water_chunk_surface_vertex(w: Water, chunk: WaterRenderChunk, vertex_idx: u32) -> WaterVertexLocal {
    let width = max(chunk.render_width, 2u);
    let height = max(chunk.render_height, 2u);
    let quad_width = width - 1u;
    let quad_height = height - 1u;
    let quad_count = quad_width * quad_height;
    let cell = vertex_idx / 6u;
    if w.sim.y == 0u || quad_count == 0u || cell >= quad_count {
        return WaterVertexLocal(vec3<f32>(0.0), vec2<f32>(0.0), 0.0, false);
    }
    var corner = array<vec2<u32>, 6>(
        vec2<u32>(0u, 0u),
        vec2<u32>(1u, 1u),
        vec2<u32>(1u, 0u),
        vec2<u32>(0u, 0u),
        vec2<u32>(0u, 1u),
        vec2<u32>(1u, 1u),
    );
    let cx = cell % quad_width;
    let cy = cell / quad_width;
    let c = corner[vertex_idx % 6u];
    let local_uv = vec2<f32>(f32(cx + c.x) / f32(quad_width), f32(cy + c.y) / f32(quad_height));
    let uv = chunk.uv_origin + local_uv * chunk.uv_scale;
    let pos = vec3<f32>(
        (uv.x - 0.5) * w.size_depth_time.x,
        water_height_geometry(w, uv, params.time_seconds),
        (uv.y - 0.5) * w.size_depth_time.y,
    );
    return WaterVertexLocal(pos, uv, 0.0, true);
}

fn water_rect_side_vertex(w: Water, side_idx: u32) -> WaterVertexLocal {
    let width = max(w.flags.x, 1u);
    let height = max(w.flags.y, 1u);
    let horizontal_segments = width - 1u;
    let vertical_segments = height - 1u;
    let side_count = horizontal_segments * 2u + vertical_segments * 2u;
    let cell = side_idx / 6u;
    if side_count == 0u || cell >= side_count {
        return WaterVertexLocal(vec3<f32>(0.0), vec2<f32>(0.0), 1.0, false);
    }
    var corner = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );
    let c = corner[side_idx % 6u];
    let top_t = 1.0 - c.y;
    var uv = vec2<f32>(0.0, 0.0);

    if cell < horizontal_segments {
        let edge_t = (f32(cell) + c.x) / f32(horizontal_segments);
        uv = vec2<f32>(edge_t, 0.0);
    } else if cell < horizontal_segments + vertical_segments {
        let seg = cell - horizontal_segments;
        let edge_t = (f32(seg) + c.x) / f32(vertical_segments);
        uv = vec2<f32>(1.0, edge_t);
    } else if cell < horizontal_segments * 2u + vertical_segments {
        let seg = cell - horizontal_segments - vertical_segments;
        let edge_t = (f32(seg) + c.x) / f32(horizontal_segments);
        uv = vec2<f32>(1.0 - edge_t, 1.0);
    } else {
        let seg = cell - horizontal_segments * 2u - vertical_segments;
        let edge_t = (f32(seg) + c.x) / f32(vertical_segments);
        uv = vec2<f32>(0.0, 1.0 - edge_t);
    }
    let top = water_height_geometry(w, uv, params.time_seconds);
    let y = mix(-max(w.size_depth_time.z, 0.001), top, top_t);
    let pos = vec3<f32>(
        (uv.x - 0.5) * w.size_depth_time.x,
        y,
        (uv.y - 0.5) * w.size_depth_time.y,
    );
    return WaterVertexLocal(pos, uv, 1.0, true);
}

fn water_circle_counts(w: Water) -> vec2<u32> {
    let width = max(w.flags.x, 1u);
    let height = max(w.flags.y, 1u);
    let segments = clamp(max(width, height) * 4u, 16u, 512u);
    let rings = clamp(min(width, height) / 2u, 1u, 512u);
    return vec2<u32>(segments, rings);
}

fn water_circle_point(w: Water, angle_t: f32, radius_t: f32) -> WaterVertexLocal {
    let angle = angle_t * 6.2831853;
    let local_xz = vec2<f32>(cos(angle), sin(angle)) * w.shape.y * radius_t;
    let uv = local_xz / w.size_depth_time.xy + vec2<f32>(0.5, 0.5);
    let pos = vec3<f32>(local_xz.x, water_height_geometry(w, uv, params.time_seconds), local_xz.y);
    return WaterVertexLocal(pos, uv, 0.0, true);
}

fn water_circle_surface_vertex(w: Water, vertex_idx: u32) -> WaterVertexLocal {
    let counts = water_circle_counts(w);
    let segments = counts.x;
    let rings = counts.y;
    let cell = vertex_idx / 6u;
    if w.sim.y == 0u || cell >= segments * rings {
        return WaterVertexLocal(vec3<f32>(0.0), vec2<f32>(0.0), 0.0, false);
    }
    var corner = array<vec2<u32>, 6>(
        vec2<u32>(0u, 0u),
        vec2<u32>(1u, 1u),
        vec2<u32>(1u, 0u),
        vec2<u32>(0u, 0u),
        vec2<u32>(0u, 1u),
        vec2<u32>(1u, 1u),
    );
    let seg = cell % segments;
    let ring = cell / segments;
    let c = corner[vertex_idx % 6u];
    let angle_t = f32((seg + c.x) % segments) / f32(segments);
    let radius_t = f32(ring + c.y) / f32(rings);
    return water_circle_point(w, angle_t, radius_t);
}

fn water_circle_side_vertex(w: Water, side_idx: u32) -> WaterVertexLocal {
    let counts = water_circle_counts(w);
    let segments = counts.x;
    let side = side_idx / 6u;
    if side >= segments {
        return WaterVertexLocal(vec3<f32>(0.0), vec2<f32>(0.0), 1.0, false);
    }
    var corner = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );
    let c = corner[side_idx % 6u];
    let angle_t = (f32(side) + c.x) / f32(segments);
    let top = water_circle_point(w, angle_t, 1.0);
    let y = mix(-max(w.size_depth_time.z, 0.001), top.position.y, 1.0 - c.y);
    return WaterVertexLocal(vec3<f32>(top.position.x, y, top.position.z), top.uv, 1.0, true);
}

@vertex
fn vs_water_3d(
    @builtin(vertex_index) vertex_idx: u32,
    @builtin(instance_index) chunk_idx: u32,
) -> Water3DVertexOut {
    let chunk = render_chunks[chunk_idx];
    let water_idx = chunk.water_idx;
    let w = waters[water_idx];
    var surface_vertex_count = (max(chunk.render_width, 2u) - 1u) * (max(chunk.render_height, 2u) - 1u) * 6u;
    var local_vertex = water_chunk_surface_vertex(w, chunk, vertex_idx);
    if (chunk.flags & 2u) != 0u {
        let counts = water_circle_counts(w);
        surface_vertex_count = counts.x * counts.y * 6u;
        local_vertex = water_circle_surface_vertex(w, vertex_idx);
        if vertex_idx >= surface_vertex_count {
            local_vertex = water_circle_side_vertex(w, vertex_idx - surface_vertex_count);
        }
    } else if vertex_idx >= surface_vertex_count && (chunk.flags & 1u) != 0u {
        local_vertex = water_rect_side_vertex(w, vertex_idx - surface_vertex_count);
    } else if vertex_idx >= surface_vertex_count {
        local_vertex.valid = false;
    }
    let scaled = vec4<f32>(local_vertex.position, 1.0);
    let model = mat4x4<f32>(
        vec4<f32>(w.model_x.xyz, 0.0),
        vec4<f32>(w.model_y.xyz, 0.0),
        vec4<f32>(w.model_z.xyz, 0.0),
        w.model_w,
    );
    let world = model * scaled;

    var out: Water3DVertexOut;
    out.clip_pos = select(vec4<f32>(2.0, 2.0, 1.0, 1.0), scene.view_proj * world, local_vertex.valid);
    out.uv = local_vertex.uv;
    out.water_idx = water_idx;
    out.world_pos = world.xyz;
    out.side_t = local_vertex.side_t;
    return out;
}

@fragment
fn fs_water_3d(in: Water3DVertexOut) -> @location(0) vec4<f32> {
    let w = waters[in.water_idx];
    if in.side_t <= 0.5 && water_shape_alpha(w, in.uv) <= 0.0 {
        return vec4<f32>(0.0);
    }
    let t = params.time_seconds;
    let top_surface_mask = 1.0 - smoothstep(0.02, 0.18, in.side_t);
    let idle = sin((in.uv.x + in.uv.y + t * w.wave.x * 0.2) * 6.2831853) * 0.5 + 0.5;
    var ripple = water_cell(w, in.uv);
    if water_coast_solid(w, in.uv) {
        return vec4<f32>(0.0);
    }
    let view_dist = distance(scene.camera_pos.xyz, in.world_pos);
    let view_dir = normalize(scene.camera_pos.xyz - in.world_pos);
    let far_t = clamp((view_dist - 180.0) / 420.0, 0.0, 1.0);
    let normal = water_analytic_normal(w, in.uv, t);
    let fresnel = water_schlick_fresnel(dot(normal, view_dir), w.visual0.w) * w.visual0.y;
    let auto_shallow_depth = max(max(w.size_depth_time.x, w.size_depth_time.y) * 0.25, 0.001);
    let shallow_depth = select(auto_shallow_depth, max(w.size_depth_time.w, 0.001), w.size_depth_time.w >= 0.0);
    let scene_thickness = water_depth_thickness(in, normal);
    let depth_t = clamp(scene_thickness / shallow_depth, 0.0, 1.0);
    let contact_foam = water_surface_contact_foam(in, normal, t) * top_surface_mask;
    let local = (in.uv - vec2<f32>(0.5, 0.5)) * w.size_depth_time.xy;
    let world_uv = in.world_pos.xz;
    let wave_flow = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.zw + w.flow_wind.xy * 0.35, length(w.flow_wind.zw + w.flow_wind.xy * 0.35) > 0.0001));
    let analytic = water_analytic_wave(w, in.uv, t, true);
    let caustic_seed = water_fbm((local + normal.xz * 2.0) * 0.42 + wave_flow * t * 0.18);
    let caustic = smoothstep(0.62, 0.92, caustic_seed) * (1.0 - depth_t) * w.visual2.x;
    let sun_dir = normalize(select(vec3<f32>(0.0, 1.0, 0.0), -scene.ray_light.direction.xyz, length(scene.ray_light.direction.xyz) > 0.001));
    let scatter = (1.0 - depth_t) * w.visual2.z * max(dot(normal, sun_dir), 0.0);
    let basin = water_perlin_fbm(world_uv * mix(0.060, 0.034, far_t) + vec2<f32>(t * 0.012, -t * 0.008));
    let shoal = water_perlin_fbm(world_uv * mix(0.130, 0.075, far_t) + vec2<f32>(4.3, 8.1));
    let macro_break = water_ridged_fbm(world_uv * mix(0.095, 0.050, far_t) - wave_flow * t * 0.032 + normal.xz * 0.6);
    let lowlight_noise = water_perlin_fbm(world_uv * mix(0.18, 0.10, far_t) - wave_flow * t * 0.045 + vec2<f32>(3.0, 17.0));
    let highlight_noise = water_perlin_fbm(world_uv * mix(0.34, 0.21, far_t) + wave_flow * t * 0.14 + vec2<f32>(9.0, 2.0));
    let micro_break = water_ridged_fbm(world_uv * mix(0.52, 0.30, far_t) + wave_flow * t * 0.11 + vec2<f32>(14.0, 5.0));
    let dark_patch = smoothstep(0.42, 0.86, basin * 0.54 + lowlight_noise * 0.36 + macro_break * 0.22) * 0.28;
    let light_patch = smoothstep(0.50, 0.88, shoal * 0.46 + highlight_noise * 0.40 + micro_break * 0.20) * (1.0 - depth_t) * 0.46;
    let scratch_ripple = (highlight_noise - lowlight_noise) * 0.08 + (micro_break - 0.5) * 0.06;
    let shallow_t = clamp(1.0 - depth_t + idle * 0.04 + caustic * 0.12 + light_patch * 0.14 + contact_foam * 0.10 - dark_patch * 0.25, 0.0, 1.0);
    let surface_t = clamp(shallow_t + abs(ripple.x + analytic.x + scratch_ripple) * 0.08 + clamp(view_dist / 256.0, 0.0, 1.0) * 0.04, 0.0, 1.0);
    let depth_rgb = mix(w.deep_color.rgb * (0.74 - dark_patch * 0.18), w.deep_color.rgb, depth_t);
    let water_rgb = mix(depth_rgb, w.shallow_color.rgb + vec3<f32>(light_patch * 0.18), surface_t);
    let refract_tint = vec3<f32>(caustic * 0.22 + w.visual2.y * (1.0 - depth_t) * 0.08 + light_patch * 0.06);
    let reflected = mix(water_rgb, w.sky_color_bias.rgb, max(w.sky_color_bias.w, w.visual0.y * fresnel * 0.62));
    let rough_blend = clamp(w.visual0.z, 0.0, 1.0);
    let half_dir = normalize(view_dir + sun_dir);
    let spec_line = pow(max(dot(normal, half_dir), 0.0), mix(128.0, 36.0, rough_blend)) * 0.12 * w.visual0.y;
    let fresnel_tint = vec3<f32>(0.18, 0.24, 0.30) * fresnel * w.visual0.y;
    let lit_water = mix(reflected, water_rgb, rough_blend * 0.48) + refract_tint + scatter + fresnel_tint + vec3<f32>(spec_line + micro_break * 0.025) - vec3<f32>(dark_patch * 0.10);
    let fog_t = clamp(view_dist / 620.0, 0.0, 1.0) * w.visual2.w;
    let base_color = mix(lit_water, w.deep_color.rgb, fog_t);
    let foam_rgb = mix(vec3<f32>(0.92, 0.97, 1.0), w.foam_color.rgb, clamp(w.foam_color.a, 0.0, 1.0) * 0.35);
    let contact_aa = max(fwidth(contact_foam), 0.01);
    let contact_blend = smoothstep(0.045 - contact_aa, 0.50 + contact_aa, contact_foam);
    let color = mix(base_color, foam_rgb, contact_blend * 0.82);
    let alpha = mix(w.deep_color.a, w.shallow_color.a, shallow_t) * (1.0 - clamp(w.visual0.x, 0.0, 1.0) * 0.72);
    let side_color = mix(w.deep_color.rgb, color, 0.28);
    let final_color = mix(color, side_color, in.side_t);
    let final_alpha = mix(alpha + (fresnel * 0.10 + contact_blend * 0.14) * top_surface_mask, w.deep_color.a * 0.82, in.side_t);
    return vec4<f32>(final_color, clamp(final_alpha, 0.0, 1.0));
}
