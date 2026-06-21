# Install + Tools

Install Perro CLI and check the local toolchain before you create a project.

## Goal

Get a working Perro command flow:

```powershell
cargo run -p perro_cli -- --help
cargo run -p perro_cli -- doctor --path D:\GameProjects\MyGame
```

## Toolchain

Perro uses Rust crates, project-local scripts, and asset build steps.

Install:

- Rust stable toolchain
- platform graphics drivers
- target platform SDKs when shipping beyond desktop
- `wasm32-unknown-unknown` when building web

```powershell
rustup target add wasm32-unknown-unknown
```

## CLI Flow

Use the workspace CLI while developing Perro itself:

```powershell
cargo run -p perro_cli -- new --name MyGame --path D:\GameProjects
cargo run -p perro_cli -- check --path D:\GameProjects\MyGame
cargo run -p perro_cli -- dev --path D:\GameProjects\MyGame
```

Installed CLI flow:

```powershell
perro new --name MyGame --path D:\GameProjects
perro check --path D:\GameProjects\MyGame
perro dev --path D:\GameProjects\MyGame
```

## Project Layout

Common files:

- `project.toml`: game config
- `deps.toml`: script dependencies
- `res/`: scenes, scripts, textures, audio, data
- `.perro/`: generated script/build glue
- `.output/`: build output

Do not hand-edit `.perro/`.

Change source files and let CLI rebuild glue.

## Check Early

Run check after adding scripts, scenes, or deps:

```powershell
perro check --path D:\GameProjects\MyGame
```

Check catches:

- script compile errors
- stale resource refs
- project config issues
- common scene warnings

## Reference

- [Perro CLI](/docs/tools/perro_cli.md)
- [`project.toml`](/docs/project/project_toml.md)
- [WASM / Web Target](/docs/WASM.md)
