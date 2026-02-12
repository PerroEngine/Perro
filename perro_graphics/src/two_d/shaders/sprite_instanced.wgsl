struct Camera2D {
    view: mat4x4<f32>,
    ndc_scale: vec2<f32>,
    pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera2D;

@group(1) @binding(0)
var tex_sampler: sampler;
@group(1) @binding(1)
var tex_color: texture_2d<f32>;

struct VertexInput {
    @location(0) local_pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct InstanceInput {
    @location(2) transform_0: vec3<f32>,
    @location(3) transform_1: vec3<f32>,
    @location(4) transform_2: vec3<f32>,
    @location(5) z_index: i32,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

fn mat3_to_mat4(t0: vec3<f32>, t1: vec3<f32>, t2: vec3<f32>) -> mat4x4<f32> {
    return mat4x4<f32>(
        vec4<f32>(t0.xy, 0.0, t0.z),
        vec4<f32>(t1.xy, 0.0, t1.z),
        vec4<f32>(0.0, 0.0, 1.0, 0.0),
        vec4<f32>(t2.xy, 0.0, 1.0),
    );
}

@vertex
fn vs_main(v: VertexInput, inst: InstanceInput) -> VertexOutput {
    let model = mat3_to_mat4(inst.transform_0, inst.transform_1, inst.transform_2);
    let world = model * vec4<f32>(v.local_pos, 0.0, 1.0);
    let view = camera.view * world;
    let ndc_xy = view.xy * camera.ndc_scale;
    let depth = 1.0 - f32(inst.z_index) * 0.001;

    var out: VertexOutput;
    out.clip_pos = vec4<f32>(ndc_xy, depth, 1.0);
    out.uv = v.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(tex_color, tex_sampler, in.uv);
}
