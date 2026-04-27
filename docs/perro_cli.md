# Perro CLI

This document covers Perro CLI in command-first style. Commands are shown using `perro`, assuming you ran `perro_cli install` and reloaded your shell profile, or installed from crates.io when available.

- `check`
- `dev`
- `mem-profile`
- `build`
- `flamegraph`
- `format`
- `clean`
- `new`
- `new_dlc`
- `new_script`
- `new_scene`
- `new_animation`
- `install`

## Quick Map

Preferred usage:

```powershell
perro check [--path <project_dir>]
perro dev [--path <project_dir>]
perro mem-profile [--path <project_dir>] [--release] [--csv [csv_name]]
perro build [--path <project_dir>]
perro flamegraph [--path <project_dir>] [--profile] [--root]
perro format [--path <project_dir>]
perro clean [--path <project_dir>]
perro new [--path <parent_dir>] [--name <project_name>]
perro new_dlc --name <dlc_name> [--path <project_dir>] [--no-open]
perro new_script --name <script_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>] [--no-open]
perro new_scene --name <scene_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>] [--template 2D|3D] [--no-open]
perro new_animation --name <animation_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>] [--no-open]
perro install
```

`--path` defaults to the current working directory when omitted.

## Project Placement

Recommended workflow:

1. Put temporary test/sandbox projects under `playground/` in this repo to be shared.
2. Put real game/application projects outside this monorepo (for example `D:\GameProjects\MyGame`) and open those project folders directly in VS Code.

Why:

1. External projects are cleaner to work with because project-local `.vscode/settings.json` is the active workspace config when you open that folder directly.
2. Internal `playground/*` projects can still be edited from the monorepo root, but they depend on repo-root `.vscode/settings.json` rust-analyzer wiring.
3. Running `perro_cli check/dev/build --path <internal_project>` now refreshes root workspace wiring for all detected `playground/*` projects (including existing ones), so linked-project drift is reduced.

## `check`

Command:

```powershell
perro check --path <project_dir>
```

What it does:

1. Syncs every `*.rs` file from `<project_dir>/res/**` into `<project_dir>/.perro/scripts/src` as generated `*.gen.rs`.
2. Regenerates module exports in `.perro/scripts/src/lib.rs` for all synced Rust files.
3. Regenerates runtime scripts registry in `.perro/scripts/src/lib.rs` for behavior scripts (files that export script constructor).
4. Builds the scripts crate in release mode (`cargo build --release`) at `<project_dir>/.perro/scripts`.

Use this when you only need script compilation/update.

## `new`

Command:

```powershell
perro new [--path <parent_dir>] [--name <project_name>]
```

What it does:

1. Creates a new project directory under `<parent_dir>` (defaults to current working directory).
2. Writes default project files (`project.toml`, `deps.toml`, `res/main.scn`, scripts scaffold, and `.perro` crates).
3. Prompts to open the project in VS Code.

Notes:

- If you run this inside a directory you want to contain projects, you can omit `--path`.
- Add extra script Rust crates in `deps.toml` under `[dependencies]`; Perro merges them into `.perro/scripts/Cargo.toml` on `check`, `dev`, and `build`.

Examples:

```powershell
perro new --path D:\GameProjects --name MyGame
perro new --name MyGame
```

## `new_dlc`

Command:

```powershell
perro new_dlc --name <dlc_name> [--path <project_dir>] [--no-open]
```

What it does:

1. Resolves `<project_dir>` (defaults to current working directory, walking up to find `project.toml`).
2. Creates `<project_dir>/dlcs/<dlc_name>/`.
3. Creates starter directories:
   - `scenes/`
   - `scripts/`
   - `materials/`
   - `meshes/`
4. Creates starter files:
   - `scenes/main.scn`
   - `scripts/script.rs`
5. Starter scene uses `dlc://<dlc_name>/scripts/script.rs`.

Name rules:

- `self` is reserved for `dlc://self/...` and is rejected as a DLC name (case-insensitive).

Examples:

```powershell
perro new_dlc --name CosmeticsPack
perro new_dlc --name CosmeticsPack --path D:\GameProjects\MyGame
```

## `dlc`

Command:

```powershell
perro dlc --name <dlc_name> [--path <project_dir>]
```

What it does:

