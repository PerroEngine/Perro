# Runtime-first performance pass — 2026-09-22

Favor game runtime over export/build time. Keep asset formats, compression codec,
decompression, and generated runtime data unchanged. Reject build gains that
require slower game execution.

Baseline: `2ab848867d0887b7492c02d70ba7300d3a2a8fe4`.
Host: Windows, Intel i9-9900K (8 cores / 16 logical CPUs), Rust 1.97.1.
Use the existing optimized bench profile: opt-level 3, no LTO, 64 codegen units.
These are CPU/heap probes, not an end-to-end GPU FPS claim.

## Keep

| Path | Change | Runtime boundary |
| --- | --- | --- |
| Nested object patches | Create dotted-path scratch only after a direct-child miss | Keep existing general get/set walkers and dotted-key fallback |
| Graphics resource GC | Compact candidate buffers in place | Keep reference counts, expiry, drop budgets, and order |
| 2D rect dirty ranges | Merge in input buffer; skip sort for ordered input | Keep the same union of upload ranges |
| Static CSV export | Borrow pooled strings; remove unused key copies; borrow strings that need no escaping | Emit identical Rust data |
| Archive assembly | Reserve exact header + payload + index capacity | Emit identical archive bytes; leave codec/decoder unchanged |

GC keeps peak candidate-buffer capacity for reuse until the store drops. This
avoids rebuilding three vectors per candidate-heavy sweep; it does not promise
lower retained memory after a resource-unload spike.

## Measure runtime

Use independent baseline/candidate executables in both orders. Ignore Criterion's
automatic comparison with whichever run happened to occupy its output directory.
Runtime query/signal probes use logical CPU 15, AboveNormal priority, 10 samples,
0.3 s warmup, and a 2 s measurement target.

| Probe | Baseline | Candidate | Interpretation |
| --- | --- | --- | --- |
| Sparse rect prep, 10k changes / 100k rects | 1.199 / 1.212 ms | 1.162 / 1.189 ms | About 2.5% lower mean; confidence ranges overlap |
| Sprite prep control, 10k sprites | 0.740 / 0.759 ms | 0.745 / 0.721 ms | Flat |
| GC candidate scan, 8,192 of each resource kind | About 0.420–0.433 ms | Same range | Time flat; buffer reuse verified |

Earlier graphics runs overlapped another project's compilation and had severe
noise. Preserve them in the evidence; use the final anchored 2D repeat for the
table. That repeat had no rustc process at its start/end snapshots. Do not infer
continuous host isolation from two snapshots.

Fresh query trial binaries supersede the initial 12–27% rare-query improvement:
that earlier result does not survive the repeat. Restore the original query
implementation; retain the new broad-union benchmark fixture.

See [graphics estimates and executable hashes](performance_graphics_2026-09-22.json).

For direct object patches, defer dotted-path allocation until a direct child
lookup misses. The final A/B/B/A repeats give:

| Probe | Baseline means | Candidate means | Interpretation |
| --- | --- | --- | --- |
| One key / 128 fields | 275.7 / 284.6 ns | 218.5 / 220.9 ns | 21.6% lower mean |
| One key / 4,096 fields | 331.8 / 337.9 ns | 280.5 / 276.6 ns | 16.8% lower mean |
| 64 keys / 128 fields | Repeat mean | Repeat mean | +0.1%, flat |
| 64 keys / 4,096 fields | Repeat mean | Repeat mean | -1.8%, no strong claim |

The first repeat also shows about 27% lower time for one key / eight fields.
An unchanged direct-set control drifted by 5.8% in the second repeat; do not
interpret small deltas as wins. General nested get/set walkers remain unchanged.
See [final script estimates, run order, and hashes](performance_scripting_2026-09-22.json).

After reverting the query experiment, repeat the single-listener signal emit
control in three pairs (A/B, B/A, A/B):

| Pair | Baseline | Final | Interpretation |
| --- | --- | --- | --- |
| 1 | 27.903 ns | 29.143 ns | Final noisy: 95% interval 28.123–30.958 ns |
| 2 | 27.981 ns | 28.045 ns | Overlapping intervals; flat |
| 3 | 27.807 ns | 27.860 ns | Overlapping intervals; flat |

The earlier apparent signal slowdown does not persist in the last two pairs.
Signal source stays unchanged; do not attribute the difference to a specific
query instruction or claim an emit optimization. Earlier connect, churn, and
stable-update controls fluctuate; preserve their trial data without a win claim.

Final-only cross-script characterization at 10k nodes: variable get/set
339.43 us (95% interval 337.52–341.20), no-op method calls 286.94 us
(285.57–288.74), and stateful calls 380.33 us (377.85–382.94).
These are whole fixture passes, not per-call times or before/after gains.
See [final runtime controls and hashes](performance_runtime_final_2026-09-22.json).

**Follow-up audit:** this first-pass frame fixture omitted the app's
`clear_dirty_flags()` at frame end and only ran 2D extraction. The figures below
therefore describe a synthetic persistent-dirty workload, not the app's normal
steady-state frame. The second pass corrects the fixture for both baseline and
candidate; do not count the fixture correction as an engine speed improvement.

The first-pass CPU frame fixture has 10k sprites with shared scripts. A frame includes
fixed update, update, 2D command extraction, and draining those commands. Allow
all CPUs at normal priority; warm up 60 frames and measure 101 batches of ten
frames in each of three independent processes per case.

