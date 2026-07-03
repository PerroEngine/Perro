// Per-instance frustum cull for dense multimeshes. One thread per instance.
// Visible instances are compacted (their source index appended) into a
// per-batch region of visible_indices identical to the batch's source range,
// and the atomic per-batch counter becomes the DrawIndexedIndirect
// instance_count. Regions never overlap, so no cross-batch races.

struct FrustumCullParams {
    planes: array<vec4<f32>, 6u>,
    draw_count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

struct MultiMeshDrawParam {
    model_row_0: vec4<f32>,
    model_row_1: vec4<f32>,
    model_row_2: vec4<f32>,
    packed_color: u32,
    packed_emissive: u32,
    scale_bits: u32,
    packed_blend_params: u32,
    custom_params: vec2<u32>,
    packed_bleed: u32,
    _pad: u32,
}

// Packed CPU layout (40 bytes). Rotation snorm16x4 stored as raw words; the
// cull only needs position + scale + draw_id, so rotation is not unpacked here.
struct MultiMeshInstance {
    px: f32,
    py: f32,
    pz: f32,
    rot_xy: u32,
    rot_zw: u32,
    sx: f32,
    sy: f32,
    sz: f32,
    draw_id: u32,
    blend_meta_id: u32,
}

// Per-batch static record. mesh_radius is the mesh local-bounds radius; the
// batch region in visible_indices is [instance_start, instance_start+cap).
struct MultiMeshCullBatch {
    instance_start: u32,
    instance_cap: u32,
    indirect_index: u32,
    mesh_radius_bits: u32,
}

struct DrawIndexedIndirect {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
}

struct MultiMeshCullParams {
    instance_count: u32,
    batch_count: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(0) @binding(0)
var<uniform> frustum: FrustumCullParams;
@group(0) @binding(1)
var<uniform> params: MultiMeshCullParams;
@group(0) @binding(2)
var<storage, read> draws: array<MultiMeshDrawParam>;
@group(0) @binding(3)
var<storage, read> instances: array<MultiMeshInstance>;
// Per-instance batch id so the thread can find its batch record + region.
@group(0) @binding(4)
var<storage, read> instance_batch: array<u32>;
@group(0) @binding(5)
var<storage, read> batches: array<MultiMeshCullBatch>;
@group(0) @binding(6)
var<storage, read_write> visible_indices: array<u32>;
@group(0) @binding(7)
var<storage, read_write> commands: array<DrawIndexedIndirect>;
// Per-batch atomic append counter, one per batch, cleared before dispatch.
@group(0) @binding(8)
var<storage, read_write> counters: array<atomic<u32>>;

fn finite3(v: vec3<f32>) -> bool {
    return all(v == v) && all(abs(v) < vec3<f32>(1.0e30));
}

@compute @workgroup_size(64u)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= params.instance_count {
        return;
    }
    let batch_id = instance_batch[i];
    let batch = batches[batch_id];
    let inst = instances[i];
    let draw = draws[inst.draw_id];

    let draw_scale = bitcast<f32>(draw.scale_bits);
    // Instance-local center at mesh origin; radius scaled by instance + draw
    // scale (conservative: max axis of the instance scale).
    let local_center = vec3<f32>(inst.px, inst.py, inst.pz);
    let p = vec4<f32>(local_center, 1.0);
    let center = vec3<f32>(
        dot(draw.model_row_0, p),
        dot(draw.model_row_1, p),
        dot(draw.model_row_2, p),
    );

    let mesh_radius = bitcast<f32>(batch.mesh_radius_bits);
    let inst_scale = max(max(inst.sx, inst.sy), inst.sz) * draw_scale;
    // Draw-model max column scale.
    let sx2 = dot(draw.model_row_0.xyz, draw.model_row_0.xyz);
    let sy2 = dot(draw.model_row_1.xyz, draw.model_row_1.xyz);
    let sz2 = dot(draw.model_row_2.xyz, draw.model_row_2.xyz);
    let model_scale = sqrt(max(max(sx2, sy2), max(sz2, 1.0e-12)));
    let radius = mesh_radius * max(inst_scale, 0.0) * model_scale;

    var visible = finite3(center);
    if visible {
        for (var pl = 0u; pl < 6u; pl = pl + 1u) {
            let plane = frustum.planes[pl];
            let d = dot(plane.xyz, center) + plane.w;
            if d < -radius {
                visible = false;
                break;
            }
        }
    }

    if visible {
        let slot = atomicAdd(&counters[batch_id], 1u);
        if slot < batch.instance_cap {
            visible_indices[batch.instance_start + slot] = i;
        }
    }
}

// One thread per batch: copy the append counter into the indirect record.
// Runs after cs_main so every append is counted. Clamped to the region cap.
@compute @workgroup_size(64u)
fn cs_finalize(@builtin(global_invocation_id) gid: vec3<u32>) {
    let b = gid.x;
    if b >= params.batch_count {
        return;
    }
    let batch = batches[b];
    let count = min(atomicLoad(&counters[b]), batch.instance_cap);
    commands[batch.indirect_index].instance_count = count;
}
