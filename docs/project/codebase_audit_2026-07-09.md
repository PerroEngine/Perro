# Codebase Audit - 2026-07-10

Full repo bug + perf + API ergonomics + doc parity audit.

Supersede old audit docs.

## 2026-07-10 Snap

- base: `64907b93`
- branch: `main`
- start tree: clean; `main` 23 commits ahead of `origin/main`
- scope: 1,061 tracked files
- Rust: 687 files / 289,845 lines
- Markdown: 139 files / 24,297 lines
- crates: 39 engine/tool crates + website + demos + editor
- method: 3 parallel domain audits + root API/docs/CI pass
- checks: source proof, focused repro design, scoped tests, fmt, clippy, generated-doc inspection
- limit: no GPU capture, audio RT trace, fuzz, Miri, cross-OS run, or full perf rebench
- note: line refs use audit base; later edits may shift them

## Gate State At Audit Freeze

- fail: `cargo fmt --all -- --check`
- res: 15 stale format diffs from pre-audit local commits
- fail: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- res: `perro_runtime/src/runtime/render/three_d.rs:225` needless `.iter().copied()`
- pass: core + script scoped tests
- pass: runtime scoped tests; one combined command hit 64s timeout w/o fail output
- pass: render/io/audio/build scoped tests
- res: 313 pass + 2 ignored
- pass: `cargo test -p perro_networking --features network-tests`
- res: 26 pass
- pass: `cargo test -p perro_website --lib`
- res: 9 pass; tests miss internal link graph + heading IDs
- pass: `cargo run -p perro_cli -- --help`
- pending: full workspace test aft fix merge

## Sev

- P0: path escape, UB, hang, stack overflow, live-data corruption, or wrong shipped binary
- P1: panic, data loss, stale state, unbound resource growth, or major wrong output
- P2: bounded wrong output, perf/power fault, API footgun, or doc break
- P3: low-risk drift, maint debt, or design gap

## Critical Queue

| ID | Area | Proof | Impact | Fix |
| --- | --- | --- | --- | --- |
| PERF-01 | graphics resources | stale generation fails arena remove; cleanup still clears slot-backed live meta/data | reused live resource corrupt | return on failed `remove_parts` + reuse tests |
| PERF-02 | postprocess | cache key uses stack wrapper address; address reuse hits old bind group | wrong camera input/depth | stable texture generation key or no external-view cache |
| PERF-03 | web export | route/icon/scene paths accept `..`; raw join escapes roots | read/write outside project/output | strict path grammar + canonical containment |
| RT-017 | node tree | reparent accepts self/descendant; DFS lacks seen set | graph cycle + infinite walk | reject cycle + defensive visited set |
| RT-018 | scripts | `on_removal` runs while script stays registered | self-remove recursion + stack overflow | detach/mark removing before callback |
| DOC-01 | website | link rewriter splits after `](` but slices `tail[2..]` | every rendered link/image loses 2 chars | correct slice + full route graph test |
| DOC-02 | website | TOC emits `#id`; rendered headings have no IDs; dup IDs collide | all TOC/anchor nav fail | inject unique heading IDs + tests |

## Core + Script Ledger

