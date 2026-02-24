struct Camera2D {
    view: mat4x4<f32>,
    ndc_scale: vec2<f32>,
    pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera2D;

struct VertexInput {
    @location(0) local_pos: vec2<f32>,
};

struct InstanceInput {
    @location(1) center: vec2<f32>,
    @location(2) size: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) z_index: i32,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(v: VertexInput, inst: InstanceInput) -> VertexOutput {
    let world_xy = inst.center + (v.local_pos * inst.size);
    let world = vec4<f32>(world_xy, 0.0, 1.0);
    let view = camera.view * world;
    let ndc_xy = view.xy * camera.ndc_scale;
    let depth = 1.0 - f32(inst.z_index) * 0.001;

    var out: VertexOutput;
    out.clip_pos = vec4<f32>(ndc_xy, depth, 1.0);
    out.color = inst.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
