@group(0) @binding(0)
var src_tex: texture_2d<f32>;
@group(0) @binding(1)
var dst_tex: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8u, 8u, 1u)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let src_dims = textureDimensions(src_tex, 0);
    let dst_dims = textureDimensions(dst_tex);
    if gid.x >= dst_dims.x || gid.y >= dst_dims.y {
        return;
    }

    let sx = i32(gid.x * 2u);
    let sy = i32(gid.y * 2u);
    let x1 = min(sx + 1, i32(src_dims.x) - 1);
    let y1 = min(sy + 1, i32(src_dims.y) - 1);

    let d00 = textureLoad(src_tex, vec2<i32>(sx, sy), 0).x;
    let d10 = textureLoad(src_tex, vec2<i32>(x1, sy), 0).x;
    let d01 = textureLoad(src_tex, vec2<i32>(sx, y1), 0).x;
    let d11 = textureLoad(src_tex, vec2<i32>(x1, y1), 0).x;
    // Conservative occlusion pyramid: keep farthest depth in the block.
    let dmax = max(max(d00, d10), max(d01, d11));
    textureStore(dst_tex, vec2<i32>(gid.xy), vec4<f32>(dmax, 0.0, 0.0, 0.0));
}
