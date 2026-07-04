// Single-pass hi-z downsampler (SPD-style). One dispatch produces up to
// MIP_CHAIN output mips from a single source mip using workgroup shared memory
// + barriers, replacing the per-mip dispatch loop (one compute pass per mip)
// and the pass-boundary barrier serialization between tiny dispatches.
//
// Reduction op is max-depth: the cull shader treats the hi-z as the FARTHEST
// depth over a bound's footprint (nearest_depth > hiz_depth => cull), so mips
// must keep the maximum depth of each covered block. NPOT edge texels are
// clamped to the source extent (min(...)), never sampled out of range, so the
// pyramid stays conservative (never loses a far occludee that would un-cull).
//
// Layout per workgroup (8x8 = 64 threads): each thread reduces a 2x2 source
// block -> an 8x8 tile (mip base). That tile is reduced in shared memory
// 8x8 -> 4x4 -> 2x2 -> 1x1, emitting one dst mip per level (4 mips total).
// A workgroup owns a 16x16 region of the source mip.

struct SpdParams {
    // Number of output mips this dispatch actually writes (1..=4). Levels beyond
    // this are bound to dst mip 0 as a harmless dummy but never stored (guarded).
    mip_count: u32,
    // Dimensions of the source (input) mip.
    src_width: u32,
    src_height: u32,
    _pad0: u32,
}

@group(0) @binding(0)
var src_tex: texture_2d<f32>;
@group(0) @binding(1)
var dst_mip0: texture_storage_2d<r32float, write>;
@group(0) @binding(2)
var dst_mip1: texture_storage_2d<r32float, write>;
@group(0) @binding(3)
var dst_mip2: texture_storage_2d<r32float, write>;
@group(0) @binding(4)
var dst_mip3: texture_storage_2d<r32float, write>;
@group(0) @binding(5)
var<uniform> params: SpdParams;

// 8x8 tile of reduced source values shared across the workgroup.
var<workgroup> tile: array<array<f32, 8u>, 8u>;

fn load_src(x: i32, y: i32) -> f32 {
    let cx = clamp(x, 0, i32(params.src_width) - 1);
    let cy = clamp(y, 0, i32(params.src_height) - 1);
    return textureLoad(src_tex, vec2<i32>(cx, cy), 0).x;
}

@compute @workgroup_size(8u, 8u, 1u)
fn cs_main(
    @builtin(workgroup_id) wg: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
) {
    let lx = lid.x;
    let ly = lid.y;

    // Base output mip (mip 0 of this dispatch): 8x8 region owned by workgroup.
    let out0_x = wg.x * 8u + lx;
    let out0_y = wg.y * 8u + ly;
    // Corresponding 2x2 source block top-left.
    let sx = i32(out0_x * 2u);
    let sy = i32(out0_y * 2u);
    let d00 = load_src(sx, sy);
    let d10 = load_src(sx + 1, sy);
    let d01 = load_src(sx, sy + 1);
    let d11 = load_src(sx + 1, sy + 1);
    let m0 = max(max(d00, d10), max(d01, d11));

    let out0_dims = textureDimensions(dst_mip0);
    if out0_x < out0_dims.x && out0_y < out0_dims.y {
        textureStore(dst_mip0, vec2<u32>(out0_x, out0_y), vec4<f32>(m0, 0.0, 0.0, 0.0));
    }
    tile[ly][lx] = m0;
    if params.mip_count <= 1u {
        return;
    }
    workgroupBarrier();

    // Mip 1: 8x8 -> 4x4. Even lanes read a 2x2 block from `tile`.
    if lx < 4u && ly < 4u {
        let a = tile[ly * 2u][lx * 2u];
        let b = tile[ly * 2u][lx * 2u + 1u];
        let c = tile[ly * 2u + 1u][lx * 2u];
        let d = tile[ly * 2u + 1u][lx * 2u + 1u];
        let m1 = max(max(a, b), max(c, d));
        let ox = wg.x * 4u + lx;
        let oy = wg.y * 4u + ly;
        let dims = textureDimensions(dst_mip1);
        if ox < dims.x && oy < dims.y {
            textureStore(dst_mip1, vec2<u32>(ox, oy), vec4<f32>(m1, 0.0, 0.0, 0.0));
        }
        tile[ly][lx] = m1;
    }
    if params.mip_count <= 2u {
        return;
    }
    workgroupBarrier();

    // Mip 2: 4x4 -> 2x2.
    if lx < 2u && ly < 2u {
        let a = tile[ly * 2u][lx * 2u];
        let b = tile[ly * 2u][lx * 2u + 1u];
        let c = tile[ly * 2u + 1u][lx * 2u];
        let d = tile[ly * 2u + 1u][lx * 2u + 1u];
        let m2 = max(max(a, b), max(c, d));
        let ox = wg.x * 2u + lx;
        let oy = wg.y * 2u + ly;
        let dims = textureDimensions(dst_mip2);
        if ox < dims.x && oy < dims.y {
            textureStore(dst_mip2, vec2<u32>(ox, oy), vec4<f32>(m2, 0.0, 0.0, 0.0));
        }
        tile[ly][lx] = m2;
    }
    if params.mip_count <= 3u {
        return;
    }
    workgroupBarrier();

    // Mip 3: 2x2 -> 1x1.
    if lx == 0u && ly == 0u {
        let a = tile[0u][0u];
        let b = tile[0u][1u];
        let c = tile[1u][0u];
        let d = tile[1u][1u];
        let m3 = max(max(a, b), max(c, d));
        let ox = wg.x;
        let oy = wg.y;
        let dims = textureDimensions(dst_mip3);
        if ox < dims.x && oy < dims.y {
            textureStore(dst_mip3, vec2<u32>(ox, oy), vec4<f32>(m3, 0.0, 0.0, 0.0));
        }
    }
}
