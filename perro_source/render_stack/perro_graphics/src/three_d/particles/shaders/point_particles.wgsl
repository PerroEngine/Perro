struct Camera3D {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera3D;

struct ParticleIn {
    @location(0) world_pos: vec3<f32>,
    @location(1) size_alpha: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) emissive: vec3<f32>,
}

struct ParticleOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) emissive: vec3<f32>,
}

@vertex
fn vs_main(in: ParticleIn) -> ParticleOut {
    var out: ParticleOut;
    out.clip_pos = camera.view_proj * vec4<f32>(in.world_pos, 1.0);
    out.color = in.color;
    out.color.a = clamp(in.size_alpha.y, 0.0, 1.0);
    out.emissive = in.emissive;
    return out;
}

@fragment
fn fs_main(in: ParticleOut) -> @location(0) vec4<f32> {
    let rgb = in.color.rgb + in.emissive;
    return vec4<f32>(rgb, in.color.a);
}