| ID | Sev | Location | Issue -> fix |
| --- | --- | --- | --- |
| COR-01 | P1 | `perro_variant/src/variant.rs:985-1003` | typed IDs encode U64 but decode via I64 -> use U64 fallback + max-ID roundtrip |
| COR-02 | P1 | `perro_structs/.../transform_2d.rs:43-60` | matrix decompose loses reflection + zero scale makes NaN -> signed determinant + degenerate guard |
| COR-03 | P1 | `perro_structs/.../matrix/mod.rs:1725-1740` | div via reciprocal breaks subnormals -> direct scalar/SIMD division |
| COR-04 | P1 | `perro_nodes/.../animation_player.rs:53-57,120-149` | public bindings bypass revision -> guarded mutation/fingerprint |
| COR-05 | P1 | `perro_csv/src/lib.rs:1058-1126` | static CSV leaks rows before late parse error -> parse owned, leak after success |
| COR-06 | P1 | `perro_particle_math/src/lib.rs:247-251` | reversed clamp bounds panic -> validate/order bounds |
| COR-07 | P1 | `perro_structs/.../color.rs:208-245` | UTF-8 byte-boundary hex slice panic -> byte parser |
| COR-08 | P2 | `perro_structs/.../matrix/mod.rs:1085-1177` | absolute epsilon rejects small invertible matrices -> scale-relative pivot |
| COR-09 | P2 | matrix `mod.rs:85-122,1743-1749,2332-2346`; `x86.rs:124-165` | SIMD wrap + scalar checked tail diverge -> one overflow policy |
| COR-10 | P2 | `perro_animation/src/anim_tree.rs:128-310` | cycles, dup slots, blend arity pass parse -> DAG + uniqueness + arity validation |
| COR-11 | P2 | `perro_ui/src/layout.rs:83-91,163-193` | translation units differ by helper -> shared offset contract |
| COR-12 | P2 | `perro_ids/src/ids.rs:53-57,354-373,449-468` | `borrowed("123") != new("123")` -> shared decimal grammar |
| COR-13 | P2 | `perro_variant/src/variant.rs:2574-2590` | huge finite duration panics -> range check |
| COR-14 | P2 | `perro_structs/src/structs/snorm.rs:4-6,63-68` | derived default decodes to -1, not ZERO -> manual default |
| COR-15 | P2 | `perro_structs/.../quaternion.rs:290-317` | local-axis docs vs world-axis multiply -> swap order or rename/doc |
| COR-16 | P2 | `perro_csv/src/lib.rs:835-848` | NaN/invalid numeric sort comparator not total -> invalid bucket + `total_cmp` |
| COR-17 | P2 | `perro_ids/src/ids.rs:96-123,202-213` | Display output fails FromStr -> roundtrip grammar |
| COR-18 | P2 | `perro_particle_math/src/lib.rs:388-568` | recursive parser has no depth cap -> budget/iterative parser |
| COR-19 | P2 | `perro_particle_math/src/lib.rs:450-471` | `^` associativity/precedence surprise -> right-assoc power above unary + docs |
| COR-20 | P2 | `perro_nodes/.../camera_3d.rs:74-168` | NaN/180 FOV + huge near make invalid projection -> strict finite setters |
| COR-21 | P2 | `perro_nodes/src/nodes/node_registry.rs:520-985` | spatial flags disagree with base dispatch for ambient/sky nodes -> align model |
| COR-22 | P3 | `perro_variant/src/variant.rs:1544-1580` | nested Option collapses states -> tagged form or explicit unsupported doc |
| COR-23 | P3 | `perro_variant/src/variant.rs:2602-2620` | pre-epoch SystemTime becomes Null -> signed/tagged time |
| COR-24 | P3 | `perro_builtin_meshes/src/lib.rs:208-239,348-421` | sphere/capsule pole tris degenerate -> pole fan topology |

## Runtime + Project Ledger

