
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

struct Camera2D {
    view: mat4x4<f32>,
    ndc_scale: vec2<f32>,
    pad: vec2<f32>,
}

@group(0) @binding(0)
var<storage, read> waters: array<Water>;
@group(0) @binding(1)
var<storage, read_write> cells: array<vec4<f32>>;
@group(0) @binding(2)
var<uniform> params: Params;
@group(0) @binding(3)
var<storage, read> coastline_cells: array<vec4<f32>>;
@group(1) @binding(0)
var<uniform> camera: Camera2D;

fn water_shape_alpha(w: Water, uv: vec2<f32>) -> f32 {
    if w.shape.x < 0.5 {
        return 1.0;
    }
    let local = (uv - vec2<f32>(0.5, 0.5)) * w.size_depth_time.xy;
    let r = w.shape.y;
    if dot(local, local) <= r * r {
        return 1.0;
    }
    return 0.0;
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
        let crest_a = pow(max(a, 0.0), 3.0) - pow(max(-a, 0.0), 1.4) * 0.42;
        let crest_b = pow(max(c, 0.0), 4.0) - pow(max(-c, 0.0), 1.3) * 0.28;
        return (crest_a * 0.42 + b * 0.20 + crest_b * 0.25 + d * 0.13) * w.wave.y * 1.45;
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
        let chop = (pow(max(a, 0.0), 3.0) * 0.30 - pow(max(-a, 0.0), 1.35) * 0.16)
            + (b * 0.12 + c * 0.14 + d * 0.10);
        return (chop + swell_a * 0.82 + swell_b * 0.56) * w.wave.y * 1.65;
    }
    let flow = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.xy, length(w.flow_wind.xy) > 0.0001));
    let cross = vec2<f32>(-flow.y, flow.x);
    let a = sin(dot(wave_coord, flow) * tau * 1.6 - phase * 1.5);
    let b = sin(dot(wave_coord, cross) * tau * 2.4 + phase * 0.55);
    return (a * 0.76 + b * 0.24) * w.wave.y * 0.45;
}

fn water_coast_diffuse(w: Water, local_idx: u32, width: u32) -> f32 {
    let height = max(w.sim.w, 1u);
    let x = local_idx % width;
    let y = local_idx / width;
    let xl = x - select(0u, 1u, x > 0u);
    let xr = min(x + 1u, width - 1u);
    let yd = y - select(0u, 1u, y > 0u);
    let yu = min(y + 1u, height - 1u);
    let left = coastline_cells[w.sim.x + min(y * width + xl, w.sim.y - 1u)].y;
    let right = coastline_cells[w.sim.x + min(y * width + xr, w.sim.y - 1u)].y;
    let down = coastline_cells[w.sim.x + min(yd * width + x, w.sim.y - 1u)].y;
    let up = coastline_cells[w.sim.x + min(yu * width + x, w.sim.y - 1u)].y;
    return (left + right + down + up) * 0.25;
}

fn water_coast_normal(w: Water, local_idx: u32, width: u32) -> vec2<f32> {
    let height = max(w.sim.w, 1u);
    let x = local_idx % width;
    let y = local_idx / width;
    let xl = x - select(0u, 1u, x > 0u);
    let xr = min(x + 1u, width - 1u);
    let yd = y - select(0u, 1u, y > 0u);
    let yu = min(y + 1u, height - 1u);
    let left = coastline_cells[w.sim.x + min(y * width + xl, w.sim.y - 1u)].x;
    let right = coastline_cells[w.sim.x + min(y * width + xr, w.sim.y - 1u)].x;
    let down = coastline_cells[w.sim.x + min(yd * width + x, w.sim.y - 1u)].x;
    let up = coastline_cells[w.sim.x + min(yu * width + x, w.sim.y - 1u)].x;
    let grad = vec2<f32>(right - left, up - down);
    let len = length(grad);
    if len <= 0.0001 {
        return vec2<f32>(0.0, 0.0);
    }
    return grad / len;
}

