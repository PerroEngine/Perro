# Repo performance fixes — baseline 3a4549f9

Track all 32 targets from [Sep 6 audit](performance_audit_2026-09-06.md).
Compare baseline `3a4549f9` with this working tree, using matching fixtures.
Keep [earlier draw-cache results](performance_fixes_2026-09-06.md) separate: that study uses a different baseline.

Record production changes, partial scope, regression coverage and reproducible probes below.
Treat `Implemented` as code scope, not a measured speedup.
Change 29 target areas, with partial scope below; defer R07, R08 and T08 production work.
Record 2,601 workspace test passes plus 57 editor test passes across verified snapshots.
Exclude v1 wall-time samples collected during competing compiler CPU load.
Reject all original `base-final-*` / `final-base-*-r1` and `r2` comparisons: a shared Cargo target reused candidate libraries in some baseline executables. Rebuild baseline workspace crates in a separate target; require original mesh lookup and texture-pool markers before accepting timing pairs.
Reject archive samples before `assets-refresh`: the candidate still linked an old baseline `perro_assets` library. Clean/rebuild that package, verify the library hash changes, and rerun archive timing and memory probes. Keep each accepted executable SHA and its preserved snapshot in the [portable result data](performance_audit_results_3a4549f9_2026-09-06.json).

## Track all 32 targets

### Render

| ID | State | Change | Remaining scope / tradeoff |
| --- | --- | --- | --- |
| R01 | Partial | Avoid GPU resource dirt for scene-reference metadata, reservations, saves and identical material writes. Preserve no-op resource events. | Actual material/resource changes still trigger broad staging; retain reference recounts. No material-slot patch implementation. |
| R02 | Partial | Reuse regular and dense opaque staging while camera movement stays inside current LOD bands. | Alpha ordering and LOD-band crossings retain full path. Add about 24 bytes per cached regular LOD draw. |
| R03 | Implemented | Mark shadow caster state dirty only when changed transform/dense-node/animation rows include casters. | Preserve pre-existing dirt and membership/resource fallbacks. No claim that every shadow dependency becomes sparse. |
| R04 | Partial | Patch equal-size changed UI primitive spans; merge adjacent runs. Limit patch path to at most eight runs and half the vertices. | Depth, topology, clip/texture changes, broad edits and logical viewport-scale changes retain full packing. Keep full text/layout behavior. |
| R05 | Implemented | Decode an uncached mesh once in native async and WASM/test load paths. Remove private validation forwarding. | Keep public asset API, builtin fallback and failure publication. No general asset cache redesign. |
| R06 | Partial | Route native texture jobs through one private four-worker Rayon pool; retain four-decode admission limit. Fall back to shared pool if thread creation fails. | Mesh jobs retain shared Rayon pool. Add four OS threads/stacks; possible CPU oversubscription. Completed-result queue remains unbounded. No frame-tail claim without a burst trace. |
| R07 | Deferred | Add uniform/mixed water-grid probes; retain current rectangular dispatch. | Need a measured GPU case before changing dispatch mapping and pause/ping-pong rules. Mixed grid launch-count model alone does not establish GPU time saved. |
| R08 | Deferred | Add GPU-attached sparse transform scale controls at 1k/10k draws. | All-draw classification, range and last-draw scans remain. Carrying exact dirty rows needs generation/compaction-safe metadata; headless CPU results do not validate this stage. |

Trace render changes in [backend](../../perro_source/render_stack/perro_graphics/src/backend.rs),
[asset loading](../../perro_source/render_stack/perro_graphics/src/backend/asset_load.rs),
[3D prepare](../../perro_source/render_stack/perro_graphics/src/three_d/gpu/prepare.rs),
and [UI GPU packing](../../perro_source/render_stack/perro_graphics/src/ui/gpu.rs).

### Runtime and scripts

