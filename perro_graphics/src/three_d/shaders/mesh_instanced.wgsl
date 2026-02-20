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
    point_lights: array<PointLightGpu, MAX_POINT_LIGHTS>,
    spot_lights: array<SpotLightGpu, MAX_SPOT_LIGHTS>,
}

@group(0) @binding(0)
var<uniform> scene: Scene3D;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct InstanceInput {
    @location(2) model_0: vec4<f32>,
    @location(3) model_1: vec4<f32>,
    @location(4) model_2: vec4<f32>,
    @location(5) model_3: vec4<f32>,
    @location(6) color: vec4<f32>,
    @location(7) pbr_params: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal_ws: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) pbr_params: vec4<f32>,
};

@vertex
fn vs_main(v: VertexInput, inst: InstanceInput) -> VertexOutput {
    let model = mat4x4<f32>(inst.model_0, inst.model_1, inst.model_2, inst.model_3);
    let world = model * vec4<f32>(v.pos, 1.0);
    let normal_ws = normalize((model * vec4<f32>(v.normal, 0.0)).xyz);

    var out: VertexOutput;
    out.clip_pos = scene.view_proj * world;
    out.world_pos = world.xyz;
    out.normal_ws = normal_ws;
    out.color = inst.color;
    out.pbr_params = inst.pbr_params;
    return out;
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

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - cos_theta, 5.0);
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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let albedo = in.color.rgb;
    let n = normalize(in.normal_ws);
    let v = normalize(scene.camera_pos.xyz - in.world_pos);
    let roughness = clamp(in.pbr_params.x, 0.04, 1.0);
    let metallic = clamp(in.pbr_params.y, 0.0, 1.0);
    let ao = clamp(in.pbr_params.z, 0.0, 1.0);
    let emissive = max(in.pbr_params.w, 0.0);

    var light_rgb = vec3<f32>(0.0);

    if scene.ambient_and_counts.w > 0.5 {
        let dir = normalize(scene.ray_light.direction.xyz);
        let l = -dir;
        let radiance = scene.ray_light.color_intensity.xyz * scene.ray_light.color_intensity.w;
        light_rgb += brdf_pbr(albedo, n, v, l, roughness, metallic, radiance);
    }

    let point_count = u32(scene.ambient_and_counts.y);
    for (var i = 0u; i < point_count; i = i + 1u) {
        let light = scene.point_lights[i];
        let to_light = light.position_range.xyz - in.world_pos;
        let dist_sq = max(dot(to_light, to_light), 1.0e-6);
        let dist = sqrt(dist_sq);
        let range = max(light.position_range.w, 1.0e-4);
        let attenuation = max(1.0 - dist / range, 0.0);
        let l = to_light / dist;
        let radiance = light.color_intensity.xyz * light.color_intensity.w * attenuation;
        light_rgb += brdf_pbr(albedo, n, v, l, roughness, metallic, radiance);
    }

    let spot_count = u32(scene.ambient_and_counts.z);
    for (var i = 0u; i < spot_count; i = i + 1u) {
        let light = scene.spot_lights[i];
        let to_surface = in.world_pos - light.position_range.xyz;
        let dist_sq = max(dot(to_surface, to_surface), 1.0e-6);
        let dist = sqrt(dist_sq);
        let range = max(light.position_range.w, 1.0e-4);
        let attenuation = max(1.0 - dist / range, 0.0);

        let l = -to_surface / dist;

        let cone_dir = normalize(light.direction_outer_cos.xyz);
        let cos_theta = dot(-l, cone_dir);
        let outer = light.direction_outer_cos.w;
        let inner = light.inner_cos_pad.x;
        let cone = clamp((cos_theta - outer) / max(inner - outer, 1.0e-4), 0.0, 1.0);

        let radiance = light.color_intensity.xyz * light.color_intensity.w * attenuation * cone;
        light_rgb += brdf_pbr(albedo, n, v, l, roughness, metallic, radiance);
    }

    let ambient = albedo * scene.ambient_color.xyz * scene.ambient_color.w * ao;
    let color = ambient + light_rgb + albedo * emissive;
    return vec4<f32>(color, in.color.a);
}
