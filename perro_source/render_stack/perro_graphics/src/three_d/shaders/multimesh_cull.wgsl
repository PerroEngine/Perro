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

// Same layout as the rigid hi-z cull params (shared uniform buffer).
struct HizCullParams {
    view_proj: mat4x4<f32>,
    draw_count: u32,
    hiz_mip_count: u32,
    hiz_width: u32,
    hiz_height: u32,
    aspect: f32,
    proj_y_scale: f32,
    depth_bias: f32,
    _pad0: u32,
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
// Hi-z occlusion inputs (used only by cs_main_hiz).
@group(0) @binding(9)
var<uniform> hiz_params: HizCullParams;
@group(0) @binding(10)
var hiz_tex: texture_2d<f32>;

fn finite3(v: vec3<f32>) -> bool {
    return all(v == v) && all(abs(v) < vec3<f32>(1.0e30));
}

fn finite4(v: vec4<f32>) -> bool {
    return all(v == v) && all(abs(v) < vec4<f32>(1.0e30));
}

// World bounding sphere of one instance: center in xyz, radius in w.
fn instance_world_sphere(i: u32) -> vec4<f32> {
    let batch = batches[instance_batch[i]];
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
    return vec4<f32>(center, radius);
}

fn sphere_in_frustum(center: vec3<f32>, radius: f32) -> bool {
    if !finite3(center) {
        return false;
    }
    for (var pl = 0u; pl < 6u; pl = pl + 1u) {
        let plane = frustum.planes[pl];
        let d = dot(plane.xyz, center) + plane.w;
        if d < -radius {
            return false;
        }
    }
    return true;
}

fn append_visible(i: u32) {
    let batch_id = instance_batch[i];
    let batch = batches[batch_id];
    let slot = atomicAdd(&counters[batch_id], 1u);
    if slot < batch.instance_cap {
        visible_indices[batch.instance_start + slot] = i;
    }
}

@compute @workgroup_size(64u)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= params.instance_count {
        return;
    }
    let sphere = instance_world_sphere(i);
    if sphere_in_frustum(sphere.xyz, sphere.w) {
        append_visible(i);
    }
}

// Conservative hi-z occlusion test against the max-depth pyramid built from
// this frame's depth prepass. Mirrors hiz_occlusion_cull.wgsl.
fn sphere_hiz_occluded(center: vec3<f32>, radius_world: f32) -> bool {
    let clip = hiz_params.view_proj * vec4<f32>(center, 1.0);
    if !finite4(clip) || clip.w <= 1.0e-5 {
        return false;
    }
    let ndc = clip.xyz / clip.w;
    if ndc.x < -1.1 || ndc.x > 1.1 || ndc.y < -1.1 || ndc.y > 1.1 || ndc.z < -1.1 || ndc.z > 1.1 {
        return false;
    }

    // Approximate projected radius in pixels to select Hi-Z mip level.
    let radius_ndc_y = (radius_world * hiz_params.proj_y_scale) / max(abs(clip.w), 1.0e-4);
    let radius_px = max(radius_ndc_y * 0.5 * f32(hiz_params.hiz_height), 1.0);
    let diameter_px = radius_px * 2.0;
    let mip = min(
        u32(max(ceil(log2(diameter_px)), 0.0)),
        max(hiz_params.hiz_mip_count, 1u) - 1u,
    );

    let dims = textureDimensions(hiz_tex, i32(mip));
    // NDC y is up; texture coordinates are top-left origin.
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, -ndc.y * 0.5 + 0.5);
    let px_f = uv.x * f32(dims.x);
    let py_f = uv.y * f32(dims.y);
    let mip_scale = exp2(f32(mip));
    let radius_px_mip = max(radius_px / mip_scale, 1.0);
    let rx = i32(ceil(radius_px_mip));
    let ry = i32(ceil(radius_px_mip));

    let cx = clamp(i32(px_f), 0, i32(dims.x) - 1);
    let cy = clamp(i32(py_f), 0, i32(dims.y) - 1);
    let x0 = clamp(cx - rx, 0, i32(dims.x) - 1);
    let x1 = clamp(cx + rx, 0, i32(dims.x) - 1);
    let y0 = clamp(cy - ry, 0, i32(dims.y) - 1);
    let y1 = clamp(cy + ry, 0, i32(dims.y) - 1);

    // Conservative with max-depth Hi-Z: keep the maximum (farthest) depth
    // across the whole footprint, corners included.
    let d_center = textureLoad(hiz_tex, vec2<i32>(cx, cy), i32(mip)).x;
    let d_00 = textureLoad(hiz_tex, vec2<i32>(x0, y0), i32(mip)).x;
    let d_10 = textureLoad(hiz_tex, vec2<i32>(x1, y0), i32(mip)).x;
    let d_01 = textureLoad(hiz_tex, vec2<i32>(x0, y1), i32(mip)).x;
    let d_11 = textureLoad(hiz_tex, vec2<i32>(x1, y1), i32(mip)).x;
    let hiz_depth = max(max(max(d_center, d_00), max(d_10, d_01)), d_11);

    let center_depth = clamp(ndc.z, 0.0, 1.0);
    let nearest_depth = max(center_depth - radius_ndc_y * 0.5, 0.0);
    return nearest_depth > hiz_depth + hiz_params.depth_bias;
}

// Second cull phase: re-compacts with frustum + hi-z after the depth prepass
// (which drew the frustum-only survivors) built this frame's pyramid. The
// main pass then draws only unoccluded instances.
@compute @workgroup_size(64u)
fn cs_main_hiz(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= params.instance_count {
        return;
    }
    let sphere = instance_world_sphere(i);
    if !sphere_in_frustum(sphere.xyz, sphere.w) {
        return;
    }
    if sphere_hiz_occluded(sphere.xyz, sphere.w) {
        return;
    }
    append_visible(i);
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
