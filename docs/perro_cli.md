# Perro CLI

This document covers Perro CLI in command-first style:

- `check`
- `dev`
- `build`
- `format`
- `install`

## Quick Map

Preferred usage:

```powershell
perro_cli check [--path <project_dir>]
perro_cli dev [--path <project_dir>]
perro_cli build [--path <project_dir>]
perro_cli format [--path <project_dir>]
perro_cli install
```

`--path` defaults to the current working directory when omitted.

## Project Placement

Recommended workflow:

1. Put temporary test/sandbox projects under `playground/` in this repo.
2. Put real game/application projects outside this monorepo (for example `D:\GameProjects\MyGame`) and open those project folders directly in VS Code.

Why:

1. External projects are cleaner to work with because project-local `.vscode/settings.json` is the active workspace config when you open that folder directly.
2. Internal `playground/*` projects can still be edited from the monorepo root, but they depend on repo-root `.vscode/settings.json` rust-analyzer wiring.
3. Running `perro_cli check/dev/build --path <internal_project>` now refreshes root workspace wiring for all detected `playground/*` projects (including existing ones), so linked-project drift is reduced.

## `check`

Command:

```powershell
perro_cli check --path <project_dir>
```

What it does:

1. Syncs every `*.rs` file from `<project_dir>/res/**` into `<project_dir>/.perro/scripts/src` as generated `*.gen.rs`.
2. Regenerates the scripts registry in `.perro/scripts/src/lib.rs`.
3. Builds the scripts crate in release mode (`cargo build --release`) at `<project_dir>/.perro/scripts`.

Use this when you only need script compilation/update.

## `dev`

Command:

```powershell
perro_cli dev --path <project_dir>
```

What it does:

1. Runs the same scripts build pipeline as `check`.
2. Builds the project-local dev runner at `<project_dir>/.perro/dev_runner` in release mode.
3. Launches the generated dev runner binary with your `--path`.

Use this for local development runs.

## `build`

Command:

```powershell
perro_cli build --path <project_dir>
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
perro_cli format --path <project_dir>
```

What it does:

1. Resolves your path to that project's `res` root.
2. Recursively finds all `*.rs` files under `res/**`.
3. Runs `rustfmt` on those files.

## `install`

Command:

```powershell
perro_cli install
```

What it does:

1. Adds/updates a `perro` PowerShell function in your profile.
2. That function runs source-mode CLI from your local repo via `cargo run -p perro_cli -- ...`.

After running install, open a new PowerShell and use:

```powershell
perro new --path D:\GameProjects --name MyGame
perro check --path D:\GameProjects\MyGame
```
