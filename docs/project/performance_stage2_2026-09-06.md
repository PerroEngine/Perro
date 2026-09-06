# Cut repeat renderer + runtime work — stage 2

Build on [stage 1](performance_fixes_2026-09-06.md).
Use stage-1 code as baseline, including lazy draw-cache rebuilds.
Keep the earlier 4.3s -> 3.67ms bulk-remove result separate.

See [stage 3](performance_stage3_2026-09-06.md) for the later connect/emit
priority change and follow-up on slower node-write and draw-removal controls.
Treat this page's reverse-index design and figures as the stage-2 snapshot.

## Trace the change

`script -> node write -> dirty state -> render extract -> retained draws -> GPU`

| Area | Change | Cost / limit |
| --- | --- | --- |
| CPU renderer | Split resource-binding + instance-count revisions from general draw revision | +16 bytes per Renderer3D; compare surface IDs on draw updates |
| Resource scans | Skip mesh/material recounts + animated-material scans on transform-only edits | Keep scans on actual binding/resource changes |
| Draw counts | Recount only when retained instance totals may change | Keep general draw revision for GPU prep |
| World state | Split visibility, modulation, suspension revisions | +24 bytes per NodeArena; keep generic/structural writes conservative |
| Sub-view suspension | Cache result per owning world; reuse across members | Hash-map memory proportional to queried sub-view worlds |
| Node writes | Classify typed/base callback changes; disarm mutation guard after snapshots | Keep broad invalidation on unwind + unknown payload access |
| Internal dispatch | Read node type once; call its existing handler | Replace 11 update-handler attempts + 4 nonempty fixed-handler attempts |
| Internal schedules | Cache immutable Rc slices by membership epoch | Copy on membership change; allocate for new size or active nested snapshot |
| Signals | Index script -> subscribed signal buckets | Extra reverse-index memory + connect/disconnect work |

Keep an inline reverse entry for one signal per script.
Promote to a boxed hash map for multiple distinct signals.
Keep a promoted map until script teardown to avoid repeated promotion allocations.
Count multiple methods on one signal; preserve full NodeID generations.
Keep emission snapshots, survivor order on script teardown, and existing
swap-remove behavior on single disconnection.

Keep an outer internal-update snapshot stable across callback edits, clear,
and nested dispatch. Apply membership changes to the next pass.
Reuse a uniquely owned Rc allocation when snapshot length stays equal.
Keep existing script-collection schedule caches and per-script state storage.
Reject wrong-type mutable access before creating a mutation guard.
Keep node, physics, and world revisions unchanged on failed type probes.
Route all three video node families to the existing shared video handler.

Keep transform math, node layout, GPU shaders, and script-facing APIs in this work.
Narrow the two physics pose/velocity pullback accessors and the 2D texture write.
Keep resource-reference mutation stamps and physics invalidation rules.
Preserve concurrent physics pose/scale fixes from separate work in this checkout.
Exclude those fixes from this report's change and speedup claims.

## Prove less work

Use counters in actual code paths, plus behavior tests.

| Trigger | Before | After |
| --- | --- | --- |
| Transform-only draw update | Recount retained instances + resource refs | Zero recounts; same totals |
| Same draw bindings, animated material | Repeat used-material scan on draw revision | Reuse scan result |
| Remove script with no connections, 8192 signal buckets | Visit 8192 buckets | Visit 0 |
| Remove script subscribed to 32 of 8192 signals | Visit 8192 buckets | Visit 32 |
| Stable internal update/fixed membership | Copy list each pass | Zero warm copies |
| Internal node dispatch | Try unrelated handlers + repeated type probes | One type route + matching handler |
| Typed transform-only node write | Invalidate visibility + modulation memo | Keep both memo epochs |

Do not infer linear total scene teardown from the signal index.
Removing one script still scans connections inside each affected signal bucket.
A shared signal with many subscribers remains a separate fan-out cost.

## Check transforms + heap

Probe real global-transform accessors on 10k flat roots + a 10k-node chain.
Match before/after counts exactly:

