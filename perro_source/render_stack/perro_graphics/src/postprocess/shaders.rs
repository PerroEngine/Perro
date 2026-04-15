const BUILTIN_POST_BODY_WGSL: &str = perro_macros::include_str_stripped!("shaders/postprocess_builtin_body.wgsl");
const EFFECT_BLUR_WGSL: &str = perro_macros::include_str_stripped!("shaders/effects/blur.wgsl");
const EFFECT_PIXELATE_WGSL: &str = perro_macros::include_str_stripped!("shaders/effects/pixelate.wgsl");
const EFFECT_WARP_WGSL: &str = perro_macros::include_str_stripped!("shaders/effects/warp.wgsl");
const EFFECT_VIGNETTE_WGSL: &str = perro_macros::include_str_stripped!("shaders/effects/vignette.wgsl");
const EFFECT_CRT_WGSL: &str = perro_macros::include_str_stripped!("shaders/effects/crt.wgsl");
const EFFECT_COLOR_FILTER_WGSL: &str = perro_macros::include_str_stripped!("shaders/effects/color_filter.wgsl");
const EFFECT_REVERSE_FILTER_WGSL: &str = perro_macros::include_str_stripped!("shaders/effects/reverse_filter.wgsl");
const EFFECT_BLOOM_WGSL: &str = perro_macros::include_str_stripped!("shaders/effects/bloom.wgsl");
const EFFECT_SATURATE_WGSL: &str = perro_macros::include_str_stripped!("shaders/effects/saturate.wgsl");
const EFFECT_BLACK_WHITE_WGSL: &str = perro_macros::include_str_stripped!("shaders/effects/black_white.wgsl");

pub fn create_builtin_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
    let mut wgsl = String::new();
    wgsl.push_str(PRELUDE_WGSL);
    wgsl.push_str(EFFECT_BLUR_WGSL);
    wgsl.push_str(EFFECT_PIXELATE_WGSL);
    wgsl.push_str(EFFECT_WARP_WGSL);
    wgsl.push_str(EFFECT_VIGNETTE_WGSL);
    wgsl.push_str(EFFECT_CRT_WGSL);
    wgsl.push_str(EFFECT_COLOR_FILTER_WGSL);
    wgsl.push_str(EFFECT_REVERSE_FILTER_WGSL);
    wgsl.push_str(EFFECT_BLOOM_WGSL);
    wgsl.push_str(EFFECT_SATURATE_WGSL);
    wgsl.push_str(EFFECT_BLACK_WHITE_WGSL);
    wgsl.push_str(BUILTIN_POST_BODY_WGSL);
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_post_builtin"),
        source: wgpu::ShaderSource::Wgsl(wgsl.into()),
    })
}

pub fn build_post_shader(custom_wgsl: &str) -> String {
    let mut out = String::new();
    out.push_str(PRELUDE_WGSL);
    out.push_str(custom_wgsl);
    out
}

const PRELUDE_WGSL: &str = r#"
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
    time: vec2<f32>,
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

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let color = textureSample(input_tex, input_sampler, in.uv);
    let depth = load_depth(in.uv);
    let out_color = post_process(in.uv, color, depth);
    return vec4<f32>(out_color.rgb, 1.0);
}

"#;

