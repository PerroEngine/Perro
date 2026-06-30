const MAX_RAY_LIGHTS: u32 = 3u;
const MAX_POINT_LIGHTS: u32 = 8u;
const MAX_SPOT_LIGHTS: u32 = 8u;

struct RayLightGpu {
    direction: vec4<f32>,
    color_intensity: vec4<f32>,
}

struct PointLightGpu {
    position_range: vec4<f32>,
    color_intensity: vec4<f32>,
}

struct SpotLightGpu {
    position_range: vec4<f32>,
    direction_outer_cos: vec4<f32>,
    color_intensity: vec4<f32>,
    inner_cos_pad: vec4<f32>,
}

struct Scene3D {
    view_proj: mat4x4<f32>,
    ambient_and_counts: vec4<f32>,
    camera_pos: vec4<f32>,
    ambient_color: vec4<f32>,
    ray_light: RayLightGpu,
    ray_lights: array<RayLightGpu, MAX_RAY_LIGHTS>,
    point_lights: array<PointLightGpu, MAX_POINT_LIGHTS>,
    spot_lights: array<SpotLightGpu, MAX_SPOT_LIGHTS>,
    inv_view_proj: mat4x4<f32>,
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
}

@group(0) @binding(0)
var<uniform> scene: Scene3D;
@group(0) @binding(1)
var<storage, read> multimesh_draws: array<MultiMeshDrawParam>;
@group(0) @binding(2)
var mesh_blend_depth_tex: texture_depth_2d;
@group(0) @binding(3)
var<storage, read> blend_shape_deltas: array<BlendShapeDelta>;
@group(0) @binding(4)
var<storage, read> blend_shape_weights: array<f32>;
@group(0) @binding(5)
var<storage, read> blend_shape_instances: array<BlendShapeInstance>;
@group(0) @binding(6)
var<storage, read> custom_params_meta: array<u32>;
@group(0) @binding(7)
var<storage, read> custom_params_values: array<f32>;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec4<f32>,
};

struct InstanceInput {
    @location(4) position: vec3<f32>,
    @location(5) rotation: vec4<f32>,
    @location(6) scale: vec3<f32>,
    @location(7) draw_id: u32,
    @location(8) blend_meta_id: u32,
};

struct BlendShapeDelta {
    position_delta: vec4<f32>,
    normal_delta: vec4<f32>,
};

struct BlendShapeInstance {
    weight_range: vec4<u32>,
    shape_range: vec4<u32>,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) lit_color: vec3<f32>,
    @location(1) @interpolate(flat) packed_blend_params: u32,
    @location(2) world_pos: vec3<f32>,
    @location(3) normal_ws: vec3<f32>,
    @location(4) @interpolate(flat) custom_range: vec2<u32>,
    @location(5) uv: vec2<f32>,
    @location(6) frag_pos: vec4<f32>,
};

struct FragmentInput {
    @location(0) lit_color: vec3<f32>,
    @location(1) @interpolate(flat) packed_blend_params: u32,
    @location(2) world_pos: vec3<f32>,
    @location(3) normal_ws: vec3<f32>,
    @location(4) @interpolate(flat) custom_range: vec2<u32>,
    @location(5) uv: vec2<f32>,
    @location(6) frag_pos: vec4<f32>,
};

fn unpack_rgba8(v: u32) -> vec4<f32> {
    let r = f32(v & 255u) * (1.0 / 255.0);
    let g = f32((v >> 8u) & 255u) * (1.0 / 255.0);
    let b = f32((v >> 16u) & 255u) * (1.0 / 255.0);
    let a = f32((v >> 24u) & 255u) * (1.0 / 255.0);
    return vec4<f32>(r, g, b, a);
}

fn unpack_byte(packed: u32, shift: u32) -> u32 {
    return (packed >> shift) & 0xffu;
}

fn unpack_unorm8(packed: u32, shift: u32) -> f32 {
    return f32(unpack_byte(packed, shift)) * (1.0 / 255.0);
}

fn decode_mesh_blend_params(packed: u32) -> vec4<f32> {
    return vec4<f32>(
        unpack_unorm8(packed, 0u) * 16.0,
        unpack_unorm8(packed, 8u) * 16.0,
        unpack_unorm8(packed, 16u),
        unpack_unorm8(packed, 24u) * 64.0,
    );
}

fn mesh_blend_hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

fn mesh_blend_noise(p: vec2<f32>) -> f32 {
    let cell = floor(p);
    let local = fract(p);
    let curve = local * local * (3.0 - 2.0 * local);
    let a = mesh_blend_hash(cell);
    let b = mesh_blend_hash(cell + vec2<f32>(1.0, 0.0));
    let c = mesh_blend_hash(cell + vec2<f32>(0.0, 1.0));
    let d = mesh_blend_hash(cell + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, curve.x), mix(c, d, curve.x), curve.y);
}

