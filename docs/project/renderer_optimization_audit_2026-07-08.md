# Renderer Optimization Audit - 2026-07-08

Scope:

- `perro_source/render_stack/perro_graphics`
- `perro_source/render_stack/perro_render_bridge`
- `perro_source/runtime_project/perro_runtime/src/runtime/render`
- audit only
- no src chg

## Summary

renderer retained-mode wins already in place

Best next wins:

1. cache camera stream extract output
2. skip 2D point-light upload when light rev same
3. target material texture bind-group invalidation
4. make shadow dirty per layer/light
5. expose renderer counters to profiling overlay/CSV

Theme:

- CPU extract + cache churn likely next bottleneck
- GPU pass count guarded in many spots
- profile b4/aft each opt

## Current Wins

| Area | Win | Evidence |
| --- | --- | --- |
| 3D retained prepare | skip full stage when draw rev + scene same | `three_d/gpu/prepare.rs:112`, `three_d/gpu/prepare.rs:180` |
| 3D transform patch | patch instance/model spans only | `three_d/gpu/prepare.rs:250`, `three_d/gpu/prepare.rs:285`, `three_d/gpu/prepare.rs:352` |
| 3D cull input patch | rewrite dirty cull rows only | `three_d/gpu/prepare.rs:400`, `three_d/gpu/prepare.rs:432` |
| 3D batch sort | skip sorted path, parallel sort big sets | `three_d/gpu/prepare.rs:1542`, `three_d/gpu/prepare.rs:1555` |
| 3D indirect draw | coalesce adjacent indirect draws | `three_d/gpu/render_pass.rs:3`, `three_d/gpu/render_pass.rs:599` |
| 3D shadows | skip valid shadow layers | `three_d/gpu/render_pass.rs:115`, `three_d/gpu/shadows.rs:60` |
| 2D sprites | stage/sort only on sprite rev | `two_d/gpu.rs:421`, `two_d/gpu.rs:505` |
| 2D rects | dirty range upload path | `two_d/renderer.rs:626`, `two_d/renderer.rs:659` |

## Opt List

| Rank | Target | Cost | Risk | Gain |
| --- | --- | --- | --- | --- |
| 1 | camera stream retained cache | med | med | high CPU + alloc cut |
| 2 | 2D point-light upload gate | low | low | small/med CPU + bus cut |
| 3 | material texture bind-group invalidation | low/med | med | stutter cut on texture churn |
| 4 | shadow dirty granularity | med/high | med | high for moving caster scenes |
| 5 | renderer perf counters export | low | low | better opt proof |
| 6 | draw signature hash | med | med | large-scene CPU cut |
| 7 | multimesh cull pass audit | med | high | GPU cut in dense scenes |
| 8 | mesh blend pass audit | high | high | GPU cut in blend-heavy scenes |

## 1. Camera Stream Retained Cache

Issue:

- scan arena per stream
- builds fresh `Arc<[...]>` outputs
- clones surfaces + dense poses
- repeats when stream source unchanged

Evidence:

- build node scratch from full arena: `runtime/render/bridge.rs:533`
- clone post effects: `runtime/render/bridge.rs:538`
- allocate stream arrays: `runtime/render/bridge.rs:554`, `runtime/render/bridge.rs:568`
- 2D stream collects into `Vec` then `Arc`: `runtime/render/bridge.rs:608`, `runtime/render/bridge.rs:728`
- 3D stream clones surfaces: `runtime/render/bridge.rs:1120`, `runtime/render/bridge.rs:1147`
- 3D stream rebuilds dense poses: `runtime/render/bridge.rs:1167`
- 3D stream returns fresh `Arc`: `runtime/render/bridge.rs:1254`

Plan:

1. add `CameraStreamExtractCache`
2. key by stream node + source cam node + render mask
3. include source revs:
   - arena mutation rev
   - 2D sprite/light/water rev
   - 3D draw/light/decal/water/particle rev
   - resource rev when used
4. return cached `Arc` slices when key same
5. reuse main 3D dense pose cache for stream multimesh
6. add cache miss counters

