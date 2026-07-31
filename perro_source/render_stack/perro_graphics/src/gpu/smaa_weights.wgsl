// SMAA 1x pass 2/3: blending-weight calculation.
//
// Reads the RG8 edges mask plus the two SMAA lookup textures (AreaTex
// 160x560 RG8, SearchTex 64x16 R8; both generated procedurally on the CPU,
// see smaa_lut.rs) and writes per-pixel blending weights into an RGBA8
// target. Port of the reference SMAABlendingWeightCalculationPS ("high"
// preset: 16 orthogonal search steps, 8 diagonal steps, 25% corner rounding)
// at subsample index 0 (SMAA 1x has no temporal/spatial subsamples, so the
// subsampleIndices terms of the reference collapse to zero).
//
// Min-work gating: the first fetch is the pixel's own edges mask; pixels
// with no edges return zero weights immediately (the vast majority of the
// frame), so the expensive searches only run on edge pixels. This is the
// branch-based alternative to the reference stencil optimization.
//
// Sampler use is load-bearing:
// - linear_sampler: edge searches exploit bilinear filtering to fetch two
//   edge texels per tap (the reference @PSEUDO_GATHER4 trick) and AreaTex
//   lookups interpolate between the quadratically compressed distance
//   samples.
// - nearest_sampler: SearchTex is a step-count table; the lookup lands on
//   texel centers by construction and must never blend adjacent entries.

@group(0) @binding(0) var edges_tex: texture_2d<f32>;
@group(0) @binding(1) var area_tex: texture_2d<f32>;
@group(0) @binding(2) var search_tex: texture_2d<f32>;
@group(0) @binding(3) var linear_sampler: sampler;
@group(0) @binding(4) var nearest_sampler: sampler;

// "High" preset search limits: 16 hardware steps cover 2x16+2 = 34 pixel
// lines; 8 diagonal steps cover 20-pixel diagonals.
const SMAA_MAX_SEARCH_STEPS: f32 = 16.0;
const SMAA_MAX_SEARCH_STEPS_DIAG: f32 = 8.0;
// How much sharp corners are kept (25 => blend 75% less on corners).
const SMAA_CORNER_ROUNDING_NORM: f32 = 25.0 / 100.0;
// AreaTex layout constants (fixed by the texture format, not tunable).
const SMAA_AREATEX_MAX_DISTANCE: f32 = 16.0;
const SMAA_AREATEX_MAX_DISTANCE_DIAG: f32 = 20.0;
const SMAA_AREATEX_PIXEL_SIZE: vec2<f32> = vec2<f32>(1.0 / 160.0, 1.0 / 560.0);
const SMAA_SEARCHTEX_SIZE: vec2<f32> = vec2<f32>(66.0, 33.0);
const SMAA_SEARCHTEX_PACKED_SIZE: vec2<f32> = vec2<f32>(64.0, 16.0);

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

fn sample_edges(uv: vec2<f32>) -> vec2<f32> {
    return textureSampleLevel(edges_tex, linear_sampler, uv, 0.0).rg;
}

// Decodes two binary edge values from one bilinear fetch taken 0.25 texels
// off-center (see the reference SMAADecodeDiagBilinearAccess).
fn decode_diag_bilinear_access_2(e_in: vec2<f32>) -> vec2<f32> {
    var e = e_in;
    e.r = e.r * abs(5.0 * e.r - 5.0 * 0.75);
    return round(e);
}

fn decode_diag_bilinear_access_4(e_in: vec4<f32>) -> vec4<f32> {
    var e = e_in;
    let rb = e.rb * abs(5.0 * e.rb - 5.0 * 0.75);
    e.r = rb.x;
    e.b = rb.y;
    return round(e);
}

