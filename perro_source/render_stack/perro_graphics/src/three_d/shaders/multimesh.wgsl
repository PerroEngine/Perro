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
    // Hemisphere ambient: radiance from below (premultiplied), w unused.
    ground_color: vec4<f32>,
    // Sky radiance at the horizon (premultiplied) for env reflections.
    sky_horizon_color: vec4<f32>,
    // x = IBL intensity, y = max specular mip, zw = environment rotation sin/cos.
    ibl_params: vec4<f32>,
    // Frame globals: x = time seconds (wraps hourly), y = delta seconds,
    // z = frame index, w = 0..1 phase over 60 seconds.
    time_params: vec4<f32>,
    // xy = viewport pixels, zw = 1 / pixels.
    resolution: vec4<f32>,
}

struct MultiMeshDrawParam {
    model_row_0: vec4<f32>,
    model_row_1: vec4<f32>,
    model_row_2: vec4<f32>,
    custom_params: vec2<u32>,
    packed_color: u32,
    packed_pbr_params_0: u32,
    packed_emissive: u32,
    packed_material_params: u32,
    scale_bits: u32,
    packed_blend_params: u32,
    // Local color bleed tint (pack_local_bleed layout); 0 = none.
    packed_bleed: u32,
    pad: array<u32, 3>,
}

@group(0) @binding(0)
var<uniform> scene: Scene3D;
@group(0) @binding(1)
var<storage, read> multimesh_draws: array<MultiMeshDrawParam>;
@group(0) @binding(2)
var mesh_blend_depth_tex: texture_depth_2d;
@group(0) @binding(13)
var ssao_tex: texture_2d<f32>;
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
// Instance data moved to storage so a GPU cull pass can compact visible
// instances. visible_indices maps draw instance_index -> source instance; the
// direct-draw fallback fills it as identity so the same fetch path works.
@group(0) @binding(8)
var<storage, read> visible_indices: array<u32>;
@group(0) @binding(9)
var<storage, read> multimesh_instances: array<MultiMeshInstanceStorage>;

// Matches the packed CPU layout (40 bytes): position f32x3, rotation snorm16x4
// (2 words), scale f32x3, draw_id, blend_meta_id. WGSL has no i16, so the
// rotation lanes are stored as raw u32 words and unpacked in perro_fetch_instance.
struct MultiMeshInstanceStorage {
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
};

fn perro_unpack_snorm16_pair(word: u32) -> vec2<f32> {
    let lo = i32(word << 16u) >> 16u;
    let hi = i32(word) >> 16u;
    return vec2<f32>(
        max(f32(lo) / 32767.0, -1.0),
        max(f32(hi) / 32767.0, -1.0),
    );
}

// ---- Decals -------------------------------------------------------------
// Projected box decals patched into the lit path before lighting: albedo
// and normal are modified in place, emission is added on top. Records are
// pre-sorted by priority on the CPU (later records blend over earlier).
struct DecalGpu {
    inv_row_0: vec4<f32>,
    inv_row_1: vec4<f32>,
    inv_row_2: vec4<f32>,
    // rgb tint, a = opacity.
    tint: vec4<f32>,
    // rgb = emission color * energy, w = normal strength.
    emission: vec4<f32>,
    // x = albedo layer (-1 none), y = normal layer, z = emission layer,
    // w = normal fade threshold.
    params0: vec4<f32>,
    // x = albedo mix, y = distance fade begin (0 off), z = 1/fade length.
    params1: vec4<f32>,
}

struct DecalsBuffer {
    count: vec4<u32>,
    decals: array<DecalGpu>,
}

@group(0) @binding(10)
var<storage, read> scene_decals: DecalsBuffer;
@group(0) @binding(11)
var decal_textures: texture_2d_array<f32>;
@group(0) @binding(12)
var decal_sampler: sampler;