| ID | Sev | Location | Issue -> fix |
| --- | --- | --- | --- |
| RT-001 | P0 | `perro_scene/src/parser.rs:408-447`; `lexer.rs:114-134` | EOF in type block loops forever -> error on EOF/Error |
| RT-002 | P0 | `perro_project/src/templates.rs:37-59` | stale write lock retries forever -> stale owner reclaim or timeout |
| RT-003 | P1 | `perro_project/src/config_parse.rs:564-732` | finite f64 casts to f32 infinity -> post-cast finite/domain checks |
| RT-004 | P0 | `config_parse.rs:991-999`; `templates.rs:541-545` | `res://../../` path escape -> component validation + containment |
| RT-005 | P1 | `perro_runtime_render/src/retained.rs:29-58`; runtime UI extract | UI texture tag `0xE9` never dirties UI -> decode/map tag |
| RT-006 | P1 | `perro_runtime_render/src/retained.rs:13-30` | packed render request IDs collide/truncate generation -> opaque IDs/side map |
| RT-007 | P1 | runtime physics API + `perro_physics/src/system/step.rs:136-281` | NaN/inf force poisons Rapier state -> reject at both boundaries |
| RT-008 | P2 | `animation_player.rs:55-60,170-201` | event-only clip exits before event dispatch -> skip pose only |
| RT-009 | P1 | `animation_player.rs:117-166,200,278-328` | large step drops crossed events -> enumerate crossings |
| RT-010 | P2 | runtime animation API + player `39-151` | pending clip inherits old playhead -> active/desired split + reset |
| RT-011 | P2 | player bindings/revision `140-160` | direct bind edit stays stale -> encapsulate/fingerprint |
| RT-012 | P1 | `animation_player.rs:1388-1417` | large/nonfinite boomerang returns invalid frame -> finite triangular wave |
| RT-013 | P1 | `animation_tree.rs:64-95,143-160,445-471` | paused/current-frame event repeats every update -> event cursor |
| RT-014 | P2 | `animation_tree.rs:151-158,282-290,463-470` | fallback pose/event use different clips -> one active clip resolution |
| RT-015 | P1 | `animation_tree.rs:331-376,516-577` | Transform2D and 3D rot/scale blend missing -> type-aware blend/mask |
| RT-016 | P0 | IK/physics bone solvers + scene loaders | raw u32 iteration count can freeze frame -> strict cap at load + runtime |
| RT-017 | P0 | `rt_ctx/nodes.rs:937-1000`; render/query DFS | self/ancestor reparent creates cycles -> reject + seen set |
| RT-018 | P0 | `rt_ctx/scripts.rs:240-285`; node removal | removal callback can recursively remove self -> detach/once guard |
| RT-019 | P1 | `runtime/scheduling.rs:44-64` | callback-added start work overwritten by scratch restore -> preserve live queue |
| RT-020 | P1 | `rt_ctx/signals.rs:203-222` | nested queued UI signal overwritten -> preserve/append live queue |
| RT-021 | P1 | `cns/node_arena.rs:561-575` | clear resets generations and revives stale IDs -> bump/retain epoch |
| RT-022 | P1 | `scene_loader/merge.rs:86-361,471+` | merge mutates live state before late error -> prevalidate/rollback |
| RT-023 | P1 | `scene_loader/mod.rs:158-171` | failed route load removes current route first -> prepare then atomic swap |
| RT-024 | P1 | merge `86-92,471-552`; unload `166-169` | hidden container/sibling roots leak -> track scene ownership root |
| RT-025 | P1 | merge `344-361,488-509` | declared child root leaves stale parent edge -> reject or proper reparent |
| RT-026 | P1 | scene parser `729-743`; merge `344-361` | parent cycles pass validation -> DAG check before merge |
| RT-027 | P2 | `scene_loader/prepare/core.rs:1159-1203` | unknown node refs silently drop -> typed error |
| RT-028 | P1 | `runtime/internal_updates.rs:65-174,363-380` | callback removal shrinks indexed schedule -> snapshot IDs/defer mutation |
| RT-029 | P2 | particle emitter 2D/3D `74-80` | infinite spawn rate overflows `+2` -> finite cap + saturating math |
| RT-030 | P2 | animated sprite/UI image `48-57` | huge step overflows frame add -> modular u64 math |
| RT-031 | P0 | project audio cfg `534-542`; solve `1266-1275` | max bounce count + reflection 1 stalls tick -> cap/analytic sum |

## Render + IO + Audio + Build Ledger