// Diagonal searches; return (distance, end-criteria, crossing edges).
fn search_diag1(texcoord: vec2<f32>, dir: vec2<f32>, rt: vec4<f32>) -> vec4<f32> {
    var coord = vec4<f32>(texcoord, -1.0, 1.0);
    let t = vec3<f32>(rt.xy, 1.0);
    var e = vec2<f32>(0.0, 0.0);
    while coord.z < SMAA_MAX_SEARCH_STEPS_DIAG - 1.0 && coord.w > 0.9 {
        let stepped = t * vec3<f32>(dir, 1.0) + coord.xyz;
        e = sample_edges(stepped.xy);
        coord = vec4<f32>(stepped, dot(e, vec2<f32>(0.5, 0.5)));
    }
    return vec4<f32>(coord.zw, e);
}

fn search_diag2(texcoord: vec2<f32>, dir: vec2<f32>, rt: vec4<f32>) -> vec4<f32> {
    var coord = vec4<f32>(texcoord, -1.0, 1.0);
    coord.x += 0.25 * rt.x; // See the reference @SearchDiag2Optimization.
    let t = vec3<f32>(rt.xy, 1.0);
    var e = vec2<f32>(0.0, 0.0);
    while coord.z < SMAA_MAX_SEARCH_STEPS_DIAG - 1.0 && coord.w > 0.9 {
        let stepped = t * vec3<f32>(dir, 1.0) + coord.xyz;
        // Fetch both edges at once using bilinear filtering.
        e = decode_diag_bilinear_access_2(sample_edges(stepped.xy));
        coord = vec4<f32>(stepped, dot(e, vec2<f32>(0.5, 0.5)));
    }
    return vec4<f32>(coord.zw, e);
}

// Area for a diagonal distance + crossing edges; diagonal areas live in the
// right half of AreaTex (subsample block 0 only, this is SMAA 1x).
fn area_diag(dist: vec2<f32>, e: vec2<f32>) -> vec2<f32> {
    var texcoord = vec2<f32>(SMAA_AREATEX_MAX_DISTANCE_DIAG) * e + dist;
    texcoord = SMAA_AREATEX_PIXEL_SIZE * texcoord + 0.5 * SMAA_AREATEX_PIXEL_SIZE;
    texcoord.x += 0.5;
    return textureSampleLevel(area_tex, linear_sampler, texcoord, 0.0).rg;
}

fn calculate_diag_weights(texcoord: vec2<f32>, e: vec2<f32>, rt: vec4<f32>) -> vec2<f32> {
    var weights = vec2<f32>(0.0, 0.0);

    // Search the "/" diagonal line ends.
    var d = vec4<f32>(0.0);
    if e.r > 0.0 {
        let r = search_diag1(texcoord, vec2<f32>(-1.0, 1.0), rt);
        d.x = r.x + select(0.0, 1.0, r.w > 0.9);
        d.z = r.y;
    }
    let r2 = search_diag1(texcoord, vec2<f32>(1.0, -1.0), rt);
    d.y = r2.x;
    d.w = r2.y;

    if d.x + d.y > 2.0 { // d.x + d.y + 1 > 3
        // Fetch the crossing edges.
        let coords = vec4<f32>(-d.x + 0.25, d.x, d.y, -d.y - 0.25) * rt.xyxy + texcoord.xyxy;
        let c_xy = textureSampleLevel(edges_tex, linear_sampler, coords.xy, 0.0, vec2<i32>(-1, 0)).rg;
        let c_zw = textureSampleLevel(edges_tex, linear_sampler, coords.zw, 0.0, vec2<i32>(1, 0)).rg;
        let dec = decode_diag_bilinear_access_4(vec4<f32>(c_xy, c_zw));
        let c = vec4<f32>(dec.y, dec.x, dec.w, dec.z);

        // Merge crossing edges at each side; drop them when the search hit
        // the step limit instead of a real line end.
        var cc = vec2<f32>(2.0, 2.0) * c.xz + c.yw;
        cc = select(cc, vec2<f32>(0.0, 0.0), d.zw > vec2<f32>(0.9));

        weights += area_diag(d.xy, cc);
    }

    // Search the "\" diagonal line ends.
    let r3 = search_diag2(texcoord, vec2<f32>(-1.0, -1.0), rt);
    d.x = r3.x;
    d.z = r3.y;
    d.y = 0.0;
    d.w = 0.0;
    if textureSampleLevel(edges_tex, linear_sampler, texcoord, 0.0, vec2<i32>(1, 0)).r > 0.0 {
        let r4 = search_diag2(texcoord, vec2<f32>(1.0, 1.0), rt);
        d.y = r4.x + select(0.0, 1.0, r4.w > 0.9);
        d.w = r4.y;
    }

    if d.x + d.y > 2.0 { // d.x + d.y + 1 > 3
        let coords = vec4<f32>(-d.x, -d.x, d.y, d.y) * rt.xyxy + texcoord.xyxy;
        var c: vec4<f32>;
        c.x = textureSampleLevel(edges_tex, linear_sampler, coords.xy, 0.0, vec2<i32>(-1, 0)).g;
        c.y = textureSampleLevel(edges_tex, linear_sampler, coords.xy, 0.0, vec2<i32>(0, -1)).r;
        let zw = textureSampleLevel(edges_tex, linear_sampler, coords.zw, 0.0, vec2<i32>(1, 0)).gr;
        c.z = zw.x;
        c.w = zw.y;
        var cc = vec2<f32>(2.0, 2.0) * c.xz + c.yw;
        cc = select(cc, vec2<f32>(0.0, 0.0), d.zw > vec2<f32>(0.9));

        weights += area_diag(d.xy, cc).gr;
    }

    return weights;
}