| ID | State | Change | Remaining scope / tradeoff |
| --- | --- | --- | --- |
| T01 | Implemented | Cache navmesh validation; use triangle BVH endpoint search, epoch-stamped A* scratch and empty-obstacle-mask fast path. | Add cold BVH construction and retained search memory. Nonempty masks still cost triangles × obstacles; keep per-layer graph copies. |
| T02 | Partial | Forward independent-world epochs across internal active-world writes, guarded by ancestor and structural revisions. | External edits and nested worlds retain conservative collection. Full world/dimension revision redesign remains deferred. |
| T03 | Partial | Group animation-tree bone writes by skeleton; take one mutable borrow and issue one rerender per target. Preserve track order, masks and name/index selectors. | Persistent bone-name index deferred: public bone/name edits lack a stable revision contract. |
| T04 | Partial | Sort/index nonbone player bindings above eight entries; reuse thread-local order scratch and preserve duplicate matches. Keep tiny/bone-only paths. | Rebuild order each frame to observe direct public edits; O(B log B) setup remains. All-bone binding scans remain. |
| T05 | Implemented | Cache graph plans with 32 weak asset entries; index larger graphs, memoize shared acyclic nodes per evaluation and reuse blend scratch. | Preserve cycle fallback. Shared poses still clone; retain bounded scratch/cache memory. Weak keys avoid pinning assets or stale pointer reuse. |
| T06 | Partial | Cache conservative primitive rejection bounds in each collider snapshot for 2D/3D bone chains. Preserve exact collider/push order. | Keep O(points × colliders) candidate loops; singular/nonfinite/ill-conditioned transforms and polyhedra use original checks. Full spatial index needs ordered requery after pushes. Add bound construction and storage costs. |
| T07 | Partial | Fill only indexed spatial candidates; reset touched slots and grow only requested dimensions. | Broad first-hit query remains unchanged under current immutable-snapshot interface. Retain high-water scratch capacity. |
| T08 | Deferred | Add typed-root codec and multi-key patch probes plus direct typed-access controls. | Imported/dynamic typed access requires a derive/schema API. A direct map shortcut would break dotted-key, hash-first-match and structural patch behavior. Keep compiler fallback unchanged. |

Trace runtime changes in [navmesh](../../perro_source/runtime_project/perro_runtime/src/runtime/navmesh.rs),
[navmesh spatial index](../../perro_source/runtime_project/perro_runtime/src/runtime/navmesh/spatial.rs),
[physics worlds](../../perro_source/runtime_project/perro_runtime/src/runtime/physics.rs),
[spatial queries](../../perro_source/runtime_project/perro_runtime/src/rt_ctx/nodes/spatial.rs),
[animation tree](../../perro_source/runtime_project/perro_internal_updates/src/nodes/animation_tree.rs),
[animation player tracks](../../perro_source/runtime_project/perro_internal_updates/src/nodes/animation_player/tracks.rs),
and bone-chain [2D](../../perro_source/runtime_project/perro_internal_updates/src/nodes/physics_bone_chain_2d.rs) / [3D](../../perro_source/runtime_project/perro_internal_updates/src/nodes/physics_bone_chain_3d.rs).

Fix adjacent physics query correctness in [body synchronization](../../perro_source/runtime_project/perro_physics/src/system/sync.rs):
update unchanged-shape collider/BVH poses when body pose changes, so queries observe a teleport before the next physics step.
Track this separately from T02 speed claims; use public [sync-query regression](../../perro_source/runtime_project/perro_physics/tests/sync_query_pose.rs) against both trees.

### Network, audio and editor

| ID | State | Change | Remaining scope / tradeoff |
| --- | --- | --- | --- |
| N01 | Implemented | Reuse idle TCP/UDP receive buffers; defer framed TCP peer formatting until an event exists. | Retain requested high-water capacity during idle. Transfer successful reads into independent owned payloads. UDP mutex preserves `Sync`; active payload reads still allocate. |
| N02 | Implemented | Maintain pending TCP byte count on enqueue and every successful partial write. | Preserve queue cap, cursor and error/WouldBlock behavior. Queue payload allocation remains. |
| N03 | Implemented | Use live bitsets, a cached lowest free ID, live count and lazy `Arc<[u32]>` poll snapshots. Reuse lowest free ID; restore removed in-range WebSocket handles on reconnect. | Replace the first BTreeSet candidate after its tiny/early-ID churn regression. Retain bitset and snapshot memory. Membership changes invalidate snapshots. Preserve ascending IDs, transport-phase event order, `Send` and `Sync`. |
| A01 | Implemented | Build lazy playback-ID index above eight voices; retain linear tiny path and first-duplicate semantics. Invalidate every production push/swap-remove. | Two maps retain high-water capacity. Rebuild after membership changes; no device callback latency claim from lookup timing alone. |
| A02 | Implemented | Count attached dry sources waiting for delay storage; stop staging spare buffers for sources that already own wet delays. Balance take/drop paths. | Preserve worker allocation and nonblocking audio take. Existing one-spare/last-layout shared-control policy remains; this does not redesign mixed-layout queues. |
| E01 | Partial | Cache `SceneDocIndex` by immutable `Arc` identity in eight weak-source entries; deduplicate root membership with a set. | Weak keys do not pin documents, but indexes consume retained memory. Gizmo-capable membership filtering remains deferred. |
| E02 | Partial | Return on identical serialized scene text before cached-document clone/replacement; preserve undo/redo and cache identity. | Serialization still runs. Full document/undo sharing, byte limits and delta history remain deferred. |

