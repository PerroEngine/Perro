// Screen-space mesh blend seam pass. Reads the blend-id mask + scene depth,
// finds pixels near a boundary between two different blend participants, and
// cross-samples the scene color from the other side so both meshes melt into
// each other along the visible intersection edge.

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

// Must match Scene3DUniform on the CPU / Scene3D in the mesh preludes.
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
    ground_color: vec4<f32>,
    sky_horizon_color: vec4<f32>,
    ibl_params: vec4<f32>,
    // Frame globals: x = time seconds (wraps hourly), y = delta seconds,
    // z = frame index, w = 0..1 phase over 60 seconds.
    time_params: vec4<f32>,
    // xy = viewport pixels, zw = 1 / pixels.
    resolution: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> scene: Scene3D;
@group(0) @binding(1)
var scene_color_tex: texture_2d<f32>;
@group(0) @binding(2)
var blend_mask_tex: texture_2d<u32>;
@group(0) @binding(3)
var scene_depth_tex: texture_depth_2d;
// Per blend id: x = seam width (world), y = min width, z = noise factor,
// w = world noise tile size.
@group(0) @binding(4)
var<storage, read> blend_id_params: array<vec4<f32>>;

// Ids 1..=127 are blend sources, 128..=255 are receivers; a seam needs at
// least one source side.
const MESH_BLEND_RECEIVER_ID_BASE: u32 = 128u;

// Two rings of 8 taps; inner ring pulls the blend tight to the seam, outer
// ring feathers it out.
const SEAM_TAP_COUNT: u32 = 16u;

struct SeamVsOut {
    @builtin(position) pos: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> SeamVsOut {
    var out: SeamVsOut;
    let x = select(-1.0, 3.0, vertex_index == 1u);
    let y = select(-1.0, 3.0, vertex_index == 2u);
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    return out;
}

fn world_from_depth(coord: vec2<f32>, dims: vec2<f32>, depth: f32) -> vec3<f32> {
    let uv = (coord + vec2<f32>(0.5)) / dims;
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    let world_h = scene.inv_view_proj * ndc;
    return world_h.xyz / max(abs(world_h.w), 1.0e-5);
}

fn seam_hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

fn seam_noise(p: vec2<f32>) -> f32 {
    let cell = floor(p);
    let local = fract(p);
    let curve = local * local * (3.0 - 2.0 * local);
    let a = seam_hash(cell);
    let b = seam_hash(cell + vec2<f32>(1.0, 0.0));
    let c = seam_hash(cell + vec2<f32>(0.0, 1.0));
    let d = seam_hash(cell + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, curve.x), mix(c, d, curve.x), curve.y);
}

