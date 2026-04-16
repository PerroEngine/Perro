const MAX_RAY_LIGHTS: u32 = 3u;
const MAX_POINT_LIGHTS: u32 = 8u;
const MAX_SPOT_LIGHTS: u32 = 8u;
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
}

struct Shadow3D {
    light_view_proj: mat4x4<f32>,
    params0: vec4<f32>, // enabled, strength, depth_bias, normal_bias
}

struct DecodedMaterialParams {
    alpha_mode: u32,
    alpha_cutoff: f32,
    double_sided: bool,
    material_flags: u32,
    meshlet_debug_view: bool,
    flat_shading: bool,
}

@group(0) @binding(0)
var<uniform> scene: Scene3D;
@group(0) @binding(1)
var<storage, read> skeletons: array<mat4x4<f32>>;
@group(0) @binding(2)
var<storage, read> custom_params: array<vec4<f32>>;
@group(1) @binding(0)
var material_sampler: sampler;
@group(1) @binding(1)
var material_base_color_tex: texture_2d<f32>;
@group(2) @binding(0)
var<uniform> shadow: Shadow3D;
@group(2) @binding(1)
var shadow_map_tex: texture_depth_2d;
@group(2) @binding(2)
var shadow_map_sampler: sampler_comparison;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
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

fn decode_material_params(packed: u32) -> DecodedMaterialParams {
    let flags = (packed >> 3u) & 0x1fffu;
    return DecodedMaterialParams(
        packed & 0x3u,
        unpack_unorm8(packed, 16u),
        ((packed >> 2u) & 0x1u) != 0u,
        flags,
        (flags & 0x1u) != 0u,
        (flags & 0x2u) != 0u,
    );
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

@vertex
fn vs_main(v: VertexInput, inst: InstanceInput) -> VertexOutput {
    let base = inst.skeleton_params.x;
    let m0 = skeletons[base + v.joints.x] * v.weights.x;
    let m1 = skeletons[base + v.joints.y] * v.weights.y;
    let m2 = skeletons[base + v.joints.z] * v.weights.z;
    let m3 = skeletons[base + v.joints.w] * v.weights.w;
    let skin = m0 + m1 + m2 + m3;
    let pos = (skin * vec4<f32>(v.pos, 1.0)).xyz;
    let normal = (skin * vec4<f32>(v.normal, 0.0)).xyz;
    let p = vec4<f32>(pos, 1.0);
    let world = vec4<f32>(
        dot(inst.model_row_0, p),
        dot(inst.model_row_1, p),
        dot(inst.model_row_2, p),
        1.0,
    );
    let normal_ws = normalize(vec3<f32>(
        dot(inst.model_row_0.xyz, normal),
        dot(inst.model_row_1.xyz, normal),
        dot(inst.model_row_2.xyz, normal),
    ));

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
    out.uv = v.uv;
    return out;
}

fn custom_param(in: FragmentInput, index: u32) -> vec4<f32> {
    if index >= in.custom_range.y {
        return vec4<f32>(0.0);
    }
    return custom_params[in.custom_range.x + index];
}

fn shadow_factor(world_pos: vec3<f32>, normal_ws: vec3<f32>, light_dir_to_light: vec3<f32>) -> f32 {
    if shadow.params0.x < 0.5 {
        return 1.0;
    }
    let receiver_pos = world_pos;
    let light_clip = shadow.light_view_proj * vec4<f32>(receiver_pos, 1.0);
    if abs(light_clip.w) <= 1.0e-6 {
        return 1.0;
    }
    let ndc = light_clip.xyz / light_clip.w;
    let uv = ndc.xy * 0.5 + vec2<f32>(0.5);
    let depth = ndc.z;
    let bias = shadow.params0.z;
    let dims = max(vec2<f32>(textureDimensions(shadow_map_tex)), vec2<f32>(1.0));
    let texel = 1.0 / dims;
    if depth <= 0.0 || depth >= 1.0
        || any(uv < texel)
        || any(uv > (vec2<f32>(1.0) - texel)) {
        return 1.0;
    }
    let uv_safe = uv;
    let visibility = textureSampleCompare(
        shadow_map_tex,
        shadow_map_sampler,
        uv_safe,
        depth - bias
    );
    let strength = clamp(shadow.params0.y, 0.0, 1.0);
    return mix(1.0, visibility, strength);
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

fn brdf_pbr(
    albedo: vec3<f32>,
    n: vec3<f32>,
    v: vec3<f32>,
    l: vec3<f32>,
    roughness: f32,
    metallic: f32,
    radiance: vec3<f32>,
) -> vec3<f32> {
    let h = normalize(v + l);
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
