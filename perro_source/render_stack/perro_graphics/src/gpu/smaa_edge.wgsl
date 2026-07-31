// SMAA 1x pass 1/3: luma edge detection.
//
// Runs on the tonemapped LDR present output (same input the FXAA pass would
// read) and writes the two-channel edges mask (R = edge on the left border,
// G = edge on the top border) into an RG8 target. Port of the standard SMAA
// luma edge detection (Jimenez et al., "SMAA: Enhanced Subpixel Morphological
// Antialiasing"), with two engine-specific choices:
//
// - Luma source: the GREEN channel, consistent with the FXAA pass (see
//   fxaa.wgsl). Green dominates perceived luminance (~0.72 of the Rec.709
//   weight vector), so it is the standard cheap luma proxy; only strongly
//   red/blue-on-black edges lose a little detection accuracy.
// - Min-work gating: pixels below the edge threshold return a zero mask after
//   3 taps, so the (branching) blending-weight pass early-outs on them. This
//   replaces the reference stencil optimization without needing a
//   depth-stencil target.

@group(0) @binding(0) var color_tex: texture_2d<f32>;
@group(0) @binding(1) var point_sampler: sampler;

// Minimum luma delta against a neighbor to call the border an edge. Standard
// SMAA threshold: 0.1 catches visible aliasing while skipping film-grain
// level contrast (the reference "high" preset value).
const SMAA_THRESHOLD: f32 = 0.1;
// Local-contrast adaptation: an edge is discarded when a neighboring delta is
// more than 2x stronger, because the stronger edge visually masks it (human
// contrast masking). Standard SMAA factor.
const SMAA_LOCAL_CONTRAST_ADAPTATION_FACTOR: f32 = 2.0;

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

fn luma_at(uv: vec2<f32>) -> f32 {
    // Green-as-luma; see header comment.
    return textureSampleLevel(color_tex, point_sampler, uv, 0.0).g;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec2<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(color_tex, 0));
    let uv = in.uv;

    let luma_center = luma_at(uv);
    let luma_left = luma_at(uv + vec2<f32>(-1.0, 0.0) * texel);
    let luma_top = luma_at(uv + vec2<f32>(0.0, -1.0) * texel);

    let delta_lt = abs(vec2<f32>(luma_center) - vec2<f32>(luma_left, luma_top));
    var edges = step(vec2<f32>(SMAA_THRESHOLD), delta_lt);

    // Early exit: no edge crosses this pixel's left/top borders.
    if edges.x == 0.0 && edges.y == 0.0 {
        return vec2<f32>(0.0, 0.0);
    }

    let luma_right = luma_at(uv + vec2<f32>(1.0, 0.0) * texel);
    let luma_bottom = luma_at(uv + vec2<f32>(0.0, 1.0) * texel);
    let delta_rb = abs(vec2<f32>(luma_center) - vec2<f32>(luma_right, luma_bottom));
    var max_delta = max(delta_lt, delta_rb);

    let luma_leftleft = luma_at(uv + vec2<f32>(-2.0, 0.0) * texel);
    let luma_toptop = luma_at(uv + vec2<f32>(0.0, -2.0) * texel);
    let delta_ll = abs(vec2<f32>(luma_left, luma_top) - vec2<f32>(luma_leftleft, luma_toptop));
    max_delta = max(max_delta, delta_ll);
    let final_delta = max(max_delta.x, max_delta.y);

    // Local contrast adaptation: drop edges masked by a much stronger
    // neighboring edge.
    edges = edges * step(vec2<f32>(final_delta), SMAA_LOCAL_CONTRAST_ADAPTATION_FACTOR * delta_lt);
    return edges;
}
