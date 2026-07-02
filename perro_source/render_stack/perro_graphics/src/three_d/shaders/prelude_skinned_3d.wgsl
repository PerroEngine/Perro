const MAX_RAY_LIGHTS: u32 = 3u;
const MAX_POINT_LIGHTS: u32 = 8u;
const MAX_SPOT_LIGHTS: u32 = 8u;
const MAX_SHADOW_RAY_CASCADES: u32 = 4u;
const MAX_SHADOW_SPOT_LIGHTS: u32 = 4u;
const MAX_SHADOW_POINT_LIGHTS: u32 = 4u;
const POINT_SHADOW_FACE_COUNT: u32 = 6u;
const INV_255: f32 = 1.0 / 255.0;

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
    // Kept for compatibility with custom shaders that still read scene.ray_light.
    ray_light: RayLightGpu,
    ray_lights: array<RayLightGpu, MAX_RAY_LIGHTS>,
    point_lights: array<PointLightGpu, MAX_POINT_LIGHTS>,
    spot_lights: array<SpotLightGpu, MAX_SPOT_LIGHTS>,
    inv_view_proj: mat4x4<f32>,
}

struct Shadow3D {
    ray_light_view_proj: array<mat4x4<f32>, 4>,
    spot_light_view_proj: array<mat4x4<f32>, 4>,
    point_light_view_proj: array<mat4x4<f32>, 24>,
    params0: vec4<f32>, // enabled, strength, depth_bias, normal_bias
    ray_params: vec4<f32>,
    ray_splits: vec4<f32>,
    ray_texel: vec4<f32>, // world units per shadow texel, per cascade
    spot_params: array<vec4<f32>, 4>,
    point_params: array<vec4<f32>, 4>,
}

struct DecodedMaterialParams {
    alpha_mode: u32,
    alpha_cutoff: f32,
    double_sided: bool,
    material_flags: u32,
    meshlet_debug_view: bool,
    flat_shading: bool,
    has_base_color_texture: bool,
    mesh_blend: bool,
    normal_blend: bool,
    mirrored_winding: bool,
    receive_shadows: bool,
}

@group(0) @binding(0)
var<uniform> scene: Scene3D;
@group(0) @binding(1)
var<storage, read> skeletons: array<mat4x4<f32>>;
@group(0) @binding(2)
var<storage, read> custom_params_meta: array<u32>;
@group(0) @binding(3)
var<storage, read> custom_params_values: array<f32>;
@group(0) @binding(4)
var<storage, read> blend_shape_deltas: array<BlendShapeDelta>;
@group(0) @binding(5)
var<storage, read> blend_shape_weights: array<f32>;
@group(0) @binding(6)
var<storage, read> blend_shape_instances: array<BlendShapeInstance>;
@group(1) @binding(0)
var material_sampler: sampler;
@group(1) @binding(1)
var material_base_color_tex: texture_2d<f32>;
@group(2) @binding(0)
var<uniform> shadow: Shadow3D;
@group(2) @binding(1)
var shadow_map_tex: texture_depth_2d_array;
@group(2) @binding(2)
var shadow_map_sampler: sampler_comparison;
@group(2) @binding(3)
var spot_shadow_map_tex: texture_depth_2d_array;
@group(2) @binding(4)
var point_shadow_map_tex: texture_depth_2d_array;
@group(3) @binding(0)
var mesh_blend_depth_tex: texture_depth_2d;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) joints: vec4<u32>,
    @location(3) weights: vec4<f32>,
    @location(12) uv: vec2<f32>,
};

