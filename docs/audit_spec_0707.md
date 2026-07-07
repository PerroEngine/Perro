# Audit Spec — 2026-07-07 (full pass)

Handoff doc 4 next model (Opus). Full-repo improvement map: hard findings w/ fle:line, per-crate look map, methodology, hail maries. Frm scan of 582 .rs fle / ~163k ln + 43 wgsl + workspace dep tree.

---

## 0. Auditor Rules (rd 1st)

- rd `CLAUDE.md` — caveman output rule + crate map. NOTE: CLAUDE.md stale — says `playground/` but real dirs = `demos/` (Demo2D, Demo3D), `perro_book/`, `perro_editor/` (perro prj, not crate). fx CLAUDE.md as 1st commit.
- rd memory dir `~/.claude/projects/D--Rust-Perro/memory/` — prior audit trackers there; don't redo landed work
- commit direct 2 main, no branch
- b4 rm "dead" code: chk all `cfg`/os/feature/test call sides — past bug src
- kp `perro_website` in workspace, off default-members — intentional
- profile b4 opt: add bench or tracy capture 1st; no vibes perf work
- test: `cargo test`; CLI smoke: `cargo run -p perro_cli -- --help`
- `csvs.rs` in build_pipeline currently modified + uncommitted — chk intent b4 touch

## 1. Landed (don't re-audit)

- SoA node storage phases 0–3 + phase-5 physics writeback (queries -30..-67%, boxing 1936→192B)
- query spatial `within[origin,size]` (100k rare_tag 10.2→2.5ms)
- runtime API steps 1–5 (snapshots, dispatch devirt, cache rm, mutation-ver split, merge cuts)
- hot-path pass 0705 (anim/variant/arena-scan/dirty-gate/query-clone/physics-sync)
- skeletal anim batched bone writes + 3-row palette; palette gate dirty-driven
- FramePacer + refresh cache + vsync×cap rule
- structural node counter; *_revision rename
- lighting/shadow 0702; water3D deviation sync; spatial audio virtual srcs
- UI treelist dirty-gated sync + O(n) visible_items
- workspace lto cfg + website default-members split
- **hi-z SPD: `three_d/shaders/hiz_downsample_spd.wgsl` + `three_d/gpu/culling.rs` EXIST** — memory says deferred but fle present; VERIFY landed + wired b4 re-plan (chk culling.rs call path in prepare/draw)

## 2. Hard Findings (this scan, w/ locations)

### 2.1 dup dep versions (compile time + bin size)

`cargo tree -d --workspace` shows:

| dep | vers | fix path |
|---|---|---|
| base64 | 0.13 + 0.22 | 0.13 pulled by `gltf 1.4.1` — chk gltf upgrade |
| thiserror | 1.x + 2.x | unify workspace 2 v2 |
| bitflags | 1.3 + 2.11 | fnd 1.x puller, upd |
| itertools | 0.10 + 0.14 | same |
| toml | 0.8 + 0.9 + 1.0 (3!) | unify; toml_datetime ×3 too |
| hashbrown | 0.14/0.15/0.16/0.17 (4!) | transitive; ptch via upgrades |
| ordered-float | 4 + 5 | upd |
| rustc-hash | 1 + 2 | upd |
| convert_case | 0.6 + 0.11 | upd |
| bit-vec, foldhash, getrandom, serde_spanned | ×2 each | upd |

Action: add `[workspace.dependencies]` table if not present; mv all shared deps there; run `cargo tree -d` aft, target < 5 dups. Also: no `cargo-machete` CI gate yet (carry-over).

### 2.2 unsafe w/o safety comment

159 `unsafe ` sites, only 110 `SAFETY` comments repo-wide. Audit gap ~50+. Hot spots:

- `runtime_project/perro_runtime/src/cns/{scripts,script_collection}.rs` — script ptr plumbing, highest risk
- `rt_ctx/{scripts,signals}.rs`
- `runtime/mesh_query/simd.rs`, `runtime/internal_updates.rs`, `runtime/scene_loader/mod.rs`
- `core/perro_structs/src/structs/matrix/{x86,aarch64,wasm32}.rs` — simd, likely fine, still comment
- `io_stack/perro_io/src/asset_io.rs`