@group(1) @binding(0)
var material_sampler: sampler;
@group(1) @binding(1)
var material_base_color_tex: texture_2d<f32>;
@group(1) @binding(2)
var custom_image_tex_0: texture_2d<f32>;
@group(1) @binding(3)
var custom_image_tex_1: texture_2d<f32>;
@group(1) @binding(4)
var custom_image_tex_2: texture_2d<f32>;
@group(1) @binding(5)
var custom_image_tex_3: texture_2d<f32>;
@group(1) @binding(6)
var custom_image_tex_4: texture_2d<f32>;
@group(1) @binding(7)
var custom_image_tex_5: texture_2d<f32>;
@group(1) @binding(8)
var custom_image_tex_6: texture_2d<f32>;
@group(1) @binding(9)
var custom_image_tex_7: texture_2d<f32>;

@group(2) @binding(0)
var<uniform> mesh_blend_mask_id: vec4<u32>;
@group(3) @binding(0)
var<storage, read> environment_data: array<u32>;

struct DecalSurface {
    albedo: vec3<f32>,
    normal: vec3<f32>,
    emissive: vec3<f32>,
}

// Decal layers hold raw sRGB bytes behind a linear view; color layers decode
// here, normal layers read raw.
// Seam width floor in pixels so distant blends never collapse to a hard line.
const MESH_BLEND_MIN_PIXELS: f32 = 2.5;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec4<f32>,
    @location(12) uv: vec2<f32>,
};

// Instance payload, fetched from storage (no longer a vertex buffer).
struct InstanceInput {
    position: vec3<f32>,
    rotation: vec4<f32>,
    scale: vec3<f32>,
    draw_id: u32,
    blend_meta_id: u32,
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
    @location(7) @interpolate(flat) packed_bleed: u32,
    @location(8) ambient_color: vec3<f32>,
    @location(9) @interpolate(flat) packed_pbr_params_0: u32,
    @location(10) @interpolate(flat) packed_material_params: u32,
    @location(11) @interpolate(flat) packed_color: u32,
    @location(12) @interpolate(flat) packed_emissive: u32,
};

struct FragmentInput {
    @builtin(position) frag_pos: vec4<f32>,
    @location(0) lit_color: vec3<f32>,
    @location(1) @interpolate(flat) packed_blend_params: u32,
    @location(2) world_pos: vec3<f32>,
    @location(3) normal_ws: vec3<f32>,
    @location(4) @interpolate(flat) custom_range: vec2<u32>,
    @location(5) uv: vec2<f32>,
    @location(7) @interpolate(flat) packed_bleed: u32,
    @location(8) ambient_color: vec3<f32>,
    @location(9) @interpolate(flat) packed_pbr_params_0: u32,
    @location(10) @interpolate(flat) packed_material_params: u32,
    @location(11) @interpolate(flat) packed_color: u32,
    @location(12) @interpolate(flat) packed_emissive: u32,
    @builtin(front_facing) is_front: bool,
};

fn perro_multimesh_ssao(frag_pos: vec2<f32>) -> f32 {
    let dims_u = textureDimensions(ssao_tex);
    let dims = vec2<i32>(dims_u);
    let uv = frag_pos * scene.resolution.zw;
    let coord = clamp(vec2<i32>(uv * vec2<f32>(dims_u)), vec2<i32>(0), dims - vec2<i32>(1));
    return textureLoad(ssao_tex, coord, 0).r;
}

fn unpack_rgba8(v: u32) -> vec4<f32> {
    let r = f32(v & 255u) * (1.0 / 255.0);
    let g = f32((v >> 8u) & 255u) * (1.0 / 255.0);
    let b = f32((v >> 16u) & 255u) * (1.0 / 255.0);
    let a = f32((v >> 24u) & 255u) * (1.0 / 255.0);
    return vec4<f32>(r, g, b, a);
}

fn perro_unpack_unorm8(packed: u32, shift: u32) -> f32 {
    return f32(perro_unpack_byte(packed, shift)) * (1.0 / 255.0);
}