struct InstanceInput {
    @location(4) model_row_0: vec4<f32>,
    @location(5) model_row_1: vec4<f32>,
    @location(6) model_row_2: vec4<f32>,
    @location(7) packed_color: u32,
    @location(8) packed_pbr_params_0: u32,
    @location(9) packed_pbr_params_1: u32,
    @location(10) packed_emissive: u32,
    @location(11) packed_material_params: u32,
    @location(13) skeleton_params: vec4<u32>,
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
    @location(0) world_pos: vec3<f32>,
    @location(1) normal_ws: vec3<f32>,
    @location(2) @interpolate(flat) packed_color: u32,
    @location(3) @interpolate(flat) packed_pbr_params_0: u32,
    @location(4) @interpolate(flat) packed_pbr_params_1: u32,
    @location(5) @interpolate(flat) packed_emissive: u32,
    @location(6) @interpolate(flat) packed_material_params: u32,
    @location(7) @interpolate(flat) custom_range: vec2<u32>,
    @location(8) uv: vec2<f32>,
};

struct FragmentInput {
    @builtin(position) frag_pos: vec4<f32>,
    @builtin(front_facing) is_front: bool,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal_ws: vec3<f32>,
    @location(2) @interpolate(flat) packed_color: u32,
    @location(3) @interpolate(flat) packed_pbr_params_0: u32,
    @location(4) @interpolate(flat) packed_pbr_params_1: u32,
    @location(5) @interpolate(flat) packed_emissive: u32,
    @location(6) @interpolate(flat) packed_material_params: u32,
    @location(7) @interpolate(flat) custom_range: vec2<u32>,
    @location(8) uv: vec2<f32>,
};

fn unpack_byte(packed: u32, shift: u32) -> u32 {
    return (packed >> shift) & 0xffu;
}

fn unpack_unorm8(packed: u32, shift: u32) -> f32 {
    return f32(unpack_byte(packed, shift)) * INV_255;
}

fn unpack_rgba8(packed: u32) -> vec4<f32> {
    return vec4<f32>(
        unpack_unorm8(packed, 0u),
        unpack_unorm8(packed, 8u),
        unpack_unorm8(packed, 16u),
        unpack_unorm8(packed, 24u),
    );
}

// rgb lanes hold the normalized color, w holds max-component / 16 (see
// pack_emissive_hdr on the CPU side).
fn unpack_emissive_hdr(packed: u32) -> vec3<f32> {
    let e = unpack_rgba8(packed);
    return e.xyz * (e.w * 16.0);
}

fn decode_material_params(packed: u32) -> DecodedMaterialParams {
    let flags = (packed >> 3u) & 0x1fffu;
    return DecodedMaterialParams(
        packed & 0x3u,
        unpack_unorm8(packed, 16u),
        ((packed >> 2u) & 0x1u) != 0u,
        flags,
        (flags & 0x1u) != 0u,
        (flags & 0x2u) != 0u,
        (flags & 0x4u) != 0u,
        (flags & 0x8u) != 0u,
        (flags & 0x10u) != 0u,
        (flags & 0x20u) != 0u,
        (flags & 0x40u) != 0u,
    );
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

fn mesh_blend_fade(in: FragmentInput, material: DecodedMaterialParams) -> f32 {
    if !material.mesh_blend {
        return 1.0;
    }
    let dims_u = textureDimensions(mesh_blend_depth_tex);
    let dims = vec2<i32>(i32(dims_u.x), i32(dims_u.y));
    let coord = vec2<i32>(floor(in.frag_pos.xy));
    if any(coord < vec2<i32>(0)) || any(coord >= dims) {
        return 1.0;
    }
    let scene_depth = textureLoad(mesh_blend_depth_tex, coord, 0);
    if scene_depth >= 0.999999 {
        return 1.0;
    }
    let params = decode_mesh_blend_params(in.packed_pbr_params_1);
    let view_dist = distance(in.world_pos, scene.camera_pos.xyz);
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
        let soft_noise = smoothstep(0.15, 0.85, mesh_blend_noise(in.frag_pos.xy / tile));
        noise = (soft_noise - 0.5) * params.z * max_width;
    }
    let depth_delta = max(raw_depth_delta + noise, 0.0);
    if depth_delta > max_width * 1.15 {
        return 1.0;
    }
    let fade = smoothstep(min_width, max_width, depth_delta);
    return fade * fade * (3.0 - 2.0 * fade);
}

