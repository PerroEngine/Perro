// TAA v1 resolve — camera-reprojection only.
//
// Runs post-tonemap on the LDR output (like FXAA/SMAA) at render resolution.
// History is reprojected through the previous frame's camera using current
// scene depth and the current/previous UNJITTERED view-proj matrices; the
// scene itself renders with a per-frame sub-pixel Halton(2,3) camera jitter
// so the accumulated history resolves sub-pixel detail.
//
// v1 tradeoff (documented): there are NO per-object motion vectors, so only
// CAMERA motion reprojects correctly. Fast-moving objects leave short
// ghosting trails bounded by the 3x3 neighborhood clamp below; per-object
// motion vectors are the planned upgrade path.
//
// Rejection rules (fall back to the current frame's color):
// - reprojected uv lands off-screen, or behind the previous camera (w <= 0)
// - depth disocclusion: the history alpha channel carries last frame's
//   device depth; if it disagrees with the reprojected expected depth the
//   surface was occluded/revealed and the history is stale
// - history reset (resize / first frame / no-3D gap): blend weight comes in
//   as 0 through the uniform.
//
// Output: rgb = resolved color, a = current device depth (next frame's
// disocclusion reference). The target is the other half of the history
// ping-pong pair; a separate blit pass copies rgb to the swapchain (doing
// the upscale when the render size is capped).

struct TaaUniform {
    // Inverse of the current frame's unjittered view-proj.
    inv_view_proj: mat4x4<f32>,
    // Previous frame's unjittered view-proj.
    prev_view_proj: mat4x4<f32>,
    // x = history blend weight (HISTORY_BLEND, or 0.0 on reset),
    // y = depth disocclusion reject threshold (device-z units), zw unused.
    params: vec4<f32>,
};

@group(0) @binding(0) var current_tex: texture_2d<f32>;
@group(0) @binding(1) var history_tex: texture_2d<f32>;
@group(0) @binding(2) var depth_tex: texture_2d<f32>;
@group(0) @binding(3) var history_sampler: sampler;
@group(0) @binding(4) var<uniform> cfg: TaaUniform;

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
    let dims = vec2<i32>(textureDimensions(current_tex));
    let pixel = clamp(vec2<i32>(in.pos.xy), vec2<i32>(0), dims - vec2<i32>(1));
    let current = textureLoad(current_tex, pixel, 0).rgb;
    let depth = textureLoad(depth_tex, pixel, 0).r;

    // Reproject this pixel into the previous frame's screen space via world
    // position (camera reprojection only; see header).
    var history_weight = cfg.params.x;
    var prev_uv = in.uv;
    var prev_depth_expected = depth;
    let ndc = vec3<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0, depth);
    let world = cfg.inv_view_proj * vec4<f32>(ndc, 1.0);
    if abs(world.w) > 1.0e-8 {
        let world_pos = world.xyz / world.w;
        let prev_clip = cfg.prev_view_proj * vec4<f32>(world_pos, 1.0);
        if prev_clip.w > 1.0e-6 {
            let prev_ndc = prev_clip.xyz / prev_clip.w;
            prev_uv = vec2<f32>(prev_ndc.x * 0.5 + 0.5, 0.5 - prev_ndc.y * 0.5);
            prev_depth_expected = prev_ndc.z;
        } else {
            // Behind the previous camera: no usable history.
            history_weight = 0.0;
        }
    } else {
        history_weight = 0.0;
    }
    if prev_uv.x < 0.0 || prev_uv.x > 1.0 || prev_uv.y < 0.0 || prev_uv.y > 1.0 {
        // Off-screen last frame: reject.
        history_weight = 0.0;
    }

    let history = textureSampleLevel(history_tex, history_sampler, prev_uv, 0.0);
    // Depth disocclusion reject: history alpha holds last frame's device z.
    if abs(history.a - prev_depth_expected) > cfg.params.y {
        history_weight = 0.0;
    }

    // Standard 3x3 neighborhood min/max AABB clamp in output color space:
    // bounds stale/ghosting history to colors present around this pixel.
    var c_min = current;
    var c_max = current;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            if x == 0 && y == 0 {
                continue;
            }
            let p = clamp(pixel + vec2<i32>(x, y), vec2<i32>(0), dims - vec2<i32>(1));
            let c = textureLoad(current_tex, p, 0).rgb;
            c_min = min(c_min, c);
            c_max = max(c_max, c);
        }
    }
    let clamped = clamp(history.rgb, c_min, c_max);

    let resolved = mix(current, clamped, history_weight);
    return vec4<f32>(resolved, depth);
}