fn perro_mesh_blend_alpha(frag_pos: vec4<f32>, world_pos: vec3<f32>, packed: u32) -> f32 {
    if packed == 0u {
        return 1.0;
    }
    let dims_u = textureDimensions(mesh_blend_depth_tex);
    let dims = vec2<i32>(i32(dims_u.x), i32(dims_u.y));
    let coord = vec2<i32>(floor(frag_pos.xy));
    if any(coord < vec2<i32>(0)) || any(coord >= dims) {
        return 1.0;
    }
    let receiver_depth = textureLoad(mesh_blend_depth_tex, coord, 0);
    if receiver_depth >= 0.999999 {
        return 1.0;
    }
    let params = perro_decode_mesh_blend_params(packed);
    let view_dist = distance(world_pos, scene.camera_pos.xyz);
    let receiver_world = perro_mesh_blend_world_from_depth(coord, dims_u, receiver_depth);
    let receiver_dist = distance(receiver_world, scene.camera_pos.xyz);
    let raw_depth_delta = receiver_dist - view_dist;
    if raw_depth_delta <= 0.0 {
        return 1.0;
    }
    // Distance-compensated width: world units covered by one pixel here.
    let texel_world = (length(dpdx(world_pos)) + length(dpdy(world_pos))) * 0.5;
    let base_width = max(params.x, 0.0001);
    let max_width = max(base_width, texel_world * MESH_BLEND_MIN_PIXELS);
    let min_width = min(params.y, base_width) * (max_width / base_width);
    var noise = 0.0;
    if params.z > 0.0 {
        // Anchor the noise to the receiver surface so it does not swim with
        // the camera.
        let tile = max(params.w * 0.05, 0.05);
        let p = (receiver_world.xz
            + vec2<f32>(receiver_world.y * 0.53, receiver_world.y * 0.29)) / tile;
        let soft_noise = smoothstep(0.1, 0.9, perro_mesh_blend_noise(p));
        noise = (soft_noise - 0.5) * params.z * max_width;
    }
    let depth_delta = max(raw_depth_delta + noise, 0.0);
    if depth_delta > max_width * 1.15 {
        return 1.0;
    }
    let fade = smoothstep(min_width, max_width, depth_delta);
    return fade * fade * (3.0 - 2.0 * fade);
}

fn perro_rotate_vec_by_quat(v: vec3<f32>, q: vec4<f32>) -> vec3<f32> {
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

fn perro_apply_blend_shapes(v: VertexInput, inst: InstanceInput, vertex_index: u32) -> VertexInput {
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
    return VertexInput(out_pos, vec4<f32>(normalize(out_normal), 0.0), v.uv);
}

struct LocalBleed {
    color: vec3<f32>,
    strength: f32,
    dir: vec3<f32>,
}

// Layout matches pack_local_bleed on the CPU: r5 g5 b5 strength5 oct_x6 oct_y6.
fn perro_distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let n_dot_h = max(dot(n, h), 0.0);
    let denom = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / max(3.14159265 * denom * denom, 1.0e-6);
}

fn perro_geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) * 0.125;
    return n_dot_v / max(n_dot_v * (1.0 - k) + k, 1.0e-6);
}

fn perro_geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    return perro_geometry_schlick_ggx(max(dot(n, v), 0.0), roughness)
        * perro_geometry_schlick_ggx(max(dot(n, l), 0.0), roughness);
}

fn perro_fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    let m = clamp(1.0 - cos_theta, 0.0, 1.0);
    let m2 = m * m;
    return f0 + (vec3<f32>(1.0) - f0) * m2 * m2 * m;
}

fn perro_fresnel_schlick_roughness(
    cos_theta: f32,
    f0: vec3<f32>,
    roughness: f32,
) -> vec3<f32> {
    let m = clamp(1.0 - cos_theta, 0.0, 1.0);
    let m2 = m * m;
    return f0 + (max(vec3<f32>(1.0 - roughness), f0) - f0) * m2 * m2 * m;
}

