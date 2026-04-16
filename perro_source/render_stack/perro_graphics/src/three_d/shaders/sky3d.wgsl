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

fn pow5(x: f32) -> f32 {
    let x2 = x * x;
    return x2 * x2 * x;
}

fn pow12(x: f32) -> f32 {
    let x2 = x * x;
    let x4 = x2 * x2;
    let x8 = x4 * x4;
    return x8 * x4;
}

fn pow14(x: f32) -> f32 {
    let x2 = x * x;
    let x4 = x2 * x2;
    let x8 = x4 * x4;
    return x8 * x4 * x2;
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
    let wind               = vec2<f32>(sky.wind.x, sky.wind.y);
    let wind_len           = max(length(wind), 1.0e-4);
    let wind_dir           = wind / wind_len;
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

    let day_col         = gradient3(sky.day_colors,     eyedir_y);
    let evening_col_raw = gradient3(sky.evening_colors, horizon_t);
    let evening_col     = mix(
        color_saturate(evening_col_raw, mix(1.0, 0.78, sunrise_twilight)),
        day_col,
        sunrise_twilight * 0.28
    ) * mix(1.0, 0.86, sunrise_twilight);
    let night_col = gradient3(sky.night_colors, eyedir_y);

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

    // ── Moon ──────────────────────────────────────────────────
    var moon_amount    = 0.0;
    let moon_size_ctrl = clamp(sky.params2.y, 0.0, 5.0);
    let moon_size      = mix(0.004, 0.040, clamp(moon_size_ctrl / 5.0, 0.0, 1.0)) * 2.8;
    let moon_dir_n     = normalize(moon_dir);

    // ── Moon cloud cover ──────────────────────────────────────
    var moon_cloud_cover = 0.0;
    if (moon_dir.y > CLOUD_BASE_HEIGHT && day_t < 0.8) {
        let moon_uv_y      = max(moon_dir.y - CLOUD_BASE_HEIGHT, 0.003) * 0.85;
        let moon_sky_uv    = moon_dir.xz / sqrt(moon_uv_y);
        let cloud_clock_m  = sky_time * (0.011 + wind_len * 0.06);
        let cloud_size_m   = clamp(sky.params0.x, 0.0, 1.0);
        let clouds_scale_m = mix(1.55, 0.55, cloud_size_m);
        let drift_m        = wind_dir * cloud_clock_m * 1.12
                           + vec2<f32>(-0.014, -0.005);
        let moon_cloud_uv  = (moon_sky_uv + drift_m) * clouds_scale_m;
        let moon_cloud_raw     = perlin_fbm(moon_cloud_uv, 2);
        let cloud_density_m    = clamp(sky.params0.y, 0.0, 1.0);
        let clouds_cutoff_m    = mix(0.8, 0.52, cloud_density_m);
        let cloud_variance_m   = clamp(sky.params0.z, 0.0, 1.0);
        let clouds_fuzziness_m = mix(0.035, 0.28, cloud_variance_m);
        moon_cloud_cover = smoothstep(
            clouds_cutoff_m - 0.08,
            clouds_cutoff_m + clouds_fuzziness_m + 0.12,
            moon_cloud_raw + cloud_density_m * 0.10
        );
        let moon_cover_fade = smoothstep(
            CLOUD_BASE_HEIGHT,
            CLOUD_BASE_HEIGHT + 0.2,
            moon_dir.y
        );
        moon_cloud_cover *= moon_cover_fade;
    }

    if (day_t < 0.8) {
        let moon_dist = distance(ray, moon_dir_n) / moon_size;
        let moon_blur = 0.2;
        moon_amount   = clamp((1.0 - moon_dist) / moon_blur, 0.0, 1.0);

        // Fade moon when clouds cover it
        let moon_cloud_fade = 1.0 - clamp(moon_cloud_cover * 0.65, 0.0, 0.85);
        moon_amount *= moon_cloud_fade;

        if (moon_amount > 0.0) {
            let moon_ref_a     = vec3<f32>(1.0, 0.0, 0.0);
            let moon_ref_b     = vec3<f32>(0.0, 0.0, 1.0);
            let moon_ref_blend = smoothstep(0.82, 0.98,
                abs(dot(moon_ref_a, moon_dir_n)));
            let moon_ref = normalize(mix(moon_ref_a, moon_ref_b, moon_ref_blend));
            let moon_tan = normalize(
                moon_ref - moon_dir_n * dot(moon_ref, moon_dir_n));
            let moon_bit   = normalize(cross(moon_dir_n, moon_tan));
            let moon_local = vec2<f32>(dot(ray, moon_tan), dot(ray, moon_bit))
                           / max(moon_size, 1.0e-4);

            let moon_fbm_base   = fbm2(moon_local * 2.8 + vec2<f32>(7.0, -11.0));
            let moon_fbm_detail = fbm2(moon_local * 5.4 + vec2<f32>(-3.0, 13.0));
            let crater_seed  = moon_fbm_detail * 0.58 + moon_fbm_base * 0.42;
            let crater_basin = smoothstep(0.44, 0.80, crater_seed);
            let maria        = smoothstep(0.38, 0.70, moon_fbm_base);
            let crater_mask  = clamp(crater_basin * 0.98 + maria * 0.40, 0.0, 1.0);

            let moon_col = vec3<f32>(0.90, 0.91, 0.94)
                         - vec3<f32>(0.42, 0.40, 0.44) * crater_mask;
            color = mix(color, moon_col, moon_amount);

            let moon_rim     = 1.0 - smoothstep(0.76, 1.32, moon_dist);
            let moon_rim_col = vec3<f32>(0.80, 0.84, 1.0);
            color += moon_rim_col * moon_rim * moon_amount * 0.08;
        }
    }

    // ── Cloud cover sample in sun direction ───────────────────
    let cloud_time_seconds = sky.params2.w;
    var sun_cloud_cover    = 0.0;
    if (sun_dir.y > CLOUD_BASE_HEIGHT) {
        let sun_uv_y       = max(sun_dir.y - CLOUD_BASE_HEIGHT, 0.003) * 0.85;
        let sun_sky_uv     = sun_dir.xz / sqrt(sun_uv_y);
        let cloud_clock_sun = sky_time * (0.011 + wind_len * 0.06);
        let cloud_size_s   = clamp(sky.params0.x, 0.0, 1.0);
        let clouds_scale_s = mix(1.55, 0.55, cloud_size_s);

        let drift_sun    = wind_dir * cloud_clock_sun * 1.12
                         + vec2<f32>(-0.014, -0.005);
        let sun_cloud_uv = (sun_sky_uv + drift_sun) * clouds_scale_s;
        let sun_cloud_raw     = perlin_fbm(sun_cloud_uv, 2);
        let cloud_density_s   = clamp(sky.params0.y, 0.0, 1.0);
        let clouds_cutoff_s   = mix(0.8, 0.52, cloud_density_s);
        let cloud_variance_s  = clamp(sky.params0.z, 0.0, 1.0);
        let clouds_fuzziness_s = mix(0.035, 0.28, cloud_variance_s);
        sun_cloud_cover = smoothstep(
            clouds_cutoff_s - 0.08,
            clouds_cutoff_s + clouds_fuzziness_s + 0.12,
            sun_cloud_raw + cloud_density_s * 0.10
        );
        let sun_cover_fade = smoothstep(
            CLOUD_BASE_HEIGHT,
            CLOUD_BASE_HEIGHT + 0.2,
            sun_dir.y
        );
        sun_cloud_cover *= sun_cover_fade / 10.0;
    }

    // ── Sun ───────────────────────────────────────────────────
    let sun_size_ctrl  = clamp(sky.params2.x, 0.0, 8.0);
    let sun_size       = mix(0.006, 0.055,
        clamp(sun_size_ctrl / 8.0, 0.0, 1.0)) * 3.0;
    let sun_elevation  = dot(ray, sun_dir);
    let sun_visibility = smoothstep(-0.46, -0.42, sun_elevation);

    if (sun_dir.y > -0.34 && sun_elevation > 0.0 && sun_visibility > 0.0) {
        let sun_up  = select(
            vec3<f32>(0.0, 1.0, 0.0),
            vec3<f32>(0.0, 0.0, 1.0),
            abs(sun_dir.y) > 0.95
        );
        let sun_tan   = normalize(cross(sun_up, sun_dir));
        let sun_bit   = normalize(cross(sun_dir, sun_tan));
        let sun_local = vec2<f32>(dot(ray, sun_tan), dot(ray, sun_bit))
                      / max(sun_size, 1.0e-4);
        let sun_r     = length(sun_local);

        let sun_vis            = smoothstep(-0.30, -0.06, sun_dir.y);
        let twilight_size_boost = 1.0 + evening_t_raw * 0.55;
        let sun_growth         = mix(0.52, 0.82, sun_vis) * twilight_size_boost;
        let sun_pulse          = 1.0 + 0.035 * sin(sky_time * 1.25);
        let sun_radius_anim    = sun_growth * sun_pulse;
        let sun_blur           = 0.22;
        let sun_r_anim         = sun_r / max(sun_radius_anim, 0.20);
        let cloud_diffusion    = mix(sun_blur, sun_blur * 3.5, sun_cloud_cover);

        // Clouds dim and blur the sun disk — power curve so partial cover
        let cloud_dim  = mix(1.0, 0.1, pow(sun_cloud_cover, 2.4));
        var sun_amount = clamp((1.0 - sun_r_anim) / cloud_diffusion, 0.0, 1.0);
        sun_amount    *= mix(1.0, 0.38, sun_cloud_cover) * cloud_dim;

        if (sun_r < 4.5 && sun_vis > 0.0) {
            let sun_day_col        = vec3<f32>(7.8,  6.9, 1.35);
            let sun_set_col_strong = vec3<f32>(16.0, 3.8, 1.0);
            let sun_set_col_dawn   = vec3<f32>(11.4, 7.3, 3.1);
            let sun_set_col = mix(sun_set_col_strong, sun_set_col_dawn,
                sunrise_twilight * 0.92);
            var sun_col = mix(sun_day_col, sun_set_col,
                pow(evening_t_raw, 0.72) * mix(1.0, 0.72, sunrise_twilight));
            sun_col  = color_saturate(sun_col, mix(1.0, 0.74, sunrise_twilight));
            sun_col *= 1.0 + evening_t_raw * mix(0.25, 0.15, sunrise_twilight);

            // Dim sun colour itself when heavily cloud-occluded
            sun_col *= mix(1.0, 0.22, pow(sun_cloud_cover, 1.2));

            sun_amount  = clamp(sun_amount * (1.0 - moon_amount), 0.0, 1.0);
            sun_amount *= mix(1.0, 1.0 - horizon_amount, 0.25) * sun_visibility;
            let sun_core_amount = sun_amount * 0.36;

            if (sun_col.r > 1.0 || sun_col.g > 1.0 || sun_col.b > 1.0) {
                sun_col *= sun_core_amount;
            }
            color = mix(color, sun_col, sun_core_amount);

            let halo_vis    = smoothstep(-0.02, 0.25, sun_dir.y);
            let halo_growth = mix(0.35, 1.0, halo_vis);
            let sun_r_halo  = sun_r_anim / max(halo_growth, 0.15);
            let core_bloom  = exp(-sun_r_halo * sun_r_halo * 2.7);
            let mid_bloom   = exp(-sun_r_halo * sun_r_halo * 0.88);
            let far_bloom   = exp(-sun_r_halo * sun_r_halo * 0.22);
            let bloom_noise = 0.97 + 0.03 * fbm2(
                sun_local * 2.5
                + vec2<f32>(sky_time * 0.04, -sky_time * 0.03));
            let twinkle  = 0.96 + 0.04
                * sin(sky_time * 2.0
                + atan2(sun_local.y, sun_local.x) * 1.6);
            let flourish = 0.96 + 0.04 * sin(sky_time * 1.2);
            let flare = (core_bloom * 0.52 + mid_bloom * 0.72 + far_bloom * 0.55)
                    * bloom_noise * twinkle * flourish;
            let flare_col = mix(
                vec3<f32>(1.0, 0.84, 0.40),
                vec3<f32>(1.0, 0.62, 0.34),
                evening_t
            );
            let flare_amt = clamp(
                flare * day_t * halo_vis * (1.0 - horizon_amount * 0.75)
                    * 0.52
                    * mix(1.0, 0.46, style_blend_global)
                    * mix(1.0, 0.18, sun_cloud_cover),
                0.0, 0.8  // was 2.0
            );
            color += flare_col * flare_amt * sun_visibility;
        }
    }

    // ── Atmospheric scatter: sky glow in front of cloud-occluded sun ──
    {
        let scatter_angle = clamp(dot(ray, sun_dir), 0.0, 1.0);
        let scatter_wide  = exp(-max(1.0 - scatter_angle, 0.0) * 2.8) * 0.30;
        let scatter_tight = pow14(scatter_angle) * 0.55;
        let scatter_total = (scatter_wide + scatter_tight)
                          * sun_cloud_cover * day_t * sun_visibility;
        let scatter_col = mix(
            vec3<f32>(0.95, 0.90, 0.72),
            vec3<f32>(1.00, 0.62, 0.38),
            evening_t
        );
        color += scatter_col * scatter_total;
    }

    // ── Stars ─────────────────────────────────────────────────
    let star_size    = clamp(sky.params1.x, 0.05, 4.0);
    let star_scatter = clamp(sky.params1.y, 0.0,  1.0);
    let star_gleam   = clamp(sky.params1.z, 0.0,  2.0);

    if (ray.y > -0.01 && day_t < 0.5) {
        let stars_uv     = dir_to_octa_uv(ray);
        let star_density = mix(0.0045, 0.018, star_scatter);
        let scale_a      = mix(180.0, 360.0, star_scatter);
        let scale_b      = scale_a * 1.73;
        let point_size   = clamp((star_size - 0.2) / 1.8, 0.0, 1.0);

        let stars_a = star_point_field(
            stars_uv + vec2<f32>( 13.2, -9.7), scale_a,
            star_density, point_size);
        let stars_b = star_point_field(
            stars_uv + vec2<f32>(-21.1,  7.4), scale_b,
            star_density * 0.42, point_size * 0.85);
        var stars = clamp(stars_a + stars_b, 0.0, 1.0);
        stars *= 1.0 - moon_amount;

        let star_seed   = hash12(floor(stars_uv * scale_a));
        let twinkle     = 0.82 + 0.18
            * sin(cloud_time_seconds * 0.65 * (2.2 + star_seed * 9.0));
        let gleam_boost = 1.0 + star_gleam * 1.35;
        let night_alpha = pow(night_t, 3.1);
        let star_band   = smoothstep(0.04, 0.20, ray.y);
        let star_col    = mix(
            vec3<f32>(0.72, 0.78, 1.0),
            vec3<f32>(1.0,  0.97, 0.89),
            star_seed
        );
        color += star_col * stars * night_alpha * star_band * twinkle * gleam_boost;
    }

    // ── Clouds ────────────────────────────────────────────────
    var cloud_mask = 0.0;

    if (ray.y > CLOUD_BASE_HEIGHT) {
        let cloud_size     = clamp(sky.params0.x, 0.0, 1.0);
        let cloud_variance = clamp(sky.params0.z, 0.0, 1.0);
        let sky_uv_y_adjusted = max(ray.y - CLOUD_BASE_HEIGHT, 0.003) * 0.85;
        let sky_uv            = ray.xz / sqrt(sky_uv_y_adjusted);

        let cloud_clock      = cloud_time_seconds * (0.011 + wind_len * 0.06);
        let clouds_scale     = mix(1.55, 0.55, cloud_size);
        let clouds_fuzziness = mix(0.035, 0.24, cloud_variance);
        let coverage_bias    = mix(0.02,  0.14, cloud_density);

        // ── Night wisp turbulence ─────────────────────────────
        // Two sub-wind pocket fields that pull clouds apart at night.
        // Ramps with night_t so daytime clouds are completely unaffected.
        let night_pocket_strength = clamp(night_t * 0.72, 0.0, 1.0);
        let night_turb_uv = sky_uv * (clouds_scale * 0.62)
            + wind_dir * cloud_clock * 1.55
            + vec2<f32>(81.0, -53.0);
        let night_pocket_a = perlin_fbm(night_turb_uv * 1.30
            + vec2<f32>( 17.0,  -9.0), 3) * 2.0 - 1.0;
        let night_pocket_b = perlin_fbm(night_turb_uv * 0.85
            + vec2<f32>(-31.0,  23.0), 3) * 2.0 - 1.0;

        // ── Wind shear ────────────────────────────────────────
        let MAX_SHEAR_RAD = 0.436;
        let dist_t = clamp(length(sky_uv) / 6.0, 0.0, 1.0);
        let shear_field = perlin_fbm(sky_uv * 0.08 + vec2<f32>(53.0, -37.0), 2)
                        * 2.0 - 1.0;
        let shear_top = MAX_SHEAR_RAD * dist_t * shear_field;
        let shear_mid = MAX_SHEAR_RAD * dist_t * shear_field
                      * mix(0.68, 0.82, hash12(vec2<f32>(71.0, 13.0)));
        let shear_bot = MAX_SHEAR_RAD * dist_t * shear_field
                      * mix(0.38, 0.55, hash12(vec2<f32>(29.0, 57.0)));
        let shear_low  = MAX_SHEAR_RAD * dist_t * shear_field * 0.44;
        let shear_high = MAX_SHEAR_RAD * dist_t * shear_field * 0.72;

        // ── Macro warping ─────────────────────────────────────
        let macro_uv = sky_uv * (clouds_scale * 0.45) + vec2<f32>(41.0, -29.0);
        let macro_warp = vec2<f32>(
            perlin_fbm(macro_uv * 0.55 + vec2<f32>( 13.0,  -7.0), 2),
            perlin_fbm(macro_uv * 0.55 + vec2<f32>( -5.0,  11.0), 2)
        ) - vec2<f32>(0.5, 0.5);
        let macro_shape = perlin_fbm(macro_uv + macro_warp * 1.35, 3);
        let macro_bias  = smoothstep(0.35, 1.0, macro_shape) * 0.22;

        // ── Top layer ─────────────────────────────────────────
        var drift  = rotate2(wind_dir, shear_top) * cloud_clock * 1.12
                    + vec2<f32>(-0.014, -0.005);
        let top_uv = (sky_uv + drift) * clouds_scale;
        let top_warp = vec2<f32>(
            perlin_fbm(top_uv * 0.7 + vec2<f32>( 19.0, -23.0), 2),
            perlin_fbm(top_uv * 0.7 + vec2<f32>(-17.0,  29.0), 2)
        ) - vec2<f32>(0.5, 0.5);
        let noise_top_raw = perlin_fbm(top_uv + top_warp * 0.95, 4);

        // ── Mid layer ─────────────────────────────────────────
        drift = rotate2(wind_dir, shear_mid) * cloud_clock * 0.94
              + vec2<f32>(0.009, 0.012);
        let mid_uv = (sky_uv + drift) * clouds_scale;
        let mid_warp = vec2<f32>(
            perlin_fbm(mid_uv * 0.68 + vec2<f32>(  7.0, -31.0), 2),
            perlin_fbm(mid_uv * 0.68 + vec2<f32>(-29.0,   5.0), 2)
        ) - vec2<f32>(0.5, 0.5);
        let noise_middle_raw = perlin_fbm(mid_uv + mid_warp * 0.82, 4);

        // ── Bottom layer ──────────────────────────────────────
        // Uses a tighter scale (1.32x) so the dark shadow underside has a
        // smaller footprint than the top, giving clouds that flare wider
        // at the crown — like real cumulonimbus anvils.
        drift = rotate2(wind_dir, shear_bot) * cloud_clock * 0.83
              + vec2<f32>(-0.006, 0.015);
        let bot_uv = (sky_uv + drift) * (clouds_scale * 1.32);
        let bot_warp = vec2<f32>(
            perlin_fbm(bot_uv * 0.75 + vec2<f32>(-13.0,  17.0), 2),
            perlin_fbm(bot_uv * 0.72 + vec2<f32>( 23.0, -11.0), 2)
        ) - vec2<f32>(0.5, 0.5);
        let noise_bottom_raw = perlin_fbm(bot_uv + bot_warp * 0.74, 4);

        // ── Micro detail ──────────────────────────────────────
        let detail_micro = perlin_fbm(
            (sky_uv + wind_dir * cloud_clock * 1.28 + vec2<f32>(9.0, -13.0))
                * (clouds_scale * 1.75),
            3
        );

        // ── Threshold layers ──────────────────────────────────
        let noise_bottom = smoothstep(
            clouds_cutoff, clouds_cutoff + clouds_fuzziness,
            noise_bottom_raw + coverage_bias + macro_bias * 0.80
        ) * 1.10;

        let noise_middle = smoothstep(
            clouds_cutoff, clouds_cutoff + clouds_fuzziness,
            noise_middle_raw + noise_bottom * 0.22
                + coverage_bias * 0.85 + macro_bias * 0.65
        ) * 1.12;

        let noise_top_s = smoothstep(
            clouds_cutoff, clouds_cutoff + clouds_fuzziness,
            noise_top_raw + noise_middle * 0.42
                + coverage_bias * 0.70 + macro_bias * 0.50
        ) * 1.22;

        // ── Extra spread layers ───────────────────────────────
        let low_layer_uv = (sky_uv + rotate2(wind_dir, shear_low) * cloud_clock * 1.25
            + vec2<f32>(-27.0, 14.0)) * (clouds_scale * 0.78);
        let high_layer_uv = (sky_uv + rotate2(wind_dir, shear_high) * cloud_clock * 0.68
            + vec2<f32>(33.0, -21.0)) * (clouds_scale * 1.34);
        let low_layer_raw  = perlin_fbm(low_layer_uv,  3);
        let high_layer_raw = perlin_fbm(high_layer_uv, 3);

        let low_layer = smoothstep(
            clouds_cutoff - 0.06,
            clouds_cutoff + clouds_fuzziness + 0.10,
            low_layer_raw + coverage_bias * 1.05 + macro_bias * 0.55
        );
        let high_layer = smoothstep(
            clouds_cutoff + 0.02,
            clouds_cutoff + clouds_fuzziness + 0.06,
            high_layer_raw + coverage_bias * 0.75 + macro_bias * 0.32
        );

        // ── Base cloud density ────────────────────────────────
        // Bottom layer weight reduced (0.80 vs original 1.15) to match its
        // tighter UV scale — keeps total density balanced.
        var clouds_base = clamp(
            noise_top_s  * 0.80
            + noise_middle * 1.00
            + noise_bottom * 0.80
            + low_layer    * 0.48
            + high_layer   * 0.30,
            0.0, 1.0
        );
        clouds_base = pow(clouds_base, mix(1.0, 0.78, cloud_density));
        clouds_base = clamp(
            clouds_base + cloud_density * 0.14 + macro_bias * 0.18,
            0.0, 1.0
        );

        // ── Erosion / puff / breakup ──────────────────────────
        let realistic_erosion      = smoothstep(0.18, 0.88, noise_top_raw);
        let realistic_puffs        = smoothstep(0.34, 0.90, noise_middle_raw);
        let realistic_fine_breakup = smoothstep(0.22, 0.86, noise_bottom_raw);

        var clouds_amount_real = clouds_base * mix(0.82, 0.96, realistic_puffs);
        clouds_amount_real *= mix(1.02, 0.70, realistic_erosion);
        clouds_amount_real *= mix(1.0,  0.84, realistic_fine_breakup);
        clouds_amount_real  = pow(clamp(clouds_amount_real, 0.0, 1.0), 1.5);

        // ── Wisp masks ────────────────────────────────────────
        let wisp_mask   = smoothstep(0.34, 0.84,
            noise_top_s * (1.0 - noise_bottom));
        let upper_veil  = smoothstep(0.10, 0.72, noise_top_s)
                        * (1.0 - smoothstep(0.56, 1.0, noise_bottom));
        let curved_mass = smoothstep(0.22, 0.86,
            noise_top_s * 0.72 + noise_middle * 0.28);

        let ray_h_cloud   = normalize(vec2<f32>(ray.x, ray.z) + vec2<f32>(1.0e-5));
        let cloud_sun_dot = clamp(dot(ray_h_cloud, sun_h) * 0.5 + 0.5, 0.0, 1.0);
        let sun_behind    = (sun_cloud_cover / 100.0) * day_t;

        let wisp_expanded = smoothstep(0.18, 0.72,
            noise_top_s * (1.0 - noise_bottom * 0.6));
        let wisp_sun = mix(wisp_mask, wisp_expanded,
            sun_behind * cloud_sun_dot * 0.85);

        clouds_amount_real  = mix(clouds_amount_real, curved_mass, 0.26);
        clouds_amount_real += upper_veil * wisp_sun
            * mix(0.24, 0.62, sun_behind * cloud_sun_dot);

        let micro_break = smoothstep(0.32, 0.86, detail_micro);
        clouds_amount_real *= mix(1.08, 0.86, micro_break);
        clouds_amount_real  = pow(clamp(clouds_amount_real, 0.0, 1.0), 0.96);

        let horizon_continuity = smoothstep(
            CLOUD_BASE_HEIGHT, CLOUD_BASE_HEIGHT + 0.06, ray.y);
        clouds_amount_real *= horizon_continuity * 1.1;
        clouds_amount_real  = clamp(clouds_amount_real, 0.0, 1.0);

        // ── Night pocket breakup ──────────────────────────────
        // Two overlapping turbulence fields punch holes and fray edges.
        // A separate wisp pass thins the cloud silhouette into fibres.
        // Both are gated by night_pocket_strength so day clouds are pristine.
        let pocket_warp = sky_uv * (clouds_scale * 0.95)
            + vec2<f32>(night_pocket_a, night_pocket_b) * 0.55;
        let pocket_noise = perlin_fbm(
            pocket_warp + vec2<f32>(cloud_clock * 0.8), 3);
        let pocket_holes = smoothstep(0.38, 0.72, pocket_noise);

        let wisp_night = perlin_fbm(
            sky_uv * (clouds_scale * 1.80)
            + wind_dir * cloud_clock * 1.75
            + vec2<f32>(-11.0, 47.0), 4
        );
        let wisp_fray = smoothstep(0.42, 0.88, wisp_night);

        let night_breakup = mix(
            clamp(pocket_holes * 0.60 + wisp_fray * 0.40, 0.0, 1.0),
            0.0,
            1.0 - night_pocket_strength
        );
        clouds_amount_real *= mix(
            1.0,
            clamp(1.0 - night_breakup * 0.78, 0.15, 1.0),
            night_pocket_strength
        );
        // Feather edges more aggressively at night → gossamer wisp look.
        let night_edge_feather = mix(0.0, 0.30, night_pocket_strength)
            * (1.0 - smoothstep(0.18, 0.55, clouds_amount_real));
        clouds_amount_real = clamp(
            clouds_amount_real - night_edge_feather, 0.0, 1.0);

        // ── Base cloud colour ─────────────────────────────────
        let clouds_edge_color_real   = vec3<f32>(0.74, 0.78, 0.86);
        let clouds_top_color_real    = vec3<f32>(0.90, 0.92, 0.95);
        let clouds_middle_color_real = vec3<f32>(0.82, 0.86, 0.92);
        let clouds_bottom_color_real = vec3<f32>(0.70, 0.74, 0.82);

        var clouds_color_real = mix(vec3<f32>(0.0), clouds_top_color_real,    noise_top_s);
        clouds_color_real     = mix(clouds_color_real, clouds_middle_color_real, noise_middle);
        clouds_color_real     = mix(clouds_color_real, clouds_bottom_color_real, noise_bottom);
        clouds_color_real    += vec3<f32>(0.04, 0.05, 0.07) * low_layer;
        clouds_color_real    += vec3<f32>(0.03, 0.04, 0.05) * high_layer;
        clouds_color_real     = mix(clouds_edge_color_real, clouds_color_real, noise_top_s);

        let micro_contrast = smoothstep(0.22, 0.82, detail_micro);
        clouds_color_real *= mix(0.90, 1.04, micro_contrast);

        // ── Self-shadow ───────────────────────────────────────
        let self_shadow = smoothstep(0.20, 0.92, realistic_erosion)
                        * (0.22 + (1.0 - realistic_puffs) * 0.40);
        clouds_color_real *= vec3<f32>(
            1.0 - self_shadow * 0.28,
            1.0 - self_shadow * 0.24,
            1.0 - self_shadow * 0.18
        );

        // ── 3D volume shading ─────────────────────────────────
        let view_up_dot = clamp(ray.y / max(ray.y + 0.12, 1.0e-4), 0.0, 1.0);
        let cloud_layer_height = clamp(
            noise_top_s * 0.55 + noise_middle * 0.30 + noise_bottom * 0.15,
            0.0, 1.0
        );
        let top_face_light = clamp(view_up_dot * cloud_layer_height, 0.0, 1.0);
        let top_roundness  = pow(top_face_light, 1.6);
        let top_col_boost  = mix(vec3<f32>(1.0), vec3<f32>(1.22, 1.18, 1.14), top_roundness);
        let bottom_shadow_view = clamp(
            (1.0 - view_up_dot) * (1.0 - cloud_layer_height), 0.0, 1.0);
        let bottom_shadow_str = pow(bottom_shadow_view, 1.4)
                              * smoothstep(0.30, 0.85, clouds_amount_real);
        let bottom_col_damp   = mix(
            vec3<f32>(1.0), vec3<f32>(0.52, 0.54, 0.60),
            bottom_shadow_str * 0.70
        );
        let sun_top_bonus = clamp(sun_dir.y * 0.6 + 0.4, 0.0, 1.0) * day_t;
        let top_sun_tint  = mix(
            vec3<f32>(1.0), vec3<f32>(1.10, 1.06, 0.96),
            top_roundness * sun_top_bonus * 0.45
        );
        clouds_color_real = clouds_color_real
                          * top_col_boost * bottom_col_damp * top_sun_tint;

        // ── Backlit shadow + silver lining ────────────────────
        let sun_angle_to_ray = clamp(dot(ray, sun_dir), 0.0, 1.0);
        let corona_phase     = pow12(sun_angle_to_ray);
        let cloud_edge_rim = smoothstep(0.04, 0.38, clouds_amount_real)
                           * (1.0 - smoothstep(0.38, 0.62, clouds_amount_real));
        let corona_intensity = corona_phase * cloud_edge_rim * sun_behind;
        let corona_col_day = vec3<f32>(0.85, 0.72, 0.52);
        let corona_col_eve = vec3<f32>(1.30, 0.72, 0.38);
        let corona_col     = mix(corona_col_day, corona_col_eve, evening_t);
        let corona_flicker  = 0.94 + 0.06 * fbm2(
            top_uv * 1.8 + vec2<f32>(cloud_time_seconds * 0.02, 0.0));
        clouds_color_real += corona_col * corona_intensity * corona_flicker * 0.95;

        let solid_interior = smoothstep(0.42, 0.82, clouds_amount_real);
        let backlit_shadow = sun_behind * corona_phase * solid_interior;
        clouds_color_real *= mix(1.0, 0.38, backlit_shadow * 0.72);

        let thin_cloud   = 1.0 - smoothstep(0.18, 0.55, clouds_amount_real);
        let transmission = corona_phase * sun_behind * thin_cloud;
        let transmit_col = mix(
            vec3<f32>(1.10, 1.02, 0.82),
            vec3<f32>(1.15, 0.72, 0.48),
            evening_t
        );
        clouds_color_real = mix(
            clouds_color_real,
            clouds_color_real * transmit_col,
            transmission * 0.40
        );

        // ── Wispy fibre cool tint ─────────────────────────────
        let fiber     = smoothstep(0.24, 0.82, noise_top_s * (1.0 - noise_middle));
        let wisp_cool = vec3<f32>(0.88, 0.95, 1.05);
        clouds_color_real = mix(
            clouds_color_real,
            clouds_color_real * wisp_cool,
            fiber * upper_veil * 0.20
        );

        // ── Silver lining ─────────────────────────────────────
        let edge_hint = smoothstep(0.18, 0.78, clamp(
            (noise_top_s - noise_middle * 0.45) + (1.0 - noise_bottom) * 0.12,
            0.0, 1.0
        ));
        let silver_lining = pow(
            clamp(1.0 - abs(dot(ray, sun_dir)), 0.0, 1.0), 7.0) * edge_hint;
        clouds_color_real += vec3<f32>(0.94, 0.92, 0.88) * silver_lining * 0.08;

        // ── Sun punch (daytime) ───────────────────────────────
        if (day_t > 0.5) {
            let sun_col_clouds_real = clamp(
                mix(vec3<f32>(1.00, 0.95, 0.84), vec3<f32>(1.00, 0.62, 0.44), evening_t),
                vec3<f32>(0.0), vec3<f32>(1.0)
            );
            let sun_dist_c = distance(ray, sun_dir);
            let sun_punch  = pow5(1.0 - clamp(sun_dist_c, 0.0, 1.0));
            clouds_color_real = mix(clouds_color_real, sun_col_clouds_real,
                sun_punch * 0.42);
        }

        // ── Sky tint ──────────────────────────────────────────
        let day_src   = gradient3(sky.day_colors,     0.34);
        let eve_src   = gradient3(sky.evening_colors, 0.43);
        let night_src = gradient3(sky.night_colors,   0.36);

        let day_tint   = stylize_cloud_tint(
            day_src   * vec3<f32>(1.10, 1.04, 0.98), 1.15, 0.92, 0.03);
        let eve_tint   = stylize_cloud_tint(
            eve_src   * vec3<f32>(1.20, 0.94, 1.00), 1.25, 0.90, 0.03);
        let night_tint = stylize_cloud_tint(
            night_src * vec3<f32>(0.98, 0.93, 1.32), 1.22, 0.94, 0.02);

        let sky_tint = clamp(
            day_tint * day_t + eve_tint * evening_t + night_tint * night_t,
            vec3<f32>(0.0), vec3<f32>(2.0)
        );
        clouds_color_real = clouds_color_real
            * mix(vec3<f32>(1.0), sky_tint, 0.10);

        // ── Coloured edge band ────────────────────────────────
        let edge_band = clamp(
            (noise_top_s - noise_middle * 0.45) + (1.0 - noise_bottom) * 0.12,
            0.0, 1.0
        );
        let edge_glow      = smoothstep(0.12, 0.84, edge_band);
        let edge_tint_base = clamp(
            mix(day_tint, eve_tint, evening_t) * vec3<f32>(1.10, 1.00, 1.20),
            vec3<f32>(0.0), vec3<f32>(2.0)
        );

        let edge_phase = fract(
            noise_top_raw * 1.25 + noise_middle_raw * 0.95
            + cloud_time_seconds * 0.035
        );
        let rim_yellow = vec3<f32>(1.20, 1.03, 0.68);
        let rim_pink   = vec3<f32>(1.20, 0.74, 1.00);
        let rim_blue   = vec3<f32>(0.72, 0.92, 1.25);
        let rim_warm   = mix(rim_yellow, rim_pink, smoothstep(0.18, 0.56, edge_phase));
        let rim_mix    = mix(rim_warm,   rim_blue,  smoothstep(0.52, 0.90, edge_phase));
        let edge_tint  = clamp(
            mix(edge_tint_base, rim_mix, 0.55),
            vec3<f32>(0.0), vec3<f32>(2.2)
        );

        let micro_edge = smoothstep(0.30, 0.85, detail_micro)
                       * (1.0 - smoothstep(0.85, 1.0, noise_bottom));
        clouds_color_real += edge_tint * edge_glow * (0.24 + micro_edge * 0.12);

        clouds_color_real = min(clouds_color_real, vec3<f32>(1.15, 1.12, 1.10));

        // ── Night fade / storm ────────────────────────────────
        clouds_color_real = mix(clouds_color_real, color,
            clamp(night_t * 0.75, 0.0, 0.92));
        clouds_color_real = mix(clouds_color_real, vec3<f32>(0.0),
            clouds_weight * 0.9);

        // ── Halo ──────────────────────────────────────────────
        let halo = smoothstep(0.18, 0.62, clouds_amount_real)
                 * (1.0 - smoothstep(0.62, 0.96, clouds_amount_real));
        let halo_tint = mix(
            vec3<f32>(1.00, 0.95, 0.86),
            vec3<f32>(1.00, 0.65, 0.58),
            evening_t
        );
        color += halo_tint * halo * 0.07;

        // ── Composite ─────────────────────────────────────────
        color      = mix(color, clouds_color_real, clouds_amount_real);
        cloud_mask = clouds_amount_real;
    }

    // ── Atmospheric scatter: sky glow in gaps (post-cloud pass) ──
    {
        let scatter_angle = clamp(dot(ray, sun_dir), 0.0, 1.0);
        let scatter_wide  = exp(-max(1.0 - scatter_angle, 0.0) * 2.8) * 0.12;
        let scatter_tight = pow14(scatter_angle) * 0.22; 
        let scatter_total = (scatter_wide + scatter_tight)
                        * sun_cloud_cover * day_t * sun_visibility;
        let scatter_col = mix(
            vec3<f32>(0.95, 0.90, 0.72),
            vec3<f32>(1.00, 0.62, 0.38),
            evening_t
        );
        color += scatter_col * scatter_total;
    }

    return vec4<f32>(max(color, vec3<f32>(0.0)), 1.0);
}
