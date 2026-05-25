struct SkyUniform {
    inv_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    day_colors: array<vec4<f32>, 3>,
    evening_colors: array<vec4<f32>, 3>,
    night_colors: array<vec4<f32>, 3>,
    horizon_colors: array<vec4<f32>, 3>,
    params0: vec4<f32>, // time_of_day, day_weight, evening_weight, night_weight
    params1: vec4<f32>, // time_seconds, reserved
};

@group(0) @binding(0)
var<uniform> sky: SkyUniform;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct SkyFragment {
    ray: vec3<f32>,
    uv: vec2<f32>,
    time_of_day: f32,
    time_seconds: f32,
    day_weight: f32,
    evening_weight: f32,
    night_weight: f32,
    horizon_weight: f32,
    color: vec4<f32>,
    params0: vec4<f32>,
    params1: vec4<f32>,
    params2: vec4<f32>,
    params3: vec4<f32>,
    params4: vec4<f32>,
    params5: vec4<f32>,
    params6: vec4<f32>,
    params7: vec4<f32>,
    params8: vec4<f32>,
    params9: vec4<f32>,
    params10: vec4<f32>,
    params11: vec4<f32>,
    params12: vec4<f32>,
    params13: vec4<f32>,
    params14: vec4<f32>,
    params15: vec4<f32>,
};

fn custom_param(in: SkyFragment, index: u32) -> vec4<f32> {
    switch index {
        case 0u: { return in.params0; }
        case 1u: { return in.params1; }
        case 2u: { return in.params2; }
        case 3u: { return in.params3; }
        case 4u: { return in.params4; }
        case 5u: { return in.params5; }
        case 6u: { return in.params6; }
        case 7u: { return in.params7; }
        case 8u: { return in.params8; }
        case 9u: { return in.params9; }
        case 10u: { return in.params10; }
        case 11u: { return in.params11; }
        case 12u: { return in.params12; }
        case 13u: { return in.params13; }
        case 14u: { return in.params14; }
        case 15u: { return in.params15; }
        default: { return vec4<f32>(0.0); }
    }
}

fn custom_f_param(in: SkyFragment, index: u32) -> vec4<f32> {
    return custom_param(in, index);
}

fn gradient3(colors: array<vec4<f32>, 3>, t: f32) -> vec3<f32> {
    let u = clamp(t, 0.0, 1.0);
    if (u < 0.5) {
        return mix(colors[0].rgb, colors[1].rgb, u * 2.0);
    }
    return mix(colors[1].rgb, colors[2].rgb, (u - 0.5) * 2.0);
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>( 3.0,  1.0),
        vec2<f32>(-1.0,  1.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(pos[vi], 0.0, 1.0);
    out.uv = out.pos.xy * 0.5 + vec2<f32>(0.5);
    return out;
}

fn sky_base_color(ray: vec3<f32>) -> vec4<f32> {
    let top_t = clamp(pow(max(ray.y, 0.0), 0.45), 0.0, 1.0);
    let lower_t = clamp((-ray.y) / 0.65, 0.0, 1.0);
    let horizon_weight = smoothstep(0.0, 0.18, -ray.y);

    let day_col = gradient3(sky.day_colors, top_t);
    let evening_col = gradient3(sky.evening_colors, top_t);
    let night_col = gradient3(sky.night_colors, top_t);
    let horizon_col = gradient3(sky.horizon_colors, lower_t);

    let day_weight = sky.params0.y;
    let evening_weight = sky.params0.z;
    let night_weight = sky.params0.w;
    let sky_col = day_col * day_weight + evening_col * evening_weight + night_col * night_weight;
    let color = mix(sky_col, horizon_col, horizon_weight);
    return vec4<f32>(color, 1.0);
}

/*__PERRO_SKY_CUSTOM_STACK__*/

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let ndc = vec4<f32>(in.uv * 2.0 - 1.0, 1.0, 1.0);
    let world_h = sky.inv_view_proj * ndc;
    let world = world_h.xyz / max(world_h.w, 1.0e-5);
    let ray = normalize(world - sky.camera_pos.xyz);
    let horizon_weight = smoothstep(0.0, 0.18, -ray.y);
    let base = SkyFragment(
        ray,
        in.uv,
        sky.params0.x,
        sky.params1.x,
        sky.params0.y,
        sky.params0.z,
        sky.params0.w,
        horizon_weight,
        sky_base_color(ray),
        vec4<f32>(0.0),
        vec4<f32>(0.0),
        vec4<f32>(0.0),
        vec4<f32>(0.0),
        vec4<f32>(0.0),
        vec4<f32>(0.0),
        vec4<f32>(0.0),
        vec4<f32>(0.0),
        vec4<f32>(0.0),
        vec4<f32>(0.0),
        vec4<f32>(0.0),
        vec4<f32>(0.0),
        vec4<f32>(0.0),
        vec4<f32>(0.0),
        vec4<f32>(0.0),
        vec4<f32>(0.0),
    );
    return apply_custom_sky_stack(base);
}
