// Fixed shader with correct group bindings - minimal changes from original
struct Camera {
    virtual_size: vec2<f32>,
    ndc_scale: vec2<f32>,
}

@group(1) @binding(0) var<uniform> camera: Camera;

// Vertex input: local quad (-0.5..0.5)
struct VertexInput {
    @location(0) position: vec2<f32>, // quad local position
    @location(1) uv: vec2<f32>,       // unit quad uv (0..1)
}

// Instance inputs (matches Rust FontInstance)
struct InstanceInput {
    @location(2) transform_0: vec4<f32>,
    @location(3) transform_1: vec4<f32>,
    @location(4) transform_2: vec4<f32>,
    @location(5) transform_3: vec4<f32>,
    @location(6) color: vec4<f32>,
    @location(7) uv_offset: vec2<f32>, // in atlas
    @location(8) uv_size: vec2<f32>,   // in atlas
    @location(9) z_index: i32,
}

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,        // atlas UV for fragment shader
    @location(1) color: vec4<f32>,
    @location(2) local_pos: vec2<f32>, // for AA/optional effects
}

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    // Reconstruct instance transform
    let transform = mat4x4<f32>(
        instance.transform_0,
        instance.transform_1,
        instance.transform_2,
        instance.transform_3
    );

    // Transform to world space
    let world_pos4 = transform * vec4<f32>(vertex.position, 0.0, 1.0);

    // Convert to NDC
    let ndc_pos = world_pos4.xy * camera.ndc_scale;
    let depth = f32(instance.z_index) * 0.001;

    var out: VertexOutput;
    out.pos = vec4<f32>(ndc_pos, depth, 1.0);

    // Map quad UVs into atlas
    out.uv = instance.uv_offset + vertex.uv * instance.uv_size;
    out.color = instance.color;
    out.local_pos = vertex.position;
    return out;
}

// Fragment shader: sample from font atlas
@group(0) @binding(0) var font_atlas: texture_2d<f32>;
@group(0) @binding(1) var font_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let alpha = textureSample(font_atlas, font_sampler, in.uv).r;
    
    // Simple alpha test - no fancy SDF for now
    if (alpha < 0.1) {
        discard;
    }

    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}