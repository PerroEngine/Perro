
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

fn water_cell_smooth(w: Water, uv: vec2<f32>) -> vec4<f32> {
    let du = vec2<f32>(1.0 / max(f32(w.sim.z), 2.0), 0.0);
    let dv = vec2<f32>(0.0, 1.0 / max(f32(w.sim.w), 2.0));
    let center = water_cell(w, uv);
    let cross = (
        water_cell(w, uv - du)
        + water_cell(w, uv + du)
        + water_cell(w, uv - dv)
        + water_cell(w, uv + dv)
    ) * 0.25;
    let diag = (
        water_cell(w, uv - du - dv)
        + water_cell(w, uv + du - dv)
        + water_cell(w, uv - du + dv)
        + water_cell(w, uv + du + dv)
    ) * 0.25;
    return center * 0.46 + cross * 0.38 + diag * 0.16;
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
    return ripple.x + ripple.y * 0.045;
}

fn water_ridge_wave(v: f32) -> f32 {
    let s = sin(v);
    let swell = s * 0.58 + sin(v * 0.53 + 1.7) * 0.24;
    let crest = pow(max(s, 0.0), 5.0) * 0.34;
    return swell + crest - pow(max(-s, 0.0), 1.35) * 0.12;
}

fn water_visual_wave_height(w: Water, uv: vec2<f32>, t: f32) -> f32 {
    let local = (uv - vec2<f32>(0.5, 0.5)) * w.size_depth_time.xy;
    let wind = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.zw, length(w.flow_wind.zw) > 0.0001));
    let cross = vec2<f32>(-wind.y, wind.x);
    let ripple_scale = max(w.visual1.y, 0.001);
    let p = local / max(ripple_scale, 0.001);
    let break_n = water_fbm(local * 0.10 + vec2<f32>(t * 0.05, -t * 0.03));
    let shard = water_fbm(local * 0.052 + vec2<f32>(3.7, 9.2)) * 6.2831853;
    let break_dir = normalize(vec2<f32>(
        cos(shard + water_fbm(local * 0.11 + vec2<f32>(11.0, 2.0)) * 1.2),
        sin(shard + water_fbm(local * 0.09 + vec2<f32>(5.0, 19.0)) * 1.2),
    ));
    let a = water_ridge_wave(dot(p, wind) * 0.34 + t * w.wave.x * 1.5 + break_n * 0.85);
    let b = water_ridge_wave(dot(p, wind * 0.82 + cross * 0.18) * 0.62 + t * w.wave.x * 2.1 + shard * 0.16);
    let c = water_ridge_wave(dot(p, wind * 0.62 - cross * 0.38) * 1.05 - t * w.wave.x * 2.8 + break_n * 1.4);
    let chop = water_ridge_wave(dot(p, break_dir) * 1.55 - t * w.wave.x * 3.4 + shard * 0.22);
    let cut_n = water_ridged_fbm(local * 0.22 + vec2<f32>(t * 0.035, -t * 0.02));
    let ridge = a * 0.46 + b * 0.27 + c * 0.18 + chop * 0.09 + (cut_n - 0.5) * 0.035;
    return ridge * w.wave.y * 0.22 * clamp(w.visual1.x, 0.0, 1.4);
}

fn water_height_visual(w: Water, uv: vec2<f32>, t: f32) -> f32 {
    return water_surface_height(w, uv) + water_visual_wave_height(w, uv, t);
}

