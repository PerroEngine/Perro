# Perro Editor

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Status | [Status](#status) |
| Start | [Start](#start) |
| Shell | [Shell](#shell) |
| Workflow | [Workflow](#workflow) |
| Main Shortcuts | [Main Shortcuts](#main-shortcuts) |
| Safety | [Safety](#safety) |
| Animation And GLB | [Animation And GLB](#animation-and-glb) |
| Release Gate | [Release Gate](#release-gate) |

## Purpose

The Perro editor is a visual authoring tool — itself a Perro project in `perro_editor` — for building and previewing scenes without hand-writing `.scn` text. It gives you a node tree, 2D/3D/UI viewports, an inspector for node fields and script vars, animation editing, GLB inspection, undo/redo, file watching, and multi-scene tabs. Lay out and tweak scenes visually here, then run and ship them with the CLI. It is an in-development milestone: play/build launch and a full import pipeline are still follow-up work.

## Use Cases

- **Open or create a project visually.** Launch `cargo run -p perro_cli -- dev --path perro_editor`, then pick a folder with `project.toml` or create one in the project manager.
- **Author a scene without editing text.** Add or select nodes in the Scene panel, edit fields in the Inspector, and save with `Ctrl+S` (or `Ctrl+Shift+S` for all dirty scenes).
- **Preview 2D, 3D, and UI in place.** Switch viewports with `1` / `2` / `3` and frame the selection with `F`.
- **Edit animations with live preview.** Select an `AnimationPlayer` to open the animation dock for clip selection, playhead control, key insert/delete, and interpolation changes.
- **Inspect an imported model.** Open a `.glb` or `.gltf` to switch to the 3D model viewer with mesh, material, animation, skeleton, and texture summaries.
- **Run the editor's own release gate.** `check`, `clippy`, and `test` through `perro_cli --path perro_editor` before landing editor changes.

## Status

The editor is an in-development Perro project in `perro_editor`.

The current milestone supports project selection, scene and asset authoring, 2D/3D/UI previews, inspector edits, animation editing, GLB inspection, undo/redo, file watching, and multi-scene tabs.

Play/build launch, a full import pipeline, and stable release packaging remain follow-up work.

## Start

Run from the repository root:

```powershell
cargo run -p perro_cli -- dev --path perro_editor
```

The project manager can open a folder with `project.toml` or create a project. Recent projects are stored under `user://` and shown on the next launch.

## Shell

| Area | Use |
| --- | --- |
| Activity rail | Switch scene and GLB workspaces. |
| Left panel | Browse scene nodes or project files. |
| Center | Edit and preview UI, 2D, or 3D scenes. |
| Inspector | Edit selected node fields, refs, and script vars. |
| Bottom dock | Inspect output and animation state. |
| Scene tabs | Keep independent scene docs, selections, undo stacks, and dirty state. |

Use the command palette with `Ctrl+Shift+P`. Search terms can match any words in a command label.

## Workflow

1. Open or create a project.
2. Select a `.scn` file in Files and open it.
3. Add or select nodes in Scene.
4. Edit fields in Inspector or use viewport tools.
5. Save with `Ctrl+S` or save all open dirty scenes with `Ctrl+Shift+S`.

The asset browser watches `res/` plus project input and localization files. Script changes invalidate inspector schema caches. Clean open scenes reload after an external edit. A changed scene with unsaved editor work stays in memory and reports `external change pending` in Output.

## Main Shortcuts

| Shortcut | Action |
| --- | --- |
| `Ctrl+Shift+P` | Open command palette. |
| `Ctrl+S` / `Ctrl+Shift+S` | Save scene / save all scenes. |
| `Ctrl+Z` / `Ctrl+Y` | Undo / redo active scene. |
| `Ctrl+N` / `Ctrl+Shift+N` | Add child / sibling node. |
| `Ctrl+O` | Open selected file or selected node asset ref. |
| `Ctrl+W` / `Ctrl+Shift+W` | Close active / all scene tabs. |
| `Ctrl+Tab` | Cycle scene tabs. |
| `Ctrl+R` | Refresh project assets. |
| `1` / `2` / `3` | Switch 2D / 3D / UI viewport. |
| `F` | Frame selected node. |
| `F2` | Rename current selection. |
| `Delete` | Delete selected node or asset. |
| `Ctrl+Shift+F11` | Toggle distraction-free layout. |

Shortcuts pause while a text box owns focus. `Escape` closes active popups or cancels pending confirmation.

## Safety

- Dirty scene tabs show a marker and keep separate in-memory docs.
- Closing a dirty tab asks for a second close action, saves, and closes only after the save succeeds.
- Asset and folder deletion needs a second confirmation action.
- Dirty assets cannot be renamed or deleted until saved.
- External changes do not reload a dirty copy of the same scene.
- Confirmation expires after a short timeout or selection change.

Save or copy work before resolving an external-change conflict. The editor keeps its in-memory version and does not merge scene text.

## Animation And GLB

Select an `AnimationPlayer` to open its animation workflow. The dock supports clip selection, playhead control, key insertion/deletion, interpolation/ease changes, and undo/redo.

Opening a `.glb` or `.gltf` switches to the 3D model viewer. The viewer frames the model and exposes mesh, material, animation, skeleton, and texture summaries. Bracket shortcuts cycle embedded refs.

## Release Gate

Run from the repository root:

```powershell
cargo run -p perro_cli -- check --path perro_editor
cargo run -p perro_cli -- clippy --path perro_editor
cargo run -p perro_cli -- test --path perro_editor
```

`clippy` also runs project doctor checks for config, scene, asset, and script refs. CI runs all three editor commands on Linux.

For a manual smoke pass:

```powershell
cargo run -p perro_cli -- dev --path perro_editor --timings --ui-profile
```

Check project manager launch, scene open/save, tab switching, node add/delete confirmation, asset refresh, viewport mode switching, inspector edits, animation dock, and GLB viewer.