| Transform work / 10k nodes | Cache hits | Rebuilt global TRS rows |
| --- | ---: | ---: |
| Clean flat/chain | 10,000 | 0 |
| Edit leaf, query all | 9,999 | 1 |
| Edit flat root, query all | 9,999 | 1 |
| Edit chain root, query all | 0 | 10,000 |

Count 10,000 dirty-subtree visits only for the chain-root case.
Scope counters to global transform queries, dirty propagation, and cached TRS rows;
exclude interpolated matrix composition and shader math.

Probe warm frames with 10k Sprite2D nodes, one shared script behavior,
and 32-byte state per script. Run fixed update + update + 2D extraction + drain.
Use a placeholder texture ID; exclude asset I/O and GPU draw acceptance here.
Measure zero allocation requests in idle, 1%-move, and all-move frames,
both before and after. Rebuild 0 / 100 / 10,000 global TRS rows respectively.

Measure requested live heap bytes via a System allocator wrapper.
Run each memory size in a fresh process to exclude prior-runtime teardown.
Disable allocator accounting for Criterion timing runs.

| Memory probe | Result |
| --- | --- |
| SceneNode / Option<SceneNode> / Node3D layout | 208 / 208 / 60 bytes, unchanged |
| Fill 10k arena nodes, including fixture ID vector | ~2.46 MiB live heap, unchanged |
| Remove + refill 10k arena nodes | Keep 10,001 slots including sentinel; reuse slots |
| Add 10k scripts with 32-byte state | ~1.62 MiB incremental live heap, unchanged |
| Add 10k one-signal subscriptions | ~669 KiB before -> ~1,196 KiB after |
| Added reverse-index overhead | ~528 KiB / 10k links |

Cut the first nested-map index design's ~2 MiB overhead to ~528 KiB
with inline single-signal entries. Keep slot/cache capacity after removal;
do not treat retained capacity as a leak or claim lower arena memory.
Expect small fixed bookkeeping changes and extra sub-view memo capacity.

Include fixture vectors and temporary NodeSpec arrays in memory snapshots.
Report per-phase differences for script/signal rows; raw request counts stay
cumulative within a probe. Exclude allocator metadata, fragmentation, RSS,
GPU buffers, driver heaps, and full game peak memory from these figures.

## Measure CPU time

Record Criterion central estimates below; retain 95% bounds in
`target/perf-stage2/selected-metrics.{json,csv}`.

| CPU renderer workload / 10k retained draws | Before | After | Result |
| --- | ---: | ---: | --- |
| Idle | 1.190 us | 1.201 us | ~same |
| Edit one transform | 296.69 us | 1.634 us | 181.5x faster |
| Edit 1% of transforms | 335.34 us | 38.47 us | 8.7x faster |
| Edit all transforms | 5.004 ms | 4.672 ms | 6.6% less time |
| Change one binding | 298.63 us | 273.49 us | 8.4% less time; keep scan |
| Unready binding fallback | 295.41 us | 1.549 us | Keep accepted bindings; skip scan |

| Runtime workload | Before | After | Result |
| --- | ---: | ---: | --- |
| Internal update / 10k default IK targets | 2.692 ms | 194.84 us | 13.8x faster |
| Internal update + one membership change / 10k | 2.781 ms | 199.71 us | 13.9x faster |
| Internal fixed / 10k default bone chains | 933.75 us | 255.80 us | 3.7x faster |
| Internal fixed + one membership change / 10k | 974.23 us | 269.69 us | 3.6x faster |
| Suspension scan / 10k nodes, no sub-views | 19.34 us | 9.77 us | 2.0x faster |
| Suspension scan / 10k nodes, depth 1 / 4 / 16 | 224 / 611 / 2329 us | 137 / 142 / 138 us | Reuse shared-world result |
| Transform write + visibility/modulation queries / 4096 | 257.15 us | 139.11 us | 1.8x faster |
| Suspension memo workload / 4096 | 92.28 us | 57.84 us | 1.6x faster |
| Script teardown / no links, 8192 buckets | 94.25 us | 0.438 us | Visit zero buckets |
| Script teardown / 32 signals, one method each | 93.96 us | 5.259 us | 17.9x faster |
| Script teardown / 32 signals, eight methods each | 106.63 us | 12.64 us | 8.4x faster |

