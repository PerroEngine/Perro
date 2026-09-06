# Cut repeat draw-cache work — 2026-09-06

See [stage 2](performance_stage2_2026-09-06.md) for resource/count revision gates,
world-state memos, internal dispatch/schedules, signals, and heap probes.
Keep this report's original baseline separate from stage 2's stage-1 baseline.

## Trace frame work

| Stage | Input -> work -> output | Existing skip |
| --- | --- | --- |
| Script runtime | update/fixed tick -> callbacks -> node edits | schedule epoch -> reuse script slots |
| World state | node edits -> dirty subtree + global transforms | dirty flags + transform cache |
| Render extract | dirty visual nodes -> resource requests + draw commands | retained state + dirty lanes |
| CPU renderer | commands -> live draw rows -> NodeID order | draw equality + sequential packet path |
| GPU prep | draw rows -> batches + staged buffers | revision gates + partial uploads |
| GPU frame | buffers -> cull + shadow + scene + post passes | retained frame + pass-specific gates |

Distinguish CPU prep cost, GPU execution, and acquire/present waits.
Cut redundant CPU work without a GPU shader or script API change.

## Change draw-cache maintenance

Before: remove one retained 3D draw -> clone + sort every surviving draw.
Bulk unload repeats that work after each removal.
Sparse updates also rebuild the full cache because the sequential fast path
requires a full draw list in matching order.

After: track NodeID -> sorted row; patch changed rows while membership stays stable.
Add/remove marks membership dirty; the next sorted-cache read rebuilds once.
Guard the sequential path against stale membership.
Keep NodeID order, resource readiness fallback, revisions, and slot generations.

Change Rust renderer accessor `retained_draws_sorted` from `&self` to `&mut self`
so reads can refresh the cache. Keep script-context APIs unchanged.
Add one hash-map index per retained draw; trade persistent memory for less repeat work.

## Keep next targets separate

- Backend resource-use counts and draw-instance counts still depend on the broad
  draw revision. A transform-only update can rescan all retained draws despite
  unchanged resource bindings and instance counts. Sparse cache patches do not
  make the whole CPU frame proportional to changed nodes.
- GPU prep still classifies draw pairs when revisions change. Preserve its
  transform, LOD, blend, resource and camera invalidation rules before replacing
  comparisons with finer change metadata.
- See [runtime + bridge audit](../dev/runtime_redundant_work.md) for measured
  sub-view checks and command ownership costs.

## Reproduce

Use identical bench profiles and hardware for before/after runs.
Build in parallel; time workloads without competing build/test jobs.
Use existing graphics `cpu_prepare` and runtime `runtime_core_hotpaths` benches.
Use graphics `gpu_frame` for GPU timestamps and presentation smoke checks.
Treat GPU checks as validation, not a shader speedup claim.

## Measure CPU changes

Use Windows, i9-9900K, RX 7800 XT, Rust 1.97.1; workspace bench profile
(`opt-level=3`, LTO off, 64 codegen units). Build baseline from `6bd8ad2a`
in a detached worktree with the same new benchmark fixtures.

Time headless `PerroGraphics::submit_many` + `draw_frame_timed` end to end.
Include command processing, sorted-cache refresh, backend reference counts and
CPU preparation; exclude fixture/command construction and GPU execution.
Alternate transforms every sample so sparse/full updates change actual data.
Use 20 samples, 1s warmup, 2s target measurement; the slow baseline bulk-remove
case needs more time to collect all samples.

| Work / retained draw count | Before | After |
| --- | ---: | ---: |
| One changed draw / 1k | 86.44 us | 30.73 us |
| 1% changed draws / 1k | 88.61 us | 34.56 us |
| Remove all / 1k | 39.54 ms | 297.24 us |
| One changed draw / 10k | 843.79 us | 292.41 us |
| 1% changed draws / 10k | 877.03 us | 327.50 us |
| All changed draws / 10k | 4.386 ms | 4.447 ms |
| Remove all / 10k | 4.328 s | 3.674 ms |