Trace [TCP](../../perro_source/api_modules/perro_networking/src/tcp.rs),
[UDP](../../perro_source/api_modules/perro_networking/src/udp.rs),
[slot storage](../../perro_source/api_modules/perro_networking/src/slot.rs),
[world polling](../../perro_source/api_modules/perro_networking/src/world.rs),
[audio lookup](../../perro_source/audio_stack/perro_pawdio/src/internal.rs),
[DSP](../../perro_source/audio_stack/perro_pawdio/src/dsp.rs),
[editor index](../../perro_editor/res/scripts/ui/editor_ui.rs),
and [editor scene writes](../../perro_editor/res/scripts/editor/main.rs).

### Build, I/O, website and CI

| ID | State | Change | Remaining scope / tradeoff |
| --- | --- | --- | --- |
| B01 | Implemented | Read only glTF document for materials; import document/buffers for mesh, skeleton and animation. Skip discarded image decode. | Valid-input output parity required. Deliberately stop rejecting irrelevant invalid image payloads; material-only import also skips irrelevant external buffers. No shared glTF import cache. |
| B02 | Implemented | Detect canonical all-entry archive reuse before payload assembly; compare changed output against already-read old bytes. | Still read old archive once. Changed archives still use whole-output assembly; streaming output remains deferred. Preserve source-stat reuse policy. |
| B03 | Partial | Reuse one lazy GPU device/queue per shader-bake export; clear failed context. Preserve per-job uniforms and output. | Shader pipeline/layout, target and readback allocation still occur per job. Do not claim pipeline-cache or whole-GPU-memory savings. |
| B04 | Partial | Gate unchanged scene, animation and CSV codegen with exact dependency membership, content hashes, context and output checks. Store the hash of known generated bytes after a successful write. | Skip cache for fewer than eight dependency paths totaling less than 16 KiB. Enabled hits still hash inputs and disk output; hash executable once per process. Retain cold hash cost. Cache whole kinds, not individual fragments; other emitters remain unchanged. |
| B05 | Implemented | Encode texture/audio/mesh misses in worker-count-sized batches, then write each batch in original order. | Bound completed blobs to one batch plus active encoder buffers. Barriers may affect throughput. Preserve collision write order; earlier batches may exist after later failure. |
| B06 | Partial | Omit unused markdown from production docs JSON and `DocPage`; retain full markdown in test payload. Include JSON through small generated Rust constants. | Preserve routes, rendered HTML and full search corpus. HTML/search still load eagerly; route-level splitting and borrowable data remain deferred. Internal Rust `DocPage.markdown` exists only under `cfg(test)`. |
| B07 | Implemented | Cache at most 64 PNGs by generated SVG; limit cold rasters to two blocking tasks; add one-hour cache header. | FIFO eviction, retained PNG bytes and duplicate concurrent cold work remain possible. Unknown route keys share fallback SVG. No mixed-endpoint tail claim from repeated-route probe. |
| B08 | Implemented | Mirror demo bundles with bounded byte comparisons; copy changed files and remove stale paths only. | Unchanged files still require reading source and destination. Preserve output bytes and untouched file metadata; no filesystem write-count claim from elapsed time alone. |
| C01 | Implemented; remote validation pending | Add `Swatinem/rust-cache@v2` to test/editor/slow-test/Clippy jobs with OS shared key and action's dependency/toolchain keying. | Validate YAML locally. Measure cache transfer, dependency build time and hit rate on actual CI runs; no local CI wall-time estimate. |

Trace [material glTF import](../../perro_source/build_pipeline/perro_static_pipeline/src/materials/gltf_import.rs),
[archive packer](../../perro_source/io_stack/perro_assets/src/packer.rs),
[shader bake](../../perro_source/build_pipeline/perro_static_pipeline/src/shader_bake.rs),
[codegen cache](../../perro_source/build_pipeline/perro_static_pipeline/src/cache.rs),
[texture batches](../../perro_source/build_pipeline/perro_static_pipeline/src/textures.rs),
[website build](../../perro_website/build.rs),
[PNG cache](../../perro_website/src/og_cache.rs),
[demo sync](../../perro_website/src/demo_sync.rs),
and [CI workflow](../../.github/workflows/ci.yml).

## Preserve cache and export behavior

- Hash codegen source/output contents through a 64 KiB buffer. Detect same-size edits even when mtime returns to its previous value. Treat unreadable, non-file or stat-changing reads as cache misses; let normal generation report errors.
- Store a successful generation using the known output bytes and actual output metadata; use matching 64 KiB hash chunks. Avoid reopening fresh output. A subsequent hit still checks full disk contents, including same-size/restored-mtime edits. Skip cache work for tiny input sets before hashing the executable.
- Include executable path, stats and contents, cached once per process. Disable this cache if executable identity cannot be read. Use local noncryptographic content fingerprints, not a security/integrity boundary.
- Include `.pretarget` presence/content, scene dependency set, actual loaded shader fingerprints, asset/DLC prefix and generator context. Bypass scene cache hits in demo mode until global exclusion rules have an explicit key.
- Regenerate missing or edited generated files. Persist sidecars only after successful generation; cache persistence failure does not turn a successful export into failure.
- Keep pre-existing binary asset/archive stat caches separate. Their same-size/restored-mtime trust policy does not change in B04.
- Keep glTF structural validation and required buffer import. Reject invalid images when the texture path consumes them; stop treating unrelated payload validation as a material/mesh/animation prerequisite.
- Keep batch output order for colliding source extensions. Publish fresh manifests/prune only after the complete pass succeeds. Partial output writes remain recoverable through a later successful pass; no transactional export guarantee.

