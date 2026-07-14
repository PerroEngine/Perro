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

@fragment fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) f32 {
    let divisor = max(params.target_divisor, 1u);
    let target_size = vec2<i32>((vec2<u32>(params.full_size) + vec2<u32>(divisor - 1u)) / divisor);
    let p = clamp(vec2<i32>(frag.xy), vec2<i32>(0), target_size - vec2<i32>(1));
    let full_p = min(p * i32(divisor), vec2<i32>(params.full_size) - vec2<i32>(1));
    let center_depth = textureLoad(depth_tex, full_p, 0);
    var sum = 0.0;
    var weight_sum = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let q = clamp(p + vec2<i32>(x, y), vec2<i32>(0), target_size - vec2<i32>(1));
            let q_depth = textureLoad(depth_tex, min(q * i32(divisor), vec2<i32>(params.full_size) - vec2<i32>(1)), 0);
            let spatial = exp(-0.75 * f32(x * x + y * y));
            let edge = exp(-abs(q_depth - center_depth) * params.depth_sigma);
            let weight = spatial * edge;
            sum += textureLoad(ao_tex, q, 0).r * weight;
            weight_sum += weight;
        }
    }
    return sum / max(weight_sum, 1e-5);
}
