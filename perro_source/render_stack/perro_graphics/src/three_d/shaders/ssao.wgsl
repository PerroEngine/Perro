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

@group(0) @binding(0) var depth_tex: texture_depth_2d;
@group(0) @binding(1) var<uniform> params: Params;

struct VsOut { @builtin(position) pos: vec4<f32> };

@vertex fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: VsOut;
    out.pos = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    return out;
}

fn world_at(pixel: vec2<i32>) -> vec3<f32> {
    let size = vec2<i32>(params.full_size);
    let p = clamp(pixel, vec2<i32>(0), size - vec2<i32>(1));
    let depth = textureLoad(depth_tex, p, 0);
    let uv = (vec2<f32>(p) + 0.5) / params.full_size;
    let h = params.inv_view_proj * vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), depth, 1.0);
    return h.xyz / max(abs(h.w), 1e-6);
}

const KERNEL: array<vec2<f32>, 16> = array<vec2<f32>, 16>(
    vec2<f32>(1.0, 0.0), vec2<f32>(0.924, 0.383), vec2<f32>(0.707, 0.707), vec2<f32>(0.383, 0.924),
    vec2<f32>(0.0, 1.0), vec2<f32>(-0.383, 0.924), vec2<f32>(-0.707, 0.707), vec2<f32>(-0.924, 0.383),
    vec2<f32>(-1.0, 0.0), vec2<f32>(-0.924, -0.383), vec2<f32>(-0.707, -0.707), vec2<f32>(-0.383, -0.924),
    vec2<f32>(0.0, -1.0), vec2<f32>(0.383, -0.924), vec2<f32>(0.707, -0.707), vec2<f32>(0.924, -0.383)
);

@fragment fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) f32 {
    let pixel = vec2<i32>(frag.xy * f32(params.target_divisor));
    let center = world_at(pixel);
    let dx = world_at(pixel + vec2<i32>(1, 0)) - center;
    let dy = world_at(pixel + vec2<i32>(0, 1)) - center;
    let normal = normalize(cross(dx, dy));
    var hit = 0.0;
    for (var i = 0u; i < params.sample_count; i++) {
        let ring = 0.35 + 0.65 * f32(i + 1u) / f32(params.sample_count);
        let q = pixel + vec2<i32>(KERNEL[i] * params.radius_px * ring);
        let delta = world_at(q) - center;
        let dist = length(delta);
        let facing = max(dot(normal, delta / max(dist, 1e-5)) - 0.08, 0.0);
        hit += facing / (1.0 + dist * dist * 2.0);
    }
    return clamp(1.0 - hit * params.strength / max(f32(params.sample_count), 1.0), 0.0, 1.0);
}
