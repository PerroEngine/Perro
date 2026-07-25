struct Scene3D {
    view_proj: mat4x4<f32>,
    _vertex_modifier_padding: array<vec4<f32>, 66>,
    time_params: vec4<f32>,
    resolution: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> scene: Scene3D;
// Bone palettes upload only the 3 affine rows (the w row is never read: the
// skinned result is consumed as .xyz), cutting palette bandwidth by 25%.
struct SkeletonBone {
    row_0: vec4<f32>,
    row_1: vec4<f32>,
    row_2: vec4<f32>,
}

@group(0) @binding(1)
var<storage, read> skeletons: array<SkeletonBone>;
@group(0) @binding(2)
var<storage, read> custom_params_meta: array<u32>;
@group(0) @binding(3)
var<storage, read> custom_params_values: array<f32>;

// Weight-blend the 4 bone palettes into 3 affine rows (returned as the columns
// of a mat3x4 container).
fn blend_skin_rows(base: u32, joints: vec4<u32>, weights: vec4<f32>) -> mat3x4<f32> {
    let b0 = skeletons[base + joints.x];
    let b1 = skeletons[base + joints.y];
    let b2 = skeletons[base + joints.z];
    let b3 = skeletons[base + joints.w];
    return mat3x4<f32>(
        b0.row_0 * weights.x + b1.row_0 * weights.y + b2.row_0 * weights.z + b3.row_0 * weights.w,
        b0.row_1 * weights.x + b1.row_1 * weights.y + b2.row_1 * weights.z + b3.row_1 * weights.w,
        b0.row_2 * weights.x + b1.row_2 * weights.y + b2.row_2 * weights.z + b3.row_2 * weights.w,
    );
}
@group(0) @binding(4)
var<storage, read> blend_shape_deltas: array<BlendShapeDelta>;
@group(0) @binding(5)
var<storage, read> blend_shape_weights: array<f32>;
@group(0) @binding(6)
var<storage, read> blend_shape_instances: array<BlendShapeInstance>;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) @interpolate(flat) joints: vec4<u32>,
    @location(3) weights: vec4<f32>,
}

struct InstanceInput {
    @location(4) model_row_0: vec4<f32>,
    @location(5) model_row_1: vec4<f32>,
    @location(6) model_row_2: vec4<f32>,
    @location(7) @interpolate(flat) packed_color: u32,
    @location(8) @interpolate(flat) packed_material_params: u32,
    @location(11) @interpolate(flat) skeleton_params: vec4<u32>,
}

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) @interpolate(flat) packed_color: u32,
    @location(1) @interpolate(flat) packed_material_params: u32,
}

struct BlendShapeDelta {
    position_delta: vec4<f32>,
    normal_delta: vec4<f32>,
}

struct BlendShapeInstance {
    weight_range: vec4<u32>,
    shape_range: vec4<u32>,
}

fn unpack_unorm8(packed: u32, shift: u32) -> f32 {
    return f32((packed >> shift) & 0xffu) / 255.0;
}

fn apply_blend_shapes(v: VertexInput, vertex_index: u32, instance_index: u32) -> VertexInput {
    let blend_meta = blend_shape_instances[instance_index];
    let weight_count = min(blend_meta.weight_range.y, blend_meta.shape_range.y);
    if weight_count == 0u || blend_meta.shape_range.w == 0u || vertex_index < blend_meta.shape_range.z {
        return v;
    }
    let local_vertex = vertex_index - blend_meta.shape_range.z;
    if local_vertex >= blend_meta.shape_range.w {
        return v;
    }
    var pos = v.pos;
    var normal = v.normal.xyz;
    for (var i = 0u; i < weight_count; i = i + 1u) {
        let weight = clamp(blend_shape_weights[blend_meta.weight_range.x + i], 0.0, 1.0);
        let delta = blend_shape_deltas[blend_meta.shape_range.x + i * blend_meta.shape_range.w + local_vertex];
        pos = pos + delta.position_delta.xyz * weight;
        normal = normal + delta.normal_delta.xyz * weight;
    }
    return VertexInput(pos, vec4<f32>(normalize(normal), 0.0), v.joints, v.weights);
}

@vertex
fn vs_main(v: VertexInput, inst: InstanceInput, @builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_index: u32) -> VertexOutput {
    let blended = apply_blend_shapes(v, vertex_index, instance_index);
    let rows = blend_skin_rows(inst.skeleton_params.x, v.joints, v.weights);
    let p_skin = vec4<f32>(blended.pos, 1.0);
    let pos = vec3<f32>(dot(rows[0], p_skin), dot(rows[1], p_skin), dot(rows[2], p_skin));
    let normal = vec3<f32>(
        dot(rows[0].xyz, blended.normal.xyz),
        dot(rows[1].xyz, blended.normal.xyz),
        dot(rows[2].xyz, blended.normal.xyz),
    );
    let p = vec4<f32>(pos, 1.0);
    let world = vec4<f32>(
        dot(inst.model_row_0, p),
        dot(inst.model_row_1, p),
        dot(inst.model_row_2, p),
        1.0,
    );
    var out: VertexOutput;
    let normal_ws = perro_depth_transform_normal(
        inst.model_row_0.xyz,
        inst.model_row_1.xyz,
        inst.model_row_2.xyz,
        normal,
    );
    out.clip_pos = perro_depth_apply_vertex_modifiers(world.xyz, normal_ws, vec2<u32>(inst.skeleton_params.z, inst.skeleton_params.w));
    out.packed_color = inst.packed_color;
    out.packed_material_params = inst.packed_material_params;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) {
    let alpha_mode = in.packed_material_params & 0x3u;
    if alpha_mode == 1u {
        let alpha = unpack_unorm8(in.packed_color, 24u);
        let cutoff = unpack_unorm8(in.packed_material_params, 16u);
        if alpha < cutoff {
            discard;
        }
    }
}
