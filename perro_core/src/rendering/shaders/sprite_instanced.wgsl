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

// ─────────────────────────────────────────────
// Inputs
// ─────────────────────────────────────────────

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct InstanceInput {
    // Mat3 (column-major)
    @location(2) transform_0: vec3<f32>,
    @location(3) transform_1: vec3<f32>,
    @location(4) transform_2: vec3<f32>,

    @location(5) pivot: vec2<f32>,
    @location(6) z_index: i32,
};

// ─────────────────────────────────────────────
// Outputs
// ─────────────────────────────────────────────

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// ─────────────────────────────────────────────
// Mat3 → Mat4 helper
// ─────────────────────────────────────────────

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

// ─────────────────────────────────────────────
// Vertex Shader
// ─────────────────────────────────────────────

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    // Build transform from Mat3
    let model = mat3_to_mat4(
        instance.transform_0,
        instance.transform_1,
        instance.transform_2,
    );

    // Transform local vertex position
    var world_pos = model * vec4<f32>(vertex.position, 0.0, 1.0);

    // Apply camera view
    world_pos = camera.view * world_pos;

    // Apply zoom
    world_pos = vec4<f32>(
        world_pos.xy * (1.0 + camera.zoom),
        world_pos.z,
        world_pos.w,
    );

    // Convert to NDC
    let ndc_pos = world_pos.xy * camera.ndc_scale;

    // Depth from z_index
    let depth = 1.0 - f32(instance.z_index) * 0.001;

    var out: VertexOutput;
    out.clip_position = vec4<f32>(ndc_pos, depth, 1.0);
    out.uv = vertex.uv;
    return out;
}

// ─────────────────────────────────────────────
// Fragment Shader
// ─────────────────────────────────────────────
// Texture is Rgba8UnormSrgb: sampling returns linear. We output linear; the sRGB
// render target will convert linear→sRGB on write. If textures look dimmed on some
// backends, the driver may not be doing that conversion — then uncomment the
// linear_to_srgb() below so we output sRGB and the target stores as-is.

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        return c * 12.92;
    }
    return 1.055 * pow(c, 1.0 / 2.4) - 0.055;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // OPTIMIZED: Add half-texel offset to ensure consistent sampling at texel centers
    let texture_size = textureDimensions(texture_diffuse);
    let half_texel = vec2<f32>(0.5 / f32(texture_size.x), 0.5 / f32(texture_size.y));
    let adjusted_uv = in.uv + half_texel;
    let linear = textureSample(texture_diffuse, texture_sampler, adjusted_uv);
    // Some backends don't apply linear→sRGB when writing to sRGB targets, causing dimming.
    // Encode to sRGB in shader so the stored value displays correctly.
    return vec4<f32>(
        linear_to_srgb(linear.r),
        linear_to_srgb(linear.g),
        linear_to_srgb(linear.b),
        linear.a,
    );
}