| ID | Sev | Location | Issue -> fix |
| --- | --- | --- | --- |
| PERF-01 | P0 | `perro_graphics/src/resources.rs:285-315,1247-1328` | stale drop clears reused live slot -> return on generation miss |
| PERF-02 | P0 | postprocess `165-174,1242-1345`; GPU camera calls | wrapper-address cache alias -> stable texture generation key |
| PERF-03 | P0 | compiler `project_bundle.rs:1415-1458,1743-1746`; project routes | web path traversal -> strict grammar + source/dest containment |
| PERF-04 | P1 | resources GC `348-387,1057-1238` | duplicate GC candidates age many times/sweep -> dedup queue/set |
| PERF-05 | P1 | `perro_graphics/src/water_gpu.rs:755-844,965-1229` | async bytes decode with next-frame metadata; growth drops pending map -> snapshot pending state/buffer gen |
| PERF-06 | P1 | 3D buffers `1678-1909`; prepare `77-78` | custom mesh arenas append forever -> free spans/compact live ranges |
| PERF-07 | P1 | resources `105-130,673-686` | decoded texture deep-copied by ID + source -> shared Arc/canonical store |
| PERF-08 | P1 | graphics backend `585-632,918-941` | async texture decode error emits no terminal event -> carry Result/waiters |
| PERF-09 | P1 | resources slot writes `60-84,452+` | explicit huge ID grows Vec to multi-GiB -> max index guard/fallible ID |
| PERF-10 | P1 | app frame pacing + winit runner | minimized/occluded app keeps Poll + full sim -> occlusion wait/low-rate |
| PERF-11 | P1 | app JoyCon `288-456,790-817` | backend drop leaves worker threads alive -> stop + join in Drop |
| PERF-12 | P1 | graphics-assets texture `13-15,88-219` | 32-entry SVG cache can hold ~8 GiB + clones under lock -> Arc + byte LRU |
| PERF-13 | P1 | graphics-assets texture `58-63`; app image helpers | `max_size` ignored for raster/PTEX -> downscale all formats |
| PERF-14 | P1 | PTEX/PMESH decode + IO compression | tiny declared raw size may inflate up to 1 GiB -> cap to expected length |
| PERF-15 | P1 | assets packer + static pipeline encoders | whole asset output set retained before write -> bounded ordered pipeline/spool |
| PERF-16 | P0 | compiler `project_bundle.rs:353-390` | Android picks newest APK from shared target -> cargo artifact JSON/exact path |
| PERF-17 | P1 | IO asset root + assets archive | invalid archive panics after root swap; stale/double archive memory -> fallible atomic swap |
| PERF-18 | P1 | pawdio controller `64-81,228-237` | constructor succeeds when player init fails in worker -> startup result handshake |
| PERF-19 | P1 | pawdio mic `620-831` | RT callback locks, allocs, drains/memmoves 30s buffer -> prealloc SPSC ring |
| PERF-20 | P1 | pawdio MIDI/player | unbound commands + HashMap/Vec mutation in audio callback -> bounded queue + fixed voice pool |
| PERF-21 | P2 | app frame pacing `19-22`; runner | 2ms busy-poll tail burns up to 12% core @60Hz -> adaptive/opt low-latency tail |
| PERF-22 | P2 | graphics GPU timestamps | native profiling always maps/polls/allocs channel -> opt-in + persistent ring |
| PERF-23 | P2 | graphics-assets size query + splash | size query full-decodes; splash decodes twice -> header dims + cache |
| PERF-24 | P2 | postprocess time path + docs | `time` counts chain calls, not seconds/frame -> pass frame time |
| PERF-25 | P2 | postprocess shader/LUT caches | dynamic keys never evict -> bounded LRU/TTL |
| PERF-26 | P2 | pawdio DSP `140-181,321-339` | dry source allocs 3 delay lines -> lazy wet init/pool |
| PERF-27 | P2 | pawdio controller/player | loaded state stale after auto-evict; source interner unbound -> eviction sync + cap |
| PERF-28 | P2 | pawdio mic `529-564` | recorder spawn error swallowed -> fallible ctor |
| PERF-29 | P2 | assets common/archive | uncompressed archive version ignored -> reject unknown version/flags |
| PERF-30 | P2 | IO asset archive `421-482` | global RwLock held through inflate -> clone Arc then unlock |
| PERF-31 | P2 | compiler build opts `1-48` | SDK/NDK paths require leaked `'static` str -> owned PathBuf/OsString |
| PERF-32 | P2 | `perro_meshlets/src/lib.rs:137,160-198` | public bad range/size can panic/overflow -> checked Result |
| PERF-33 | P2 | water GPU + shader | max-size draw for all mixed-LOD chunks -> size buckets/indirect draw |

## API + Docs + CI Ledger

| ID | Sev | Location | Issue -> fix |
| --- | --- | --- | --- |
| DOC-01 | P0 | `perro_website/build.rs:286-301` | all Markdown targets lose first 2 chars -> correct tail indexing + route graph test |
| DOC-02 | P1 | `perro_website/src/highlight.rs`; pages/docs TOC | headings render no IDs; duplicates collide -> unique injected IDs |
| DOC-03 | P2 | `docs/index.md` | 8 anchor refs point to absent headers in HTTP/shaders/query/state -> use real headers |
| DOC-04 | P2 | website docs tests | 9 tests pass while all internal links break -> validate every internal route + fragment |
| DOC-05 | P3 | `agents.md`, `CLAUDE.md` | main doc map names missing `docs/perro_cli.md`; real path `docs/tools/perro_cli.md` -> chg path |
| DOC-06 | P2 | `perro_website/build.rs`; `demos/Demo3D/docs` | website collects demo README only; its 12 detail links point to absent routes -> collect full demo doc set |
| API-01 | P1 | networking HTTP `http.rs:316-469` | unbounded work/event channels + one serial worker -> bounded queue/pool + backpressure API |
| API-02 | P2 | CLI `main.rs:152-166` + all commands | missing flag value consumes next flag; unknown flags ignored -> schema parser/validation |
| API-03 | P3 | input `keycode.rs:205-229` | `from_name` formats up to 194 strings/call -> static name map/macro table |
| API-04 | P3 | public APIs | broad `Result<_, String>` + bool mutations hide structured cause -> typed errors/additive checked calls |
| API-05 | P3 | crate Cargo manifests | only 3/39 inherit rust-version + description; CI tests stable only -> workspace metadata + MSRV job |
| GATE-01 | P1 | changed Rust files | fmt check fails -> run workspace fmt |
| GATE-02 | P1 | runtime 3D extract | clippy all-feature fail -> remove needless copy |

## Gap Review Ledger

