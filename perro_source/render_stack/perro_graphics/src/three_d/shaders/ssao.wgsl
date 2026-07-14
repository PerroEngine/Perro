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

fn clamp_pixel(pixel: vec2<i32>) -> vec2<i32> {
    return clamp(pixel, vec2<i32>(0), vec2<i32>(params.full_size) - vec2<i32>(1));
}

fn depth_at(pixel: vec2<i32>) -> f32 {
    return textureLoad(depth_tex, clamp_pixel(pixel), 0);
}

fn world_at_depth(pixel: vec2<i32>, depth: f32) -> vec3<f32> {
    let p = clamp_pixel(pixel);
    let uv = (vec2<f32>(p) + 0.5) / params.full_size;
    let ndc = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), depth, 1.0);
    let world = params.inv_view_proj * ndc;
    return world.xyz / max(abs(world.w), 1.0e-6);
}

fn world_at(pixel: vec2<i32>) -> vec3<f32> {
    return world_at_depth(pixel, depth_at(pixel));
}

fn hash12(pixel: vec2<f32>) -> f32 {
    let p = fract(pixel * vec2<f32>(0.1031, 0.1030));
    let p3 = vec3<f32>(p.x, p.y, p.x)
        + dot(vec3<f32>(p.x, p.y, p.x), vec3<f32>(p.y, p.x, p.x) + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

const KERNEL: array<vec2<f32>, 16> = array<vec2<f32>, 16>(
    vec2<f32>(1.0, 0.0), vec2<f32>(0.924, 0.383), vec2<f32>(0.707, 0.707), vec2<f32>(0.383, 0.924),
    vec2<f32>(0.0, 1.0), vec2<f32>(-0.383, 0.924), vec2<f32>(-0.707, 0.707), vec2<f32>(-0.924, 0.383),
    vec2<f32>(-1.0, 0.0), vec2<f32>(-0.924, -0.383), vec2<f32>(-0.707, -0.707), vec2<f32>(-0.383, -0.924),
    vec2<f32>(0.0, -1.0), vec2<f32>(0.383, -0.924), vec2<f32>(0.707, -0.707), vec2<f32>(0.924, -0.383)
);

@fragment fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) f32 {
    let divisor = max(params.target_divisor, 1u);
    let pixel = clamp_pixel(vec2<i32>(frag.xy * f32(divisor)));
    let center_depth = depth_at(pixel);
    if center_depth >= 0.999999 {
        return 1.0;
    }

    let center = world_at(pixel);
    let left = world_at(pixel - vec2<i32>(1, 0));
    let right = world_at(pixel + vec2<i32>(1, 0));
    let up = world_at(pixel - vec2<i32>(0, 1));
    let down = world_at(pixel + vec2<i32>(0, 1));
    let dx = select(right - center, center - left, distance(left, center) < distance(right, center));
    let dy = select(down - center, center - up, distance(up, center) < distance(down, center));
    let normal_raw = cross(dx, dy);
    if dot(normal_raw, normal_raw) < 1.0e-10 {
        return 1.0;
    }
    var normal = normalize(normal_raw);
    let near_point = world_at_depth(pixel, 0.0);
    if dot(normal, near_point - center) < 0.0 {
        normal = -normal;
    }

    let sample_count = min(params.sample_count, 16u);
    if sample_count == 0u {
        return 1.0;
    }
    let angle = hash12(vec2<f32>(pixel)) * 6.2831853;
    let rotation = mat2x2<f32>(cos(angle), -sin(angle), sin(angle), cos(angle));
    let radius_world = max((length(dx) + length(dy)) * 0.5 * params.radius_px, 1.0e-4);
    var occlusion = 0.0;
    for (var i = 0u; i < sample_count; i++) {
        let ring = 0.25 + 0.75 * f32(i + 1u) / f32(sample_count);
        let offset = vec2<i32>(rotation * KERNEL[i] * params.radius_px * ring);
        let sample_pixel = clamp_pixel(pixel + offset);
        let sample_depth = depth_at(sample_pixel);
        if sample_depth < 0.999999 {
            let delta = world_at_depth(sample_pixel, sample_depth) - center;
            let sample_dist = length(delta);
            let horizon = max(dot(normal, delta / max(sample_dist, 1.0e-5)) - 0.06, 0.0);
            let range_weight = 1.0 - smoothstep(radius_world * 0.15, radius_world, sample_dist);
            occlusion += horizon * range_weight;
        }
    }
    let normalized = occlusion / f32(sample_count);
    return clamp(1.0 - normalized * params.strength, 0.0, 1.0);
}
