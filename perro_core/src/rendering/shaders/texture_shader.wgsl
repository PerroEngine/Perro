// shaders/texture_shader.wgsl

// group 0: your texture + sampler
@group(0) @binding(0) var img_sampler: sampler;
@group(0) @binding(1) var img_texture: texture_2d<f32>;

// group 1: your 4×4 transform uniform
@group(1) @binding(0) var<uniform> transform: mat4x4<f32>;

// this struct carries position+UV from vertex→fragment
struct VSOut {
    @builtin(position) Position: vec4<f32>,
    @location(0)        uv:       vec2<f32>,
};

@vertex
fn vs_main(
    @location(0) pos: vec2<f32>,
    @location(1) uv:  vec2<f32>,
) -> VSOut {
    var out: VSOut;
    // embed your 2D pos into a vec4, apply the 4×4 transform
    let p4 = transform * vec4<f32>(pos, 0.0, 1.0);
    out.Position = p4;
    out.uv       = uv;
    return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
    // sample the bound texture
    return textureSample(img_texture, img_sampler, in.uv);
}