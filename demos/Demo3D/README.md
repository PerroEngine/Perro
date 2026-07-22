# Demo3D

3D feature lab + complete scene/script ownership map.

Use the hub to isolate rendering, physics, animation, and audio features. Read a
lane's scene for authored data and its script for runtime decisions.

Run `perro check` to sync scripts and get rust-analyzer working.

## Project Layout

The project owns source files under `res/`. Perro owns generated files under
`.perro/`. This boundary matters because every sync may replace generated glue.

- `project.toml` is the project config (main scene, icon, graphics defaults).
- `deps.toml` is optional. Add `[dependencies]` here for extra Rust crates used by scripts.
- `res/` holds your assets, scripts, and scenes. `res://` paths resolve into this folder.
- `res/main.scn` is the default scene because `project.toml` points to it by default.
- `.perro/` contains generated Rust crates (project, scripts, dev runner). You generally don’t touch these.
  - `project/` is the static project crate produced by `perro build`. It bakes assets and links scripts into the final executable.
  - `scripts/` is generated from any `.rs` file under `res/` plus Perro’s internal glue. It gets overwritten on build, so don’t edit it directly.
  - `dev_runner/` is built and run by `perro dev`. It loads the scripts dynamic library in dev mode.
  - Output from `perro build` goes to `.output/` for convenience so you do not have to dig through `target/`.

## Common Commands
- `perro new` creates a project (you just ran this).
- `perro dev` builds scripts and runs the dev runner.
- `perro check` builds scripts only.
- `perro build` builds the full static bundle.
- `perro format` formats `.rs`, `.scn`, `.pmat`, and related text resources under `res/`.
- `perro new_script` creates a new script template in `res/` (use `--res` for subfolders).
- `perro new_scene` creates a new scene template in `res/` (use `--res` and `--template 2D|3D`).
- `perro new_animation` creates a new `.panim` animation clip template (defaults to `res/animations`).
- If you run these inside the project root, you do not need `--path`.

## Scenes And Scripts
- Scenes are `.scn` files under `res/`.
- Script files are Rust files under `res/` (any `.rs` file under `res/`).
- You attach scripts to nodes in scenes using a `script` field with a `res://` path.
- Example:
```text
[Player]
    script = "res://scripts/demo_manager.rs"
    [Node3D]
        position = (0, 0, 0)
    [/Node3D]
[/Player]
```
- Use `res://` paths to reference files in res/
- Use `user://` when you want user data, either to read or write. On Windows this resolves to:
  `C:\Users\<You>\AppData\Local\<ProjectName>\data\...`
- You cannot write to res in release

Scenes own topology, fixed `NodeID` refs, and per-instance asset choices.
Scripts own lifecycle and mutable behavior. Tags/queries remain for sets created
or replaced at runtime. This makes dependencies visible in the scene and avoids
repeated name lookup in update hooks.

## Demo3D Hub
- `res/main.scn` loads `DemoManager`.
- `DemoManager` owns hub, pause menu, transition fade, demo load, restart, and hub return.
- Demo scenes live in `res/scenes/demos/`.
- Demo cameras use shared `res://scripts/demo_freecam_3d.rs`.
- Demo docs start at `docs/README.md`.

## Demo Table

| Demo | Scene | Docs |
| --- | --- | --- |
| Mesh + Materials | `res://scenes/demos/mesh_materials.scn` | `docs/mesh_materials.md` |
| Lights | `res://scenes/demos/lights.scn` | `docs/lights.md` |
| Water | `res://scenes/demos/water.scn` | `docs/water.md` |
| Animations | `res://scenes/demos/animations.scn` | `docs/animations.md` |
| Sky | `res://scenes/demos/sky.scn` | `docs/sky.md` |
| Mesh Blending | `res://scenes/demos/mesh_blending.scn` | `docs/mesh_blending.md` |
| MultiMesh | `res://scenes/demos/multimesh.scn` | `docs/multimesh.md` |
| Particles | `res://scenes/demos/particles.scn` | `docs/particles.md` |
| Positional Audio | `res://scenes/demos/positional_audio.scn` | `docs/positional_audio.md` |
| Physics Bones | `res://scenes/demos/physics_bones.scn` | `docs/physics_bones.md` |
| Physics Collisions | `res://scenes/demos/physics_collisions.scn` | `docs/physics_collisions.md` |
| Decals | `res://scenes/demos/decals.scn` | `docs/decals.md` |

## Demo Controls

| Input | Action |
| --- | --- |
| Mouse | Look in demo scenes. |
| `W` `A` `S` `D` | Move camera. |
| `Space` | Move up. |
| `Shift` | Move down. |
| Mouse wheel | Change free camera speed. |
| `Esc` | Pause/resume or return through menu. |
| Left mouse | Shoot in Water and Physics Bones. |
| Mouse wheel | Change projectile size in shooting demos. |
| `R` | Toggle audio debug rays in Positional Audio. |

## Documentation
The comprehensive docs live in the main Perro repository on GitHub: `https://github.com/PerroEngine/Perro/blob/main/docs/index.md`

Script std: [`../../docs/scripting/authoring/index.md`](../../docs/scripting/authoring/index.md)

Demo patterns:

- scene-known refs -> typed `NodeID` state + `script_vars`
- loaded/spawned sets -> structural access or `query!`
- own node -> `ctx.id`
- cross-scene events -> signals; known targets -> methods
- delayed/repeat work -> named timers + finish signals

## Feature Flows

| Flow | Scene + script | Why this shape |
| --- | --- | --- |
| hub -> demo load -> fade | [`res/main.scn`](res/main.scn) + [`demo_manager.rs`](res/scripts/demo_manager.rs) | one manager owns cross-scene navigation; target refs come from scene state |
| camera input -> node motion | demo scene + [`demo_freecam_3d.rs`](res/scripts/demo_freecam_3d.rs) | camera mutates its own typed node through `ctx.id` |
| collision -> reset | [`physics_collisions.scn`](res/scenes/demos/physics_collisions.scn) + [`physics_collisions_demo.rs`](res/scripts/physics_collisions_demo.rs) | collision is an event; named timer owns delayed reset |
| audio control -> spatial nodes | [`positional_audio.scn`](res/scenes/demos/positional_audio.scn) + [`positional_audio_demo.rs`](res/scripts/positional_audio_demo.rs) | fixed emitters stay scene refs; runtime script controls playback/debug state |
| manager -> camera command | [`demo_manager.rs`](res/scripts/demo_manager.rs) + [`demo_freecam_3d.rs`](res/scripts/demo_freecam_3d.rs) | method targets one known receiver; no dynamic var/name probe |

## Tradeoffs

- split scenes keep feature cost and config readable; a game may group features by level or gameplay ownership instead
- shared camera removes repeated control code; feature-specific scripts stay on roots that own their lifecycle
- hub preloading favors fast demo switches; a large game may stream scenes
- stress values show behavior under load, not recommended shipping defaults
