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
@group(2) @binding(1)
var scene_color_tex: texture_2d<f32>;

struct Water3DVertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) water_idx: u32,
    @location(2) world_pos: vec3<f32>,
    @location(3) side_t: f32,
    @location(4) normal: vec3<f32>,
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

fn water_local_from_uv(w: Water, uv: vec2<f32>) -> vec2<f32> {
    return (uv - vec2<f32>(0.5, 0.5)) * w.size_depth_time.xy;
}

fn water_crest_wave(v: f32) -> f32 {
    return pow(max(v, 0.0), 3.0) - pow(max(-v, 0.0), 1.35) * 0.30;
}

fn water_idle_height(w: Water, local: vec2<f32>, t: f32) -> f32 {
    let phase = t * w.wave.x * 0.2;
    let wave_coord = local / max(w.wave_profile.x, 0.001);
    let tau = 6.2831853;
    if w.idle_mode == 0u {
        return 0.0;
    }
    if w.idle_mode == 1u {
        let wind = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.zw, length(w.flow_wind.zw) > 0.0001));
        let cross = vec2<f32>(-wind.y, wind.x);
        let a = sin(dot(wave_coord, wind) * tau + phase);
        let b = sin(dot(wave_coord, cross) * tau * 1.73 - phase * 0.61);
        let c = sin((wave_coord.x * 0.37 + wave_coord.y * 0.91) * tau * 2.37 + phase * 1.41);
        return (water_crest_wave(a) * 0.52 + b * 0.24 + water_crest_wave(c) * 0.24) * w.wave.y;
    }
    if w.idle_mode == 2u {
        let wind = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.zw, length(w.flow_wind.zw) > 0.0001));
        let cross = vec2<f32>(-wind.y, wind.x);
        let a = sin(dot(wave_coord, wind) * tau * 0.72 + phase * 0.84);
        let b = cos(dot(wave_coord, cross) * tau * 1.21 - phase * 1.17);
        let c = sin((wave_coord.x * 0.74 + wave_coord.y * 1.36) * tau * 1.83 + phase * 1.46);
        let d = cos((wave_coord.x * -1.19 + wave_coord.y * 0.48) * tau * 2.79 - phase * 2.08);
        return (water_crest_wave(a) * 0.42 + b * 0.20 + water_crest_wave(c) * 0.25 + d * 0.13) * w.wave.y * 1.45;
    }
    if w.idle_mode == 3u {
        let wind = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.zw, length(w.flow_wind.zw) > 0.0001));
        let cross = vec2<f32>(-wind.y, wind.x);
        let a = sin(dot(wave_coord, wind) * tau * 0.58 + phase * 0.77);
        let b = cos(dot(wave_coord, cross) * tau * 1.02 - phase * 0.91);
        let c = sin((wave_coord.x * 1.43 + wave_coord.y * 0.61) * tau * 1.74 + phase * 1.37);
        let d = cos((wave_coord.x * -0.51 + wave_coord.y * 1.18) * tau * 2.52 - phase * 1.91);
        let swell_a = pow(max(0.0, sin(dot(wave_coord, wind) * tau * 0.39 + phase * 0.63)), 5.0);
        let swell_b = pow(max(0.0, sin(dot(wave_coord, cross) * tau * 0.64 - phase * 1.09 + 1.7)), 4.0);
        return (water_crest_wave(a) * 0.30 + b * 0.12 + water_crest_wave(c) * 0.14 + d * 0.10
            + swell_a * 0.82 + swell_b * 0.56) * w.wave.y * 1.65;
    }
    let fallback_dir = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.zw, length(w.flow_wind.zw) > 0.0001));
    let flow = normalize(select(fallback_dir, w.flow_wind.xy, length(w.flow_wind.xy) > 0.0001));
    let cross = vec2<f32>(-flow.y, flow.x);
    let downstream = dot(wave_coord, flow);
    let across = dot(wave_coord, cross);
    let rush = sin(downstream * tau * 2.6 - phase * 4.2);
    let train = sin(downstream * tau * 5.1 - phase * 7.4 + across * 1.15);
    let shear = sin(across * tau * 1.35 + downstream * 0.9 - phase * 1.1);
    return (water_crest_wave(rush) * 0.58 + train * 0.28 + shear * 0.14) * w.wave.y * 0.52;
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
    return center * 0.58 + cross * 0.42;
}