fn apply_mesh_blend_alpha(in: FragmentInput, material: DecodedMaterialParams, alpha: f32) -> f32 {
    return alpha * mesh_blend_fade(in, material);
}

fn perro_material_alpha(in: FragmentInput, alpha: f32) -> f32 {
    let material = decode_material_params(in.packed_material_params);
    var out_alpha = clamp(alpha, 0.0, 1.0);
    if material.alpha_mode == 1u && out_alpha < material.alpha_cutoff {
        discard;
    }
    if material.alpha_mode == 0u {
        out_alpha = 1.0;
    }
    return apply_mesh_blend_alpha(in, material, out_alpha);
}

fn apply_mesh_normal_blend(
    material: DecodedMaterialParams,
    normal_ws: vec3<f32>,
    world_pos: vec3<f32>,
    mesh_blend_fade_value: f32,
) -> vec3<f32> {
    if !material.normal_blend {
        return normal_ws;
    }
    let contact = 1.0 - mesh_blend_fade_value;
    if contact <= 0.0001 {
        return normal_ws;
    }
    let proxy_raw = cross(dpdx(world_pos), dpdy(world_pos));
    let proxy_len_sq = dot(proxy_raw, proxy_raw);
    if proxy_len_sq <= 1.0e-8 {
        return normal_ws;
    }
    var proxy = proxy_raw * inverseSqrt(proxy_len_sq);
    if dot(proxy, normal_ws) < 0.0 {
        proxy = -proxy;
    }
    let softened = normalize(normal_ws + proxy);
    return normalize(mix(normal_ws, softened, clamp(contact * 0.35, 0.0, 0.35)));
}

fn decode_standard_pbr_params(packed_0: u32, packed_1: u32) -> vec4<f32> {
    let _future = packed_1;
    return vec4<f32>(
        unpack_unorm8(packed_0, 0u),
        unpack_unorm8(packed_0, 8u),
        unpack_unorm8(packed_0, 16u),
        unpack_unorm8(packed_0, 24u) * 4.0,
    );
}

fn decode_toon_params(packed_0: u32, packed_1: u32) -> vec3<f32> {
    let _future = packed_1;
    return vec3<f32>(
        max(1.0, f32(unpack_byte(packed_0, 0u))),
        unpack_unorm8(packed_0, 8u) * 4.0,
        unpack_unorm8(packed_0, 16u) * 4.0,
    );
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
    var out_pos = v.pos;
    var out_normal = v.normal.xyz;
    for (var i = 0u; i < weight_count; i = i + 1u) {
        let weight = clamp(blend_shape_weights[blend_meta.weight_range.x + i], 0.0, 1.0);
        let delta = blend_shape_deltas[blend_meta.shape_range.x + i * blend_meta.shape_range.w + local_vertex];
        out_pos = out_pos + delta.position_delta.xyz * weight;
        out_normal = out_normal + delta.normal_delta.xyz * weight;
    }
    return VertexInput(out_pos, vec4<f32>(normalize(out_normal), 0.0), v.joints, v.weights, v.uv);
}

