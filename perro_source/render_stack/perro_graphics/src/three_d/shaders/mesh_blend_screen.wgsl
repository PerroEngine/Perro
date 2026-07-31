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
// Two vec4 slots per blend id, indexed id * 2 + k.
// Slot 0: x = seam width (world), y = min width (inner full-blend band
// control, see the band mapping in fs_main), z = noise factor, w = world noise
// tile size.
// Slot 1: x = slope falloff exponent (0 = slope falloff off), y = overall
// blend strength 0..1, z/w reserved.
@group(0) @binding(4)
var<storage, read> blend_id_params: array<vec4<f32>>;

// Ids 1..=127 are blend sources, 128..=255 are receivers; a seam needs at
// least one source side.
const MESH_BLEND_RECEIVER_ID_BASE: u32 = 128u;

// Two rings of 8 taps; inner ring pulls the blend tight to the seam, outer
// ring feathers it out.
const SEAM_TAP_COUNT: u32 = 16u;

// ---- Seam tuning ----------------------------------------------------------
// Minimum on-screen seam footprint in pixels (plugin MinSize behavior): far
// seams hold roughly this much blend instead of fading out with distance.
const MIN_SEAM_PX: f32 = 3.0;
// Anti-shimmer fade window over the *authored* pair width in pixels; only
// genuinely sub-pixel seams fade out.
const SUBPX_FADE_LO: f32 = 0.75;
const SUBPX_FADE_HI: f32 = 1.5;
// Tap-ring radius clamp in pixels.
const R_PX_MIN: f32 = 2.0;
const R_PX_MAX: f32 = 20.0;
// The noise dissolve needs a few px of seam; below this window it fades to a
// clean gradient so distance never produces chunky blotches.
const NOISE_FADE_PX_LO: f32 = 4.0;
const NOISE_FADE_PX_HI: f32 = 12.0;
// Noise features never shrink under this many pixels.
const NOISE_MIN_FEATURE_PX: f32 = 8.0;
// Cross-blend cap: base keeps clean seams a <=50/50 wash; raggedness may swap
// more, and the core of a wide on-screen dissolve may swap up to 100%.
const BLEND_MAX_BASE: f32 = 0.5;
const BLEND_MAX_RAGGED: f32 = 0.35;
const BLEND_MAX_WIDE: f32 = 0.15;
const WIDE_SEAM_PX_LO: f32 = 12.0;
const WIDE_SEAM_PX_HI: f32 = 28.0;
// Depth tolerance: taps whose view distance differs more than
// max(width * WIDTH_FACTOR, world_per_px * PX_FACTOR) belong to another
// surface and are ignored.
const DEPTH_TOL_WIDTH_FACTOR: f32 = 2.0;
const DEPTH_TOL_PX_FACTOR: f32 = 10.0;
// Tap weight falloff: w = 1 - len^2 * TAP_WEIGHT_FALLOFF.
const TAP_WEIGHT_FALLOFF: f32 = 0.6;
// Band mapping (min width wiring): band = mix(BAND_CLEAN, BAND_RAGGED, rag)
// scaled by the min-width/width ratio; DEFAULT_MIN_WIDTH_RATIO is the
// authoring default (min_distance 0.03 / distance 0.6), at which the scale is
// exactly 1.0 so legacy scenes keep their look.
const BAND_CLEAN: f32 = 0.42;
const BAND_RAGGED: f32 = 0.15;
const DEFAULT_MIN_WIDTH_RATIO: f32 = 0.05;
// Slope falloff floor (the exponent itself is authored per id, slot 1 x;
// plugin SlopeFactor default 2.0): parallel surfaces blend fully,
// near-perpendicular contacts pull back to the floor. The floor keeps the
// term subtle so wall/floor seams still blend visibly.
const SLOPE_FALLOFF_FLOOR: f32 = 0.4;
// Per-seam noise seed offset (world noise cells per id) so identical
// instances do not share one dissolve pattern.
const NOISE_ID_OFFSET: vec2<f32> = vec2<f32>(17.17, 31.71);

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

fn world_at(coord: vec2<i32>, dims: vec2<f32>) -> vec3<f32> {
    let depth = textureLoad(scene_depth_tex, coord, 0);
    return world_from_depth(vec2<f32>(coord), dims, depth);
}