struct EnvironmentCubeCoord {
    face: u32,
    uv: vec2<f32>,
}


fn perro_multimesh_brdf(
    albedo: vec3<f32>,
    n: vec3<f32>,
    v: vec3<f32>,
    l: vec3<f32>,
    roughness: f32,
    metallic: f32,
    radiance: vec3<f32>,
) -> vec3<f32> {
    let hv = v + l;
    let h = hv * inverseSqrt(max(dot(hv, hv), 1.0e-8));
    let f0 = mix(vec3<f32>(0.04), albedo, vec3<f32>(metallic));
    let f = perro_fresnel_schlick(max(dot(h, v), 0.0), f0);
    let numerator = perro_distribution_ggx(n, h, roughness) * perro_geometry_smith(n, v, l, roughness) * f;
    let denominator = 4.0 * max(dot(n, v), 0.0) * max(dot(n, l), 0.0) + 1.0e-5;
    let specular = numerator / denominator;
    let diffuse = (vec3<f32>(1.0) - f) * (1.0 - metallic) * albedo / 3.14159265;
    return (diffuse + specular) * radiance * max(dot(n, l), 0.0);
}

fn perro_apply_multimesh_normal_map(
    in: FragmentInput,
    normal_ws: vec3<f32>,
    scale: f32,
) -> vec3<f32> {
    let dpdx_ws = dpdx(in.world_pos);
    let dpdy_ws = dpdy(in.world_pos);
    let duvdx = dpdx(in.uv);
    let duvdy = dpdy(in.uv);
    let det = duvdx.x * duvdy.y - duvdx.y * duvdy.x;
    if abs(det) <= 1.0e-8 {
        return normal_ws;
    }
    let inv_det = 1.0 / det;
    let tangent_raw = (dpdx_ws * duvdy.y - dpdy_ws * duvdx.y) * inv_det;
    let bitangent_raw = (-dpdx_ws * duvdy.x + dpdy_ws * duvdx.x) * inv_det;
    let tangent_ortho = tangent_raw - normal_ws * dot(normal_ws, tangent_raw);
    let tangent_len_sq = dot(tangent_ortho, tangent_ortho);
    if tangent_len_sq <= 1.0e-8 || dot(bitangent_raw, bitangent_raw) <= 1.0e-8 {
        return normal_ws;
    }
    let tangent = tangent_ortho * inverseSqrt(tangent_len_sq);
    let cross_nt = normalize(cross(normal_ws, tangent));
    let handedness = select(-1.0, 1.0, dot(cross_nt, bitangent_raw) >= 0.0);
    let bitangent = cross_nt * handedness;
    var mapped = textureSample(custom_image_tex_1, material_sampler, in.uv).xyz * 2.0 - 1.0;
    mapped = normalize(vec3<f32>(mapped.xy * scale, mapped.z));
    return normalize(tangent * mapped.x + bitangent * mapped.y + normal_ws * mapped.z);
}

