struct Camera3D {
    view_proj: mat4x4<f32>,
    inv_view_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera3D;

struct ComputedParticle {
    world_pos: vec4<f32>,
    color: vec4<f32>,
    emissive: vec4<f32>,
}

@group(1) @binding(8)
var<storage, read> particles_read: array<ComputedParticle>;

struct ParticleOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) emissive: vec3<f32>,
}

@vertex
fn vs_main(@builtin(instance_index) particle_index: u32) -> ParticleOut {
    var out: ParticleOut;
    let p = particles_read[particle_index];
    out.clip_pos = camera.view_proj * vec4<f32>(p.world_pos.xyz, 1.0);
    out.color = p.color;
    out.emissive = p.emissive.xyz;
    return out;
}

@vertex
fn vs_billboard(
    @builtin(instance_index) particle_index: u32,
    @builtin(vertex_index) vertex_index: u32,
) -> ParticleOut {
    var out: ParticleOut;
    let p = particles_read[particle_index];
    let center_clip = camera.view_proj * vec4<f32>(p.world_pos.xyz, 1.0);
    let corners = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, 1.0),
    );
    let half_size = max(p.world_pos.w * 0.5, 1.0);
    let ndc_offset = corners[vertex_index] * half_size * camera.inv_view_size * 2.0;
    out.clip_pos = center_clip + vec4<f32>(ndc_offset * center_clip.w, 0.0, 0.0);
    out.color = p.color;
    out.emissive = p.emissive.xyz;
    return out;
}

@fragment
fn fs_main(in: ParticleOut) -> @location(0) vec4<f32> {
    let rgb = in.color.rgb + in.emissive;
    return vec4<f32>(rgb, in.color.a);
}
