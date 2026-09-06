# Prioritize connect, emit, script calls + writes

Follow [stage 2](performance_stage2_2026-09-06.md).
Use the stage-2 working tree as this pass's baseline, including its signal
reverse index, scoped node mutation guards, and renderer revision gates.
Keep the original 4.3-second draw-removal baseline separate.

## Compare CPU time

Use matched stage-3 b4/aft executables; lower time = better.
Show central estimate + 95% confidence interval in brackets.
Use slope when Criterion supplies it; use mean otherwise.
Compute deltas from separate b4/aft estimates, not Criterion's previous-run
comparison against the same named baseline.

| Signal path | B4 | Aft | Result |
| --- | --- | --- | --- |
| First connect, empty bound params | 2.317 [2.249, 2.393] us | 1.750 [1.583, 1.976] us | 24% less time |
| Second method connect, 2 bound params | 1.146 [1.108, 1.188] us | 1.175 [1.116, 1.243] us | Overlap; no clear gain |
| Connect + disconnect churn | 183.96 [183.82, 184.15] ns | 35.00 [34.95, 35.05] ns | 5.26x; both ops |
| Emit, 1 listener, no params | 30.58 [30.26, 30.99] ns | 27.86 [27.85, 27.88] ns | 9% less time |
| Emit from active script, 1 listener | 32.47 [32.04, 33.22] ns | 31.06 [31.00, 31.13] ns | 4% less time |
| Emit, 1 listener, bound params only | 54.15 [53.75, 54.63] ns | 31.25 [30.92, 31.62] ns | 42% less time |
| Emit, 4 listeners, emitted + bound params | 228.27 [227.26, 229.34] ns | 208.21 [206.38, 210.96] ns | 9% less time |
| Emit, 16 listeners, no params | 258.74 [257.68, 260.57] ns | 153.28 [152.96, 153.57] ns | 41% less time |
| Warm 16 listeners -> connect 17th + emit | 1.988 [1.878, 2.109] us | 0.904 [0.874, 0.940] us | 55% less time |
| Warm 16 listeners -> connect 17th only | 0.739 [0.692, 0.802] us | 0.936 [0.907, 0.968] us | **27% more time** |

Flag listener-vector growth as a remaining connect regression: +197 ns at
16 -> 17 listeners. Repeat in reverse executable order -> same result;
first pair 0.739 -> 0.909 us, repeat 0.739 -> 0.936 us.
Both versions grow capacity 16 -> 32 here. New shared storage also requires
an ownership check + pointer access; no isolated measurement of each cost.
Avoid an exact causal claim for the 197 ns difference.

Keep this tradeoff visible alongside faster common connects + emits.
Avoid an early capacity reserve solely to shift this cost into fixture setup.
Keep four inline listeners to limit small-signal heap cost.
Do not subtract isolated-connect time from connect-plus-emit time:
separate fixtures, allocator/cache state, and timing noise prevent that inference.

| Runtime / renderer path | B4 | Aft | Result |
| --- | --- | --- | --- |
| Bulk draw remove, 10k | 4.511 [4.460, 4.569] ms | 3.844 [3.805, 3.887] ms | 15% less time |
| Full runtime frame, all 10k nodes move | 4.660 [4.608, 4.741] ms | 4.595 [4.577, 4.616] ms | Small central gain; overlap |
| Full runtime frame, 1% of 10k nodes move | 2.921 [2.879, 2.974] ms | 3.029 [2.940, 3.109] ms | 3.7% higher central time; overlap |
| Full runtime frame, 10k idle scripts | 2.767 [2.738, 2.801] ms | 2.715 [2.707, 2.722] ms | 1.9% less time |
| Sparse draw update, 1 of 10k | 1.644 [1.641, 1.647] us | 1.655 [1.650, 1.661] us | +11 ns / 0.7%; no gain |
| Cross-script no-op method, 10k calls | 282.56 [282.45, 282.68] us | 279.74 [279.39, 280.11] us | 1% less time |
| Cross-script stateful method, 10k calls | 375.65 [374.10, 377.89] us | 375.41 [372.92, 378.50] us | Overlap; flat |
| Cross-script var get/set, 10k targets | 351.39 [350.35, 352.95] us | 358.54 [346.10, 374.68] us | Overlap; no clear gain |
| Typed cross-script state read, 10k targets | 36.81 [36.70, 36.92] us | 36.64 [36.55, 36.74] us | Overlap; flat |

Treat intervals as descriptive uncertainty, not a formal paired significance
test. Avoid a broad frame-speed or cross-script-speed claim from these results.
Exclude the typed cross-script write control from conclusions: compiler overlap
in that pair. Preserve its raw estimates with `compiler_free_pair: false`.

## Change the work

| Path | Change | Boundary |
| --- | --- | --- |
| Signal connect | Remove the script-to-signal reverse index; store one listener inline; represent empty bound params without an allocation | Accept slower script teardown |
| Signal emit | Share the multi-listener vector through Rc; clone it only when an edit overlaps a live emission snapshot | Keep callback order and nested emission snapshots |
| Bound signal params | Borrow bound params directly when emitted params are empty | Keep emitted-then-bound order when both exist |
| Script lookup | Resolve index + instance in one generation-checked lookup for method and signal dispatch | Keep behavior Rc alive across callbacks |
| Cross-script vars | Inline the small get/set wrappers across crate boundaries | Keep Variant conversion and generated script behavior |
| Node writes | Fuse slot, generation, and type validation with scoped mutable access | Keep failed type probes side-effect free and unwind invalidation conservative |
| Draw removal | Skip empty light, sky, water, decal, and particle maps | Still remove every role owned by a node when maps contain entries |

