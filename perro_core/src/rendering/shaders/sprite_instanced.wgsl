struct Camera {
    virtual_size: vec2<f32>,
    window_size: vec2<f32>,
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
    
    let aspect_x = camera.virtual_size.x / camera.window_size.x;
    let aspect_y = camera.virtual_size.y / camera.window_size.y;
    let scale = min(aspect_x, aspect_y);
    
    let ndc_pos = vec2<f32>(
        world_pos.x * 2.0 / camera.virtual_size.x,
        world_pos.y * 2.0 / camera.virtual_size.y
    ) * scale;

    var out: VertexOutput;
    out.clip_position = vec4<f32>(ndc_pos, 0.0, 1.0);
    out.uv = vertex.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(texture_diffuse, texture_sampler, in.uv);
}