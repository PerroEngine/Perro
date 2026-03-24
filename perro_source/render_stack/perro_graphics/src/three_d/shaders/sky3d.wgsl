struct SkyUniform {
    inv_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    day_colors: array<vec4<f32>, 3>,
    evening_colors: array<vec4<f32>, 3>,
    night_colors: array<vec4<f32>, 3>,
    params0: vec4<f32>, // cloud_size, cloud_density, cloud_variance, time_of_day
    params1: vec4<f32>, // star_size, star_scatter, star_gleam, sky_angle
    params2: vec4<f32>, // sun_size, moon_size, day_weight, cloud_time_seconds
    wind: vec4<f32>, // x,y cloud wind, z style_blend (0 toon, 1 realistic)
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
        return mix(colors[0].xyz, colors[1].xyz, t * 2.0);
    }
    return mix(colors[1].xyz, colors[2].xyz, (t - 0.5) * 2.0);
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

const PRIME_X: i32 = 501125321;
const PRIME_Y: i32 = 1136930381;

fn _cubic_lerp(a: f32, b: f32, c: f32, d: f32, t: f32) -> f32 {
    let p = d - c - (a - b);
    return t * t * t * p + t * t * (a - b - p) + t * (c - a) + b;
}

fn _ping_pong(t: f32) -> f32 {
    var v = t - trunc(t * 0.5) * 2.0;
    return select(2.0 - v, v, v < 1.0);
}

fn _val_coord(seed: i32, xp: i32, yp: i32) -> f32 {
    var h: i32 = (seed ^ xp ^ yp) * 0x27d4eb2d;
    h = h * h;
    h = h ^ (h << 19u);
    return f32(h) * (1.0 / 2147483648.0);
}

fn _single_value_cubic(seed: i32, x: f32, y: f32) -> f32 {
    let x1i = i32(floor(x));
    let y1i = i32(floor(y));
    let xs = x - f32(x1i);
    let ys = y - f32(y1i);

    let x1 = x1i * PRIME_X;
    let y1 = y1i * PRIME_Y;
    let x0 = x1 - PRIME_X;
    let y0 = y1 - PRIME_Y;
    let x2 = x1 + PRIME_X;
    let y2 = y1 + PRIME_Y;
    let x3 = x2 + PRIME_X;
    let y3 = y2 + PRIME_Y;

    return _cubic_lerp(
        _cubic_lerp(_val_coord(seed,x0,y0), _val_coord(seed,x1,y0), _val_coord(seed,x2,y0), _val_coord(seed,x3,y0), xs),
        _cubic_lerp(_val_coord(seed,x0,y1), _val_coord(seed,x1,y1), _val_coord(seed,x2,y1), _val_coord(seed,x3,y1), xs),
        _cubic_lerp(_val_coord(seed,x0,y2), _val_coord(seed,x1,y2), _val_coord(seed,x2,y2), _val_coord(seed,x3,y2), xs),
        _cubic_lerp(_val_coord(seed,x0,y3), _val_coord(seed,x1,y3), _val_coord(seed,x2,y3), _val_coord(seed,x3,y3), xs),
        ys
    ) * (1.0 / 2.25); // 1 / (1.5 * 1.5)
}

// 5-octave fractal ping-pong noise.
// seed_in: per-layer seed (0=top, 1=mid, 2=bottom)
// frequency: per-layer base frequency (0.5, 0.75, 1.0)
fn gen_fractal_ping_pong(pos: vec2<f32>, seed_in: i32, frequency: f32) -> f32 {
    let FRACTAL_BOUNDING: f32  = 1.0 / 1.75;
    let PING_PONG_STRENGTH: f32 = 2.0;
    let GAIN: f32               = 0.5;
    let LACUNARITY: f32         = 2.0;

    var x   = pos.x * frequency;
    var y   = pos.y * frequency;
    var sum = 0.0;
    var amp = FRACTAL_BOUNDING;
    var s   = seed_in;

    for (var i = 0; i < 5; i++) {
        let noise = _ping_pong((_single_value_cubic(s, x, y) + 1.0) * PING_PONG_STRENGTH);
        sum += (noise - 0.5) * 2.0 * amp;
        x   *= LACUNARITY;
        y   *= LACUNARITY;
        amp *= GAIN;
        s   += 1;
    }
    return sum * 0.5 + 0.5;
}

