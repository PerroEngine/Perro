// SMAA 1x pass 3/3: neighborhood blending.
//
// Reads the LDR color target plus the blending-weight target and writes the
// final anti-aliased image to the swapchain (doing the upscale when the
// render size is capped, exactly like the FXAA pass does). The blend
// exploits bilinear filtering: one linear fetch mixes the pixel with its
// chosen neighbor at the exact weight computed by pass 2.
//
// Min-work gating: pixels whose four relevant weights are all zero return
// the center color after 3 texture fetches, so flat regions cost almost
// nothing (the branch pairs with the zero-weight early-out of pass 2).

@group(0) @binding(0) var color_tex: texture_2d<f32>;
@group(0) @binding(1) var blend_tex: texture_2d<f32>;
@group(0) @binding(2) var linear_sampler: sampler;

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

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(blend_tex, 0));
    let rt = vec4<f32>(1.0 / dims, dims);
    let texcoord = in.uv;
    let offset = rt.xyxy * vec4<f32>(1.0, 0.0, 0.0, 1.0) + texcoord.xyxy;

    // Fetch the blending weights for the current pixel:
    // x = right, y = top, z = left, w = bottom.
    var a: vec4<f32>;
    a.x = textureSampleLevel(blend_tex, linear_sampler, offset.xy, 0.0).a;
    a.y = textureSampleLevel(blend_tex, linear_sampler, offset.zw, 0.0).g;
    let own = textureSampleLevel(blend_tex, linear_sampler, texcoord, 0.0).xz;
    a.w = own.x;
    a.z = own.y;

    // Early exit: nothing blends into this pixel.
    if dot(a, vec4<f32>(1.0, 1.0, 1.0, 1.0)) < 0.00001 {
        let color = textureSampleLevel(color_tex, linear_sampler, texcoord, 0.0).rgb;
        return vec4<f32>(color, 1.0);
    }

    // max(horizontal) > max(vertical)?
    let h = max(a.x, a.z) > max(a.y, a.w);

    // Calculate the blending offsets.
    var blending_offset = vec4<f32>(0.0, a.y, 0.0, a.w);
    var blending_weight = a.yw;
    blending_offset = select(blending_offset, vec4<f32>(a.x, 0.0, a.z, 0.0), vec4<bool>(h, h, h, h));
    blending_weight = select(blending_weight, a.xz, vec2<bool>(h, h));
    blending_weight = blending_weight / dot(blending_weight, vec2<f32>(1.0, 1.0));

    // Calculate the texture coordinates.
    let blending_coord = blending_offset * vec4<f32>(rt.xy, -rt.xy) + texcoord.xyxy;

    // Bilinear filtering mixes the current pixel with the chosen neighbor.
    var color = blending_weight.x * textureSampleLevel(color_tex, linear_sampler, blending_coord.xy, 0.0).rgb;
    color += blending_weight.y * textureSampleLevel(color_tex, linear_sampler, blending_coord.zw, 0.0).rgb;
    return vec4<f32>(color, 1.0);
}
