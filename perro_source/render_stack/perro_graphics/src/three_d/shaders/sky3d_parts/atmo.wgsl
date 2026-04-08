struct SkyUniform {
    inv_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    day_colors: array<vec4<f32>, 3>,
    evening_colors: array<vec4<f32>, 3>,
    night_colors: array<vec4<f32>, 3>,
    params0: vec4<f32>, // cloud_size, cloud_density, cloud_variance, time_of_day
    params1: vec4<f32>, // star_size, star_scatter, star_gleam, sky_angle
    params2: vec4<f32>, // sun_size, moon_size, day_weight, cloud_time_seconds
    wind: vec4<f32>, // x,y cloud wind, z style_blend (0 toon, 1 realistic), w reserved
};

@group(0) @binding(0)
var<uniform> sky: SkyUniform;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// ──────────────────────────────────────────────────────────────
// Utility
// ──────────────────────────────────────────────────────────────

fn hash12(p: vec2<f32>) -> f32 {
    let p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    let q = p3 + dot(p3, p3.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let n00 = hash12(i + vec2<f32>(0.0, 0.0));
    let n10 = hash12(i + vec2<f32>(1.0, 0.0));
    let n01 = hash12(i + vec2<f32>(0.0, 1.0));
    let n11 = hash12(i + vec2<f32>(1.0, 1.0));
    let nx0 = mix(n00, n10, u.x);
    let nx1 = mix(n01, n11, u.x);
    return mix(nx0, nx1, u.y);
}

fn grad2(cell: vec2<f32>) -> vec2<f32> {
    let a = hash12(cell) * 6.28318530718;
    return vec2<f32>(cos(a), sin(a));
}

fn rotate2(v: vec2<f32>, angle: f32) -> vec2<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return vec2<f32>(v.x * c - v.y * s, v.x * s + v.y * c);
}

fn perlin2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let g00 = grad2(i + vec2<f32>(0.0, 0.0));
    let g10 = grad2(i + vec2<f32>(1.0, 0.0));
    let g01 = grad2(i + vec2<f32>(0.0, 1.0));
    let g11 = grad2(i + vec2<f32>(1.0, 1.0));

    let n00 = dot(g00, f - vec2<f32>(0.0, 0.0));
    let n10 = dot(g10, f - vec2<f32>(1.0, 0.0));
    let n01 = dot(g01, f - vec2<f32>(0.0, 1.0));
    let n11 = dot(g11, f - vec2<f32>(1.0, 1.0));

    let nx0 = mix(n00, n10, u.x);
    let nx1 = mix(n01, n11, u.x);
    return mix(nx0, nx1, u.y) * 0.5 + 0.5;
}

fn perlin_fbm(p: vec2<f32>, octaves: i32) -> f32 {
    var q = p;
    var amp = 0.5;
    var sum = 0.0;
    var norm = 0.0;
    for (var i = 0; i < octaves; i++) {
        sum += perlin2(q) * amp;
        norm += amp;
        q = q * 2.0 + vec2<f32>(31.0, -19.0);
        amp *= 0.5;
    }
    return sum / max(norm, 1.0e-5);
}

fn fbm2(p: vec2<f32>) -> f32 {
    var q = p;
    var amp = 0.5;
    var sum = 0.0;
    for (var i = 0; i < 4; i++) {
        sum += noise2(q) * amp;
        q = q * 2.03 + vec2<f32>(17.0, -13.0);
        amp *= 0.5;
    }
    return sum;
}

fn dir_to_octa_uv(d: vec3<f32>) -> vec2<f32> {
    let n = d / max(abs(d.x) + abs(d.y) + abs(d.z), 1.0e-6);
    var uv = n.xy;
    if (n.z < 0.0) {
        uv = (vec2<f32>(1.0, 1.0) - abs(uv.yx)) * sign(uv);
    }
    return uv * 0.5 + vec2<f32>(0.5, 0.5);
}

fn gradient3(colors: array<vec4<f32>, 3>, t_in: f32) -> vec3<f32> {
    let t = clamp(t_in, 0.0, 1.0);
    if (t < 0.5) {
        let local_t = smoothstep(0.0, 1.0, t * 2.0);
        return mix(colors[0].xyz, colors[1].xyz, local_t);
    }
    let local_t = smoothstep(0.0, 1.0, (t - 0.5) * 2.0);
    return mix(colors[1].xyz, colors[2].xyz, local_t);
}

fn gradient3_blur(colors: array<vec4<f32>, 3>, t_in: f32, radius: f32) -> vec3<f32> {
    let r = clamp(radius, 0.0, 0.25);
    let c0 = gradient3(colors, clamp(t_in - r, 0.0, 1.0));
    let c1 = gradient3(colors, t_in);
    let c2 = gradient3(colors, clamp(t_in + r, 0.0, 1.0));
    return c0 * 0.27901 + c1 * 0.44198 + c2 * 0.27901;
}

