struct Scene3D {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> scene: Scene3D;
@group(0) @binding(1)
var<storage, read> skeletons: array<mat4x4<f32>>;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(2) joints: vec4<u32>,
    @location(3) weights: vec4<f32>,
}

struct InstanceInput {
    @location(4) model_row_0: vec4<f32>,
    @location(5) model_row_1: vec4<f32>,
    @location(6) model_row_2: vec4<f32>,
    @location(11) skeleton_params: vec4<u32>,
}

@vertex
fn vs_main(v: VertexInput, inst: InstanceInput) -> @builtin(position) vec4<f32> {
    var pos = v.pos;
    if inst.skeleton_params.y > 0u {
        let base = inst.skeleton_params.x;
        let m0 = skeletons[base + v.joints.x] * v.weights.x;
        let m1 = skeletons[base + v.joints.y] * v.weights.y;
        let m2 = skeletons[base + v.joints.z] * v.weights.z;
        let m3 = skeletons[base + v.joints.w] * v.weights.w;
        let skin = m0 + m1 + m2 + m3;
        pos = (skin * vec4<f32>(pos, 1.0)).xyz;
    }
    let p = vec4<f32>(pos, 1.0);
    let world = vec4<f32>(
        dot(inst.model_row_0, p),
        dot(inst.model_row_1, p),
        dot(inst.model_row_2, p),
        1.0,
    );
    return scene.view_proj * world;
}