| ID | Sev | Location | Issue -> fix |
| --- | --- | --- | --- |
| GAP-01 | P0 | `perro_assets/walkdir.rs`; `perro_io/walkdir.rs` | recursive walkers follow link/reparse loops + escape roots -> iterative contained walk + reject links |
| GAP-02 | P0 | runtime scene loader DLC cache writes | preseeded cache link/reparse redirects writes outside cache root -> reject linked components + root containment |
| GAP-03 | P0 | `perro_scene/src/parser.rs` value + var parsing | nested arrays/objects/vars overflow call stack -> strict depth budget + iterative depth validation |
| GAP-04 | P1 | `perro_steamworks/events.rs`; runtime pump | callback event queue grows w/o bound -> bounded queue + coalesce/drop policy + counter |
| GAP-05 | P1 | runtime UI dropdown child sync | option shrink drops tracked IDs but leaves nodes/schedules/names -> reuse high-water children or fully remove |

## Open Design Risk

### P1 - dynamic script ABI

- Rust trait-object pointer still crosses `extern "C"`
- exact-build fingerprint cuts mismatch risk; !stable ABI
- req opaque handle + repr(C) fn table + C-safe values + panic containment

## Fix Order

1. close path escape + hang/stack-overflow/live-corruption P0s
2. close build gates
3. close high-confidence small panics + stale-state bugs
4. close website link + heading graph; add regression tests
5. close async GPU request/readback identity bugs
6. close RT audio unbound/alloc paths w/ trace + p99 bench
7. close large memory/cache/build pipeline items w/ RSS benches
8. evolve typed API errors + CLI schema additively
9. design stable script ABI before impl

## Fix Pass Selection

- lane A: core correctness quick/high items
- lane B: runtime graph/schedule/parse items
- lane C: render resource/postprocess/path items
- root: docs link/heading tests + fmt/clippy + merge gates

## 2026-07-10 Applied Pass State

Closed in integration:

- P0: `RT-001`, `RT-002`, `RT-004`, `RT-016`, `RT-017`, `RT-018`, `RT-031`
- P0: `PERF-01`, `PERF-02`, `PERF-03`, `PERF-16`, `DOC-01`
- P0 gap: `GAP-01`, `GAP-02`, `GAP-03`
- P1: `COR-01`, `COR-02`, `COR-03`, `COR-04`, `COR-05`, `COR-06`, `COR-07`
- P1: `RT-003`, `RT-005`, `RT-006`, `RT-007`, `RT-009`, `RT-012`, `RT-013`, `RT-015`
- P1: `RT-019` thru `RT-026`, `RT-028`
- P1: `PERF-04`, `PERF-05`, `PERF-07`, `PERF-08`, `PERF-09`, `PERF-12`, `PERF-13`, `PERF-14`
- P1: `PERF-17`, `PERF-18`
- P1: `DOC-02`, `API-01`, `GATE-02`, `GAP-04`, `GAP-05`
- P2: `COR-13`, `COR-14`, `COR-16`, `PERF-29`, `PERF-32`, `DOC-03`, `DOC-04`, `DOC-06`, `API-02`
- P3: `DOC-05`, `API-03`
- opt: prior `O3` CSV top-k
- review: removal reattach race + heading natural-suffix collision

Main fix commits:

- `7eeed5dd`: core correctness + CSV top-k
- `00948c86`: runtime cycle/callback/parser/schedule fixes
- `43c1512b`: website route/link/heading/doc graph fixes
- `e7490a68`: bounded project write lock + icon containment
- `1ca938e9`: skeletal/audio work caps
- `4f5f6401`: contained asset walk + scene value depth cap
- `b0bd742d`: render/resource/build path + artifact fixes
- `2c78fdda`: DLC cache containment + review regressions
- `0d7c7b97`: dropdown high-water child reuse
- `a414d1b5`, `99a3ed9b`: bounded Steam + HTTP queues
- `a2ce60d1`: cfg/physics/arena boundary guards
- `23ca0a6d`: full-width render request identity
- `1596b589`: canonical texture data + async failure fanout + ID cap
- `a2fe0162`: atomic scene merge/routes + ownership roots
- `771d51d7`, `2456f9d9`: animation events/schedules/fingerprint/blend
- `2fa21744`, `19497763`: atomic asset root + audio startup handshake
- `b555adea`, `793f7e8b`: keycode table + CLI flag schema
- `e68ca563`: asset cache/resize/inflate bounds

Profile-gated defer:

- `PERF-06`: custom mesh GPU storage spans 5 coupled append lanes
- req allocation records + coalescing free lists + full offset rebuild on compaction
- req randomized churn validator + render parity + VRAM/fragmentation bench b4 land
- `PERF-11`: JoyCon BLE connect/discover/write awaits lack timeout
- req cancellable/time-bound BLE ops b4 worker `Drop` join; direct join can deadlock

