struct PostUniform {
    effect_type: u32,
    param_count: u32,
    projection_mode: u32,
    _pad0: u32,
    params0: vec4<f32>,
    params1: vec4<f32>,
    params2: vec4<f32>,
    params3: vec4<f32>,
    resolution: vec2<f32>,
    inv_resolution: vec2<f32>,
    near: f32,
    far: f32,
};

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var depth_tex: texture_depth_2d;
@group(0) @binding(3) var<uniform> post: PostUniform;
@group(0) @binding(4) var<storage, read> custom_params: array<vec4<f32>>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(pos[vid], 0.0, 1.0);
    out.uv = (out.pos.xy * vec2<f32>(0.5, -0.5)) + vec2<f32>(0.5, 0.5);
    return out;
}

fn load_depth(uv: vec2<f32>) -> f32 {
    let dims = textureDimensions(depth_tex);
    let ix = clamp(i32(uv.x * f32(dims.x)), 0, i32(dims.x) - 1);
    let iy = clamp(i32(uv.y * f32(dims.y)), 0, i32(dims.y) - 1);
    return textureLoad(depth_tex, vec2<i32>(ix, iy), 0);
}

fn linearize_depth(depth: f32) -> f32 {
    if post.projection_mode == 1u {
        return post.near + depth * (post.far - post.near);
    }
    return (post.near * post.far) / (post.far - depth * (post.far - post.near));
}

fn blur_sample(uv: vec2<f32>, strength: f32) -> vec4<f32> {
    let s = max(strength, 0.0);
    if s <= 0.001 {
        return textureSample(input_tex, input_sampler, uv);
    }
    let o = s * post.inv_resolution;
    var sum = vec4<f32>(0.0);
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(-o.x, -o.y));
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(0.0, -o.y));
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(o.x, -o.y));
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(-o.x, 0.0));
    sum += textureSample(input_tex, input_sampler, uv);
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(o.x, 0.0));
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(-o.x, o.y));
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(0.0, o.y));
    sum += textureSample(input_tex, input_sampler, uv + vec2<f32>(o.x, o.y));
    return sum / 9.0;
}

fn post_process(uv: vec2<f32>, color: vec4<f32>, depth: f32) -> vec4<f32> {
    if post.effect_type == 1u {
        let strength = post.params0.x;
        return blur_sample(uv, strength);
    }
    if post.effect_type == 2u {
        let size = max(post.params0.x, 1.0);
        let pix = floor(uv * post.resolution / size) * size / post.resolution;
        return textureSample(input_tex, input_sampler, pix);
    }
    if post.effect_type == 3u {
        let waves = max(post.params0.x, 0.0);
        let strength = post.params0.y;
        let offset = sin(uv.y * waves * 6.2831853) * strength * post.inv_resolution.x;
        return textureSample(input_tex, input_sampler, uv + vec2<f32>(offset, 0.0));
    }
    return color;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let color = textureSample(input_tex, input_sampler, in.uv);
    let depth = load_depth(in.uv);
    return post_process(in.uv, color, depth);
}