## Match workloads and measurement boundaries

Build baseline and candidate before timing. Copy only fixtures, test hooks and required benchmark manifest entries into baseline `3a4549f9`; preserve baseline production code.
Finish this task's compiler/test work before final timing. Record other compiler activity during each probe and reject affected wall-time comparisons. Keep raw output, executable/profile identity, feature flags and host details alongside results.
Compare like profiles: Criterion uses workspace bench profile; private release-test probes use their matching test profile. Do not mix either with generated-game O3/fat-LTO/one-CGU results.

| Probe | Matched workload / boundary | Limit |
| --- | --- | --- |
| Graphics `resource_loading`, `cpu_prepare`, `gpu_frame` | Completed mesh/texture load; metadata dirt; mixed LOD bands; caster/noncaster motion; sparse/full UI; sparse transforms at 1k/10k draws; uniform/mixed water. GPU: 30 warm + 120 sampled frames for R02/R03/R07/R08; 60 warm + 240 samples for final R01 and three R04 pairs. | Keep host prepare/encode, GPU timestamps and present waits separate. UI packing falls outside the current encode/post timers; use whole-frame host time without attributing that entire difference to packing. Texture fixture uses per-iteration teardown to bound retained payloads. |
| Runtime `audit_runtime` | Warm 10k/100k-triangle local/long nav queries; fixed 1024 total awake bodies over 1/4/16 worlds; 60/120-bone rigs; 8/100/1000 bindings; 10k/100k-slot spatial queries. | Include real public API work. Keep first-hit spatial query unchanged control. Cold BVH cost requires separate measurement. |
| Runtime `audit_runtime_remaining` | Full runtime graph updates with linear/diamond graphs; sparse/dense bone contacts at 0/16/256/2048 colliders plus tiny controls. | Preserve dense-contact controls; rejection bounds can lose when most candidates collide. |
| Scripting `nested_fallback_audit` | Typed map roots at 8/128/4096 fields; first/last/missing get, set, 1/8/64-key Variant patch; direct typed controls. | Imported/dynamic fallback surrogate, not a generated-script end-to-end benchmark. T08 remains deferred. |
| Network private probes | 1k/10k queued appends; 10k idle polls at 4/64 KiB; 16/10k-slot fill and first/last-ID churn; 4096 historical slots with 1/32 live and 128 fully live UDP sockets. | Slot probe isolates bookkeeping. World probe also includes N01 UDP changes and real socket syscalls. Churn case includes bind/remove. |
| Audio `audit_playback` / DSP probe | Paused, muted 1/8/32/256/1000 voices with last/cyclic lookup; 100 wet sources and repeated control updates. | Audio device required for public playback probe. Delay capacity counts exclude allocator overhead and unrelated audio buffers. |
| Editor ignored probes | 1k/10k-node immutable document index reads and identical scene writes. | Helper scope only; no full-editor FPS or total undo-memory claim. |
| Export `export_hotpaths` | Material: eight models, four 1024² image refs each. Archive: eight 4 MiB files, unchanged build. Textures: 64 × 512² PNG cold bakes. Shader: ten 32² bakes. Codegen: 100 scenes, 100 animations and 25k CSV rows. Small codegen: one scene, one animation frame and one CSV row. | Exclude fixture creation, cold-output reset, verification and teardown. Include generation/output writes. Repeat `--first-call` in seven fresh processes; include lazy executable hash, exclude process/Rayon startup and fixture creation. This is not full CLI startup latency. |
| Website OG probe | Same real PNG handler route, response/body consumption; 20 warm samples with byte parity and checksum. | Exclude first cache fill. Do not infer cold/concurrent endpoint latency. |
| Website demo-sync probe | Six 8 MiB files; 20 alternating old/new warm syncs; exact output parity and mtime checks. | Embed exact pre-change sync/copy algorithm in same executable. Label this local algorithm comparison separately from detached-baseline runs. |

Run export timing with `--memory` absent. Run memory instrumentation separately.
Record `peak_requested_bytes` as Rust requested live-heap delta, not process RSS, GPU memory or native-driver allocation.
Keep the disabled allocator flag check in both timing binaries.