fn perro_vs_main_base(v: VertexInput, inst: InstanceInput, vertex_index: u32, instance_index: u32) -> VertexOutput {
    let blended = apply_blend_shapes(v, vertex_index, instance_index);
    let base = inst.skeleton_params.x;
    let m0 = skeletons[base + blended.joints.x] * blended.weights.x;
    let m1 = skeletons[base + blended.joints.y] * blended.weights.y;
    let m2 = skeletons[base + blended.joints.z] * blended.weights.z;
    let m3 = skeletons[base + blended.joints.w] * blended.weights.w;
    let skin = m0 + m1 + m2 + m3;
    let pos = (skin * vec4<f32>(blended.pos, 1.0)).xyz;
    let normal = (skin * vec4<f32>(blended.normal.xyz, 0.0)).xyz;
    let p = vec4<f32>(pos, 1.0);
    let world = vec4<f32>(
        dot(inst.model_row_0, p),
        dot(inst.model_row_1, p),
        dot(inst.model_row_2, p),
        1.0,
    );
    let normal_ws = transform_normal_ws(
        inst.model_row_0.xyz,
        inst.model_row_1.xyz,
        inst.model_row_2.xyz,
        normal,
    );

    var out: VertexOutput;
    out.clip_pos = scene.view_proj * world;
    out.world_pos = world.xyz;
    out.normal_ws = normal_ws;
    out.packed_color = inst.packed_color;
    out.packed_pbr_params_0 = inst.packed_pbr_params_0;
    out.packed_pbr_params_1 = inst.packed_pbr_params_1;
    out.packed_emissive = inst.packed_emissive;
    out.packed_material_params = inst.packed_material_params;
    out.custom_range = vec2<u32>(inst.skeleton_params.z, inst.skeleton_params.w);
    out.uv = blended.uv;
    return out;
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

fn shadow_factor(world_pos: vec3<f32>, normal_ws: vec3<f32>, light_dir_to_light: vec3<f32>) -> f32 {
    return ray_shadow_factor(world_pos, normal_ws, light_dir_to_light);
}

// Returns -1.0 when the position falls outside this cascade's map so the
// caller can fall through to the next cascade.
fn sample_ray_shadow_array(light_view_proj: mat4x4<f32>, world_pos: vec3<f32>, normal_ws: vec3<f32>, bias_dir: vec3<f32>, layer: u32) -> f32 {
    let texel_world = max(shadow.ray_texel[layer], 1.0e-4);
    let normal_offset = max(texel_world * 1.75, shadow.params0.w * 0.25);
    let sample_pos = world_pos + normalize(normal_ws) * normal_offset + normalize(bias_dir) * normal_offset * 0.25;
    let light_clip = light_view_proj * vec4<f32>(sample_pos, 1.0);
    if abs(light_clip.w) <= 1.0e-6 {
        return -1.0;
    }
    let ndc = light_clip.xyz / light_clip.w;
    let uv = vec2<f32>(ndc.x, -ndc.y) * 0.5 + vec2<f32>(0.5);
    let depth = ndc.z;
    let bias = shadow.params0.z;
    let dims = max(vec2<f32>(textureDimensions(shadow_map_tex)), vec2<f32>(1.0));
    let texel = 1.0 / dims;
    if depth <= 0.0 || depth >= 1.0
        || any(uv < texel)
        || any(uv > (vec2<f32>(1.0) - texel)) {
        return -1.0;
    }
    var sum = 0.0;
    let layer_i = i32(layer);
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            sum += textureSampleCompare(shadow_map_tex, shadow_map_sampler, uv + vec2<f32>(f32(x), f32(y)) * texel, layer_i, depth - bias);
        }
    }
    return sum / 9.0;
}

fn sample_shadow_array(light_view_proj: mat4x4<f32>, world_pos: vec3<f32>, normal_ws: vec3<f32>, bias_dir: vec3<f32>, layer: u32, is_point: bool) -> f32 {
    let sample_pos = world_pos + normalize(normal_ws) * shadow.params0.w + normalize(bias_dir) * shadow.params0.w * 0.25;
    let light_clip = light_view_proj * vec4<f32>(sample_pos, 1.0);
    if abs(light_clip.w) <= 1.0e-6 {
        return 1.0;
    }
    let ndc = light_clip.xyz / light_clip.w;
    let uv = vec2<f32>(ndc.x, -ndc.y) * 0.5 + vec2<f32>(0.5);
    let depth = ndc.z;
    let bias = shadow.params0.z;
    var dims = vec2<f32>(1.0);
    if is_point {
        dims = max(vec2<f32>(textureDimensions(point_shadow_map_tex)), vec2<f32>(1.0));
    } else {
        dims = max(vec2<f32>(textureDimensions(spot_shadow_map_tex)), vec2<f32>(1.0));
    }
    let texel = 1.0 / dims;
    if depth <= 0.0 || depth >= 1.0
        || any(uv < texel)
        || any(uv > (vec2<f32>(1.0) - texel)) {
        return 1.0;
    }
    let layer_i = i32(layer);
    var sum = 0.0;
    if is_point {
        for (var y = -1; y <= 1; y = y + 1) {
            for (var x = -1; x <= 1; x = x + 1) {
                sum += textureSampleCompare(point_shadow_map_tex, shadow_map_sampler, uv + vec2<f32>(f32(x), f32(y)) * texel, layer_i, depth - bias);
            }
        }
        return sum / 9.0;
    }
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            sum += textureSampleCompare(spot_shadow_map_tex, shadow_map_sampler, uv + vec2<f32>(f32(x), f32(y)) * texel, layer_i, depth - bias);
        }
    }
    return sum / 9.0;
}