Keep one-listener signals free of separate listener-list and empty-param
allocations. The containing hash table may still grow on connect.
Promote multiple listeners into a shared SmallVec with room for four listeners
inside its Rc allocation. Spill to a heap vector above four. Normal edits reuse unique
storage, while reentrant edits copy the vector to preserve the outer pass.
Keep promoted vectors after a partial disconnect to avoid repeated promotion.
Duplicate connect and failed disconnect leave listener ownership unchanged.

Keep signal teardown as a full bucket scan. This deliberately follows the
user's priority: connect/emit and script interaction matter more than teardown.
Keep per-script state storage, transform math, renderer count/resource gates,
and the existing internal-dispatch/schedule improvements.

## Check memory + behavior

Measure 10k scripts connected to one signal with no bound parameters:
incremental requested live heap falls from 1,225,124 to 524,660 bytes
(~1,196 to ~512 KiB, 57% less). Signal setup allocation requests fall from
10,028 to 14 in this fixture. Count allocator requests, not RSS or driver memory.
Keep the arena layout and per-script payload allocation unchanged.
Keep zero allocation requests in the warm 10k-node all-move frame probe.

Pass 532 graphics and 782 runtime library tests, all-target Clippy with
warnings denied, and workspace formatting checks. Cover wrong-type writes,
unwind, generation reuse, script replacement and swap-remove, reentrant signal
edits/teardown, parameter order, and removal of multiple retained node roles.

## Measure

Use identical fixtures before and after, Windows/i9-9900K, workspace release
bench profile, and Criterion. Run compiled executable snapshots serially.
Pin each benchmark process to logical CPU 15, affinity mask 32768, with
AboveNormal priority. Leave other apps and their processes unchanged.
Use 20 samples, 1-second warmup, and a 2-second measurement target per case.
Sample `rustc`/`link` process counts every 500 ms; use pairs with zero observed
compiler/linker processes for the tables above. Do not claim an idle machine:
other app load and short unsampled compiler activity remain possible.
Discard earlier unpinned, compiler-contaminated runs from performance claims.
Repeat the listener growth + cross-var pairs in reverse executable order.
Exclude fixture setup and destruction from isolated connect timing.
Include connect-after-emit and connect-plus-emit controls, not only steady
emission, to expose costs deferred from one operation to another.

Measure script calls with both a no-op method and a method that mutates the
target's typed state and returns a Variant. Run cross-script get/set and typed
read/write controls from an active caller. Time complete warm runtime frames
separately from these smaller paths; do not convert their ratios into FPS claims.

Keep local executables, logs, compiler-load samples, and estimates in
`target/perf-stage3/`. Keep the detached baseline at
`D:/Rust/Perro-perf-stage3-baseline` with matching benchmark fixtures.
Include the concurrent physics fixes in both stage-3 source trees.

Save final estimates + intervals in `target/perf-stage3/final-estimates.json`
and `.csv`; preserve compiler flags even for excluded controls.
Save benchmark SHA-256 hashes in `target/perf-stage3/executable-hashes.json`.
Use `run-pinned.ps1`, `run-quiet.ps1`, and `export-results.ps1` in that directory
for the local paired-run workflow. Keep `quiet-results.json`,
`compiler-pinned-*.json`, per-case logs, and memory/frame-work probes as evidence.

## Carry limits forward

- Prioritize a separate fix for 16 -> 17 listener growth if real projects hit it often.
- Measure realistic cross-script dispatch + Variant conversion before a larger rewrite; this pass gives no material stateful-call or var-access gain.
- Keep transform math, game-world state, and script payload layout unchanged; cut redundant validation around writes.
- Keep signal teardown as a scan; accept that cost to cut connect bookkeeping + memory.
- Treat every time above as CPU work in its named fixture; no GPU-time or FPS claim.

## Reproduce

```powershell
cargo bench -p perro_runtime --features bench --bench signal_hotpaths -- 'signal/connect|signal/emit_matrix' --sample-size 20 --warm-up-time 1 --measurement-time 2 --noplot
cargo bench -p perro_runtime --features bench --bench node_state_hotpaths -- 'node_state/script_state/(with_state_cross_read|with_state_cross_mut|get_set_var_cross|call_method_cross|call_method_cross_state|call_method_self)/10000$' --sample-size 20 --warm-up-time 1 --measurement-time 2 --noplot
cargo bench -p perro_runtime --features bench --bench runtime_frame_memory -- 'runtime_frame/.*/10000$' --sample-size 20 --warm-up-time 1 --measurement-time 2 --noplot
cargo bench -p perro_graphics --bench cpu_prepare -- 'graphics_3d_bulk_remove/10000$|graphics_3d_retained_updates/(one|all)/10000$' --sample-size 20 --warm-up-time 1 --measurement-time 2 --noplot
cargo bench -p perro_runtime --features bench --bench runtime_frame_memory -- --memory --nodes=10000
cargo test -p perro_graphics -p perro_runtime --lib --features perro_runtime/bench
cargo clippy -p perro_graphics -p perro_runtime --all-targets --features perro_runtime/bench -- -D warnings
cargo fmt --all -- --check
```
