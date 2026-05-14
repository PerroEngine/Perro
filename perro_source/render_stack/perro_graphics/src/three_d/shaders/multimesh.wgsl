const MAX_RAY_LIGHTS: u32 = 3u;

struct RayLightGpu {
    direction: vec4<f32>,
    color_intensity: vec4<f32>,
}

struct Scene3D {
    view_proj: mat4x4<f32>,
    ambient_and_counts: vec4<f32>,
    camera_pos: vec4<f32>,
    ambient_color: vec4<f32>,
    ray_light: RayLightGpu,
    ray_lights: array<RayLightGpu, MAX_RAY_LIGHTS>,
}

struct MultiMeshDrawParam {
    model_row_0: vec4<f32>,
    model_row_1: vec4<f32>,
    model_row_2: vec4<f32>,
    packed_color: u32,
    packed_emissive: u32,
    scale_bits: u32,
    packed_blend_params: u32,
}

@group(0) @binding(0)
var<uniform> scene: Scene3D;
@group(0) @binding(1)
var<storage, read> multimesh_draws: array<MultiMeshDrawParam>;
@group(0) @binding(2)
var mesh_blend_depth_tex: texture_depth_2d;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

struct InstanceInput {
    @location(4) position: vec3<f32>,
    @location(5) rotation: vec4<f32>,
    @location(6) draw_id: u32,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) lit_color: vec3<f32>,
    @location(1) @interpolate(flat) packed_blend_params: u32,
};

struct FragmentInput {
    @location(0) lit_color: vec3<f32>,
    @location(1) @interpolate(flat) packed_blend_params: u32,
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

fn mesh_blend_noise(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

fn mesh_blend_alpha(frag_pos: vec4<f32>, packed: u32) -> f32 {
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
    let max_width = max(params.x * 0.01, 0.00001);
    let min_width = min(params.y * 0.01, max_width);
    var noise = 0.0;
    if params.z > 0.0 {
        let tile = max(params.w, 1.0);
        noise = (mesh_blend_noise(floor(frag_pos.xy / tile)) - 0.5) * params.z * max_width;
    }
    let depth_delta = abs(frag_pos.z - scene_depth) + noise;
    return smoothstep(min_width, max_width, depth_delta);
}

fn rotate_vec_by_quat(v: vec3<f32>, q: vec4<f32>) -> vec3<f32> {
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

@vertex
fn vs_main(v: VertexInput, inst: InstanceInput) -> VertexOutput {
    let draw = multimesh_draws[inst.draw_id];
    let scale = bitcast<f32>(draw.scale_bits);
    let rot = normalize(inst.rotation);
    let local_pos = rotate_vec_by_quat(v.pos * scale, rot) + inst.position;
    let local_nrm = rotate_vec_by_quat(v.normal, rot);
    let p = vec4<f32>(local_pos, 1.0);
    let world = vec4<f32>(
        dot(draw.model_row_0, p),
        dot(draw.model_row_1, p),
        dot(draw.model_row_2, p),
        1.0,
    );
    let normal_ws = normalize(vec3<f32>(
        dot(draw.model_row_0.xyz, local_nrm),
        dot(draw.model_row_1.xyz, local_nrm),
        dot(draw.model_row_2.xyz, local_nrm),
    ));

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
    return out;
}

@fragment
fn fs_main(in: FragmentInput, @builtin(position) frag_pos: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(in.lit_color, mesh_blend_alpha(frag_pos, in.packed_blend_params));
}
