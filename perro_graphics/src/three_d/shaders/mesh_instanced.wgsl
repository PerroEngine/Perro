struct Camera3D {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera3D;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct InstanceInput {
    @location(2) model_0: vec4<f32>,
    @location(3) model_1: vec4<f32>,
    @location(4) model_2: vec4<f32>,
    @location(5) model_3: vec4<f32>,
    @location(6) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) normal_ws: vec3<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(v: VertexInput, inst: InstanceInput) -> VertexOutput {
    let model = mat4x4<f32>(inst.model_0, inst.model_1, inst.model_2, inst.model_3);
    let world = model * vec4<f32>(v.pos, 1.0);
    let normal_ws = normalize((model * vec4<f32>(v.normal, 0.0)).xyz);

    var out: VertexOutput;
    out.clip_pos = camera.view_proj * world;
    out.normal_ws = normal_ws;
    out.color = inst.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let light_dir = normalize(vec3<f32>(0.6, 0.8, 0.3));
    let ndotl = max(dot(in.normal_ws, light_dir), 0.0);
    let lit = 0.2 + ndotl * 0.8;
    return vec4<f32>(in.color.rgb * lit, in.color.a);
}

