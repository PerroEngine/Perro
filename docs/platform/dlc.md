# DLC Guide

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Practical Example | [Practical Example](#practical-example) |
| Core Idea | [Core Idea](#core-idea) |
| Authoring Layout | [Authoring Layout](#authoring-layout) |
| Path Rules | [Path Rules](#path-rules) |
| Build + Export | [Build + Export](#build-export) |
| Runtime Mount | [Runtime Mount](#runtime-mount) |
| Auto Scan + Rescan | [Auto-Scan--Rescan](#auto-scan-rescan) |

## Purpose

DLC lets you ship extra content, such as scenes, scripts, materials, and meshes,
after launch without rebuilding the base game. Each pack is authored inside the
project, exported to a single `.dlc` file, and mounted at startup under its own
`dlc://<name>/` path space. Base content stays in `res://`; DLC content lives
beside it and can reference across boundaries, so add-ons integrate with the
shipped game instead of replacing it.

## Use Cases

- Cosmetic pack shipped after launch: a `dlc://skins/` pack of materials and
  meshes that the base game references from `res://`.
- Expansion campaign: new levels and scripts under `dlc://episode2/scenes/`,
  loaded when the player owns the pack.
- Free content drop: a small `.dlc` dropped into the install's `dlc` folder and
  mounted automatically on the next launch.
- Self-contained add-on: DLC-authored content uses `dlc://self/...` so it keeps
  working regardless of the pack's final name.
- Cross-pack content: one pack referencing another with `dlc://OtherName/...`.

## Practical Example

DLC mounts automatically at startup, so game code just references `dlc://` paths
like any other resource:

```rust
lifecycle!({
    fn on_shop_open(&self, ctx: &mut ScriptContext<'_, API>) {
        // Load a scene shipped in the "cosmetics" DLC, if it is installed.
        match scene_load!(ctx.run, "dlc://cosmetics/scenes/shop.scn") {
            Ok(_node) => { /* shop opened */ }
            Err(_err) => { /* pack not installed; show base UI */ }
        }
    }
});
```

If a referenced path is missing at runtime (the pack is not installed), the
load fails with a normal resource/script load error, which you handle like any
other missing asset.

## Core Idea

- DLC is always runtime-loaded.
- Base game content stays in `res://`.
- DLC content lives under `dlc://<name>/`.
- DLC is authored in project source, exported to `.dlc`, and mounted at startup.

## Authoring Layout

Inside the project root:

```text
project/
  res/
  dlcs/
    NAME/
      scenes/
      scripts/
      materials/
      meshes/
      ...
```

CLI helper:

```powershell
perro new_dlc --name NAME
```

Reserved names:

- `self` is reserved for `dlc://self/...` path resolution.
- You cannot create, build, or mount a DLC named `self` (case-insensitive).

`new_dlc` creates:

- `dlcs/NAME/scenes/main.scn`
- `dlcs/NAME/scripts/script.rs`
- `dlcs/NAME/materials/`
- `dlcs/NAME/meshes/`

Create files inside a DLC with `--dlc`:

```powershell
perro new_script --name Foo --dlc NAME --res /scripts
perro new_scene --name Intro --dlc NAME --res /scenes
perro new_animation --name Idle --dlc NAME --res /animations
```

## Path Rules

- Base content: `ResPath::new("res://...")`
- DLC content: `ResPath::new("dlc://NAME/...")`
- User data: `ResPath::new("user://...")`
- Inside DLC-authored content, `ResPath::new("dlc://self/...")` resolves to the current DLC mount.

See [ResPath](../resources/respath.md).

Reference behavior:

- Base -> DLC: allowed (`res` can reference `dlc://NAME/...`).
- DLC -> base: allowed (`dlc://NAME/...` can reference `res://...`).
- DLC -> same DLC: allowed (`dlc://self/...` or `dlc://NAME/...`).
- DLC -> other DLC: allowed (`dlc://OtherName/...`).

If a referenced path is missing at runtime, lookup/load fails with a normal
resource/script load error.

## Build + Export

Build one DLC:

```powershell
perro dlc --name NAME
```

Pipeline:

1. Reads `project/dlcs/NAME/`.
2. Generates the DLC scripts crate:
   - `.perro/dlc/NAME/scripts/`
3. Generates the DLC pack crate:
   - `.perro/dlc/NAME/pack/`
4. Builds both runtime-loadable modules.
5. Packs the manifest, modules, and DLC resources into:
   - `.output/dlc/NAME.dlc`
6. Compresses the final `.dlc` when it reduces file size.
7. Removes the temporary `.dlc.staging` folder after a successful pack.

Important split:

- `.perro/scripts/` => base game scripts only.
- `.perro/dlc/NAME/scripts/` => DLC `NAME` scripts only.
- `.perro/dlc/NAME/pack/` => DLC `NAME` pack lookup module only.

## Runtime Mount

On startup, the runtime mounts DLC automatically.

Dev source mount:

- Scans `project/dlcs/*`.
- Mounts each as `dlc://NAME/...`.
- Uses generated DLC scripts from `.perro/dlc/NAME/scripts/`.

Release installed mount:

- Scans the install directory:
  - `LocalAppData/<ProjectName>/dlc/*.dlc`
- Loads the manifest, scripts module, and pack module from each `.dlc`.
- Decompresses compressed `.dlc` packs in memory during mount.
- Mounts each as `dlc://NAME/...`.

The `user://` data path uses:

- `LocalAppData/<ProjectName>/data`

So the DLC install directory is a sibling path:

- `LocalAppData/<ProjectName>/dlc`

## Auto-Scan + Rescan

- Startup auto-scan is built in.
- A manual runtime rescan helper API is not exposed yet.
- A future helper (for example `scan_dlc()`) can be added to trigger a remount without restart.