fn star_point_field_triplanar(
    dir: vec3<f32>,
    scale: f32,
    density: f32,
    size: f32
) -> f32 {
    let n = normalize(dir);
    let an = max(abs(n), vec3<f32>(1.0e-5));
    var w = pow(an, vec3<f32>(4.0));
    w /= max(w.x + w.y + w.z, 1.0e-5);

    let uv_x = n.yz * 0.5 + vec2<f32>(0.5, 0.5);
    let uv_y = n.xz * 0.5 + vec2<f32>(0.5, 0.5);
    let uv_z = n.xy * 0.5 + vec2<f32>(0.5, 0.5);

    let sx = star_point_field(uv_x + vec2<f32>(0.17, -0.23), scale, density, size);
    let sy = star_point_field(uv_y + vec2<f32>(-0.31, 0.11), scale, density, size);
    let sz = star_point_field(uv_z + vec2<f32>(0.43, 0.29), scale, density, size);
    return sx * w.x + sy * w.y + sz * w.z;
}

fn sun_dir_from_time(tod: f32, sky_angle: f32) -> vec3<f32> {
    let theta = (tod * 6.28318530718) - 1.57079632679 + sky_angle;
    return normalize(vec3<f32>(cos(theta), sin(theta), -0.25));
}

fn twilight_weight_from_sun_elevation(sun_y: f32) -> f32 {
    let near_horizon = 1.0 - smoothstep(0.02, 0.24, abs(sun_y));
    let not_midnight = smoothstep(-0.24, -0.02, sun_y);
    return clamp(near_horizon * not_midnight, 0.0, 1.0);
}

fn color_saturate(c: vec3<f32>, sat: f32) -> vec3<f32> {
    let l = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    return mix(vec3<f32>(l), c, sat);
}

fn stylize_cloud_tint(base: vec3<f32>, sat: f32, gamma: f32, lift: f32) -> vec3<f32> {
    let s = color_saturate(base, sat);
    let g = pow(max(s, vec3<f32>(0.0)), vec3<f32>(gamma));
    return clamp(g + vec3<f32>(lift), vec3<f32>(0.0), vec3<f32>(2.0));
}


const CLOUD_BASE_HEIGHT: f32 = 0.035;



fn sphere_intersect(view_dir: vec3<f32>, sphere_pos: vec3<f32>, radius: f32) -> f32 {
    let b = dot(-sphere_pos, view_dir);
    let c = dot(-sphere_pos, -sphere_pos) - radius * radius;
    let h = b * b - c;
    if (h < 0.0) { return -1.0; }
    return -b - sqrt(h);
}

// ──────────────────────────────────────────────────────────────
// Stars (procedural point field)
// ──────────────────────────────────────────────────────────────

fn star_point_field(uv: vec2<f32>, scale: f32, density: f32, size: f32) -> f32 {
    let p     = uv * scale;
    let cell  = floor(p);
    let local = fract(p);
    let rnd_x = hash12(cell + vec2<f32>(17.0, 41.0));
    let rnd_y = hash12(cell + vec2<f32>(73.0, 11.0));
    let keep  = select(0.0, 1.0, rnd_x > (1.0 - density));
    let d      = distance(local, vec2<f32>(rnd_x, rnd_y));
    let radius = mix(0.030, 0.010, clamp(size, 0.0, 1.0));
    let core   = smoothstep(radius, radius * 0.10, d);
    let glow   = smoothstep(radius * 3.8, radius * 0.35, d) * 0.35;
    return keep * (core + glow);
}

// ──────────────────────────────────────────────────────────────
// Vertex shader
// ──────────────────────────────────────────────────────────────

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 3.0,  1.0)
    );
    var out: VsOut;
    out.pos = vec4<f32>(pos[vi], 0.0, 1.0);
    out.uv  = out.pos.xy * 0.5 + vec2<f32>(0.5);
    return out;
}