fn water_height_geometry(w: Water, uv: vec2<f32>, t: f32) -> f32 {
    // 5-tap smoothed deviation: raw bilinear cells crease at sim-cell borders
    // and the crease reads as a visible grid on crowns/craters
    let ripple = water_cell_smooth(w, uv);
    let deviation = ripple.x + ripple.y * 0.045;
    return water_idle_height(w, water_local_from_uv(w, uv), t) + deviation;
}

fn water_idle_amp(w: Water) -> f32 {
    if w.idle_mode == 1u {
        return 1.0;
    }
    if w.idle_mode == 2u {
        return 1.45;
    }
    if w.idle_mode == 3u {
        return 2.6;
    }
    if w.idle_mode == 4u {
        return 0.52;
    }
    return 1.0;
}

fn water_hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
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

fn water_fbm3(p: vec2<f32>) -> f32 {
    var q = p;
    var amp = 0.54;
    var sum = 0.0;
    var norm = 0.0;
    for (var i = 0u; i < 3u; i = i + 1u) {
        sum += water_noise(q) * amp;
        norm += amp;
        q = vec2<f32>(q.x * 1.72 + q.y * 1.11, q.x * -1.04 + q.y * 1.83) + vec2<f32>(17.0, 9.0);
        amp *= 0.52;
    }
    return sum / max(norm, 0.001);
}

fn water_fbm2(p: vec2<f32>) -> f32 {
    let a = water_noise(p);
    let q = vec2<f32>(p.x * 1.72 + p.y * 1.11, p.x * -1.04 + p.y * 1.83) + vec2<f32>(17.0, 9.0);
    let b = water_noise(q);
    return a * 0.66 + b * 0.34;
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
    for (var i = 0u; i < 3u; i = i + 1u) {
        let n = abs(water_hex_noise(q) * 2.0 - 1.0);
        sum += (1.0 - n) * amp;
        norm += amp;
        q = vec2<f32>(q.x * 1.73 - q.y * 0.94, q.x * 0.88 + q.y * 1.61) + vec2<f32>(13.0, 29.0);
        amp *= 0.52;
    }
    return sum / max(norm, 0.001);
}

fn water_wind_dir(w: Water) -> vec2<f32> {
    return normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.zw, length(w.flow_wind.zw) > 0.0001));
}

// wave normal comes interpolated from the vertex stage; fragment only layers
// cheap micro ripple detail on top
fn water_detail_normal(w: Water, base: vec3<f32>, local: vec2<f32>, wind: vec2<f32>, t: f32) -> vec3<f32> {
    let strength = clamp(w.visual1.x, 0.0, 1.6);
    let p = local / max(w.visual1.y, 0.001);
    let eps = 0.35;
    let base_uv = p * 0.85 + wind * t * 0.055;
    let m0 = water_fbm2(base_uv);
    let mx = water_fbm2(base_uv + vec2<f32>(eps, 0.0));
    let my = water_fbm2(base_uv + vec2<f32>(0.0, eps));
    let micro = strength * 0.22;
    var n = base;
    n.x += (m0 - mx) / eps * micro;
    n.z += (m0 - my) / eps * micro;
    return normalize(n);
}

fn water_schlick_fresnel(cos_theta: f32, power: f32) -> f32 {
    let f0 = 0.028;
    let grazing = 1.0 - clamp(cos_theta, 0.0, 1.0);
    let edge = pow(grazing, max(power * 0.72, 0.001));
    let shoulder = smoothstep(0.12, 0.78, grazing);
    return f0 + (1.0 - f0) * (edge * 0.88 + shoulder * 0.26);
}

fn water_snells_window(normal: vec3<f32>, view: vec3<f32>, ior: f32) -> f32 {
    let cos_theta = clamp(abs(dot(normal, view)), 0.0, 1.0);
    let sin_theta = sqrt(max(0.0, 1.0 - cos_theta * cos_theta));
    return 1.0 - smoothstep(0.96, 1.02, sin_theta * ior);
}

fn water_scene_world_from_depth(coord: vec2<i32>, dims_u: vec2<u32>, depth: f32) -> vec3<f32> {
    let uv = (vec2<f32>(coord) + vec2<f32>(0.5)) / vec2<f32>(dims_u);
    let ndc_xy = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let ndc = vec4<f32>(ndc_xy, depth, 1.0);
    let world_h = scene.inv_view_proj * ndc;
    return world_h.xyz / max(abs(world_h.w), 1.0e-5);
}

struct WaterSsrHit {
    color: vec3<f32>,
    confidence: f32,
}