fn ray_shadow_factor(world_pos: vec3<f32>, normal_ws: vec3<f32>, light_dir_to_light: vec3<f32>) -> f32 {
    if shadow.params0.x < 0.5 || shadow.ray_params.x < 0.5 {
        return 1.0;
    }
    let view_dist = distance(scene.camera_pos.xyz, world_pos);
    if view_dist > shadow.ray_splits.w {
        return 1.0;
    }
    // Pick the tightest cascade that actually contains the position; split
    // distances alone mismatch the fitted ortho bounds at screen edges.
    let cascade_count = min(u32(shadow.ray_params.y + 0.5), MAX_SHADOW_RAY_CASCADES);
    var visibility = 1.0;
    for (var cascade = 0u; cascade < cascade_count; cascade = cascade + 1u) {
        let sampled = sample_ray_shadow_array(shadow.ray_light_view_proj[cascade], world_pos, normal_ws, light_dir_to_light, cascade);
        if sampled >= 0.0 {
            visibility = sampled;
            break;
        }
    }
    let strength = clamp(shadow.params0.y, 0.0, 1.0);
    return mix(1.0, visibility, strength);
}

fn spot_shadow_factor(world_pos: vec3<f32>, normal_ws: vec3<f32>, light_index: u32) -> f32 {
    if shadow.params0.x < 0.5 {
        return 1.0;
    }
    for (var i = 0u; i < MAX_SHADOW_SPOT_LIGHTS; i = i + 1u) {
        let params = shadow.spot_params[i];
        if params.x > 0.5 && u32(params.y + 0.5) == light_index {
            let light = scene.spot_lights[light_index];
            let visibility = sample_shadow_array(shadow.spot_light_view_proj[i], world_pos, normal_ws, light.position_range.xyz - world_pos, u32(params.z + 0.5), false);
            return mix(1.0, visibility, clamp(shadow.params0.y, 0.0, 1.0));
        }
    }
    return 1.0;
}

fn point_shadow_face(to_light: vec3<f32>) -> u32 {
    let v = -to_light;
    let a = abs(v);
    if a.x >= a.y && a.x >= a.z {
        return select(1u, 0u, v.x >= 0.0);
    }
    if a.y >= a.z {
        return select(3u, 2u, v.y >= 0.0);
    }
    return select(5u, 4u, v.z >= 0.0);
}

fn point_shadow_factor(world_pos: vec3<f32>, normal_ws: vec3<f32>, light_index: u32, to_light: vec3<f32>) -> f32 {
    if shadow.params0.x < 0.5 {
        return 1.0;
    }
    for (var i = 0u; i < MAX_SHADOW_POINT_LIGHTS; i = i + 1u) {
        let params = shadow.point_params[i];
        if params.x > 0.5 && u32(params.y + 0.5) == light_index {
            let face = point_shadow_face(to_light);
            let layer = u32(params.z + 0.5) + face;
            let matrix_index = u32(params.z + 0.5) + face;
            let visibility = sample_shadow_array(shadow.point_light_view_proj[matrix_index], world_pos, normal_ws, to_light, layer, true);
            return mix(1.0, visibility, clamp(shadow.params0.y, 0.0, 1.0));
        }
    }
    return 1.0;
}