// ──────────────────────────────────────────────────────────────
// Fragment shader
// ──────────────────────────────────────────────────────────────
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let ndc     = vec4<f32>(in.uv * 2.0 - 1.0, 1.0, 1.0);
    let world_h = sky.inv_view_proj * ndc;
    let world   = world_h.xyz / max(world_h.w, 1.0e-5);
    let ray     = normalize(world - sky.camera_pos.xyz);

    let tod     = sky.params0.w;
    let sun_dir = sun_dir_from_time(tod, sky.params1.w);
    let moon_dir = -sun_dir;

    // ── Sky gradient ──────────────────────────────────────────
    let eyedir_y  = abs(sin(ray.y * 3.14159265 * 0.5));
    let horizon_t = smoothstep(-0.32, 0.52, ray.y);

    let sky_time           = sky.params2.w;
    let day_t_runtime      = sky.params2.z;
    let style_blend_global = clamp(sky.wind.z, 0.0, 1.0);
    let day_from_sun       = smoothstep(-0.14, 0.28, sun_dir.y);
    let day_t              = mix(day_t_runtime, day_from_sun, 0.75);
    let night_t            = clamp(1.0 - day_t, 0.0, 1.0);
    let twilight_t = clamp(
        (1.0 - smoothstep(-0.02, 0.32, sun_dir.y))
            * smoothstep(-0.46, 0.08, sun_dir.y)
            * 1.25,
        0.0, 1.0
    );
    let evening_t_raw = clamp(
        mix(twilight_weight_from_sun_elevation(sun_dir.y), twilight_t, 0.72),
        0.0, 1.0
    );
    let sunrise_side     = smoothstep(-0.20, 0.45, sun_dir.x);
    let sunrise_twilight = sunrise_side * evening_t_raw;
    let evening_t        = evening_t_raw * mix(1.0, 0.64, sunrise_twilight);
    let sun_h   = normalize(vec2<f32>(sun_dir.x, sun_dir.z) + vec2<f32>(1.0e-5, 0.0));
    let ray_h   = normalize(vec2<f32>(ray.x,     ray.z)     + vec2<f32>(1.0e-5, 0.0));
    let sun_side = clamp(dot(ray_h, sun_h) * 0.5 + 0.5, 0.0, 1.0);

    let pole_blur = smoothstep(0.80, 0.99, abs(ray.y));
    let dusk_blur = smoothstep(0.05, 0.85, evening_t_raw);
    let gradient_blur = 0.008 + dusk_blur * 0.020 + pole_blur * dusk_blur * 0.018;

    let day_col = gradient3_blur(sky.day_colors, eyedir_y, gradient_blur * 0.60);
    let evening_col_raw = gradient3_blur(sky.evening_colors, horizon_t, gradient_blur * 1.30);
    let evening_col     = mix(
        color_saturate(evening_col_raw, mix(1.0, 0.78, sunrise_twilight)),
        day_col,
        sunrise_twilight * 0.28
    ) * mix(1.0, 0.86, sunrise_twilight);
    let night_col = gradient3_blur(sky.night_colors, eyedir_y, gradient_blur * 0.80);

    var w_day     = smoothstep(-0.10,  0.34, sun_dir.y);
    var w_night   = smoothstep( 0.12, -0.40, sun_dir.y);
    var w_evening = clamp(1.0 - w_day - w_night, 0.0, 1.0)
                  * mix(0.15, 1.0, pow(sun_side, 1.6));
    let w_sum = max(w_day + w_evening + w_night, 1.0e-4);
    w_day     /= w_sum;
    w_evening /= w_sum;
    w_night   /= w_sum;

    var color = day_col * w_day + evening_col * w_evening + night_col * w_night;
    color *= 1.0 - evening_t_raw * (1.0 - sun_side) * 0.18;

    let noon_t = pow(clamp(sun_dir.y * 0.9 + 0.1, 0.0, 1.0), 1.45) * w_day;
    color = mix(color, day_col * vec3<f32>(1.05, 1.08, 1.12), noon_t * 0.20);

    let zenith         = pow(clamp(ray.y, 0.0, 1.0), 1.85);
    let day_glow_col   = vec3<f32>(0.16, 0.22, 0.34);
    let night_glow_col = vec3<f32>(0.07, 0.05, 0.12);
    let zenith_glow    = day_glow_col * day_t + night_glow_col * night_t;
    color += zenith_glow * zenith * 0.28;

    let sun_warm = day_t * smoothstep(-0.10, 0.85, sun_dir.y);
    color *= vec3<f32>(1.0 + 0.02 * sun_warm, 1.0, 1.0 - 0.004 * sun_warm);

    let purple_night = smoothstep(0.25, 1.0, night_t);
    color = mix(color, color * vec3<f32>(1.03, 1.00, 1.06), purple_night * 0.32);

    let cloud_density = clamp(sky.params0.y, 0.0, 1.0);
    let clouds_cutoff = mix(0.84, 0.52, cloud_density);
    let clouds_weight = 0.0;
    color = mix(color, vec3<f32>(0.0),
        clamp((0.7 - clouds_cutoff) * clouds_weight, 0.0, 1.0));

    // ── Horizon ───────────────────────────────────────────────
    var horizon_amount = 0.0;
    let horizon_blur   = 0.075;
    if (ray.y < 0.01) {
        horizon_amount = clamp(abs(ray.y) / horizon_blur, 0.0, 1.0);
        var h_color = mix(color, night_col, night_t * 0.9);
        h_color = mix(h_color, vec3<f32>(0.0),
            (1.0 - clouds_cutoff) * clouds_weight * 0.7);
        color = mix(color, h_color, horizon_amount);
    }