Gate state:

- scoped lane tests + clippy pass
- website internal route + anchor graph: 13 pass
- full workspace fmt/clippy/test pending final merge
- all ledger IDs not listed above stay open

## Prior 2026-07-09 Audit

## Snap

- base: `c1b0e80e`
- branch: `main`
- scope: full Rust ws + old audit claims
- dirty tree: 10 user src/demo fles b4 audit
- act: kp all user chg
- method: source scan + old claim recheck + compile/clippy/tests
- limit: no GPU trace, fuzz run, cross-OS run, or full perf rebench

## Gates

- pass: `cargo fmt --all -- --check`
- pass: `cargo check --workspace --all-targets`
- pass: `cargo clippy --workspace --all-targets -- -D warnings`
- pass: `cargo test -p perro_networking --features network-tests`
- res: 26 net tests pass
- pass: changed-crate lib tests
- res: 757 asset/compiler/IO/modules/res/runtime/static tests pass
- pass: ignored `generated_dlc_registry_pack_crate_compiles`
- res: generated DLC pack build + registry source compile
- note: GPU opts below still need trace/bench proof

## Fix Pass

Closed:

- TCP partial tx, multi-frame rx, EOF, O(n) front drain, + 16 MiB tx backpressure
- NavMesh invariant boundary + safe store/query guards
- NavMesh O(T) edge build + heap A* + graph cache by ID/layer mask
- ZIP link/reparse guards + entry/byte/total/ratio caps + partial-file cleanup
- HTTP TLS provider mapping + empty-URL single terminal evt
- `NodeArena` public tracked RAII mut guard + raw public bypass rm
- DLC registry v1 inventory/API + stable sort/collision guard + navmesh kind 17
- `ResPath` DLC dot-name grammar parity

Open:

- dynamic script C ABI
- generic DLC FILE inventory
- renderer/CSV opts O2-O5

ABI stop reason:

- opaque producer state conflict w/ host `with_state<T>` in lifecycle ctx
- partial bridge break dynamic scripts or keep hidden Rust ABI
- req: C-safe host ctx/state API + producer-side state route
- act: rm draft; kp exact-build descriptor/fingerprint gate

## Sev

- P0: path escape/destructive risk
- P1: data loss, panic, UB, dead conn, or wrong core res
- P2: silent cfg/API fault, resource abuse, or stale state
- P3: low-risk drift/maint debt

## Bugs

### P1 - TCP frame path lose/corrupt data - CLOSED

Proof:

- `perro_source/api_modules/perro_networking/src/tcp.rs:116-124` set stream nonblock
- `tcp.rs:165-173` call `Write::write_all` on same nonblock stream
- partial write + `WouldBlock` -> API ret err w/o unsent offset
- caller retry whole frame -> dup prefix/payload risk
- `tcp.rs:206-225` cap whole recv buf to `max_frame_bytes + 4`
- 2 valid queued frames whose sum > cap -> false `FrameTooLarge`
- `tcp.rs:209` EOF -> `Ok(())`; framed poll never emit disconnect
- `world.rs:353-378` framed world keep dead conn on `Ok(None)`
- `tcp.rs:301` front `drain` shift all queued bytes/frame

Fx:

1. add tx queue + byte cursor
2. flush until `WouldBlock`; keep unsent tail
3. dcod complete buffered frm b4 socket rd
4. cap declared frame len; add separate total queue cap
5. track EOF; emit `TcpDisconnected`
6. use read cursor/`BytesMut`/`VecDeque`; rm front memmove

Tests:

- force tiny socket send buf + multi-MiB frame
- push 2 max-valid frames in 1 write
- close peer w/ partial + empty recv buf
- decode 10k queued tiny frames

### P1 - NavMesh safe API allow panic data - CLOSED

Proof:

- `perro_source/api_modules/perro_resource_api/src/sub_apis/navmesh.rs:8-17` expose verts + tris
- `navmesh.rs:107-112` accept any `NavMesh3D` in create/write trait
- `perro_runtime/src/runtime/navmesh.rs:89-91` index verts w/o bounds chk
- `runtime/navmesh.rs:247-251` repeat unchecked tri index use
- safe script call can create tri `[u32::MAX, 0, 1]` -> path query panic
- `navmesh.rs:93-96` accept `NaN`/`Inf`
- nonfinite verts -> NaN snap/path dist + unstable order

Fx:

