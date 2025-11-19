// ---------------------------------------------------------------
// basic3d.wgsl — Multi-light support (Directional + Omni ready)
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

const MAX_LIGHTS: u32 = 16u;

// Bind groups
@group(0) @binding(0)
var<uniform> camera: Camera3D;

// You now have an array, not a single light
@group(1) @binding(0)
var<uniform> lights: array<Light, MAX_LIGHTS>;

// ------------------------------------
// Vertex stage
// ------------------------------------
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

// ------------------------------------
// Fragment stage
// ------------------------------------
@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
    let N = normalize(in.world_normal);
    var lighting = vec3<f32>(0.0);

    // Compute the camera position once
    let view_inv = inverse_view_matrix(camera.view);
    let camera_pos = view_inv[3].xyz;

    // === LOOP THROUGH ALL LIGHTS ===
    for (var i: u32 = 0u; i < MAX_LIGHTS; i++) {
        let L = lights[i];

        // skip empty (zero-intensity) lights
        if (L.intensity <= 0.0001) {
            continue;
        }

        // Direction from fragment → light
        let light_dir = normalize(L.position - in.world_pos);
        // Diffuse
        let diff = max(dot(N, light_dir), 0.0);
        // View + specular
        let V = normalize(camera_pos - in.world_pos);
        let H = normalize(light_dir + V);
        let spec = pow(max(dot(N, H), 0.0), 32.0);

        let diffuse = diff * L.color * L.intensity;
        let specular = spec * 0.5 * L.color * L.intensity;
        let ambient = L.ambient * 0.3;

        lighting += ambient + diffuse + specular;
    }

    // Give each mesh a simple base color by material_id (for debug)
    var base_color = vec3<f32>(1.0, 0.45, 0.1);
    if (in.material_id == 1u) { base_color = vec3<f32>(0.2, 0.6, 1.0); }
    if (in.material_id == 2u) { base_color = vec3<f32>(0.3, 0.9, 0.3); }

    let final_color = base_color * lighting;
    return vec4<f32>(final_color, 1.0);
}

// ------------------------------------
// Helper: invert view (extract camera pos)
// ------------------------------------
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