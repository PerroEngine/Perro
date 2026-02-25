@group(0) @binding(0)
var depth_tex: texture_depth_2d;
@group(0) @binding(1)
var dst_mip0: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8u, 8u, 1u)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(dst_mip0);
    if gid.x >= dims.x || gid.y >= dims.y {
        return;
    }
    let d = textureLoad(depth_tex, vec2<i32>(gid.xy), 0);
    textureStore(dst_mip0, vec2<i32>(gid.xy), vec4<f32>(d, 0.0, 0.0, 0.0));
}
