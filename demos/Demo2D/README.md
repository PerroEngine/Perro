# Demo2D

2D stress + feature map for Perro.

Use this demo to compare feature cost and to read complete scene-to-script
wiring. Each lane keeps authored nodes in the scene, fixed dependencies in
state, and changing sets behind tags/queries.

Run:

```text
cargo run -p perro_cli -- dev --path demos\Demo2D
```

## Runtime Flow

- hub menu like `Demo3D`
- click lane -> jump 2 mirror zone
- `Esc` -> pause
- restart/hub btns like `Demo3D`

The manager owns navigation because scene changes, fade, pause, and restart are
one cross-lane lifecycle. Each lane owns only its feature behavior. This avoids
copying menu logic into every stress zone.

Parity map:

- `Mesh + Materials` -> static sprite atlas stress
- `Lights + Shadows` -> point/spot/ray lights + moving shadow casters
- `Animations` -> animated sprites + `AnimationPlayer`
- `Physics Bones` -> `Skeleton2D` + `PhysicsBoneChain2D`
- `Physics Collisions` -> rigid-body stacks
- `MultiMesh` analog -> dense sprite batches
- `Particles` -> 4 `ParticleEmitter2D` stress emitters
- `Positional Audio` -> 3 MIDI speakers + masks + effect zones

No-analog note:

- `Sky` and `Mesh Blending` stay 3D-only lanes

Controls:

- `WASD` / arrows => pan
- mouse wheel => zoom
- `R` => rebuild stress sections
- `T` => toggle audio debug rays in positional-audio lane

## Feature Map

| Need | Read | Why |
| --- | --- | --- |
| scene refs + lane navigation | [`res/main.scn`](res/main.scn) + [`demo2d_manager.rs`](res/scripts/demo2d_manager.rs) | fixed UI/camera refs come from scene data; manager owns cross-lane flow |
| own-node camera mutation | [`camera_pan_2d.rs`](res/scripts/camera_pan_2d.rs) | `ctx.id` targets the attached camera without lookup |
| scene-known UI refs | [`InfoOverlay.scn`](res/Menu/InfoOverlay.scn) + [`demo_info_overlay.rs`](res/scripts/demo_info_overlay.rs) | scene injects stable targets; script copies state before node access |
| optional device output | [`webcam.scn`](res/scenes/webcam.scn) + [`webcam_demo_2d.rs`](res/scripts/webcam_demo_2d.rs) | state refs describe output nodes; unavailable capture stays optional |

See [`docs/README.md`](docs/README.md) for load counts and comparison method.

Script std: [`../../docs/scripting/authoring/index.md`](../../docs/scripting/authoring/index.md)

Demo patterns:

- scene-known refs -> typed `NodeID` state + `script_vars`
- spawned sets -> tags + `query!`
- own node -> `ctx.id`
- delayed/repeat work -> named timers + finish signals
- global demo assets -> const `res://` paths; per-instance assets -> typed state IDs

## Tradeoffs

- stress counts favor comparison, not production defaults
- queries fit rebuilt/spawned sets; fixed lane controls stay injected refs
- one manager simplifies demo navigation; separate feature scripts keep lane state local
- continuous camera motion uses frame updates; delayed work uses named timers
