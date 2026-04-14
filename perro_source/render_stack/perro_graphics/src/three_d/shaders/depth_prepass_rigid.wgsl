struct Scene3D {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> scene: Scene3D;

struct VertexInput {
    @location(0) pos: vec3<f32>,
}

struct InstanceInput {
    @location(4) model_row_0: vec4<f32>,
    @location(5) model_row_1: vec4<f32>,
    @location(6) model_row_2: vec4<f32>,
}

@vertex
fn vs_main(v: VertexInput, inst: InstanceInput) -> @builtin(position) vec4<f32> {
    let p = vec4<f32>(v.pos, 1.0);
    let world = vec4<f32>(
        dot(inst.model_row_0, p),
        dot(inst.model_row_1, p),
        dot(inst.model_row_2, p),
        1.0,
    );
    return scene.view_proj * world;
}