Action: each unsafe block gets `// SAFETY:` w/ invariant; miri run on cns/rt_ctx tests if feasible.

### 2.3 unwrap/panic in runtime paths

408 unwrap total; non-test: 6 in `perro_runtime/src/runtime`, 7 in `perro_graphics/src`, 20 panic!/expect in runtime. Low count = prior passes work. Action: triage those ~33 by hand; shipped-game path → err or debug_assert; init-only expect w/ msg ok.

### 2.4 string-keyed maps in hot structs (intern candidate)

46 `HashMap<String`-family non-test. Key ones:

- `perro_graphics/src/resources.rs:109-121` — mesh/texture/material/decoded-texture by source str; every asset lookup hashes full path str
- `perro_graphics/src/three_d/gpu.rs:590,659,730` — custom pipeline/token/texture-slot maps
- `perro_graphics/src/postprocess/mod.rs:211-216` — lut + custom pipeline maps
- `perro_runtime_render/src/retained.rs:152,632` — particle_path_cache 2D+3D
- `perro_runtime/src/runtime.rs:143` — scene_cache
- `rs_ctx/core.rs:264-266` — bone maps behind Mutex
- `three_d/particles/gpu.rs:245` — compiled_expr_lookup

Action: intern src paths → `SourceId(u32)` at load; maps become `AHashMap<SourceId,_>` or dense Vec. Biggest win where lookup per-frame (particle_path_cache, custom_pipeline_tokens). chk callers 1st: if lookup only @ load, skip.

### 2.5 per-frame str alloc in render extract

52 `format!/to_string` in `runtime/render` non-test. Confirmed sites:

- `render/two_d.rs:902` — `"__default__".to_string()` fallback per sprite w/o src? chk freq; make `static DEFAULT: &str`
- `render/two_d.rs:1163,1807,1947-1972` + `render/three_d/helpers.rs:622-647` — particle profile cache insert path: `source.to_string()` ×4 per miss; ok if miss-only, chk
- `render/three_d/helpers.rs:739-741`, `two_d.rs:2099-2103` — expr str copy per parse; cold-ish
- `render/ui/color_picker.rs:33-59` — format! per redraw of picker; gate on value chg

### 2.6 lock surface

152 `lock()` non-test in runtime_project, spread over `rs_ctx/*` (15 fle) + `runtime/audio/*` + scene_loader. Pattern: rs_ctx = script-facing ctx, each sub-api own Mutex. Risks:

- lock per script call per frame → contention w/ many scripts
- `rs_ctx/core.rs:264-266` Mutex<HashMap<String, Vec<Bone>>> — double cost: lock + str hash
- audio solve path locks (`runtime/audio/solve.rs` 2552 ln)

Action: map which locks taken per-frame vs per-event; per-frame ones → chk if single-thread anyway (main-loop only) → replace w/ RefCell/plain; or batch: snapshot once per frame, script calls rd snapshot.

### 2.7 wgsl prelude triplication

`three_d/shaders/prelude_3d.wgsl` (1115) + `prelude_rigid_3d.wgsl` (1080) + `prelude_skinned_3d.wgsl` (1115) — 3 near-dup preludes via `include_str_stripped!` (`shaders.rs:2-6`). ~2.2k dup ln. Action: diff the 3; extract common core + tiny variant suffix; macro concat. Cuts shader maintenance bug class (fx in 1, forget 2). Note wgsl macro rebuild-tracking fix exists (mesh-blend memory) — kp that behavior.

### 2.8 test coverage holes

0 test markers in:

- `runtime_project/perro_runtime_render` — retained render state, real logic, needs tests
- `devtools/perro_dev_runner`
- `script_stack/perro_scripting_macros` + `render_stack/perro_macros` — proc macros; add trybuild/expansion snapshot tests

