struct Params {
    inv_view_proj: mat4x4<f32>,
    full_size: vec2<f32>,
    radius_px: f32,
    strength: f32,
    depth_sigma: f32,
    sample_count: u32,
    target_divisor: u32,
    _pad: f32,
};

@group(0) @binding(0) var ao_tex: texture_2d<f32>;
@group(0) @binding(1) var depth_tex: texture_depth_2d;
@group(0) @binding(2) var<uniform> params: Params;

struct VsOut { @builtin(position) pos: vec4<f32> };

@vertex fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: VsOut;
    out.pos = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    return out;
}

fn full_pixel(pixel: vec2<i32>, divisor: i32) -> vec2<i32> {
    let full_size = vec2<i32>(params.full_size);
    return clamp(
        pixel * divisor + vec2<i32>(divisor / 2),
        vec2<i32>(0),
        full_size - vec2<i32>(1),
    );
}

fn world_at_depth(pixel: vec2<i32>, depth: f32) -> vec3<f32> {
    let uv = (vec2<f32>(pixel) + 0.5) / params.full_size;
    let ndc = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), depth, 1.0);
    let world = params.inv_view_proj * ndc;
    return world.xyz / max(abs(world.w), 1.0e-6);
}

fn view_distance(pixel: vec2<i32>, depth: f32) -> f32 {
    return distance(world_at_depth(pixel, depth), world_at_depth(pixel, 0.0));
}

@fragment fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) f32 {
    let divisor = i32(max(params.target_divisor, 1u));
    let target_size = vec2<i32>(textureDimensions(ao_tex));
    let pixel = clamp(vec2<i32>(frag.xy), vec2<i32>(0), target_size - vec2<i32>(1));
    let center_full = full_pixel(pixel, divisor);
    let center_depth = textureLoad(depth_tex, center_full, 0);
    if center_depth >= 0.999999 {
        return 1.0;
    }
    let center_distance = view_distance(center_full, center_depth);
    var sum = 0.0;
    var weight_sum = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let q = clamp(pixel + vec2<i32>(x, y), vec2<i32>(0), target_size - vec2<i32>(1));
            let q_full = full_pixel(q, divisor);
            let q_depth = textureLoad(depth_tex, q_full, 0);
            if q_depth < 0.999999 {
                let q_distance = view_distance(q_full, q_depth);
                let relative_depth = abs(q_distance - center_distance) / max(center_distance, 1.0e-3);
                let spatial = exp(-0.75 * f32(x * x + y * y));
                let edge = exp(-relative_depth * params.depth_sigma * 0.33);
                let weight = spatial * edge;
                sum += textureLoad(ao_tex, q, 0).r * weight;
                weight_sum += weight;
            }
        }
    }
    return sum / max(weight_sum, 1.0e-5);
}