fn mesh_blend_world_from_depth(coord: vec2<i32>, dims_u: vec2<u32>, depth: f32) -> vec3<f32> {
    let uv = (vec2<f32>(coord) + vec2<f32>(0.5)) / vec2<f32>(dims_u);
    let ndc_xy = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let ndc = vec4<f32>(ndc_xy, depth, 1.0);
    let world_h = scene.inv_view_proj * ndc;
    return world_h.xyz / max(abs(world_h.w), 1.0e-5);
}

fn mesh_blend_alpha(frag_pos: vec4<f32>, world_pos: vec3<f32>, packed: u32) -> f32 {
    if packed == 0u {
        return 1.0;
    }
    let dims_u = textureDimensions(mesh_blend_depth_tex);
    let dims = vec2<i32>(i32(dims_u.x), i32(dims_u.y));
    let coord = vec2<i32>(floor(frag_pos.xy));
    if any(coord < vec2<i32>(0)) || any(coord >= dims) {
        return 1.0;
    }
    let scene_depth = textureLoad(mesh_blend_depth_tex, coord, 0);
    if scene_depth >= 0.999999 {
        return 1.0;
    }
    let params = decode_mesh_blend_params(packed);
    let view_dist = distance(world_pos, scene.camera_pos.xyz);
    let scene_world = mesh_blend_world_from_depth(coord, dims_u, scene_depth);
    let raw_depth_delta = distance(scene_world, scene.camera_pos.xyz) - view_dist;
    if raw_depth_delta <= 0.0 {
        return 1.0;
    }
    let max_width = max(params.x, 0.0001);
    let min_width = min(params.y, max_width);
    var noise = 0.0;
    if params.z > 0.0 {
        let tile = max(params.w, 1.0);
        let soft_noise = smoothstep(0.15, 0.85, mesh_blend_noise(frag_pos.xy / tile));
        noise = (soft_noise - 0.5) * params.z * max_width;
    }
    let depth_delta = max(raw_depth_delta + noise, 0.0);
    if depth_delta > max_width * 1.15 {
        return 1.0;
    }
    let fade = smoothstep(min_width, max_width, depth_delta);
    return fade * fade * (3.0 - 2.0 * fade);
}