Use Criterion central estimates. Repeat focused 10k cases with 10 samples:
one edit 845.33 -> 288.66 us; 1% edits 880.51 -> 329.31 us;
remove-all 4.341 s -> 3.676 ms.
Full updates: 4.448 -> 4.556 ms; 95% intervals 4.392–4.555 and 4.475–4.646 ms.
Flag the +1.4–2.4% full-update estimates across both passes; intervals overlap,
so do not claim a proven full-update improvement or regression.
Keep idle ~1.2 us at both sizes. Control cases: 10k sprites 570.45 -> 548.05 us;
unchanged 100k dense instances 1.426 -> 1.387 us. Make no 2D/GPU speedup claim.

Keep allocation request counts/bytes unchanged in all ten focused samples.
For one edit / 10k: 3 requests, 144 bytes. For remove-all / 10k: 1 request,
1,040,000 bytes. Count allocations/reallocations during submission + frame prep;
exclude packet construction, initial caches and teardown. These are requested
bytes, not peak/live memory. Existing vectors already reuse capacity; the main
gain comes from removing repeated copies, Arc reference-count traffic and sorts.

## Check behavior

Pass `cargo test -p perro_graphics -p perro_runtime --lib`:
527 graphics + 748 runtime tests, zero failures or ignored tests.
Include sparse/no-op edits, bulk removal, mixed membership, duplicate packets,
generation reuse, unready resources, sequential updates and backend resource counts.

Pass all-target Clippy for both crates with `perro_runtime/bench` enabled.
Use `expect` in regression fixtures; keep production behavior unchanged by lint cleanup.

Run GPU checks on Vulkan, 1280x720, immediate presentation:
six `gpu_frame` cases, 30 warmup + 120 measured frames each.
Cover empty idle/present, 10k sprites, 10k meshes, 100k dense instances and
100k instances with camera motion. GPU main medians: 25.3 us empty-present,
395.3 us sprites, 59.6 us meshes, 117.8 us dense, 58.1 us camera-motion case.
Keep acquire/submit/present outside GPU timestamp numbers.
Run six 2D/3D camera-stream cases, 8 warmup + 60 measured frames each.
Pass sample-count, active draw/pass and zero 3D full-rebuild assertions.
Use these as execution checks; no before/after pixel-diff claim.

## Record scene baselines

| Existing workload | Current time | 95% interval |
| --- | ---: | ---: |
| Prepare parsed scene / 2048 nodes | 1.037 ms | 1.022–1.050 ms |
| Prepare + fresh runtime + merge / 2048 | 22.734 ms | 22.540–22.953 ms |
| Compiled scene spawn / 2048 | 1.837 ms | 1.823–1.851 ms |
| Prepare + fresh runtime + merge + extract / 512 | 22.485 ms | 22.312–22.706 ms |
| Prepare + fresh runtime + merge + extract / 8192 | 44.846 ms | 44.563–45.193 ms |

Keep fixture boundaries distinct. Compiled-spawn setup excludes runtime construction;
fresh-runtime helpers include it. Merge/extract uses a different mixed mesh/multimesh
scene. Do not subtract these rows to infer isolated stage costs or claim a scene-load
speedup. No first-visible-frame or asset-I/O latency measurement in these helpers.
Use 20 samples; scene-loading groups retain their built-in 100ms/1s overrides
for runtime-build and compiled-spawn cases. Use 1s warmup/2s target elsewhere.

## Reproduce focused checks

```powershell
cargo bench -p perro_graphics --bench cpu_prepare -- 'graphics_3d_(retained_updates|bulk_remove)' --sample-size 20 --warm-up-time 1 --measurement-time 2 --noplot
cargo bench -p perro_runtime --features bench --bench scene_loading -- 'scene_static_parsed_prepare(_runtime_build)?/2048$|scene_compiled_spawn/2048$' --noplot
cargo bench -p perro_runtime --features bench --bench scene_merge_extract -- 'scene_merge_extract/(512|8192)$' --sample-size 20 --warm-up-time 1 --measurement-time 2 --noplot
cargo clippy -p perro_graphics -p perro_runtime --all-targets --features perro_runtime/bench
```

Keep local raw graphics logs + estimate JSONs in `target/perf-2026-09-06/`.
Keep test, lint, GPU CSV and scene logs under `target/perf-2026-09-06-*`.
Remove detached baseline worktree. Retain `D:/Rust/Perro-bench-target`: automatic
approval review rejects recursive cleanup with reason “blocked by policy”.