// Approximate world-space surface normal from depth derivatives at the +x/+y
// neighbor pixels. Returns vec3(0) when a neighbor jumps to another surface
// (depth discontinuity); the caller then skips the slope falloff instead of
// trusting a bogus normal.
fn depth_normal(
    coord: vec2<i32>,
    dims_i: vec2<i32>,
    dims: vec2<f32>,
    world: vec3<f32>,
    max_step: f32,
) -> vec3<f32> {
    let cx = min(coord + vec2<i32>(1, 0), dims_i - vec2<i32>(1));
    let cy = min(coord + vec2<i32>(0, 1), dims_i - vec2<i32>(1));
    let dx = world_at(cx, dims) - world;
    let dy = world_at(cy, dims) - world;
    if length(dx) > max_step || length(dy) > max_step {
        return vec3<f32>(0.0);
    }
    let n = cross(dx, dy);
    let len = length(n);
    if len < 1.0e-8 {
        return vec3<f32>(0.0);
    }
    return n / len;
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
    let params_c = blend_id_params[id_c * 2u];
    // Upper-bound width estimate for the tap radius and early-outs. Sources
    // carry their own authored width; receivers carry the widest source's
    // (see prepare_mesh_blend_screen) purely so the search ring reaches far
    // enough. The width actually blended with is the pair width found below,
    // which can only be <= this estimate.
    let width_est = max(params_c.x, 0.0001);
    // World units per pixel at this depth, for a distance-stable seam size.
    let world_step = world_from_depth(coord_f + vec2<f32>(1.0, 0.0), dims, depth_c);
    let world_per_px = max(distance(world_c, world_step), 1.0e-5);
    let est_px = width_est / world_per_px;
    // Sub-pixel early-out: the pair width is <= the estimate, so a sub-pixel
    // estimate means the final anti-shimmer fade is zero anyway.
    if est_px < SUBPX_FADE_LO {
        return center;
    }
    // Distance floor (plugin MinSize): far seams keep at least MIN_SEAM_PX of
    // on-screen blend instead of fading out with distance.
    let r_px = clamp(max(est_px, MIN_SEAM_PX), R_PX_MIN, R_PX_MAX);
    let depth_tolerance =
        max(width_est * DEPTH_TOL_WIDTH_FACTOR, world_per_px * DEPTH_TOL_PX_FACTOR);
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
    // Dominant opposing sample: the highest-weight tap with a different id.
    // Its id supplies the other half of the pair params; its position, the
    // opposing surface normal for the slope falloff.
    var opp_best_w = 0.0;
    var opp_id = 0u;
    var opp_coord = coord;
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
        let w = 1.0 - len * len * TAP_WEIGHT_FALLOFF;
        sum_all += w;
        if id_t != id_c {
            sum_opp += w;
            col_opp += textureLoad(scene_color_tex, tap, 0).rgb * w;
            if w > opp_best_w {
                opp_best_w = w;
                opp_id = id_t;
                opp_coord = tap;
            }
        }
    }
    if (sum_opp <= 0.0) || (sum_all <= 0.0) {
        return center;
    }

    // Effective pair params (plugin smallest-size-wins): between two sources
    // the smaller authored width governs the seam; a receiver has no authored
    // params of its own, so the touching source's set is used outright. The
    // rule is symmetric (min / source-side), so both sides of a seam agree on
    // width, noise, and band.
    var eff = params_c;
    var eff_id = id_c;
    if center_is_receiver {
        eff = blend_id_params[opp_id * 2u];
        eff_id = opp_id;
    } else if opp_id < MESH_BLEND_RECEIVER_ID_BASE {
        let params_o = blend_id_params[opp_id * 2u];
        if params_o.x > 0.0 && (params_o.x < eff.x || (params_o.x == eff.x && opp_id < id_c)) {
            eff = params_o;
            eff_id = opp_id;
        }
    }
    // Authored per-pair extras follow the same smallest-of-pair id choice.
    let eff_ext = blend_id_params[eff_id * 2u + 1u];
    let pair_width = max(eff.x, 0.0001);
    let authored_px = pair_width / world_per_px;
    // On-screen footprint, floored at MIN_SEAM_PX so distance does not erase
    // the seam.
    let seam_px = max(authored_px, MIN_SEAM_PX);
    // Anti-shimmer only: the whole effect fades once the *authored* width
    // drops sub-pixel. (This replaces the old smoothstep(2.5, 8.0) fade-out
    // that erased distant seams entirely.)
    let subpx_fade = smoothstep(SUBPX_FADE_LO, SUBPX_FADE_HI, authored_px);
    if subpx_fade <= 0.0 {
        return center;
    }
    // Fraction of the neighborhood on the other side of the seam: ~0.5 on
    // the contact line, falling toward 0 away from it.
    let f = sum_opp / sum_all;
    // Dissolve: world-anchored noise sets a per-pixel threshold so the
    // contact line breaks into interlocking fingers. eff.z is the raggedness
    // (0 = clean gradient, 1 = crisp fingers), eff.w the world-space feature
    // size. The dissolve needs a few px of seam; below the window it fades to
    // a clean gradient, and features never shrink under NOISE_MIN_FEATURE_PX.
    let raggedness =
        clamp(eff.z, 0.0, 1.0) * smoothstep(NOISE_FADE_PX_LO, NOISE_FADE_PX_HI, seam_px);
    var threshold = 0.5;
    if raggedness > 0.0 {
        let tile = max(max(eff.w, 0.05), world_per_px * NOISE_MIN_FEATURE_PX);
        // Seed the pattern with the smaller participating id: identical
        // instances at the same height stop sharing one pattern, while min()
        // stays symmetric so both sides of one seam share the same field and
        // the fingers still interlock.
        let seed = f32(min(id_c, opp_id));
        let p = (world_c.xz + vec2<f32>(world_c.y * 0.53, world_c.y * 0.29)) / tile
            + NOISE_ID_OFFSET * seed;
        let n = seam_noise(p) * 0.65 + seam_noise(p * 2.7 + vec2<f32>(13.7, 41.3)) * 0.35;
        threshold = mix(0.5, 0.15 + n * 0.7, raggedness);
    }

    // Inner-band control (min width, eff.y): the min/width ratio sets how
    // much of the seam is a fully blended core vs. soft falloff. Normalized
    // so the authoring defaults (min 0.03 / distance 0.6 = ratio 0.05) give a
    // scale of 1.0 and reproduce the legacy band mix(0.42, 0.15, raggedness)
    // exactly; a larger min width tightens the transition toward a hard
    // fully-blended core. `narrow` additionally shrinks the band when the
    // pair width is smaller than the search ring (mismatched pair widths), so
    // the visible seam tracks the smaller width instead of the ring radius.
    let min_ratio = clamp(min(eff.y, pair_width) / pair_width, 0.0, 1.0);
    let band_scale = clamp((1.0 - min_ratio) / (1.0 - DEFAULT_MIN_WIDTH_RATIO), 0.0, 1.0);
    let narrow = clamp(seam_px / r_px, 0.0, 1.0);
    let band = max(mix(BAND_CLEAN, BAND_RAGGED, raggedness) * band_scale * narrow, 1.0e-3);

    // Slope falloff: depth-derivative normals at the center and the dominant
    // opposing tap. Parallel surfaces (rock resting on ground) blend fully;
    // near-perpendicular contacts pull back to SLOPE_FALLOFF_FLOOR so walls
    // do not smear onto floors. The exponent is authored per pair (eff_ext.x,
    // slope_factor); 0 turns the falloff off entirely. Skipped when either
    // normal degenerates at a depth discontinuity or screen edge.
    var slope_term = 1.0;
    let slope_factor = eff_ext.x;
    if slope_factor > 0.0 {
        let n_c = depth_normal(coord, dims_i, dims, world_c, depth_tolerance);
        let n_o =
            depth_normal(opp_coord, dims_i, dims, world_at(opp_coord, dims), depth_tolerance);
        if dot(n_c, n_c) > 0.5 && dot(n_o, n_o) > 0.5 {
            let align = clamp(dot(n_c, n_o), 0.0, 1.0);
            slope_term = mix(SLOPE_FALLOFF_FLOOR, 1.0, pow(align, slope_factor));
        }
    }

    // Cap: clean seams stay a <=50/50 wash; raggedness may swap more, and the
    // core of a wide on-screen dissolve may swap the color entirely. The
    // authored strength (eff_ext.y) scales the whole cap; 1.0 is neutral.
    let wide = smoothstep(WIDE_SEAM_PX_LO, WIDE_SEAM_PX_HI, seam_px);
    let blend_max =
        (BLEND_MAX_BASE + BLEND_MAX_RAGGED * raggedness + BLEND_MAX_WIDE * raggedness * wide)
        * subpx_fade * clamp(eff_ext.y, 0.0, 1.0);
    let blend = blend_max * slope_term
        * smoothstep(clamp(threshold - band, 0.02, 1.0), threshold, f);
    return vec4<f32>(mix(center.rgb, col_opp / sum_opp, blend), center.a);
}
