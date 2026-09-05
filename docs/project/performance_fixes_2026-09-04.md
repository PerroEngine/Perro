# Prf fixes — 2026-09-04

Apply prod fixes for all 10 [audit findings](performance_audit_2026-09-04.md).
Use three GPT-5.6 Sol agents for runtime, graphics, CSV/editor; root owns orchestration, audio/network, cross-review + final checks.
Keep prior audit as baseline; no commit or release in this task.

## Changes

| Finding | Prod fix | Correctness guard |
| --- | --- | --- |
| World membership rebuild | Track exact live subview count; bypass owner lookup for fresh roots; mark batch nodes dirty directly and resolve world ownership after topology exists | Nested subview ownership, remove/slot reuse + deferred rebuild tests |
| Camera-only 3D restage | Reuse static single-LOD opaque staging; update camera, frustum/HiZ params + shadow state | Force full rebuild for resource changes; retain view-dependent LOD/alpha restage; moving-camera stream bench asserts zero full rebuilds |
| CSV limits | Return immediately for zero; stop unsorted scan once enough matches exist | Preserve sorted top-k and invalid-sort-column behavior; indexed/unindexed query tests |
| Editor sort | Cache lowercase/path sort keys once per sort, in initial scan + refresh | Existing key/order semantics; equal-output A/B probe |
| Editor file diff/poll | Borrow map keys + signatures; gate before snapshot clone; poll by 1 s clock | Retain one owned snapshot per accepted job; compare snapshot before apply; remove stale project jobs + reject late results |
| Editor folder collection | Use membership set instead of repeated linear searches | Keep root folder + stable final ordering; shared/existing-folder regression |
| Deep tree construction | Skip ancestor-cycle walk for trusted detached leaves; skip UI ancestor work when moved subtree has no UI descendants | Invalidate trust on direct topology edits; retain bounded corrupt/cycle guard; mixed HUD + 1k chain tests |
| LAN drain | Cap one poll at 256 datagrams + 1 MiB; retain reusable receive buffer | Count discovery + empty packets; keep whole payloads and socket backlog; final packet may exceed byte cap by <16 KiB |
| Audio cold-cache load | Release state lock during IO/decompression/copy; recheck cache on publish; track only in-flight load tokens | Concurrent reload wins; drop cancels old load; epoch-check PCM/duration/use-count writes; clear failed-load tokens |
| Water capture | Cache single-sample blit bind group by source-view generation | Recreate on key change; release on sample-mode change + idle target release; GPU binding/creation-count checks |

Fix adjacent audio clone found by Clippy: move final `Arc` into decoder.
Keep packet payload ownership: each returned packet still owns its `Vec`; cap limits work without changing transport API.

## Bench repairs

- Keep prior idle-timer + WGSL fixes and CSV probes.
- Fix active camera-stream workload: move stream cameras each frm; assert exact sample count, draws/passes + zero static-scene restages.
- Fix audio queue benchmark: use existing typed enqueue results + new explicit worker `flush` fence; include drain time; cap chunks at 64 commands. Rename cases `submit_drain` to distinguish accepted-command throughput from old queue-only latency. `flush` blocks; use for load/test synchronization, not frame hot paths.
- Use bulk deep-chain fixtures for cold transform query benches; add explicit incremental-chain construction case. Cold-query fixtures change, so those figures are not a direct before/after comparison.

## Checks

- Pass 13 CSV + 519 graphics + 67 networking + 81 audio + 748 runtime + 43 editor tests = 1471 tests.
- Keep 23 pre-existing ignored tests in networking/audio; no full workspace unit-suite claim.
- Pass editor script build + script tests through Perro CLI.
- Pass workspace Clippy for all targets with runtime bench + audio playback features; existing unrelated test/bench clone warnings remain.
- Pass audio profile tests + `--no-default-features` compile check.
- Fix audit link caught by website docs validation.
- Keep engine profile, host and scene-size limits from baseline audit. Short benches establish large path changes; no universal FPS claim.

## Bench results

Pass all seven affected bench targets after camera-stream repair: `huge_csv`, `node_state_hotpaths`, `query_hotpaths`, `runtime_core_hotpaths`, `play_source`, `camera_stream`, `gpu_frame`.
Collect 193 Criterion case timings + 12 focused repeats, 42 GPU-frame cases + 8 active camera-stream cases.
Complete full node + query targets that stopped during baseline audit; no budget stops in this run.

Use Criterion point estimate below; use 30 samples, 1 s warmup/target + 10,000 resamples for final CPU confirmations.
Keep 10-sample baseline node figures from audit; compare large effects, not small regressions.