Count A02 delay buffers separately: 100 wet 48 kHz stereo sources own 11,443,200 sample bytes in both versions;
the unused spare allocation changes from another 11,443,200 bytes to zero.
Use direct buffer-capacity accounting; do not convert this into an audio latency claim.

Compare website JSON within one candidate build and one document corpus: latest checked pair contains 174 pages, 6,657,979 bytes with test markdown versus 4,876,437 production bytes. Remove 1,781,542 bytes (26.758%); remove only the 174 markdown fields. Verify identical remaining fields, page order, routes, rendered HTML and full search text. Record file hashes and read-stability checks in `same-build-docs-parity-clippy.json`; do not compare different baseline/candidate documentation inventories or infer browser latency from byte size alone. Retain the earlier 173-page same-build pair separately.

## Final measured results

Use Windows, i9-9900K (8 cores / 16 logical), RX 7800 XT, driver `32.0.11029.1008`.
Use Rust `1.97.1 (8bab26f4f 2026-07-14)` and Cargo `1.97.1 (c980f4866 2026-06-30)`.
Match workspace bench O3, no LTO, 64 codegen units, no incremental/debug; match generated editor O3 + debug level 2 in both trees.
Pin CPU probes to logical CPU 15; allow all 16 logical CPUs for completed asset, export and GPU probes. Set export worker count to four.
Observe compiler/link activity during each accepted run: all 82 run records report exit 0 and no compiler load. This does not establish an otherwise idle OS.

Read 69 Criterion rows, sample arrays, GPU controls, hashes and run arguments in [portable result data](performance_audit_results_3a4549f9_2026-09-06.json).
Use Criterion central estimates below; preserve lower/upper 95% intervals in its `base_ns` / `new_ns` arrays. Use 20 samples, 0.5 s warmup and 1 s measurement per Criterion case. Ignore Criterion's stored `change:` percentages from prior runs.
Use sample medians for private/export probes; divide only cases with explicit `operations_per_sample` by that count. Keep batch totals labeled.

### CPU and helper costs

| ID | Workload | Baseline | Candidate | Scope |
| --- | --- | ---: | ---: | --- |
| T01 | Local nav, 100k triangles | 17.803 ms | 2.732 us | Warm nearby query; excludes cold BVH build |
| T01 | Long nav, 100k triangles | 26.703 ms | 9.027 ms | 2.96x speed |
| T02 | 1,024 awake bodies / 16 worlds | 1.707 ms | 1.230 ms | Fixed total bodies; 1-world control 887 -> 879 us |
| T03 | 120 bones x 100 rigs | 3.540 ms | 1.276 ms | Full runtime tree update |
| T04 | 1,000 nonbone bindings | 3.851 ms | 0.373 ms | 8-binding control 1.708 -> 1.729 us |
| T03 + T05 | Diamond graph, depth 8 | 1.804 ms | 40.45 us | Includes batch bone writes; not graph cache alone |
| T06 | Sparse 256 colliders, 16 rigs x 16 bones | 1.876 ms | 0.801 ms | 2.34x speed |
| T06 | Sparse 2,048 colliders | 16.047 ms | 12.957 ms | Candidate loop remains |
| T07 | One candidate / 100k slots | 71.22 us | 2.060 us | Broad first-hit control 1.436 -> 1.425 ms |
| T08 | Typed-root set, 4,096 fields | 2.482 ms | 2.464 ms | Unchanged path; no claimed gain |
| R05 | Completed compressed mesh, 10k triangles | 2.508 ms | 1.388 ms | Source lookups 2 -> 1 |
| R06 | Completed 32-texture batch | 18.567 ms | 18.934 ms | Pool isolation; no throughput gain |
| N01 | Idle raw TCP poll, 64 KiB | 1.884 us | 0.328 us | Real idle socket syscall |
| N01 | Idle UDP poll, 64 KiB | 1.894 us | 0.378 us | Real idle socket syscall |
| N02 | 10k queued appends, whole batch | 48.67 ms | 0.142 ms | Queue bookkeeping |
| N03 | Fill 10k slots, whole batch | 17.63 ms | 66.6 us | Includes slot growth |
| N03 | Churn last ID / 10k slots | 3,550 ns/op | 6.94 ns/op | Slot bookkeeping |
| N01 + N03 | Poll 4,096 historical slots / 1 live | 3.072 us | 0.413 us | Includes UDP buffer reuse |
| A01 | Last-voice lookup / 1,000 voices | 1.086 us | 0.167 us | Public paused/muted playback control |
| E01 | Cached index / 10k nodes | 327.2 us | 37 ns | Warm immutable document; not editor FPS |
| E02 | Identical scene write / 10k nodes | 30.07 ms | 27.25 ms | Serialization remains |
| B07 | Warm same-route PNG handler | 70.77 ms | 5.15 us | Same 45,521-byte PNG; excludes cold raster |

### Export and memory costs