| Final-only frame case | Median of process medians | Process median range |
| --- | --- | --- |
| Idle scripts | 2.929 ms | 2.826–3.029 ms |
| 1% of nodes move | 3.169 ms | 3.076–3.355 ms |
| All nodes move | 5.023 ms | 4.950–5.073 ms |

Separate allocator-instrumented runs record zero allocation requests and zero
net live-byte growth during the 1,010 measured frames for each warmed fixture.
This excludes scene creation, warmup, GPU work, and allocations outside the Rust
global allocator. No baseline full-frame executable was measured in this pass:
these numbers characterize the retained build and do not establish a frame-rate
improvement. See [frame samples, method, and executable hash](performance_frames_2026-09-22.json).

## Measure export separately

Alternate five baseline/candidate process pairs, seven samples per process,
four Rayon workers. Run allocator instrumentation separately from timing.
Time includes export, output writes, and result destruction; excludes fixture
creation, output removal/marker edits, and output validation.

| Probe | Baseline median | Candidate median | Paired time delta | Peak requested heap |
| --- | --- | --- | --- | --- |
| Cold CSV, 25k unique rows | 75.32 ms | 58.06 ms | -23.5%, all five pairs faster | 19.55 -> 16.07 MB (-17.8%) |
| 32 MiB archive, tiny entry changes | 53.29 ms | 36.98 ms | -29.6%, all five pairs faster | 100.67 -> 67.11 MB (-33.3%) |
| Unchanged archive control | 13.35 ms | 13.44 ms | -0.7%, mixed pairs | About 33.56 MB, unchanged |
| Tiny codegen control | 1.200 ms | 1.205 ms | -1.1%, mixed pairs | About 10.7 KB, unchanged |

The time columns are medians of process medians. The paired delta is the median
of candidate/baseline ratios, so it need not equal the ratio of those columns.
All four probes match output checksums. Archive rebuild reuses the large encoded
payloads and changes a tiny entry; it is not a cold compression-speed claim.
Heap numbers count requested live bytes above the sample's starting live heap,
not RSS, allocator overhead, or GPU memory. Another project's compiler was
observed during these runs; the unchanged controls stayed roughly flat.

See [raw export samples, memory probes, hashes, and checksums](performance_exports_2026-09-22.json).

## Reject

| Candidate | Evidence | Decision |
| --- | --- | --- |
| Stack-backed general nested-member path | Small gets improve about 22%, but 128-field sets rise about 3.3%; larger sets fluctuate | Restore original get/set path; narrow the change to lazy patch scratch |
| Expand fixed-array stack decode from 16 to 128 | Decode128: 330/332 ns baseline vs 405/407 ns candidate; heap-reference control flat | Keep original threshold; avoid about 23% regression |
| Inline query child plans + reuse candidate vectors | Rare-tag/name at 10k nodes: 151.35/151.47 us baseline vs 152.54/153.65 us trial; broad union varies -0.6% to +4.0% | Restore query implementation; no repeatable runtime win |
| Restore signal reverse index | Prior pass deliberately removed it for connect/emit speed and memory | Keep prior runtime priority; no implementation change |

See [rejected-prototype timings, confidence intervals, order, and hashes](performance_rejected_2026-09-22.json).
See [query trials and earlier runtime controls](performance_runtime_trials_2026-09-22.json).

## Validate

`cargo test --quiet`: exit 0; **2,785 passed, 0 failed, 57 ignored** across 99
test-result summaries, including doctests and script compile/UI checks. This is
the workspace's default engine test set; the website is outside default-members.
Ignored probes were not counted as passes. This run preceded the final query
revert, which also removed two prototype tests. After restoring its original
implementation, `cargo test -p perro_runtime --lib rt_ctx::query` passes all 17
query tests. Scoped `cargo fmt --check` and `git diff --check` also pass.

Cover query fallback/order/dedup, nested dotted and Unicode keys, resource GC
budgets and buffer reuse, unordered/adjacent rect ranges, CSV dedup/escaping,
archive size arithmetic, archive roundtrips, and incremental output equivalence.
Keep the complete local test log at `target/perf-2026-09-22/cargo-test.log`.

## Reproduce

Build each baseline before source edits and preserve its executable. For a fresh
checkout of the baseline commit, copy the final benchmark harness additions
unchanged before building. The added export cases and persistent `broad_any`
query fixture use existing public APIs.

```powershell
cargo bench -p perro_static_pipeline --bench export_hotpaths --no-run
python tools/bench_exports.py BASELINE.exe CANDIDATE.exe --output export-results.json --repeats 5 --samples 7
cargo bench -p perro_graphics --bench cpu_prepare --bench resource_gc --no-run
cargo bench -p perro_scripting --bench nested_fallback_audit --bench variant_hotpaths --no-run
cargo bench -p perro_runtime --features bench --bench query_hotpaths --no-run
cargo bench -p perro_runtime --features bench --bench signal_hotpaths --bench node_state_hotpaths --bench runtime_frame_memory --no-run
```

For final-only frames, run the `runtime_frame_memory` executable with
`--architecture-probe --nodes=10000 --case=frame_idle` (or `frame_sparse`,
`frame_all`). Add `--architecture-alloc` in a separate process for allocation
counts; do not use those instrumented timings as the ordinary timing result.

Pass `--bench` when invoking Criterion executables directly; otherwise they may
only run smoke checks. Anchor regex filters with `$` so `/10000` does not also
match `/100000`. Retain independent estimates instead of relying on Criterion's
previous-run change percentage. Run timing, compilation, and correctness tests
in separate phases.
