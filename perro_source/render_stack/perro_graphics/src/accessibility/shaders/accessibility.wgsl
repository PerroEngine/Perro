struct AccessibilityUniform {
    mode: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    params0: vec4<f32>,
};

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var<uniform> accessibility: AccessibilityUniform;

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

fn color_blind_matrix(mode: u32) -> mat3x3<f32> {
    if mode == 0u {
        return mat3x3<f32>(
            vec3<f32>(0.567, 0.433, 0.000),
            vec3<f32>(0.558, 0.442, 0.000),
            vec3<f32>(0.000, 0.242, 0.758),
        );
    }
    if mode == 1u {
        return mat3x3<f32>(
            vec3<f32>(0.625, 0.375, 0.000),
            vec3<f32>(0.700, 0.300, 0.000),
            vec3<f32>(0.000, 0.300, 0.700),
        );
    }
    return mat3x3<f32>(
        vec3<f32>(0.950, 0.050, 0.000),
        vec3<f32>(0.000, 0.433, 0.567),
        vec3<f32>(0.000, 0.475, 0.525),
    );
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let color = textureSample(input_tex, input_sampler, in.uv);
    let m = color_blind_matrix(accessibility.mode);
    let t = clamp(accessibility.params0.x, 0.0, 1.0);
    let simulated = m * color.rgb;
    return vec4<f32>(mix(color.rgb, simulated, t), color.a);
}