fn water_height_geometry(w: Water, uv: vec2<f32>, t: f32) -> f32 {
    let du = vec2<f32>(1.0 / max(f32(w.sim.z), 2.0), 0.0);
    let dv = vec2<f32>(0.0, 1.0 / max(f32(w.sim.w), 2.0));
    let center = water_surface_height(w, uv);
    let cross = (
        water_surface_height(w, uv - du)
        + water_surface_height(w, uv + du)
        + water_surface_height(w, uv - dv)
        + water_surface_height(w, uv + dv)
    ) * 0.25;
    let diag = (
        water_surface_height(w, uv - du - dv)
        + water_surface_height(w, uv + du - dv)
        + water_surface_height(w, uv - du + dv)
        + water_surface_height(w, uv + du + dv)
    ) * 0.25;
    let smoothed = center * 0.34 + cross * 0.46 + diag * 0.20;
    let analytic = water_analytic_wave(w, uv, t, false).x;
    return smoothed * 0.20 + analytic * 0.48;
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

fn water_foam_web(p: vec2<f32>) -> vec2<f32> {
    let ip = floor(p);
    let fp = fract(p);
    var d1 = 64.0;
    var d2 = 64.0;
    var id = 0.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let g = vec2<f32>(f32(x), f32(y));
            let h = water_hash2(ip + g);
            let jitter = 0.28 + h * 0.54;
            let r = g + jitter - fp;
            let d = dot(r, r);
            if d < d1 {
                d2 = d1;
                d1 = d;
                id = water_hash(ip + g);
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    let gap = sqrt(d2) - sqrt(d1);
    let edge = 1.0 - smoothstep(0.020, 0.105, gap);
    let broken = smoothstep(0.22, 0.82, water_hex_noise(p * 1.7 + vec2<f32>(id * 17.0, id * 31.0)));
    return vec2<f32>(edge * broken, id);
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
    let du = vec2<f32>(1.0 / max(f32(w.sim.z), 2.0), 0.0);
    let dv = vec2<f32>(0.0, 1.0 / max(f32(w.sim.w), 2.0));
    let center = water_surface_height(w, uv);
    let left = water_surface_height(w, uv - du);
    let right = water_surface_height(w, uv + du);
    let down = water_surface_height(w, uv - dv);
    let up = water_surface_height(w, uv + dv);
    let ripple = center * 0.50 + (left + right + down + up) * 0.125;
    let ripple_dx = (right - left)
        / max(w.size_depth_time.x * du.x * 2.0, 0.001);
    let ripple_dz = (up - down)
        / max(w.size_depth_time.y * dv.y * 2.0, 0.001);
    let strength = clamp(w.visual1.x, 0.0, 1.6);
    return normalize(vec3<f32>(
        -(wave.y + ripple_dx * 0.18) * strength,
        1.0 + ripple * 0.02,
        -(wave.z + ripple_dz * 0.18) * strength,
    ));
}

fn water_light_brdf(base: vec3<f32>, n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, radiance: vec3<f32>, roughness: f32, reflectivity: f32) -> vec3<f32> {
    let nl = max(dot(n, l), 0.0);
    if nl <= 0.0 {
        return vec3<f32>(0.0);
    }
    let h = normalize(v + l);
    let nv = max(dot(n, v), 0.0);
    let nh = max(dot(n, h), 0.0);
    let vh = max(dot(v, h), 0.0);
    let a = max(roughness * roughness, 0.002);
    let a2 = a * a;
    let d_denom = max(nh * nh * (a2 - 1.0) + 1.0, 0.001);
    let d = a2 / (3.14159265 * d_denom * d_denom);
    let k = (roughness + 1.0) * (roughness + 1.0) * 0.125;
    let gv = nv / max(nv * (1.0 - k) + k, 0.001);
    let gl = nl / max(nl * (1.0 - k) + k, 0.001);
    let f0 = mix(vec3<f32>(0.020), vec3<f32>(0.080), clamp(reflectivity, 0.0, 1.0));
    let f = f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - vh, 5.0);
    let spec = d * gv * gl * f / max(4.0 * nv * nl, 0.001);
    let diffuse = base * (1.0 - f) * (0.22 / 3.14159265);
    return (diffuse + spec) * radiance * nl;
}

fn water_scene_lighting(base: vec3<f32>, n: vec3<f32>, v: vec3<f32>, pos: vec3<f32>, roughness: f32, reflectivity: f32) -> vec3<f32> {
    var lit = base * scene.ambient_color.xyz * scene.ambient_color.w * (0.42 + 0.58 * max(n.y, 0.0));
    let ray_count = u32(scene.ambient_and_counts.x);
    for (var i = 0u; i < ray_count; i = i + 1u) {
        let ray = scene.ray_lights[i];
        let ray_dir = ray.direction.xyz;
        let l = -ray_dir * inverseSqrt(max(dot(ray_dir, ray_dir), 1.0e-8));
        let radiance = ray.color_intensity.xyz * ray.color_intensity.w;
        lit += water_light_brdf(base, n, v, l, radiance, roughness, reflectivity);
    }
    let point_count = u32(scene.ambient_and_counts.y);
    for (var i = 0u; i < point_count; i = i + 1u) {
        let light = scene.point_lights[i];
        let to_light = light.position_range.xyz - pos;
        let dist_sq = dot(to_light, to_light);
        let range_sq = light.position_range.w * light.position_range.w;
        if dist_sq <= range_sq {
            let l = to_light * inverseSqrt(max(dist_sq, 1.0e-8));
            let range_t = clamp(1.0 - dist_sq / max(range_sq, 0.001), 0.0, 1.0);
            let attenuation = range_t * range_t / max(dist_sq, 1.0);
            let radiance = light.color_intensity.xyz * light.color_intensity.w * attenuation;
            lit += water_light_brdf(base, n, v, l, radiance, roughness, reflectivity);
        }
    }
    let spot_count = u32(scene.ambient_and_counts.z);
    for (var i = 0u; i < spot_count; i = i + 1u) {
        let light = scene.spot_lights[i];
        let to_light = light.position_range.xyz - pos;
        let dist_sq = dot(to_light, to_light);
        let range_sq = light.position_range.w * light.position_range.w;
        if dist_sq <= range_sq {
            let l = to_light * inverseSqrt(max(dist_sq, 1.0e-8));
            let cos_theta = dot(light.direction_outer_cos.xyz, -l);
            let cone = clamp((cos_theta - light.direction_outer_cos.w) / max(light.inner_cos_pad.x - light.direction_outer_cos.w, 0.0001), 0.0, 1.0);
            let range_t = clamp(1.0 - dist_sq / max(range_sq, 0.001), 0.0, 1.0);
            let attenuation = cone * cone * range_t * range_t / max(dist_sq, 1.0);
            let radiance = light.color_intensity.xyz * light.color_intensity.w * attenuation;
            lit += water_light_brdf(base, n, v, l, radiance, roughness, reflectivity);
        }
    }
    return lit;
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
    let ripple_raw = water_cell(w, in.uv);
    var ripple = water_cell_smooth(w, in.uv);
    if water_coast_solid(w, in.uv) {
        return vec4<f32>(0.0);
    }
    let view_dist = distance(scene.camera_pos.xyz, in.world_pos);
    let view_dir = normalize(scene.camera_pos.xyz - in.world_pos);
    let far_t = smoothstep(140.0, 760.0, view_dist);
    let detail_lod = clamp(w.model_x.w, 0.0, 1.0);
    let normal = normalize(mix(water_analytic_normal(w, in.uv, t), water_visual_normal(w, in.uv, t), 0.42));
    let fresnel = water_schlick_fresnel(dot(normal, view_dir), w.visual0.w) * w.visual0.y;
    let auto_shallow_depth = max(max(w.size_depth_time.x, w.size_depth_time.y) * 0.25, 0.001);
    let shallow_depth = select(auto_shallow_depth, max(w.size_depth_time.w, 0.001), w.size_depth_time.w >= 0.0);
    let scene_thickness = water_depth_thickness(in, normal);
    let depth_t = clamp(scene_thickness / shallow_depth, 0.0, 1.0);
    let local = (in.uv - vec2<f32>(0.5, 0.5)) * w.size_depth_time.xy;
    let world_uv = in.world_pos.xz;
    let wave_flow = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.zw + w.flow_wind.xy * 0.35, length(w.flow_wind.zw + w.flow_wind.xy * 0.35) > 0.0001));
    let analytic = water_analytic_wave(w, in.uv, t, true);
    let caustic_seed = water_fbm((local + normal.xz * 2.0) * 0.42 + wave_flow * t * 0.18);
    let caustic = smoothstep(0.62, 0.92, caustic_seed) * (1.0 - depth_t) * w.visual2.x;
    let sun_dir = normalize(select(vec3<f32>(0.0, 1.0, 0.0), -scene.ray_light.direction.xyz, length(scene.ray_light.direction.xyz) > 0.001));
    let forward_light = pow(max(dot(-view_dir, sun_dir), 0.0), 2.0);
    let scatter = (1.0 - depth_t) * w.visual2.z * (0.18 + 0.42 * forward_light) * max(dot(normal, sun_dir), 0.0);
    let basin = water_perlin_fbm(world_uv * mix(0.050, 0.028, far_t) + vec2<f32>(t * 0.008, -t * 0.005));
    let shoal = water_perlin_fbm(world_uv * mix(0.095, 0.052, far_t) + vec2<f32>(4.3, 8.1));
    let macro_break = water_ridged_fbm(world_uv * mix(0.082, 0.045, far_t) - wave_flow * t * 0.026 + normal.xz * 0.38);
    let detail_amp = mix(0.32, 1.0, detail_lod);
    let lowlight_noise = water_perlin_fbm(world_uv * mix(0.11, 0.060, far_t) - wave_flow * t * 0.030 + vec2<f32>(3.0, 17.0));
    let highlight_noise = water_perlin_fbm(world_uv * mix(0.18, 0.095, far_t) + wave_flow * t * 0.080 + vec2<f32>(9.0, 2.0));
    let micro_break = water_ridged_fbm(world_uv * mix(0.26, 0.13, far_t) + wave_flow * t * 0.070 + vec2<f32>(14.0, 5.0));
    let wave_extrema = smoothstep(w.visual1.w + 0.18, w.visual1.w + 0.78, abs(analytic.x) + abs(ripple.x) * 0.12);
    let crest_slope = smoothstep(0.28, 0.78, length(normal.xz));
    let crest_side = smoothstep(0.16, 0.62, analytic.x) + smoothstep(0.18, 0.76, -analytic.x) * 0.34;
    let wind_coord = vec2<f32>(dot(local, wave_flow), dot(local, vec2<f32>(-wave_flow.y, wave_flow.x)));
    let web_a = water_foam_web(wind_coord * vec2<f32>(0.145, 0.230) + vec2<f32>(t * w.wave.x * 0.10, -t * 0.04));
    let web_b = water_foam_web(wind_coord * vec2<f32>(0.310, 0.480) + vec2<f32>(19.0, 7.0) - vec2<f32>(t * 0.06, t * 0.09));
    let lace = water_line_layer(wind_coord, vec2<f32>(1.0, 0.0), t, 0.060 + far_t * 0.020).x;
    let fine_spray = water_ridged_fbm(world_uv * mix(3.10, 1.35, far_t) + wave_flow * t * 0.46 + normal.xz * 3.5);
    let vein = max(web_a.x, web_b.x * 0.54);
    let web_break = smoothstep(0.50, 0.92, fine_spray * 0.38 + water_hex_noise(wind_coord * 0.42 + vec2<f32>(t * 0.05, -t * 0.03)) * 0.62);
    let crest_foam = wave_extrema * crest_slope * crest_side * (vein * 0.96 + lace * 0.32) * web_break * w.visual1.z;
    let wake_foam = smoothstep(0.70, 0.99, ripple.z) * smoothstep(0.20, 0.56, abs(ripple.x) + max(ripple.y, 0.0) * 0.016) * vein;
    let surface_foam = clamp((crest_foam + wake_foam * 0.14) * top_surface_mask, 0.0, 1.0);
    let dark_patch = smoothstep(0.48, 0.90, basin * 0.62 + lowlight_noise * 0.24 + macro_break * 0.14) * 0.16 * detail_amp;
    let light_patch = smoothstep(0.58, 0.93, shoal * 0.55 + highlight_noise * 0.28 + micro_break * 0.12) * (1.0 - depth_t) * 0.22 * detail_amp;
    let scratch_ripple = (highlight_noise - lowlight_noise) * 0.026 + (micro_break - 0.5) * 0.018;
    let shallow_t = smoothstep(0.0, 1.0, clamp(1.0 - depth_t + idle * 0.018 + caustic * 0.06 + light_patch * 0.08 + surface_foam * 0.018 - dark_patch * 0.12, 0.0, 1.0));
    let surface_t = smoothstep(0.0, 1.0, clamp(shallow_t + abs(ripple.x + analytic.x * 0.55 + scratch_ripple) * 0.040 + clamp(view_dist / 420.0, 0.0, 1.0) * 0.025, 0.0, 1.0));
    let depth_rgb = mix(w.deep_color.rgb * (0.86 - dark_patch * 0.08), w.deep_color.rgb, depth_t);
    let water_rgb = mix(depth_rgb, w.shallow_color.rgb + vec3<f32>(light_patch * 0.10), surface_t);
    let refract_tint = vec3<f32>(caustic * 0.16 + w.visual2.y * (1.0 - depth_t) * 0.055 + light_patch * 0.045);
    let reflected = mix(water_rgb, w.sky_color_bias.rgb, max(w.sky_color_bias.w, w.visual0.y * fresnel * 0.82));
    let rough_blend = clamp(w.visual0.z, 0.0, 1.0);
    let half_dir = normalize(view_dir + sun_dir);
    let spec_line = pow(max(dot(normal, half_dir), 0.0), mix(128.0, 36.0, rough_blend)) * 0.12 * w.visual0.y;
    let fresnel_tint = vec3<f32>(0.22, 0.30, 0.38) * fresnel * w.visual0.y;
    let ambient_strength = clamp(scene.ambient_color.w, 0.0, 4.0);
    let ambient_tint = mix(vec3<f32>(1.0), scene.ambient_color.xyz, clamp(ambient_strength * 0.42, 0.0, 1.0));
    let shaded_base = mix(reflected, water_rgb, rough_blend * 0.48) * ambient_tint;
    let scene_lit = water_scene_lighting(
        max(shaded_base, vec3<f32>(0.0)),
        normal,
        view_dir,
        in.world_pos,
        mix(0.035, 0.64, rough_blend),
        w.visual0.y,
    );
    let light_gain = clamp(length(scene_lit) / max(length(shaded_base), 0.001), 0.48, 2.8);
    let lowlight = (1.0 - clamp(light_gain * 0.58 + ambient_strength * 0.14, 0.0, 1.0)) * 0.30;
    let lit_water = mix(shaded_base, scene_lit, 0.72)
        + refract_tint
        + scatter * ambient_tint
        + fresnel_tint
        + vec3<f32>(spec_line + micro_break * 0.006 * detail_amp)
        - vec3<f32>(dark_patch * (0.08 + lowlight));
    let fog_t = clamp(view_dist / 900.0, 0.0, 1.0) * w.visual2.w * 0.45;
    let base_color = mix(lit_water, w.deep_color.rgb, fog_t);
    let foam_fresnel = pow(1.0 - clamp(dot(normal, view_dir), 0.0, 1.0), 2.6);
    let foam_lit = clamp(0.78 + ambient_strength * 0.10 + max(dot(normal, sun_dir), 0.0) * 0.18 + foam_fresnel * 0.22, 0.76, 1.26);
    let foam_rgb = mix(vec3<f32>(0.97, 0.985, 1.0) * foam_lit, w.foam_color.rgb, clamp(w.foam_color.a, 0.0, 1.0) * 0.18);
    let foam_aa = max(fwidth(surface_foam), 0.01);
    let foam_blend = smoothstep(0.24 - foam_aa, 0.82 + foam_aa, surface_foam);
    let color = mix(base_color, foam_rgb, foam_blend * (0.58 + detail_lod * 0.18));
    let alpha = mix(w.deep_color.a, w.shallow_color.a, shallow_t) * (1.0 - clamp(w.visual0.x, 0.0, 1.0) * 0.72);
    let side_color = mix(w.deep_color.rgb, color, 0.28);
    let final_color = mix(color, side_color, in.side_t);
    let final_alpha = mix(alpha + (fresnel * 0.10 + foam_blend * 0.08) * top_surface_mask, w.deep_color.a * 0.82, in.side_t);
    return vec4<f32>(final_color, clamp(final_alpha, 0.0, 1.0));
}
