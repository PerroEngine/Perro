struct Camera {
    virtual_size: vec2<f32>,
    ndc_scale: vec2<f32>,  // Pre-computed scaling factors
}

@group(0) @binding(0)
var texture_sampler: sampler;
@group(0) @binding(1)
var texture_diffuse: texture_2d<f32>;

@group(1) @binding(0)
var<uniform> camera: Camera;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

struct InstanceInput {
    @location(2) transform_0: vec4<f32>,
    @location(3) transform_1: vec4<f32>,
    @location(4) transform_2: vec4<f32>,
    @location(5) transform_3: vec4<f32>,
    @location(6) pivot: vec2<f32>,
    @location(7) z_index: i32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let transform = mat4x4<f32>(
        instance.transform_0,
        instance.transform_1,
        instance.transform_2,
        instance.transform_3,
    );

    let world_pos = transform * vec4<f32>(vertex.position, 0.0, 1.0);
    
    // Use pre-computed NDC scaling (no runtime calculation!)
    let ndc_pos = world_pos.xy * camera.ndc_scale;
    
    // Convert z_index to depth
    let depth = f32(instance.z_index) * 0.001;

    var out: VertexOutput;
    out.clip_position = vec4<f32>(ndc_pos, depth, 1.0);
    out.uv = vertex.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(texture_diffuse, texture_sampler, in.uv);
}