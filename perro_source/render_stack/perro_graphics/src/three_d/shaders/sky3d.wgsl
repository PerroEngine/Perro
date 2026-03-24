struct SkyUniform {
    inv_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    day_colors: array<vec4<f32>, 3>,
    evening_colors: array<vec4<f32>, 3>,
    night_colors: array<vec4<f32>, 3>,
    params0: vec4<f32>, // cloud_size, cloud_density, cloud_variance, time_of_day
    params1: vec4<f32>, // star_size, star_scatter, star_gleam, sky_angle
    params2: vec4<f32>, // sun_size, moon_size, day_weight, cloud_time_seconds
    wind: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> sky: SkyUniform;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

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
    // Keep this intentionally tiny for performance.
    let a = noise2(p);
    let b = noise2(p * 2.02 + vec2<f32>(17.0, 41.0));
    let c = noise2(p * 4.05 + vec2<f32>(53.0, 7.0));
    return a * 0.60 + b * 0.28 + c * 0.12;
}

fn gradient3(colors: array<vec4<f32>, 3>, t_in: f32) -> vec3<f32> {
    let t = clamp(t_in, 0.0, 1.0);
    if (t < 0.5) {
        return mix(colors[0].xyz, colors[1].xyz, t * 2.0);
    }
    return mix(colors[1].xyz, colors[2].xyz, (t - 0.5) * 2.0);
}

fn sun_dir_from_time(tod: f32, sky_angle: f32) -> vec3<f32> {
    let theta = (tod * 6.28318530718) + sky_angle;
    return normalize(vec3<f32>(cos(theta), sin(theta), -0.25));
}