Done:

- static stream frame N+1 alloc near zero
- dense multimesh stream uses same pose `Arc`
- camera move only updates camera uniform where possible
- tests cover visibility/layer/resource invalidation

## 2. 2D Point-Light Upload Gate

Issue:

- 2D GPU prepare restages point lights every frame
- renderer already tracks point-light revision

Evidence:

- retained light rev exists: `two_d/renderer.rs:514`
- GPU clears/reserves lights each prepare: `two_d/gpu.rs:554`
- GPU writes point-light buffer each prepare: `two_d/gpu.rs:562`

Plan:

1. add `point_lights_revision` to `Prepare2D`
2. add `last_point_light_stage: Option<u64>` to `Gpu2D`
3. upload point lights only when rev differs
4. keep camera uniform update separate
5. reset on buffer resize

Done:

- camera pan with static lights writes camera buffer only
- light move/add/rm updates light buffer
- unit test covers rev gate

## 3. Targeted Material Texture Bind-Group Invalidation

Issue:

- any material texture slot change clears all material texture bind groups
- one streaming/external texture -> unrelated material combo churn

Evidence:

- miss/remove clears all groups: `three_d/gpu/buffers.rs:263`
- pending resource clears all groups: `three_d/gpu/buffers.rs:280`
- load fail clears all groups: `three_d/gpu/buffers.rs:286`
- slot upload clears all groups: `three_d/gpu/buffers.rs:302`
- source upload clears all groups: `three_d/gpu/buffers.rs:347`
- external texture clears all groups: `three_d/gpu/buffers.rs:363`

Plan:

1. track `MaterialTextureKey -> slots`
2. track `slot -> SmallVec<MaterialTextureKey>`
3. on slot change, remove only keys using that slot
4. keep full clear for sampler/filter/fallback layout change
5. add counter: bind-group evict count

Done:

- update one custom image slot keeps unrelated bind groups
- texture swap test checks old key gone + unrelated key kept

## 4. Shadow Dirty Granularity

Issue:

- any caster transform/full rebuild marks all shadow casters dirty
- shadow cache then rerenders all shadow layers

Evidence:

- transform patch marks global dirty: `three_d/gpu/prepare.rs:524`
- full rebuild marks global dirty: `three_d/gpu/prepare.rs:1747`
- shadow update reads single dirty flag: `three_d/gpu/shadows.rs:60`
- layer render decision uses global dirty: `three_d/gpu/shadows.rs:78`, `three_d/gpu/shadows.rs:99`
- render pass skips only when layer already valid: `three_d/gpu/render_pass.rs:115`

Plan:

1. track dirty caster bounds on transform patch
2. map caster render layers to affected lights
3. mark only affected shadow layers invalid
4. keep global dirty fallback for topology/material/shadow flag chg
5. add debug counter: shadow layers rendered/skipped

Done:

- one moving caster near one point light rerenders only that cubemap
- moving non-caster keeps shadow cache valid
- tests cover ray/spot/point layer invalidation

## 5. Renderer Perf Counters Export

Issue:

- key renderer counters exist but stay mostly internal
- opt work lacks easy per-frame proof in demos

Evidence:

- 3D prepare timing stored: `three_d/gpu.rs:1021`, `three_d/gpu/buffers.rs:109`
- 3D render counters exist: `three_d/gpu.rs:1063`
- 3D render counts pipeline/texture switches: `three_d/gpu/render_pass.rs:579`
- 2D sprite perf counters exist: `two_d/gpu.rs:105`
- CLI profile tooling exists: `docs/tools/perro_cli.md:46`, `docs/tools/perro_cli.md:598`

Plan:

1. expose renderer perf snapshot from `PerroGraphics`
2. add runtime profiling fields:
   - 2D sprite batches
   - 3D draw batches
   - pipeline switches
   - texture bind switches
   - shadow layers rendered/skipped
   - prepare skip counts
3. add CSV columns under `--csv-profile`
4. add demo overlay compact row

Done:

- perf overlay shows renderer hot path state
- CSV captures b4/aft for this audit list