// ──────────────────────────────────────────────────────────────
// Moon phase (sphere intersection — ported from Godot shader)
// Source: https://kelvinvanhoorn.com/2022/03/17/skybox-tutorial-part-1/
// ──────────────────────────────────────────────────────────────

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

    let tod      = sky.params0.w;
    let sun_dir  = sun_dir_from_time(tod, sky.params1.w);
    let moon_dir = -sun_dir;

    // ── Sky gradient ──────────────────────────────────────────
    // eyedir_y mirrors Godot's abs(sin(EYEDIR.y * PI * 0.5)) — gives a
    // perceptually even horizon-to-zenith gradient that doesn't rush at the top.
    let eyedir_y  = abs(sin(ray.y * 3.14159265 * 0.5));
    let horizon_t = smoothstep(-0.32, 0.52, ray.y); // separate remap used by evening blend

    let sky_time      = sky.params2.w;
    let day_t_runtime = sky.params2.z;
    let day_from_sun  = smoothstep(-0.14, 0.28, sun_dir.y);
    let day_t         = mix(day_t_runtime, day_from_sun, 0.75);
    let night_t       = clamp(1.0 - day_t, 0.0, 1.0);
    let twilight_t = clamp(
        (1.0 - smoothstep(-0.02, 0.32, sun_dir.y)) * smoothstep(-0.46, 0.08, sun_dir.y) * 1.25,
        0.0,
        1.0
    );
    let evening_t_raw = clamp(mix(twilight_weight_from_sun_elevation(sun_dir.y), twilight_t, 0.72), 0.0, 1.0);
    let sunrise_side = smoothstep(-0.20, 0.45, sun_dir.x); // 1=dawn side, 0=dusk side
    let sunrise_twilight = sunrise_side * evening_t_raw;
    let evening_t = evening_t_raw * mix(1.0, 0.64, sunrise_twilight);
    let sun_h = normalize(vec2<f32>(sun_dir.x, sun_dir.z) + vec2<f32>(1.0e-5, 0.0));
    let ray_h = normalize(vec2<f32>(ray.x, ray.z) + vec2<f32>(1.0e-5, 0.0));
    let sun_side = clamp(dot(ray_h, sun_h) * 0.5 + 0.5, 0.0, 1.0);

    let day_col         = gradient3(sky.day_colors,     eyedir_y);
    let evening_col_raw = gradient3(sky.evening_colors, horizon_t);
    let evening_col     = mix(
        color_saturate(evening_col_raw, mix(1.0, 0.78, sunrise_twilight)),
        day_col,
        sunrise_twilight * 0.28
    ) * mix(1.0, 0.86, sunrise_twilight);
    let night_col   = gradient3(sky.night_colors,   eyedir_y);

    // Smooth phase blend across sunrise → noon → evening/sunset → night.
    var w_day = smoothstep(-0.10, 0.34, sun_dir.y);
    var w_night = smoothstep(0.12, -0.40, sun_dir.y);
    var w_evening = clamp(1.0 - w_day - w_night, 0.0, 1.0) * mix(0.15, 1.0, pow(sun_side, 1.6));
    let w_sum = max(w_day + w_evening + w_night, 1.0e-4);
    w_day /= w_sum;
    w_evening /= w_sum;
    w_night /= w_sum;
    var color = day_col * w_day + evening_col * w_evening + night_col * w_night;
    color *= 1.0 - evening_t_raw * (1.0 - sun_side) * 0.18;
    let noon_t = pow(clamp(sun_dir.y * 0.9 + 0.1, 0.0, 1.0), 1.45) * w_day;
    color = mix(color, day_col * vec3<f32>(1.05, 1.08, 1.12), noon_t * 0.20);
    // Zenith atmospheric glow lift (slightly overbright at top of sky).
    let zenith = pow(clamp(ray.y, 0.0, 1.0), 1.85);
    let day_glow_col = vec3<f32>(0.16, 0.22, 0.34);
    let night_glow_col = vec3<f32>(0.07, 0.05, 0.12);
    let zenith_glow = day_glow_col * day_t + night_glow_col * night_t;
    color += zenith_glow * zenith * 0.28;

    // Daytime: subtle warm/red sun bias (~2% at peak day).
    let sun_warm = day_t * smoothstep(-0.10, 0.85, sun_dir.y);
    color *= vec3<f32>(1.0 + 0.02 * sun_warm, 1.0, 1.0 - 0.004 * sun_warm);

    // Nighttime: slight purple cast.
    let purple_night = smoothstep(0.25, 1.0, night_t);
    color = mix(color, color * vec3<f32>(1.03, 1.00, 1.06), purple_night * 0.35);

    // Storm darkening (clouds_weight = 0 by default; wire up if you want rain mode)
    let cloud_density  = clamp(sky.params0.y, 0.0, 1.0);
    let clouds_cutoff  = mix(0.84, 0.52, cloud_density);
    let clouds_weight  = 0.0; // expose via uniform for storm/rain effect
    color = mix(color, vec3<f32>(0.0), clamp((0.7 - clouds_cutoff) * clouds_weight, 0.0, 1.0));

    // ── Horizon ───────────────────────────────────────────────
    // Explicit below-horizon band darkening, ported from Godot.
    // Keeps horizon dark at night and with heavy clouds independently
    // of the sky gradient above.
    var horizon_amount = 0.0;
    let horizon_blur   = 0.05;
    if (ray.y < 0.0) {
        horizon_amount = clamp(abs(ray.y) / horizon_blur, 0.0, 1.0);
        // Night darkening: pull horizon toward night sky (* 0.9 matches Godot)
        var h_color = mix(color, night_col, night_t * 0.9);
        // Heavy cloud darkening
        h_color = mix(h_color, vec3<f32>(0.0), (1.0 - clouds_cutoff) * clouds_weight * 0.7);
        color = mix(color, h_color, horizon_amount);
    }

    // ── Moon ──────────────────────────────────────────────────
    // Visible on the night side (day_t < 0.5), mirrors Godot's is_night guard.
    var moon_amount = 0.0;
    let moon_size_ctrl = clamp(sky.params2.y, 0.0, 5.0);
    let moon_size      = mix(0.004, 0.040, clamp(moon_size_ctrl / 5.0, 0.0, 1.0)) * 3.0;
    
    // Fade out moon when 25 degrees below horizon (exactly at -0.4226)
    let moon_elevation = dot(ray, moon_dir);
    let moon_visibility = smoothstep(-0.46, -0.42, moon_elevation); // Fade out at exactly 25° below horizon
    
    if (day_t < 0.5 && moon_visibility > 0.0) {
        let moon_horizon_vis = smoothstep(-0.24, 0.06, moon_dir.y);
        let moon_night_vis = smoothstep(0.45, 0.88, night_t);
        // Fixed size - no longer changes with elevation
        let moon_dist     = distance(ray, moon_dir) / moon_size;
        let moon_blur     = 0.1;
        moon_amount = clamp((1.0 - moon_dist) / moon_blur, 0.0, 1.0);
        moon_amount *= moon_horizon_vis * moon_night_vis * moon_visibility;

        if (moon_amount > 0.0) {
            // Phase shading: sphere intersection gives a surface normal,
            // then N·L from the sun position determines the lit crescent.
            let moon_intersect = sphere_intersect(ray, moon_dir, moon_size);
            let moon_normal    = normalize(moon_dir - ray * moon_intersect);
            let moon_ndotl     = pow(clamp(dot(moon_normal, -sun_dir), 0.0, 1.0), 1.8);
            moon_amount *= 1.0 - horizon_amount;
            let moon_ref_a = vec3<f32>(1.0, 0.0, 0.0);
            let moon_ref_b = vec3<f32>(0.0, 0.0, 1.0);
            let moon_ref_blend = smoothstep(0.82, 0.98, abs(dot(moon_ref_a, moon_dir)));
            let moon_ref = normalize(mix(moon_ref_a, moon_ref_b, moon_ref_blend));
            let moon_tan = normalize(moon_ref - moon_dir * dot(moon_ref, moon_dir));
            let moon_bit = normalize(cross(moon_dir, moon_tan));
            let moon_local = vec2<f32>(dot(ray, moon_tan), dot(ray, moon_bit)) / max(moon_size, 1.0e-4);
            // Larger, darker, softer crater fields (less sharp).
            let moon_fbm_base = fbm2(moon_local * 2.25 + vec2<f32>(7.0, -11.0));
            let moon_fbm_detail = fbm2(moon_local * 5.4 + vec2<f32>(-3.0, 13.0));
            let crater_seed = moon_fbm_detail * 0.58 + moon_fbm_base * 0.42;
            let crater_basin = smoothstep(0.44, 0.80, crater_seed);
            let maria = smoothstep(0.38, 0.70, moon_fbm_base);
            let crater_mask = clamp(crater_basin * 0.98 + maria * 0.40, 0.0, 1.0);

            let moon_col = vec3<f32>(0.90, 0.91, 0.94) - vec3<f32>(0.42, 0.40, 0.44) * crater_mask;
            color = mix(color, moon_col, moon_ndotl * moon_amount * moon_horizon_vis * moon_night_vis);

            // Bloomed moon outline/rim (suppressed near horizon).
            let moon_rim = 1.0 - smoothstep(0.76, 1.32, moon_dist);
            let moon_rim_col = vec3<f32>(0.80, 0.84, 1.0);
            let moon_rim_sky = smoothstep(0.30, 0.48, moon_dir.y);
            let moon_rim_night = smoothstep(-0.14, -0.34, sun_dir.y);
            color += moon_rim_col * moon_rim * moon_amount * 0.08 * moon_rim_sky * moon_rim_night;
        }
    }

    // ── Sun ───────────────────────────────────────────────────
    // Visible on the day side (day_t > 0.5), mirrors Godot's !is_night guard.
    let sun_size_ctrl = clamp(sky.params2.x, 0.0, 8.0);
    let sun_size = mix(0.006, 0.055, clamp(sun_size_ctrl / 8.0, 0.0, 1.0)) * 3.0;
    
    // Fade out sun when 25 degrees below horizon (exactly at -0.4226)
    let sun_elevation = dot(ray, sun_dir);
    let sun_visibility = smoothstep(-0.46, -0.42, sun_elevation); // Fade out at exactly 25° below horizon

    // Only render sun if it's roughly in front of the camera
    if (sun_dir.y > -0.34 && sun_elevation > 0.0 && sun_visibility > 0.0) {
        // Fixed size - no longer changes with elevation
        let sun_up = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(sun_dir.y) > 0.95);
        let sun_tan = normalize(cross(sun_up, sun_dir));
        let sun_bit = normalize(cross(sun_dir, sun_tan));
        let sun_local = vec2<f32>(dot(ray, sun_tan), dot(ray, sun_bit)) / max(sun_size, 1.0e-4);
        let sun_r = length(sun_local);
        let sun_vis = smoothstep(-0.30, -0.06, sun_dir.y);
        let twilight_size_boost = 1.0 + evening_t_raw * 0.95;
        let sun_growth = mix(0.46, 0.72, sun_vis) * twilight_size_boost;
        let sun_pulse = 1.0 + 0.035 * sin(sky_time * 1.25);
        let sun_radius_anim = sun_growth * sun_pulse;
        let sun_blur = 0.22;
        let sun_r_anim = sun_r / max(sun_radius_anim, 0.20);
        var sun_amount = clamp((1.0 - sun_r_anim) / sun_blur, 0.0, 1.0);

        if (sun_r < 4.5 && sun_vis > 0.0) {
            // Colour transitions toward sunset red at the horizon via evening_t
            let sun_day_col = vec3<f32>(9.8, 6.9, 1.35);
            let sun_set_col_strong = vec3<f32>(16.0, 3.8, 1.0);
            let sun_set_col_dawn = vec3<f32>(11.4, 7.3, 3.1);
            let sun_set_col = mix(sun_set_col_strong, sun_set_col_dawn, sunrise_twilight * 0.92);
            var sun_col = mix(sun_day_col, sun_set_col, pow(evening_t_raw, 0.72) * mix(1.0, 0.72, sunrise_twilight));
            sun_col = color_saturate(sun_col, mix(1.0, 0.74, sunrise_twilight));
            sun_col *= 1.0 + evening_t_raw * mix(0.55, 0.26, sunrise_twilight);

            sun_amount = clamp(sun_amount * (1.0 - moon_amount), 0.0, 1.0);
            sun_amount *= mix(1.0, 1.0 - horizon_amount, 0.25) * sun_visibility;
            let sun_core_amount = sun_amount * 0.36;

            // HDR levelling: scale bright colour down by amount to avoid uniform blow-out
            if (sun_col.r > 1.0 || sun_col.g > 1.0 || sun_col.b > 1.0) {
                sun_col *= sun_core_amount;
            }
            color = mix(color, sun_col, sun_core_amount);

            // Smooth center-out radial bloom (no ring banding).
            let halo_vis = smoothstep(-0.02, 0.22, sun_dir.y);
            let halo_growth = mix(0.35, 1.0, halo_vis);
            let sun_r_halo = sun_r_anim / max(halo_growth, 0.15);
            let core_bloom = exp(-sun_r_halo * sun_r_halo * 2.7);
            let mid_bloom  = exp(-sun_r_halo * sun_r_halo * 0.88);
            let far_bloom  = exp(-sun_r_halo * sun_r_halo * 0.22);
            let bloom_noise = 0.97 + 0.03 * fbm2(sun_local * 2.5 + vec2<f32>(sky_time * 0.04, -sky_time * 0.03));
            let twinkle = 0.96 + 0.04 * sin(sky_time * 2.0 + atan2(sun_local.y, sun_local.x) * 1.6);
            let flourish = 0.96 + 0.04 * sin(sky_time * 1.2);
            let flare = (core_bloom * 0.52 + mid_bloom * 0.92 + far_bloom * 1.36) * bloom_noise * twinkle * flourish;
            let flare_col = mix(vec3<f32>(1.0, 0.84, 0.40), vec3<f32>(1.0, 0.62, 0.34), evening_t);
            let flare_amt = clamp(flare * day_t * halo_vis * (1.0 - horizon_amount * 0.75) * 0.52, 0.0, 2.0);
            color += flare_col * flare_amt * sun_visibility;
        }
    }
    
    // ── Stars ─────────────────────────────────────────────────
    // Same UV projection as Godot (ray.xz / sqrt(ray.y)) with a slow
    // rotation over time matching Godot's stars_speed * time * 0.005.
    let cloud_time_seconds = sky.params2.w;
    let star_size    = clamp(sky.params1.x, 0.05, 4.0);
    let star_scatter = clamp(sky.params1.y, 0.0, 1.0);
    let star_gleam   = clamp(sky.params1.z, 0.0, 2.0);
    let stars_speed  = 1.0; // wire to a uniform if you need per-scene control

    if (ray.y > -0.01 && day_t < 0.5) {
        let stars_uv = dir_to_octa_uv(ray);

        let star_density = mix(0.0045, 0.018, star_scatter);
        let scale_a      = mix(180.0, 360.0, star_scatter);
        let scale_b      = scale_a * 1.73;
        let point_size   = clamp((star_size - 0.2) / 1.8, 0.0, 1.0);

        let stars_a = star_point_field(stars_uv + vec2<f32>( 13.2, -9.7), scale_a, star_density,        point_size);
        let stars_b = star_point_field(stars_uv + vec2<f32>(-21.1,  7.4), scale_b, star_density * 0.42, point_size * 0.85);
        var stars   = clamp(stars_a + stars_b, 0.0, 1.0);

        // Hide stars behind moon (matches Godot _stars_color *= 1.0 - _moon_amount)
        stars *= 1.0 - moon_amount;

        let star_seed   = hash12(floor(stars_uv * scale_a));
        let twinkle     = 0.82 + 0.18 * sin(cloud_time_seconds * 0.65 * (2.2 + star_seed * 9.0));
        let gleam_boost = 1.0 + star_gleam * 1.35;
        let night_alpha = pow(night_t, 3.1);
        let star_band   = smoothstep(0.04, 0.20, ray.y);
        let star_col    = mix(vec3<f32>(0.72, 0.78, 1.0), vec3<f32>(1.0, 0.97, 0.89), star_seed);

        color += star_col * stars * night_alpha * star_band * twinkle * gleam_boost;
    }

    // ── Clouds ────────────────────────────────────────────────
    // Noise: replaced fbm2 with gen_fractal_ping_pong (5-octave cubic ping-pong)
    // matching the Godot shader. Everything else — smoothstep layering, sun
    // pass-through, time-of-day tinting — is kept from the WGSL version.
    var cloud_mask = 0.0;
    if (ray.y > -0.06) {
        let cloud_size     = clamp(sky.params0.x, 0.0, 1.0);
        let cloud_variance = clamp(sky.params0.z, 0.0, 1.0);
        let style_blend    = clamp(sky.wind.z, 0.0, 1.0);
        let wind           = vec2<f32>(sky.wind.x, sky.wind.y);
        let wind_len       = max(length(wind), 1.0e-4);
        let wind_dir       = wind / wind_len;

        // Same UV projection as Godot: denser shapes near zenith, stretched near horizon
        let sky_uv_y    = max(ray.y + 0.06, 0.015);
        let sky_uv      = ray.xz / sqrt(sky_uv_y);
        let cloud_clock = cloud_time_seconds * (0.011 + wind_len * 0.06);
        let clouds_scale     = mix(1.55, 0.55, cloud_size);
        let clouds_fuzziness = mix(0.035, 0.24, cloud_variance);
        let coverage_bias    = mix(0.02, 0.14, cloud_density);
        let macro_shape      = gen_fractal_ping_pong(
            sky_uv * (clouds_scale * 0.45) + vec2<f32>(41.0, -29.0), 7, 0.5
        );
        let macro_bias       = smoothstep(0.35, 1.0, macro_shape) * 0.22;

        // Three cloud layers — speed ratios and seed offsets match Godot exactly
        var drift = wind_dir * cloud_clock;
        let noise_top_raw = gen_fractal_ping_pong(
            (sky_uv + drift) * clouds_scale, 0, 0.5
        );

        drift = wind_dir * cloud_clock * 0.89 + vec2<f32>(0.009, -0.013);
        let noise_middle_raw = gen_fractal_ping_pong(
            (sky_uv + drift) * clouds_scale, 1, 0.75
        );

        drift = wind_dir * cloud_clock * 0.79 + vec2<f32>(-0.014, 0.006);
        let noise_bottom_raw = gen_fractal_ping_pong(
            (sky_uv + drift) * clouds_scale, 2, 1.0
        );
        let detail_wisp = gen_fractal_ping_pong(
            (sky_uv + wind_dir * cloud_clock * 1.33 + vec2<f32>(19.0, -31.0)) * (clouds_scale * 2.15),
            11,
            1.15
        );
        let detail_puff = gen_fractal_ping_pong(
            (sky_uv + wind_dir * cloud_clock * 0.57 + vec2<f32>(-7.0, 23.0)) * (clouds_scale * 0.90),
            13,
            0.55
        );

        // Godot smoothstep layering: each lower layer bleeds into the one above,
        // giving the illusion that clouds have internal depth and weight.
        let noise_bottom = smoothstep(clouds_cutoff, clouds_cutoff + clouds_fuzziness,
                                      noise_bottom_raw + coverage_bias + macro_bias * 0.80);
        let noise_middle = smoothstep(clouds_cutoff, clouds_cutoff + clouds_fuzziness,
                                      noise_middle_raw + noise_bottom * 0.22 + coverage_bias * 0.85 + macro_bias * 0.65) * 1.12;
        let noise_top_s  = smoothstep(clouds_cutoff, clouds_cutoff + clouds_fuzziness,
                                      noise_top_raw + noise_middle * 0.42 + coverage_bias * 0.70 + macro_bias * 0.50) * 1.22;

        var clouds_base = clamp(noise_top_s * 0.85 + noise_middle * 1.08 + noise_bottom * 1.25, 0.0, 1.0);
        clouds_base = pow(clouds_base, mix(1.0, 0.72, cloud_density));
        clouds_base = clamp(clouds_base + cloud_density * 0.14 + macro_bias * 0.18, 0.0, 1.0);

        let realistic_erosion = smoothstep(0.15, 0.95, detail_wisp);
        let realistic_puffs = smoothstep(0.36, 0.88, detail_puff);
        var clouds_amount_real = clouds_base * mix(0.86, 1.20, realistic_puffs);
        clouds_amount_real *= mix(1.10, 0.78, realistic_erosion);
        clouds_amount_real = clamp(clouds_amount_real, 0.0, 1.0);

        let toon_pattern = hash12(floor((sky_uv + vec2<f32>(cloud_time_seconds * 0.02, -cloud_time_seconds * 0.015)) * 28.0));
        let toon_breakup = (toon_pattern - 0.5) * (0.06 + cloud_variance * 0.10);
        let toon_steps = mix(3.0, 5.0, cloud_size);
        var clouds_amount_toon = floor(clamp(clouds_base + toon_breakup, 0.0, 1.0) * toon_steps) / toon_steps;
        clouds_amount_toon = smoothstep(0.02, 0.98, clouds_amount_toon);

        var clouds_amount = mix(clouds_amount_toon, clouds_amount_real, style_blend);
        // Continuous horizon blend so clouds do not hard-cut at y ~= 0.
        let horizon_continuity = smoothstep(-0.06, 0.08, ray.y);
        clouds_amount *= horizon_continuity;

        let clouds_edge_color_toon   = vec3<f32>(0.82, 0.84, 1.00);
        let clouds_top_color_toon    = vec3<f32>(1.00, 1.00, 1.00);
        let clouds_middle_color_toon = vec3<f32>(0.92, 0.93, 0.99);
        let clouds_bottom_color_toon = vec3<f32>(0.82, 0.84, 0.95);
        var clouds_color_toon = mix(vec3<f32>(0.0), clouds_top_color_toon, noise_top_s);
        clouds_color_toon = mix(clouds_color_toon, clouds_middle_color_toon, noise_middle);
        clouds_color_toon = mix(clouds_color_toon, clouds_bottom_color_toon, noise_bottom);
        clouds_color_toon = mix(clouds_edge_color_toon, clouds_color_toon, noise_top_s);
        let toon_tex = hash12(floor((sky_uv + vec2<f32>(7.0, -13.0)) * 36.0));
        let toon_tex_band = floor((toon_tex * 0.55 + detail_wisp * 0.45) * 4.0) / 4.0;
        clouds_color_toon *= mix(0.92, 1.06, toon_tex_band);
        let toon_ink_rim = smoothstep(0.25, 0.82, noise_top_s) * (1.0 - smoothstep(0.78, 1.0, noise_top_s)) * (0.65 + detail_wisp * 0.35);
        clouds_color_toon = mix(clouds_color_toon * 0.88, clouds_color_toon, 1.0 - toon_ink_rim * 0.28);

        let clouds_edge_color_real   = vec3<f32>(0.74, 0.77, 0.84);
        let clouds_top_color_real    = vec3<f32>(0.97, 0.98, 0.99);
        let clouds_middle_color_real = vec3<f32>(0.87, 0.89, 0.93);
        let clouds_bottom_color_real = vec3<f32>(0.73, 0.76, 0.82);
        var clouds_color_real = mix(vec3<f32>(0.0), clouds_top_color_real, noise_top_s);
        clouds_color_real = mix(clouds_color_real, clouds_middle_color_real, noise_middle);
        clouds_color_real = mix(clouds_color_real, clouds_bottom_color_real, noise_bottom);
        clouds_color_real = mix(clouds_edge_color_real, clouds_color_real, noise_top_s);
        let self_shadow = smoothstep(0.22, 0.86, realistic_erosion) * (0.25 + (1.0 - realistic_puffs) * 0.50);
        clouds_color_real *= vec3<f32>(1.0 - self_shadow * 0.22, 1.0 - self_shadow * 0.18, 1.0 - self_shadow * 0.12);
        let cotton_lobe = smoothstep(0.48, 0.95, realistic_puffs) * (1.0 - realistic_erosion * 0.65);
        clouds_color_real += vec3<f32>(0.08, 0.08, 0.09) * cotton_lobe;

        // Sun shining through clouds — only on the day side
        let sun_dist_c = distance(ray, sun_dir);
        if (day_t > 0.5) {
            let sun_day_col    = vec3<f32>(10.0, 8.0, 1.0);
            let sun_set_col    = vec3<f32>(10.0, 0.0, 0.0);
            let sun_col_clouds_toon = clamp(mix(sun_day_col, sun_set_col, evening_t),
                                       vec3<f32>(0.0), vec3<f32>(1.0));
            let sun_col_clouds_real = clamp(mix(vec3<f32>(1.00, 0.95, 0.84), vec3<f32>(1.00, 0.62, 0.44), evening_t),
                                       vec3<f32>(0.0), vec3<f32>(1.0));
            let sun_punch = pow(1.0 - clamp(sun_dist_c, 0.0, 1.0), 5.0);
            clouds_color_toon = mix(clouds_color_toon, sun_col_clouds_toon, sun_punch);
            clouds_color_real = mix(clouds_color_real, sun_col_clouds_real, sun_punch * 0.65);
        }

        // Time-of-day tinting — kept from the WGSL version (richer than Godot's
        // simple night lerp, and uses the continuous day_t / evening_t values).
        let day_src   = gradient3(sky.day_colors,     0.34);
        let eve_src   = gradient3(sky.evening_colors, 0.40);
        let night_src = gradient3(sky.night_colors,   0.36);
        let day_tint   = stylize_cloud_tint(day_src   * vec3<f32>(1.07, 1.02, 0.96), 1.12, 0.93, 0.01);
        let eve_tint   = stylize_cloud_tint(eve_src   * vec3<f32>(1.16, 0.92, 0.98), 1.22, 0.91, 0.01);
        let night_tint = stylize_cloud_tint(night_src * vec3<f32>(0.95, 0.90, 1.28), 1.20, 0.95, 0.00);
        let sky_tint   = clamp(
            day_tint * day_t + eve_tint * evening_t + night_tint * night_t,
            vec3<f32>(0.0), vec3<f32>(2.0)
        );
        clouds_color_toon = clouds_color_toon * mix(vec3<f32>(1.0), sky_tint, 0.22);
        clouds_color_real = clouds_color_real * mix(vec3<f32>(1.0), sky_tint, 0.09);

        let edge_band = clamp((noise_top_s - noise_middle * 0.45) + (1.0 - noise_bottom) * 0.12, 0.0, 1.0);
        let edge_glow = smoothstep(0.18, 0.78, edge_band);
        let edge_tint = clamp(mix(day_tint, eve_tint, evening_t) * vec3<f32>(1.12, 1.00, 1.18),
                              vec3<f32>(0.0), vec3<f32>(2.0));
        clouds_color_toon += edge_tint * edge_glow * 0.38;
        clouds_color_real += edge_tint * edge_glow * 0.11;

        var clouds_color = mix(clouds_color_toon, clouds_color_real, style_blend);

        // Fade clouds toward sky colour at night — clamp(night_t, 0, 0.98) mirrors Godot
        clouds_color = mix(clouds_color, color, clamp(night_t, 0.0, 0.98));
        // Storm darkening
        clouds_color = mix(clouds_color, vec3<f32>(0.0), clouds_weight * 0.9);

        let halo = smoothstep(0.18, 0.62, clouds_amount) * (1.0 - smoothstep(0.62, 0.96, clouds_amount));
        let halo_tint = mix(vec3<f32>(1.00, 0.95, 0.86), vec3<f32>(1.00, 0.65, 0.58), evening_t);
        color += halo_tint * halo * (1.0 - style_blend * 0.65) * 0.06;

        color      = mix(color, clouds_color, clouds_amount);
        cloud_mask = clouds_amount;
    }

    // ── Dither ────────────────────────────────────────────────
    let dither = (hash12(in.uv * vec2<f32>(1920.0, 1080.0)
                  + vec2<f32>(tod * 317.0, tod * 911.0)) - 0.5) * (1.0 / 255.0);
    color += vec3<f32>(dither);

    return vec4<f32>(max(color, vec3<f32>(0.0)), 1.0);
}