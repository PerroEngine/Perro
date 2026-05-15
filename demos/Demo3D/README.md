# Demo3D

Welcome to your Perro project. This README is a quick map of how things fit together.

Run `perro check` to sync scripts and get rust-analyzer working.

## Project Layout
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
- `perro format` runs rustfmt for all `.rs` scripts under `res/`.
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
    script = "res://scripts/player.rs"
    [Node2D]
            position = (0, 0)
    [/Node2D]
[/Player]
```
- Use `res://` paths to reference files in res/
- Use `user://` when you want user data, either to read or write. On Windows this resolves to:
  `C:\Users\<You>\AppData\Local\<ProjectName>\data\...`
- You cannot write to res in release

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
