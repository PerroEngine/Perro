struct FrustumCullParams {
    planes: array<vec4<f32>, 6u>,
    draw_count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

struct CullStatic {
    local_center_radius: vec4<f32>,
    cull_flags: vec4<u32>,
}

struct CullDynamic {
    model_0: vec4<f32>,
    model_1: vec4<f32>,
    model_2: vec4<f32>,
    model_3: vec4<f32>,
}

struct DrawIndexedIndirect {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
}

@group(0) @binding(0)
var<uniform> params: FrustumCullParams;
@group(0) @binding(1)
var<storage, read> cull_static: array<CullStatic>;
@group(0) @binding(2)
var<storage, read> cull_dynamic: array<CullDynamic>;
@group(0) @binding(3)
var<storage, read_write> commands: array<DrawIndexedIndirect>;

fn finite4(v: vec4<f32>) -> bool {
    return all(v == v) && all(abs(v) < vec4<f32>(1.0e30));
}

@compute @workgroup_size(64u)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= params.draw_count {
        return;
    }

    let dyn = cull_dynamic[i];
    let stat = cull_static[i];
    let model = mat4x4<f32>(dyn.model_0, dyn.model_1, dyn.model_2, dyn.model_3);
    let center_local = vec4<f32>(stat.local_center_radius.xyz, 1.0);
    let center_world = model * center_local;

    if !finite4(center_world) {
        commands[i].instance_count = 0u;
        return;
    }

    let sx2 = dot(dyn.model_0.xyz, dyn.model_0.xyz);
    let sy2 = dot(dyn.model_1.xyz, dyn.model_1.xyz);
    let sz2 = dot(dyn.model_2.xyz, dyn.model_2.xyz);
    let scale = sqrt(max(max(sx2, sy2), max(sz2, 1.0e-12)));
    let radius_world = max(stat.local_center_radius.w, 0.0) * scale;
    let center = center_world.xyz;

    var visible = true;
    for (var p = 0u; p < 6u; p = p + 1u) {
        let plane = params.planes[p];
        let d = dot(plane.xyz, center) + plane.w;
        if d < -radius_world {
            visible = false;
            break;
        }
    }

    // Do not rely on previous-frame command contents. If visible this frame,
    // force at least one instance so culled draws can become visible again.
    let instances = max(commands[i].instance_count, 1u);
    commands[i].instance_count = select(0u, instances, visible);
}
