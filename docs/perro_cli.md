# Perro CLI

This document covers Perro CLI in flag-first style:

- `--scripts`
- `--dev`
- `--project`
- `--format`

## Quick Map

Preferred usage:

```powershell
perro_cli --path <project_dir> --scripts
perro_cli --path <project_dir> --dev
perro_cli --path <project_dir> --project
perro_cli --path <project_dir> --format
```

`--path` defaults to the current working directory when omitted.

Note: command aliases (`build`, `dev`, `project`, `format`) still exist, but flags are the primary workflow.

## `--scripts`

Command:

```powershell
perro_cli --scripts --path <project_dir>
```

What it does:

1. Syncs every `*.rs` file from `<project_dir>/res/**` into `<project_dir>/.perro/scripts/src` as generated `*.gen.rs`.
2. Regenerates the scripts registry in `.perro/scripts/src/lib.rs`.
3. Builds the scripts crate in release mode (`cargo build --release`) at `<project_dir>/.perro/scripts`.

Use this when you only need script compilation/update.

## `--dev`

Command:

```powershell
perro_cli --dev --path <project_dir>
```

What it does:

1. Runs the same scripts build pipeline as `--scripts`.
2. Builds `perro_dev_runner` in release mode from the workspace root.
3. Launches the dev runner binary with your `--path`.

Use this for local development runs.

## `--project`

Command:

```powershell
perro_cli --project --path <project_dir>
```

What it does:

1. Runs script compilation (same core script pipeline as `--scripts`).
2. Generates static scene/material/particle/mesh/texture outputs.
3. Generates embedded project entry files under `.perro/project`.
4. Packs `res` assets into `.perro/project/embedded/assets.perro`.
5. Builds the generated project crate in release mode from `.perro/project`.
6. Copies the built executable to `<project>/.output/` for clean, predictable exports.

Use this for full static project bundle generation and build.

## `--format`

Command:

```powershell
perro_cli --format --path <project_dir>
```

What it does:

1. Resolves your path to that project's `res` root.
2. Recursively finds all `*.rs` files under `res/**`.
3. Runs `rustfmt` on those files.