fn perro_lit_standard_with_ssao(
    in: FragmentInput,
    base: vec4<f32>,
    roughness: f32,
    metallic: f32,
    occlusion: f32,
    emissive: vec3<f32>,
    surface_ssao: f32,
) -> vec4<f32> {
    let flags = (in.packed_material_params >> 3u) & 0x1fffu;
    let mirrored_winding = (flags & 0x20u) != 0u;
    var n = normalize(in.normal_ws);
    if (flags & 0x2u) != 0u {
        n = normalize(cross(dpdx(in.world_pos), dpdy(in.world_pos)));
        if mirrored_winding {
            n = -n;
        }
    }
    let double_sided = ((in.packed_material_params >> 2u) & 0x1u) != 0u;
    if double_sided && (in.is_front == mirrored_winding) {
        n = -n;
    }
    if (flags & 0x400u) != 0u {
        let normal_scale = perro_unpack_unorm8(in.packed_pbr_params_0, 24u) * 4.0;
        n = perro_apply_multimesh_normal_map(in, n, normal_scale);
    }
    var albedo = base.rgb;
    var decal_emissive = vec3<f32>(0.0);
    if scene_decals.count.x > 0u {
        let decal_surface = perro_apply_decals(in.world_pos, albedo, n);
        albedo = decal_surface.albedo;
        n = decal_surface.normal;
        decal_emissive = decal_surface.emissive;
    }
    let roughness_safe = clamp(roughness, 0.04, 1.0);
    let metallic_safe = clamp(metallic, 0.0, 1.0);
    let ao = clamp(occlusion, 0.0, 1.0) * surface_ssao;
    let v = normalize(scene.camera_pos.xyz - in.world_pos);
    let hemi = clamp(n.y * 0.5 + 0.5, 0.0, 1.0);
    var ambient = mix(scene.ground_color.xyz, scene.ambient_color.xyz * scene.ambient_color.w, hemi);
    let bleed = perro_decode_local_bleed(in.packed_bleed);
    let bleed_wrap = clamp(dot(n, bleed.dir) * 0.5 + 0.5, 0.0, 1.0);
    ambient += bleed.color * bleed.strength * 0.45 * (0.35 + 0.65 * bleed_wrap);
    let f0 = mix(vec3<f32>(0.04), albedo, vec3<f32>(metallic_safe));
    let n_dot_v = max(dot(n, v), 0.0);
    let ambient_fresnel = perro_fresnel_schlick_roughness(n_dot_v, f0, roughness_safe);
    let ambient_kd = (vec3<f32>(1.0) - ambient_fresnel) * (1.0 - metallic_safe);
    var ambient_diffuse =
        (vec3<f32>(1.0) - ambient_fresnel) * (1.0 - metallic_safe) * albedo * ambient * ao;
    var ambient_specular =
        ambient_fresnel * ambient * (0.15 + 0.35 * (1.0 - roughness_safe)) * ao;
    if scene.ibl_params.x > 0.0 {
        let intensity = scene.ibl_params.x;
        let irradiance = perro_sample_environment_irradiance(perro_rotate_environment_direction(n));
        ambient_diffuse = ambient_kd * albedo * irradiance * intensity * ao;
        let reflection = reflect(-v, n);
        let prefiltered = perro_sample_environment_specular(
            perro_rotate_environment_direction(reflection),
            roughness_safe * scene.ibl_params.y,
        );
        let brdf = perro_sample_environment_brdf(vec2<f32>(n_dot_v, roughness_safe));
        ambient_specular =
            prefiltered * (ambient_fresnel * brdf.x + vec3<f32>(brdf.y)) * intensity * ao;
    }
    var direct = vec3<f32>(0.0);
    let ray_count = u32(scene.ambient_and_counts.x);
    for (var i = 0u; i < ray_count; i = i + 1u) {
        let ray = scene.ray_lights[i];
        let ray_dir = ray.direction.xyz;
        let l = -ray_dir * inverseSqrt(max(dot(ray_dir, ray_dir), 1.0e-8));
        let radiance = ray.color_intensity.xyz * ray.color_intensity.w;
        direct += perro_multimesh_brdf(albedo, n, v, l, roughness_safe, metallic_safe, radiance);
    }
    let alpha_mode = in.packed_material_params & 0x3u;
    var material_alpha = base.a;
    if alpha_mode == 0u {
        material_alpha = 1.0;
    }
    if alpha_mode == 1u {
        let cutoff = perro_unpack_unorm8(in.packed_material_params, 16u);
        if material_alpha < cutoff {
            discard;
        }
    }
    let alpha = perro_mesh_blend_alpha(in.frag_pos, in.world_pos, in.packed_blend_params)
        * material_alpha;
    return vec4<f32>(
        ambient_diffuse + ambient_specular + direct + emissive + decal_emissive,
        alpha,
    );
}

