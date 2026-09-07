# Audit repo prf — 2026-09-06

Scan baseline `3a4549f9`; start w/ clean worktree.
Count 44 workspace crates; 1,393 tracked fles; 931 Rust fles; 60 shader fles; 45 Rust bench fles.
Cover engine + editor + demos + website + build/CI cfg.
Use repo-wide inventory/search + deep call-path checks; ! every-fn proof.
List 32 remaining targets; rank by source cost + workload trigger.
Treat all gains as hypotheses; run no new timing, GPU capture, or game workload.
Use high confidence 4 cited work/repeat paths; leave actual time/FPS gains unmeasured.
Add report only; run no cargo tests/builds.

Use P1 4 broad hot-path cost at named scale; P2 4 workload-specific gains; P3 4 lower-priority cost.
Use effort low/med/high as scope estimate, ! schedule promise.

| Start | Target | Why |
| --- | --- | --- |
| 1 | R05 mesh decode | Low effort; duplicate load/decode in same path |
| 2 | N02 TCP queue bytes + A02 audio spare buffers | Local changes; direct scan/mem cost |
| 3 | R01 resource dirt + T02 world physics dirt | Broad repeat work; add counters b4 refactor |
| 4 | T01 navmesh + T03 tree bone writes | Scale w/ agents/bones despite warm state |
| 5 | R02/R04/R08 GPU prep | Large-scene gains; higher cache/invalidation risk |
| 6 | B01–B05 export path | Load/build latency + peak mem |

## Audit render + GPU paths

### R01 — narrow resource dirt — P1 / med-high effort