@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let water_idx = gid.y;
    if water_idx >= params.water_count {
        return;
    }
    let w = waters[water_idx];
    let local_idx = gid.x;
    if w.sim.y == 0u || local_idx >= w.sim.y {
        return;
    }
    if (w.flags.z & 2u) != 0u {
        return;
    }
    let cell_idx = w.sim.x + local_idx;
    let width = max(w.sim.z, 1u);
    let x_cell = local_idx % width;
    let y_cell = local_idx / width;
    let fx = f32(x_cell) / max(f32(width - 1u), 1.0);
    let fy = f32(y_cell) / max(f32(max(w.sim.w, 1u) - 1u), 1.0);
    if water_shape_alpha(w, vec2<f32>(fx, fy)) <= 0.0 {
        cells[cell_idx] = vec4<f32>(0.0);
        return;
    }
    let t = params.time_seconds;
    let phase = t * w.wave.x * 0.2;
    let local = (vec2<f32>(fx, fy) - vec2<f32>(0.5, 0.5)) * w.size_depth_time.xy;
    let idle = water_idle_height(w, local, t);
    let coast = coastline_cells[cell_idx];
    if coast.x > 0.985 {
        cells[cell_idx] = vec4<f32>(0.0, 0.0, 1.0, 1.0);
        return;
    }
    let edge = max(0.0, 1.0 - min(min(fx, 1.0 - fx), min(fy, 1.0 - fy)) * max(w.coastline.y, 0.001) * 8.0);
    let neighbor_shore = water_coast_diffuse(w, local_idx, width);
    let coast_normal = water_coast_normal(w, local_idx, width);
    let shore = max(edge, max(coast.y, neighbor_shore * 0.92)) * (1.0 - coast.x * 0.40);
    let wake = coast.z * w.wave.w * 1.45;
    if shore <= 0.0 && wake <= 0.0 && coast.w <= 0.0 && abs(idle) <= 0.00001 {
        cells[cell_idx] = vec4<f32>(0.0);
        return;
    }
    let edge_noise = (sin((local.x * 0.31 + local.y * 0.47) + phase * 7.0) + sin((local.x * -0.53 + local.y * 0.29) - phase * 4.3)) * 0.34 * w.model_z.w;
    let spill = clamp(coast.w, 0.0, 1.0);
    let diffusion = max(neighbor_shore - coast.y, 0.0) * 0.72 + spill * 0.28;
    let wave_dir = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.zw + w.flow_wind.xy * 0.35, length(w.flow_wind.zw + w.flow_wind.xy * 0.35) > 0.0001));
    let coast_push = max(dot(-wave_dir, coast_normal), 0.0);
    let coast_slide = abs(dot(vec2<f32>(-wave_dir.y, wave_dir.x), coast_normal));
    let crash_wave = max(0.0, water_crest_wave(sin((local.x * 0.19 - local.y * 0.23) + phase * 5.5 + edge_noise)));
    let reflected = shore * (0.58 + coast_push * 1.38 + coast_slide * 0.42)
        * water_crest_wave(sin((local.x * -0.27 + local.y * 0.18) - phase * 4.1 - edge_noise))
        * w.model_y.w
        * w.wave.y
        * w.model_z.w;
    let crash_up = shore * (1.18 + coast_push * 1.12 + coast_slide * 0.24) * pow(crash_wave, 2.4) * w.model_y.w * w.wave.y * 2.45;
    let crash_down = -shore * pow(max(-crash_wave, 0.0), 1.2) * w.wave.y * 0.34;
    let crash = (crash_up + crash_down + max(reflected, 0.0) * 0.88) * (0.70 + spill * 0.68) + diffusion * w.wave.y * 1.08;
    let prev = cells[cell_idx].x
        * w.wave.z
        * (1.0 - shore * (0.72 + coast_push * 0.44 + coast_slide * 0.16) * w.coastline.w)
        * (0.50 + spill * 0.28 + coast_slide * 0.12);
    let crest_norm = idle / max(w.wave.y, 0.001);
    let crest_line = smoothstep(0.44, 0.82, crest_norm) * (1.0 - smoothstep(1.04, 1.72, crest_norm));
    let wave_foam = crest_line * bitcast<f32>(w.flags.w) * 0.46;
    let impact_foam = smoothstep(0.06, 0.84, wake + abs(crash)) * bitcast<f32>(w.flags.w) * 0.62;
    let shore_foam = smoothstep(0.18, 1.20, crash + shore * 0.62) * (1.0 - smoothstep(1.42, 2.45, crash)) * w.coastline.x * bitcast<f32>(w.flags.w) * 1.12;
    let foam = clamp(wave_foam + impact_foam + shore_foam + spill * max(wake, shore) * 0.34, 0.0, 1.0);
    let height = mix(prev + idle * (0.030 + shore * w.model_y.w * 0.18) + wake * 0.30 + crash, idle + wake * 0.28 + crash, 0.44 + spill * 0.14);
    cells[cell_idx] = vec4<f32>(height, idle, foam, shore);
}