// March the reflected view ray against the opaque scene depth captured before
// water draws. This keeps nearby reflected geometry locked to the scene while
// the configured sky color remains the off-screen / rough-surface fallback.
fn water_ssr(world_pos: vec3<f32>, normal: vec3<f32>, view_dir: vec3<f32>, roughness: f32, reflection_weight: f32) -> WaterSsrHit {
    var result: WaterSsrHit;
    result.color = vec3<f32>(0.0);
    result.confidence = 0.0;
    // Rough or barely reflective water hides SSR detail. Skip its depth march.
    if roughness >= 0.96 || reflection_weight <= 0.012 {
        return result;
    }
    let dims_u = textureDimensions(scene_depth_tex);
    let dims = vec2<i32>(i32(dims_u.x), i32(dims_u.y));
    let ray_dir = normalize(reflect(-view_dir, normal));
    var ray_pos = world_pos + normal * 0.06 + ray_dir * 0.10;
    var travel = 0.10;
    // Smooth reflections keep the full search. Rough reflections use a shorter
    // ray because their confidence and final contribution are already low.
    let step_limit = u32(round(mix(24.0, 10.0, clamp(roughness, 0.0, 0.96))));

    for (var step = 0u; step < 24u; step = step + 1u) {
        if step >= step_limit {
            break;
        }
        let stride = 0.12 + f32(step) * 0.055;
        ray_pos += ray_dir * stride;
        travel += stride;
        let clip = scene.view_proj * vec4<f32>(ray_pos, 1.0);
        if clip.w <= 0.0001 {
            break;
        }
        let ndc = clip.xyz / clip.w;
        let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
        if any(uv <= vec2<f32>(0.002)) || any(uv >= vec2<f32>(0.998)) || ndc.z < 0.0 || ndc.z > 1.0 {
            break;
        }
        let coord = clamp(vec2<i32>(floor(uv * vec2<f32>(dims_u))), vec2<i32>(0), dims - vec2<i32>(1));
        let depth = textureLoad(scene_depth_tex, coord, 0);
        if depth < 0.999999 {
            let scene_world = water_scene_world_from_depth(coord, dims_u, depth);
            let ray_distance = distance(ray_pos, scene.camera_pos.xyz);
            let scene_distance = distance(scene_world, scene.camera_pos.xyz);
            let thickness = 0.10 + stride * 1.35;
            let crossing = ray_distance - scene_distance;
            if crossing >= 0.0 && crossing <= thickness {
                let edge = min(min(uv.x, uv.y), min(1.0 - uv.x, 1.0 - uv.y));
                let edge_fade = smoothstep(0.0, 0.08, edge);
                let distance_fade = 1.0 - smoothstep(8.0, 34.0, travel);
                result.color = textureLoad(scene_color_tex, coord, 0).rgb;
                result.confidence = edge_fade * distance_fade * (1.0 - roughness * 0.78);
                return result;
            }
        }
    }
    return result;
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

struct WaterDepthInfo {
    thickness: f32,
    bed_world: vec3<f32>,
    hit: f32,
    scene_depth: f32,
    scene_coord: vec2<i32>,
}

fn water_refraction_offset(
    w: Water,
    normal: vec3<f32>,
    local: vec2<f32>,
    cell: vec4<f32>,
    crest: f32,
    wind: vec2<f32>,
    t: f32,
) -> vec2<f32> {
    // refraction_strength stays the base control. Wave slope supplies the
    // large bend while sim velocity, flow and crests make it track motion.
    let slope = clamp(length(normal.xz) / max(abs(normal.y), 0.18), 0.0, 2.4);
    let wave_speed = clamp(abs(cell.y) / max(w.wave.y * max(w.wave.x, 0.25), 0.20), 0.0, 2.0);
    let flow_speed = clamp(length(w.flow_wind.xy) * 0.22, 0.0, 1.25);
    let crest_bend = smoothstep(0.08, 0.90, max(crest, 0.0));
    let motion = clamp(slope * 1.15 + wave_speed * 0.42 + flow_speed * 0.24 + crest_bend * 0.34, 0.0, 2.8);

    let flow_dir = normalize(select(wind, w.flow_wind.xy, length(w.flow_wind.xy) > 0.0001));
    let cross_dir = vec2<f32>(-flow_dir.y, flow_dir.x);
    let shimmer_phase = dot(local, flow_dir) * 7.1 - t * (1.8 + flow_speed * 1.4);
    let shimmer = flow_dir * sin(shimmer_phase)
        + cross_dir * sin(dot(local, cross_dir) * 11.3 + t * 2.7) * 0.55;
    let shimmer_weight = (0.045 + slope * 0.075) * (0.35 + motion * 0.65);
    let direction = normal.xz + shimmer * shimmer_weight;
    // Cap the vector before pixel conversion so grazing normals never pull
    // scene samples far outside the water silhouette.
    return clamp(direction * (1.0 + motion), vec2<f32>(-3.2), vec2<f32>(3.2));
}

fn water_depth_thickness(in: Water3DVertexOut, w: Water, refraction_offset: vec2<f32>) -> WaterDepthInfo {
    let dims_u = textureDimensions(scene_depth_tex);
    let dims = vec2<i32>(i32(dims_u.x), i32(dims_u.y));
    let base_coord = clamp(vec2<i32>(floor(in.clip_pos.xy)), vec2<i32>(0), dims - vec2<i32>(1));
    let base_depth = textureLoad(scene_depth_tex, base_coord, 0);
    let view_water = distance(in.world_pos, scene.camera_pos.xyz);
    var base_thickness = w.size_depth_time.z;
    if base_depth < 0.999999 {
        let base_world = water_scene_world_from_depth(base_coord, dims_u, base_depth);
        base_thickness = max(distance(base_world, scene.camera_pos.xyz) - view_water, 0.0);
    }
    let thickness_refraction = clamp(0.35 + smoothstep(0.08, max(w.size_depth_time.z * 0.45, 0.5), base_thickness) * 0.65, 0.0, 1.0);
    let offset = vec2<i32>(round(refraction_offset * clamp(w.visual2.y * 26.0 * thickness_refraction, 0.0, 48.0)));
    let coord = clamp(base_coord + offset, vec2<i32>(0), dims - vec2<i32>(1));
    // Most calm/front-facing pixels round to no offset. Reuse the first depth
    // fetch and avoid a duplicate texture load in that common path.
    var scene_depth = base_depth;
    if any(coord != base_coord) {
        scene_depth = textureLoad(scene_depth_tex, coord, 0);
    }
    var info: WaterDepthInfo;
    if scene_depth >= 0.999999 {
        info.thickness = w.size_depth_time.z;
        info.bed_world = in.world_pos - vec3<f32>(0.0, w.size_depth_time.z, 0.0);
        info.hit = 0.0;
        info.scene_depth = scene_depth;
        info.scene_coord = coord;
        return info;
    }
    let scene_world = water_scene_world_from_depth(coord, dims_u, scene_depth);
    let view_scene = distance(scene_world, scene.camera_pos.xyz);
    info.thickness = max(view_scene - view_water, 0.0);
    info.bed_world = scene_world;
    info.hit = 1.0;
    info.scene_depth = scene_depth;
    info.scene_coord = coord;
    return info;
}

// Approximate the angular blur caused by many small wave normals between the
// surface and an immersed object. Depth weights stop bright background from
// bleeding across the silhouette itself while the wide gather spreads light
// across shadows cast onto the same submerged surface.
fn water_transmission_tap(coord: vec2<i32>, center_depth: f32, dims: vec2<i32>) -> vec4<f32> {
    let c = clamp(coord, vec2<i32>(0), dims - vec2<i32>(1));
    let sample_depth = textureLoad(scene_depth_tex, c, 0);
    let depth_delta = abs(sample_depth - center_depth);
    let depth_span = 0.0008 + (1.0 - center_depth) * 0.004;
    let depth_weight = 1.0 - smoothstep(depth_span, depth_span * 4.0, depth_delta);
    return vec4<f32>(textureLoad(scene_color_tex, c, 0).rgb * depth_weight, depth_weight);
}

fn water_diffused_transmission(center: vec2<i32>, thickness: f32, scene_depth: f32, strength: f32) -> vec3<f32> {
    let dims_u = textureDimensions(scene_color_tex);
    let dims = vec2<i32>(i32(dims_u.x), i32(dims_u.y));
    let c = clamp(center, vec2<i32>(0), dims - vec2<i32>(1));
    let center_rgb = textureLoad(scene_color_tex, c, 0).rgb;
    let scatter = clamp(strength, 0.0, 2.0)
        * (0.18 + 0.82 * (1.0 - exp(-max(thickness, 0.0) * 0.32)));
    if scatter <= 0.015 {
        return center_rgb;
    }
    // Water depth grows the diffusion cone. Four wide taps soften shadows
    // without paying for a full screen-space blur pass.
    let radius = i32(round(clamp(2.0 + thickness * 1.15 + scatter * 2.0, 2.0, 16.0)));
    let ox = vec2<i32>(radius, 0);
    let oy = vec2<i32>(0, radius);
    let a = water_transmission_tap(c + ox, scene_depth, dims);
    let b = water_transmission_tap(c - ox, scene_depth, dims);
    let d = water_transmission_tap(c + oy, scene_depth, dims);
    let e = water_transmission_tap(c - oy, scene_depth, dims);
    let weighted_rgb = center_rgb * 1.6 + a.rgb + b.rgb + d.rgb + e.rgb;
    let weight = 1.6 + a.a + b.a + d.a + e.a;
    let diffuse_rgb = weighted_rgb / max(weight, 0.001);
    // Deep paths scatter more; open sky stays subtle so it does not turn flat.
    let open_sky = select(0.0, 1.0, scene_depth >= 0.999999);
    let blend = clamp(scatter * mix(0.52, 0.24, open_sky), 0.0, 0.72);
    let blurred_rgb = mix(center_rgb, diffuse_rgb, blend);
    // Participating media fills low-frequency occlusion. Keep lit pixels
    // stable and lift only the dark side of the local irradiance range.
    let center_luma = dot(center_rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let diffuse_luma = dot(diffuse_rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let shadow_fill = clamp((diffuse_luma - center_luma) * scatter * 0.85, 0.0, 0.45);
    return blurred_rgb + max(diffuse_rgb - center_rgb, vec3<f32>(0.0)) * shadow_fill;
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
        water_height_geometry(w, uv, w.wave_profile.y),
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
        water_height_geometry(w, uv, w.wave_profile.y),
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
    let top = water_height_geometry(w, uv, w.wave_profile.y);
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
    let pos = vec3<f32>(local_xz.x, water_height_geometry(w, uv, w.wave_profile.y), local_xz.y);
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

    var normal_local = vec3<f32>(0.0, 1.0, 0.0);
    if local_vertex.valid && local_vertex.side_t < 0.5 {
        let t = w.wave_profile.y;
        // central differences: forward diffs bias the gradient per-quad and
        // neighbouring triangles disagree -> faceted grid seams under spec
        let ndu = vec2<f32>(1.0 / max(f32(w.flags.x), 2.0), 0.0);
        let ndv = vec2<f32>(0.0, 1.0 / max(f32(w.flags.y), 2.0));
        let h_l = water_height_geometry(w, local_vertex.uv - ndu, t);
        let h_r = water_height_geometry(w, local_vertex.uv + ndu, t);
        let h_d = water_height_geometry(w, local_vertex.uv - ndv, t);
        let h_u = water_height_geometry(w, local_vertex.uv + ndv, t);
        let sx = max(w.size_depth_time.x * ndu.x * 2.0, 0.001);
        let sz = max(w.size_depth_time.y * ndv.y * 2.0, 0.001);
        let strength = clamp(w.visual1.x, 0.0, 1.6);
        normal_local = normalize(vec3<f32>((h_l - h_r) / sx * strength, 1.0, (h_d - h_u) / sz * strength));
    }
    let model3 = mat3x3<f32>(w.model_x.xyz, w.model_y.xyz, w.model_z.xyz);
    let normal_world = normalize(model3 * normal_local);

    var out: Water3DVertexOut;
    out.clip_pos = select(vec4<f32>(2.0, 2.0, 1.0, 1.0), scene.view_proj * world, local_vertex.valid);
    out.uv = local_vertex.uv;
    out.water_idx = water_idx;
    out.world_pos = world.xyz;
    out.side_t = local_vertex.side_t;
    out.normal = normal_world;
    return out;
}

@fragment
fn fs_water_3d(in: Water3DVertexOut, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    let w = waters[in.water_idx];
    if in.side_t <= 0.5 && water_shape_alpha(w, in.uv) <= 0.0 {
        return vec4<f32>(0.0);
    }
    let t = w.wave_profile.y;
    let top_surface_mask = 1.0 - smoothstep(0.02, 0.18, in.side_t);
    let coast = water_coast_sample(w, in.uv);
    if coast.x > 0.985 {
        return vec4<f32>(0.0);
    }

    let local = water_local_from_uv(w, in.uv);
    let view_dist = distance(scene.camera_pos.xyz, in.world_pos);
    let view_dir = normalize(scene.camera_pos.xyz - in.world_pos);
    let wind = water_wind_dir(w);
    let normal = water_detail_normal(w, normalize(in.normal), local, wind, t);
    let cell = water_cell(w, in.uv);
    let idle_h = water_idle_height(w, local, t);

    let sun_dir = normalize(select(vec3<f32>(0.0, 1.0, 0.0), -scene.ray_light.direction.xyz, length(scene.ray_light.direction.xyz) > 0.001));
    let sun_radiance = scene.ray_light.color_intensity.xyz * min(scene.ray_light.color_intensity.w, 6.0);
    let sun_up = clamp(sun_dir.y, 0.0, 1.0);

    // Treat Fresnel as an energy split, not a blue surface tint.  Applying
    // reflectivity twice made broad view angles read like opaque blue ground.
    let dielectric_fresnel = water_schlick_fresnel(dot(normal, view_dir), w.visual0.w);
    let fresnel = clamp(dielectric_fresnel * w.visual0.y, 0.0, 1.0);
    let auto_shallow_depth = max(max(w.size_depth_time.x, w.size_depth_time.y) * 0.25, 0.001);
    let shallow_depth = select(auto_shallow_depth, max(w.size_depth_time.w, 0.001), w.size_depth_time.w >= 0.0);
    let crest_t = clamp(
        idle_h / max(w.wave.y * water_idle_amp(w), 0.001) + clamp(cell.x, 0.0, 1.0) * 0.35,
        -0.5,
        1.5,
    );
    let refraction_offset = water_refraction_offset(w, normal, local, cell, crest_t, wind, t);
    let depth_info = water_depth_thickness(in, w, refraction_offset);
    let scene_thickness = max(depth_info.thickness, 0.0);
    let depth_t = clamp(1.0 - exp(-scene_thickness / max(shallow_depth, 0.001)), 0.0, 1.0);

    // Beer-Lambert extinction: clear at short paths, colored only by depth.
    // The old coefficient filled even modest depths with the shallow color.
    let absorb_k = 0.42 / max(shallow_depth, 0.25);
    let absorption = exp(-scene_thickness * absorb_k * vec3<f32>(1.0, 0.40, 0.17));
    let shallow_rgb = max(w.shallow_color.rgb, vec3<f32>(0.0));
    let deep_rgb = max(w.deep_color.rgb, vec3<f32>(0.0));
    // Let participating media inherit illumination from Sky3D instead of
    // behaving like self-lit blue material.  Keep user water colors as the
    // absorption hue, not the sole radiance source.
    let sky_scatter_rgb = max(w.sky_color_bias.rgb, vec3<f32>(0.0));
    let volume_tint = mix(deep_rgb, shallow_rgb, absorption);
    let sky_scatter_weight = clamp(w.visual2.z * (0.10 + 0.22 * (1.0 - absorption.g)), 0.0, 0.48);
    let volume_rgb = mix(volume_tint, volume_tint * sky_scatter_rgb * 1.35, sky_scatter_weight);
    let refracted_scene = water_diffused_transmission(
        depth_info.scene_coord,
        scene_thickness,
        depth_info.scene_depth,
        w.visual2.z,
    );
    // Out-scattered direct light returns as broad in-scatter. This prevents
    // opaque-scene shadow maps from surviving unchanged below clear water.
    let scatter_extinction = vec3<f32>(1.0) - absorption;
    let water_irradiance = scene.ambient_color.rgb * scene.ambient_color.w
        + sun_radiance * (0.10 + 0.18 * sun_up);
    let in_scatter = water_irradiance
        * mix(shallow_rgb, vec3<f32>(1.0), 0.22)
        * scatter_extinction
        * clamp(w.visual2.z, 0.0, 2.0)
        * 0.34;
    let transmitted_rgb = refracted_scene * absorption
        + volume_rgb * scatter_extinction
        + in_scatter;

    let forward_light = pow(max(dot(-view_dir, sun_dir), 0.0), 2.0);
    let sss = pow(max(crest_t, 0.0), 1.4)
        * clamp(w.visual2.z, 0.0, 2.0)
        * (0.30 + 0.70 * forward_light)
        * clamp(dot(normal, sun_dir) * 0.5 + 0.5, 0.0, 1.0);
    let sss_rgb = (shallow_rgb * 1.30 + vec3<f32>(0.0, 0.09, 0.07)) * sun_radiance * 0.5;
    let water_rgb = transmitted_rgb + sss_rgb * sss;

    let rough_blend = clamp(w.visual0.z, 0.0, 1.0);
    let reflection_weight = clamp(max(w.sky_color_bias.w * w.visual0.y, fresnel), 0.0, 1.0);
    let ssr = water_ssr(in.world_pos, normal, view_dir, rough_blend, reflection_weight);
    let reflection_source = mix(w.sky_color_bias.rgb, ssr.color, ssr.confidence);
    let reflected = mix(water_rgb, reflection_source, reflection_weight);
    let half_dir = normalize(view_dir + sun_dir);
    let spec = pow(max(dot(normal, half_dir), 0.0), mix(140.0, 34.0, rough_blend)) * w.visual0.y * 0.35;
    let ambient_strength = clamp(scene.ambient_color.w, 0.0, 4.0);
    let ambient_tint = mix(vec3<f32>(1.0), scene.ambient_color.xyz, clamp(ambient_strength * 0.42, 0.0, 1.0));
    // Water has no diffuse top-surface lobe. Keep transmitted scene radiance
    // intact; ambient only colors light scattered by the water volume.
    let shaded_base = reflected;

    var caustic = 0.0;
    // sun_up scales caustic to zero when sun at/below horizon; gate on it so
    // the hex-ridged fbm is skipped when its weight is ~0 (output-identical)
    if w.visual2.x > 0.001 && depth_info.hit > 0.5 && top_surface_mask > 0.02
        && sun_up > 0.0 && scene_thickness < shallow_depth * 4.0 {
        let bed_xz = depth_info.bed_world.xz;
        let drift = wind * t * 0.42;
        let c1 = water_hex_ridged_fbm(bed_xz * 0.55 + drift);
        let pattern = pow(clamp(c1 * 1.55 - 0.62, 0.0, 1.0), 1.9);
        let clarity = exp(-scene_thickness * 0.30);
        caustic = pattern * clarity * clamp(w.visual2.x, 0.0, 2.0) * sun_up * 1.6;
    }

    // two distinct foam layers:
    // 1) coastline outline - thin crisp band hugging the collider/water
    //    intersection silhouette ("water holds around this mass")
    // 2) breaking foam - patchy sim-driven wash frm wakes/splashes/crashes,
    //    randomized in position, size and opacity so strands don't pop
    let foam_strength = bitcast<f32>(w.flags.w);
    let edge_noise = clamp(w.model_z.w, 0.0, 1.0);
    let foam_scale = 2.6 / max(w.visual1.y, 0.001);
    let grain = water_fbm2(local * foam_scale * 2.7 - wind * t * 0.55 + vec2<f32>(9.0, 4.0));
    // coast.y ~1 right at the mass, falls off across foam_width: slice the
    // top of the band for a thin outline that follows the intersection shape
    let outline = smoothstep(0.60, 0.86, coast.y + (grain - 0.5) * 0.22 * edge_noise)
        * clamp(w.coastline.x, 0.0, 2.5)
        * top_surface_mask;
    let wash_foam = coast.y * clamp(w.coastline.x, 0.0, 2.5) * 0.5;
    let body_foam = cell.z * max(foam_strength, 0.0) * 0.85;
    let foam_total = clamp(wash_foam + body_foam, 0.0, 1.6);
    var foam_mask = 0.0;
    var foam_sparkle = 0.0;
    if foam_total > 0.02 && top_surface_mask > 0.02 {
        // low-freq patch fields randomize where foam sits, how large the
        // patches are and how strongly each patch shows over time
        // Advect one coherent foam field.  Slow opacity drift avoids temporal
        // noise making otherwise stable simulation foam blink in place.
        let foam_drift = wind * t * 0.12;
        let patch_field = water_fbm2(local * 0.16 - foam_drift * 0.16);
        let patch_fade = water_fbm2(local * 0.11 - foam_drift * 0.11 + vec2<f32>(t * 0.012, -t * 0.009));
        let web = water_hex_ridged_fbm(local * foam_scale * 0.8 - foam_drift * foam_scale * 0.8);
        let density = clamp(web * 0.74 + grain * 0.36, 0.0, 1.2);
        let energy = foam_total * (0.55 + patch_field * 0.75);
        let coverage = smoothstep(0.05, 0.85, energy);
        let cut = 1.02 - coverage * 0.88;
        let opacity = 0.52 + 0.48 * smoothstep(0.22, 0.80, patch_fade);
        foam_mask = smoothstep(cut - 0.04, cut + 0.34, density)
            * min(energy * 1.6, 1.0)
            * opacity
            * top_surface_mask;
        foam_sparkle = smoothstep(cut + 0.26, cut + 0.42, density) * coverage * opacity;
    }
    let shore_frac = clamp(wash_foam / max(foam_total, 0.001), 0.0, 1.0);
    // rigid-body foam reads lighter than shoreline wash
    let foam_rgb = mix(w.foam_color.rgb * 0.88, w.coastline_foam_color.rgb, shore_frac);
    let foam_lit = foam_rgb * clamp(
        scene.ambient_color.xyz * scene.ambient_color.w * 0.72
            + sun_radiance * (max(dot(normal, sun_dir), 0.0) * 0.55 + 0.20)
            + vec3<f32>(0.25),
        vec3<f32>(0.0),
        vec3<f32>(1.55),
    ) * (0.86 + foam_sparkle * 0.35);

    let scatter = (1.0 - depth_t) * w.visual2.z * (0.12 + 0.58 * forward_light) * max(dot(normal, sun_dir), 0.0);
    let lit_water = shaded_base
        + scatter * ambient_tint
        + sun_radiance * caustic * vec3<f32>(0.55, 0.85, 1.0) * 0.30
        + spec * sun_radiance * (1.0 - foam_mask);
    let fog_t = clamp(view_dist / 900.0, 0.0, 1.0) * w.visual2.w * 0.34;
    let base_color = mix(lit_water, deep_rgb, fog_t);
    var color = mix(base_color, foam_lit, foam_mask * clamp(w.foam_color.a, 0.0, 1.0));
    // crisp coastline outline layered on top of the patchy wash
    let outline_lit = w.coastline_foam_color.rgb * clamp(
        scene.ambient_color.xyz * scene.ambient_color.w * 0.72
            + sun_radiance * (max(dot(normal, sun_dir), 0.0) * 0.55 + 0.24)
            + vec3<f32>(0.30),
        vec3<f32>(0.0),
        vec3<f32>(1.55),
    );
    let outline_mask = min(outline, 1.0) * clamp(w.coastline_foam_color.a, 0.0, 1.0) * 0.92;
    color = mix(color, outline_lit, outline_mask);
    // Coverage follows energy removed from transmission. This prevents color
    // alpha from laying an opaque sheet over shallow water while deep water
    // still closes naturally through absorption.
    let transmission_luma = dot(absorption, vec3<f32>(0.2126, 0.7152, 0.0722));
    let tint_density = mix(w.shallow_color.a, w.deep_color.a, depth_t);
    let absorption_opacity = (1.0 - transmission_luma) * tint_density;
    let optical_opacity = reflection_weight + (1.0 - reflection_weight) * absorption_opacity;
    var alpha = optical_opacity * (1.0 - clamp(w.visual0.x, 0.0, 1.0)) * top_surface_mask;
    alpha = max(alpha, max(foam_mask * 0.92, outline_mask));
    let side_color = mix(w.deep_color.rgb, color, 0.28);
    let snell_window = water_snells_window(normal, view_dir, 1.333);
    let underside_mask = select(1.0, 0.0, front_facing) * top_surface_mask;
    let underside_color = mix(w.deep_color.rgb * 0.72, color, snell_window);
    // Top surface already composites captured scene color for refraction.
    // Apply material transparency inside that composite; alpha-blending the
    // captured scene over the same scene again weakens refraction and leaves
    // a flat blue veil.
    let transparent_top = mix(color, refracted_scene, clamp(w.visual0.x, 0.0, 1.0));
    let final_color = mix(mix(transparent_top, underside_color, underside_mask), side_color, in.side_t);
    let underside_alpha = mix(max(alpha, 0.72), alpha, snell_window);
    // Surface pixel is a finished scene composite. Side walls remain regular
    // translucent geometry.
    let top_alpha = mix(1.0, underside_alpha, underside_mask);
    let final_alpha = mix(top_alpha, w.deep_color.a * 0.82, in.side_t);
    // screen-space dither breaks 8-bit banding on the long depth gradients
    let dither = (water_hash(floor(in.clip_pos.xy) * 0.7311) - 0.5) * (2.0 / 255.0);
    return vec4<f32>(max(final_color + vec3<f32>(dither), vec3<f32>(0.0)), clamp(final_alpha + dither * 0.5, 0.0, 1.0));
}
