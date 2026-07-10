# Codebase Audit - 2026-07-09

Sole bug + perf audit.

Supersede 8 old audit docs.

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
