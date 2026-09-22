# Runtime performance follow-up — 2026-09-22

Continue the first pass with runtime speed ahead of export/build time. Keep its
accepted changes. Compare against that working tree, not the original Git HEAD.
Preserve independent executables and hashes in the evidence.

## Correct the frame fixture

The first pass's frame probe omitted `clear_dirty_flags()` and only extracted 2D
commands. Its numbers describe a synthetic persistent-dirty workload. The app
extracts 2D, 3D, and UI, drains/submits commands, then clears dirty flags.

Both second-pass baseline and candidate use the corrected CPU fixture: fixed
update, update, all three extraction paths, command drain, and dirty reset.
GPU submission/presentation remain outside this probe. Correcting the benchmark
is not an engine optimization and is not counted as a speed gain.

Add `--frame-times` for coarse phase clocks without per-script clock overhead.
Add `--empty-extract --dimension=2|3` to isolate one empty renderer over a scene
containing only nodes of the opposite dimension.

## Retained changes

- Empty extraction: remember a completed full pass only if it saw no nodes of
  that spatial dimension. Arena mutation invalidates the stamp; scene reset
  clears it. Scenes containing hidden or unresolved visuals retain the previous
  retry behavior. Record the dimension during an existing node read on a clean
  full pass, stopping at the first match. Dirty passes defer the proof and skip
  these type checks. This intentionally favors compatibility over skipping every
  opposite-dimension scan during active scene changes.
- Empty UI extraction: retain a separate arena-revision stamp only after a
  complete main-world check finds no UI node and extraction leaves the arena
  revision unchanged. Reuse valid proof across input-only passes. Hidden and
  pending UI nodes prevent caching; ordinary input handling remains active.
  Only an uncached empty-bootstrap pass with an unchanged arena revision since
  the prior UI extraction performs the presence check. Moving scenes gain no
  extra proof scan; the first stable frame can establish the cache.
- HashMap to Variant: bulk-build the ordered object for at least 128 entries,
  the smallest size with measured benefit. Preserve direct insertion below that
  threshold, key conversion, and duplicate converted-key last-write semantics.
- Graphics benchmarks: add a hidden-window option and CPU stream-encode metric.
  Both production graphics candidates in this follow-up fail to show a stable
  benefit; restore their source changes and keep the benchmark improvements.

## Final CPU frame results

Use 10k scripted Sprite2D nodes, with idle, 1%-moving, and all-moving cases.
Results below are the mean of two independent process medians per executable,
with A/B/B/A order, the stable-revision UI guard, and clean-pass 2D/3D proof.
Negative deltas are faster.

| CPU probe | Baseline | Final | Time delta |
| --- | ---: | ---: | ---: |
| Idle full frame | 2.623 ms | 0.117 ms | -95.5% |
| 1% moving full frame | 2.101 ms | 2.071 ms | -1.4% |
| All moving full frame | 8.968 ms | 8.907 ms | -0.7% |
| Empty 2D extraction over 3D nodes | 0.740 ms | 0.000220 ms | -99.97% |
| Empty 3D extraction over 2D nodes | 1.413 ms | 0.000195 ms | -99.99% |

Idle improvement comes from skipping repeated empty 3D/UI work. It does not
predict GPU frame time or FPS for a game with active 3D/UI content. Timing
hundreds of nanoseconds describes a cache hit, not rendering actual content.
Active-scene controls show no slowdown in this final run, but their small
negative deltas are inconclusive on this shared host. See
[final measurements and executable hashes](performance_pass2_frames_final_2026-09-22.json).

Separate allocation probes over 1,010 warmed frames record idle allocation
requests 13,130 -> 0. Sparse/all-moving requests remain 13,130 in both builds.
All cases retain zero net heap growth. The three cache stamps increase the
runtime struct from 20,784 to 20,832 bytes (+48 bytes).

## Method and limits

Reject the first 3D candidate, an equality short circuit in `DrawChanges::between`:
the quiet first pair measured 4.267 ms baseline versus 4.393 ms candidate with
overlapping confidence intervals; later runs encountered compiler activity.
Restore that source change. Preserve [the rejected trial](performance_pass2_graphics_rejected_2026-09-22.json).

Also reject conditional instance-range validation. Hidden-window A/B/B/A on an
AMD RX 7800 XT (Vulkan), one 1,024-pixel 3D stream and 10k meshes: CPU stream
encode is 2,199/2,242 us baseline versus 2,505/1,945 us candidate. The 2D control
also varies widely. No stable speed gain supports keeping this production
change. Keep [the stream trial](performance_pass2_stream_trial_2026-09-22.json),
hidden-window option, and stream timing column for future measurement.

