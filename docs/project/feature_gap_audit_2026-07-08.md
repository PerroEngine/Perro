# Feature Gap Audit - 2026-07-08

Scope:

- start from current feature matrix
- ignore rows already `done` unless they hide follow-up work
- plan how to add `partial`, `in dev`, `research`, and missing-adjacent features
- use existing Perro systems first

## Gap List

| Gap | State | Why it matters | Best first ship |
| --- | --- | --- | --- |
| Demo2D parity | done | parity zones shipped in Demo2D | keep smoke/web checks current |
| Mesh blend polish | done | MSAA and multimesh use the screen seam path | keep renderer tests + Demo3D docs current |
| 3D shadow controls | done | shadow tuning fields and guide exist | keep Demo3D tuning lane on backlog |
| Navmesh | partial | text `.pnav`, resource API, and runtime path query exist | static bake, node, binary format, and Demo3D lane |
| Auto retarget | partial | `.pretarget` alias maps and CLI import remap exist | rest-pose bake, static pipeline, and humanoid solve |
| 2D shadows | partial | `cast_shadows` drives hard 2D shadows from visible `CollisionShape2D` casters | soft shadows, sprite/tilemap casters, Demo2D lane |
| Editor release | in dev | editor exists but not release-grade | smoke, docs, save/load tests, inspector coverage |
| Joint polish | planned | current joints cover core use, not tuning-heavy rigs | optional limits/motors/springs only after demo need |
| Docs parity | partial | features exist but docs hide them or split them | camera stream, decals, editor paths |
| Test/smoke coverage | partial | several features need user-path proof, not just unit tests | demo smoke + screenshot/perf baselines |

## Priority

1. Navmesh static bake + Demo3D lane
2. Auto retarget rest-pose/static bake
3. 2D shadow polish
4. Editor release pass
5. Joint polish
6. Docs parity
7. Test/smoke coverage

Reason:

- ship visible gaps first
- fix existing partial features before large new systems
- add new core systems only after demo/docs surface is honest

## 1. Demo2D Parity

Goal:

- make Demo2D mirror major runtime paths
- avoid new engine work

Shipped:

- 2D particle stress lane
- positional audio lane
- docs rows + controls

Still needed:

- web sync after assets added

Use:

- `ParticleEmitter2D`
- `.ppart`
- `Audio2D`
- `AudioMask2D`
- `AudioEffectZone2D`
- `AudioPortal2D` if small demo fits
- existing Demo2D hub + pause scripts

Impl:

1. add `res/particles/*.ppart`
2. add particle stress zone in main Demo2D scene
3. add `res/scripts/positional_audio_2d_demo.rs`
4. add audio source, listener/camera, mask wall, effect zone
5. add debug draw toggle if already exposed
6. upd `demos/Demo2D/docs/README.md`
7. build web target + sync website demo files

Done:

- hub button reaches both zones
- profiler overlay shows cost
- particle lane uses 4 mixed `ParticleEmitter2D` profiles
- positional-audio lane uses attached MIDI speakers, `AudioMask2D`, `AudioEffectZone2D`, debug rays

Follow-up:

- web build runs
- website demo files synced

## 2. Mesh Blend Polish

Goal:

- same blend behavior for single mesh, multimesh, and MSAA

State:

- done in `feature/mesh-blend-polish`
- screen-space seam pass works for `MeshInstance3D`
- MSAA resolves to the single-sample scene target before seam
- `MultiMeshInstance3D` writes stable batch ids into the seam mask
- legacy depth fade stays as compatibility fallback when the seam path is disabled

Use:

- current blend mask pass
- current seam pass
- current `blend_layers` / `blend_mask` fields
- current Demo3D mesh blend scene

Impl:

1. done: move blend ID/depth mask to single-sample target when MSAA > 1
2. done: resolve scene color before seam pass
3. done: assign stable participant IDs for multimesh batches
4. done: write multimesh batch IDs into blend mask path
5. done: keep legacy fade as fallback cfg for low-end path
6. done: add renderer tests for mask IDs and pass routing
7. todo: add screenshot captures for MSAA off/on and multimesh

Done:

- mesh and multimesh seam output match
- MSAA no longer changes feature class
- Demo3D docs list fallback only as cfg/compat path

## 3. 3D Shadow Controls

Goal:

