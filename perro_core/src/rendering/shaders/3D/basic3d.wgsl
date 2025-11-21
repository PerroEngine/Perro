// ---------------------------------------------------------------
// basic3d.wgsl — Enhanced PBR (Directional, Point & Spot Support)
// ---------------------------------------------------------------
//
// Compatible with current LightUniform, MaterialUniform, Camera3DUniform.
// Supports Directional (sun), Point (Omni), and Spot lights with cone falloff.
// Adds Hemispheric ambient, ACES filmic tonemap, and exponential fog.
//
// ---------------------------------------------------------------

struct Camera3D {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
};

struct Light {
    position: vec3<f32>,
    _pad0: f32,
    color: vec3<f32>,
    intensity: f32,
    ambient: vec3<f32>,
    _pad1: f32,
};

struct Material {
    base_color: vec4<f32>,
    metallic: f32,
    roughness: f32,
    _pad0: vec2<f32>,
    emissive: vec4<f32>,
};

const MAX_LIGHTS: u32 = 16u;
const MAX_MATERIALS: u32 = 64u;

@group(0) @binding(0)
var<uniform> camera: Camera3D;
@group(1) @binding(0)
var<uniform> lights: array<Light, MAX_LIGHTS>;
@group(2) @binding(0)
var<uniform> materials: array<Material, MAX_MATERIALS>;

// ---------------------------------------------------------------
// Vertex stage
// ---------------------------------------------------------------
struct VSIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) model_row0: vec4<f32>,
    @location(3) model_row1: vec4<f32>,
    @location(4) model_row2: vec4<f32>,
    @location(5) model_row3: vec4<f32>,
    @location(6) material_id: u32,
};

struct VSOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) material_id: u32,
};

@vertex
fn vs_main(in: VSIn) -> VSOut {
    var out: VSOut;

    let model = mat4x4<f32>(
        in.model_row0,
        in.model_row1,
        in.model_row2,
        in.model_row3
    );

    let world_pos = model * vec4<f32>(in.position, 1.0);
    out.world_pos = world_pos.xyz;
    out.pos = camera.projection * camera.view * world_pos;

    out.world_normal = normalize((model * vec4<f32>(in.normal, 0.0)).xyz);
    out.material_id = in.material_id;
    return out;
}

// ---------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------
fn inverse_view_matrix(view: mat4x4<f32>) -> mat4x4<f32> {
    let r = mat3x3<f32>(view[0].xyz, view[1].xyz, view[2].xyz);
    let r_t = transpose(r);
    let t = -r_t * view[3].xyz;
    return mat4x4<f32>(
        vec4<f32>(r_t[0], 0.0),
        vec4<f32>(r_t[1], 0.0),
        vec4<f32>(r_t[2], 0.0),
        vec4<f32>(t, 1.0)
    );
}

fn fresnel_schlick_roughness(cos_theta: f32, F0: vec3<f32>, roughness: f32) -> vec3<f32> {
    return F0 + (max(vec3<f32>(1.0 - roughness), F0) - F0) * pow(1.0 - cos_theta, 5.0);
}

fn distribution_ggx(NdotH: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = (NdotH * NdotH) * (a2 - 1.0) + 1.0;
    return a2 / (3.14159 * denom * denom);
}

fn geometry_schlick_ggx(NdotV: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return NdotV / (NdotV * (1.0 - k) + k);
}

fn geometry_smith(NdotV: f32, NdotL: f32, roughness: f32) -> f32 {
    return geometry_schlick_ggx(NdotV, roughness) * geometry_schlick_ggx(NdotL, roughness);
}

fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// ---------------------------------------------------------------
// Fragment stage
// ---------------------------------------------------------------
@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
    let mat = materials[in.material_id];
    let N = normalize(in.world_normal);
    var lighting = vec3<f32>(0.0);

    // Camera world pos
    let view_inv = inverse_view_matrix(camera.view);
    let camera_pos = view_inv[3].xyz;
    let V = normalize(camera_pos - in.world_pos);

    // Soft hemispheric ambient
    let hemi_top = vec3<f32>(0.38, 0.45, 0.55);
    let hemi_bottom = vec3<f32>(0.05, 0.05, 0.07);
    let hemi = mix(hemi_bottom, hemi_top, N.y * 0.5 + 0.5);
    lighting += hemi * 0.1;

    // Lights
    for (var i: u32 = 0u; i < MAX_LIGHTS; i++) {
        let L = lights[i];
        if (L.intensity <= 0.0001) { continue; }

        let lenL = length(L.position);
        let is_directional = abs(lenL - 1.0) < 0.01;
        let is_spot = lenL > 1.5;

        var Ldir: vec3<f32>;
        var attenuation: f32 = 1.0;

        if (is_directional) {
            Ldir = normalize(-L.position);
        } else {
            let light_vec = in.world_pos - L.position;
            let dist = length(light_vec);
            Ldir = -light_vec / dist;
            attenuation = 1.0 / (1.0 + dist * dist * 0.1);
        }

        // Base BRDF
        let H = normalize(V + Ldir);
        let NdotL = max(dot(N, Ldir), 0.0);
        let NdotV = max(dot(N, V), 0.0);
        let NdotH = max(dot(N, H), 0.0);
        let VdotH = max(dot(V, H), 0.0);

        let F0 = mix(vec3<f32>(0.04), mat.base_color.rgb, vec3<f32>(mat.metallic));
        let D = distribution_ggx(NdotH, mat.roughness);
        let G = geometry_smith(NdotV, NdotL, mat.roughness);
        let F = fresnel_schlick_roughness(VdotH, F0, mat.roughness);

        let numerator = D * G * F;
        let denom = max(4.0 * NdotV * NdotL, 0.001);
        let specular = numerator / denom;
        let kS = F;
        let kD = (vec3<f32>(1.0) - kS) * (1.0 - mat.metallic);

        var light_contrib = (kD * mat.base_color.rgb / 3.14159 + specular)
            * (L.color * L.intensity) * NdotL * attenuation;

        // Spot cone
        if (is_spot) {
            let spot_dir = normalize(-L.position);
            let frag_dir = normalize(in.world_pos - (normalize(L.position) * (lenL - 1.0)));
            let cos_theta = dot(spot_dir, -frag_dir);
            let inner = cos(radians(25.0));
            let outer = cos(radians(45.0));
            let epsilon = inner - outer;
            let factor = clamp((cos_theta - outer) / epsilon, 0.0, 1.0);
            light_contrib *= factor;
        }

        lighting += light_contrib;
        lighting += L.ambient * 0.1;
    }

    // Emission
    lighting += mat.emissive.rgb;

    // Fog
    let dist = length(camera_pos - in.world_pos);
    let fog_density = 0.02;
    let fog = exp(-pow(dist * fog_density, 2.0));
    let fog_col = vec3<f32>(0.7, 0.8, 1.0);

    // Tone map + gamma
    var col = aces_tonemap(lighting);
    col = pow(col, vec3<f32>(1.0 / 2.2));
    col = mix(fog_col, col, fog);

    return vec4<f32>(col, 1.0);
}