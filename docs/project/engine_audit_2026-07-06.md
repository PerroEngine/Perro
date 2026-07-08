# Perro Audit 2026-07-06

full workspace sweep: 39 crates, ~162k loc rust, 1005 tracked fles.

sweep method:

- clippy `--workspace --all-targets` -> 0 warn
- `cargo test --workspace` -> all pass (1200+ tests, 0 fail)
- grep sweep: unwrap/expect/panic/todo/dead-code/println
- cargo-machete dep scan
- rd hot fles + recent commits

verdict: codebase health = high. no crash bug fnd. main iss = test cost, dead code bits, 1 known stale-flag bug, build hygiene.

---

## 1. Bugs

### 1.1 audio scene flag gate — stale flags — DONE (`fe294c05`)

- fx land: NodeArena `structural_version` (bump only insert/rm/clear/reparent). audio gate -> structural_version. +regression test.
- open bonus: wire same counter -> query cache / physics collect / ui treelist gates.

- fle: `perro_runtime/src/runtime/audio/scene.rs:113` + `audio.rs:340`
- gate = node COUNT snapshot
- add+rm pair same tick -> count same -> flags !refresh
- ex: rm AudioMask2D + add Node3D -> `has_audio_mask_2d` stay true 4ever
- fx: !use `mutation_version()` (bump on any write -> full rescan / tick = worse)
- fx: add structural-only counter on node arena (insert/rm ver, !general mutation)
- bonus: counter unlock cheap structural gates engine-wide (query cache, physics collect, ui treelist)

### 1.2 pskel decode — unbounded alloc frm fle bytes — DONE

- fx land: clamp `with_capacity` vs remaining payload len. +regression tests (hostile count -> Err, no OOM).
- `skeleton.rs` decode_pskel + decode_pskel_2d: `bone_count.min(raw.len()/MIN_BONE_RECORD_BYTES=8)`
- `perro_render_bridge/two_d.rs` decode_tileset_2d_binary: `tile_count.min(rem/14)` + polygon `count.min(rem/8)`
- audit sibling decoders:
  - pmesh (`mesh_query/decode.rs:200`) — SAFE, guard `raw.len()` b4 all with_capacity
  - panim (`rs_ctx/animation.rs`) — SAFE, text parse (`parse_panim`), no bin count
- still open: cargo-fuzz corpus 4 bin decoders (6.3)

orig iss:
- fle: `perro_runtime/src/rs_ctx/skeleton.rs:470`
- `Vec::with_capacity(bone_count)` — bone_count rd raw frm fle u32
- corrupt/hostile fle: bone_count = 4B -> huge alloc -> abort
- len guards ok (chk b4 slice rd) — alloc = only hole

### 1.3 spatial audio reconcile — verify gap

- reconciler + persistent field land (555af696, ef1d6e12)
- demo verify still pending
- fx: run audio demo, chk pan boost + virtual src vs expect

---

## 2. Dead Code

### 2.1 pass DONE — cfg/test-aware re-verify

lesson: 1st grep use `grep -v test` -> HIDE test-only + cfg callers. re-verify all refs (no filter) b4 rm.

rm (true dead all targets):

| fle | item | act |
| --- | --- | --- |
| `core/perro_animation/Cargo.toml:7` | dep `perro_ids` unused | RM — 0 ref whole crate |
| `perro_compiler/src/script_methods.rs:1154` | `rel_to_path` | RM — 0 caller; dup of `dlc_/res_rel_to_path` |
| `perro_runtime/src/rs_ctx/animation_tree.rs:103` | `is_animation_tree_id_pending` | RM — 0 caller any cfg |

KEEP — audit claim stale/wrong (NOT dead):

| fle | item | why kp |
| --- | --- | --- |
| `perro_runtime/src/runtime/physics.rs:420,432` | `prepared_audio_raycast_2d/3d` | NOW wired -> `audio/solve.rs` (many sites). audit stale |
| `perro_runtime/src/runtime/audio/zones.rs:601,618` | `play_runtime_audio_2d/3d` | `#[cfg(test)]` helpers — 30+ call in `audio/tests.rs`. rm -> break tests |
| `perro_runtime/src/runtime/mesh_query/simd.rs:142` | `x86` sse mod | WIRED: `accel.rs:611` -> `simd::` -> `x86::` on x86 arch. `allow` = cross-arch defensive, kp |
| `perro_runtime/src/runtime/internal_updates.rs:11` | `rebuild_internal_node_schedules` | hot-reload primitive; kp + add wire-by comment |

rule proven: cfg/os/feat + test callers hide frm naive grep. verify ALL refs b4 rm. never trust `allow(dead_code)` = truly dead — check call side cfg (win vs mac/linux/wasm, feature gates, `#[cfg(test)]`).

kp (intent):

- `perro_steamworks/src/disabled.rs` — feature-off stub, kp
- `perro_runtime_api` `allow(unused_variables)` in macro-gen arms — kp

rule 4ward: new `#[allow(dead_code)]` need comment w/ why + wire-by date. no comment -> rm at next audit.

---

## 3. Tests

### 3.1 big win: compiler tests = 243s of 250s total

