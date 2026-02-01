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

    let t = -(r_t * view[3].xyz);
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

// OPTIMIZED: Pre-compute PI constant
const PI: f32 = 3.14159265359;
const INV_PI: f32 = 0.31830988618; // 1.0 / PI

fn distribution_ggx(NdotH: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = (NdotH * NdotH) * (a2 - 1.0) + 1.0;
    // OPTIMIZED: Use pre-computed PI constant
    return a2 / (PI * denom * denom);
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
    // OPTIMIZED: Normalize once, reuse
    let N = normalize(in.world_normal);
    var lighting = vec3<f32>(0.0);

    // OPTIMIZED: Cache camera position calculation (only compute once)
    let view_inv = inverse_view_matrix(camera.view);
    let camera_pos = view_inv[3].xyz;
    let V = normalize(camera_pos - in.world_pos);
    
    // OPTIMIZED: Pre-compute NdotV once (used in multiple places)
    let NdotV = max(dot(N, V), 0.0);
    
    // OPTIMIZED: Early exit if facing away from camera (backface culling in fragment)
    // Note: This is optional - vertex culling is usually better, but helps with transparent objects
    // if (NdotV < 0.001) { return vec4<f32>(0.0, 0.0, 0.0, 0.0); }

    // Soft hemispheric ambient
    let hemi_top = vec3<f32>(0.38, 0.45, 0.55);
    let hemi_bottom = vec3<f32>(0.05, 0.05, 0.07);
    let hemi = mix(hemi_bottom, hemi_top, N.y * 0.5 + 0.5);
    lighting += hemi * 0.1;

    // OPTIMIZED: Pre-compute F0 once (doesn't depend on light)
    let F0 = mix(vec3<f32>(0.04), mat.base_color.rgb, vec3<f32>(mat.metallic));
    let one_minus_metallic = 1.0 - mat.metallic;

    // Lights
    for (var i: u32 = 0u; i < MAX_LIGHTS; i++) {
        let L = lights[i];
        // OPTIMIZED: Early exit for zero-intensity lights (moved to top)
        if (L.intensity <= 0.0001) { continue; }

        // OPTIMIZED: Pre-compute lenL once
        let lenL = length(L.position);
        let is_directional = abs(lenL - 1.0) < 0.01;
        let is_spot = lenL > 1.5;

        var Ldir: vec3<f32>;
        var attenuation: f32 = 1.0;

        if (is_directional) {
            // OPTIMIZED: For directional lights, normalize once
            Ldir = normalize(-L.position);
        } else {
            // OPTIMIZED: Compute distance squared first (cheaper than length)
            let light_vec = in.world_pos - L.position;
            let dist_sq = dot(light_vec, light_vec);
            let dist = sqrt(dist_sq);
            Ldir = -light_vec / dist;
            // OPTIMIZED: Use dist_sq directly for attenuation
            attenuation = 1.0 / (1.0 + dist_sq * 0.1);
        }

        // OPTIMIZED: Early exit if light doesn't contribute (NdotL <= 0)
        let NdotL = max(dot(N, Ldir), 0.0);
        if (NdotL <= 0.001) { continue; }

        // Base BRDF
        let H = normalize(V + Ldir);
        let NdotH = max(dot(N, H), 0.0);
        let VdotH = max(dot(V, H), 0.0);

        // OPTIMIZED: Reuse pre-computed F0 and NdotV
        let D = distribution_ggx(NdotH, mat.roughness);
        let G = geometry_smith(NdotV, NdotL, mat.roughness);
        let F = fresnel_schlick_roughness(VdotH, F0, mat.roughness);

        let numerator = D * G * F;
        let denom = max(4.0 * NdotV * NdotL, 0.001);
        let specular = numerator / denom;
        let kS = F;
        // OPTIMIZED: Reuse pre-computed one_minus_metallic
        let kD = (vec3<f32>(1.0) - kS) * one_minus_metallic;

        // OPTIMIZED: Reuse pre-computed INV_PI constant
        var light_contrib = (kD * mat.base_color.rgb * INV_PI + specular)
            * (L.color * L.intensity) * NdotL * attenuation;

        // Spot cone
        if (is_spot) {
            // OPTIMIZED: Pre-compute spot direction once
            let spot_dir = normalize(-L.position);
            let frag_dir = normalize(in.world_pos - (normalize(L.position) * (lenL - 1.0)));
            let cos_theta = dot(spot_dir, -frag_dir);
            // OPTIMIZED: Pre-compute cos values (could be uniforms, but constants are fine)
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