The first retained 2D/3D empty-scan gate reduces the corrected idle 10k-node frame
from 2.718 ms to 1.366 ms in A/B/B/A. Sparse/all-moving controls measure -5.3% and
-2.7% respectively; these smaller deltas need caution on a shared host. A phase
probe shows UI extraction accounts for about 1.212 ms of the remaining idle
frame. Preserve [this intermediate build](performance_pass2_frames_trial_2026-09-22.json)
separately from any subsequent UI-gate result.

The first UI-gate build measures 2.653 ms -> 0.118 ms for idle frames. Its active
control measures +8.2% in one run and -3.2% in a repeat, so neither is a reliable
active-scene claim. Review also finds a redundant absence-proof scan on every
moving frame. Restrict proof to a stable arena revision before the final build;
this removes that known overhead regardless of the noisy timing result.
Preserve [the initial UI trial](performance_pass2_frames_ui_trial_2026-09-22.json)
and [its active repeat](performance_pass2_frames_active_repeat_2026-09-22.json).

The stable-UI build still measures +4.6% / +3.0% sparse-frame time in A/B/B/A
and B/A/A/B, with all-moving +1.8% / -2.6%. Keep
[the stable-UI trial](performance_pass2_frames_stable_ui_trial_2026-09-22.json)
and [the reverse-order repeat](performance_pass2_frames_stable_ui_reverse_2026-09-22.json).
Phase probes also show host variation in unchanged fixed-update and command-drain
work. The final source nevertheless removes unnecessary 2D/3D type classification
from dirty passes, rather than relying on noise to dismiss the sparse result.

The allocator audit at 4,096 map entries records borrowed-conversion peak
439,072 -> 534,848 bytes (+21.8%), but retained object bytes 439,072 -> 338,240
(-23.0%). Bulk construction makes a denser BTreeMap while using temporary scratch.
The owned audit peak is 709,434 -> 598,042 bytes (-15.7%); those owned measurements
include the input clone. Timing excludes that clone with Criterion setup.
These are requested bytes from one randomized-map fixture, not RSS or GPU
memory. See [the allocation audit](performance_pass2_variant_memory_2026-09-22.csv).

Three same-binary insertion/bulk comparisons give these time changes:

| HashMap encode | 128 entries | 4,096 entries |
| --- | --- | --- |
| Borrowed | -12.5%, -12.6%, -15.5% | -26.0%, -16.5%, -19.9% |
| Owned | -19.5%, -19.3%, -20.5% | -21.5%, -19.1%, -22.7% |

These compare algorithms in the pre-change benchmark, not whole game frames.
The final separate-executable A/B/B/A is noisy: unchanged BTreeMap controls move
about 4–15%. Small-map owned encode averages +0.3%; borrowed small-map changes
track control noise. Keep the bounded large-map change for the repeated
algorithm result and lower retained memory, without claiming an exact final
cross-build speed percentage. See [all Variant estimates and hashes](performance_pass2_variant_2026-09-22.json).

Use the same Windows i9-9900K host and optimized Cargo bench profile as pass one.
Run our tests/builds separately from timing. Other project builds and editor
checks can still run on this shared host; record observed interference and avoid
claims from small or inconsistent deltas.

Frame comparisons use A/B/B/A independent processes, 60 warmup frames and 101
batches of ten frames per process. Keep allocation instrumentation in separate
processes. Signal, method, and variable source paths remain unchanged by this
follow-up, apart from generic large-map Variant encoding.

```powershell
cargo bench -p perro_runtime --features bench --bench runtime_frame_memory --no-run
python tools/bench_runtime_frames.py BASELINE.exe CANDIDATE.exe --output frames.json
# Target a suspect control without repeating allocation instrumentation:
python tools/bench_runtime_frames.py BASELINE.exe CANDIDATE.exe --output repeat.json --cases frame_sparse frame_all --order BAAB --timing-only
cargo bench -p perro_variant --bench hashmap_variant --bench hashmap_variant_alloc --no-run
cargo bench -p perro_graphics --bench camera_stream --no-run
```

Pass `--bench` when invoking Criterion executables. The custom frame and memory
probe flags select their own entry points. Use `PERRO_CAMERA_STREAM_BENCH_HIDDEN`
for the graphics probe's hidden window.

## Validation

Final clean-proof source: `cargo test --quiet --color never` passes 2,793
tests, with zero failures and 57 ignored tests. Scoped formatting checks pass
for runtime, runtime-render, Variant, and graphics. The optimized runtime frame
benchmark builds successfully. Added coverage checks empty-cache invalidation,
pending-resource retries, scene-reset invalidation, and deferring the UI absence
proof while the scene changes. Independent review finds no issue with the UI
gate's correctness.