1. add `NavMesh3D::try_new` + `validate`
2. reject out-of-range/dup tri idx, nonfinite verts, degenerate XZ tris, empty layers
3. validate create/write + parsed data at cache boundary
4. keep query bounds-safe as defense
5. make raw fields private or mark unchecked ctor unsafe

Tests:

- direct invalid create/write -> err
- NaN/Inf text -> err
- bad cache data -> failed path, no panic

### P1 - ZIP output can hit symlink target + no expansion cap - CLOSED

Proof:

- `perro_source/io_stack/perro_io/src/zip.rs:87-96` canonicalize parent only
- `zip.rs:83` `File::create(target)` follow final pre-made symlink
- existing `out/file` symlink -> write outside `out`
- `zip.rs:54-60` entry rd use unbound `read_to_end`
- `zip.rs:63-86` extract use unbound `io::copy`
- no entry count, per-entry size, total size, or ratio cap

Fx:

1. reject any symlink/reparse comp incl final target
2. open new output w/ no-follow + create-new semantics where OS allow
3. add opts: max entries, max entry bytes, max total bytes, max ratio
4. bound in-mem entry rd too
5. rm partial output on limit/error

Tests:

- pre-made final symlink/reparse target
- nested dir symlink
- high-ratio zip bomb
- many tiny entries + total cap

### P1 - dynamic script ABI still pass Rust trait obj thru C ABI - OPEN

Proof:

- `perro_source/script_stack/perro_scripting/src/script_trait.rs:72-74` use `extern "C" fn() -> *mut dyn ScriptBehavior`
- `perro_runtime/src/cns/scripts.rs:222-299` load + cal ctor ptr frm dylib
- v2 magic/version/build fingerprint reduce mismatch risk
- trait fat ptr, vtable, Rust values, panic/unwind stay !stable C ABI

Risk:

- exact-build gate help
- compiler/layout/unwind drift still UB class

Fx:

1. use opaque handle
2. add `#[repr(C)]` fn tbl: create/drop/lifecycle/state ops
3. use C-safe byte/value args only
4. wrap every boundary in panic containment
5. keep version/size/fingerprint gate

### P2 - HTTP TLS modes map to same native provider - CLOSED

Proof:

- `perro_source/api_modules/perro_networking/Cargo.toml:18` enable `native-tls`; !enable rustls
- `perro_networking/src/http.rs:503-513` all 3 modes set `TlsProvider::NativeTls`
- `DefaultRustls` + `PlatformVerifier` behave same
- API cfg silently lie

Fx:

1. enable ureq rustls feat
2. map `DefaultRustls` -> `TlsProvider::Rustls`
3. define cert-root diff 4 `PlatformVerifier`
4. cfg-gate unsupported providers or ret cfg err

Tests:

- expose/test built provider + root mode
- HTTPS smoke per TLS mode in CI

### P2 - empty HTTP req emit 2 fail events - CLOSED

Proof:

- `perro_networking/src/http.rs:339-359` send work b4 empty URL chk
- local queue add fail aft send ok
- worker also run empty URL req -> 2nd fail same `HttpID`

Fx:

1. validate b4 `tx.send`
2. emit 1 local fail only
3. add 1 req -> exactly 1 terminal evt invariant test

### P2 - `NodeArena::get_mut` still bypass index repair - CLOSED

Prior audit state: partial.

Proof:

- `perro_runtime/src/cns/node_arena.rs:308-314` return raw `&mut SceneNode`
- docs warn caller !chg name/tags/parent
- safe fn cannot enforce warning
- `edit` @ `node_arena.rs:321-336` repair path exists but raw path remain public

Fx:

1. make raw mutable lookup crate-private/unsafe
2. use tracked guard for public mutable access
3. split indexed fields frm free node payload

### P2 - DLC generic registry stay empty - CLOSED

Prior audit state: open + ABI design doc land.

Proof:

- `perro_source/build_pipeline/perro_compiler/src/dlc.rs:223-238` emit len `0` + get `false`
- typed hash lookup work only for subset
- discovery API report no assets in nonempty pack
- `docs/project/dlc_registry_abi_v1.md` define target ABI; impl absent

Fx:

1. emit canonical `(kind, uri, hash, payload mode)` inventory in static pipeline
2. implement v1 registry API tbl
3. reject same-kind hash collision by full URI
4. add build -> load -> enum -> find E2E test

### P3 - `ResPath` accept invalid DLC dot names - CLOSED

Proof:

- `perro_resource_api/src/res_path.rs:483-494` allow `.` + `..` as DLC name
- `perro_io/src/asset_io.rs:321-330` reject same names at resolve boundary
- `ResPath::try_new("dlc://../file")` succeed but IO reject