fn evening_weight_from_time(time_of_day: f32) -> f32 {
    let t = time_of_day - floor(time_of_day);
    let angle = t * 6.28318530718;
    let dusk = max(0.0, sin(angle));
    let dawn = max(0.0, -sin(angle));
    return clamp(max(pow(dusk, 2.6), pow(dawn, 2.3)) * 0.38, 0.0, 1.0);
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

const CLOUD_BASE_HEIGHT: f32 = 1000.0;
const CLOUD_BASE_THICKNESS: f32 = 85.0;

fn cloud_density_sample(
    pos: vec3<f32>,
    y_norm: f32,
    cloud_size: f32,
    cloud_density: f32,
    cloud_variance: f32,
    wind_world: vec2<f32>
) -> f32 {
    let scale = mix(0.0048, 0.0022, cloud_size);
    let p = (pos.xz + wind_world) * scale;
    let warp = vec2<f32>(
        noise2(p * 0.65 + vec2<f32>(17.0, -9.0)),
        noise2(p * 0.65 + vec2<f32>(-31.0, 23.0))
    ) - vec2<f32>(0.5, 0.5);
    let q = p + warp * (0.40 + cloud_variance * 0.55);

    // Billowy structure: low-freq body + abs/folded noise for cauliflower-like masses.
    let body = fbm2(q * 0.95);
    let billow = 1.0 - abs(fbm2(q * 1.85 + vec2<f32>(37.0, 11.0)) * 2.0 - 1.0);
    let detail = fbm2(q * 2.70 + vec2<f32>(-19.0, 53.0));
    let shape = body * 0.60 + billow * 0.34 + detail * 0.06;

    let bottom = smoothstep(0.02, 0.28, y_norm);
    let top = 1.0 - smoothstep(0.62, 1.0, y_norm);
    let vertical_profile = bottom * top;

    let threshold = mix(0.76, 0.56, cloud_density);
    let softness = mix(0.24, 0.15, cloud_variance);
    let core = smoothstep(threshold, threshold + softness, shape);
    let rounded = smoothstep(0.0, 1.0, pow(core, 0.95));
    // Suppress ultra-thin wisps so clouds grow from dense centers outward.
    let centered = smoothstep(0.18, 1.0, rounded);
    return pow(centered, 1.18) * vertical_profile;
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(3.0, 1.0)
    );
    var out: VsOut;
    out.pos = vec4<f32>(pos[vi], 0.0, 1.0);
    out.uv = out.pos.xy * 0.5 + vec2<f32>(0.5);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let ndc = vec4<f32>(in.uv * 2.0 - 1.0, 1.0, 1.0);
    let world_h = sky.inv_view_proj * ndc;
    let world = world_h.xyz / max(world_h.w, 1.0e-5);
    let ray = normalize(world - sky.camera_pos.xyz);

    let horizon_t = smoothstep(-0.32, 0.52, ray.y);
    let day_col = gradient3(sky.day_colors, horizon_t);
    let evening_col = gradient3(sky.evening_colors, horizon_t);
    let night_col = gradient3(sky.night_colors, horizon_t);
    let day_t = sky.params2.z;
    let evening_t = evening_weight_from_time(sky.params0.w);
    var color = mix(mix(night_col, day_col, day_t), evening_col, evening_t);

    let tod = sky.params0.w;
    let sun_dir = sun_dir_from_time(tod, sky.params1.w);
    let moon_dir = -sun_dir;

    let sun_size = max(0.08, sky.params2.x);
    let moon_size = max(0.08, sky.params2.y);
    let sun_radius = mix(0.006, 0.045, clamp(sun_size * 0.2, 0.0, 1.0));
    let moon_radius = mix(0.005, 0.040, clamp(moon_size * 0.2, 0.0, 1.0));

    let sun_dot = dot(ray, sun_dir);
    let moon_dot = dot(ray, moon_dir);
    let sun_disk = smoothstep(cos(sun_radius), cos(sun_radius * 0.35), sun_dot);
    let moon_disk = smoothstep(cos(moon_radius), cos(moon_radius * 0.35), moon_dot);

    let sun_col = vec3<f32>(1.0, 0.95, 0.82);
    let moon_col = vec3<f32>(0.75, 0.82, 0.96);
    color = color + sun_col * (sun_disk * pow(day_t, 1.15));
    color = color + moon_col * (moon_disk * pow(1.0 - day_t, 1.35));

    let cloud_size = clamp(sky.params0.x, 0.0, 1.0);
    let cloud_density = clamp(sky.params0.y, 0.0, 1.0);
    let cloud_variance = clamp(sky.params0.z, 0.0, 1.0);
    let wind = vec2<f32>(sky.wind.x, sky.wind.y);

    // Low world-space cloud slab for controllable cloud height and better shape.
    // Independent cloud clock (seconds). Not tied to time_of_day.
    let cloud_time_seconds = sky.params2.w;
    // wind_vector is interpreted as world-space units per second on XZ cloud plane.
    let wind_world = wind * cloud_time_seconds;
    // Fixed world cloud altitude.
    let cloud_base = CLOUD_BASE_HEIGHT;
    let cloud_thickness = CLOUD_BASE_THICKNESS * mix(0.90, 1.30, cloud_density);
    let horizon_mask = smoothstep(-0.06, 0.20, ray.y);
    var cloud_mask_refined = 0.0;
    var cloud_outline = 0.0;

    if (abs(ray.y) > 1.0e-4) {
        let y0 = cloud_base;
        let y1 = cloud_base + cloud_thickness;
        let t0 = (y0 - sky.camera_pos.y) / ray.y;
        let t1 = (y1 - sky.camera_pos.y) / ray.y;
        let t_enter = min(t0, t1);
        let t_exit = max(t0, t1);

        if (t_exit > 0.0) {
            let seg_start = max(t_enter, 0.0);
            let seg_end = t_exit;
            let seg_len = max(seg_end - seg_start, 1.0e-4);
            let t_base = seg_start + seg_len * 0.10;
            let t_mid = seg_start + seg_len * 0.50;
            let t_top = seg_start + seg_len * 0.90;

            let p0 = sky.camera_pos.xyz + ray * t_base;
            let p1 = sky.camera_pos.xyz + ray * t_mid;
            let p2 = sky.camera_pos.xyz + ray * t_top;

            let y_norm0 = clamp((p0.y - y0) / max(cloud_thickness, 1.0e-4), 0.0, 1.0);
            let y_norm1 = clamp((p1.y - y0) / max(cloud_thickness, 1.0e-4), 0.0, 1.0);
            let y_norm2 = clamp((p2.y - y0) / max(cloud_thickness, 1.0e-4), 0.0, 1.0);
            let d0 = cloud_density_sample(p0, y_norm0, cloud_size, cloud_density, cloud_variance, wind_world);
            let d1 = cloud_density_sample(p1, y_norm1, cloud_size, cloud_density, cloud_variance, wind_world);
            let d2 = cloud_density_sample(p2, y_norm2, cloud_size, cloud_density, cloud_variance, wind_world);
            let density_mix = clamp(d0 * 0.55 + d1 * 1.00 + d2 * 0.70, 0.0, 1.0);

            let distance_fade = 1.0 - smoothstep(12000.0, 42000.0, seg_start);
            let core_mask = smoothstep(0.24, 0.62, density_mix);
            let cloud_mask = clamp(density_mix * 0.92, 0.0, 1.0) * horizon_mask * distance_fade * core_mask;
            let cloud_inner = clamp((density_mix - 0.12) * 1.20, 0.0, 1.0) * horizon_mask * core_mask;
            cloud_outline = clamp(cloud_mask - cloud_inner, 0.0, 1.0);

            // Cheap 3D-looking accumulation through the slab.
            var trans = 1.0;
            var accum = vec3<f32>(0.0);
            let sun2 = normalize(sun_dir.xz + vec2<f32>(1.0e-4, 1.0e-4));
            let march_steps = 12.0;
            let march_jitter = hash12(in.uv * vec2<f32>(2048.0, 1024.0) + vec2<f32>(cloud_time_seconds * 17.3, cloud_time_seconds * 9.1));
            for (var i = 0u; i < 12u; i = i + 1u) {
                let fi = f32(i);
                let t = seg_start + seg_len * ((fi + march_jitter) / march_steps);
                let ps = sky.camera_pos.xyz + ray * t;
                let yn = clamp((ps.y - y0) / max(cloud_thickness, 1.0e-4), 0.0, 1.0);
                let d = cloud_density_sample(ps, yn, cloud_size, cloud_density, cloud_variance, wind_world) * horizon_mask * distance_fade;
                if (d < 0.035) {
                    continue;
                }

                let lp = ps + vec3<f32>(sun2.x, 0.0, sun2.y) * mix(22.0, 10.0, cloud_size) + vec3<f32>(0.0, 2.0, 0.0);
                let lpn = clamp((lp.y - y0) / max(cloud_thickness, 1.0e-4), 0.0, 1.0);
                let d_light = cloud_density_sample(lp, lpn, cloud_size, cloud_density, cloud_variance, wind_world);
                let light_trans = clamp(1.0 - d_light * 1.55, 0.16, 1.0);

                let top = smoothstep(0.45, 1.0, yn);
                let bottom = 1.0 - smoothstep(0.08, 0.60, yn);
                let base_col = mix(vec3<f32>(0.80, 0.84, 0.92), vec3<f32>(0.98, 0.99, 1.0), day_t);
                // Undersides darker/cooler, tops brighter/warmer.
                let underside_shadow = 1.0 - bottom * (0.32 + (1.0 - light_trans) * 0.24);
                var cloud_col = base_col * (0.58 + light_trans * 0.56) * underside_shadow;
                let night_t = clamp(1.0 - day_t, 0.0, 1.0);
                let day_src = gradient3(sky.day_colors, 0.34);
                let eve_src = gradient3(sky.evening_colors, 0.40);
                let night_src = gradient3(sky.night_colors, 0.36);

                let day_tint = stylize_cloud_tint(day_src * vec3<f32>(1.10, 1.00, 0.92), 1.18, 0.92, 0.02);
                let eve_tint = stylize_cloud_tint(eve_src * vec3<f32>(1.18, 0.92, 0.98), 1.28, 0.90, 0.015);
                let night_tint = stylize_cloud_tint(night_src * vec3<f32>(0.96, 0.90, 1.30), 1.24, 0.95, 0.00);
                let sky_tint = clamp(day_tint * day_t + eve_tint * evening_t + night_tint * night_t, vec3<f32>(0.0), vec3<f32>(2.0));
                cloud_col = cloud_col * mix(vec3<f32>(1.0), sky_tint, 0.22);

                let underside_tint = clamp(
                    day_tint * (0.58 + 0.18 * (1.0 - light_trans)) * day_t +
                    eve_tint * (0.76 + 0.22 * (1.0 - light_trans)) * evening_t +
                    night_tint * (0.80 + 0.26 * (1.0 - light_trans)) * night_t,
                    vec3<f32>(0.0),
                    vec3<f32>(2.0)
                );
                let dense_core = smoothstep(0.22, 0.62, d);
                cloud_col = cloud_col + underside_tint * bottom * dense_core * (0.18 + (1.0 - light_trans) * 0.22);
                cloud_col = cloud_col + vec3<f32>(0.10, 0.09, 0.06) * top * light_trans * (0.35 + day_t * 0.45);

                // Built-in pastel rim/bloom-like edge tint.
                let edge = clamp(d * (1.0 - d) * 3.2, 0.0, 1.0);
                let sun_view = clamp(dot(normalize(vec3<f32>(ray.x, ray.y * 0.45 + 0.15, ray.z)), sun_dir) * 0.5 + 0.5, 0.0, 1.0);
                let rim_day = stylize_cloud_tint(day_src * vec3<f32>(1.20, 1.05, 0.86), 1.22, 0.90, 0.02);
                let rim_eve = stylize_cloud_tint(eve_src * vec3<f32>(1.22, 0.86, 1.00), 1.30, 0.88, 0.02);
                let rim_night = stylize_cloud_tint(night_src * vec3<f32>(0.92, 0.86, 1.45), 1.26, 0.92, 0.01);
                let rim_tint = clamp(
                    rim_day * day_t * pow(sun_view, 1.55) +
                    rim_eve * evening_t * (0.55 + 0.45 * pow(1.0 - sun_view, 1.35)) +
                    rim_night * night_t * (0.50 + 0.50 * pow(1.0 - light_trans, 1.2)),
                    vec3<f32>(0.0),
                    vec3<f32>(2.0)
                );
                cloud_col = cloud_col + rim_tint * edge * core_mask * (0.18 + day_t * 0.48 + evening_t * 0.42 + night_t * 0.34);

                let a = clamp((d - 0.06) * 0.26, 0.0, 0.84) * trans;
                accum = accum + cloud_col * a;
                trans = trans * (1.0 - a);
                if (trans < 0.03) {
                    break;
                }
            }

            cloud_mask_refined = clamp(1.0 - trans, 0.0, 1.0);
            // Additional fade near horizon to avoid visible slab edges.
            let horizon_fade = smoothstep(-0.02, 0.12, ray.y) * distance_fade;
            let blend = cloud_mask_refined * horizon_fade;
            color = color * (1.0 - blend) + accum * horizon_fade;
        }
    }

    // Cheap stars at night only.
    let star_size = clamp(sky.params1.x, 0.05, 4.0);
    let star_scatter = clamp(sky.params1.y, 0.0, 1.0);
    let star_gleam = clamp(sky.params1.z, 0.0, 2.0);
    let star_scale = 220.0 * mix(0.55, 1.85, star_scatter);
    let star_seed = hash12(floor(ray.xz * star_scale));
    let star_threshold = 1.0 - (0.0019 * star_size);
    let star = select(0.0, 1.0, star_seed > star_threshold);
    let stars_alpha = star * pow(1.0 - day_t, 2.7) * (1.0 - cloud_mask_refined * 0.9);
    let twinkle = 0.8 + 0.2 * sin((sky.params0.w * 6.28318530718) * (2.0 + star_seed * 8.0));
    let star_col = mix(vec3<f32>(0.72, 0.78, 1.0), vec3<f32>(1.0, 0.97, 0.89), star_seed);
    color = color + star_col * stars_alpha * twinkle * (0.7 + star_gleam * 0.9);

    let dither = (hash12(in.uv * vec2<f32>(1920.0, 1080.0) + vec2<f32>(sky.params0.w * 317.0, sky.params0.w * 911.0)) - 0.5) * (1.0 / 255.0);
    color = color + vec3<f32>(dither);

    return vec4<f32>(max(color, vec3<f32>(0.0)), 1.0);
}