// Number of pixels to add in the last step of the horizontal/vertical
// searches, read from the packed SearchTex (values 0/1/2 stored as n*127).
fn search_length(e: vec2<f32>, offset: f32) -> f32 {
    // Left and right cases each take half the texture horizontally; the
    // texture is stored vertically flipped.
    var scale = SMAA_SEARCHTEX_SIZE * vec2<f32>(0.5, -1.0);
    var bias = SMAA_SEARCHTEX_SIZE * vec2<f32>(offset, 1.0);
    // Scale and bias to access texel centers.
    scale += vec2<f32>(-1.0, 1.0);
    bias += vec2<f32>(0.5, -0.5);
    // Convert from pixel coordinates to texcoords of the cropped texture.
    scale *= 1.0 / SMAA_SEARCHTEX_PACKED_SIZE;
    bias *= 1.0 / SMAA_SEARCHTEX_PACKED_SIZE;
    return textureSampleLevel(search_tex, nearest_sampler, scale * e + bias, 0.0).r;
}

fn search_x_left(texcoord_in: vec2<f32>, end: f32, rt: vec4<f32>) -> f32 {
    var texcoord = texcoord_in;
    var e = vec2<f32>(0.0, 1.0);
    while texcoord.x > end && e.g > 0.8281 && e.r == 0.0 {
        e = sample_edges(texcoord);
        texcoord = -vec2<f32>(2.0, 0.0) * rt.xy + texcoord;
    }
    let offset = -(255.0 / 127.0) * search_length(e, 0.0) + 3.25;
    return rt.x * offset + texcoord.x;
}

fn search_x_right(texcoord_in: vec2<f32>, end: f32, rt: vec4<f32>) -> f32 {
    var texcoord = texcoord_in;
    var e = vec2<f32>(0.0, 1.0);
    while texcoord.x < end && e.g > 0.8281 && e.r == 0.0 {
        e = sample_edges(texcoord);
        texcoord = vec2<f32>(2.0, 0.0) * rt.xy + texcoord;
    }
    let offset = -(255.0 / 127.0) * search_length(e, 0.5) + 3.25;
    return -rt.x * offset + texcoord.x;
}

