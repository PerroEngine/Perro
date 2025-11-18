struct Camera3D {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
};

@group(0) @binding(0)  // ← Change to group 0, binding 0
var<uniform> camera: Camera3D;

struct VSOut {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_main(@location(0) position: vec3<f32>) -> VSOut {
    var out: VSOut;
    let world = vec4<f32>(position, 1.0);
    out.pos = camera.projection * camera.view * world;
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.45, 0.0, 1.0); // reddish-orange
}