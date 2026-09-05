# Prf audit — 2026-09-04

Keep this report as pre-fix baseline; read [prod fixes + validation](performance_fixes_2026-09-04.md) for follow-up status.

Scan commit `90390d5dbba8d843634b7dde4396080cd2c3d958`.
Scan 44 workspace crates + editor + demos + website; 928 Rust + 60 WGSL fles.
Use repo-wide src-pattern scan + manual hot-path review + Clippy + bench runs.
Treat scan coverage as triage; ! proof of zero issues in every fn.

Run 43 bench targets -> 40 full pass aft shader repair, 1 audio failure, 2 partial targets.
Collect 612 unique Criterion cases (585 broad + 45 focused measurements; 18 repeat IDs), 42 GPU-frame cases + 8 camera-stream cases.
Rank 10 prf candidates below; apply bench fixes only.

## Ranked fx

| Rank | Path | Evidence + next fx |
| --- | --- | --- |
| P1 | [World membership rebuild](../../perro_source/runtime_project/perro_runtime/src/runtime/world_state.rs#L198) | `NodeArena::insert` bumps structural revision; `NodeAPI::create` calls `mark_needs_rerender` -> `sub_view_ancestor` -> `has_sub_views` -> full world-membership rebuild. Rebuild visits all nodes + allocs visited/stack/member storage even with zero subviews. N individual creates -> O(N²); bulk specs also mark each insert. Add root-parent fast path, maintain cheap subview presence, defer/coalesce batch rebuilds. Preserve ownership for nested subviews + reparent/remove. This explains costly flat-node bench setup as well as deep-tree setup; ! blame all cost on transforms. |
| P1 | [Camera-only 3D prepare](../../perro_source/render_stack/perro_graphics/src/three_d/gpu/prepare.rs#L468) | Camera uniform chg sets `scene_changed`; retained fast path requires `!scene_changed`; transform patch path requires `!draws_unchanged`. Thus static draws + camera-only move fall thru to full scene restage at line 892. GPU fixture toggles camera x by 1e-6. `shader_variant_plain_generic`: 100k static instances -> ~221 ms CPU `gpu_prepare_3d`, GPU median ~74 us. Add camera-only path; update uniforms, cull, shadows, view-dependent LOD/order while reusing static staging. ! simply drop gate without preserving those dependencies. |
| P1 | [CSVQuery::run](../../perro_source/core/perro_csv/src/lib.rs#L710) | Scan + collect all matches b4 unsorted `limit`; even `limit(0)` scans table. At 250k rows: ~0.86 ms for 0 rows; ~13.74 ms for numeric filter + 32 rows. Return early for zero; stop unsorted scan at limit. Preserve sorted top-k + invalid-sort-col behavior. |
| P2 | [Editor asset sort](../../perro_editor/res/scripts/assets/editor_assets.rs#L302) + [key fn](../../perro_editor/res/scripts/assets/editor_files.rs#L8) | `sort_by_key` recomputes heap-backed lowercase/path key per cmp. Use `sort_by_cached_key` or precompute keys once. Probe uses actual key fn + equal-output assert. |
| P2 | [Editor file diff](../../perro_editor/res/scripts/assets/editor_file_watch.rs#L97) + [poll](../../perro_editor/res/scripts/scene/editor_viewport.rs#L614) | Clone path + full sig into two `BTreeMap`s every scan. Also clone all prior sigs on UI thread every 30 frm, b4 worker-busy check. Borrow map keys/vals; share sig snapshot; gate job b4 clone. Replace frm-count poll w/ elapsed-time budget or file events. At 144 fps, 30 frm = up to 4.8 scan requests/sec. |
| P2 | [Editor folder list](../../perro_editor/res/scripts/assets/editor_assets.rs#L289) | Per-file parent loop calls `folders.iter().any` + `out.iter().any`; O(files² × depth) worst case. Build membership set once; retain final sort order. Also runs during asset refresh, ! just idle bg poll. |
| P2 | [Deep tree construction](../../perro_source/runtime_project/perro_runtime/src/rt_ctx/nodes/node_api.rs#L518) | `reparent` walks all proposed-parent ancestors for cycle check, plus transform/UI work. Repeated fresh-leaf attach on depth N chain -> O(N²) ancestor visits. Use bulk `NodeSpec`/scene construction for fresh trees after world-membership fix above; consider valid-leaf fast path while preserving cycle/corruption guards. Slow deep-tree bench setup exposes these costs outside reported query time. |
| P2 | [LAN drain](../../perro_source/api_modules/perro_networking/src/multiplayer/lan_transport.rs#L116) | Loop drains socket until WouldBlock; alloc packet `Vec` per recv; no packet/byte/time budget. Sustained input -> unbounded work in one poll. Add per-tick budget + reuse event scratch; bench burst + steady traffic. No network timing in this audit. |
| P2 | [Audio cold-cache load](../../perro_source/audio_stack/perro_pawdio/src/player/cache.rs#L4) + [caller](../../perro_source/audio_stack/perro_pawdio/src/player/playback.rs#L25) | Hold shared player-state mutex thru disk/archive load, static decompression + byte copy on cache miss. Other player controls wait behind IO. Load outside lock; recheck cache/asset epoch at insert. Device callback impact ! established. |
| P3 | [Water capture](../../perro_source/render_stack/perro_graphics/src/water_gpu.rs#L779) | Create bind group each single-sample capture w/ visible 3D water. Cache by source texture-view identity + sampler/layout; invalidate on target resize/replacement. CPU alloc win candidate; GPU win ! measured. |

## Bench validity

- Fx [timer idle bench](../../perro_source/runtime_project/perro_runtime/benches/timer_hotpaths.rs#L18): 60 simulated seconds expire after ~3600 ticks, often inside warmup. Old steady-state result measures empty heap. Use `Duration::MAX`; assert active/deadline counts aft samples.
- Add [CSV limit probes](../../perro_source/core/perro_csv/benches/huge_csv.rs#L85): limits 0 + 32, numeric filter + limit 32; assert output count b4 timing.
- Fx [shader bench](../../perro_source/render_stack/perro_graphics/benches/shader_wgsl.rs#L6): use composed material prelude for shared helpers; include chroma-key + pixel-art FX required by builtin post body. Initial run fails on missing helper; first repair exposes missing post FX; final rerun passes all 19 build/parse/validate cases.
- Fnd audio controller bench panic at [play_source.rs:306](../../perro_source/audio_stack/perro_pawdio/benches/play_source.rs#L306): `play_source` returns false. Queue cap = 4096; periodic `source_length_seconds` hits cache, so ! drain barrier. Likely queue saturation; bool result hides Full vs Disconnected. Add typed-error reporting + explicit backpressure/accepted/rejected cases; ! treat this as device failure or engine throughput result.
- Fnd [camera-stream redraw](../../perro_source/render_stack/perro_graphics/benches/camera_stream.rs#L341) resubmits same camera. Six render-target stream cases report zero draw/encode/submit work aft warmup -> retained idle path, ! active stream throughput. Two webcam cases upload fresh bytes + draw. Exit 0 alone ! proof of useful active-work timing; add per-frame invalidation + draw-count assertions b4 using six idle figures.
- Kp two broad targets partial: `node_state_hotpaths` stops at wide-tree fixture after ~385 s (next estimate 660 s); `query_hotpaths` stops at 50k fixture after ~236 s (next estimate 560 s). Slow setup + remaining cases exceed per-target budget. Preserve partial logs; rerun smaller node/script/query cases successfully. ! full pass for either target.
- Kp prod engine/editor src unchanged; list fx candidates separately from bench harness fixes.
- Use 10 samples, 100 ms warmup, 200 ms target, 1000 resamples for broad Criterion triage. Existing per-group overrides still apply; mesh LOD cases request 15 s.
- Kp timing runs serial; finish build b4 bench run. Short runs -> triage, ! small-regression proof.
- Distinguish Criterion total closure time from internal `prepare_cpu` or GPU timestamp. `black_box(timing.prepare_cpu)` ! restrict Criterion timer to that field.
- Distinguish synthetic math/lookup cases from engine paths; ! treat them as game-frame time.
- Treat cold transform benches as query + owned runtime teardown: `iter_batched` consumes runtime in timed closure. Use `iter_batched_ref` b4 attributing all cost to transform fn.
- Treat material sweep as repeated writes to **one** material; setup creates one ID despite comment about distinct materials. ! evidence for 2048 independent material assets.
- Use bench opt-level 3; release default = 2 except package overrides. Bench figures ! exact ship-profile figures.
- Include actual hardware GPU benches; CPU graphics benches without surface ! GPU measurements.
- Treat timer idle ns/op as one heap-front check, ! count timers processed; existing Gelem/s display uses resident count and overstates actual work.

## Scan result

- Pass `cargo clippy --workspace --all-targets --features perro_runtime/bench,perro_pawdio/playback -- -W clippy::perf -W clippy::redundant_clone -W clippy::map_entry`.
- Fnd 0 `clippy::perf` group warn; 1 prod redundant `Arc` clone at `player/playback.rs:65`; remaining clone warns in tests/bench. Rank clone below IO-lock scope.
- Pass all 43 bench-target builds, incl runtime `bench` + audio `playback` features; build time 7m 51s.
- Pass final Clippy recheck + Rust 2024 format check for all three bench edits; no full unit-suite run.
- Recheck old water audit: per-chunk LOD + per-LOD draw groups already exist at `water_gpu/chunks.rs:101` + `water_gpu.rs:1346`. Do ! propose those as missing.
- Recheck physics overlap membership: `AHashSet`, ! quadratic Vec search.
- Recheck graph transforms: dirty-index + cached path; ! blanket full-arena-scan claim.
- Recheck website search: lowercase search text at build/load stage; ! lowercase full doc per keystroke. Route/group scans remain secondary candidates without timing evidence.
- Cover core/data/math/UI, runtime/physics/scenes, render/GPU/shaders, audio, API/jobs/network, IO/assets, scripts/macros, build/devtools, website, editor + demo scripts. Asset blobs + generated/ignored build output exclude from src scan.

## CPU baseline picks

Use Criterion point estimate (slope, or mean for flat samples). ! before/aft engine speedup.

| Case | Time | Scope |
| --- | ---: | --- |
| `runtime_core/create_nodes_10k_batch_transform_and_render` | 6.750 s | Misleading name: 10k Node2D + 10k Sprite2D + parent = 20,001 nodes; create + mutate + extract + drain + teardown |
| `runtime_core/transform_dirty_propagate_and_refresh` | 1.148 ms | 10k-node chain query + owned fixture teardown; setup outside timer |
| `graphics_3d_blend_prepare/100000` | 52.459 ms | Submit + headless draw path; ! GPU time |
| `animation_tree/pose_apply_frame/tracks/100` | 34.717 us | Runtime update on 100 animation tracks |
| `label3d_ui/projection_only/1000` | 1.345 ms | CPU UI paint prep |
| `label3d_ui/text_change/1000` | 7.422 ms | CPU UI text-change path |
| `material_param_write_cycle/params_128/2048` | 19.368 ms | Full read/modify/write + queue drain; one material |
| `material_set_param/params_128/2048` | 0.425 ms | Narrow setter + queue drain; same one-material sweep |
| `physics/runtime_fixed_step_resting/resting_2d_4096` | 5.998 ms | Existing resting-body fixture |

Use existing narrow material setter for per-frame param chg: ~45.5x path-time ratio in above sweep. ! claim for arbitrary scenes or 2048 distinct materials.

## Focused probes

Use 30 samples + 1 s warmup/target for CSV + timer; 10 samples + 100/200 ms for node/query. Use 10,000 resamples.

| Case | Point estimate | 95% CI |
| --- | ---: | ---: |
| Batch create 1k root Node3D | 21.374 ms | 20.580–22.403 ms |
| Batch create 10k root Node3D | 1.9752 s | 1.9502–1.9996 s |
| CSV unsorted limit 0 / 250k rows | 859.23 us | 851.57–865.37 us |
| CSV unsorted limit 32 / 250k rows | 866.94 us | 849.31–886.94 us |
| CSV numeric filter + limit 32 / 250k rows | 13.743 ms | 13.477–14.031 ms |
| Idle tick / 100k live timers | 9.625 ns | 9.602–9.662 ns |
| Selective query vec / 2500 nodes | 11.509 us | 11.454–11.559 us |
| Selective query mask / 2500 nodes | 11.406 us | 11.230–11.623 us |

Scale node count 10x -> ~92x create-path time; consistent w/ full world-membership rebuild per insert. Include owned runtime teardown in create timing; ! isolated insert-only time. Query mask/vec CIs overlap -> no firm win from this probe.

## Editor A/B probes

Use actual repo sort-key + file-diff fns; compare local candidates in [probe src](../../target/perf-audit-2026-09-04/editor_perf.rs).
Assert equal output; use deterministic synthetic path sets, 3 warm pairs + 15 measured pairs, alternating order.
Exclude input clone + output drop from timer. Modify 1% of signatures for diff fixture.

| Case | Count | Current median | Candidate median | Ratio |
| --- | ---: | ---: | ---: | ---: |
| Asset sort -> cached keys | 1k | 7.460 ms | 0.428 ms | 17.4x |
| Asset sort -> cached keys | 10k | 96.785 ms | 4.796 ms | 20.2x |
| Asset sort -> cached keys | 50k | 582.164 ms | 26.954 ms | 21.6x |
| File diff -> borrowed maps | 1k | 0.792 ms | 0.434 ms | 1.83x |
| File diff -> borrowed maps | 10k | 9.912 ms | 5.805 ms | 1.71x |
| File diff -> borrowed maps | 50k | 59.017 ms | 35.134 ms | 1.68x |

Treat ratios as helper-level synthetic results; ! full-editor speedup. Kp candidate code in probe; prod editor unchanged.

## GPU baseline picks

Use Vulkan, 1280×720, vsync off / Immediate, 30 warm + 120 measured frm/case; wait for GPU idle each frm.
Use GPU timestamps below; ! CPU stage durations or steady pipelined FPS.

| Case | GPU main median | GPU main p95 |
| --- | ---: | ---: |
| `empty_present` | 0.027 ms | 0.027 ms |
| `sprites_100k_same_z` | 3.612 ms | 3.663 ms |
| `water_sim_16_64_i2` | 5.166 ms | 5.213 ms |
| `blend_sphere_256_smooth` | 10.520 ms | 10.742 ms |
| `water_sim_64_128_r2_i2` | 20.236 ms | 20.400 ms |

Separate host cost: `shader_variant_plain_generic` reports **221.486 ms CPU 3D prepare**, versus **0.074 ms GPU main median**; `lights_point_8` reports 28.035 ms CPU prepare versus 0.073 ms GPU. Prioritize camera-only restage fix for these workloads; GPU shader tuning alone ! address host bottleneck.

## Host + repro

- Use Windows x86_64 MSVC; rustc 1.97.1; LLVM 22.1.6.
- Use Intel i9-9900K, 8 cores / 16 threads, ~64 GiB RAM.
- Use AMD Radeon RX 7800 XT; driver 32.0.11029.1008.
- Kp raw logs, exact exe paths/args + estimates in [audit dir](../../target/perf-audit-2026-09-04/).
- Kp [source inventory](../../target/perf-audit-2026-09-04/source-files.txt), [pattern hits](../../target/perf-audit-2026-09-04/source-hits.txt), [Clippy log](../../target/perf-audit-2026-09-04/clippy.log), [run ledger](../../target/perf-audit-2026-09-04/runs.json).
- Read [all Criterion estimates + CIs](../../target/perf-audit-2026-09-04/measurements.json), [GPU CSV](../../target/perf-audit-2026-09-04/gpu.csv), [editor CSV](../../target/perf-audit-2026-09-04/editor-probes.csv), [focused ledger](../../target/perf-audit-2026-09-04/focused-runs.json), [final shader status](../../target/perf-audit-2026-09-04/shader-repaired-run.json), [query repr status](../../target/perf-audit-2026-09-04/query-repr-run.json).
- Kp original failures in broad/focused ledgers; use final shader status for repair result. Logs under ignored `target/`; preserve dir for later comparisons.

```powershell
cargo bench --workspace --benches --features perro_runtime/bench,perro_pawdio/playback --no-run
cargo bench -p perro_csv --bench huge_csv -- --sample-size 30 --warm-up-time 1 --measurement-time 1 --noplot
cargo bench -p perro_runtime --features bench --bench timer_hotpaths -- --sample-size 30 --warm-up-time 1 --measurement-time 1 --noplot
cargo bench -p perro_graphics --bench shader_wgsl -- --sample-size 10 --warm-up-time 0.1 --measurement-time 0.2 --noplot
```

Limit coverage to host/default feature graph + named bench features; ! all platform/Steam/WASM feature variants.
Editor/demo script review = src only; ! full interactive app test.
Keep GPU driver/window/scheduler noise in mind; ! compare old audit figures as same-run regression baseline.