// Windowed inverse-square falloff: smooth fade to zero at range instead of a
// hard circle at the range cutoff.
fn range_attenuation(dist_sq: f32, range_sq: f32) -> f32 {
    let ratio = clamp(dist_sq / max(range_sq, 1.0e-8), 0.0, 1.0);
    let window = 1.0 - ratio * ratio;
    return (window * window) / (dist_sq + 1.0e-2);
}

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let n_dot_h = max(dot(n, h), 0.0);
    let n_dot_h2 = n_dot_h * n_dot_h;
    let denom = n_dot_h2 * (a2 - 1.0) + 1.0;
    return a2 / max(3.14159265 * denom * denom, 1.0e-5);
}

fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return n_dot_v / max(n_dot_v * (1.0 - k) + k, 1.0e-5);
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let n_dot_v = max(dot(n, v), 0.0);
    let n_dot_l = max(dot(n, l), 0.0);
    let ggx2 = geometry_schlick_ggx(n_dot_v, roughness);
    let ggx1 = geometry_schlick_ggx(n_dot_l, roughness);
    return ggx1 * ggx2;
}

fn pow5(x: f32) -> f32 {
    let x2 = x * x;
    return x2 * x2 * x;
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    let m = 1.0 - cos_theta;
    return f0 + (vec3<f32>(1.0) - f0) * pow5(m);
}

fn fresnel_schlick_roughness(cos_theta: f32, f0: vec3<f32>, roughness: f32) -> vec3<f32> {
    let one_minus_roughness = vec3<f32>(1.0 - roughness);
    let m = 1.0 - cos_theta;
    return f0 + (max(one_minus_roughness, f0) - f0) * pow5(m);
}

