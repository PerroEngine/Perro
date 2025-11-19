struct Camera3D {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera3D;

struct VSIn {
    @location(0) position: vec3<f32>,
    @location(1) model_row0: vec4<f32>,
    @location(2) model_row1: vec4<f32>,
    @location(3) model_row2: vec4<f32>,
    @location(4) model_row3: vec4<f32>,
    @location(5) material_id: u32, // optional
};

struct VSOut {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_main(in: VSIn) -> VSOut {
    var out: VSOut;

    let model = mat4x4<f32>(
        in.model_row0,
        in.model_row1,
        in.model_row2,
        in.model_row3
    );

    let world_pos = model * vec4<f32>(in.position, 1.0);

    // Apply view-projection transform
    out.pos = camera.projection * camera.view * world_pos;
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4(1.0, 0.45, 0.0, 1.0);
}