- expose shadow quality knobs already implied by renderer
- avoid shadow model rewrite

State:

- ray/spot/point shadows exist
- cascades, slots, culling, and multimesh shadow casters exist
- fields: light `cast_shadows`, mesh `cast_shadows`, mesh `receive_shadows`
- fields: `shadow_strength`, `shadow_depth_bias`, `shadow_normal_bias`
- nested scene form: `shadow = { strength = 0.82 depth_bias = 0.00018 normal_bias = 0.045 }`
- docs: `docs/resources/shadows3d.md`
- limitation: current shader uses one frame-wide tuning triplet picked from the first active shadow caster

Use:

- `three_d/gpu/shadows.rs`
- shadow map array target
- existing light nodes
- `project.toml` graphics cfg pattern

Impl:

1. done: add per-light strength, depth-bias, and normal-bias fields
2. done: thread fields through scene parser -> render bridge -> GPU setup
3. done: add docs page `docs/resources/shadows3d.md`
4. done: add unit tests for scene parse, runtime emit, and GPU uniform tuning
5. todo: add Demo3D shadow tuning lane or extend mesh materials lane
6. todo: add project cfg fields: map size, cascade count, max spot, max point

Done:

- user can fix acne/peter-pan artifacts without code edits
- docs show cost and default values

## 4. Navmesh MVP

Goal:

- let scripts ask for a walk path
- keep pathfinding static and cheap

MVP:

- static baked navmesh only
- 3D only first
- no dynamic obstacle carve
- no crowd sim
- no agent avoidance

Use:

- glTF mesh load path
- trimesh extraction logic
- `BitMask` layers
- resource ID/cache model
- `Draw2D/3D` debug lines if useful

Current packet:

- done: `NavMeshID`
- done: `.pnav` text parser
- done: `ctx.res.NavMeshes()` load/create/write/drop API
- done: `ctx.run.NavMesh()` static 3D path query API
- done: triangle shared-edge A*
- done: layer mask, same-poly, corridor, unreachable tests
- todo: binary `.pnav`
- todo: static pipeline bake
- todo: `NavMesh3D` node + scene fields
- todo: Demo3D lane
- todo: real funnel/string-pull smoothing

New:

- `NavMeshID`
- `.pnav` asset format
- `ctx.res.NavMeshes()` load API
- `ctx.run.NavMesh()` query API

Impl:

1. add `NavMeshID` in ids crate
2. define `.pnav` text + binary static bake format
3. write navmesh data structs in core/asset formats
4. add static pipeline bake from mesh or scene-marked geometry
5. add runtime loader/cache
6. add polygon graph A*
7. add funnel/string-pull smoothing
8. expose `find_path(nav, start, end, opts) -> Vec<Vector3>`
9. add Demo3D nav lane with click-to-path or scripted agents

Later:

- `NavAgent3D`
- dynamic obstacle rebuild tiles
- off-mesh links
- area costs
- 2D navmesh or grid nav

Done:

- path query works on baked mesh
- no runtime bake required
- tests cover unreachable, same-poly, corridor turn, layer mask

## 5. Automatic Retargeting

Goal:

- reuse animation across mismatched but similar rigs

MVP:

- offline bake first
- exact-name map + manual alias map
- no runtime retarget solve first
- no IK foot lock first

Use:

- glTF skeleton import
- `.panim`
- `Skeleton3D`
- `AnimationPlayer`
- CLI tooling path

New:

- `.pretarg` retarget map
- CLI command: `perro_cli retarget`
- optional editor view later

Impl:

1. export source/target rest poses
2. build bone map by exact names
3. allow alias file for mismatch names
4. compute source-rest -> target-rest conversion
5. bake target-local keyframes into new `.panim`
6. add report: missing bones, scale mismatch, twist risk
7. add demo with two rigs sharing one source clip

Later:

- runtime retarget cache
- additive retarget
- foot lock
- humanoid profile

Done:

- baked target clip plays with no per-frame retarget cost
- missing bone report is useful
- original shared-skeleton path stays unchanged

## 6. 2D Shadowed Lights

Goal:

- make 2D `cast_shadows` meaningful

Current packet:

- done: `cast_shadows` reaches `RayLight2DState`, `PointLight2DState`, and `SpotLight2DState`
- done: visible `CollisionShape2D` nodes emit `ShadowCaster2DState`
- done: GPU light shader reads caster storage buffer
- done: point and spot lights hard-mask pixels blocked by quad/circle/triangle casters
- done: ray lights use directional hard shadows in virtual 2D space
- todo: body layer/mask filtering
- todo: sprite alpha silhouettes
- todo: tilemap collision casters
- todo: soft penumbra
- todo: Demo2D shadow lane

Use:

- `CollisionShape2D`
- 2D light extraction
- `BitMask`
- current additive 2D light pass

Impl:

1. done: add bridge shadow flags + caster state
2. done: retain shadow casters in 2D renderer
3. done: upload caster storage buffer beside the 2D light pass
4. done: mask light fragments with segment-vs-caster tests
5. done: add runtime + renderer tests
6. todo: filter blockers by body layer/mask + camera
7. todo: add Demo2D shadow lane

Later:

- soft penumbra
- normal maps for 2D sprites
- cached static blocker meshes

Done:

- `cast_shadows` no longer no-ops for ray/point/spot 2D lights
- collision shape blockers update through retained render state
- shader validation covers the new storage-buffer light mask

## 7. Editor Release Pass

Goal:

- make `perro_editor` safe enough to recommend as in-dev tool

Current:

- manager/editor scenes exist
- asset browser/watch scripts exist
- scene nav, viewport, gizmos, inspector, animation scripts exist

Missing:

- run docs
- known limits doc
- smoke scripts
- save/load regression tests
- new field coverage in inspector

Impl:

1. add `perro_editor/README.md` with run flow
2. add known limits section
3. smoke `cargo run -p perro_cli -- dev --path perro_editor`
4. add `SceneDocs` load/save roundtrip tests for edited nodes
5. add asset watch failure cases
6. audit inspector rows vs node registry fields
7. add missing inspector rows for decals, camera streams, shadow fields

Done:

- editor opens project manager
- scene save/load has regression tests
- docs state current limits plainly

## 8. Joint Polish

Goal:

- add tuning knobs only where gameplay needs them

Current:

- core 2D/3D joints exist
- prior packet says optional limits/motors/springs if needed

Impl:

1. inspect Rapier support per joint type
2. add limit fields where native support is direct
3. add motor fields only for hinge/prismatic-like use
4. add spring/damping fields after demo need
5. add physics demo lane before broad API growth

Done:

- no fake fields
- every exposed knob maps to solver behavior
- docs show units and defaults

## 9. Docs Parity

Goal:

- make existing features findable

Needed:

- standalone decals page or move Demo3D decal docs into docs tree
- camera streams page
- UI widgets summary update
- 2D light note: shadows not implemented
- mesh blend page with limits

Impl:

1. add docs pages after feature polish, not before
2. link from docs index
3. link from feature matrix
4. add scene examples only after tests/demos exist

Done:

- matrix row links to feature doc or demo doc
- no stale "not implemented" text for implemented features

## 10. Test And Smoke Coverage

Goal:

- prove feature paths from user-facing scenes

Needed:

- Demo2D smoke build
- Demo3D smoke build
- web export smoke
- screenshot/perf baseline for mesh blend + shadows
- live Steam AppID 480 smoke behind env flag
- navmesh path golden tests
- retarget bake golden tests

Impl:

1. add CLI smoke scripts under docs/tooling or CI
2. keep live Steam opt-in: `PERRO_STEAMWORKS_LIVE_TESTS`
3. use golden asset fixtures for nav/retarget
4. keep renderer image tests opt-in if GPU needed

Done:

- normal `cargo test` stays fast
- feature smoke can run before release

## Not Gaps

These do not need implementation plan, only docs/tests polish:

- Steamworks native path
- Decal3D/TextDecal3D
- Demo3D hub
- Demo2D hub shell
- camera streams
- UI dropdown/checkbox/color picker/shape/text block
- blend shapes

## Matrix Drift Appendix

Keep this only as proof that matrix changed:

| Item | b4 | aft |
| --- | --- | --- |
| Steamworks | partial | done |
| Decals | research | done |
| Demo hubs | planned | done |
| Mesh blending | in dev | partial |
| Camera streams | missing | done |
| UI widgets | missing | done |
| Editor | missing | in dev |
| Demo2D parity | missing | done |
| 3D shadows | partial | done |
| Retargeting | research | research |
| Navmesh | research | research |