fn perro_lit_standard(
    in: FragmentInput,
    base: vec4<f32>,
    roughness: f32,
    metallic: f32,
    occlusion: f32,
    emissive: vec3<f32>,
) -> vec4<f32> {
    return perro_lit_standard_with_ssao(
        in,
        base,
        roughness,
        metallic,
        occlusion,
        emissive,
        perro_multimesh_ssao(in.frag_pos.xy),
    );
}

fn shade_standard_multimesh(in: FragmentInput) -> vec4<f32> {
    let flags = (in.packed_material_params >> 3u) & 0x1fffu;
    let color = unpack_rgba8(in.packed_color);
    var base_sample = vec4<f32>(1.0);
    if (flags & 0x4u) != 0u {
        base_sample = textureSample(material_base_color_tex, material_sampler, in.uv);
    }
    var albedo = color.rgb * base_sample.rgb;
    if (flags & 0x100u) != 0u {
        let saturation = max(max(color.r, color.g), color.b) - min(min(color.r, color.g), color.b);
        let tint_weight = 0.2 * clamp(saturation, 0.0, 1.0);
        let texture_luma = dot(base_sample.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        albedo = mix(albedo, color.rgb * texture_luma, tint_weight);
    }
    var roughness = perro_unpack_unorm8(in.packed_pbr_params_0, 0u);
    var metallic = perro_unpack_unorm8(in.packed_pbr_params_0, 8u);
    var ao = 1.0;
    let emissive = unpack_rgba8(in.packed_emissive);
    var lit_emissive = emissive.xyz * (emissive.w * 16.0);
    if (flags & 0x200u) != 0u {
        let metallic_roughness = textureSample(custom_image_tex_0, material_sampler, in.uv);
        roughness *= metallic_roughness.g;
        metallic *= metallic_roughness.b;
    }
    if (flags & 0x800u) != 0u {
        let sampled_ao = textureSample(custom_image_tex_2, material_sampler, in.uv).r;
        let strength = perro_unpack_unorm8(in.packed_pbr_params_0, 16u);
        ao = mix(1.0, sampled_ao, strength);
    }
    if (flags & 0x1000u) != 0u {
        lit_emissive *= textureSample(custom_image_tex_3, material_sampler, in.uv).rgb;
    }
    return perro_lit_standard_with_ssao(
        in,
        vec4<f32>(albedo, color.a * base_sample.a),
        roughness,
        metallic,
        ao,
        lit_emissive,
        perro_multimesh_ssao(in.frag_pos.xy),
    );
}


fn perro_multimesh_vs_main_base(v: VertexInput, inst: InstanceInput, vertex_index: u32) -> VertexOutput {
    let draw = multimesh_draws[inst.draw_id];
    let scale = bitcast<f32>(draw.scale_bits);
    let rot = normalize(inst.rotation);
    let blended = perro_apply_blend_shapes(v, inst, vertex_index);
    let local_pos = perro_rotate_vec_by_quat(blended.pos * (inst.scale * scale), rot) + inst.position;
    // Inverse-transpose of a diagonal scale: divide, so non-uniform instance
    // scale does not skew the normal.
    let local_nrm = perro_rotate_vec_by_quat(
        normalize(blended.normal.xyz / max(inst.scale, vec3<f32>(1.0e-6))),
        rot,
    );
    let p = vec4<f32>(local_pos, 1.0);
    let world = vec4<f32>(
        dot(draw.model_row_0, p),
        dot(draw.model_row_1, p),
        dot(draw.model_row_2, p),
        1.0,
    );
    let normal_ws = perro_transform_normal_ws(
        draw.model_row_0.xyz,
        draw.model_row_1.xyz,
        draw.model_row_2.xyz,
        local_nrm,
    );

    let base = unpack_rgba8(draw.packed_color);
    let emissive_packed = unpack_rgba8(draw.packed_emissive);
    let emissive = emissive_packed.xyz * (emissive_packed.w * 16.0);
    let n = normal_ws;
    let hemi = clamp(n.y * 0.5 + 0.5, 0.0, 1.0);
    let ambient =
        mix(scene.ground_color.xyz, scene.ambient_color.xyz * scene.ambient_color.w, hemi);
    var ambient_lit = ambient;
    var lit = vec3<f32>(0.0);
    // Local color bleed: one tint per multimesh draw, vertex-lit.
    let bleed = perro_decode_local_bleed(draw.packed_bleed);
    let bleed_wrap = clamp(dot(n, bleed.dir) * 0.5 + 0.5, 0.0, 1.0);
    ambient_lit += bleed.color * bleed.strength * 0.45 * (0.35 + 0.65 * bleed_wrap);
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
    out.lit_color = base.rgb * lit + emissive;
    out.ambient_color = base.rgb * ambient_lit;
    out.packed_blend_params = draw.packed_blend_params;
    out.world_pos = world.xyz;
    out.normal_ws = normal_ws;
    out.custom_range = draw.custom_params;
    out.uv = blended.uv;
    out.packed_bleed = draw.packed_bleed;
    out.packed_pbr_params_0 = draw.packed_pbr_params_0;
    out.packed_material_params = draw.packed_material_params;
    out.packed_color = draw.packed_color;
    out.packed_emissive = draw.packed_emissive;
    return out;
}

fn perro_fetch_instance(instance_index: u32) -> InstanceInput {
    let src = visible_indices[instance_index];
    let raw = multimesh_instances[src];
    let rot_xy = perro_unpack_snorm16_pair(raw.rot_xy);
    let rot_zw = perro_unpack_snorm16_pair(raw.rot_zw);
    return InstanceInput(
        vec3<f32>(raw.px, raw.py, raw.pz),
        vec4<f32>(rot_xy.x, rot_xy.y, rot_zw.x, rot_zw.y),
        vec3<f32>(raw.sx, raw.sy, raw.sz),
        raw.draw_id,
        raw.blend_meta_id,
    );
}

// ---- Frame globals for custom shaders ----------------------------------
// Seconds since app start; wraps every hour to stay f32-precise.
// Seconds covered by the previous frame.
// Frames rendered since app start (wraps with f32 precision).
// 0..1 sawtooth over 60 seconds; precision-safe looping animation driver.
// Viewport size in pixels.
// 1 / viewport size.
@vertex
fn vs_main(
    v: VertexInput,
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    let inst = perro_fetch_instance(instance_index);
    return perro_multimesh_vs_main_base(v, inst, vertex_index);
}

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {
    return shade_standard_multimesh(in);
}

// Depth-prepass entry: position only (opaque multimesh has no alpha cutout).
struct DepthOnlyOutput {
    @builtin(position) clip_pos: vec4<f32>,
};

@vertex
fn vs_depth(
    v: VertexInput,
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> DepthOnlyOutput {
    let inst = perro_fetch_instance(instance_index);
    let draw = multimesh_draws[inst.draw_id];
    let scale = bitcast<f32>(draw.scale_bits);
    let rot = normalize(inst.rotation);
    let blended = perro_apply_blend_shapes(v, inst, vertex_index);
    let local_pos = perro_rotate_vec_by_quat(blended.pos * (inst.scale * scale), rot) + inst.position;
    let p = vec4<f32>(local_pos, 1.0);
    let world = vec4<f32>(
        dot(draw.model_row_0, p),
        dot(draw.model_row_1, p),
        dot(draw.model_row_2, p),
        1.0,
    );
    var out: DepthOnlyOutput;
    out.clip_pos = scene.view_proj * world;
    return out;
}

@fragment
fn fs_depth() {
}

@fragment
fn fs_mask(in: VertexOutput) -> @location(0) u32 {
    return mesh_blend_mask_id.x;
}