`perro_compiler` unit tests: 21 tests, **243s** — ~97% of whole workspace test wall time.

cause: 3 tests spawn nested `cargo check` on generated prj crate w/ fresh target dir -> full engine dep tree compile inside test:

- `generated_state_all_variant_types_compiles` (`tests.rs:524`)
- `set_var + nested var` compile chk (`tests.rs:777` region)
- `generated_project_crate_compiles_after_static_embed` (`tests.rs:781`)

val: high — real end-2-end proof gen code compiles. but wrong tier 4 default `cargo test`.

fx:

1. tag 3 tests `#[ignore = "nested cargo check; run in CI slow job"]`
2. CI: add job `cargo test -p perro_compiler -- --ignored`
3. res: dev inner loop `cargo test` ~4min -> ~10s

### 3.2 trybuild borrow matrix = 21.7s

- `perro_scripting/tests/script_context_borrow_matrix.rs` — 4 trybuild cases
- val high (borrow rules = api contract). kp, but same slow-job gate opt if inner loop matters

### 3.3 bench-style tests -> benches/

8 `#[test]` fns = timing probes, already `#[ignore]`:

- `ik_target_2d/3d`, `physics_bone_chain_2d/3d` (perro_internal_updates)
- `perro_ids/src/ids.rs:420`
- `perro_static_pipeline/src/lib.rs:264`
- `perro_graphics_assets/src/texture.rs:499`

workspace already own 34 bench fles. mv probes -> `benches/`, rm frm test count. res: 1 home 4 perf num, ignore-list stay clean.

### 3.4 fine as-is

- networking: 14 ignored socket tests behind `network-tests` feature — good pattern
- fixed_step tests deterministic, no wall-clock assert — good
- graphics 10k-loop plan tests: cpu-side, fast — kp

---

## 4. Regressions

- none fnd: clippy 0 warn, all tests green
- risk source = commit shape, !code:
  - `40d013ce "variant super"` mix variant + winit ctrl-c + audio solve + physics in 1 commit
  - grab-bag commit break bisect + revert
  - rule: 1 concern / commit. msg = `area: what`
- git obj dir hold tmp garbage (interrupted op) -> run `git gc`

---

## 5. Build Hygiene

| iss | fx | res |
| --- | --- | --- |
| ~~`lto = "none"`~~ DONE | chg -> `lto = "off"` | ext tools (cargo-machete, cargo_toml crate) parse again; no build chg (none==off) |
| ~~`perro_website` on default build path~~ DONE | add `default-members` = all !website (user kp website in ws 4 docs/book/demos) | bare `cargo test/check/build` skip leptos+axum+tokio; website still `-p perro_website` + `--workspace` |
| no unused-dep gate | add cargo-machete CI step (aft lto fx) | dead deps caught auto — OPEN |

note: user want website in workspace (docs+book+demos). NOT mv out. default-members exclusion = keep usable + cut inner loop.

---

## 6. Roadmap: best engine path

### 6.1 close known feature gaps (frm feature matrix)

| gap | status | next act |
| --- | --- | --- |
| mesh blend | in dev | fx known rndr iss in screen-space id-mask + seam pass |
| 3d shadows | partial | add control knobs + docs |
| steamworks | partial | fill missing api surface, doc matrix |
| demo hubs | done | build `demos/Demo2D` + `demos/Demo3D` — biggest adoption lever |
| retarget / decals / navmesh | research | navmesh 1st: most game-blocking of 3 |

### 6.2 perf backlog (open, frm past audits)

- structural-only node counter (unlock 1.1 fx + cheap gates)
- prepass depth unify
- transform pck 4 render extract
- Variant stride 96 -> 80B
- query candidate-restricted spatial fill
- mutation-ver split (blocked on physics WIP — unblock 1st)
- SoA bench suite 4 regression guard

### 6.3 robustness

- fuzz bin decoders (pskel/panim/ptileset/pck) — cargo-fuzz, corrupt-fle corpus
- clamp all len/count fields frm fle bytes b4 alloc
- audit `RwLock` unwrap poison policy in `perro_io` — panic-in-panic risk on asset thread

### 6.4 dev velocity

- fx 3.1 (slow tests gate) — biggest single win, 4min -> 10s
- opt: cargo-nextest 4 parallel test bins + per-test timing 4 free
- CI tiers: fast (default tests) / slow (--ignored + trybuild) / bench (nightly, trend graph)

---

## priority order

1. ~~gate compiler slow tests (3.1)~~ DONE `8c0081d8` — inner loop 250s->8s
2. ~~structural node counter + audio flag fx (1.1)~~ DONE `fe294c05`
3. ~~pskel alloc clamp (1.2)~~ DONE — clamp + tests, sibling decoders audited; fuzz corpus (6.3) still open
4. ~~dead code rm pass (2)~~ DONE — 3 true-dead rm (perro_ids dep, rel_to_path, is_animation_tree_id_pending); 4 kept (audit stale/cfg/test). clippy all-targets 0 warn
5. ~~lto spell + website default-members (5)~~ DONE — website KP in ws (user), just off default build set. OPEN: cargo-machete CI gate
6. demo hubs (6.1) — adoption  <- NEXT
7. perf backlog grind (6.2)
