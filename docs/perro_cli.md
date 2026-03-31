# Perro CLI

This document covers Perro CLI in command-first style. Commands are shown using `perro`, assuming you ran `perro_cli install` and restarted PowerShell, or installed from crates.io when available.

- `check`
- `dev`
- `build`
- `format`
- `clean`
- `new`
- `new_script`
- `new_scene`
- `new_animation`
- `install`

## Quick Map

Preferred usage:

```powershell
perro check [--path <project_dir>]
perro dev [--path <project_dir>]
perro build [--path <project_dir>]
perro format [--path <project_dir>]
perro clean [--path <project_dir>]
perro new [--path <parent_dir>] [--name <project_name>]
perro new_script --name <script_name> [--path <project_dir>] [--res <res_subdir>] [--no-open]
perro new_scene --name <scene_name> [--path <project_dir>] [--res <res_subdir>] [--template 2D|3D] [--no-open]
perro new_animation --name <animation_name> [--path <project_dir>] [--res <res_subdir>] [--no-open]
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
2. Regenerates the scripts registry in `.perro/scripts/src/lib.rs`.
3. Builds the scripts crate in release mode (`cargo build --release`) at `<project_dir>/.perro/scripts`.

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

1. Adds/updates a `perro` PowerShell function in your profile.
2. That function runs source-mode CLI from your local repo via `cargo run -p perro_cli -- ...`.

After running install, open a new PowerShell and use:

```powershell
perro new --path D:\GameProjects --name MyGame
perro check --path D:\GameProjects\MyGame
```

## `new_script`

Command:

```powershell
perro new_script --name <script_name> [--path <project_dir>] [--res <res_subdir>] [--no-open]
```

What it does:

1. Resolves `<project_dir>` (defaults to current working directory, walking up to find `project.toml`).
2. Resolves `<res_subdir>` relative to the project `res` root.
3. Creates a new `*.rs` script using the empty script template.
4. Opens the new file in VS Code (disable with `--no-open`).

Notes:

- `--res` accepts `res://` or `/`-style paths, for example `res://scripts` or `/scripts`.
- `--name` can be passed without `.rs`; the extension is added automatically.

Examples:

```powershell
perro new_script --name PlayerController
perro new_script --name PlayerController --res /scripts
perro new_script --name PlayerController --path D:\GameProjects\MyGame --res res://scripts
perro new_script --name PlayerController --no-open
```

## `new_scene`

Command:

```powershell
perro new_scene --name <scene_name> [--path <project_dir>] [--res <res_subdir>] [--template 2D|3D] [--no-open]
```

What it does:

1. Resolves `<project_dir>` (defaults to current working directory, walking up to find `project.toml`).
2. Resolves `<res_subdir>` relative to the project `res` root.
3. Creates a new `*.scn` scene using the selected template.
4. Opens the new file in VS Code (disable with `--no-open`).

Notes:

- `--template` defaults to `2D`.
- `--res` accepts `res://` or `/`-style paths, for example `res://scenes` or `/scenes`.
- `--name` can be passed without `.scn`; the extension is added automatically.
- `--name` must be a file name only (no path separators).

Examples:

```powershell
perro new_scene --name Main
perro new_scene --name Main3D --template 3D
perro new_scene --name Main --res /scenes
perro new_scene --name Main --path D:\GameProjects\MyGame --res res://scenes --template 2D
perro new_scene --name Main --no-open
```

## `new_animation`

Command:

```powershell
perro new_animation --name <animation_name> [--path <project_dir>] [--res <res_subdir>] [--no-open]
```

What it does:

1. Resolves `<project_dir>` (defaults to current working directory, walking up to find `project.toml`).
2. Resolves `<res_subdir>` relative to the project `res` root.
3. Creates a new `*.panim` animation clip using the default animation template.
4. Opens the new file in VS Code (disable with `--no-open`).

Notes:

- Defaults to `res/animations` when `--res` is omitted.
- `--res` accepts `res://` or `/`-style paths, for example `res://animations` or `/animations`.
- `--name` can be passed without `.panim`; the extension is added automatically.
- `--name` must be a file name only (no path separators).

Examples:

```powershell
perro new_animation --name CubeMove
perro new_animation --name HeroRun --res /animations
perro new_animation --name HeroRun --path D:\GameProjects\MyGame --res res://animations
perro new_animation --name HeroRun --no-open
```