Fx:

1. share 1 DLC name validator/type
2. reject dot names in const + runtime paths
3. add cross-crate grammar parity tests

## Opts

### O1 - prebuild nav graph + use heap A* - CLOSED

Proof:

- `perro_runtime/src/runtime/navmesh.rs:167-185` pair every tri per query -> O(T^2)
- `navmesh.rs:198-230` open list use linear min + `contains`
- `navmesh.rs:232-252` recalc centroids thru search

Chg:

- build shared-edge map + adjacency + centroids once on load/write
- cache by `NavMeshID` rev
- use binary heap + closed bitset
- target: load O(T), query O((V+E) log V)

### O2 - cache camera stream extract

Proof:

- `perro_runtime/src/runtime/render/bridge.rs:623-625` rebuild full node ID list
- `bridge.rs:697-1819` each 2D/3D collector rescan list
- each stream extract mk fresh Vec/Arc slices
- static stream still pay scan + alloc

Chg:

- cache by stream/cam IDs + render mask + arena/render/resource revs
- share main dense pose Arc
- split cam uniform-only upd frm scene payload rebuild
- add hit/miss + alloc counters

### O3 - top-k CSV sort

Proof:

- `perro_source/core/perro_csv/src/lib.rs:729-736` full sort b4 `limit`
- old bench show sort+limit hot; source path stay same

Chg:

- `limit << matches` -> heap/top-k or partial select + sort prefix
- full sort only w/o limit
- bench match counts + limit ratios

### O4 - cut global shadow invalidation

Proof:

- `perro_graphics/src/three_d/gpu/prepare.rs:550,1776` set 1 global caster dirty bit
- `gpu/shadows.rs:60-83` global bit invalidate every shadow layer
- 1 moving caster -> all ray/spot/point maps redraw

Chg:

- track dirty caster bounds + render layers
- map dirty caster -> hit light/frustum layers
- keep global fallback 4 topology/material chg

### O5 - rm duplicate multimesh cull work

Proof:

- `perro_graphics/src/three_d/gpu/render_pass.rs:221-240` frustum cull pass
- `render_pass.rs:457-474` 2nd Hi-Z cull + counter clear
- same frm can dispatch + clear/finalize 2x

Chg:

- measure prepass need vs Hi-Z main-pass need
- reuse/refine 1 visible set when valid
- expose cull dispatch + visible count + GPU time

### O6 - keep lower-prio opts profile-gated

- CSV/source IDs + WGSL prelude dedup: land
- 2D point-light rev gate: land
- targeted material texture eviction: land
- renderer perf counters: land
- camera cache/shadow granularity/draw signature/dual cull: open
- trimesh Arc share: bench proof only; impl open
- schedule unregister: old 82-98ms microbench; rebench b4 chg
- dup dep fam: transitive-heavy; `cargo tree -d --workspace` still broad
- runtime/resource locks: batch only aft contention trace
- matrix unchecked neighbor path: old bench anomaly; rebench current code b4 edit

## Old Audit Recheck

Closed + !carry:

- DLC path escape + mount validator
- disk/archive/static remount stale backing
- lexer silent skip + recursion
- `Transform3D::looking_at`
- script write-lock stale hang
- tileset finite geom
- `MicClip` invariant/size checks
- runtime texture alloc cap
- static callback unsafe boundary + len cap
- CSV hash collision
- static pipeline override restore
- strict ID parse
- typed audio enqueue API
- parser typed/fallible path
- WGSL prelude triplication
- hot string-key maps + scratch reuse sweep
- test holes named in old spec audit
- `NodeArena` raw public mut path
- DLC registry false-empty stub
- NavMesh invariant + query graph cost
- TCP/HTTP/ZIP/ResPath items frm this pass

Carry:

- script stable ABI -> P1
- generic DLC FILE inventory + dev loader parity
- renderer open opts -> O2/O4/O5
- CSV limit sort -> O3

Do not carry:

- naming/coherence backlog -> API design, !bug/perf audit
- feature gaps -> `docs/project/feature_matrix.md`
- stale line nums/bench nums/commit tracker noise
- landed items

## Fix Order

1. design C-safe script ctx/state route
2. close script ABI on new route
3. expose archive emitted-member inventory -> add DLC FILE rows
4. add dev registry parity + bad-fingerprint loader E2E
5. profile O2-O5 b4 impl

## Done Rule

- add fail test b4 each bug fx
- run fmt + scoped test + clippy
- run ws check/test 4 cross-crate chg
- run Windows + Linux net/path CI
- run GPU trace b4/aft renderer opt
- upd this fle only; no new side audit
