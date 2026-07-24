# Perro CLI

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| End-to-End Example | [End-to-End Example](#end-to-end-example) |
| Quick Map | [Quick Map](#quick-map) |
| Project Placement | [Project Placement](#project-placement) |
| Build And Run | [Build And Run](#build-and-run) |
| New Projects And Templates | [New Projects And Templates](#new-projects-and-templates) |
| Health And Maintenance | [Health And Maintenance](#health-and-maintenance) |
| Profiling | [Profiling](#profiling) |
| Install | [Install](#install) |

## Purpose

`perro` is the one command you run at every stage of a project: create it, compile scripts, run a live dev loop, cook a release build, package DLC, import animations, and profile hot code. `perro dev` loads assets straight from disk for fast edit-run cycles, while `perro build` bakes assets through the static pipeline for release. The CLI wraps all compiler and setup glue, so your project folder stays plain files with no import database to babysit.

Commands use `perro`, assuming you ran `perro_cli install` and reloaded your shell profile, or installed from crates.io when available. `--path` defaults to the current working directory when omitted.

## Use Cases

- **Start a new game.** `perro new --name MyGame` scaffolds `project.toml`, `input_map.toml`, `deps.toml`, a `res/main.scn`, and the `.perro` crates.
- **Fast edit-run loop.** `perro dev` compiles scripts, builds a project-local dev runner, and runs the game reading assets live from disk, so scene and script edits show up quickly.
- **Add content without hand-writing boilerplate.** `perro new_script`, `perro new_scene`, `perro new_animation`, and `perro new_panimtree` drop templated files into `res/` (or a DLC) and rebuild.
- **Cook a shippable build.** `perro build` bakes supported assets and links a release executable into `.output/`; `perro build --target web` and `perro build --target android` export browser and Android bundles.
- **Package optional or paid content.** `perro dlc --name <name>` builds one runtime-loadable `.output/dlc/<name>.dlc` from `dlcs/<name>/`.
- **Import animation and keep the project healthy.** `perro import_anim` converts glTF/GLB clips to `.panim`; `perro doctor`, `clippy`, `format`, and `test` check refs and script quality; `perro bench`, `perro flamegraph`, and `perro mem-profile` profile hot paths.

## Command Choice

Use `check` for the shortest script/scene feedback loop, `doctor` for project
wiring and missing refs, `dev` for behavior, and `build` for shipped/static
behavior. Run `clippy` and `test` after structural checks pass. A successful
`dev` run does not replace a release build check because asset and linking paths
differ.

Generated `.perro` output belongs to the CLI. Fix source under `res/`, config,
or engine crates; do not patch generated glue as a durable solution.

## End-to-End Example

```powershell
# 1. Install the `perro` shell command, then open a new shell.
perro_cli install

# 2. Scaffold a new project next to your other games.
perro new --path D:\GameProjects --name MyGame

# 3. Add a behavior script and a 3D scene.
perro new_script --path D:\GameProjects\MyGame --name PlayerController --res /scripts
perro new_scene  --path D:\GameProjects\MyGame --name Main --template 3D --res /scenes

# 4. Run the live dev loop with timing overlays while you edit.
perro dev --path D:\GameProjects\MyGame --timings

# 5. Cook the release executable into .output/.
perro build --path D:\GameProjects\MyGame
```

## Quick Map

Build and run:

```powershell
perro check [--path <project_dir>]
perro test [--path <project_dir>] [-- <cargo_test_args>]
perro dev [--path <project_dir>] [--target native|web|android] [--headless] [--timings] [--profile] [--ui-profile] [--release] [--csv-profile [csv_name]] [--host <addr>] [--port <num>]
perro build [--path <project_dir>] [--target native|web|android] [--triple <rust_target> | --universal-macos] [--headless] [--profile] [--console]
perro targets [--host windows|linux|macos]
perro dlc --name <dlc_name> [--path <project_dir>]
```

New projects and templates:

```powershell
perro new [--path <parent_dir>] [--name <project_name>]
perro new_dlc --name <dlc_name> [--path <project_dir>] [--no-open]
perro new_script --name <script_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>] [--no-open]
perro new_scene --name <scene_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>] [--template 2D|3D] [--no-open]
perro new_animation --name <animation_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>] [--no-open]
perro new_panimtree --name <tree_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>] [--no-open]
perro import_anim <model.glb|model.gltf> --output <clip.panim> [--clip <name|index>] [--fps <fps>] [--skeleton <object_name>] [--retarget-map <map.pretarget>] [--target-rig <model.glb|model.gltf>]
```

Health and maintenance:

```powershell
perro doctor [--path <project_dir>]
perro test [--path <project_dir>] [-- <cargo_test_args>]
perro format [--path <project_dir>]
perro clippy [--path <project_dir>]
perro clean [--path <project_dir>]
```

Profiling:

```powershell
perro bench [--path <project_dir>] [--script <hash>] [--method <name>] [--var <name>] [-- <criterion_args>]
perro mem-profile [--path <project_dir>] [--release] [--csv [csv_name]]
perro spec [--path <project_dir>] [--target-fps <fps>]
perro flamegraph [--path <project_dir>] [--profile] [--root]
```

Install:

```powershell
perro install
```

## Project Placement

Recommended workflow:

1. Use shipped sample projects under `demos/` for repo examples.
2. Put temporary test/sandbox projects outside this monorepo, for example `D:\GameProjects\MyGame`.
3. Open external project folders directly in VS Code.

Why:

1. External projects keep project-local `.vscode/settings.json` active.
2. `demos/Demo2D` and `demos/Demo3D` stay as known-good sample projects.
3. `perro check`, `perro dev`, and `perro build` work with any project passed by `--path`.

## Build And Run

Use these commands for normal compile, run, export, and DLC package workflows.

| Command | Main job | Output |
| --- | --- | --- |
| `check` | Compile project scripts only. | `.perro/scripts` build output |
| `test` | Sync project scripts and run their Rust tests. | `cargo test` result |
| `dev` | Compile scripts, build dev runner, run project. | running dev app |
| `build` | Compile scripts, bake static assets, build release project. | `.output/` executable + packed assets |
| `targets` | Show ready, setup-required, and unavailable build targets for a development OS. | support matrix |
| `dlc` | Build one runtime-loadable DLC package. | `.output/dlc/<name>.dlc` |

### `check`

Command:

```powershell
perro check --path <project_dir>
```

What it does:

1. Syncs every `*.rs` file from `<project_dir>/res/**` into `<project_dir>/.perro/scripts/src` as generated `*.gen.rs`.
2. Regenerates module exports in `.perro/scripts/src/lib.rs` for all synced Rust files.
3. Regenerates runtime scripts registry in `.perro/scripts/src/lib.rs` for behavior scripts.
4. Builds the scripts crate at `<project_dir>/.perro/scripts`.

Use this when you only need script compilation/update.

### `test`

Command:

```powershell
perro test --path <project_dir> [-- <cargo_test_args>]
```

What it does:

1. Syncs every `*.rs` file from `<project_dir>/res/**` into `<project_dir>/.perro/scripts/src` as generated `*.gen.rs`.
2. Regenerates module exports and the runtime scripts registry in `.perro/scripts/src/lib.rs`.
3. Refreshes source overrides in `.perro/scripts/Cargo.toml`.
4. Runs `cargo test` from `<project_dir>/.perro/scripts`.
5. Sets `CARGO_TARGET_DIR=<project_dir>/target` so script tests share the project build cache.
6. Enables the generated scripts crate `steamworks` feature when project Steam support is enabled.

Flags:

- `-- <cargo_test_args>`: forwards remaining args to `cargo test`.

Examples:

```powershell
perro test --path D:\GameProjects\MyGame
perro test --path D:\GameProjects\MyGame -- --lib -- --nocapture
perro test --path D:\GameProjects\MyGame -- player_state_tests
```

### `dev`

Command:

```powershell
perro dev --path <project_dir> [--target native|web|android] [--headless] [--demo] [--timings] [--profile] [--ui-profile] [--release] [--csv-profile [csv_name]] [--host <addr>] [--port <num>]
```

What it does:

1. Runs the same scripts build pipeline as `check`.
2. With `--target native` or no `--target`, builds the project-local dev runner at `<project_dir>/.perro/dev_runner`.
3. With `--target native`, launches the generated dev runner binary with your `--path`.
4. With `--target web`, builds a wasm web bundle from `.perro/project`.
5. With `--target web`, starts a built-in static server and opens your browser.

Flags:

- `--target native|web|android`: selects native runner, browser wasm bundle, or Android app target. Default `native`.
- `--headless`: runs the native `perro_headless` dev path with no window, input, or GPU render loop. Native only; rejected with `--target web` or `--target android`, and cannot combine with `--timings` or `--ui-profile`.
- `--demo`: applies `[demo]` config overrides, skips excluded scripts/assets/scenes, strips tagged node trees, and enables `demo_exclude!`.
- `--timings`: prints lightweight native timing averages: sim, gfx, delta, fps.
- `--profile`: enables profiling feature for the selected dev target.
- `--ui-profile`: enables native dev runner `ui_profile` feature.
- `--release`: builds release dev target.
- `--csv-profile [csv_name]`: writes native dev profile metrics CSV under `.output/profiling/`.
- `--host <addr>`: web target only. Static server bind host. Default `127.0.0.1`.
- `--port <num>`: web target only. Static server bind port. Default `8000`.

Android target notes:

- `--timings`, `--ui-profile`, and `--csv-profile` are not supported with `perro dev --target android` yet.
- Android dev builds require an installed Android SDK/NDK and a running emulator or device.

Web target notes:

- `--ui-profile` is not supported with `perro dev --target web` yet.
- `--timings` is not supported with `perro dev --target web` yet.
- `--csv-profile` is not supported with `perro dev --target web` yet.
- web output dir: `<project_dir>/.output/web-dev/`
- web path uses static embedded wasm runtime, not the native dynamic file-loading dev runner.
- see [WASM / Web Target](../WASM.md)

Use this for local development runs and testing.
The dev runner keeps assets dynamic and reads from normal project files.
Dynamic scene/resource loading is optimized for development.
Perro CLI handles compiler/setup glue so day-to-day workflow stays simple while project structure stays flexible.
For release-like asset loading numbers, run `perro build`.
See [Performance + Flexibility Philosophy](../project/performance_philosophy.md).

### `build`

Command:

```powershell
perro build --path <project_dir> [--target native|web|android] [--triple <rust_target> | --universal-macos] [--headless] [--profile] [--console] [--demo]
```

`--headless` use native `perro_headless` feature path.

- rm `perro_app`, `perro_graphics`, `winit` frm final dep graph
- kp scripts, scenes, timers, net, CPU physics + water physics
- force CPU particle cfg
- skip window, input device, GPU + rndr loop
- sync new + old `.perro/project` + `.perro/dev_runner` manifests

Steam-enabled headless builds use Steam GameServer API, not Steam client login.

- anonymous login default
- `PERRO_STEAM_GSLT` -> token login
- `PERRO_STEAM_GAME_PORT` -> game port; default `27015`
- `PERRO_STEAM_QUERY_PORT` -> query port; default `27016`
- `PERRO_STEAM_SERVER_IP` -> bind IPv4; default `0.0.0.0`
- `PERRO_STEAM_SERVER_NAME` -> browser name
- `PERRO_STEAM_MAX_PLAYERS` -> browser cap; default `64`
- `PERRO_STEAM_LISTED=0` -> disable browser listing
- `PERRO_STEAM_SECURE=0` -> auth w/o VAC-secure mode

Server scripts use `steam::game_server` for ticket auth, player stats, and server-set achievements.

What it does:

1. Runs script compilation, like `check`.
2. Packs `res` assets through the static pipeline.
3. Generates embedded project entry files under `.perro/project`.
4. Optimizes supported assets into match tables and preparsed compile-time statics.
5. Packs unsupported/generic assets into `.perro/project/embedded/assets.perro`.
6. Builds the generated project crate in release mode from `.perro/project`.
7. With `--target native` or no `--target`, copies the built executable to `<project>/.output/`.
8. With `--target web`, exports browser bundle files to `<project>/.output/web/`.

Flags:

- `--target native|web|android`: selects native executable, browser wasm bundle, or Android app target. Default `native`.
- `--demo`: builds only the demo-visible source and applies `[demo]` config overrides.
- `--triple <rust_target>`: cross-compiles a native build for one Rust target triple. The CLI installs the Rust standard-library target when needed. The host still needs the target linker, SDK, and native libraries.
- `--universal-macos`: on macOS, builds `aarch64-apple-darwin` and `x86_64-apple-darwin`, then merges the executables with `lipo`. Per-architecture exports are kept beside the universal export.
- `--profile`: enables profile build options for the generated project bundle.
- `--console`: enables console build options for generated native project bundle.

Web target notes:

- `--console` is not supported with `perro build --target web`.
- web build uses stable `wasm32-unknown-unknown` + `wasm-bindgen --target web`.
- web output includes `index.html`, `boot.js`, `app.js`, and `app_bg.wasm`.
- see [WASM / Web Target](../WASM.md)

Android target notes:

- `--console` is not supported with `perro build --target android`.
- Android builds require an installed Android SDK/NDK; the CLI resolves them from `ANDROID_SDK_ROOT`/`ANDROID_HOME` and `ANDROID_NDK_ROOT`/`ANDROID_NDK_HOME`/`NDK_HOME` or the default platform location.

Use this to build the final executable into `<project>/.output/`.

Native cross-build examples:

```text
perro build --triple x86_64-pc-windows-msvc
perro build --triple i686-pc-windows-msvc
perro build --triple aarch64-pc-windows-msvc
perro build --triple x86_64-unknown-linux-gnu
perro build --triple i686-unknown-linux-gnu
perro build --triple aarch64-unknown-linux-gnu
perro build --triple x86_64-apple-darwin
perro build --triple aarch64-apple-darwin
perro build --universal-macos
```

Windows MSVC architecture cross-builds need the matching Visual Studio C++ tools. Linux cross-builds need the matching GNU or compatible linker and target system libraries. macOS builds need macOS/Xcode tooling; use a Mac for release signing and notarization.

### `targets`

```text
perro targets
perro targets --host windows
perro targets --host linux
perro targets --host macos
```

Without `--host`, this shows the current development OS. `READY` means the host can build the target directly. `SETUP` means the build is possible after installing the listed linker, SDK, or system libraries. `NO` means use another development OS.

| Development OS | Windows | Linux | macOS | Web | Android |
|---|---|---|---|---|---|
| Windows | ready/setup by architecture | setup | no | ready | setup |
| Linux | setup with GNU/LLVM target | ready/setup by architecture | no | ready | setup |
| macOS | setup with GNU/LLVM target | setup | ready, including universal | ready | setup |

The static pipeline packs all `res` assets.
Supported assets, such as scenes, animations, materials, particles, meshes, textures, and CSV tables, are optimized into match tables and preparsed formats as compile-time statics for efficient runtime performance.
This is main Perro trade: author normal files in dev, then let compiler pipeline reshape them for release performance.
Other `res` files are packed generically into `assets.perro`.
See [Performance + Flexibility Philosophy](../project/performance_philosophy.md).

### `dlc`

Command:

```powershell
perro dlc --name <dlc_name> [--path <project_dir>]
```

What it does:

1. Reads source from `<project_dir>/dlcs/<dlc_name>/`.
2. Generates DLC scripts crate under `.perro/dlc/<dlc_name>/scripts/`.
3. Generates DLC pack crate under `.perro/dlc/<dlc_name>/pack/`.
4. Builds both runtime-loadable modules.
5. Packs manifest, scripts module, pack module, and DLC resources into `<project_dir>/.output/dlc/<dlc_name>.dlc`.
6. Compresses final `.dlc` when it reduces file size.
7. Removes temporary `.dlc.staging` folder after successful pack.

Name rules:

- `self` is reserved for `dlc://self/...` and is rejected as a DLC name.

## New Projects And Templates

Use these commands to create projects, DLC folders, scripts, scenes, animation clips, and animation trees.

Shared rules:

- `--path` resolves to a project root for every command except `new`.
- `new --path` resolves to the parent directory that receives the new project.
- Commands with `--dlc <name>` target `dlcs/<name>/` instead of project `res/`.
- `--res` accepts `res://...` or `/...` for base game content.
- `--res` accepts `dlc://<name>/...` or `/...` for DLC content.
- `--no-open` disables VS Code open for generated files.

### `new`

Command:

```powershell
perro new [--path <parent_dir>] [--name <project_name>]
```

What it does:

1. Creates a new project directory under `<parent_dir>`.
2. Writes default project files: `project.toml`, `input_map.toml`, `deps.toml`, `res/main.scn`, scripts scaffold, and `.perro` crates.
3. Prompts to open the project in VS Code.

Notes:

- If you run this inside a directory you want to contain projects, omit `--path`.
- Add extra script Rust crates in `deps.toml` under `[dependencies]`.
- Perro merges `deps.toml` into `.perro/scripts/Cargo.toml` on `check`, `dev`, and `build`.

Examples:

```powershell
perro new --path D:\GameProjects --name MyGame
perro new --name MyGame
```

### `new_dlc`

Command:

```powershell
perro new_dlc --name <dlc_name> [--path <project_dir>] [--no-open]
```

What it does:

1. Resolves `<project_dir>`.
2. Creates `<project_dir>/dlcs/<dlc_name>/`.
3. Creates starter directories: `scenes/`, `scripts/`, `materials/`, and `meshes/`.
4. Creates starter files: `scenes/main.scn` and `scripts/script.rs`.
5. Uses `dlc://<dlc_name>/scripts/script.rs` in starter scene.

Name rules:

- `self` is reserved for `dlc://self/...` and is rejected as a DLC name.

Examples:

```powershell
perro new_dlc --name CosmeticsPack
perro new_dlc --name CosmeticsPack --path D:\GameProjects\MyGame
```

### `new_script`

Command:

```powershell
perro new_script --name <script_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>] [--no-open]
```

What it does:

1. Resolves `<project_dir>`.
2. Resolves target root: project `res/`, or project `dlcs/<name>/` with `--dlc`.
3. Resolves `<res_subdir>` relative to target root.
4. Creates a new `*.rs` script from the empty script template.
5. Opens the new file in VS Code unless `--no-open` is passed.
6. Rebuilds scripts after file creation.

Notes:

- `--name` can omit `.rs`; extension is added automatically.
- `--name` must be a file name only.

Examples:

```powershell
perro new_script --name PlayerController
perro new_script --name PlayerController --res /scripts
perro new_script --name PlayerController --path D:\GameProjects\MyGame --res res://scripts
perro new_script --name DlcController --path D:\GameProjects\MyGame --dlc ExpansionOne --res /scripts
perro new_script --name DlcController --path D:\GameProjects\MyGame --dlc ExpansionOne --res dlc://ExpansionOne/scripts
perro new_script --name PlayerController --no-open
```

### `new_scene`

Command:

```powershell
perro new_scene --name <scene_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>] [--template 2D|3D] [--no-open]
```

What it does:

1. Resolves `<project_dir>`.
2. Resolves target root: project `res/`, or project `dlcs/<name>/` with `--dlc`.
3. Resolves `<res_subdir>` relative to target root.
4. Creates a new `*.scn` scene from the selected template.
5. Opens the new file in VS Code unless `--no-open` is passed.

Notes:

- `--template` defaults to `2D`.
- Generated scenes use `$root = @main`.
- `$root` marks the scene root and can be reused as a node ref.
- `--name` can omit `.scn`; extension is added automatically.
- `--name` must be a file name only.

Examples:

```powershell
perro new_scene --name Main
perro new_scene --name Main3D --template 3D
perro new_scene --name Main --res /scenes
perro new_scene --name Main --path D:\GameProjects\MyGame --res res://scenes --template 2D
perro new_scene --name DlcIntro --path D:\GameProjects\MyGame --dlc ExpansionOne --res /scenes
perro new_scene --name Main --no-open
```

### `new_animation`

Command:

```powershell
perro new_animation --name <animation_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>] [--no-open]
```

What it does:

1. Resolves `<project_dir>`.
2. Resolves target root: project `res/`, or project `dlcs/<name>/` with `--dlc`.
3. Resolves `<res_subdir>` relative to target root.
4. Creates a new `*.panim` animation clip from the default animation template.
5. Opens the new file in VS Code unless `--no-open` is passed.

Notes:

- Defaults to `res/animations` when `--res` is omitted.
- `--name` can omit `.panim`; extension is added automatically.
- `--name` must be a file name only.

Examples:

```powershell
perro new_animation --name CubeMove
perro new_animation --name HeroRun --res /animations
perro new_animation --name HeroRun --path D:\GameProjects\MyGame --res res://animations
perro new_animation --name DlcIdle --path D:\GameProjects\MyGame --dlc ExpansionOne --res /animations
perro new_animation --name HeroRun --no-open
```

### `new_panimtree`

Command:

```powershell
perro new_panimtree --name <tree_name> [--path <project_dir>] [--res <res_subdir>] [--dlc <dlc_name>] [--no-open]
```

What it does:

1. Resolves `<project_dir>`.
2. Resolves target root: project `res/`, or project `dlcs/<name>/` with `--dlc`.
3. Resolves `<res_subdir>` relative to target root.
4. Creates a new `*.panimtree` animation tree from the default animation tree template.
5. Opens the new file in VS Code unless `--no-open` is passed.

Notes:

- Defaults to `res/animations` when `--res` is omitted.
- `--name` can omit `.panimtree`; extension is added automatically.
- `--name` must be a file name only.

Examples:

```powershell
perro new_panimtree --name HeroMove
perro new_panimtree --name HeroMove --res /animations
perro new_panimtree --name HeroMove --path D:\GameProjects\MyGame --res res://animations
perro new_panimtree --name DlcMove --path D:\GameProjects\MyGame --dlc ExpansionOne --res /animations
perro new_panimtree --name HeroMove --no-open
```

### `import_anim`

Command:

```powershell
perro import_anim <model.glb|model.gltf> --output <clip.panim> [--clip <name|index>] [--fps <fps>] [--skeleton <object_name>] [--retarget-map <map.pretarget>] [--target-rig <model.glb|model.gltf>]
```

`gltf_to_panim` and `glb_to_panim` are aliases.

What it does:

1. Loads the glTF document.
2. Selects one animation by `--clip` name or index.
3. Converts translation, rotation, and scale channels into `.panim` keyframes.
4. Writes node tracks as `Node3D` objects.
5. Writes skin joint tracks as `Skeleton3D` bone tracks on `--skeleton` object.
6. With `--retarget-map`, bakes bone aliases, rest-pose alignment, and translation policy.

Notes:

- `--clip` defaults to `0`.
- `--fps` defaults to `60`.
- `--skeleton` defaults to `Rig`.
- Scene or script bindings still map `.panim` object names to actual scene nodes.
- Bone names come from glTF node names, for example `bone["Spine"].rotation`.
- Joint rotations convert from glTF local rotations to Perro rest-relative pose deltas.
- `--target-rig` reads target joint rest poses and needs `--retarget-map`.
- Inline rest poses in the map override glTF rest poses.
- Morph target weights are ignored.

Examples:

```powershell
perro import_anim res/models/hero.glb --output res/animations/idle.panim --clip Idle
perro import_anim res/models/hero.glb --output res/animations/run.panim --clip 1 --fps 30 --skeleton HeroRig
perro import_anim res/models/mocap.glb --output res/animations/run.panim --retarget-map res/animations/humanoid.pretarget --target-rig res/models/hero.glb
```

Retarget map:

```ini
source = Rig
target = HeroRig
keep_unmapped = false
translation = root_only
root_bone = mixamorig:Hips

bone mixamorig:Hips => Hips
bone mixamorig:LeftArm => upper_arm.L

# position | rotation quaternion | scale
source_rest mixamorig:Hips = (0, 0.9, 0) | (0, 0, 0, 1) | (1, 1, 1)
target_rest Hips = (0, 1.02, 0) | (0, 0, 0, 1) | (1, 1, 1)
```

`translation` values:

- `all`: keep all bone translation tracks; default for old maps.
- `root_only`: keep only `root_bone` translation tracks.
- `none`: remove all bone translation tracks.

Rest solve maps source-rest position/scale to target-rest position/scale.

Rotation keys stay rest-relative deltas, matching `Skeleton3D` playback.

## Health And Maintenance

Use these commands to check references, run user script tests, format user scripts, lint user scripts, and remove build output.

### `doctor`

Command:

```powershell
perro doctor [--path <project_dir>]
```

What it does:

1. Loads `project.toml`.
2. Checks `project.main_scene`, `project.icon`, and `project.startup_splash`.
3. Scans text assets under `res/` and `dlcs/` for quoted `res://` and `dlc://` references.
4. Scans user scripts for likely missing `res://` and `dlc://` load paths.
5. Warns when `get_var!`, `set_var!`, or `call_method!` reference names not found in any script state or `methods!` block.
6. Warns when those dynamic calls target `ctx.id` and a typed self access path is available.
7. Reports missing scene/config references as errors and script findings as warnings.

### `format`

Command:

```powershell
perro format --path <project_dir> [--dedup]
```

What it does:

1. Resolves your path to that project's `res` root.
2. Recursively finds format targets under `res/**`.
3. Runs `rustfmt` on `*.rs` files.
4. Formats `*.scn` and `*.fur` scene files.
5. Formats key/value resource files: `*.pmat`, `*.ppart`, and `*.uistyle`.
6. With `--dedup`, creates `$varN` values for large repeated scene values used 3+ times.

### `clippy`

Command:

```powershell
perro clippy --path <project_dir>
```

What it does:

1. Resolves your path to that project's `res` root.
2. Recursively finds all `*.rs` files under `res/**`.
3. Syncs those files into `.perro/scripts`.
4. Runs `cargo clippy --all-targets -- -D warnings` for the generated scripts crate.

### `clean`

Command:

```powershell
perro clean [--path <project_dir>]
```

What it does:

1. Removes the project's `target/` directory.

## Profiling

Use these commands to record memory samples or produce flamegraphs from the dev runner.

### `bench`

Command:

```powershell
perro bench --path <project_dir> [--script <hash>] [--method <name>] [--var <name>] [-- <criterion_args>]
```

What it does:

1. Syncs project scripts into `<project_dir>/.perro/scripts`.
2. Adds a Criterion bench target for script benches.
3. Runs `cargo bench --bench perro_script_bench` from the generated scripts crate.
4. Benches script constructor/state creation and lifecycle callbacks.
5. Benches methods passed with `--method` using empty params.
6. Benches vars passed with `--var` through generated get/set state paths.

Flags:

- `--script <hash>`: filters to one script registry hash. Repeat for more scripts.
- `--method <name>`: benches a generated script method by member name. Repeat for more methods.
- `--var <name>`: benches generated state get/set by member name. Repeat for more vars.
- `-- <criterion_args>`: forwards remaining args to Criterion.

Examples:

```powershell
perro bench --path D:\GameProjects\MyGame
perro bench --path D:\GameProjects\MyGame --method tick_ai --method rebuild_path -- --sample-size 20
perro bench --path D:\GameProjects\MyGame --script 529874888977469606 --var health
```

### `mem-profile`

Command:

```powershell
perro mem-profile --path <project_dir> [--release] [--csv [csv_name]]
```

What it does:

1. Runs the same scripts build pipeline as `check`.
2. Builds the project-local dev runner with `profile` feature enabled.
3. Launches dev runner with memory profiling enabled: `PERRO_MEM_PROFILE=1`.
4. Writes batch memory samples CSV in `<project_dir>/.output/profiling/`.

Flags:

- `--release`: builds and runs release dev runner binary.
- `--csv [csv_name]`: custom output file name under `.output/profiling/`.

### `spec`

```text
perro spec --path <project_dir> [--target-fps <fps>]
```

Runs the project in release mode and records frame data plus bounded memory
summaries until the game closes. It writes `report.json`, `report.md`,
`steam.txt`, `frames.csv`, `samples.csv`, and `markers.jsonl` under
`.output/profiling/spec/`.

Add test-path markers in scripts:

```rust
spec_begin!("boss fight");
spec_end!("boss fight");
spec_point!("streaming gate");
```

Marker recording is enabled only for the spec scripts feature. Normal project
builds strip marker calls and the marker implementation.

The first report uses CPU-side update/render timings and process RSS. CPU/GPU
equivalents are relative to the test PC and remain estimates until a benchmark
score database and GPU timestamp queries are available.

### `flamegraph`

Command:

```powershell
perro flamegraph --path <project_dir> [--profile] [--root]
```

What it does:

1. Runs the same scripts build pipeline as `check`.
2. Checks `cargo flamegraph` availability.
3. Auto-runs `cargo install flamegraph` when missing.
4. Runs `cargo flamegraph --release` from `<project_dir>/.perro/dev_runner`.
5. Sets `CARGO_TARGET_DIR=<project_dir>/target`.
6. Forces debug symbols for release profiling with `CARGO_PROFILE_RELEASE_DEBUG=true`.
7. Passes project path through to dev runner with `-- --path <project_dir>`.

Flags:

- `--profile`: enables dev runner `profile` feature when building/profiling.
- `--root`: forwards `--root` to `cargo flamegraph`.

Notes:

- Linux: install `perf` (`linux-tools` package family).
- macOS: install `dtrace`/Xcode command line tools.
- Windows: CLI asks to relaunch elevated before flamegraph when shell lacks admin rights.
- Windows: `cargo-flamegraph` uses `blondie` and often needs elevated PowerShell/Terminal.
- Windows: if error includes `NotAnAdmin`, rerun as Administrator.
- Windows fallback: prefer WSL/Linux profiling for full flamegraph support.

Examples:

```powershell
perro flamegraph --path D:\GameProjects\MyGame
perro flamegraph --path D:\GameProjects\MyGame --profile
```

## Install

### `install`

Command:

```powershell
perro install
```

What it does:

1. Adds/updates a `perro` shell function in your profile.
2. On Windows, updates PowerShell profiles.
3. On Linux, updates POSIX shell profiles: `~/.profile`, `~/.bashrc`, `~/.zshrc`.
4. Function builds source-mode CLI, copies it to temp, then runs args.

After running install, open a new shell or source your updated profile.

Examples:

```powershell
perro new --path D:\GameProjects --name MyGame
perro check --path D:\GameProjects\MyGame
```