- Trace [cmd dirt](../../perro_source/render_stack/perro_graphics/src/backend.rs#L390) -> [main GPU prepare](../../perro_source/render_stack/perro_graphics/src/gpu/frame.rs#L435) + stream prepare at line 1074.
- Trig any resource cmd, incl one param write, no-op write, or reservation change -> `DIRTY_RESOURCES` -> full 3D restage. [Param setter](../../perro_source/render_stack/perro_graphics/src/backend/command_process.rs#L553) drops changed/no-change result.
- Pay O(scene instances + batches + buffer hashes), even w/ no geometry change.
- Add post-apply dirt; split resource shape/data/lifetime stamps; patch affected material slots. Kp reload fallback + mesh/material/texture deps.
- Measure 10k static draws + one param/no-op/reservation cmd/frm; count full rebuilds + upload bytes + CPU prepare.

### R02 — isolate camera-dependent draws — P2 / high effort

- Trace [camera gate](../../perro_source/render_stack/perro_graphics/src/three_d/gpu/prepare.rs#L508), full rebuild at line 924, LOD/alpha flags at lines 1132/1274/1617/1770.
- Trig camera move + one multi-LOD or alpha-blend draw among many opaque single-LOD draws -> whole regular scene restage. Dense multimesh reuse still skips some payload work.
- Retain fixed-LOD opaque rows; patch actual LOD changes + re-sort alpha order. Kp cull, shadows, blend deps + valid draw ranges.
- Measure 9,999 stable opaque draws + one LOD/alpha draw; compare tiny camera move vs LOD-boundary cross; count staged rows + CPU prepare.
- Exclude already-fixed all-opaque single-LOD camera path.

### R03 — skip noncaster shadow dirt — P2 / med-high effort

- Trace [span upload hash reset](../../perro_source/render_stack/perro_graphics/src/three_d/gpu/prepare.rs#L636), caster dirt at line 913 -> [whole-lane caster key](../../perro_source/render_stack/perro_graphics/src/three_d/gpu/shadows.rs#L722), dirty decision at line 956.
- Trig moving `cast_shadows=false` draw w/ static lights + static casters -> unknown whole-lane hash -> mark local shadow maps dirty; existing budgets limit work/frm.
- Add caster-only stamps + dirty spans; avoid shadow depth work 4 noncaster transforms. Kp skin, vertex modifier, blend + alpha-cutout deps.
- Measure shadow-layer renders + GPU shadow time w/ one moving noncaster; use real moving caster as control.

### R04 — patch sparse UI GPU buffers — P2 / high effort

- Trace [whole-mesh gate + repack](../../perro_source/render_stack/perro_graphics/src/ui/gpu.rs#L588), full upload at line 672.
- Trig one counter/caret/color/projected-label mesh change -> repack + upload all UI verts/indices. Kp existing per-node tessellation cache; ! full text reshape claim.
- Pay O(all verts + indices); upload `28 × verts + 4 × indices` bytes.
- Add persistent primitive spans; patch equal-size changes; use chunk/rebuild fallback 4 topology changes. Kp clip/z-order, atlas + depth rules.
- Measure one changed node among 1k/10k UI nodes; count repacked verts + upload bytes; extend `label3d_ui` + GPU fixture.

### R05 — decode mesh once — P2 / low effort

- Trace [validate + load](../../perro_source/render_stack/perro_graphics/src/backend/asset_load.rs#L37) -> [validator full decode](../../perro_source/render_stack/perro_graphics_assets/src/mesh.rs#L276) + loader at line 289.
- Trig uncached non-builtin mesh -> read/decompress/parse/alloc twice; first decoded mesh drops b4 second load. Native async + sync WASM/test branches share pattern.
- Use one fallible loader -> decoded mesh or err; publish same result. Kp builtin behavior + failure events.
- Measure cold/warm filesystem GLB + compressed PMESH; count reads/decodes + load CPU; chk malformed-file parity.

### R06 — keep asset waits off shared CPU pool — P2 / med effort

- Trace [Rayon texture jobs](../../perro_source/render_stack/perro_graphics/src/backend/asset_load.rs#L138) -> [blocking decode permit](../../perro_source/render_stack/perro_graphics/src/backend/asset_load.rs#L301).
- Trig texture burst >4 decodes -> shared Rayon workers wait on Condvar or I/O. Same pool serves [script jobs](../../perro_source/api_modules/perro_jobs/src/lib.rs#L62), renderer joins + sorts.
- Use bounded asset executor or admission queue; submit only permitted decode work. Kp four-decode mem cap; bound completed-payload backlog too.
- Measure frame p95/p99 + script-job latency during 4/16/64-texture burst; track decode throughput + queue bytes. Leave actual frame impact open until trace.

### R07 — dispatch water compute by sim size — P2 / med effort

- Trace [max grid size](../../perro_source/render_stack/perro_graphics/src/water_gpu.rs#L1104), dispatch at line 1305 -> [excess-invocation exit](../../perro_source/render_stack/perro_graphics/src/water_shaders/water_compute.wgsl#L163).
- Trig mixed water sim grids -> max-cell X grid × every water; small waters launch excess work that loads descriptor + exits.
- Group dispatch by size or map workgroups -> water/cell. Cut launch model `waters × max_cells` toward `sum(cells)`; actual GPU gain depends on overhead.
- Kp water offsets + pause/ping-pong rules. Measure useful/launched workgroups + isolated sim GPU time, uniform vs mixed quality.

### R08 — carry dirty draw rows into GPU prepare — P2/P3 / high effort

- Trace [all-pair classify](../../perro_source/render_stack/perro_graphics/src/three_d/gpu/draw/batch.rs#L332) -> [patch passes](../../perro_source/render_stack/perro_graphics/src/three_d/gpu/prepare.rs#L572), batch scans at lines 657/765 + whole last-draw sync at line 915.
- Trig one transform edit among N stable draws -> repeat O(N) CPU scans despite sparse GPU writes. ! full-restage claim.
- Carry dirty row IDs + change kinds; add draw -> batch map; kp full revision fallback, slot generations + compaction aliases.
- Measure GPU-attached 1/100/all edits over 10k/100k draws; count visited rows + CPU prepare. Headless `cpu_prepare` alone misses this stage.

## Audit runtime + core + scripts

### T01 — cut warm navmesh query prep — P1 / med-high effort

- Trace [prepared query](../../perro_source/runtime_project/perro_runtime/src/runtime/navmesh.rs#L125), endpoint scan at line 218, A* scratch at line 435 + obstacle mask at line 614.
- Repeat full resource validation, both endpoint triangle scans + full obstacle-mask build even w/ zero obstacles; alloc fresh triangle-sized A* arrays when endpoints need graph search. Kp existing cached search graph + same-triangle early return.
- Pay O(Q × T) prep 4 Q agents / T triangles; obstacle checks add O(Q × T × obstacles), even 4 short local paths.
- Validate on resource insert/replace; add triangle BVH, empty-mask fast path + generation-stamped scratch. Kp endpoint tie/layer/distance + invalid-resource rules; bind caches to resource identity.
- Measure warm 10k/100k-triangle mesh; 1/100 local queries; 0/32 obstacles; split prep/search + alloc counts.

### T02 — scope physics sync by world + dim — P1/P2 / high effort

- Trace [global epoch gate](../../perro_source/runtime_project/perro_runtime/src/runtime/physics/step.rs#L81), world loop at line 241 -> [global body-list filter](../../perro_source/runtime_project/perro_runtime/src/runtime/physics/world_sync.rs#L344).
- Trace awake-body pullback -> [pose accessor](../../perro_source/runtime_project/perro_runtime/src/cns/node_arena.rs#L731) -> global physics epoch; [world switch](../../perro_source/runtime_project/perro_runtime/src/runtime/physics.rs#L118) stores per-world sync epochs.
- Trig awake world A -> stale saved epochs 4 B/C; both dims compare same global stamp. Recollect scans all global body IDs/world; joints follow same pattern.
- Pay O(W × B) membership probes/collect phase 4 W worlds / B total bodies. Add world/dim stamps + body/joint membership; batch sparse changes.
- Kp topology, ancestor transforms, world moves + query freshness. Measure fixed 10k total bodies over 1/4/16 worlds; asleep/awake/one edit; count collections + world-isolation checks.

### T03 — batch animation-tree bone writes — P2 / med effort

- Trace [tree pose apply](../../perro_source/runtime_project/perro_internal_updates/src/nodes/animation_tree.rs#L850) -> [bone write](../../perro_source/runtime_project/perro_internal_updates/src/nodes/animation_player/tracks.rs#L1084) -> [rerender subtree walk](../../perro_source/runtime_project/perro_runtime/src/runtime/render/bridge/commands.rs#L522).
- Pay one mutable skeleton borrow + force-rerender walk/hash-set per bone track; name selector also scans bone list at `tracks.rs:1153`.
- Group tracks by skeleton -> one borrow + one rerender; cache bone-name indices by layout. Kp masks, pose order + duplicate-target last-writer rules.
- Kp existing animation-player batch at `tracks.rs:153`; tree still uses per-track path.
- Measure 60/120 bones × 1/100 skeletons, 0/20 attachments; count borrows, subtree visits + allocs; compare name/index selectors.

### T04 — index animation-player bindings — P2 / med effort

- Trace [object binding filter](../../perro_source/runtime_project/perro_internal_updates/src/nodes/animation_player/tracks.rs#L128) + per-skeleton track scan at line 161.
- Pay O(tracks × bindings) comparisons/applied frame for stable clips/bindings.
- Compile object -> track ranges + matching bindings; retain duplicate object bindings. Kp direct public binding edits via mutation stamps or fingerprint checks.
- Measure 100/1k tracks × 100/1k bindings; stable + one binding edit; chk duplicate-target output. Exclude tree's existing binding index.

### T05 — memo shared animation graph eval — P2 / med-high effort

- Trace [recursive graph lookup](../../perro_source/runtime_project/perro_internal_updates/src/nodes/animation_tree.rs#L486) + fresh blend Vecs at line 507.
- Trig shared DAG node -> repeat sample/blend per inbound path; visiting flags catch cycles only. Linear key lookup repeats per visit; layered diamonds expand work by path count.
- Resolve graph keys once; memo completed poses/eval + pool blend args/weights. Kp cycle, mask, blend order + numeric rules; bound retained pose mem.
- Measure chain vs diamond DAG w/ same distinct node count; count samples/blends/allocs. Rank lower 4 tree-only graphs w/o sharing.

### T06 — add bone-chain collision broad phase — P2 / high effort

- Trace [3D solver loop](../../perro_source/runtime_project/perro_internal_updates/src/nodes/physics_bone_chain_3d.rs#L376) + all-collider tests at line 430; same pattern in 2D.
- Pay roughly `2 × iterations × chain_points × colliders × chains`; far colliders still incur shape checks. Shared collider collection already exists.
- Cache collider AABBs + grid/BVH; query nearby candidates/solver pass. Kp collision order where pushes interact; use conservative margin/requery after pushes.
- Measure 100 chains × 8 points × 10/1k colliders; sparse vs dense contacts; count candidates + chk pose tolerance.

### T07 — avoid dense spatial query snapshots — P2 / med-high effort

- Trace [query-first](../../perro_source/runtime_project/perro_runtime/src/rt_ctx/nodes/node_api.rs#L908) -> [spatial snapshot](../../perro_source/runtime_project/perro_runtime/src/rt_ctx/nodes/spatial.rs#L24).
- Clear/resize both 2D + 3D position arrays to arena slot count/query; candidate-only fill happens aft dense init. Query-first also prepares positions b4 first-match scan.
- Pay O(max arena slots) writes even w/ one indexed candidate + one dim. Recycled buffers avoid alloc, ! dense reset cost.
- Use lazy sequential first-match resolve, stamped sparse snapshots + needed dim only. Kp deterministic order, parallel-read safety + slot generations.
- Measure 100k-slot arena, one tagged candidate/first-node hit, 100 repeated queries, zero/one transform edit; count slots cleared + resolved positions.

### T08 — skip whole-root dynamic nested codec — P2/P3 / high effort

- Trace [generated fallback](../../perro_source/build_pipeline/perro_compiler/src/script_methods.rs#L1098) -> [nested walk](../../perro_source/script_stack/perro_scripting/src/nested_vars.rs#L13), per-key patch at line 141.
- Trig imported/dynamic nested field fallback -> typed root -> owned Variant; set decodes root again. Multi-key patch repeats field walks.
- Pay O(root size) encode/alloc per fallback; M-key patch of N fields can visit O(M × N) fields. Known-field generated fast paths already exist.
- Add typed nested access table from schema + direct keyed object patch. Kp unknown-key + patch rules.
- Measure imported 64/1k-field roots; first/last/missing reads; 1/32-key patches; count codec bytes + allocs.

## Audit net + audio + editor

### N01 — cut idle socket allocs — P2 / low-med effort

- Trace [TCP raw recv](../../perro_source/api_modules/perro_networking/src/tcp.rs#L150), [UDP recv](../../perro_source/api_modules/perro_networking/src/udp.rs#L38), [world poll](../../perro_source/api_modules/perro_networking/src/world.rs#L312).
- Alloc + zero `max_bytes` buf b4 every raw recv, even WouldBlock. [Framed TCP poll](../../perro_source/api_modules/perro_networking/src/tcp.rs#L211) formats peer str b4 checking for event.
- Trig S idle sockets × F polls/sec -> S × F avoidable recv-buffer alloc requests on raw path; framed path still allocs peer str. LAN's separate reusable buf already fixed.
- Add caller/connection scratch or `recv_into`; allocate owned payload only on receipt; defer peer str until event. Kp packet ownership + recv size semantics.
- Measure idle + burst sockets; count allocs + poll CPU; ensure returned payloads remain independent.

### N02 — cache TCP pending-byte count — P2 / low effort

- Trace [pending bytes sum](../../perro_source/api_modules/perro_networking/src/tcp.rs#L296) -> each enqueue at line 304.
- Trig slow peer + N queued small writes -> scan prior queue per append; O(N²) total queue-length work while backlog grows. Byte cap still allows many tiny frames.
- Maintain pending-byte counter on enqueue + successful writes; kp partial-write cursor, queue cap + err paths.
- Measure blocked/slow loopback peer w/ 1k/10k small writes; count queue visits + enqueue CPU; chk cap, partial writes + complete drain.

### N03 — index socket slots + live IDs — P3 / med effort

- Trace [slot insert](../../perro_source/api_modules/perro_networking/src/slot.rs#L3) + [poll slot sweep](../../perro_source/api_modules/perro_networking/src/world.rs#L318).
- Scan from slot zero per insert -> O(N²) fill; poll every historical slot even aft most connections close.
- Add free-slot list + dense live-ID list; kp public handle rules + all disconnect/reconnect removal paths.
- Measure 1k/10k connection churn + large peak -> few live sockets; count insert probes + idle slots/frm. Low priority 4 small lobbies.

### A01 — index live audio playback IDs — P2 / med effort

- Trace [spatial update lookup](../../perro_source/audio_stack/perro_pawdio/src/player/playback.rs#L634), MIDI lookup at line 666; [propagation caller](../../perro_source/runtime_project/perro_runtime/src/runtime/audio/scene.rs#L369).
- Scan live playback Vec by ID/update under state mutex. M updates / V live voices -> O(M × V); updating all voices -> O(V²) lookup work/update batch.
- Add ID -> slot map or indexed slot store; kp prune, swap-remove, stop + MIDI paths. Do ! infer device callback stalls from worker mutex alone.
- Extend `play_source` beyond current single-target spatial update; measure 32/256/1k live voices w/ worker drain fence; count probes + queue latency.

### A02 — skip spare delay buffers for wet sources — P2 / low-med effort

- Trace [source wet alloc](../../perro_source/audio_stack/perro_pawdio/src/dsp.rs#L230), [wet update](../../perro_source/audio_stack/perro_pawdio/src/dsp.rs#L78) -> spare alloc at line 57; consumer only takes spare when `self.wet.is_none()` at line 284.
- Trig already-wet source + spatial update -> stage extra delay set; source keeps own set + never consumes spare. Dry -> wet -> next wet update also refills unused spare.
- Retain extra `114,432` sample bytes/source at 48k stereo: `(8640 + 2256 + 3408) × 2 × 4`; exclude Vec/Box overhead. Calculate from delay sizes; ! heap measurement.
- Stage only for attached sources that lack delays; kp worker-side alloc + nonblocking audio take, shared-control/trim/loop lifecycle.
- Measure live requested bytes b4/aft wet updates; chk dry/wet transitions + concurrent control changes; no new audio-thread alloc/block.

### E01 — cache editor scene index + gizmo membership — P2 / med effort

- Trace [per-frame 2D gizmos](../../perro_editor/res/scripts/scene/editor_viewport.rs#L2392) -> [SceneDocIndex build](../../perro_editor/res/scripts/ui/editor_ui.rs#L3213).
- Trig idle 2D preview -> clone ID/key lists + build/sort node index + child lists/frm; loop all preview nodes even w/ few gizmo types.
- Pay O(N log N) worst-case sort + O(N) list work/frm; many root nodes also hit `roots.contains` quadratic path at line 3234.
- Store index w/ immutable cached SceneDoc; cache gizmo-capable IDs on membership change; use set/root flag 4 root de-dupe. Kp viewport transforms + live gizmo draw output.
- Measure idle 2D 1k/10k-node preview, few cameras/lights; count index builds + allocs; edit/reparent/undo controls.

### E02 — reduce editor scene snapshot duplication — P3 / med-high effort

- Trace [text-keyed doc cache](../../perro_editor/res/scripts/editor/main.rs#L38), deep cache store at line 65, whole-text serialize/store at line 140 + undo snapshot at line 156.
- Kp up to 72 cached docs w/ owned source text + parsed doc/index, beside up to 64 undo text snapshots. Count limits bound entries, ! bytes; large scenes retain many full copies.
- Use document revision/identity 4 hot cache lookup; share immutable text/doc snapshots; consider byte budget + delta undo. Avoid full clone/cache replacement on true no-op edits.
- Kp undo fidelity + drag coalescence + cache lifetime. Measure large scene aft 64 edits, undo/redo + no-op edit; count retained bytes + serialize/clone CPU.

## Audit build + I/O + website

### B01 — skip unused glTF image decode — P2 / low-med effort

- Trace [material import](../../perro_source/build_pipeline/perro_static_pipeline/src/materials/gltf_import.rs#L7), [mesh import](../../perro_source/build_pipeline/perro_static_pipeline/src/meshes.rs#L299), [skeleton import](../../perro_source/build_pipeline/perro_static_pipeline/src/skeletons.rs#L267) + [CLI animation import](../../perro_source/devtools/perro_cli/src/gltf_animation.rs#L108).
- Call `gltf::import` -> load buffers + decode images; callers discard images. Material path discards buffers too + repeats import on each static build.
- Use document-only import 4 material; document/buffer import 4 mesh/skeleton/anim; share by source. Kp external buffer base paths + GLB blobs; separate unused-image validation if required.
- Measure models w/ many 4K images; count decode calls + peak mem; compare generated asset bytes + import errs.

### B02 — skip unchanged archive assembly + reread — P2 / low-med effort

- Trace [old archive load](../../perro_source/io_stack/perro_assets/src/packer.rs#L118), compare reread at line 151, full assembly at line 230 + final write gate at line 271.
- Trig all-source stat hit -> read old archive, copy payloads into full new archive, reread old archive for equality.
- Model size A -> roughly 2A file-read bytes + A payload copy; roughly 3A simultaneously live archive bytes at compare, excluding capacity/index overhead.
- First reuse old bytes for compare; add all-entry/source-set/output-validity fast path b4 assembly. Use stream/temp output 4 large changed archive.
- Kp output integrity + unchanged mtime. Measure 100 MB/1 GB archive: no change/one edit/add/remove; compare exact bytes + mtime.

### B03 — reuse shader-bake GPU context — P2 / med effort

- Trace [GPU device init](../../perro_source/build_pipeline/perro_static_pipeline/src/shader_bake.rs#L79) + [serial bake loop](../../perro_source/build_pipeline/perro_static_pipeline/src/textures.rs#L104).
- Trig N uncached shader materials -> N Instance/adapter/device setups + layout/pipeline builds.
- Create one context/build; cache pipeline by shader/layout; reuse compatible targets/readback buffers. Kp per-job uniforms, validation + device-loss handling.
- Measure 1/10/100 jobs, shared/distinct shaders; count devices/pipelines + total bake time; compare RGBA output.

### B04 — cache unchanged text-resource codegen — P2 / med-high effort

- Trace [scenes](../../perro_source/build_pipeline/perro_static_pipeline/src/scenes.rs#L44), [animations](../../perro_source/build_pipeline/perro_static_pipeline/src/animations.rs#L35), [CSVs](../../perro_source/build_pipeline/perro_static_pipeline/src/csvs.rs#L47).
- Re-read/reparse/re-emit all sources/bundle build; `write_if_changed` skips final write only.
- Cache emitted fragments or gate unchanged resource kinds. Include source-set membership, `.pretarget`, demo flag, DLC prefix, shader-bake refs + codegen version in keys.
- Kp current binary-asset/script-sync caches. Measure no-op/one scene/one retarget edit; byte-compare output w/ clean build.

### B05 — bound cold asset-bake peak mem — P2 / med effort

- Trace [texture output collect](../../perro_source/build_pipeline/perro_static_pipeline/src/textures.rs#L72), [audio collect](../../perro_source/build_pipeline/perro_static_pipeline/src/audios.rs#L54) + [mesh collect](../../perro_source/build_pipeline/perro_static_pipeline/src/meshes.rs#L145).
- Parallel encode retains all result blobs b4 first write -> sum(completed blobs) + live worker decode/encode buffers; concurrent kinds compound peak.
- Write unique blobs in workers + return metadata only, or use bounded queue/chunks. Kp deterministic metadata sort + hash names; publish/prune only aft successful pass.
- Measure large cold texture/audio/model set; peak working set + wall time; compare manifests/blobs + failure recovery.

### B06 — split website docs payload — P2 / med-high effort

- Trace [full DocPage payload + LazyLock](../../perro_website/src/docs.rs#L20) + [JSON emit](../../perro_website/build.rs#L88).
- Embed markdown + HTML + search text for whole corpus; first `docs()` parses all into owned strings. Production views need HTML/search; markdown reads occur in tests.
- Rm markdown from prod payload first; split compact nav/search index + page content; consider static borrowable data. Kp SSR/hydration + full-text search.
- Measure actual compressed WASM size, cold hydrate + heap; chk route/search output. No current-build payload-size claim in this audit.

### B07 — cache/prebake social images — P2/P3 / low-med effort

- Trace [async OG handler](../../perro_website/src/main.rs#L54) -> [sync raster/PNG encode](../../perro_website/src/main.rs#L212).
- Trig repeated `/og/*.png` -> repeat SVG parse + raster + PNG encode on async executor. Alloc 3,024,000 pixel bytes/request (`1200 × 630 × 4`) + tree/PNG storage.
- Prebake known routes or bounded route cache + cache headers; send cold raster to blocking worker. Kp bounded keys + rebuild invalidation.
- Measure repeat/mixed OG requests; req p95 + unrelated endpoint latency + mem at concurrency.

### B08 — skip unchanged demo bundle copies — P3 / low-med effort

- Trace [website demo sync](../../perro_website/build.rs#L231) + [rm/copy dir](../../perro_website/build.rs#L344).
- Trig website build-script rerun w/ local `.output/web` -> unconditional directory replacement/copy, even when demo rebuild gate skips build.
- Diff content manifests; copy changes + rm stale paths only. Kp final tree parity + unchanged mtimes.
- Measure docs-only website build aft local demo export; count bytes copied + wall time.

### C01 — reuse CI compile caches — P2 / low-med effort

- Trace [test jobs](../../.github/workflows/ci.yml#L36), editor/slow-test + separate three-OS Clippy jobs in same workflow.
- Use fresh checkout/toolchain per job; no explicit Cargo registry/target cache step. Repeat dependency fetch/compile across runs + compatible jobs.
- Add Rust build cache by OS/toolchain/lockfile/features/profile; cancel superseded branch/PR runs where safe. Kp separate platform + GPU checks.
- Measure dependency build time + cache transfer cost over cold/warm CI runs; avoid cache keys that mix incompatible outputs.

## Keep current gains + explicit tradeoffs

- Kp [Sep 4 fixes](performance_fixes_2026-09-04.md): root-node creation, trusted-leaf reparent, CSV limits, editor sort/folder/diff, bounded LAN receive, audio load lock release + water blit cache.
- Kp [Sep 6 stages](performance_stage3_2026-09-06.md): lazy retained draw cache, resource-use/count gates, transform/world memos, cached schedules, scoped node writes + signal emit storage.
- Distinguish retained CPU draw update from GPU prepare; sparse CPU cache patches do ! prove sparse GPU prep.
- Kp stage-3 full signal teardown scan as explicit connect/emit/mem tradeoff; no default reverse-index rollback.
- Leave 16 -> 17 signal growth as prior documented regression; ! new finding here.
- Kp [generated release profile](release_build_profile.md): O3 + fat LTO + one codegen unit. Root workspace O2/no-LTO/64-CGU cfg differs; no blanket release-profile recommendation.
- Treat older baseline numbers as history, ! fresh results or projected wins.

## Record coverage + validation limits

| Area | Trace/scan scope | Result |
| --- | --- | --- |
| Render stack | Graphics prep, resource load, UI/2D/3D, water/particles, shadows/cull, postFX, streams, app pacing/input, bridge, meshlets/LOD, mips, WGSL/macros | R01–R08; no equally strong new app-pacing/macro/mip target |
| Runtime/core/scripts | Node arena/query/world/transform, physics sync/query, navmesh, animation, bones, script glue/schedules/signals, Variant; core math/UI/IDs/formats scan | T01–T08; skip unproven SIMD/hash/inline micro-tuning |
| API/audio | Raw/frame TCP, UDP/LAN/WebSocket, HTTP workers, Steam queue/API, jobs, input-map/frame state, resource wrappers, web/storage, module helpers; DSP/playback/MIDI/mic/cache | N01–N03 + A01–A02; kp HTTP agent reuse, bounded queues + current input indexes |
| Editor | App/update path, scene cache/index, viewport/gizmos/pick, file watch/browser, inspector/undo paths | E01–E02; kp recent file-watch/sort fixes |
| Build/I/O/tools | Asset compilers, scene/script codegen, cache/packer/ZIP/static reads, CLI import/scaffold/install + dev runner | B01–B05; no stronger new ZIP-stream/dev-bootstrap target |
| Website/demos/CI | Docs generation/search/render, OG routes, sponsor client, demo sync/scripts, root profiles + workflow | B06–B08 + C01; kp sponsor client reuse + intentional demo FPS/animation work |
| Docs/book/tests/assets | Prior perf reports, architecture/release policy, bench inventory + fixture paths; binary-asset inventory only | Context + workload selection; no full game/asset validity audit |

Use source/caller checks + cross-review for findings; no runtime proof of net speedup.
Validate report links + `git diff --check` after write.

Start each fix w/ counters for claimed repeat work + realistic fixture.
Time b4/aft identical build profile/executable workload; stop concurrent builds during timing.
Record CPU stage time + alloc/live bytes; use GPU timestamps for GPU work.
Report median + tail where relevant; keep throughput, latency + mem tradeoffs visible.
Run affected behavior tests aft source changes; include cache invalidation, no-op edits, resource reload, reentrancy + slot reuse where applicable.
Measure exported game profile as well as workspace bench profile b4 broad engine-speed claims.
