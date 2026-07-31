// FXAA 3.11-quality pass (PC quality preset, 12-step edge search).
//
// Runs on the tonemapped LDR present output, before the UI composites onto
// the swapchain, and only while the scene sample count is 1 (never on top of
// MSAA). Port of the standard FXAA 3.11 quality algorithm (Timothy Lottes;
// structure follows the well-known quality-preset implementation).
//
// Luma source: the GREEN channel (FXAA_GREEN_AS_LUMA). Green dominates
// perceived luminance (~0.72 of the Rec.709 weight vector), so it is the
// standard cheap luma proxy in FXAA itself; it saves a dot product per tap
// (13+ taps per edge pixel) and avoids needing luma precomputed into alpha
// by the tonemap pass. Only strongly red/blue-on-black edges lose a little
// detection accuracy.
//
// Early exit: pixels whose 3x3 local contrast stays below
// max(EDGE_THRESHOLD_MIN, luma_max * EDGE_THRESHOLD_MAX) return the center
// texel after 5 taps, so flat regions cost almost nothing.

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;

// Minimum absolute local contrast to bother anti-aliasing (dark/flat skip).
const EDGE_THRESHOLD_MIN: f32 = 0.0312;
// Relative contrast threshold: required range as a fraction of local max.
const EDGE_THRESHOLD_MAX: f32 = 0.125;
// Strength of the sub-pixel (single-texel feature) blend.
const SUBPIXEL_QUALITY: f32 = 0.75;
// Fixed maximum edge-search iterations (steps 2..11 after the first probe).
const ITERATIONS: i32 = 12;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(pos[vid], 0.0, 1.0);
    out.uv = (out.pos.xy * vec2<f32>(0.5, -0.5)) + vec2<f32>(0.5, 0.5);
    return out;
}

fn rgb_luma(rgb: vec3<f32>) -> f32 {
    // Green-as-luma; see header comment.
    return rgb.g;
}