## 6. Draw Signature Hash

Issue:

- transform-only path still compares draw pairs
- deep compare fallback costs on huge scenes
- dense multimesh fast path relies on `Arc::ptr_eq`, good when producer reuses `Arc`

Evidence:

- prepare classifies whole scene: `three_d/gpu/prepare.rs:118`
- classify loops all draws: `three_d/gpu/draw.rs:1140`
- dense instances use ptr eq then deep compare: `three_d/gpu/draw.rs:1095`
- main runtime caches dense pose `Arc`: `runtime/render/three_d.rs:1049`

Plan:

1. add stable draw signature at runtime retained draw build
2. include mesh/surface/material/lod/blend/shadow flags
3. exclude model rows for transform-only compare
4. compare signature first
5. keep deep compare debug assert/fallback

Done:

- transform-only classify uses O(1) compare per draw
- dense multimesh no deep compare on unchanged poses
- tests keep material/topology changes as full rebuild

## 7. Multimesh Cull Pass Audit

Issue:

- multimesh cull runs before depth prepass
- hi-z path may run another multimesh cull after pyramid
- counters clear each pass

Evidence:

- first multimesh cull pass: `three_d/gpu/render_pass.rs:224`
- second hi-z multimesh cull pass: `three_d/gpu/render_pass.rs:457`
- counter clear per pass: `three_d/gpu/render_pass.rs:226`, `three_d/gpu/render_pass.rs:460`

Plan:

1. measure dense scenes with cull off/frustum/hi-z
2. skip first pass when hi-z path fully replaces result for main pass
3. keep first pass when depth prepass needs same visible set
4. avoid second clear when visible buffer safe to refine

Done:

- no mismatch between prepass + main visible instances
- dense static scene lowers compute dispatch time
- screenshots match across cull modes

## 8. Mesh Blend Pass Audit

Issue:

- mesh blend forces extra depth/mask/blend work
- high risk area due depth, MSAA, multimesh feature gaps

Evidence:

- blend forces depth prepass path: `three_d/gpu/render_pass.rs:68`
- blend mask after depth prepass: `three_d/gpu/render_pass.rs:345`
- blend-only passes draw source batches again: `three_d/gpu/render_pass.rs:703`, `three_d/gpu/render_pass.rs:797`
- feature gap already tracks mesh blend polish: `docs/project/feature_gap_audit_2026-07-08.md:35`

Plan:

1. finish correctness first:
   - MSAA seam path
   - multimesh IDs
2. add perf counters per blend pass
3. test pass fusion only after correctness stable
4. keep fallback cfg for low-end path

Done:

- same output MSAA on/off
- multimesh blend matches single mesh
- blend-heavy demo shows b4/aft GPU timings

## Bench Plan

Use:

- Demo3D multimesh scene
- Demo3D mesh blend scene
- Demo3D lights/shadows scene
- Demo2D sprite stress scene
- camera stream scene with static source + moving main cam

Metrics:

- frame ms
- CPU extract ms
- 2D sprite batches
- 3D draw batches
- pipeline switches
- texture bind switches
- GPU buffer write bytes
- shadow layers rendered/skipped
- camera stream cache hit/miss

Cmd:

```powershell
cargo run -p perro_cli -- dev --path demos/Demo3D --release --csv-profile renderer_audit
cargo run -p perro_cli -- dev --path demos/Demo2D --release --csv-profile renderer_audit
```

## Priority

1. perf counters export
2. 2D point-light upload gate
3. material texture bind-group invalidation
4. camera stream retained cache
5. draw signature hash
6. shadow dirty granularity
7. multimesh cull pass audit
8. mesh blend pass audit

Reason:

- add proof first
- ship low-risk gates early
- defer pass rewrites until counters + correctness stable

## Not First

Pipeline prewarm:

- resize/pipeline setup not main per-frame path
- custom shader pipeline cache already exists

Full renderer rewrite:

- retained-mode base already strong
- focused cache gates give better risk/reward

GPU occlusion readback:

- debug readback gated
- avoid tuning before real trace shows cost
