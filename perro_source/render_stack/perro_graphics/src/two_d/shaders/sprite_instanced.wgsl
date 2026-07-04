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
    @location(2) transform_0: vec2<f32>,
    @location(3) transform_1: vec2<f32>,
    @location(4) translation: vec2<f32>,
    @location(5) uv_min: vec2<f32>,
    @location(6) uv_max: vec2<f32>,
    @location(7) size: vec2<f32>,
    @location(8) @interpolate(flat) z_index: i32,
    @location(9) tint: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) tint: vec4<f32>,
};

@vertex
fn vs_main(v: VertexInput, inst: InstanceInput) -> VertexOutput {
    let tex_size = vec2<f32>(textureDimensions(tex_color));
    let local = v.local_pos * inst.size;
    let world_xy = inst.transform_0 * local.x + inst.transform_1 * local.y + inst.translation;
    let view = camera.view * vec4<f32>(world_xy, 0.0, 1.0);
    let ndc_xy = view.xy * camera.ndc_scale;
    let depth = 1.0 - f32(inst.z_index) * 0.001;

    var out: VertexOutput;
    out.clip_pos = vec4<f32>(ndc_xy, depth, 1.0);
    out.uv = (inst.uv_min + v.uv * (inst.uv_max - inst.uv_min)) / tex_size;
    out.tint = inst.tint;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(tex_color, tex_sampler, in.uv) * in.tint;
}