| Same workload | Before | After | After 95% CI |
| --- | ---: | ---: | ---: |
| CSV limit 0 / 250k rows | 859.23 us | 13.844 ns | 13.837–13.855 ns |
| CSV unsorted limit 32 / 250k rows | 866.94 us | 322.57 ns | 322.41–322.83 ns |
| CSV numeric filter + limit 32 / 250k rows | 13.743 ms | 3.8648 us | 3.8615–3.8684 us |
| Batch create 1k root Node3D | 21.374 ms | 0.34509 ms | 0.33765–0.35345 ms |
| Batch create 10k root Node3D | 1.9752 s | 2.5041 ms | 2.4840–2.5309 ms |
| Create/mutate/extract/drain 20,001 nodes | 6.750 s | 14.037 ms | 13.963–14.116 ms |

Include owned runtime teardown in node timings. Keep identical root-batch/composite workloads; do not compare changed cold-query fixtures as same-workload gains.
Measure new incremental-chain case: 1k = 0.40605 ms; 10k = 4.4359 ms (95% CI 4.4004–4.4794 ms). Scale 10x nodes -> ~10.9x time.

| GPU fixture host stage | Before CPU 3D prepare | After CPU 3D prepare | After GPU main median |
| --- | ---: | ---: | ---: |
| 100k static instances + camera move, `shader_variant_plain_generic` | 221.486 ms | 0.139 ms | 0.060 ms |
| 10k instances + point lights, `lights_point_8` | 28.035 ms | 0.043 ms | 0.059 ms |

Use final 30-warm/120-sample camera confirmation for first row; use full GPU sweep for second. Host stage is an average; GPU column is timestamp median. Keep these distinct.
Keep expensive GPU-only workloads visible: 64×128 water simulation remains ~20.0 ms GPU main median; no broad GPU-kernel speedup claim.

| Editor helper A/B / 50k paths | Old algorithm median | Prod algorithm median |
| --- | ---: | ---: |
| Asset sort | 598.588 ms | 26.781 ms |
| File diff / 1% modified | 61.033 ms | 36.262 ms |

Use same-run deterministic synthetic fixtures, 3 warm + 15 measured pairs, alternating order; assert equal outputs. Reproduce old clone/sort algorithm in local probe and call current diff/key helpers. Exclude input clone + result drop; no full-editor FPS claim.

Pass audio submit/drain cases: cached play ~1.85 us/command; spatial update ~150.5 ns/command. Include worker completion; no comparison to failed queue-only baseline.
Pass camera-stream assertions: all 60 measured frames/case render; 2D reports real stream draw counts, 3D reports draws/passes + zero full restages for static single-LOD camera moves. First retry exposes 3D-only pass counter; add separate 2D draw metric rather than accepting zero work.

Observe unrelated local build during part of first CPU sweep + first node confirmation. Repeat final node/CSV/composite/camera measurements after build activity stops; no build overlap observed in final confirmation ledgers. Keep initial timings + failures for traceability.

## Repro + artifacts

Read [initial run ledger](../../target/perf-fixes-2026-09-04/runs.json), [final camera-stream status](../../target/perf-fixes-2026-09-04/camera-repaired-run.json), [confirmations](../../target/perf-fixes-2026-09-04/confirm-runs.json), [final node confirmation](../../target/perf-fixes-2026-09-04/node-quiet-run.json).
Read [Criterion estimates + CIs](../../target/perf-fixes-2026-09-04/measurements.json), [GPU CSV](../../target/perf-fixes-2026-09-04/gpu.csv), [GPU confirmation](../../target/perf-fixes-2026-09-04/gpu-confirm.csv), [editor A/B CSV](../../target/perf-fixes-2026-09-04/editor-probes.csv), [Clippy log](../../target/perf-fixes-2026-09-04/clippy.log).
Keep raw stdout/stderr and Criterion estimates under `target/perf-fixes-2026-09-04/`; this dir is local ignored output.

```powershell
cargo test -p perro_csv -p perro_pawdio -p perro_networking -p perro_graphics -p perro_runtime --lib --tests --features perro_runtime/bench,perro_pawdio/playback
cargo run -p perro_cli -- check --path perro_editor
cargo run -p perro_cli -- test --path perro_editor
cargo clippy --workspace --all-targets --features perro_runtime/bench,perro_pawdio/playback -- -W clippy::perf -W clippy::redundant_clone -W clippy::map_entry
cargo bench -p perro_csv --bench huge_csv -- 'huge_csv_query_(unsorted|numeric)' --sample-size 30 --warm-up-time 1 --measurement-time 1 --noplot
cargo bench -p perro_runtime --features bench --bench node_state_hotpaths -- --sample-size 10 --warm-up-time 0.1 --measurement-time 0.2 --noplot
cargo bench -p perro_graphics --bench camera_stream
```