Keep controls and costs visible:

| Control | Before | After | Interpretation |
| --- | ---: | ---: | --- |
| Full runtime frame / 10k idle scripts | 2.848 ms | 2.908 ms | Intervals overlap; no gain claim |
| Full runtime frame / 10k scripts, 1% move | 3.058 ms | 2.966 ms | 3.0% less time |
| Full runtime frame / 10k scripts, all move | 4.749 ms | 5.079 ms | 6.9% more time in latest pair |
| Signal connect + disconnect | 159.28 ns | 184.06 ns | 15.6% more time for reverse-index maintenance |
| Emit one signal, no parameters | 30.42 ns | 30.59 ns | ~same |
| Dense instance-count change, repeat | 3.430 ms | 3.438 ms | Intervals overlap |
| Single draw add/remove | 1.032 ms | 1.038 ms | Intervals overlap |
| Bulk-remove 10k draws, repeat | 4.248 ms | 4.445 ms | 4.6% more time; first pair +3.1% |
| Queue/drain/drop 10k boxed 3D commands | 828 us | 877 us | Wide overlapping intervals; transport unchanged |

Flag the all-move frame and bulk-remove regressions for follow-up.
Keep the sparse-work gains; do not claim a universal frame speedup.
Observe a smaller all-move delta in earlier runs; later repeats encounter
compiler load and baseline drift. Keep the latest complete pair above,
without assigning its whole delta to one instruction or cache change.
Identify one remaining successful-node-write cost: validate the arena slot
for the type precheck, then validate it again for the scoped mutable accessor.
Consider fusing those checks while preserving wrong-type no-op and unwind rules.

Use Windows, i9-9900K, Rust 1.97.1, workspace bench profile.
Use Criterion: 20 samples, 1s warmup, 2s target, identical fixtures.
Discard the first graphics run with competing compiler load.
Use paired repeats and retain confidence intervals in local JSON output.
Use `stage2-final-*` for graphics, `stage2-control-*` for graphics repeats,
and `stage2-verified-*` for final frame/world controls.
Use the earlier clean `stage2-final-before` signal/internal baseline against
`stage2-verified-after`; reject two later baseline repeats with compiler load.
Sample compiler process counts every 500ms during the final runtime runs;
record zero in the selected final runs. This does not prove an idle machine.
Keep monitor output in `compiler-load*.json`; record executable SHA-256 hashes.
Keep small control-case differences separate from clear workload gains.
Review the first scoped guard's per-write panic-state query while checking
the frame controls; do not assign an isolated speedup to its removal.
Replace the query with explicit normal-path disarming; split the tiny
no-subview predicate from the out-of-line world memo walk.
Use phase timers + an isolated Vec-snapshot rollback to locate a further
regression: failed type probes armed the guard and reset caches.
Fix the type checks; keep the Rc snapshot after separating these causes.
Replace the old dispatch chain with a direct type route.

Time graphics submission + command processing + headless CPU frame prep.
Exclude fixture/command construction, GPU execution, and presentation.
Measure CPU elapsed time around these calls; do not derive it from FPS or
the number of GPU draw calls. Retained draw/instance counts describe scene
records and instances, not a count of hardware draw submissions.
Time runtime frames separately: real fixed/update callbacks + 2D extraction
+ command drain; exclude graphics consumption and rendering.
Time signal teardown with fixture creation/destruction outside the timer.
Use actual update/fixed paths for schedule cases, including membership churn.
Use default IKTarget2D / PhysicsBoneChain2D nodes without skeletons for those
cases: measure runtime dispatch overhead, not active IK or bone-solver math.
Add/remove one node per churn pass. Do not apply the dispatch ratio to whole
game frames or to the useful work inside other handlers.

Enable bench-only counters for transforms + signal/schedule work.
Note extra world-memo counter increments in the new runtime bench build;
the baseline lacks those increments. Keep this instrumentation difference
visible when interpreting small runtime deltas.
Keep the late concurrent physics changes in the final checkout; the baseline
predates them. Use runtime fixtures without rigid bodies here, and avoid
attributing small whole-runtime differences to one isolated code change.

## Check behavior