struct Water2DVertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) water_idx: u32,
}

fn quad_pos(vertex_idx: u32) -> vec2<f32> {
    var p = array<vec2<f32>, 6>(
        vec2<f32>(-0.5, -0.5),
        vec2<f32>( 0.5, -0.5),
        vec2<f32>( 0.5,  0.5),
        vec2<f32>(-0.5, -0.5),
        vec2<f32>( 0.5,  0.5),
        vec2<f32>(-0.5,  0.5),
    );
    return p[vertex_idx];
}

@vertex
fn vs_water_2d(
    @builtin(vertex_index) vertex_idx: u32,
    @builtin(instance_index) water_idx: u32,
) -> Water2DVertexOut {
    let w = waters[water_idx];
    let local = quad_pos(vertex_idx);
    let scaled = local * w.size_depth_time.xy;
    let model = mat3x3<f32>(w.model_x.xyz, w.model_y.xyz, w.model_z.xyz);
    let world_xy = (model * vec3<f32>(scaled, 1.0)).xy;
    let view = camera.view * vec4<f32>(world_xy, 0.0, 1.0);
    let depth = 1.0 - f32(w.z_index) * 0.001;

    var out: Water2DVertexOut;
    out.clip_pos = vec4<f32>(view.xy * camera.ndc_scale, depth, 1.0);
    out.uv = local + vec2<f32>(0.5, 0.5);
    out.water_idx = water_idx;
    return out;
}

@fragment
fn fs_water_2d(in: Water2DVertexOut) -> @location(0) vec4<f32> {
    let w = waters[in.water_idx];
    if water_shape_alpha(w, in.uv) <= 0.0 {
        return vec4<f32>(0.0);
    }
    let t = params.time_seconds;
    let idle = sin((in.uv.x + in.uv.y + t * w.wave.x * 0.2) * 6.2831853) * 0.5 + 0.5;
    var ripple = vec4<f32>(0.0);
    if w.sim.y > 0u {
        let width = max(w.sim.z, 1u);
        let height = max(w.sim.w, 1u);
        let x = u32(clamp(in.uv.x, 0.0, 1.0) * f32(max(width - 1u, 1u)));
        let y = u32(clamp(in.uv.y, 0.0, 1.0) * f32(max(height - 1u, 1u)));
        let local_idx = min(y * width + x, w.sim.y - 1u);
        let cell_idx = w.sim.x + local_idx;
        ripple = cells[cell_idx] * w.model_x.w;
        if coastline_cells[cell_idx].x > 0.985 {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    }
    let edge = max(0.0, 1.0 - min(min(in.uv.x, 1.0 - in.uv.x), min(in.uv.y, 1.0 - in.uv.y)) * max(w.coastline.y, 0.001) * 8.0);
    let crest = smoothstep(1.05, 2.7, abs(ripple.x)) * bitcast<f32>(w.flags.w) * 0.22;
    let foam = clamp(ripple.z * 1.04 + max(edge, ripple.w) * w.coastline.x * 0.26 + crest * 1.12, 0.0, 1.0);
    let auto_shallow_depth = max(max(w.size_depth_time.x, w.size_depth_time.y) * 0.25, 0.001);
    let shallow_depth = select(auto_shallow_depth, max(w.size_depth_time.w, 0.001), w.size_depth_time.w >= 0.0);
    let depth_t = clamp(w.size_depth_time.z / shallow_depth, 0.0, 1.0);
    let shallow_t = clamp(1.0 - depth_t + idle * 0.10 + foam * 0.12, 0.0, 1.0);
    let surface_t = clamp(shallow_t + abs(ripple.x) * 0.16 + foam * 0.10, 0.0, 1.0);
    let water_rgb = mix(w.deep_color.rgb, w.shallow_color.rgb, surface_t);
    let sky_rgb = mix(water_rgb, w.sky_color_bias.rgb, w.sky_color_bias.w);
    let color = mix(sky_rgb, w.coastline_foam_color.rgb, foam * 0.58);
    let alpha = mix(w.deep_color.a, w.shallow_color.a, shallow_t);
    return vec4<f32>(color, clamp(alpha + foam * 0.18, 0.0, 1.0));
}
