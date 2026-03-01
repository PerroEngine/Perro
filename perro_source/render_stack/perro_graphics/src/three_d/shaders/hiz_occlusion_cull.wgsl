struct HizCullParams {
    view_proj: mat4x4<f32>,
    draw_count: u32,
    hiz_mip_count: u32,
    hiz_width: u32,
    hiz_height: u32,
    aspect: f32,
    proj_y_scale: f32,
    depth_bias: f32,
    _pad0: u32,
}

struct CullItem {
    model_0: vec4<f32>,
    model_1: vec4<f32>,
    model_2: vec4<f32>,
    model_3: vec4<f32>,
    local_center_radius: vec4<f32>,
    cull_flags: vec4<u32>,
}

struct DrawIndexedIndirect {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
}

@group(0) @binding(0)
var<uniform> params: HizCullParams;
@group(0) @binding(1)
var<storage, read> cull_items: array<CullItem>;
@group(0) @binding(2)
var<storage, read_write> commands: array<DrawIndexedIndirect>;
@group(0) @binding(3)
var hiz_tex: texture_2d<f32>;

fn finite4(v: vec4<f32>) -> bool {
    return all(v == v) && all(abs(v) < vec4<f32>(1.0e30));
}

@compute @workgroup_size(64u)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= params.draw_count {
        return;
    }
    if commands[i].instance_count == 0u {
        return;
    }

    let item = cull_items[i];
    if (item.cull_flags.x & 1u) != 0u {
        return;
    }
    let model = mat4x4<f32>(item.model_0, item.model_1, item.model_2, item.model_3);
    let center_local = vec4<f32>(item.local_center_radius.xyz, 1.0);
    let center_world = model * center_local;
    if !finite4(center_world) {
        return;
    }

    let clip = params.view_proj * center_world;
    if !finite4(clip) || clip.w <= 1.0e-5 {
        return;
    }

    let ndc = clip.xyz / clip.w;
    if ndc.x < -1.1 || ndc.x > 1.1 || ndc.y < -1.1 || ndc.y > 1.1 || ndc.z < -1.1 || ndc.z > 1.1 {
        return;
    }

    let sx = length(item.model_0.xyz);
    let sy = length(item.model_1.xyz);
    let sz = length(item.model_2.xyz);
    let scale = max(max(sx, sy), max(sz, 1.0e-6));
    let radius_world = max(item.local_center_radius.w, 0.0) * scale;

    // Approximate projected radius in pixels to select Hi-Z mip level.
    let radius_ndc_y = (radius_world * params.proj_y_scale) / max(abs(clip.w), 1.0e-4);
    let radius_px = max(radius_ndc_y * 0.5 * f32(params.hiz_height), 1.0);
    let diameter_px = radius_px * 2.0;
    let mip = min(
        u32(max(floor(log2(diameter_px)), 0.0)),
        max(params.hiz_mip_count, 1u) - 1u,
    );

    let dims = textureDimensions(hiz_tex, i32(mip));
    // NDC y is up; texture coordinates are top-left origin.
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, -ndc.y * 0.5 + 0.5);
    let px_f = uv.x * f32(dims.x);
    let py_f = uv.y * f32(dims.y);
    let mip_scale = exp2(f32(mip));
    let radius_px_mip = max(radius_px / mip_scale, 1.0);
    let rx = i32(ceil(radius_px_mip));
    let ry = i32(ceil(radius_px_mip));

    let cx = clamp(i32(px_f), 0, i32(dims.x) - 1);
    let cy = clamp(i32(py_f), 0, i32(dims.y) - 1);
    let x0 = clamp(cx - rx, 0, i32(dims.x) - 1);
    let x1 = clamp(cx + rx, 0, i32(dims.x) - 1);
    let y0 = clamp(cy - ry, 0, i32(dims.y) - 1);
    let y1 = clamp(cy + ry, 0, i32(dims.y) - 1);

    // Conservative with max-depth Hi-Z: sample several points and keep the maximum
    // (farthest depth) across the bound footprint to avoid false culls.
    let d_center = textureLoad(hiz_tex, vec2<i32>(cx, cy), i32(mip)).x;
    let d_l = textureLoad(hiz_tex, vec2<i32>(x0, cy), i32(mip)).x;
    let d_r = textureLoad(hiz_tex, vec2<i32>(x1, cy), i32(mip)).x;
    let d_b = textureLoad(hiz_tex, vec2<i32>(cx, y0), i32(mip)).x;
    let d_t = textureLoad(hiz_tex, vec2<i32>(cx, y1), i32(mip)).x;
    let hiz_depth = max(max(max(d_center, d_l), max(d_r, d_b)), d_t);

    // Projection matrix is RH_GL; map NDC z [-1, 1] to depth [0, 1].
    let center_depth = clamp(ndc.z * 0.5 + 0.5, 0.0, 1.0);
    // Conservative front depth estimate for object bounds.
    let nearest_depth = max(center_depth - radius_ndc_y * 0.5, 0.0);
    if nearest_depth > hiz_depth + params.depth_bias {
        commands[i].instance_count = 0u;
    }
}
