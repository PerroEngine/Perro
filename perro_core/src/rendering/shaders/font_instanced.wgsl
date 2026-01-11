// ------------------- //
// Camera UBO (group 1)
// ------------------- //
struct Camera {
    virtual_size: vec2<f32>,
    ndc_scale: vec2<f32>,
    zoom: f32,
    _pad0: f32,
    _pad1: vec2<f32>,
    view: mat4x4<f32>,
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
// Instance Input (matches Rust FontInstance)
// ------------------- //
struct InstanceInput {
    // Mat3 (column-major)
    @location(2) transform_0: vec3<f32>,
    @location(3) transform_1: vec3<f32>,
    @location(4) transform_2: vec3<f32>,

    @location(5) color: vec4<f32>,
    @location(6) uv_offset: vec2<f32>,
    @location(7) uv_size: vec2<f32>,
    @location(8) z_index: i32,
};

// ------------------- //
// Vertex Output
// ------------------- //
struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
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
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    // Build transform from Mat3
    let transform = mat3_to_mat4(
        instance.transform_0,
        instance.transform_1,
        instance.transform_2,
    );

    // Transform to world space
    var world = transform * vec4<f32>(vertex.position, 0.0, 1.0);
    
    // Apply camera view (for UI scaling)
    world = camera.view * world;
    
    // Apply zoom
    world = vec4<f32>(
        world.xy * (1.0 + camera.zoom),
        world.z,
        world.w,
    );
    
    // Convert to NDC
    let ndc = world.xy * camera.ndc_scale;

    // Depth from z_index (inverted: higher z_index = lower depth = renders on top)
    let depth = 1.0 - f32(instance.z_index) * 0.001;

    var out: VertexOutput;
    out.pos = vec4<f32>(ndc, depth, 1.0);
    out.uv = instance.uv_offset + vertex.uv * instance.uv_size;
    out.color = instance.color;
    return out;
}

// ------------------- //
// Fragment Shader (SDF Decode)
// ------------------- //
@group(0) @binding(0)
var font_atlas: texture_2d<f32>;
@group(0) @binding(1)
var font_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the gamma-corrected font atlas
    // The atlas is stored in gamma space for better mipmap quality
    let gamma_alpha = textureSample(font_atlas, font_sampler, in.uv).r;
    
    // Convert from gamma space back to linear for correct alpha blending
    // Use gamma 1.8 (matches the encoding gamma)
    let linear_alpha = pow(gamma_alpha, 1.8);
    
    // Dynamic threshold based on mip level (texture detail)
    // Calculate how much the texture is being stretched/compressed
    let dx = dpdx(in.uv);
    let dy = dpdy(in.uv);
    let texel_delta = length(dx) + length(dy);
    
    // For large text (low texel_delta), use higher threshold to remove artifacts
    // For small text (high texel_delta), use lower threshold to preserve detail
    let threshold = mix(0.15, 0.05, clamp(texel_delta * 100.0, 0.0, 1.0));
    
    if linear_alpha < threshold {
        discard;
    }
    
    return vec4<f32>(in.color.rgb, in.color.a * linear_alpha);
}