fn search_y_up(texcoord_in: vec2<f32>, end: f32, rt: vec4<f32>) -> f32 {
    var texcoord = texcoord_in;
    var e = vec2<f32>(1.0, 0.0);
    while texcoord.y > end && e.r > 0.8281 && e.g == 0.0 {
        e = sample_edges(texcoord);
        texcoord = -vec2<f32>(0.0, 2.0) * rt.xy + texcoord;
    }
    let offset = -(255.0 / 127.0) * search_length(e.gr, 0.0) + 3.25;
    return rt.y * offset + texcoord.y;
}

fn search_y_down(texcoord_in: vec2<f32>, end: f32, rt: vec4<f32>) -> f32 {
    var texcoord = texcoord_in;
    var e = vec2<f32>(1.0, 0.0);
    while texcoord.y < end && e.r > 0.8281 && e.g == 0.0 {
        e = sample_edges(texcoord);
        texcoord = vec2<f32>(0.0, 2.0) * rt.xy + texcoord;
    }
    let offset = -(255.0 / 127.0) * search_length(e.gr, 0.5) + 3.25;
    return -rt.y * offset + texcoord.y;
}

// Area for an orthogonal distance pair + crossing edges. The distances are
// passed as sqrt(d) because AreaTex compresses distances quadratically.
fn area(dist: vec2<f32>, e1: f32, e2: f32) -> vec2<f32> {
    // Rounding prevents precision errors of bilinear filtering.
    var texcoord = vec2<f32>(SMAA_AREATEX_MAX_DISTANCE) * round(4.0 * vec2<f32>(e1, e2)) + dist;
    texcoord = SMAA_AREATEX_PIXEL_SIZE * texcoord + 0.5 * SMAA_AREATEX_PIXEL_SIZE;
    return textureSampleLevel(area_tex, linear_sampler, texcoord, 0.0).rg;
}

fn detect_horizontal_corner_pattern(weights_in: vec2<f32>, texcoord: vec4<f32>, d: vec2<f32>) -> vec2<f32> {
    let left_right = step(d.xy, d.yx);
    var rounding = (1.0 - SMAA_CORNER_ROUNDING_NORM) * left_right;
    // Reduce blending for pixels in the center of a line.
    rounding = rounding / (left_right.x + left_right.y);

    var factor = vec2<f32>(1.0, 1.0);
    factor.x -= rounding.x * textureSampleLevel(edges_tex, linear_sampler, texcoord.xy, 0.0, vec2<i32>(0, 1)).r;
    factor.x -= rounding.y * textureSampleLevel(edges_tex, linear_sampler, texcoord.zw, 0.0, vec2<i32>(1, 1)).r;
    factor.y -= rounding.x * textureSampleLevel(edges_tex, linear_sampler, texcoord.xy, 0.0, vec2<i32>(0, -2)).r;
    factor.y -= rounding.y * textureSampleLevel(edges_tex, linear_sampler, texcoord.zw, 0.0, vec2<i32>(1, -2)).r;

    return weights_in * saturate(factor);
}

