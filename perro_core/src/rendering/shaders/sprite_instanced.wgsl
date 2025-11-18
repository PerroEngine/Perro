// ─────────────────────────────────────────────
// Sprite Instanced Shader
// Supports full Camera2D transform (translation, rotation, zoom)
// Origin (0,0) = center of world
// ─────────────────────────────────────────────

struct Camera {
    virtual_size: vec2<f32>,
    ndc_scale: vec2<f32>,
    zoom: f32,
    _pad0: f32,
    _pad1: vec2<f32>,
    view: mat4x4<f32>,
};

// Texture bindings
@group(0) @binding(0)
var texture_sampler: sampler;
@group(0) @binding(1)
var texture_diffuse: texture_2d<f32>;

// Camera uniform
@group(1) @binding(0)
var<uniform> camera: Camera;

// Per-vertex and per-instance inputs
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct InstanceInput {
    @location(2) transform_0: vec4<f32>,
    @location(3) transform_1: vec4<f32>,
    @location(4) transform_2: vec4<f32>,
    @location(5) transform_3: vec4<f32>,
    @location(6) pivot: vec2<f32>,
    @location(7) z_index: i32,
};

// Vertex -> Fragment outputs
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// ─────────────────────────────────────────────
// Vertex Shader
// ─────────────────────────────────────────────

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    // Per-instance transform matrix
    let model = mat4x4<f32>(
        instance.transform_0,
        instance.transform_1,
        instance.transform_2,
        instance.transform_3,
    );

    // Transform the local vertex position by instance's model transform
    var world_pos = model * vec4<f32>(vertex.position, 0.0, 1.0);

    // Apply camera view matrix (inverse camera transform)
    world_pos = camera.view * world_pos;

    // Apply zoom (must rebuild full vector; WGSL doesn’t allow swizzle assignment)
    world_pos = vec4<f32>(world_pos.xy * camera.zoom, world_pos.z, world_pos.w);

    // Convert to clip-space (scale from virtual world to NDC range)
    let ndc_pos = world_pos.xy * camera.ndc_scale;

    // Depth from z-index
    let depth = f32(instance.z_index) * 0.001;

    var out: VertexOutput;
    out.clip_position = vec4<f32>(ndc_pos, depth, 1.0);
    out.uv = vertex.uv;
    return out;
}

// ─────────────────────────────────────────────
// Fragment Shader
// ─────────────────────────────────────────────

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(texture_diffuse, texture_sampler, in.uv);
}