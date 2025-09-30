// ------------------- //
// Camera UBO (group 1)
// ------------------- //
struct Camera {
    virtual_size: vec2<f32>,
    ndc_scale: vec2<f32>,
};

@group(1) @binding(0) var<uniform> camera: Camera;

// ------------------- //
// Vertex Input
// ------------------- //
struct VertexInput {
    @location(0) position: vec2<f32>, // quad vertex -0.5..+0.5
    @location(1) uv: vec2<f32>,       // unit quad uv 0..1
};

// ------------------- //
// Instance Input (matches Rust ColorFontInstance)
// ------------------- //
struct ColorInstanceInput {
    @location(2) transform_0: vec4<f32>,
    @location(3) transform_1: vec4<f32>,
    @location(4) transform_2: vec4<f32>,
    @location(5) transform_3: vec4<f32>,
    @location(6) uv_offset: vec2<f32>,
    @location(7) uv_size: vec2<f32>,
    @location(8) atlas_id: u32,
    @location(9) z_index: i32,
};

// ------------------- //
// Vertex Output
// ------------------- //
struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(vertex: VertexInput, instance: ColorInstanceInput) -> VertexOutput {
    let transform = mat4x4<f32>(
        instance.transform_0,
        instance.transform_1,
        instance.transform_2,
        instance.transform_3,
    );

    let world = transform * vec4<f32>(vertex.position, 0.0, 1.0);
    let ndc = world.xy * camera.ndc_scale;
    let depth = f32(instance.z_index) * 0.001;

    var out: VertexOutput;
    out.pos = vec4<f32>(ndc, depth, 1.0);
    out.uv = instance.uv_offset + vertex.uv * instance.uv_size;
    return out;
}

// ------------------- //
// Fragment Shader (Direct Color Sampling)
// ------------------- //
@group(0) @binding(0) var color_atlas: texture_2d<f32>;
@group(0) @binding(1) var color_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Direct color sampling - no SDF processing
    return textureSample(color_atlas, color_sampler, in.uv);
}