| ID | Workload | Baseline | Candidate | Scope |
| --- | --- | ---: | ---: | --- |
| B01 | Eight material imports, four image refs/model | 45.73 ms | 0.469 ms | Output parity |
| B02 | Unchanged eight-file archive, 32 MiB payload | 43.797 ms | 11.462 ms | Fresh assets build only; 20 samples |
| B03 | Ten 32x32 shader bakes | 2.309 s | 0.336 s | Same pixels/checksum |
| B04 | Large warm scene/animation/CSV generation | 88.03 ms | 25.77 ms | Warm median; input/output hashing remains |
| B04 | Large first generation | 89.56 ms | 115.20 ms | Seven fresh processes; 28.6% slower |
| B04 | Small warm generation | 1.077 ms | 1.176 ms | 0.099 ms / 9.2% slower |
| B04 | Small first generation | 2.676 ms | 2.830 ms | 0.155 ms / 5.8% slower |
| B05 | Cold bake, 64 textures at 512x512 | 414.1 ms | 453.8 ms | 9.6% slower; lower peak memory |
| B08 | Warm unchanged demo sync, six 8 MiB files | 18.429 ms | 20.274 ms | Same-executable old/new algorithms; 10.0% slower |

| ID | Memory / bytes | Baseline | Candidate | Measurement |
| --- | --- | ---: | ---: | --- |
| A02 | Unused spare delay samples / 100 wet sources | 11,443,200 B | 0 B | Direct capacity; live owned delays stay 11,443,200 B |
| B01 | Material import peak | 85,047,458 B | 51,787 B | Rust requested live-heap delta; separate five-sample median |
| B02 | Unchanged archive peak | 134,222,922 B | 33,562,274 B | Fresh assets; separate three-sample median; 75% lower |
| B05 | Texture bake peak | 84,188,647 B | 22,165,973 B | Separate memory pass; 73.7% lower |
| B06 | Same-build 174-page docs JSON | 6,657,979 B | 4,876,437 B | 26.758% smaller; remaining fields exactly equal |

Keep measured costs visible:

- Retain T06 tiny sparse control: 1 rig x 2 bones / 16 colliders, 5.398 -> 5.642 us (+0.244 us / 4.5%). Zero-collider control stays about 57 us; tiny dense control improves 9.872 -> 6.512 us.
- Retain A01 cyclic 32-voice control: 81.23 -> 88.84 ns (+7.61 ns / 9.4%); last-voice 32 control stays about 88 ns. Large-voice index gains do not erase small-path cost.
- Replace initial N03 BTreeSet candidate after 72-80 ns first-ID churn. Final bitset path: 2.67 -> 6.94 ns (+4.27 ns) at 10k slots; large fill and last-ID churn gain substantially. Small 16-slot fill: 18.75 -> 25 ns/insert.
- Fix B04 first candidate's repeated fresh-output read by hashing known generated bytes; bypass cache for tiny sets before executable hashing. Retain final cold-large +25.64 ms hash cost. First warm-series sample reaches about 0.37 s in both versions; keep raw arrays and avoid tail-latency claims from the median.
- Keep B05 bounded batches for lower peak memory despite the measured cold-texture slowdown. This fixture does not establish audio/mesh batch throughput.
- Keep B08 content-aware copy semantics; this Windows warm-cache fixture shows a slowdown. Both old and new paths preserve mtime here. No measured write-count reduction, general mtime advantage or Linux timing claim.
- Keep R06 isolation scope: four-texture completed batch 2.808 -> 2.662 ms; 32-texture batch essentially unchanged. No decode-burst frame-tail trace.

### GPU-attached controls

Use Vulkan, 1280x720, Immediate present, no vsync and maximum frame latency 8.
Read `gpu_3d_us` as host CPU staging time, despite its field name. Read GPU main/shadow/water values from device timestamps.
Exclude separate `wait_idle` and fixture redraw submissions from reported host draw-frame totals.

| ID | Workload | Baseline -> candidate | Limit / control |
| --- | --- | --- | --- |
| R01 | Identical material write / 10k draws | Host 3D prep 22,099 us -> idle | Whole-frame idle fast path; zero timestamps mean skipped work, not infinite FPS |
| R01 | Resource reservation / 10k draws | Host 3D prep 22,090 -> 8 us; host frame 23,068 -> 295 us | Scene passes 1 -> 0; all 240 candidate frames skip 3D |
| R01 | Actual material change control | Host 3D prep 22,289 -> 22,576 us | Broad staging remains; +1.3% in this pair |
| R02 | Mixed LOD / 10k draws, camera within bands | Host 3D prep 22,002 -> 48 us; host frame 22,918 -> 496 us | GPU main median 102.36 -> 95.80 us; same 10k instances, two draws and one scene pass |
| R03 | Move noncaster | Shadow layers 7 -> 0; shadow draws 2 -> 0 | GPU shadow mean 28 -> 1 us; empty timestamp overhead remains |
| R03 | Move caster control | Shadow layers 7 -> 7; draws 2 -> 2 | GPU shadow mean 28 -> 28 us |
| R04 | Sparse UI / 1k primitives | Host frame 1.243 -> 0.502 ms | Median of three run means; 59.6% lower |
| R04 | Full UI change control | Host frame 2.934 -> 2.767 ms | Run ranges overlap; no strong full-change speed claim |
| R07 | Unchanged water dispatch | Low 14 -> 14 us; mixed 17 -> 17 us; ultra 195 -> 195 us | Mixed active 192,512 vs rectangular 2,097,152 invocations does not imply proportional GPU savings |
| R08 | One sparse transform / 10k draws | Host 3D prep 583 -> 614 us | Broad scans remain; candidate 1k -> 10k scale 57 -> 614 us |

