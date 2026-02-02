# Engine overview: how Perro works

This document describes the **engine-side architecture**: why we transpile, how dev vs release differ, how assets and project fit together, and how the `.perro` folder and `project.toml` drive the build.

## Why transpile?

Perro does **not** run PUP, TypeScript, or C# in a VM or runtime. All of them are **transpiled to Rust** and then built by the normal Rust toolchain. That gives you:

- **Compile-time guarantees** — Type checking, borrow checking, and optimizations happen at compile time. Your script logic becomes real Rust code; there is no interpreter or JIT.
- **No VM overhead** — No separate runtime for scripts. No crossing a VM boundary on every call. Script functions are just Rust functions the engine calls directly.
- **Direct interop** — The engine exposes Rust APIs; the generated code calls them with normal function calls. No marshalling layer between “script world” and “engine world.”
- **Single binary in release** — In release builds, script code is statically linked into the game binary. No DLLs, no script files to ship.

So: **we transpile to leverage Rust’s compile-time model and to avoid a VM**, while still letting you write in a higher-level language (PUP, or experimentally TypeScript/C#).

## How it works in theory

1. You write scripts in PUP (or TS/C#). The **transpiler** turns them into Rust and writes that Rust under `.perro/scripts/src`.
2. That Rust is compiled by `cargo` — either as a **DLL** (dev) or as part of the **project binary** (release).
3. The **Perro runtime** loads either the DLL (dev) or the statically linked code (release) and calls into it via a fixed trait/ABI. From the engine’s point of view, it’s always calling Rust.

So the “script” layer is really **generated Rust** that the engine treats as native code.

## Dev vs release

### Dev mode

- The **scripts crate** (`.perro/scripts/`) is built as a **cdylib** (DLL / dynamic library).
- The **Perro runtime** (e.g. `perro_dev`) loads that DLL at startup and uses it for all script logic.
- Assets (scenes, UI, images) are typically read from disk (e.g. `res/`) so you can iterate without a full re-export.
- `project.toml` is read from the project folder at runtime.
- Updating script sources recompiles in about 3–5 seconds; only changing engine core takes longer.

So in dev: **scripts = DLL, runtime loads DLL, assets and config from disk.**

### Release / export

- The **project crate** (`.perro/project/`) is the actual game binary. It **depends on** the scripts crate and on `perro_core`.
- Script code is **statically linked** into the executable — no DLL. Same Rust, just compiled as part of the project binary.
- **Assets are compiled into the binary:**
  - **UI and scenes** — `.scn` and `.fur` are turned into **runtime data structures** at compile time and emitted into the project crate’s `static_assets` (e.g. `scenes.rs`, `fur.rs`). At runtime, UI and scenes already exist in their final form; no parsing of `.scn`/`.fur` in release.
  - **Images** — Converted to **.ptex** (Perro texture format: Zstd-compressed RGBA, etc.) and placed under `.perro/project/embedded_assets/`. The build then embeds these into the binary (e.g. via `static_assets::textures`). So in release, images are not loaded from PNG/JPG on disk; they’re already in the binary as textures.
  - **3D meshes** — You import **models** (`.gltf`/`.glb`); each model can have multiple meshes. Use **`res://model.glb`** if the file has one mesh; if it has multiple, use **`res://model.glb:0`**, **`res://model.glb:1`**, etc. (index; do not rely on internal mesh names, they vary by exporter). Baked data is **.pmesh** (one per mesh, Zstd-compressed) in `embedded_assets/`. In dev, meshes are loaded from disk; in release they're in the binary.
  - **Project manifest** — `project.toml` is compiled into a static manifest (e.g. `static_assets::manifest`) so the release binary doesn’t need a `project.toml` file on disk.
- Optionally a **BRK** (Binary Resource pacK) is built from `res/` for any remaining assets not covered under static embedding; scripting sources, scene source, and preprocessed images are **not** put in the BRK because they’re already in the binary or in `embedded_assets`.

So in release: **one executable, scripts and static assets (scenes, UI, textures, manifest) compiled in; no VM, no script/config files required at runtime.**

## The `.perro` folder and project layout

Inside a game project you have:

- **`project.toml`** — Project settings: display name, main scene, icon, version, graphics (virtual resolution, MSAA), performance (fps_cap, xps), input action mappings, etc. This is the **source of truth** for project metadata. In release, this is read at **compile time** and baked into the project crate (e.g. manifest, and optionally the project/crate name).
- **`.perro/scripts/`** — The **scripts crate**. PUP (and TS/C#) are transpiled to Rust here. In dev this crate is built as a DLL; in release it is a dependency of the project crate.
- **`.perro/project/`** — The **project crate**. This is the actual game binary in release. It depends on `scripts` and `perro_core`, and contains:
  - Generated `main.rs`, `build.rs`, and `Cargo.toml`
  - `src/static_assets/` — Generated Rust that embeds scenes, UI (FUR), textures (.ptex), meshes (.pmesh), and the project manifest
  - `embedded_assets/` — Preprocessed assets in subfolders: `textures/` (.ptex), `meshes/` (.pmesh), embedded by the build

So: **dev** = runtime + scripts DLL + assets from disk; **release** = project crate (binary) that statically links scripts and embeds static assets, with optional BRK for the rest.

## Crate name and `project.toml`

The **display name** in `project.toml` (`[project] name = "My Game"`) can be human-readable. The **Rust crate name** for the project (used in `.perro/project/Cargo.toml` as `[package] name` and `[[bin]] name`) must be a valid Rust identifier. When you run **release** build, the compiler can **read `project.toml` and sync** the project crate name: it derives a crate name from the project name (e.g. lowercase, spaces → underscores) and updates `.perro/project/Cargo.toml` so the built binary matches your project name. That way you can rename the game in `project.toml` and get a consistent executable name without manually editing Cargo.toml.

## Summary

| Aspect            | Dev                                      | Release                                                                 |
|-------------------|------------------------------------------|-------------------------------------------------------------------------|
| Scripts           | Scripts crate → DLL; runtime loads DLL    | Scripts crate linked into project binary                                |
| Scenes / UI       | Loaded from disk (e.g. `res/`)           | Compiled into `static_assets` (scenes.rs, fur.rs) — runtime form only   |
| Images            | Loaded from disk                         | Converted to .ptex, stored in `embedded_assets`, embedded in binary     |
| 3D meshes         | Loaded from disk (GLTF/GLB; path or path:index) | Converted to .pmesh per mesh, stored in `embedded_assets`, embedded in binary |
| project.toml      | Read from project folder at runtime      | Baked into `static_assets::manifest` at compile time                     |
| Binary            | Engine binary + scripts DLL              | Single project binary (scripts + static assets inside)                  |

The pipeline is designed to **use Rust’s compile-time model** (no VM, direct calls, static linking) and to **move as much as possible into the binary in release** (scenes, UI, textures, manifest) so the shipped game is self-contained and fast.
