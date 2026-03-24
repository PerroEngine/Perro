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

fn hash13(p: vec3<f32>) -> f32 {
    let q = fract(p * 0.1031);
    let r = q + dot(q, q.yzx + 33.33);
    return fract((r.x + r.y) * r.z);
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let n000 = hash13(i + vec3<f32>(0.0, 0.0, 0.0));
    let n100 = hash13(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash13(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash13(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash13(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash13(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash13(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash13(i + vec3<f32>(1.0, 1.0, 1.0));

    let nx00 = mix(n000, n100, u.x);
    let nx10 = mix(n010, n110, u.x);
    let nx01 = mix(n001, n101, u.x);
    let nx11 = mix(n011, n111, u.x);
    let nxy0 = mix(nx00, nx10, u.y);
    let nxy1 = mix(nx01, nx11, u.y);
    return mix(nxy0, nxy1, u.z);
}

fn fbm3(p: vec3<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0u; i < 5u; i = i + 1u) {
        v = v + a * noise3(q);
        q = q * 2.03 + vec3<f32>(37.1, 13.7, 29.3);
        a = a * 0.5;
    }
    return v;
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

fn cloud_field(p: vec3<f32>, cloud_size: f32, cloud_density: f32, cloud_variance: f32) -> f32 {
    let freq = mix(2.25, 0.7, cloud_size);
    let q = p * freq;

    let base = fbm3(q + vec3<f32>(11.0, 3.0, 7.0));
    let mid = fbm3(q * 2.05 + vec3<f32>(-17.0, 31.0, 13.0));
    let fine = fbm3(q * 4.1 + vec3<f32>(53.0, -23.0, 29.0));

    let shape = mix(base, mid, 0.42 + cloud_variance * 0.34)
        + (fine - 0.5) * (0.16 + cloud_variance * 0.26);

    let threshold = mix(0.84, 0.30, cloud_density);
    return smoothstep(threshold, threshold + 0.24, shape);
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 3.0,  1.0)
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

    let horizon_t = smoothstep(-0.25, 0.45, ray.y);
    let day_col = gradient3(sky.day_colors, horizon_t);
    let night_col = gradient3(sky.night_colors, horizon_t);
    let day_t = sky.params2.z;
    var color = mix(night_col, day_col, day_t);

    let tod = sky.params0.w;
    let sun_dir = sun_dir_from_time(tod, sky.params1.w);
    let moon_dir = -sun_dir;

    let sun_size = max(0.08, sky.params2.x);
    let moon_size = max(0.08, sky.params2.y);
    let sun_radius = mix(0.007, 0.06, clamp(sun_size * 0.2, 0.0, 1.0));
    let moon_radius = mix(0.005, 0.05, clamp(moon_size * 0.2, 0.0, 1.0));

    let sun_dot = dot(ray, sun_dir);
    let moon_dot = dot(ray, moon_dir);
    let sun_disk = smoothstep(cos(sun_radius), cos(sun_radius * 0.35), sun_dot);
    let moon_disk = smoothstep(cos(moon_radius), cos(moon_radius * 0.35), moon_dot);

    let sun_col = vec3<f32>(1.0, 0.93, 0.78);
    let moon_col = vec3<f32>(0.76, 0.82, 0.96);
    color = color + sun_col * (sun_disk * pow(day_t, 1.2));
    color = color + moon_col * (moon_disk * pow(1.0 - day_t, 1.4));

    let cloud_size = clamp(sky.params0.x, 0.0, 1.0);
    let cloud_density = clamp(sky.params0.y, 0.0, 1.0);
    let cloud_variance = clamp(sky.params0.z, 0.0, 1.0);
    let wind = vec3<f32>(sky.wind.x, 0.0, sky.wind.y);

    var cloud_transmittance = 1.0;
    var cloud_accum = vec3<f32>(0.0);
    var cloud_cover = 0.0;

    if (ray.y > -0.08) {
        let cloud_time = sky.params2.w * 24.0;
        let drift = wind * cloud_time;
        let horizon_mask = smoothstep(-0.02, 0.32, ray.y);

        for (var i = 0u; i < 7u; i = i + 1u) {
            let fi = f32(i);
            let step_t = 0.34 + fi * 0.24;
            let sample_dir = normalize(ray + vec3<f32>(0.0, step_t * 0.10, 0.0));
            let sample_pos = sample_dir * (1.3 + fi * 0.26) + drift;

            let d = cloud_field(sample_pos, cloud_size, cloud_density, cloud_variance) * horizon_mask;
            if (d <= 0.0001) {
                continue;
            }

            let sun_probe = cloud_field(sample_pos + sun_dir * 0.28, cloud_size, cloud_density, cloud_variance) * horizon_mask;
            let moon_probe = cloud_field(sample_pos + moon_dir * 0.22, cloud_size, cloud_density, cloud_variance) * horizon_mask;
            let sun_rim = clamp(d - sun_probe + 0.06, 0.0, 1.0);
            let moon_rim = clamp(d - moon_probe + 0.08, 0.0, 1.0);

            let deep_col = mix(vec3<f32>(0.09, 0.11, 0.16), vec3<f32>(0.68, 0.74, 0.84), day_t);
            let light_col = mix(vec3<f32>(0.20, 0.24, 0.33), vec3<f32>(0.95, 0.97, 1.0), day_t);
            var cloud_col = mix(deep_col, light_col, clamp(0.36 + sun_rim * 0.78 + moon_rim * 0.35, 0.0, 1.0));
            cloud_col = cloud_col + vec3<f32>(1.0, 0.86, 0.62) * sun_rim * pow(day_t, 1.15) * 0.34;
            cloud_col = cloud_col + vec3<f32>(0.70, 0.80, 1.0) * moon_rim * pow(1.0 - day_t, 1.25) * 0.22;

            let slice_alpha = clamp(d * (0.20 + fi * 0.045), 0.0, 0.98) * cloud_transmittance;
            cloud_accum = cloud_accum + cloud_col * slice_alpha;
            cloud_transmittance = cloud_transmittance * (1.0 - slice_alpha);
            cloud_cover = max(cloud_cover, 1.0 - cloud_transmittance);

            if (cloud_transmittance < 0.02) {
                break;
            }
        }

        color = color * cloud_transmittance + cloud_accum;
    }

    let star_size = clamp(sky.params1.x, 0.05, 4.0);
    let star_scatter = clamp(sky.params1.y, 0.0, 1.0);
    let star_gleam = clamp(sky.params1.z, 0.0, 2.0);
    let star_scale = 360.0 * mix(0.55, 2.25, star_scatter);
    let star_seed = hash13(floor(ray * star_scale));
    let cloud_time = sky.params2.w * 6.28318530718;
    let star_twinkle = 0.7 + 0.3 * sin(cloud_time * (2.0 + star_seed * 8.0) + star_seed * 12.0);
    let star_threshold = 1.0 - (0.0017 * star_size);
    let star = select(0.0, 1.0, star_seed > star_threshold);
    let stars_alpha = star * pow(1.0 - day_t, 2.6) * (1.0 - cloud_cover * 0.92) * star_twinkle;
    let star_col = mix(vec3<f32>(0.72, 0.78, 1.0), vec3<f32>(1.0, 0.97, 0.89), star_seed);
    color = color + star_col * stars_alpha * (0.7 + star_gleam * 0.9);

    return vec4<f32>(max(color, vec3<f32>(0.0)), 1.0);
}