// ACES filmic fit (Narkowicz). Applied at the end of lit materials so HDR
// light sums roll off instead of clipping; UI/2D/unlit stay untouched.
fn tonemap_aces(x: vec3<f32>) -> vec3<f32> {
    let mapped = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
    return clamp(mapped, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn brdf_pbr(
    albedo: vec3<f32>,
    n: vec3<f32>,
    v: vec3<f32>,
    l: vec3<f32>,
    roughness: f32,
    metallic: f32,
    radiance: vec3<f32>,
) -> vec3<f32> {
    let hv = v + l;
    // v == -l at grazing opposition makes normalize(0) NaN.
    let h = hv * inverseSqrt(max(dot(hv, hv), 1.0e-8));
    let ndf = distribution_ggx(n, h, roughness);
    let g = geometry_smith(n, v, l, roughness);
    let f0 = mix(vec3<f32>(0.04), albedo, vec3<f32>(metallic));
    let f = fresnel_schlick(max(dot(h, v), 0.0), f0);

    let numerator = ndf * g * f;
    let denom = 4.0 * max(dot(n, v), 0.0) * max(dot(n, l), 0.0) + 1.0e-5;
    let specular = numerator / denom;

    let k_s = f;
    let k_d = (vec3<f32>(1.0) - k_s) * (1.0 - metallic);
    let diffuse = k_d * albedo / 3.14159265;
    let n_dot_l = max(dot(n, l), 0.0);
    return (diffuse + specular) * radiance * n_dot_l;
}

fn perro_lit_standard(
    in: FragmentInput,
    base_color: vec4<f32>,
    roughness_in: f32,
    metallic_in: f32,
    ao_in: f32,
    emissive: vec3<f32>,
) -> vec4<f32> {
    let material = decode_material_params(in.packed_material_params);
    let albedo = base_color.rgb;
    var n = normalize(in.normal_ws);
    if material.flat_shading {
        n = normalize(cross(dpdx(in.world_pos), dpdy(in.world_pos)));
        if material.mirrored_winding {
            n = -n;
        }
    }
    if material.double_sided && (in.is_front == material.mirrored_winding) {
        n = -n;
    }
    let mesh_fade = mesh_blend_fade(in, material);
    n = apply_mesh_normal_blend(material, n, in.world_pos, mesh_fade);
    let v = normalize(scene.camera_pos.xyz - in.world_pos);
    let roughness = clamp(roughness_in, 0.04, 1.0);
    let metallic = clamp(metallic_in, 0.0, 1.0);
    let ao = clamp(ao_in, 0.0, 1.0);
    let alpha = perro_material_alpha(in, base_color.a);
    if material.meshlet_debug_view {
        return vec4<f32>(albedo, 1.0);
    }

    var light_rgb = vec3<f32>(0.0);

    let ray_count = u32(scene.ambient_and_counts.x);
    for (var i = 0u; i < ray_count; i = i + 1u) {
        let ray = scene.ray_lights[i];
        let ray_dir = ray.direction.xyz;
        let l = -ray_dir * inverseSqrt(max(dot(ray_dir, ray_dir), 1.0e-8));
        var radiance = ray.color_intensity.xyz * ray.color_intensity.w;
        if i == 0u && material.receive_shadows {
            radiance *= shadow_factor(in.world_pos, n, l);
        }
        light_rgb += brdf_pbr(albedo, n, v, l, roughness, metallic, radiance);
    }

    let point_count = u32(scene.ambient_and_counts.y);
    for (var i = 0u; i < point_count; i = i + 1u) {
        let light = scene.point_lights[i];
        let to_light = light.position_range.xyz - in.world_pos;
        let dist_sq = dot(to_light, to_light);
        let range_sq = light.position_range.w * light.position_range.w;
        if dist_sq <= range_sq {
            let inv_dist = inverseSqrt(max(dist_sq, 1.0e-8));
            let l = to_light * inv_dist;
            let radiance = light.color_intensity.xyz * light.color_intensity.w;
            let attenuation = range_attenuation(dist_sq, range_sq);
            let shadow_vis = select(1.0, point_shadow_factor(in.world_pos, n, i, to_light), material.receive_shadows);
            light_rgb += brdf_pbr(albedo, n, v, l, roughness, metallic, radiance * attenuation * shadow_vis);
        }
    }

    let spot_count = u32(scene.ambient_and_counts.z);
    for (var i = 0u; i < spot_count; i = i + 1u) {
        let light = scene.spot_lights[i];
        let to_light = light.position_range.xyz - in.world_pos;
        let dist_sq = dot(to_light, to_light);
        let range_sq = light.position_range.w * light.position_range.w;
        if dist_sq <= range_sq {
            let inv_dist = inverseSqrt(max(dist_sq, 1.0e-8));
            let l = to_light * inv_dist;
            let spot_dir = light.direction_outer_cos.xyz;
            let cos_theta = dot(spot_dir, -l);
            let outer_cos = light.direction_outer_cos.w;
            let inner_cos = light.inner_cos_pad.x;
            let t = clamp((cos_theta - outer_cos) / max(inner_cos - outer_cos, 0.0001), 0.0, 1.0);
            let radiance = light.color_intensity.xyz * light.color_intensity.w * t;
            let attenuation = range_attenuation(dist_sq, range_sq);
            let shadow_vis = select(1.0, spot_shadow_factor(in.world_pos, n, i), material.receive_shadows);
            light_rgb += brdf_pbr(albedo, n, v, l, roughness, metallic, radiance * attenuation * shadow_vis);
        }
    }

    let f_ambient = fresnel_schlick_roughness(max(dot(n, v), 0.0), vec3<f32>(0.04), roughness);
    let k_s_ambient = f_ambient;
    let k_d_ambient = (vec3<f32>(1.0) - k_s_ambient) * (1.0 - metallic);
    let ambient_radiance = scene.ambient_color.xyz * scene.ambient_color.w * ao;
    let ambient_diffuse = k_d_ambient * albedo * ambient_radiance;
    let ambient_specular = k_s_ambient * ambient_radiance * (0.25 + 0.75 * (1.0 - roughness));

    let shaded = ambient_diffuse + ambient_specular + light_rgb + emissive;
    return vec4<f32>(tonemap_aces(shaded), alpha);
}