Pass 531 graphics + 776 runtime + 57 internal-update library tests.
Keep 6 existing internal-update benchmark-style tests ignored.
Enable runtime bench probes for runtime tests.
Pass all-target Clippy with warnings denied.
Cover accepted/unready resources, sparse/full/batch draw edits, counts,
material changes, callback visibility, node generation reuse, reparenting,
typed/base unwind, nested dispatch, duplicate signals, and signal teardown.
Check every registered update type against direct-dispatch route coverage.
Keep fixed-route coverage tied to the two runtime bone-chain routes plus the
two legacy particle handlers exposed by the public dispatch entry point.
Update that coverage alongside any future runtime fixed-dispatch filter change.

Pass five GPU cases on RX 7800 XT / Vulkan / immediate presentation:
empty idle, empty present, 10k sprites, 10k meshes, 100k dense instances.
Use 30 warmup + 120 sample frames, 1280x720.
Confirm 10k / 100k 3D instance counts in GPU output.
Pass six 2D/3D camera-stream cases with 8 warmup + 60 sample frames.
Keep these as execution checks; claim no GPU speedup or pixel-diff result.

## Reproduce

```powershell
cargo bench -p perro_graphics --bench cpu_prepare -- 'graphics_3d_revision_gate_paths|graphics_3d_retained_updates' --sample-size 20 --warm-up-time 1 --measurement-time 2 --noplot
cargo bench -p perro_runtime --features bench --bench runtime_frame_memory -- 'runtime_frame/' --sample-size 20 --warm-up-time 1 --measurement-time 2 --noplot
cargo bench -p perro_runtime --features bench --bench runtime_core_hotpaths -- 'runtime_core/sub_view_suspension' --sample-size 20 --warm-up-time 1 --measurement-time 2 --noplot
cargo bench -p perro_runtime --features bench --bench node_state_hotpaths -- 'node_state/world_state_memos' --sample-size 20 --warm-up-time 1 --measurement-time 2 --noplot
cargo bench -p perro_runtime --features bench --bench signal_hotpaths -- 'signal/script_teardown|signal/connect_disconnect_churn|internal_schedule/full_pass' --sample-size 20 --warm-up-time 1 --measurement-time 2 --noplot
cargo bench -p perro_runtime --features bench --bench runtime_frame_memory -- --memory --nodes=10000
cargo bench -p perro_runtime --features bench --bench runtime_frame_memory -- --transform-work
cargo bench -p perro_runtime --features bench --bench runtime_frame_memory -- --frame-work --nodes=10000 --stride=100
cargo bench -p perro_runtime --features bench --bench runtime_frame_memory -- --internal-times
cargo bench -p perro_runtime --features bench --bench runtime_frame_memory -- --internal-times --fixed-internal
cargo test -p perro_graphics -p perro_runtime -p perro_internal_updates --lib --features perro_runtime/bench
cargo clippy -p perro_graphics -p perro_runtime -p perro_internal_updates --all-targets --features perro_runtime/bench -- -D warnings
```

Use `--stride=0`, `100`, or `1` for idle, 1%, or all nodes moving.
Use a separate process for `--nodes=1000` memory samples.
Keep raw logs, executable snapshots, memory probes, GPU CSV, and estimates
under `target/perf-stage2/`.
Keep the detached stage-1 baseline at `D:/Rust/Perro-perf-stage2-baseline`.

## Pick the next measured target

1. Isolate the all-move frame and bulk-remove control regressions on an idle
   machine; try a single validation/type check per successful node mutation.
2. Profile a representative game frame, with CPU samples + GPU timestamps;
   include node creation, first visible scene frame, and scene unload.
3. Audit full-frame physics pose capture / clean global-transform queries;
   the flat idle probe still performs 10k cache-hit queries.
4. Audit owned 3D command transport and sparse GPU uploads;
   the existing queue/drain/drop probe still boxes every command.
5. Measure broad generic node-write invalidation frequency before narrowing it.
6. Measure arena high-water capacity, script payload sizes, signal fan-out,
   and sub-view memo growth in a real scene before changing storage layout.

Use workload evidence before SIMD, arena-layout rewrites, or shader changes.