fn detect_vertical_corner_pattern(weights_in: vec2<f32>, texcoord: vec4<f32>, d: vec2<f32>) -> vec2<f32> {
    let left_right = step(d.xy, d.yx);
    var rounding = (1.0 - SMAA_CORNER_ROUNDING_NORM) * left_right;
    rounding = rounding / (left_right.x + left_right.y);

    var factor = vec2<f32>(1.0, 1.0);
    factor.x -= rounding.x * textureSampleLevel(edges_tex, linear_sampler, texcoord.xy, 0.0, vec2<i32>(1, 0)).g;
    factor.x -= rounding.y * textureSampleLevel(edges_tex, linear_sampler, texcoord.zw, 0.0, vec2<i32>(1, 1)).g;
    factor.y -= rounding.x * textureSampleLevel(edges_tex, linear_sampler, texcoord.xy, 0.0, vec2<i32>(-2, 0)).g;
    factor.y -= rounding.y * textureSampleLevel(edges_tex, linear_sampler, texcoord.zw, 0.0, vec2<i32>(-2, 1)).g;

    return weights_in * saturate(factor);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(edges_tex, 0));
    // Reference SMAA_RT_METRICS: xy = 1/size, zw = size.
    let rt = vec4<f32>(1.0 / dims, dims);
    let texcoord = in.uv;
    let pixcoord = texcoord * rt.zw;

    // Search start offsets (@PSEUDO_GATHER4 sampling positions) and the
    // texcoords where each directional search must give up.
    let offset0 = rt.xyxy * vec4<f32>(-0.25, -0.125, 1.25, -0.125) + texcoord.xyxy;
    let offset1 = rt.xyxy * vec4<f32>(-0.125, -0.25, -0.125, 1.25) + texcoord.xyxy;
    let offset2 = rt.xxyy * (vec4<f32>(-2.0, 2.0, -2.0, 2.0) * SMAA_MAX_SEARCH_STEPS)
        + vec4<f32>(offset0.xz, offset1.yw);

    var weights = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var e = textureSampleLevel(edges_tex, nearest_sampler, texcoord, 0.0).rg;

    // Early out on the (common) no-edge pixel: zero weights, no searches.
    if e.g > 0.0 { // Edge at north.
        // Diagonals have both north and west edges, so searching for them in
        // one of the boundaries is enough; diagonals get priority.
        let diag_weights = calculate_diag_weights(texcoord, e, rt);
        weights = vec4<f32>(diag_weights, 0.0, 0.0);

        if weights.r == -weights.g { // No diagonal found: weights.rg == 0.
            var d: vec2<f32>;

            // Distance + crossing edges to the left...
            let coord_left_x = search_x_left(offset0.xy, offset2.x, rt);
            let cross_y = offset1.y; // texcoord.y - 0.25 * rt.y (@CROSSING_OFFSET)
            d.x = coord_left_x;
            let e1 = textureSampleLevel(edges_tex, linear_sampler, vec2<f32>(coord_left_x, cross_y), 0.0).r;

            // ...and to the right.
            let coord_right_x = search_x_right(offset0.zw, offset2.y, rt);
            d.y = coord_right_x;

            // Convert distances to pixel units.
            d = abs(round(rt.zz * d - pixcoord.xx));
            // AreaTex compresses distances quadratically (see smaa_lut.rs).
            let sqrt_d = sqrt(d);

            let e2 = textureSampleLevel(edges_tex, linear_sampler, vec2<f32>(coord_right_x, cross_y), 0.0, vec2<i32>(1, 0)).r;
            var w_rg = area(sqrt_d, e1, e2);
            w_rg = detect_horizontal_corner_pattern(
                w_rg,
                vec4<f32>(coord_left_x, texcoord.y, coord_right_x, texcoord.y),
                d,
            );
            weights = vec4<f32>(w_rg, weights.zw);
        } else {
            e.r = 0.0; // Skip vertical processing: diagonal found.
        }
    }

    if e.r > 0.0 { // Edge at west.
        var d: vec2<f32>;

        // Distance + crossing edges to the top...
        let coord_top_y = search_y_up(offset1.xy, offset2.z, rt);
        let cross_x = offset0.x; // texcoord.x - 0.25 * rt.x
        d.x = coord_top_y;
        let e1 = textureSampleLevel(edges_tex, linear_sampler, vec2<f32>(cross_x, coord_top_y), 0.0).g;

        // ...and to the bottom.
        let coord_bottom_y = search_y_down(offset1.zw, offset2.w, rt);
        d.y = coord_bottom_y;

        d = abs(round(rt.ww * d - pixcoord.yy));
        let sqrt_d = sqrt(d);

        let e2 = textureSampleLevel(edges_tex, linear_sampler, vec2<f32>(cross_x, coord_bottom_y), 0.0, vec2<i32>(0, 1)).g;
        var w_ba = area(sqrt_d, e1, e2);
        w_ba = detect_vertical_corner_pattern(
            w_ba,
            vec4<f32>(texcoord.x, coord_top_y, texcoord.x, coord_bottom_y),
            d,
        );
        weights = vec4<f32>(weights.xy, w_ba);
    }

    return weights;
}