Thin (1 test fle): perro_compiler (big crate!), perro_asset_formats, perro_csv, perro_io, perro_meshlets, perro_graphics_assets. perro_compiler at 1 = worst gap vs size.

### 2.9 misc

- 19 `allow(dead_code)` — audit each (cfg caution)
- 47 `collect::<Vec` in runtime_project non-test — sweep per-frame ones 4 reuse-buf pattern (scratch buf conv already in codebase)
- 36 `Box<dyn` — chk hot-path virtual calls; dispatch devirt landed 4 scripts, chk rest
- 10 sorts in runtime tick paths: `physics.rs:2324,2431` staged-pose sorts per step (chk n + already-sorted fast path), `render/bridge.rs:1266`, `render/ui/events.rs:1632` focus sort per event ok
- 0 TODO/FIXME — clean, kp

## 3. Per-Crate Look Map

### core/
- `perro_variant/src/variant.rs` 4201 ln — stride 96→80B deferred item; enum layout audit; split fle
- `perro_structs/.../matrix/mod.rs` 3784 + per-arch simd fle — bench exists (`benches/math_hotpaths.rs`); chk vs glam parity, chk wasm32 path tested in CI
- `perro_nodes` — node defs; new-node checklist in memory (character-body-nodes)
- `perro_animation/src/panim/core.rs` — HashMap<String,SceneValue> vars; intern candidate if eval per-frame
- `perro_csv`, `perro_asset_formats` — 1 test fle each; fuzz candidates (decoder fuzz = carry-over item)

### runtime_project/
- `perro_runtime/src/runtime/render/{two_d,three_d}.rs` (2261/2353) + ui/ — extract path; findings 2.5; chk full-walk vs dirty-gated per node type
- `runtime/physics.rs` 3101 — broad-phase single-thread; sorts 2.4k ln in; SoA writeback landed
- `runtime/audio/solve.rs` 2552 — alloc + lock audit per solve tick
- `runtime/mesh_query/` — accel + simd; has unsafe; bench exists (`benches/unsafe_hotpaths.rs`)
- `rs_ctx/` 5915 ln 15 fle — lock map 2.6
- `runtime/scheduling.rs` only 114 ln — scheduling thin; frame-overlap hail mary lands here
- `perro_runtime_render/retained.rs` — 0 tests + str caches
- `perro_scene/node_fields.rs` 2821 — likely macro-able repetition; chk codegen option
- `perro_scene/parser.rs` — HashMap<String,SceneValue>; parse-time ok

### render_stack/
- `perro_graphics/three_d/gpu/{init 2554, prepare 2261, buffers 2206}` — chk every queue.write_buffer dirty-gated; chk buffer growth strategy (shrink never? cap?)
- `postprocess/mod.rs` 2054 — pass chain; merge fullscreen passes where format allows
- `water_gpu.rs` 2753 + water_shaders/ 1804 ln wgsl — self-contained; feature-gate candidate (compile diet)
- `ui/painter.rs` 2477 — batch-break audit: texture switch vs scissor vs z
- `gpu.rs` 2176 + `backend.rs` 2036 — surface cfg, present modes; FramePacer landed
- `perro_meshlets` — 1 test fle; gpu-driven hail mary consumer
- `perro_app/winit_runner.rs` 2702 — split candidate; input/joycon has own HashMap<String> (fine, dev freq)

### api_modules/
- `perro_runtime_api/sub_apis/node.rs` 3770 — API surface; chk macro-gen possible; 2 test fle only
- `perro_networking` — full stack (tcp/udp/websocket/http + web_stub); chk err handling + tests count; audit alloc per packet
- `perro_web`, `perro_steamworks` — stub depth? doc what's real vs stub
- `perro_input_api/keycode.rs` — unsafe here, chk why (transmute keycode?)

### script_stack/
- macros: 0 tests → trybuild
- BTreeMap in enum derive (carry-over item 5)
- macro expansion size → `cargo expand` sample, compile-time cost