1. Reads source from `<project_dir>/dlcs/<dlc_name>/`.
2. Generates DLC scripts crate under `.perro/dlc/<dlc_name>/scripts/`.
3. Generates DLC pack crate under `.perro/dlc/<dlc_name>/pack/`.
4. Builds both runtime-loadable modules.
5. Packs manifest + scripts module + pack module + DLC resources into:
   - `<project_dir>/.output/dlc/<dlc_name>.dlc`

Name rules:

- `self` is reserved for `dlc://self/...` and is rejected as a DLC name (case-insensitive).

## `dev`

Command:

```powershell
perro dev --path <project_dir>
```

What it does:

1. Runs the same scripts build pipeline as `check`.
2. Builds the project-local dev runner at `<project_dir>/.perro/dev_runner` in release mode.
3. Launches the generated dev runner binary with your `--path`.

Use this for local development runs.

## `build`

Command:

```powershell
perro build --path <project_dir>
```

What it does:

1. Runs script compilation (same core script pipeline as `check`).
2. Generates static scene/material/particle/mesh/texture outputs.
3. Generates embedded project entry files under `.perro/project`.
4. Packs `res` assets into `.perro/project/embedded/assets.perro`.
5. Builds the generated project crate in release mode from `.perro/project`.
6. Copies the built executable to `<project>/.output/` for clean, predictable exports.

Use this for full static project bundle generation and build.

## `mem-profile`

Command:

```powershell
perro mem-profile --path <project_dir> [--release] [--csv [csv_name]]
```

What it does:

1. Runs the same scripts build pipeline as `check`.
2. Builds the project-local dev runner with `profile` feature enabled.
3. Launches dev runner with memory profiling enabled (`PERRO_MEM_PROFILE=1`).
4. Writes batch memory samples CSV in `<project_dir>/.output/profiling/` (default file: `memory_profile.csv`).

Flags:

- `--release`: builds and runs release dev runner binary.
- `--csv [csv_name]`: custom output file name under `.output/profiling/`.

## `flamegraph`

Command:

```powershell
perro flamegraph --path <project_dir> [--profile] [--root]
```

What it does:

1. Runs the same scripts build pipeline as `check`.
2. Checks `cargo flamegraph` availability; auto-runs `cargo install flamegraph` when missing.
3. Runs `cargo flamegraph --release` from `<project_dir>/.perro/dev_runner`.
4. Sets `CARGO_TARGET_DIR=<project_dir>/target` so profiler build output stays project-local.
5. Forces debug symbols for release profiling (`CARGO_PROFILE_RELEASE_DEBUG=true`).
6. Passes project path through to dev runner (`-- --path <project_dir>`).

Flags:

- `--profile`: enables dev runner `profile` feature when building/profiling.
- `--root`: forwards `--root` to `cargo flamegraph` (useful on Linux when elevated perf access is required).

Notes:

- `perro flamegraph` auto-installs `cargo-flamegraph` when missing.
- Linux: install `perf` (`linux-tools` package family).
- macOS: install `dtrace`/Xcode command line tools.
- Windows: CLI asks to relaunch elevated (UAC) before flamegraph when shell lacks admin rights.
- Windows: `cargo-flamegraph` uses `blondie` and often needs elevated PowerShell/Terminal.
- Windows: if error includes `NotAnAdmin`, rerun as Administrator.
- Windows fallback: prefer WSL/Linux profiling for full flamegraph support.

Example:

```powershell
perro flamegraph --path D:\GameProjects\MyGame
perro flamegraph --path D:\GameProjects\MyGame --profile
```

## `format`

Command:

```powershell
perro format --path <project_dir>
```

What it does:

1. Resolves your path to that project's `res` root.
2. Recursively finds all `*.rs` files under `res/**`.
3. Runs `rustfmt` on those files.

## `clean`

Command:

```powershell
perro clean [--path <project_dir>]
```

What it does:

1. Removes the project's `target/` directory (defaults to current project).

## `install`

Command:

```powershell
perro install
```

What it does:

1. Adds/updates a `perro` shell function in your profile.
2. On Windows, updates PowerShell profiles.
3. On Linux, updates POSIX shell profiles (`~/.profile`, `~/.bashrc`, `~/.zshrc`).
4. Function runs source-mode CLI from your local repo via `cargo run -p perro_cli -- ...`.