Use final quiet `resource-isolated2` pair; reject both `resource-isolated1` timings due compiler load.
Use three alternating UI pairs: sparse baseline run-mean range 1.213-1.316 ms, candidate 0.498-0.511 ms; full baseline 2.759-3.023 ms, candidate 2.663-2.907 ms. These ranges are not confidence intervals.
Reject the preliminary single-pair full-UI regression claim: it does not reproduce in the three quiet pairs.
Keep UI GPU main near 55-56 us; host total includes packing but does not isolate it. Packing occurs outside both main-encode and post timers.

### Keep evidence and rejected samples separate

Keep raw artifacts in `C:\Users\super\.codex\tmp\perro-perf-audit-results-2026-09-06`.
Keep baseline workspace in `D:\Rust\Perro-perf-audit-baseline` and separate Cargo targets on C: `perro-perf-audit-base-target-2026-09-06` / `perro-perf-audit-target-2026-09-06` under `C:\Users\super\.codex\tmp`.
Do not share a target directory across those source roots.

| Evidence | Raw files |
| --- | --- |
| Exact source / profile / feature identity | `isolated-base-build-benches-final.jsonl`, `isolated-base-package-proof.json`, `isolated-base-executables.json`, `matched-bench-identities.json`; 305 common artifact identities, no profile/feature difference, 13 identical fixture files |
| CPU Criterion | `final-{base,new}-{runtime,remaining,voices,metadata,mesh,texture,nested}-isolated1.*`, `summary-criterion-isolated1.json` |
| Socket / slot / DSP / editor / PNG | `final-{base,new}-{net,slots,dsp,editor,og}-isolated1.*` |
| Export timing / memory | `final-{base,new}-export-{material,textures,shader,codegen,codegen_small}-isolated1.*`, `final-{base,new}-memory-{material,textures}-isolated1.*` |
| Accepted archive only | `final-{base,new}-{export,memory}-archive-assets-refresh.*`, `summary-assets-refresh.json` |
| First generation | `final-{base,new}-first-{codegen,codegen_small}-{1..7}-isolated1.*` |
| GPU | `gpu-paired-read-audit-final.json` for R01/R04; `gpu-paired-read-audit-isolated1.json` for R02/R03/R07/R08 only |
| Local demo-sync algorithm comparison | `final-demo-sync-isolated1.txt` |
| Same-build docs parity | `same-build-docs-parity-clippy.json` |

Keep `measured-pre-assets-*.exe` snapshots for earlier accepted nonarchive timings. The assets refresh relinks all eight `new-final-*` benchmark executables; those mutable names no longer identify the earlier measured bytes. Match each run SHA to the preserved snapshot. Earlier accepted kernels do not call the archive packer; rerun the archive with the fresh library.
Confirm stale candidate/base assets library SHA `029AFB9C8BFC2D94162E33244E353A4D1DB5D835FBCD039F942DBBA40A912F99` changes to candidate `21F28BD05DABD53059F0A819B3625AF453B6A08D31B76D347F1B64E328B43461` after a real rebuild. Keep `assets-refresh-before-hashes.json`, `assets-refresh-current-artifacts.json` and `new-assets-refresh-manifest.json` as provenance.
Keep C01 CI duration/cache-hit evidence pending until remote workflow execution; no local speed estimate.

## Reproduce focused probes

Use these targets in both trees with identical feature/profile settings. Build all required executables first; run timing probes serially.
Set `CARGO_TARGET_DIR` to the corresponding separate baseline/candidate target above before each checkout's build commands.

```powershell
cargo bench -p perro_runtime --features bench --bench audit_runtime --no-run
cargo bench -p perro_runtime --features bench --bench audit_runtime_remaining --no-run
cargo bench -p perro_scripting --bench nested_fallback_audit --no-run
cargo bench -p perro_graphics --bench resource_loading --no-run
cargo bench -p perro_graphics --bench cpu_prepare --no-run
cargo bench -p perro_graphics --bench gpu_frame --no-run
cargo bench -p perro_pawdio --features playback --bench audit_playback --no-run
cargo bench -p perro_static_pipeline --bench export_hotpaths --no-run
```