### build_pipeline/
- `perro_compiler` — 1 test fle vs 2064-ln project_bundle; incremental bundle candidate; nested-cargo-check tests gated --ignored (recent commit)
- `perro_static_pipeline/csvs.rs` — UNCOMMITTED CHG, resolve 1st

### io_stack/
- `perro_io/asset_io.rs` unsafe — chk mmap already? if not: mmap pck hail mary
- decoder fuzz corpus carry-over

### audio_stack/perro_pawdio/
- codec/dsp/mic/midi mods; chk dsp alloc per block; chk codec fuzz; resample quality cfg

### devtools/
- `perro_cli/doctor.rs` 2294 ln 112 fn — table-driven checks refactor
- `perro_cli/bench.rs` has unsafe — chk

## 4. Carry-Over Open Items (prior audits)

1. prepass depth unify (render 0703) — prepass + main dup depth work
2. ~~hi-z SPD~~ — VERIFY: fle exist, likely landed post-memory
3. transform pck 4 gpu upload
4. Variant stride 96→80B
5. enum derive BTreeMap swap
6. query candidate-restricted fill
7. bench suite 4 runtime API regressions (criterion)
8. demo hubs §6.1 (engine audit 0706) — note: `playground/` gone, demos/ = Demo2D+Demo3D now
9. decoder fuzz corpus
10. cargo-machete CI gate
11. inv_bind precompute (wait 4 SoA)
12. nested-var access path

## 5. Hail Maries (big swing; profile b4+aft; 1 per session max)

1. **frame overlap job graph** — sim N+1 while rndr N; snapshot infra part-exists (runtime API snapshots); land site: `runtime/scheduling.rs` (only 114 ln now — greenfield); biggest latency/throughput swing; risk: script mutation mid-extract
2. **gpu-driven draw** — meshlets crate + multimesh_cull.wgsl + hiz exist; next: indirect draw all static geo, bindless materials; kills cpu draw-call cost
3. **rayon over SoA lanes** — transforms/anim/particles data-parallel now; hazard: rs_ctx locks (2.6) — fx lock map 1st
4. **full SoA phase 6** — rm remaining boxed cold fields; hot/cold split so hot node data ≤ 1 cache line
5. **str interning engine-wide** — SourceId u32; unblocks 2.4 wholesale; touch resources.rs + retained.rs + rs_ctx
6. **pipeline disk cache** — wgpu pipeline cache API; cut startup stutter; pairs w/ prelude dedup 2.7
7. **incremental scene serialize** — dirty-node save; editor QoL
8. **compile diet** — feature-gate water (2.7k rs + 1.8k wgsl), meshlets, midi/mic; `cargo build --timings` 4 long poles; dup-dep unify 2.1 helps too
9. **mmap asset pck** — asset_io.rs; zero-copy rd 4 big pck
10. **arena hot/cold field split** — 192B → hot 64B slab + cold side arr

## 6. Methodology 4 Opus

- 1 session = 1 theme; wr res back 2 memory tracker fle
- b4 perf claim: criterion bench or tracy capture in repo; cite nums in commit msg
- grep recipes used here (rerun 2 re-verify):
  - dups: `cargo tree -d --workspace`
  - locks: `grep -rn "lock()" perro_source/runtime_project --include="*.rs" | grep -v test`
  - str maps: `grep -rn "HashMap<String" ... `
  - unsafe gap: count `unsafe ` vs `SAFETY`
  - hot alloc: `format!|to_string()` under `runtime/render`
- suggested order:
  1. hygiene sweep: CLAUDE.md fx, csvs.rs resolve, dup deps 2.1, machete gate (½ session, safe)
  2. unsafe SAFETY audit 2.2 + unwrap triage 2.3
  3. bench suite (item 7) — unblocks perf work
  4. lock map 2.6 + str intern 2.4 (pairs)
  5. wgsl prelude dedup 2.7 + pipeline cache
  6. verify hi-z status → prepass depth unify
  7. then hail maries, 1 per session