fn sample_luma(uv: vec2<f32>) -> f32 {
    return rgb_luma(textureSampleLevel(input_tex, input_sampler, uv, 0.0).rgb);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(input_tex, 0));
    let uv = in.uv;

    let color_center = textureSampleLevel(input_tex, input_sampler, uv, 0.0).rgb;
    let luma_center = rgb_luma(color_center);
    let luma_down = sample_luma(uv + vec2<f32>(0.0, -1.0) * texel);
    let luma_up = sample_luma(uv + vec2<f32>(0.0, 1.0) * texel);
    let luma_left = sample_luma(uv + vec2<f32>(-1.0, 0.0) * texel);
    let luma_right = sample_luma(uv + vec2<f32>(1.0, 0.0) * texel);

    let luma_min = min(luma_center, min(min(luma_down, luma_up), min(luma_left, luma_right)));
    let luma_max = max(luma_center, max(max(luma_down, luma_up), max(luma_left, luma_right)));
    let luma_range = luma_max - luma_min;

    // Early exit: local contrast below threshold => no visible aliasing here.
    if luma_range < max(EDGE_THRESHOLD_MIN, luma_max * EDGE_THRESHOLD_MAX) {
        return vec4<f32>(color_center, 1.0);
    }

    let luma_down_left = sample_luma(uv + vec2<f32>(-1.0, -1.0) * texel);
    let luma_up_right = sample_luma(uv + vec2<f32>(1.0, 1.0) * texel);
    let luma_up_left = sample_luma(uv + vec2<f32>(-1.0, 1.0) * texel);
    let luma_down_right = sample_luma(uv + vec2<f32>(1.0, -1.0) * texel);

    let luma_down_up = luma_down + luma_up;
    let luma_left_right = luma_left + luma_right;
    let luma_left_corners = luma_down_left + luma_up_left;
    let luma_down_corners = luma_down_left + luma_down_right;
    let luma_right_corners = luma_down_right + luma_up_right;
    let luma_up_corners = luma_up_right + luma_up_left;

    let edge_horizontal = abs(-2.0 * luma_left + luma_left_corners)
        + abs(-2.0 * luma_center + luma_down_up) * 2.0
        + abs(-2.0 * luma_right + luma_right_corners);
    let edge_vertical = abs(-2.0 * luma_up + luma_up_corners)
        + abs(-2.0 * luma_center + luma_left_right) * 2.0
        + abs(-2.0 * luma_down + luma_down_corners);
    let is_horizontal = edge_horizontal >= edge_vertical;

    // Two neighbors perpendicular to the edge; pick the steeper gradient side.
    let luma1 = select(luma_left, luma_down, is_horizontal);
    let luma2 = select(luma_right, luma_up, is_horizontal);
    let gradient1 = luma1 - luma_center;
    let gradient2 = luma2 - luma_center;
    let is_1_steepest = abs(gradient1) >= abs(gradient2);
    let gradient_scaled = 0.25 * max(abs(gradient1), abs(gradient2));

    var step_length = select(texel.x, texel.y, is_horizontal);
    var luma_local_average = 0.0;
    if is_1_steepest {
        step_length = -step_length;
        luma_local_average = 0.5 * (luma1 + luma_center);
    } else {
        luma_local_average = 0.5 * (luma2 + luma_center);
    }

    // Shift half a texel toward the edge.
    var current_uv = uv;
    if is_horizontal {
        current_uv.y += step_length * 0.5;
    } else {
        current_uv.x += step_length * 0.5;
    }

    // March along the edge in both directions until the luma delta says the
    // edge ended, with fixed growing step sizes after the first few texels.
    let offset = select(vec2<f32>(0.0, texel.y), vec2<f32>(texel.x, 0.0), is_horizontal);
    var uv1 = current_uv - offset;
    var uv2 = current_uv + offset;

    var luma_end1 = sample_luma(uv1) - luma_local_average;
    var luma_end2 = sample_luma(uv2) - luma_local_average;
    var reached1 = abs(luma_end1) >= gradient_scaled;
    var reached2 = abs(luma_end2) >= gradient_scaled;
    var reached_both = reached1 && reached2;
    if !reached1 {
        uv1 -= offset;
    }
    if !reached2 {
        uv2 += offset;
    }

    if !reached_both {
        var steps = array<f32, 12>(1.0, 1.0, 1.0, 1.0, 1.0, 1.5, 2.0, 2.0, 2.0, 2.0, 4.0, 8.0);
        for (var i: i32 = 2; i < ITERATIONS; i += 1) {
            if !reached1 {
                luma_end1 = sample_luma(uv1) - luma_local_average;
                reached1 = abs(luma_end1) >= gradient_scaled;
            }
            if !reached2 {
                luma_end2 = sample_luma(uv2) - luma_local_average;
                reached2 = abs(luma_end2) >= gradient_scaled;
            }
            reached_both = reached1 && reached2;
            if !reached1 {
                uv1 -= offset * steps[i];
            }
            if !reached2 {
                uv2 += offset * steps[i];
            }
            if reached_both {
                break;
            }
        }
    }

    let distance1 = select(uv.y - uv1.y, uv.x - uv1.x, is_horizontal);
    let distance2 = select(uv2.y - uv.y, uv2.x - uv.x, is_horizontal);
    let is_direction1 = distance1 < distance2;
    let distance_final = min(distance1, distance2);
    let edge_thickness = distance1 + distance2;
    let luma_end = select(luma_end2, luma_end1, is_direction1);

    // Only offset when the edge-end luma variation is consistent with the
    // center being on the darker/brighter side; otherwise we stepped too far.
    let is_luma_center_smaller = luma_center < luma_local_average;
    let correct_variation = (luma_end < 0.0) != is_luma_center_smaller;
    var final_offset = select(
        0.0,
        0.5 - distance_final / max(edge_thickness, 1e-7),
        correct_variation,
    );

    // Sub-pixel anti-aliasing for single-texel features.
    let luma_average = (1.0 / 12.0)
        * (2.0 * (luma_down_up + luma_left_right) + luma_left_corners + luma_right_corners);
    let subpixel_offset1 = clamp(abs(luma_average - luma_center) / luma_range, 0.0, 1.0);
    let subpixel_offset2 = (-2.0 * subpixel_offset1 + 3.0) * subpixel_offset1 * subpixel_offset1;
    let subpixel_offset_final = subpixel_offset2 * subpixel_offset2 * SUBPIXEL_QUALITY;
    final_offset = max(final_offset, subpixel_offset_final);

    var final_uv = uv;
    if is_horizontal {
        final_uv.y += final_offset * step_length;
    } else {
        final_uv.x += final_offset * step_length;
    }
    let rgb = textureSampleLevel(input_tex, input_sampler, final_uv, 0.0).rgb;
    return vec4<f32>(rgb, 1.0);
}