Pass export executable arguments `--case=material|archive|textures|shader|codegen|codegen_small --samples=10 --threads=4`, choosing one case per process. Add `--first-call` for one cold generation per fresh process; repeat seven times.
Repeat texture cases at worker counts 1 and 4; add a separate `--memory` pass.
Set GPU fixture environment/filter parameters exactly alike in both trees; retain sparse/full and camera-band controls.

Run private test filters from matching built test executables with `--ignored --nocapture --test-threads=1`:

| Crate / target | Filters |
| --- | --- |
| `perro_networking` library | `audit_queue_append`, `audit_idle_socket_polls`, `time_slot_insert_and_min_free_churn`, `time_world_highwater_sparse_full_and_churn` |
| `perro_pawdio` library, playback | `audit_wet_delay_storage` |
| Generated editor script tests | `audit_editor_index`, `audit_editor_noop_write` |
| `perro_website` binary, SSR | `time_og_png_repeated_route` |
| `perro_website` library or standalone demo-sync test | `time_unchanged_demo_bundle_sync` |

Build standalone std-only demo-sync probe with `rustc --edition 2021 --test -O perro_website/src/demo_sync.rs -o <temporary-executable>`.
Copy network/DSP/OG fixture files and their test module hooks into baseline; slot storage type comes from `Default` inference.
Copy export `tests/support/mod.rs`, benchmark source and its target/dev-dependency entry unchanged.

## Verify behavior

```powershell
cargo test -p perro_graphics --lib
cargo test -p perro_runtime --lib --features bench
cargo test -p perro_internal_updates --lib
cargo test -p perro_physics --test sync_query_pose
cargo test -p perro_networking --features network-tests
cargo test -p perro_pawdio --features playback
cargo test -p perro_static_pipeline --lib --test export_regressions
cargo test -p perro_assets
cargo test -p perro_cli
cargo test -p perro_website --lib --bin perro_website
cargo run -p perro_cli -- test --path perro_editor
```

Run explicit ignored shader-bake pixel tests when GPU access exists; keep them separate from timing filters.
Check scene/animation/CSV clean-versus-warm bytes, same-stat edits, inventory and missing-output recovery.
Check serial/parallel bake output order, archive source-set/layout repair, docs JSON route/search/HTML parity and unchanged bundle metadata.
Check network minimum-ID reuse, held snapshot validity, removed-ID reconnect, event ordering and `Send`/`Sync`.
Check graph reference parity/cycles, bone masks/order, nav endpoint ties/layers/obstacles, world ancestry edits and spatial slot reuse.
Check renderer no-op event/resource behavior, real caster controls, LOD-boundary/alpha fallbacks and viewport-cap resize.

Record 2,601 normal workspace test passes across 59 test targets, with 29 ignored tests in their normal runs. Record editor separately: 57 pass / three ignored. Run one additional GPU shader pixel test explicitly; pass.
Count staged suites and replacement reruns, not one atomic final `cargo test` execution. Start with 2,538 passes across 55 targets; recover CLI 59 and script-context 1 through Cargo, plus two zero-test macro targets. Replace networking 93 -> 95 and static-pipeline library 80 -> 81 after final fixes. Keep its three export integration tests in the existing total. Recheck the fresh assets library: 14 pass / one ignored.
Resolve the four initial harness failures: use Cargo with the correct context for trybuild and macro DLL lookup; replace the overwritten CLI test snapshot. Resolve the generated editor missing-module build after its owner regenerates scripts. Preserve the failed and successful logs.
Verify public physics teleport regression against both trees: baseline two failures -> candidate two passes. Keep this correctness evidence separate from physics timing.

Run final workspace Clippy with `--profile bench --workspace --all-targets --features perro_runtime/bench,perro_networking/network-tests --keep-going -- -D warnings`: pass, including CLI. Record command and timestamps in `final-clippy-complete.run.json` with stdout/stderr beside it.
Check `cargo fmt --all -- --check` and `git diff --check`: pass. Record `final-format-diff-report.json`; apply only formatting to the three late concurrent CLI/runtime formatting differences.
Keep suite evidence in `final-test-results.json`, `validate-{cli,macros,script-context}-cargo.*`, `n03-bitset-tests.jsonl`, `assets-refresh-tests.jsonl`, codegen fix logs, `validate-editor-tests.*` and `final-shader-pixel.txt`.
Keep scope explicit: no doctest, all-feature matrix, WASM/platform matrix or remote CI run. Preserve concurrent editor/CLI/compiler changes; test counts and benchmarks describe the recorded snapshots, not later edits by other tasks.
