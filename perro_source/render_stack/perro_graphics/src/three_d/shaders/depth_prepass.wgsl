struct Scene3D {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> scene: Scene3D;

struct VertexInput {
    @location(0) pos: vec3<f32>,
}

struct InstanceInput {
    @location(2) model_0: vec4<f32>,
    @location(3) model_1: vec4<f32>,
    @location(4) model_2: vec4<f32>,
    @location(5) model_3: vec4<f32>,
}

@vertex
fn vs_main(v: VertexInput, inst: InstanceInput) -> @builtin(position) vec4<f32> {
    let model = mat4x4<f32>(inst.model_0, inst.model_1, inst.model_2, inst.model_3);
    return scene.view_proj * (model * vec4<f32>(v.pos, 1.0));
}
