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
| 3D shadow controls | partial | shadows work but tuning is hidden | cfg + per-light bias/quality + docs |
| Navmesh | research | most game-blocking missing gameplay feature | static baked navmesh + path query API |
| Auto retarget | research | skinned asset reuse limited to exact rig contract | offline retarget bake tool |
| 2D shadows | planned | `cast_shadows` fields exist but do nothing in 2D | hard-shadow mask from 2D colliders |
| Editor release | in dev | editor exists but not release-grade | smoke, docs, save/load tests, inspector coverage |
| Joint polish | planned | current joints cover core use, not tuning-heavy rigs | optional limits/motors/springs only after demo need |
| Docs parity | partial | features exist but docs hide them or split them | camera stream, decals, shadow guide |
| Test/smoke coverage | partial | several features need user-path proof, not just unit tests | demo smoke + screenshot/perf baselines |

## Priority

1. 3D shadow controls + docs
2. Navmesh MVP
3. Auto retarget bake
4. 2D shadowed lights
5. Editor release pass
6. Joint polish
7. Docs parity
8. Test/smoke coverage

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

Current:

- ray/spot/point shadows exist
- cascades, slots, culling, and multimesh shadow casters exist
- fields: light `cast_shadows`, mesh `cast_shadows`, mesh `receive_shadows`
- missing: user quality/bias knobs + dedicated docs

Use:

- `three_d/gpu/shadows.rs`
- shadow map array target
- existing light nodes
- `project.toml` graphics cfg pattern

Impl:

1. add global `ShadowSettings3D`
2. add project cfg fields: map size, cascade count, max spot, max point
3. add per-light fields: bias, normal_bias, shadow_range/quality
4. thread fields through scene parser -> render bridge -> GPU setup
5. add docs page `docs/resources/shadows3d.md`
6. add Demo3D shadow tuning lane or extend mesh materials lane
7. add perf notes: ray cascades vs point cubemap cost

Done:

- user can fix acne/peter-pan artifacts without code edits
- docs show cost and default values
- demo shows receiver/caster toggles

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

New:

- `NavMeshID`
- `.pnav` asset format
- `ctx.res.NavMeshes()` load API
- `ctx.run.Navigation()` query API

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

MVP:

- hard shadows
- static/dynamic 2D collision blockers
- point + spot first
- ray light after

Use:

- `CollisionShape2D`
- 2D light extraction
- `BitMask`
- current additive 2D light pass

Impl:

1. collect blocker edges from 2D collision shapes
2. filter blockers by layer/mask + camera
3. extrude silhouette quads from light origin
4. render shadow mask into light target
5. multiply/subtract mask during light draw
6. add spot cone clipping
7. add ray-light directional extrusion
8. add Demo2D shadow lane

Later:

- soft penumbra
- normal maps for 2D sprites
- cached static blocker meshes

Done:

- point/spot shadows visible
- moving blockers update
- cost stable with many sprites

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

- `docs/resources/shadows3d.md`
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
| 3D shadows | partial | partial |
| Retargeting | research | research |
| Navmesh | research | research |