fn rotate_vec_by_quat(v: vec3<f32>, q: vec4<f32>) -> vec3<f32> {
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

fn transform_normal_ws(
    row_0: vec3<f32>,
    row_1: vec3<f32>,
    row_2: vec3<f32>,
    normal: vec3<f32>,
) -> vec3<f32> {
    let cof_0 = cross(row_1, row_2);
    let cof_1 = cross(row_2, row_0);
    let cof_2 = cross(row_0, row_1);
    let det = dot(row_0, cof_0);
    if abs(det) <= 1.0e-8 {
        return normalize(vec3<f32>(
            dot(row_0, normal),
            dot(row_1, normal),
            dot(row_2, normal),
        ));
    }
    let det_sign = select(-1.0, 1.0, det >= 0.0);
    return normalize(vec3<f32>(
        dot(cof_0, normal),
        dot(cof_1, normal),
        dot(cof_2, normal),
    ) * det_sign);
}

fn apply_blend_shapes(v: VertexInput, inst: InstanceInput, vertex_index: u32) -> VertexInput {
    let blend_meta = blend_shape_instances[inst.blend_meta_id];
    let weight_count = min(blend_meta.weight_range.y, blend_meta.shape_range.y);
    if weight_count == 0u || blend_meta.shape_range.w == 0u || vertex_index < blend_meta.shape_range.z {
        return v;
    }
    let local_vertex = vertex_index - blend_meta.shape_range.z;
    if local_vertex >= blend_meta.shape_range.w {
        return v;
    }
    var out_pos = v.pos;
    var out_normal = v.normal.xyz;
    for (var i = 0u; i < weight_count; i = i + 1u) {
        let weight = clamp(blend_shape_weights[blend_meta.weight_range.x + i], 0.0, 1.0);
        let delta = blend_shape_deltas[blend_meta.shape_range.x + i * blend_meta.shape_range.w + local_vertex];
        out_pos = out_pos + delta.position_delta.xyz * weight;
        out_normal = out_normal + delta.normal_delta.xyz * weight;
    }
    return VertexInput(out_pos, vec4<f32>(normalize(out_normal), 0.0));
}

fn custom_f_param(in: FragmentInput, index: u32) -> vec4<f32> {
    if index >= in.custom_range.y {
        return vec4<f32>(0.0);
    }
    let packed_meta = custom_params_meta[in.custom_range.x + index];
    let kind = packed_meta & 0x3u;
    let value_offset = packed_meta >> 2u;
    if kind == 0u {
        return vec4<f32>(custom_params_values[value_offset], 0.0, 0.0, 0.0);
    }
    if kind == 1u {
        return vec4<f32>(
            custom_params_values[value_offset],
            custom_params_values[value_offset + 1u],
            0.0,
            0.0,
        );
    }
    if kind == 2u {
        return vec4<f32>(
            custom_params_values[value_offset],
            custom_params_values[value_offset + 1u],
            custom_params_values[value_offset + 2u],
            0.0,
        );
    }
    return vec4<f32>(
        custom_params_values[value_offset],
        custom_params_values[value_offset + 1u],
        custom_params_values[value_offset + 2u],
        custom_params_values[value_offset + 3u],
    );
}

fn custom_v_param(out: VertexOutput, index: u32) -> vec4<f32> {
    if index >= out.custom_range.y {
        return vec4<f32>(0.0);
    }
    let packed_meta = custom_params_meta[out.custom_range.x + index];
    let kind = packed_meta & 0x3u;
    let value_offset = packed_meta >> 2u;
    if kind == 0u {
        return vec4<f32>(custom_params_values[value_offset], 0.0, 0.0, 0.0);
    }
    if kind == 1u {
        return vec4<f32>(
            custom_params_values[value_offset],
            custom_params_values[value_offset + 1u],
            0.0,
            0.0,
        );
    }
    if kind == 2u {
        return vec4<f32>(
            custom_params_values[value_offset],
            custom_params_values[value_offset + 1u],
            custom_params_values[value_offset + 2u],
            0.0,
        );
    }
    return vec4<f32>(
        custom_params_values[value_offset],
        custom_params_values[value_offset + 1u],
        custom_params_values[value_offset + 2u],
        custom_params_values[value_offset + 3u],
    );
}

fn custom_param(in: FragmentInput, index: u32) -> vec4<f32> {
    return custom_f_param(in, index);
}

fn custom_param_vertex(out: VertexOutput, index: u32) -> vec4<f32> {
    return custom_v_param(out, index);
}

fn perro_lit_standard(
    in: FragmentInput,
    base: vec4<f32>,
    roughness: f32,
    metallic: f32,
    occlusion: f32,
    emissive: vec3<f32>,
) -> vec4<f32> {
    let _roughness = roughness;
    let _metallic = metallic;
    let n = normalize(in.normal_ws);
    let ambient = scene.ambient_color.xyz * scene.ambient_color.w * occlusion;
    var lit = ambient;
    let ray_count = u32(scene.ambient_and_counts.x);
    if ray_count > 0u {
        let ray = scene.ray_lights[0];
        let ray_dir = ray.direction.xyz;
        let l = -ray_dir * inverseSqrt(max(dot(ray_dir, ray_dir), 1.0e-8));
        let lambert = max(dot(n, l), 0.0);
        lit += ray.color_intensity.xyz * ray.color_intensity.w * lambert;
    }
    let alpha = mesh_blend_alpha(in.frag_pos, in.world_pos, in.packed_blend_params) * base.a;
    return vec4<f32>(base.rgb * lit + emissive, alpha);
}

fn perro_multimesh_vs_main_base(v: VertexInput, inst: InstanceInput, vertex_index: u32) -> VertexOutput {
    let draw = multimesh_draws[inst.draw_id];
    let scale = bitcast<f32>(draw.scale_bits);
    let rot = normalize(inst.rotation);
    let blended = apply_blend_shapes(v, inst, vertex_index);
    let local_pos = rotate_vec_by_quat(blended.pos * (inst.scale * scale), rot) + inst.position;
    let local_nrm = rotate_vec_by_quat(blended.normal.xyz, rot);
    let p = vec4<f32>(local_pos, 1.0);
    let world = vec4<f32>(
        dot(draw.model_row_0, p),
        dot(draw.model_row_1, p),
        dot(draw.model_row_2, p),
        1.0,
    );
    let normal_ws = transform_normal_ws(
        draw.model_row_0.xyz,
        draw.model_row_1.xyz,
        draw.model_row_2.xyz,
        local_nrm,
    );

    let base = unpack_rgba8(draw.packed_color);
    let emissive = unpack_rgba8(draw.packed_emissive);
    let n = normal_ws;
    let ambient = scene.ambient_color.xyz * scene.ambient_color.w;
    var lit = ambient;
    let ray_count = u32(scene.ambient_and_counts.x);
    if ray_count > 0u {
        let ray = scene.ray_lights[0];
        let ray_dir = ray.direction.xyz;
        let l = -ray_dir * inverseSqrt(max(dot(ray_dir, ray_dir), 1.0e-8));
        let lambert = max(dot(n, l), 0.0);
        lit += ray.color_intensity.xyz * ray.color_intensity.w * lambert;
    }
    var out: VertexOutput;
    out.clip_pos = scene.view_proj * world;
    out.lit_color = base.rgb * lit + emissive.rgb;
    out.packed_blend_params = draw.packed_blend_params;
    out.world_pos = world.xyz;
    out.normal_ws = normal_ws;
    out.custom_range = draw.custom_params;
    out.uv = vec2<f32>(0.0);
    out.frag_pos = out.clip_pos;
    return out;
}

@vertex
fn vs_main(v: VertexInput, inst: InstanceInput, @builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    return perro_multimesh_vs_main_base(v, inst, vertex_index);
}

@fragment
fn fs_main(in: FragmentInput, @builtin(position) frag_pos: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(in.lit_color, mesh_blend_alpha(frag_pos, in.world_pos, in.packed_blend_params));
}