@fragment
fn fs_main(in: SeamVsOut) -> @location(0) vec4<f32> {
    let dims_u = textureDimensions(scene_color_tex);
    let dims_i = vec2<i32>(dims_u);
    let coord = vec2<i32>(floor(in.pos.xy));
    let center = textureLoad(scene_color_tex, coord, 0);
    let id_c = textureLoad(blend_mask_tex, coord, 0).x;
    if id_c == 0u {
        return center;
    }
    let depth_c = textureLoad(scene_depth_tex, coord, 0);
    if depth_c >= 0.999999 {
        return center;
    }
    let dims = vec2<f32>(dims_u);
    let coord_f = vec2<f32>(coord);
    let world_c = world_from_depth(coord_f, dims, depth_c);
    let dist_c = distance(world_c, scene.camera_pos.xyz);
    let params_c = blend_id_params[id_c];
    let width = max(params_c.x, 0.0001);
    // World units per pixel at this depth, for a distance-stable seam size.
    let world_step = world_from_depth(coord_f + vec2<f32>(1.0, 0.0), dims, depth_c);
    let world_per_px = max(distance(world_c, world_step), 1.0e-5);
    // Natural on-screen size of the seam; the effect only ramps up as the
    // camera gets closer and fades out entirely once it would be subpixel
    // noise in the distance.
    let seam_px = width / world_per_px;
    let r_px = clamp(seam_px, 2.0, 20.0);
    let distance_fade = smoothstep(2.5, 8.0, seam_px);
    if distance_fade <= 0.0 {
        return center;
    }
    let depth_tolerance = max(width * 2.0, world_per_px * 10.0);
    let center_is_receiver = id_c >= MESH_BLEND_RECEIVER_ID_BASE;

    // Per-pixel rotation + radius jitter turn the sparse ring into fine
    // grain instead of structured speckle.
    let jitter = seam_hash(vec2<f32>(coord));
    let angle = jitter * 6.2831853;
    let rot_c = cos(angle);
    let rot_s = sin(angle);
    var seam_taps = array<vec2<f32>, 16>(
        vec2<f32>(0.924, 0.383),
        vec2<f32>(0.383, 0.924),
        vec2<f32>(-0.383, 0.924),
        vec2<f32>(-0.924, 0.383),
        vec2<f32>(-0.924, -0.383),
        vec2<f32>(-0.383, -0.924),
        vec2<f32>(0.383, -0.924),
        vec2<f32>(0.924, -0.383),
        vec2<f32>(0.45, 0.19),
        vec2<f32>(0.19, 0.45),
        vec2<f32>(-0.19, 0.45),
        vec2<f32>(-0.45, 0.19),
        vec2<f32>(-0.45, -0.19),
        vec2<f32>(-0.19, -0.45),
        vec2<f32>(0.19, -0.45),
        vec2<f32>(0.45, -0.19),
    );
    var sum_all = 0.0;
    var sum_opp = 0.0;
    var col_opp = vec3<f32>(0.0);
    for (var i = 0u; i < SEAM_TAP_COUNT; i = i + 1u) {
        let base_tap = seam_taps[i];
        let rotated = vec2<f32>(
            base_tap.x * rot_c - base_tap.y * rot_s,
            base_tap.x * rot_s + base_tap.y * rot_c,
        );
        let offset = rotated * r_px * (0.75 + jitter * 0.5);
        let tap = coord + vec2<i32>(round(offset));
        if any(tap < vec2<i32>(0)) || any(tap >= dims_i) {
            continue;
        }
        let id_t = textureLoad(blend_mask_tex, tap, 0).x;
        if id_t == 0u {
            continue;
        }
        // Receiver-receiver boundaries are not seams; one side must be a
        // blend source.
        if center_is_receiver && id_t >= MESH_BLEND_RECEIVER_ID_BASE && id_t != id_c {
            continue;
        }
        let depth_t = textureLoad(scene_depth_tex, tap, 0);
        let world_t = world_from_depth(vec2<f32>(tap), dims, depth_t);
        let dist_t = distance(world_t, scene.camera_pos.xyz);
        if abs(dist_t - dist_c) > depth_tolerance {
            continue;
        }
        let len = length(base_tap);
        let w = 1.0 - len * len * 0.6;
        sum_all += w;
        if id_t != id_c {
            sum_opp += w;
            col_opp += textureLoad(scene_color_tex, tap, 0).rgb * w;
        }
    }
    if (sum_opp <= 0.0) || (sum_all <= 0.0) {
        return center;
    }
    // Fraction of the neighborhood on the other side of the seam: ~0.5 on
    // the contact line, falling toward 0 away from it.
    let f = sum_opp / sum_all;
    // Dissolve: world-anchored noise sets a per-pixel threshold so the
    // contact line breaks into interlocking fingers. noise factor (z) is
    // the raggedness (0 = clean gradient, 1 = crisp fingers), noise tile
    // (w) is the world-space feature size, and the fingers can swap most
    // of the color instead of stopping at a 50/50 wash.
    // Fade the raggedness out as the seam band shrinks on screen, and keep
    // noise features at least ~8 px, so distance never turns the dissolve
    // into chunky blotches; far seams settle into a clean thin gradient.
    let raggedness = clamp(params_c.z, 0.0, 1.0) * smoothstep(4.0, 12.0, seam_px);
    var threshold = 0.5;
    if raggedness > 0.0 {
        let tile = max(max(params_c.w, 0.05), world_per_px * 8.0);
        let p = (world_c.xz + vec2<f32>(world_c.y * 0.53, world_c.y * 0.29)) / tile;
        let n = seam_noise(p) * 0.65 + seam_noise(p * 2.7 + vec2<f32>(13.7, 41.3)) * 0.35;
        threshold = mix(0.5, 0.15 + n * 0.7, raggedness);
    }
    let band = mix(0.42, 0.15, raggedness);
    let blend_max = (0.5 + 0.35 * raggedness) * distance_fade;
    let blend = blend_max
        * smoothstep(clamp(threshold - band, 0.02, 1.0), threshold, f);
    return vec4<f32>(mix(center.rgb, col_opp / sum_opp, blend), center.a);
}
