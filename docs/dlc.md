# DLC Guide

This document explains how Perro DLC works in authoring, export, and runtime mount flow.

## Core Idea

- DLC is always runtime-loaded.
- Base game content stays in `res://`.
- DLC content lives under `dlc://<name>/`.
- DLC is authored in project source, exported to `.dlc`, and mounted at startup.

## Authoring Layout

Inside project root:

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
- You cannot create/build/mount DLC named `self` (case-insensitive).

Creates:

- `dlcs/NAME/scenes/main.scn`
- `dlcs/NAME/scripts/script.rs`
- `dlcs/NAME/materials/`
- `dlcs/NAME/meshes/`

You can also create files inside DLC with `--dlc`:

```powershell
perro new_script --name Foo --dlc NAME --res /scripts
perro new_scene --name Intro --dlc NAME --res /scenes
perro new_animation --name Idle --dlc NAME --res /animations
```

## Path Rules

- Base content: `res://...`
- DLC content: `dlc://NAME/...`
- Inside DLC-authored content, `dlc://self/...` resolves to current DLC mount.

Reference behavior:

- Base -> DLC: allowed (`res` can reference `dlc://NAME/...`).
- DLC -> base: allowed (`dlc://NAME/...` can reference `res://...`).
- DLC -> same DLC: allowed (`dlc://self/...` or `dlc://NAME/...`).
- DLC -> other DLC: allowed (`dlc://OtherName/...`).

If referenced path is missing at runtime, lookup/load fails with normal resource/script load error.

## Build + Export

Build one DLC:

```powershell
perro dlc --name NAME
```

Pipeline:

1. Reads `project/dlcs/NAME/`.
2. Generates DLC scripts crate:
   - `.perro/dlc/NAME/scripts/`
3. Generates DLC pack crate:
   - `.perro/dlc/NAME/pack/`
4. Builds both runtime-loadable modules.
5. Packs manifest + modules + DLC resources into:
   - `.output/dlc/NAME.dlc`
6. Compresses the final `.dlc` when it reduces file size.
7. Removes the temporary `.dlc.staging` folder after a successful pack.

Important split:

- `.perro/scripts/` => base game scripts only.
- `.perro/dlc/NAME/scripts/` => DLC `NAME` scripts only.
- `.perro/dlc/NAME/pack/` => DLC `NAME` pack lookup module only.

## Runtime Mount

On startup, runtime mounts DLC automatically.

Dev source mount:

- Scans `project/dlcs/*`.
- Mounts each as `dlc://NAME/...`.
- Uses generated DLC scripts from `.perro/dlc/NAME/scripts/`.

Release installed mount:

- Scans install directory:
  - `LocalAppData/<ProjectName>/dlc/*.dlc`
- Loads manifest + scripts module + pack module from each `.dlc`.
- Decompresses compressed `.dlc` packs in memory during mount.
- Mounts each as `dlc://NAME/...`.

`user://` data path uses:

- `LocalAppData/<ProjectName>/data`

So DLC install dir is sibling path:

- `LocalAppData/<ProjectName>/dlc`

## Auto Scan + Rescan

- Startup auto-scan is built-in.
- Manual runtime rescan helper API is not exposed yet.
- A future helper (for example `scan_dlc()`) can be added to trigger remount without restart.
