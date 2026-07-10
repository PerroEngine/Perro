struct FlipWater {
    particle_offset_count: vec4<u32>,
    grid_offset_dims_x: vec4<u32>,
    dims_yz_pad: vec4<u32>,
    size_depth_cell: vec4<f32>,
    flow_splash: vec4<f32>,
    splash_pos_radius: vec4<f32>,
    deep_color: vec4<f32>,
    shallow_color: vec4<f32>,
    model_x: vec4<f32>,
    model_y: vec4<f32>,
    model_z: vec4<f32>,
    model_w: vec4<f32>,
}
struct Particle {
    position: vec4<f32>,
    velocity: vec4<f32>,
    affine_x: vec4<f32>,
    affine_y: vec4<f32>,
}
@group(0) @binding(0) var<storage, read> waters: array<FlipWater>;
@group(0) @binding(1) var<storage, read> particles: array<Particle>;
struct Camera3D { view_proj: mat4x4<f32> }
@group(1) @binding(0) var<uniform> camera: Camera3D;

fn model(w: FlipWater) -> mat4x4<f32> {
    return mat4x4<f32>(w.model_x, w.model_y, w.model_z, w.model_w);
}
struct SplashOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
}
@vertex
fn vs_splash(@builtin(instance_index) idx: u32, @builtin(vertex_index) vertex: u32) -> SplashOut {
    let wi = u32(particles[idx].affine_x.w);
    let w = waters[wi];
    let p = particles[idx];
    let world = model(w) * vec4<f32>(p.position.xyz, 1.0);
    let center = camera.view_proj * world;
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let uv = corners[vertex];
    let visible = select(0.0, 1.0, p.velocity.w > 0.5);
    let radius = w.size_depth_cell.w * 0.9 * visible;
    var out: SplashOut;
    // World-space radius projected into clip space. Perspective division by
    // center.w supplies correct distance scaling.
    let projected_radius = radius * vec2<f32>(camera.view_proj[0][0], camera.view_proj[1][1]);
    out.clip = center + vec4<f32>(uv * projected_radius, 0.0, 0.0);
    out.color = mix(w.deep_color, w.shallow_color, 0.8);
    out.color.a *= visible;
    out.uv = uv;
    return out;
}
@fragment
fn fs_splash(in: SplashOut) -> @location(0) vec4<f32> {
    let edge = smoothstep(1.0, 0.55, length(in.uv));
    return vec4<f32>(in.color.rgb, in.color.a * edge);
}
