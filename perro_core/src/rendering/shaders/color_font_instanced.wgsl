// ------------------- //
// Camera UBO (group 1)
// ------------------- //
struct Camera {
    virtual_size: vec2<f32>,
    ndc_scale: vec2<f32>,
};

@group(1) @binding(0)
var<uniform> camera: Camera;

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
    // Mat3 (column-major)
    @location(2) transform_0: vec3<f32>,
    @location(3) transform_1: vec3<f32>,
    @location(4) transform_2: vec3<f32>,

    @location(5) uv_offset: vec2<f32>,
    @location(6) uv_size: vec2<f32>,
    @location(7) atlas_id: u32,
    @location(8) z_index: i32,
};

// ------------------- //
// Vertex Output
// ------------------- //
struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// ------------------- //
// Mat3 → Mat4 helper
// ------------------- //
fn mat3_to_mat4(
    t0: vec3<f32>,
    t1: vec3<f32>,
    t2: vec3<f32>,
) -> mat4x4<f32> {
    return mat4x4<f32>(
        vec4<f32>(t0.xy, 0.0, t0.z),
        vec4<f32>(t1.xy, 0.0, t1.z),
        vec4<f32>(0.0, 0.0, 1.0, 0.0),
        vec4<f32>(t2.xy, 0.0, 1.0),
    );
}

@vertex
fn vs_main(vertex: VertexInput, instance: ColorInstanceInput) -> VertexOutput {
    let transform = mat3_to_mat4(
        instance.transform_0,
        instance.transform_1,
        instance.transform_2,
    );

    let world = transform * vec4<f32>(vertex.position, 0.0, 1.0);
    let ndc = world.xy * camera.ndc_scale;

    // Depth from z_index (inverted: higher z_index = lower depth = renders on top)
    let depth = 1.0 - f32(instance.z_index) * 0.001;

    var out: VertexOutput;
    out.pos = vec4<f32>(ndc, depth, 1.0);
    out.uv = instance.uv_offset + vertex.uv * instance.uv_size;
    return out;
}

// ------------------- //
// Fragment Shader (Direct Color Sampling)
// ------------------- //
@group(0) @binding(0)
var color_atlas: texture_2d<f32>;
@group(0) @binding(1)
var color_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(color_atlas, color_sampler, in.uv);
}