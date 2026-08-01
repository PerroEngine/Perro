// Stream-compaction of GPU-culled DrawIndexedIndirect commands, one workgroup
// per CPU-built state run. The frustum / hi-z cull passes write
// instance_count = 0 into every culled slot; this pass copies the surviving
// slots of a run into the compacted buffer (order preserved) and stores the
// survivor count alongside, so the render pass can issue
// multi_draw_indexed_indirect_count over the run instead of walking max_count
// zero-instance commands on the GPU frontend.
//
// Destination slot = run.dst_base + run.start, where dst_base picks the
// consuming pass's region (main / depth prepass / mesh-blend depth). Passes
// share source slots, so without the region split the main pass's compaction
// would clobber the prepass's commands.
//
// A dispatch covers runs [params.run_offset, +params.run_count) of one shared
// run array: the whole frame's plan is uploaded once, and the pass is dispatched
// once per distinct state of the source indirect buffer (hi-z rewrites it
// between the prepass and the main pass).
//
// Runs are tens-to-hundreds of commands, so one workgroup handles a run in
// 64-wide chunks: a Hillis-Steele inclusive scan gives each surviving lane its
// destination slot, and a workgroup-shared running base carries across chunks.
// Order preservation is required: alpha / overlay groups depend on draw order.
//
// Names use the reserved perro_ prefix (engine namespace).

struct PerroIndirectCompactParams {
    run_offset: u32,
    run_count: u32,
    _pad0: u32,
    _pad1: u32,
}

struct PerroIndirectRun {
    start: u32,
    len: u32,
    dst_base: u32,
}

struct PerroDrawIndexedIndirect {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
}

@group(0) @binding(0)
var<uniform> perro_compact_params: PerroIndirectCompactParams;
@group(0) @binding(1)
var<storage, read> perro_compact_runs: array<PerroIndirectRun>;
@group(0) @binding(2)
var<storage, read> perro_compact_src: array<PerroDrawIndexedIndirect>;
@group(0) @binding(3)
var<storage, read_write> perro_compact_dst: array<PerroDrawIndexedIndirect>;
@group(0) @binding(4)
var<storage, read_write> perro_compact_counts: array<u32>;

const PERRO_COMPACT_GROUP: u32 = 64u;

var<workgroup> perro_compact_scan: array<u32, 64>;
var<workgroup> perro_compact_base: u32;

@compute @workgroup_size(64u)
fn cs_compact(
    @builtin(workgroup_id) wid: vec3<u32>,
    @builtin(local_invocation_index) lid: u32,
) {
    let run_index = wid.x;
    if run_index >= perro_compact_params.run_count {
        return;
    }
    let run = perro_compact_runs[perro_compact_params.run_offset + run_index];
    let dst = run.dst_base + run.start;
    if lid == 0u {
        perro_compact_base = 0u;
    }
    workgroupBarrier();

    var chunk = 0u;
    loop {
        if chunk >= run.len {
            break;
        }
        let i = chunk + lid;
        var keep = 0u;
        var cmd = PerroDrawIndexedIndirect(0u, 0u, 0u, 0, 0u);
        if i < run.len {
            cmd = perro_compact_src[run.start + i];
            if cmd.instance_count > 0u {
                keep = 1u;
            }
        }
        perro_compact_scan[lid] = keep;

        // Hillis-Steele inclusive prefix sum over the 64-lane chunk. Every
        // barrier sits in uniform control flow (offset and lid bounds only).
        for (var offset = 1u; offset < PERRO_COMPACT_GROUP; offset = offset << 1u) {
            workgroupBarrier();
            var carried = 0u;
            if lid >= offset {
                carried = perro_compact_scan[lid - offset];
            }
            workgroupBarrier();
            if lid >= offset {
                perro_compact_scan[lid] = perro_compact_scan[lid] + carried;
            }
        }
        workgroupBarrier();

        let inclusive = perro_compact_scan[lid];
        let chunk_total = perro_compact_scan[PERRO_COMPACT_GROUP - 1u];
        let base = perro_compact_base;
        if keep == 1u {
            perro_compact_dst[dst + base + inclusive - 1u] = cmd;
        }
        workgroupBarrier();
        if lid == 0u {
            perro_compact_base = base + chunk_total;
        }
        workgroupBarrier();
        chunk = chunk + PERRO_COMPACT_GROUP;
    }

    if lid == 0u {
        perro_compact_counts[dst] = perro_compact_base;
    }
}