After running install, open a new shell (or source your updated profile) and use:

```powershell
perro new --path D:\GameProjects --name MyGame
perro check --path D:\GameProjects\MyGame
```

## `new_script`

Command:

```powershell
perro new_script --name <script_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>] [--no-open]
```

What it does:

1. Resolves `<project_dir>` (defaults to current working directory, walking up to find `project.toml`).
2. Resolves target root:
   - default: project `res/`
   - with `--dlc <name>`: project `dlcs/<name>/`
3. Resolves `<res_subdir>` relative to that selected root.
3. Creates a new `*.rs` script using the empty script template.
4. Opens the new file in VS Code (disable with `--no-open`).

Notes:

- Base game mode:
  - `--res` accepts `res://` or `/`-style paths, for example `res://scripts` or `/scripts`.
- DLC mode (`--dlc <name>`):
  - `--res` accepts `dlc://<name>/...` or `/`-style paths.
  - Example: `--res dlc://ExpansionOne/scripts` or `--res /scripts`.
- `--name` can be passed without `.rs`; the extension is added automatically.

Examples:

```powershell
perro new_script --name PlayerController
perro new_script --name PlayerController --res /scripts
perro new_script --name PlayerController --path D:\GameProjects\MyGame --res res://scripts
perro new_script --name DlcController --path D:\GameProjects\MyGame --dlc ExpansionOne --res /scripts
perro new_script --name DlcController --path D:\GameProjects\MyGame --dlc ExpansionOne --res dlc://ExpansionOne/scripts
perro new_script --name PlayerController --no-open
```

## `new_scene`

Command:

```powershell
perro new_scene --name <scene_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>] [--template 2D|3D] [--no-open]
```

What it does:

1. Resolves `<project_dir>` (defaults to current working directory, walking up to find `project.toml`).
2. Resolves target root:
   - default: project `res/`
   - with `--dlc <name>`: project `dlcs/<name>/`
3. Resolves `<res_subdir>` relative to that selected root.
3. Creates a new `*.scn` scene using the selected template.
4. Opens the new file in VS Code (disable with `--no-open`).

Notes:

- `--template` defaults to `2D`.
- Base game mode:
  - `--res` accepts `res://` or `/`-style paths, for example `res://scenes` or `/scenes`.
- DLC mode (`--dlc <name>`):
  - `--res` accepts `dlc://<name>/...` or `/`-style paths.
- `--name` can be passed without `.scn`; the extension is added automatically.
- `--name` must be a file name only (no path separators).

Examples:

```powershell
perro new_scene --name Main
perro new_scene --name Main3D --template 3D
perro new_scene --name Main --res /scenes
perro new_scene --name Main --path D:\GameProjects\MyGame --res res://scenes --template 2D
perro new_scene --name DlcIntro --path D:\GameProjects\MyGame --dlc ExpansionOne --res /scenes
perro new_scene --name Main --no-open
```

## `new_animation`

Command:

```powershell
perro new_animation --name <animation_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>] [--no-open]
```

What it does:

1. Resolves `<project_dir>` (defaults to current working directory, walking up to find `project.toml`).
2. Resolves target root:
   - default: project `res/`
   - with `--dlc <name>`: project `dlcs/<name>/`
3. Resolves `<res_subdir>` relative to that selected root.
3. Creates a new `*.panim` animation clip using the default animation template.
4. Opens the new file in VS Code (disable with `--no-open`).

Notes:

- Defaults to `res/animations` when `--res` is omitted.
- Base game mode:
  - `--res` accepts `res://` or `/`-style paths, for example `res://animations` or `/animations`.
- DLC mode (`--dlc <name>`):
  - `--res` accepts `dlc://<name>/...` or `/`-style paths.
- `--name` can be passed without `.panim`; the extension is added automatically.
- `--name` must be a file name only (no path separators).

Examples:

```powershell
perro new_animation --name CubeMove
perro new_animation --name HeroRun --res /animations
perro new_animation --name HeroRun --path D:\GameProjects\MyGame --res res://animations
perro new_animation --name DlcIdle --path D:\GameProjects\MyGame --dlc ExpansionOne --res /animations
perro new_animation --name HeroRun --no-open
```
