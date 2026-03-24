struct SkyUniform {
    inv_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    day_colors: array<vec4<f32>, 3>,
    night_colors: array<vec4<f32>, 3>,
    params0: vec4<f32>, // cloud_size, cloud_density, cloud_variance, time_of_day
    params1: vec4<f32>, // star_size, star_scatter, star_gleam, sky_angle
    params2: vec4<f32>, // sun_size, moon_size, day_weight, time_phase
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
    let night_col = gradient3(sky.night_colors, horizon_t);
    let day_t = sky.params2.z;
    var color = mix(night_col, day_col, day_t);

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

    // Flat stylized cloud field in projected sky space.
    let cloud_time = sky.params2.w * 12.0;
    let drift = wind * cloud_time * 0.22;
    let proj = ray.xz / max(ray.y + 0.16, 0.08);
    let scale = mix(1.85, 0.90, cloud_size);
    let n_base = fbm2(proj * scale + drift);
    let n_detail = noise2(proj * (scale * 3.4) + drift * 1.8);
    let n = n_base * (0.86 - cloud_variance * 0.22) + n_detail * (0.14 + cloud_variance * 0.22);

    let threshold = mix(0.72, 0.38, cloud_density);
    let softness = mix(0.10, 0.22, cloud_variance);
    let horizon_mask = smoothstep(-0.02, 0.18, ray.y);
    let cloud_mask = smoothstep(threshold, threshold + softness, n) * horizon_mask;

    // Toon-style edge band around cloud silhouettes.
    let edge_offset = mix(0.034, 0.018, cloud_size);
    let cloud_inner = smoothstep(threshold + edge_offset, threshold + edge_offset + softness, n) * horizon_mask;
    let cloud_outline = clamp(cloud_mask - cloud_inner, 0.0, 1.0);

    let sun_lobe = pow(clamp(dot(normalize(vec3<f32>(ray.x, ray.y * 0.45 + 0.15, ray.z)), sun_dir) * 0.5 + 0.5, 0.0, 1.0), 4.0);
    let base_cloud_col = mix(vec3<f32>(0.83, 0.87, 0.94), vec3<f32>(0.98, 0.99, 1.0), day_t);
    let lit_cloud_col = base_cloud_col + vec3<f32>(0.08, 0.05, 0.02) * sun_lobe * pow(day_t, 1.2);
    let outline_col = mix(vec3<f32>(0.26, 0.30, 0.38), vec3<f32>(0.56, 0.61, 0.72), day_t);

    color = mix(color, lit_cloud_col, cloud_mask);
    color = mix(color, outline_col, cloud_outline * 0.88);

    // Cheap stars at night only.
    let star_size = clamp(sky.params1.x, 0.05, 4.0);
    let star_scatter = clamp(sky.params1.y, 0.0, 1.0);
    let star_gleam = clamp(sky.params1.z, 0.0, 2.0);
    let star_scale = 220.0 * mix(0.55, 1.85, star_scatter);
    let star_seed = hash12(floor(ray.xz * star_scale));
    let star_threshold = 1.0 - (0.0019 * star_size);
    let star = select(0.0, 1.0, star_seed > star_threshold);
    let stars_alpha = star * pow(1.0 - day_t, 2.7) * (1.0 - cloud_mask * 0.9);
    let twinkle = 0.8 + 0.2 * sin((sky.params2.w * 6.28318530718) * (2.0 + star_seed * 8.0));
    let star_col = mix(vec3<f32>(0.72, 0.78, 1.0), vec3<f32>(1.0, 0.97, 0.89), star_seed);
    color = color + star_col * stars_alpha * twinkle * (0.7 + star_gleam * 0.9);

    let dither = (hash12(in.uv * vec2<f32>(1920.0, 1080.0) + vec2<f32>(sky.params2.w * 317.0, sky.params2.w * 911.0)) - 0.5) * (1.0 / 255.0);
    color = color + vec3<f32>(dither);

    return vec4<f32>(max(color, vec3<f32>(0.0)), 1.0);
}
